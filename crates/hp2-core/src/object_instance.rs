//! Instance reflection, console invocation, property export and execution policy.
//! Handles are copied values; native iterators and scratch buffers are stack-owned.
//! No Rust reference to an SDK object spans virtual dispatch or script reentry.
use crate::vm_frame;
use std::{ffi::c_void, ptr};

type Result<T, E = i32> = std::result::Result<T, E>;
const CPF_EXPORT_OBJECT: usize = 0x8;
const CPF_OPTIONAL_PARM: usize = 0x10;
const CPF_PARM: usize = 0x80;
const CPF_RETURN_PARM: usize = 0x400;
const FUNC_EXEC: usize = 0x200;
const HASH_MASK: usize = 255;
// UnNames.h assigns consecutive enum ordinals, not REGISTER_NAME's legacy IDs.
const PROBE_MIN: i32 = 197;
const PROBE_MAX: i32 = 261;

/// Returns the active probe-mask bit only for source probe names.
pub(crate) fn probe_bit(name: i32) -> Option<u32> {
    (PROBE_MIN..PROBE_MAX).contains(&name).then_some((name - PROBE_MIN) as u32)
}

/// Source UObject::IsProbing policy: ordinary/out-of-range events and objects
/// without an active state frame are always accepted.
pub(crate) fn probe_enabled(name: i32, mask: Option<u64>) -> bool {
    probe_bit(name).is_none_or(|bit| mask.is_none_or(|mask| mask & (1_u64 << bit) != 0))
}


/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectInstanceOperation {
    Export,
    Initialize,
    Probe,
    ConsoleFind,
    ConsoleRun,
    Field,
    Function,
    CheckedFunction,
    State,
    ParseParms,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectInstanceLeaf {
    Class,
    Object,
    Frame,
    StateNode,
    ProbeMask,
    PublishFrame,
    SetHasStack,
    HashHead,
    HashNext,
    NameIndex,
    CastFunction,
    CastState,
    Scriptable,
    MissingFunction,
    ParseToken,
    TokenName,
    FunctionFlags,
    Function,
    Executor,
    PropertiesBegin,
    PropertiesNext,
    IteratorStruct,
    PropertyFlags,
    ObjectProperty,
    PropertyClass,
    WriteExecutor,
    ParseNext,
    Import,
    BadProperty,
    ProcessEvent,
    DestroyValue,
    ExportClass,
    Port,
    SetExportName,
    ArrayDim,
    DiffClass,
    DiffData,
    ExportValue,
    ObjectValue,
    Tagged,
    ExportObject,
    TagObject,
    LogScalar,
    LogArray,
    HasParms,
    StaticObjectClass,
    PropertyOwner,
    ParsePropertyValue,
    ImportPropertyValue,
    PropertyName,
    StaticPropertyClass,
    InheritanceSuper,
}

#[repr(C)]
pub struct Hp2ObjectInstanceServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(
        *mut c_void,
        Hp2ObjectInstanceLeaf,
        usize,
        usize,
        usize,
        *mut usize,
    ) -> i32,
}

struct Native<'a>(&'a Hp2ObjectInstanceServices);
impl Native<'_> {
    fn call(&self, op: Hp2ObjectInstanceLeaf, a: usize, b: usize, c: usize) -> Result<usize> {
        let mut value = 0;
        let status = unsafe { (self.0.call)(self.0.context, op, a, b, c, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
    fn get(&self, op: Hp2ObjectInstanceLeaf, a: usize) -> Result<usize> {
        self.call(op, a, 0, 0)
    }
    fn field(&self, name: i32, global: bool) -> Result<usize> {
        console_field(&mut NativeConsoleOps(self), name, global)
    }

    fn initialize(&self) -> Result<usize> {
        use Hp2ObjectInstanceLeaf::*;
        let previous = self.get(Frame, 0)? as *mut hp2_abi::engine::generated::FStateFrame;
        let object = self.get(Object, 0)? as *mut c_void;
        let class = self.get(Class, 0)? as *mut c_void;
        // SAFETY: the bridge supplies live object/class handles and owns the frame slot.
        unsafe {
            initialize_state_frame(
                object,
                class,
                previous,
                |frame| self.get(PublishFrame, frame as usize).map(|_| ()),
                || self.get(SetHasStack, 0).map(|_| ()),
            )?;
        }
        Ok(0)
    }

    fn parse_parms(&self) -> Result<usize> {
        use Hp2ObjectInstanceLeaf::*;
        if self.get(HasParms, 0)? == 0 {
            return Ok(0);
        }
        let class = self.get(Class, 0)?;
        let object_class = self.get(StaticObjectClass, 0)?;
        let mut matcher = std::mem::MaybeUninit::<[i32; crate::name::NAME_SIZE + 1]>::uninit();
        // SAFETY: the bridge supplies live, compiler-probed reflection objects.
        // Keep native virtual superclass dispatch, but own field traversal here.
        let mut fields = unsafe {
            crate::class_residue::FieldIter::new(
                class as *mut std::ffi::c_void,
                |field| {
                    let target = self.get(StaticPropertyClass, 0)?;
                    Ok(crate::object_model::is_a(field, target as *mut std::ffi::c_void))
                },
                |struct_| {
                    self.get(InheritanceSuper, struct_ as usize)
                        .map(|parent| parent as *mut std::ffi::c_void)
                },
            )?
        };
        while !fields.field().is_null() {
            let property = fields.field() as usize;
            if self.get(PropertyOwner, property)? != object_class {
                let name = self.get(PropertyName, property)? as *const i32;
                let matched = {
                    // SAFETY: native FName text is live, NUL-terminated, and
                    // shorter than NAME_SIZE. Copy before callbacks can reenter.
                    unsafe {
                        let length = libc::wcslen(name);
                        let text = matcher.as_mut_ptr().cast::<i32>();
                        std::ptr::copy_nonoverlapping(name, text, length);
                        *text.add(length) = i32::from(b'=');
                        *text.add(length + 1) = 0;
                        self.call(ParsePropertyValue, property, text as usize, 0)?
                    }
                };
                if matched != 0 {
                    self.get(ImportPropertyValue, property)?;
                }
            }
            // SAFETY: the current field remains live through import callbacks.
            unsafe { fields.advance()? };
        }
        Ok(0)
    }
    fn probe(&self, name: i32) -> Result<usize> {
        use Hp2ObjectInstanceLeaf::*;
        if probe_bit(name).is_none() {
            return Ok(1);
        }
        let frame = self.get(Frame, 0)?;
        let mask = if frame == 0 {
            None
        } else {
            Some(self.get(ProbeMask, frame)? as u64)
        };
        Ok(usize::from(probe_enabled(name, mask)))
    }
    fn console_find(&self) -> Result<usize> {
        console_find_with(&mut NativeConsoleOps(self))
    }
    fn console_run(&self) -> Result<usize> {
        console_run_with(&mut NativeConsoleOps(self))
    }

}
trait ConsoleExecOps {
    type Error;
    fn call(
        &mut self,
        operation: Hp2ObjectInstanceLeaf,
        a: usize,
        b: usize,
        c: usize,
    ) -> std::result::Result<usize, Self::Error>;
    fn cleanup_after_error(&self) -> bool { false }
}

struct NativeConsoleOps<'native, 'services>(&'native Native<'services>);
impl ConsoleExecOps for NativeConsoleOps<'_, '_> {
    type Error = i32;
    fn call(&mut self, operation: Hp2ObjectInstanceLeaf, a: usize, b: usize, c: usize) -> Result<usize> {
        self.0.call(operation, a, b, c)
    }
}

fn console_field<O: ConsoleExecOps>(ops: &mut O, name: i32, global: bool) -> std::result::Result<usize, O::Error> {
    use Hp2ObjectInstanceLeaf::*;
    let frame = ops.call(Frame, 0, 0, 0)?;
    let state = if frame == 0 { 0 } else { ops.call(StateNode, frame, 0, 0)? };
    let class = ops.call(Class, 0, 0, 0)?;
    find_state_first(name, global, state, class, |structure, name| {
        let head = ops.call(HashHead, structure, name as usize & HASH_MASK, 0)?;
        console_chain(ops, head, name)
    })
}

pub(crate) fn find_state_first<E>(
    name: i32,
    global: bool,
    state: usize,
    class: usize,
    mut find: impl FnMut(usize, i32) -> std::result::Result<usize, E>,
) -> std::result::Result<usize, E> {
    if !global && state != 0 {
        let field = find(state, name)?;
        if field != 0 {
            return Ok(field);
        }
    }
    find(class, name)
}

fn console_chain<O: ConsoleExecOps>(ops: &mut O, mut field: usize, name: i32) -> std::result::Result<usize, O::Error> {
    use Hp2ObjectInstanceLeaf::*;
    while field != 0 {
        if ops.call(NameIndex, field, 0, 0)? as i32 == name { return Ok(field); }
        field = ops.call(HashNext, field, 0, 0)?;
    }
    Ok(0)
}

fn console_find_with<O: ConsoleExecOps>(ops: &mut O) -> std::result::Result<usize, O::Error> {
    use Hp2ObjectInstanceLeaf::*;
    if ops.call(Scriptable, 0, 0, 0)? == 0 || ops.call(ParseToken, 0, 0, 0)? == 0 { return Ok(0); }
    let name = ops.call(TokenName, 0, 0, 0)? as i32;
    if name == 0 { return Ok(0); }
    let field = console_field(ops, name, false)?;
    let function = ops.call(CastFunction, field, 0, 0)?;
    if function == 0 || ops.call(FunctionFlags, function, 0, 0)? & FUNC_EXEC == 0 { return Ok(0); }
    Ok(function)
}

fn console_parameter<O: ConsoleExecOps>(ops: &mut O, property: usize) -> std::result::Result<bool, O::Error> {
    Ok(property != 0
        && ops.call(Hp2ObjectInstanceLeaf::PropertyFlags, property, 0, 0)?
            & (CPF_PARM | CPF_RETURN_PARM) == CPF_PARM)
}

fn console_run_with<O: ConsoleExecOps>(ops: &mut O) -> std::result::Result<usize, O::Error> {
    let result = console_run_body(ops);
    match result {
        Ok(handled) => {
            console_destroy_parameters(ops)?;
            Ok(handled)
        }
        Err(error) if ops.cleanup_after_error() => {
            console_destroy_parameters(ops)?;
            Err(error)
        }
        Err(error) => Err(error),
    }
}

fn console_run_body<O: ConsoleExecOps>(ops: &mut O) -> std::result::Result<usize, O::Error> {
    use Hp2ObjectInstanceLeaf::*;
    let function = ops.call(Function, 0, 0, 0)?;
    let mut property = ops.call(PropertiesBegin, function, 0, 0)?;
    let mut count = 0;
    let mut failed = false;
    while console_parameter(ops, property)? {
        let mut implicit = false;
        if count == 0 {
            let executor = ops.call(Executor, 0, 0, 0)?;
            if executor != 0 {
                let object_property = ops.call(ObjectProperty, property, 0, 0)?;
                let property_class = if object_property != 0 {
                    ops.call(PropertyClass, object_property, 0, 0)?
                } else { 0 };
                if object_property != 0 && unsafe {
                    crate::object_model::is_a(executor as *const c_void, property_class as *const c_void)
                } {
                    ops.call(WriteExecutor, property, executor, 0)?;
                    implicit = true;
                }
            }
        }
        if !implicit {
            ops.call(ParseNext, 0, 0, 0)?;
            if ops.call(Import, property, 0, 0)? == 0 {
                if ops.call(PropertyFlags, property, 0, 0)? & CPF_OPTIONAL_PARM == 0 {
                    ops.call(BadProperty, property, 0, 0)?;
                    failed = true;
                }
                break;
            }
        }
        property = ops.call(PropertiesNext, 0, 0, 0)?;
        count += 1;
    }
    if !failed { ops.call(ProcessEvent, function, 0, 0)?; }
    Ok(1)
}

fn console_destroy_parameters<O: ConsoleExecOps>(ops: &mut O) -> std::result::Result<(), O::Error> {
    use Hp2ObjectInstanceLeaf::*;
    let function = ops.call(Function, 0, 0, 0)?;
    let mut property = ops.call(PropertiesBegin, function, 0, 0)?;
    while console_parameter(ops, property)? {
        ops.call(DestroyValue, property, 0, 0)?;
        property = ops.call(PropertiesNext, 0, 0, 0)?;
    }
    Ok(())
}

impl Native<'_> {
    fn export(&self) -> Result<usize> {
        use Hp2ObjectInstanceLeaf::*;
        let mut property = self.get(PropertiesBegin, self.get(ExportClass, 0)?)?;
        while property != 0 {
            if self.get(Port, property)? != 0 {
                self.get(SetExportName, property)?;
                let mut index = 0i32;
                while index < self.get(ArrayDim, property)? as i32 {
                    let diff_class = self.get(DiffClass, 0)?;
                    let diff = if diff_class != 0
                        && unsafe {
                            crate::object_model::is_child_of(
                                diff_class as *const c_void,
                                self.get(IteratorStruct, 0)? as *const c_void,
                            )
                        }
                    {
                        self.get(DiffData, 0)?
                    } else {
                        0
                    };
                    if self.call(ExportValue, property, index as usize, diff)? != 0 {
                        if self.get(ObjectProperty, property)? != 0
                            && self.get(PropertyFlags, property)? & CPF_EXPORT_OBJECT != 0
                        {
                            let object = self.call(ObjectValue, property, index as usize, 0)?;
                            if object != 0 && self.get(Tagged, object)? == 0 {
                                self.get(ExportObject, object)?;
                                self.get(TagObject, object)?;
                            }
                        }
                        let log = if self.get(ArrayDim, property)? == 1 {
                            LogScalar
                        } else {
                            LogArray
                        };
                        self.call(log, property, index as usize, 0)?;
                    }
                    index += 1;
                }
            }
            property = self.get(PropertiesNext, 0)?;
        }
        Ok(0)
    }
}
/// Shared initialization sequence for UObject::InitExecution and direct Core owners.
///
/// # Safety
/// `object`, `class`, and a non-null `previous` frame are live records owned by
/// this engine thread; publish must store the returned frame in object state.
pub(crate) unsafe fn initialize_state_frame<E>(
    object: *mut c_void,
    class: *mut c_void,
    previous: *mut hp2_abi::engine::generated::FStateFrame,
    publish: impl FnOnce(*mut hp2_abi::engine::generated::FStateFrame) -> Result<(), E>,
    set_has_stack: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    if !previous.is_null() {
        unsafe { vm_frame::free_state(previous) };
    }
    let frame = vm_frame::allocate_state(object, class);
    publish(frame)?;
    set_has_stack()
}

/// Shared ParseParms matching/import loop for reflected property owners.
///
/// Property names and source text are copied snapshots, so import hooks may
/// reenter Core without retaining a metadata or object-body borrow.
pub(crate) fn parse_property_assignments<E>(
    source: &[i32],
    properties: impl IntoIterator<Item = (usize, Vec<i32>)>,
    mut import: impl FnMut(usize, &[i32]) -> Result<(), E>,
) -> Result<(), E> {
    for (property, mut key) in properties {
        if key.last() == Some(&0) { key.pop(); }
        key.push(i32::from(b'='));
        key.push(0);
        let mut value = [0i32; 1024];
        // SAFETY: source and key are NUL-terminated TCHAR strings and value is writable.
        let matched = unsafe {
            crate::text_parse::parse_string(
                source.as_ptr(),
                key.as_ptr(),
                value.as_mut_ptr(),
                value.len() as i32,
            )
        };
        if matched {
            let length = value.iter().position(|&unit| unit == 0).unwrap_or(value.len());
            let mut terminated = Vec::with_capacity(length + 1);
            terminated.extend_from_slice(&value[..length]);
            terminated.push(0);
            import(property, &terminated)?;
        }
    }
    Ok(())
}

/// # Safety
/// Native context, callbacks and their handles stay live through synchronous
/// reentry. Every callback captures C++ exceptions and returns their status.
/// Initialize's existing persistent frame is null or uniquely owned by
/// vm_frame::allocate_state; publication transfers that ownership to the object.
/// ParseParms property names are live NUL-terminated native FName strings,
/// shorter than name::NAME_SIZE; parser callbacks do not retain matcher pointers.
pub unsafe fn execute(
    services: &Hp2ObjectInstanceServices,
    operation: Hp2ObjectInstanceOperation,
    name: i32,
    global: i32,
) -> Result<usize> {
    use Hp2ObjectInstanceLeaf::*;
    let n = Native(services);
    match operation {
        Hp2ObjectInstanceOperation::Export => n.export(),
        Hp2ObjectInstanceOperation::Initialize => n.initialize(),
        Hp2ObjectInstanceOperation::ParseParms => n.parse_parms(),
        Hp2ObjectInstanceOperation::Probe => n.probe(name),
        Hp2ObjectInstanceOperation::ConsoleFind => n.console_find(),
        Hp2ObjectInstanceOperation::ConsoleRun => n.console_run(),
        Hp2ObjectInstanceOperation::Field => n.field(name, global != 0),
        Hp2ObjectInstanceOperation::Function => n.get(CastFunction, n.field(name, global != 0)?),
        Hp2ObjectInstanceOperation::CheckedFunction => {
            if n.get(Scriptable, 0)? == 0 {
                return Ok(0);
            }
            let function = n.get(CastFunction, n.field(name, global != 0)?)?;
            if function == 0 {
                n.get(MissingFunction, name as usize)?;
            }
            Ok(function)
        }
        Hp2ObjectInstanceOperation::State => n.get(CastState, n.field(name, true)?),
    }
}

/// Executes the existing ConsoleFind/ConsoleRun policy over canonical Core records.
///
/// # Safety
/// `object`, `executor`, and the NUL-terminated command must remain live through
/// synchronous VM reentry; all function/property metadata must belong to `access`.
pub unsafe fn execute_core_console(
    access: crate::core_access::CoreAccess<'_>,
    object: *mut hp2_abi::engine::generated::UObject,
    executor: *mut hp2_abi::engine::generated::UObject,
    command: *const u32,
    output: &mut dyn FnMut(&[u32]) -> crate::core_access::RuntimeResult<()>,
) -> crate::core_access::RuntimeResult<bool> {
    let mut process_event = |object, function, parameters| unsafe {
        access.execute_event(object, function, parameters, ptr::null_mut())
    };
    unsafe {
        execute_core_console_with(access, object, executor, command, output, &mut process_event)
    }
}

/// Executes canonical ScriptConsoleExec while allowing source-specific
/// ProcessEvent gates (such as AActor's BeginPlay guard) to run before VM entry.
///
/// # Safety
/// Object, executor, command and reflected metadata remain live through reentry;
/// `process_event` must dispatch the same canonical object/function/parameter frame.
pub unsafe fn execute_core_console_with<'access, 'borrow>(
    access: crate::core_access::CoreAccess<'access>,
    object: *mut hp2_abi::engine::generated::UObject,
    executor: *mut hp2_abi::engine::generated::UObject,
    command: *const u32,
    output: &'borrow mut dyn FnMut(&[u32]) -> crate::core_access::RuntimeResult<()>,
    process_event: &'borrow mut dyn FnMut(
        *mut hp2_abi::engine::generated::UObject,
        *mut hp2_abi::engine::generated::UFunction,
        *mut c_void,
    ) -> crate::core_access::RuntimeResult<()>,
) -> crate::core_access::RuntimeResult<bool> {
    use crate::core_access::RuntimeError;
    use hp2_abi::engine::generated::{UFunction, UObject};
    if object.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid canonical ScriptConsoleExec arguments"));
    }
    let function_class = access.native_class("Function")?;
    let property_class = access.native_class("Property")?;
    let object_property_class = access.native_class("ObjectProperty")?;
    let mut state = CoreConsoleState {
        access,
        object,
        executor,
        command: command.cast(),
        token: [0; crate::name::NAME_SIZE],
        function: ptr::null_mut(),
        current_struct: ptr::null_mut(),
        current_property: ptr::null_mut(),
        parameter_bytes: 0,
        function_class,
        property_class,
        object_property_class,
        parameters: Vec::new(),
        output,
        dispatch: process_event,
    };
    let function = console_find_with(&mut state)?;
    if function == 0 { return Ok(false); }
    state.function = function as *mut UFunction;
    let parameter_bytes = unsafe { (*state.function).parms_size as usize };
    state.parameter_bytes = parameter_bytes;
    state.parameters.resize((parameter_bytes + std::mem::size_of::<u128>() - 1) / std::mem::size_of::<u128>(), 0);
    Ok(console_run_with(&mut state)? != 0)
}

struct CoreConsoleState<'access, 'borrow> {
    access: crate::core_access::CoreAccess<'access>,
    object: *mut hp2_abi::engine::generated::UObject,
    executor: *mut hp2_abi::engine::generated::UObject,
    command: *const crate::ansi::Tchar,
    token: [crate::ansi::Tchar; crate::name::NAME_SIZE],
    function: *mut hp2_abi::engine::generated::UFunction,
    current_struct: *mut hp2_abi::engine::generated::UStruct,
    current_property: *mut hp2_abi::engine::generated::UProperty,
    function_class: *mut hp2_abi::engine::generated::UClass,
    property_class: *mut hp2_abi::engine::generated::UClass,
    object_property_class: *mut hp2_abi::engine::generated::UClass,
    parameters: Vec<u128>,
    parameter_bytes: usize,
    output: &'borrow mut dyn FnMut(&[u32]) -> crate::core_access::RuntimeResult<()>,
    dispatch: &'borrow mut dyn FnMut(
        *mut hp2_abi::engine::generated::UObject,
        *mut hp2_abi::engine::generated::UFunction,
        *mut c_void,
    ) -> crate::core_access::RuntimeResult<()>,
}


impl CoreConsoleState<'_, '_> {
    fn next_property(&mut self) -> *mut hp2_abi::engine::generated::UProperty {
        use hp2_abi::engine::generated::{UField, UStruct};
        loop {
            if !self.current_property.is_null() {
                let field = self.current_property.cast::<UField>();
                self.current_property = unsafe { (*field).next.cast() };
                if unsafe { crate::object_model::is_a(field.cast(), self.property_class.cast()) } {
                    return field.cast();
                }
                continue;
            }
            if self.current_struct.is_null() { return ptr::null_mut(); }
            let structure = self.current_struct;
            self.current_struct = if unsafe { crate::object_model::is_a(structure.cast(), self.function_class.cast()) } {
                ptr::null_mut()
            } else {
                unsafe { (*structure.cast::<UField>()).super_field.cast::<UStruct>() }
            };
            self.current_property = unsafe { (*structure).children.cast() };
        }
    }

    unsafe fn leaf(
        &mut self,
        operation: Hp2ObjectInstanceLeaf,
        a: usize,
        b: usize,
        _c: usize,
    ) -> crate::core_access::RuntimeResult<usize> {
        use Hp2ObjectInstanceLeaf as L;
        use hp2_abi::engine::generated::{FStateFrame, UObject, UFunction, UField, UProperty, UState, UStruct, UObjectProperty};
        let value = match operation {
            L::Class => unsafe { (*self.object).class as usize },
            L::Object => self.object as usize,
            L::Frame => unsafe { (*self.object).state_frame as usize },
            L::StateNode => unsafe { (*(a as *mut FStateFrame)).state_node as usize },
            L::ProbeMask => unsafe { (*(a as *mut FStateFrame)).probe_mask as usize },
            L::HashHead => unsafe { (*(a as *mut UState)).vf_hash[b] as usize },
            L::HashNext => unsafe { (*(a as *mut UField)).hash_next as usize },
            L::NameIndex => unsafe {
                i32::from_ne_bytes((*((a as *mut c_void).cast::<UObject>())).name) as usize
            },
            L::CastFunction => {
                let field = a as *mut UField;
                if unsafe { crate::object_model::is_a(field.cast(), self.function_class.cast()) } { a } else { 0 }
            }
            L::Scriptable => usize::from(self.access.process_mode().is_scriptable),
            L::ParseToken => usize::from(unsafe {
                crate::tokenize::parse_token(&mut self.command, self.token.as_mut_ptr(), self.token.len() as i32, true)
            }),
            L::TokenName => {
                let name = self.token.iter().copied().take_while(|&unit| unit != 0)
                    .map(|unit| char::from_u32(unit as u32).unwrap_or(char::REPLACEMENT_CHARACTER))
                    .collect::<String>();
                self.access.intern_str(&name, crate::name_owner::FindName::Find)? as usize
            }
            L::FunctionFlags => unsafe { (*(a as *mut UFunction)).function_flags as usize },
            L::Function => self.function as usize,
            L::Executor => self.executor as usize,
            L::PropertiesBegin => {
                self.current_struct = a as *mut UStruct;
                self.current_property = ptr::null_mut();
                self.next_property() as usize
            }
            L::PropertiesNext => self.next_property() as usize,
            L::PropertyFlags => unsafe { (*(a as *mut UProperty)).property_flags as usize },
            L::ObjectProperty => {
                let field = a as *mut UField;
                if unsafe { crate::object_model::is_a(field.cast(), self.object_property_class.cast()) } { a } else { 0 }
            }
            L::PropertyClass => unsafe { (*(a as *mut UObjectProperty)).property_class as usize },
            L::WriteExecutor => {
                let property = a as *mut UProperty;
                let offset = usize::try_from(unsafe { (*property).offset })
                    .map_err(|_| crate::core_access::RuntimeError::new("Negative console parameter offset"))?;
                let extent = usize::try_from(unsafe { (*property).element_size })
                    .map_err(|_| crate::core_access::RuntimeError::new("Invalid console parameter size"))?;
                let end = offset.checked_add(extent).ok_or_else(|| crate::core_access::RuntimeError::new("Console parameter extent overflow"))?;
                if end > self.parameter_bytes || extent < std::mem::size_of::<*mut c_void>() {
                    return Err(crate::core_access::RuntimeError::new("Console executor parameter exceeds reflected frame"));
                }
                unsafe { self.parameters.as_mut_ptr().cast::<u8>().add(offset).cast::<*mut c_void>().write_unaligned(self.executor.cast()); }
                0
            }
            L::ParseNext => { unsafe { crate::tokenize::parse_next(&mut self.command); } 0 }
            L::Import => {
                let property = a as *mut UProperty;
                let offset = usize::try_from(unsafe { (*property).offset })
                    .map_err(|_| crate::core_access::RuntimeError::new("Negative console parameter offset"))?;
                let extent = usize::try_from(unsafe { (*property).element_size })
                    .ok().and_then(|size| size.checked_mul(unsafe { (*property).array_dim.max(0) as usize }))
                    .ok_or_else(|| crate::core_access::RuntimeError::new("Invalid console parameter extent"))?;
                let end = offset.checked_add(extent).ok_or_else(|| crate::core_access::RuntimeError::new("Console parameter extent overflow"))?;
                if end > self.parameter_bytes { return Err(crate::core_access::RuntimeError::new("Console parameter exceeds reflected frame")); }
                let start = self.command;
                let mut length = 0usize;
                while unsafe { *start.add(length) } != 0 { length += 1; }
                let input = unsafe { std::slice::from_raw_parts(start.cast::<u32>(), length + 1) };
                let consumed = unsafe {
                    self.access.import_console_argument(property, self.parameters.as_mut_ptr().cast::<u8>().add(offset), input)?
                };
                if consumed == 0 { 0 } else { self.command = unsafe { start.add(consumed) }; 1 }
            }
            L::BadProperty => {
                let token = self.token.iter().copied().take_while(|&unit| unit != 0)
                    .map(|unit| char::from_u32(unit as u32).unwrap_or(char::REPLACEMENT_CHARACTER))
                    .collect::<String>();
                let message_name = self.access.intern_str(&token, crate::name_owner::FindName::Find)?;
                let section = [69, 114, 114, 111, 114];
                let key = [66, 97, 100, 80, 114, 111, 112, 101, 114, 116, 121];
                let package = [67, 111, 114, 101];
                let mut text = self.access.localize_text(&section, &key, Some(&package), None, false)?;
                let command_name = self.access.name_text(message_name)?;
                let property_name = self.access.name_text(i32::from_ne_bytes(unsafe { (*(a as *mut UObject)).name }))?;
                format_console_error(&mut text, &[&command_name, &property_name]);
                (self.output)(&text)?;
                0
            }
            L::ProcessEvent => {
                (self.dispatch)(self.object, self.function, self.parameters.as_mut_ptr().cast::<u8>().cast())?;
                0
            }
            L::DestroyValue => {
                let property = a as *mut UProperty;
                let offset = usize::try_from(unsafe { (*property).offset })
                    .map_err(|_| crate::core_access::RuntimeError::new("Negative console parameter offset"))?;
                let extent = usize::try_from(unsafe { (*property).element_size })
                    .ok().and_then(|size| size.checked_mul(unsafe { (*property).array_dim.max(0) as usize }))
                    .ok_or_else(|| crate::core_access::RuntimeError::new("Invalid console parameter extent"))?;
                let end = offset.checked_add(extent).ok_or_else(|| crate::core_access::RuntimeError::new("Console parameter extent overflow"))?;
                if end > self.parameter_bytes { return Err(crate::core_access::RuntimeError::new("Console parameter exceeds reflected frame")); }
                unsafe {
                    self.access.property_value(
                        crate::property_value::Hp2ValueOperation::ValueDestroy,
                        property,
                        self.parameters.as_mut_ptr().cast::<u8>().add(offset).cast(),
                        ptr::null_mut(),
                        0,
                        None,
                    )?;
                }
                0
            }
            _ => return Err(crate::core_access::RuntimeError::new("Unsupported canonical console operation")),
        };
        Ok(value)
    }
}

impl ConsoleExecOps for CoreConsoleState<'_, '_> {
    type Error = crate::core_access::RuntimeError;

    fn call(
        &mut self,
        operation: Hp2ObjectInstanceLeaf,
        a: usize,
        b: usize,
        c: usize,
    ) -> crate::core_access::RuntimeResult<usize> {
        unsafe { self.leaf(operation, a, b, c) }
    }

    fn cleanup_after_error(&self) -> bool { true }
}

fn format_console_error(format: &mut Vec<u32>, values: &[&[u32]]) {
    let mut result = Vec::with_capacity(format.len());
    let mut value = 0;
    let mut index = 0;
    while index < format.len() {
        if format.get(index..index + 2) == Some(&[u32::from('%'), u32::from('s')]) && value < values.len() {
            result.extend_from_slice(values[value]);
            value += 1;
            index += 2;
        } else {
            result.push(format[index]);
            index += 1;
        }
    }
    *format = result;
}
