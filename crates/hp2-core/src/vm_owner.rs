//! Direct Core VM execution over canonical catalog identities and live Rust-owned records.
//!
//! The executor owns no registry or reflected object record. It keeps only an immutable host
//! capability and passes stable raw frame pointers across callbacks, so recursive script and log
//! reentry never overlaps a Rust borrow of a live frame or object body.

use crate::core_access::{RuntimeError, RuntimeResult};
use crate::core_native_catalog::{self, CoreNative};
use crate::canonical_vm_native::CanonicalVmNative;
use crate::object_owner::CanonicalNativeBinding;
use crate::{math, vm_convert, vm_flow, vm_frame, vm_scalar, vm_string, vm_vector};
use hp2_abi::engine::generated::{FFrame, TArrayRaw, UFunction, UStruct};
use crate::math::FRotator;
use hp2_abi::FVector;
use std::alloc::Layout;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

const fn zero_vector() -> FVector {
    FVector { x: 0.0, y: 0.0, z: 0.0 }
}

const fn zero_rotator() -> FRotator {
    FRotator { pitch: 0, yaw: 0, roll: 0 }
}

/// Result of one bytecode step or script event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepControl {
    /// The current frame remains live and may execute another expression.
    Continue,
    /// The current script event has returned or executed `EX_Stop`.
    Return,
}

/// Boundary callback kind for direct event tracing and explicit local cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventBoundary {
    /// Begin after the raw frame and zeroed locals storage are initialized, before reflected fields.
    Enter,
    /// End this event after execution, including an error path.
    Exit,
}

/// Direct reflected property execution routed to the canonical object owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmPropertyOp {
    /// Locals-segment address.
    LocalVariable,
    /// Current-instance property address.
    InstanceVariable,
    /// Class-default property address.
    DefaultVariable,
    /// Class-context expression.
    ClassContext,
    /// Fixed or dynamic array element.
    ArrayElement,
    /// Dynamic-array element.
    DynamicArrayElement,
    /// Dynamic-array count.
    DynamicArrayCount,
    /// Packed boolean lvalue.
    BoolVariable,
    /// Struct member lvalue.
    StructMember,
    /// Native parameter lvalue.
    NativeParameter,
    /// Reflected assignment.
    Let,
    /// Packed-boolean assignment.
    LetBool,
    /// Self object expression.
    SelfObject,
    /// Conditional object context.
    Context,
    /// Struct equality.
    StructEqual,
    /// Struct inequality.
    StructNotEqual,
}

/// Direct function invocation kind from canonical function bytecode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmCallOp {
    /// Virtual class lookup.
    Virtual,
    /// Exact function record.
    Final,
    /// Global function record.
    Global,
    /// Iterator initialization/step/cleanup.
    Iterator,
}

/// Direct flow operation whose policy remains shared with the script frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmFlowOp {
    /// Debug line and input metadata.
    DebugInfo,
    /// Typed switch.
    Switch,
    /// Switch case.
    Case,
    /// Absolute script jump.
    Jump,
    /// Conditional script jump.
    JumpIfNot,
    /// Script assertion.
    Assert,
    /// State label lookup.
    GotoLabel,
}

/// Exact system native whose leaf effect belongs to CoreAccess or an existing subsystem owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmSystemOp {
    /// Dynamic class cast.
    DynamicCast,
    /// Metaclass cast.
    MetaCast,
    /// Class ancestry query.
    ClassIsChildOf,
    /// Object ancestry query.
    IsA,
    /// Canonical object construction.
    NewObject,
    /// Engine random operation.
    Rand,
    /// Engine random operation.
    FRand,
    /// Engine random operation.
    VRand,
    /// Engine random operation.
    RotRand,
    /// Engine random operation.
    RandRange,
    /// State transition.
    GotoState,
    /// State control.
    Enable,
    /// State control.
    Disable,
    /// State query.
    IsInState,
    /// State query.
    GetStateName,
    /// Engine logging.
    Log,
    /// Engine warning.
    Warn,
    /// Configuration persistence.
    SaveConfig,
    /// Configuration reset.
    ResetConfig,
    /// Localization lookup.
    Localize,
    /// Property text retrieval.
    GetPropertyText,
    /// Property text update.
    SetPropertyText,
    /// Static configuration persistence.
    StaticSaveConfig,
    /// Enum lookup.
    GetEnum,
    /// Language lookup.
    GetLanguage,
    /// Dynamic object loading.
    DynamicLoadObject,
    /// Native Commandlet.Main wrapper dispatching the receiver's Main event.
    CommandletMain,
    /// Explicit catalog slots represented by the high-native adapters.
    NativeTableSlot(u32),
}

/// FString operation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmStringOp {
    /// Discard an FString expression.
    Eat,
    /// Concatenate strings.
    Concat,
    /// Append strings with a space.
    At,
    /// Lexical string comparison.
    Compare,
    /// Case-insensitive equality.
    CaseInsensitiveEqual,
    /// String length.
    Len,
    /// Substring search.
    InStr,
    /// Substring extraction.
    Mid,
    /// Prefix extraction.
    Left,
    /// Suffix extraction.
    Right,
    /// ASCII uppercase.
    Caps,
    /// TCHAR construction.
    Chr,
    /// First TCHAR value.
    Asc,
    /// String conversion.
    Convert,
}

/// Engine-thread operations required by the direct Core VM.
///
/// Every callback uses shared access to make reentry explicit. A host must implement the actual
/// engine operation; there are no placeholder implementations or foreign status transports.
pub trait VmHost: Sized {
    /// Resolve a serialized object-table index through canonical `ObjectStorage`.
    fn object_at(&self, index: i32) -> RuntimeResult<*mut c_void>;

    /// Resolve the native binding owned by this canonical function record.
    fn native_binding(&self, function: *mut UFunction) -> RuntimeResult<CanonicalNativeBinding>;

    /// Resolve an external numeric native after the Core catalog has no match.
    fn external_native(&self, opcode: u32) -> RuntimeResult<Option<CanonicalVmNative>>;

    /// Executes an external native against this host's canonical owner.
    ///
    /// # Safety
    /// The receiver, frame and result satisfy the registered native's contract.
    unsafe fn execute_external_native(
        &self, native: CanonicalVmNative, object: *mut c_void,
        frame: *mut FFrame, result: *mut c_void,
    ) -> RuntimeResult<()>;

    /// Resolve a name index to its exact UTF-32 spelling.
    fn name_text(&self, index: i32) -> RuntimeResult<Vec<u32>>;

    /// Find a name without interning an absent spelling.
    fn find_name(&self, text: &[u32]) -> RuntimeResult<i32>;

    /// Resolve the canonical `NAME_None` index without interning it.
    fn none_name(&self) -> RuntimeResult<i32>;

    /// Emit an engine log event; implementations may recursively execute script.
    fn log(&self, event: i32, text: &[u32]) -> RuntimeResult<()>;

    /// Read a configuration value by the engine's canonical section/key identity.
    fn config_get(&self, section: &[u32], key: &[u32]) -> RuntimeResult<Option<Vec<u32>>>;

    /// Persist a configuration value through the active engine configuration owner.
    fn config_set(&self, section: &[u32], key: &[u32], value: &[u32]) -> RuntimeResult<()>;

    /// Serialize or relocate one object reference through the active archive owner.
    fn archive_object_reference(&self, object: *mut c_void) -> RuntimeResult<*mut c_void>;

    /// Return the live property storage address for a canonical object/property pair.
    fn property_address(&self, object: *mut c_void, property: *mut c_void)
    -> RuntimeResult<*mut u8>;

    /// Evaluate a by-reference operand, returning its canonical lvalue or `scratch`.
    ///
    /// If the expression is not an lvalue, the host must evaluate it into the supplied initialized
    /// scratch storage. The returned pointer remains valid through argument completion and may be
    /// changed by reentrant execution.
    fn reference_argument(
        &self,
        vm: &VmExecutor<'_, Self>,
        frame: *mut FFrame,
        scratch: *mut c_void,
        size: usize,
    ) -> RuntimeResult<*mut c_void>;

    /// Assign one terminated ANSI or UTF-16 literal into initialized FString storage.
    fn assign_string_constant(
        &self,
        vm: &VmExecutor<'_, Self>,
        unicode: bool,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()>;

    /// Allocate one aligned locals body using the canonical Core allocator.
    fn allocate_locals(&self, layout: Layout) -> RuntimeResult<NonNull<u8>>;

    /// Release the raw locals allocation after all reflected fields are destroyed.
    fn free_locals(&self, locals: NonNull<u8>, layout: Layout);

    /// Initialize function value storage from offset zero, including typed fields.
    fn construct_locals(&self, function: *mut UFunction, locals: *mut u8) -> RuntimeResult<()>;

    /// Destroy reflected locals in the engine-observable field order.
    fn destroy_locals(&self, function: *mut UFunction, locals: *mut u8) -> RuntimeResult<()>;

    /// Evaluate one owning FString expression into initialized array storage.
    fn string_argument(
        &self,
        vm: &VmExecutor<'_, Self>,
        frame: *mut FFrame,
        result: *mut TArrayRaw,
    ) -> RuntimeResult<()>;

    /// Release one owning FString argument through its canonical array owner.
    fn release_string(&self, value: *mut TArrayRaw) -> RuntimeResult<()>;

    /// Evaluate a canonical property operation; `VmExecutor` owns opcode/operand stepping.
    fn execute_property(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: VmPropertyOp,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl>;

    /// Invoke a canonical function or iterator operation.
    fn execute_call(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: VmCallOp,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl>;

    /// Perform an exact object/system native leaf operation.
    fn execute_system(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: VmSystemOp,
        native_slot: u32,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl>;

    /// Perform an FString operation using the owning string/array kernels.
    fn execute_string(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: VmStringOp,
        opcode: u32,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl>;

    /// Execute a flow kernel whose script-base/diagnostic leaves belong to CoreAccess.
    fn execute_flow(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: VmFlowOp,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl>;


    /// Notify the object owner at event entry/exit; the executor owns frame/local allocation.
    fn event_boundary(
        &self,
        phase: EventBoundary,
        object: *mut c_void,
        function: *mut UFunction,
        frame: *mut FFrame,
    ) -> RuntimeResult<()>;

    /// Runs a real interpreted function inside the host's optional observation scope.
    /// Failed execution does not invent an observer exit callback.
    fn interpreted_call(
        &self, object: *mut c_void, function: *mut UFunction, frame: *mut FFrame,
        result: *mut c_void, from_script_call: bool,
        execute: impl FnOnce() -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        let _ = (object, function, frame, result, from_script_call);
        execute()
    }

    /// Apply the canonical runaway/recursion guard before each event step.
    fn check_runaway(
        &self,
        object: *mut c_void,
        function: *mut UFunction,
        frame: *mut FFrame,
        steps: u64,
    ) -> RuntimeResult<()>;
}

/// Reentrant executor over one engine-thread host capability.
pub struct VmExecutor<'host, H: VmHost> {
    host: &'host H,
}

impl<'host, H: VmHost> VmExecutor<'host, H> {
    /// Create an executor borrowing the canonical host without taking mutable access.
    #[must_use]
    pub const fn new(host: &'host H) -> Self {
        Self { host }
    }

    /// Access the immutable engine-thread host capability.
    #[must_use]
    pub const fn host(&self) -> &'host H {
        self.host
    }

    /// Execute exactly one expression, consuming its opcode before reentrant work.
    ///
    /// # Errors
    /// Returns the first host/runtime failure with the opcode appended to ordered context.
    ///
    /// # Safety
    /// `frame` points to a stable live `FFrame`; `result` meets the selected expression's width
    /// and alignment requirements. Every bytecode operand reachable by the opcode is valid.
    pub unsafe fn step(
        &self,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl> {
        let object = unsafe { std::ptr::addr_of!((*frame).object).read() };
        let outcome = unsafe {
            vm_frame::step_with(frame, object, result, |opcode, object, frame, result| {
                self.dispatch(opcode, object, frame, result)
            })
        };
        outcome.map_err(|error| error.with_context("stepping Core VM bytecode"))
    }

    /// Execute a script event with Rust-owned frame and locals scratch.
    ///
    /// Parameter bytes are copied into the reflected locals prefix before execution. Once local
    /// construction succeeds, the parameter prefix is copied back even when execution or later
    /// teardown fails, preserving mutations committed before the first error. The host constructs
    /// and destroys reflected members; this owner allocates, initializes and releases the backing
    /// locals bytes in the required order.
    ///
    /// # Errors
    /// Returns the first execution or teardown failure, retaining UTF-32 diagnostics and ordered
    /// context from nested operations.
    ///
    /// # Safety
    /// `object`, `function`, `parent` and non-null `parameters` remain stable through reentry.
    /// `function` is a live generated record with immutable `UStruct.script`; parameter storage
    /// holds at least `UFunction.parms_size` disjoint readable/writable bytes.
    pub unsafe fn execute_event(
        &self,
        object: *mut c_void,
        function: *mut UFunction,
        parent: *mut FFrame,
        parameters: *mut u8,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        let structure = function.cast::<UStruct>();
        let script = unsafe { std::ptr::addr_of!((*structure).script).read() };
        let base = script.data.cast::<u8>();
        let script_length = usize::try_from(script.num)
            .map_err(|_| RuntimeError::new("Invalid Core VM script size"))?;
        if script_length != 0 && base.is_null() {
            return Err(RuntimeError::new(
                "Core VM function has script bytes but no script storage",
            ));
        }
        let raw_size = unsafe { std::ptr::addr_of!((*structure).properties_size).read() };
        let raw_align = unsafe { std::ptr::addr_of!((*structure).properties_align).read() };
        let parameter_size = unsafe { std::ptr::addr_of!((*function).parms_size).read() } as usize;
        let size = usize::try_from(raw_size)
            .map_err(|_| RuntimeError::new("Invalid Core VM locals size"))?;
        let align = usize::try_from(raw_align)
            .ok()
            .filter(|align| align.is_power_of_two())
            .or_else(|| (size == 0 && raw_align == 0).then_some(1))
            .ok_or_else(|| RuntimeError::new("Invalid Core VM locals alignment"))?;
        if parameter_size > size {
            return Err(RuntimeError::new("Core VM parameter area exceeds locals size"));
        }
        if parameter_size != 0 && parameters.is_null() {
            return Err(RuntimeError::new("Missing Core VM parameter storage"));
        }
        let layout = Layout::from_size_align(size.max(1), align)
            .map_err(|_| RuntimeError::new("Invalid Core VM locals allocation layout"))?;
        let locals = self
            .host
            .allocate_locals(layout)
            .map_err(|error| error.with_context("allocating Core VM locals"))?;
        let locals_ptr = locals.as_ptr();

        let mut raw_frame = MaybeUninit::<FFrame>::uninit();
        let frame = raw_frame.as_mut_ptr();
        unsafe {
            std::ptr::addr_of_mut!((*frame).node).write(function.cast());
            std::ptr::addr_of_mut!((*frame).object).write(object);
            std::ptr::addr_of_mut!((*frame).code).write(base);
            std::ptr::addr_of_mut!((*frame).locals).write(locals_ptr);
            std::ptr::addr_of_mut!((*frame).parent).write(parent);
        }

        let mut entered = false;
        let mut constructed = false;
        let execution: RuntimeResult<()> = (|| {
            entered = true;
            self.host
                .event_boundary(EventBoundary::Enter, object, function, frame)
                .map_err(|error| error.with_context("entering Core VM event"))?;
            self.host.construct_locals(function, locals_ptr)
                .map_err(|error| error.with_context("constructing Core VM locals"))?;
            constructed = true;
            if parameter_size != 0 {
                unsafe { std::ptr::copy_nonoverlapping(parameters, locals_ptr, parameter_size) };
            }
            let observed_result = result;
            let return_offset = unsafe { (*function).return_value_offset };
            let result = if result.is_null() && return_offset != u16::MAX {
                if usize::from(return_offset) >= size {
                    return Err(RuntimeError::new("Function return slot exceeds locals storage"));
                }
                unsafe { locals_ptr.add(usize::from(return_offset)).cast() }
            } else { result };
            if unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0 {
                let native = self.host.native_binding(function)?;
                unsafe { self.execute_native(native, u32::from((*function).i_native), object, frame, result)?; }
            } else {
                self.host.interpreted_call(
                    object, function, frame, observed_result, !parent.is_null(),
                    || unsafe { self.run_script(object, function, frame, result, base, script_length) },
                )?;
            }
            Ok(())
        })();

        let mut outcome = execution;
        if constructed && parameter_size != 0 {
            // The native caller observes mutations committed before a later VM
            // failure; locals cleanup must not roll them back.
            unsafe { std::ptr::copy_nonoverlapping(locals_ptr, parameters, parameter_size) };
        }

        if constructed {
            outcome = preserve_first(
                outcome,
                self.host
                    .destroy_locals(function, locals_ptr)
                    .map_err(|error| error.with_context("destroying Core VM locals")),
            );
        }
        if entered {
            outcome = preserve_first(
                outcome,
                self.host
                    .event_boundary(EventBoundary::Exit, object, function, frame)
                    .map_err(|error| error.with_context("leaving Core VM event")),
            );
        }
        self.host.free_locals(locals, layout);
        outcome.map_err(|error| {
            error.with_context(&format!("Core event {:#x}", function as usize))
        })
    }

    unsafe fn run_script(
        &self,
        object: *mut c_void,
        function: *mut UFunction,
        frame: *mut FFrame,
        result: *mut c_void,
        base: *mut u8,
        length: usize,
    ) -> RuntimeResult<()> {
        if length == 0 {
            return Ok(());
        }
        if base.is_null() {
            return Err(RuntimeError::new(
                "Core VM function has script bytes but no script storage",
            ));
        }
        // ProcessInternal uses a separate 1024-byte statement buffer and clears
        // only its FString header. Only EX_Return evaluates into the caller's slot.
        let mut statement_buffer = MaybeUninit::<[u128; 64]>::uninit();
        let statement_result = statement_buffer.as_mut_ptr().cast::<c_void>();
        unsafe {
            statement_result.cast::<u8>().write_bytes(0, std::mem::size_of::<TArrayRaw>());
        }
        let mut steps = 0_u64;
        loop {
            let code = unsafe { std::ptr::addr_of!((*frame).code).read() };
            if code.is_null() {
                return Ok(());
            }
            let offset = (code as usize).wrapping_sub(base as usize);
            if offset >= length {
                return Err(
                    RuntimeError::new("Core VM instruction pointer is outside function script")
                        .with_context(&format!("function {:#x}", function as usize)),
                );
            }
            self.host
                .check_runaway(object, function, frame, steps)
                .map_err(|error| error.with_context("checking Core VM runaway guard"))?;
            steps = steps.wrapping_add(1);
            if unsafe { code.read() } == 4 {
                unsafe {
                    std::ptr::addr_of_mut!((*frame).code).write(code.add(1));
                    self.step(frame, result)?;
                }
                let next = unsafe { std::ptr::addr_of!((*frame).code).read() };
                if !next.is_null()
                    && (next as usize).wrapping_sub(base as usize) < length
                    && unsafe { next.read() } == 0x38
                {
                    unsafe { self.step(frame, result)? };
                }
                return Ok(());
            }
            if unsafe { self.step(frame, statement_result)? } == StepControl::Return {
                return Ok(());
            }
        }
    }

    /// Evaluate one direct expression argument without consulting `GNatives`.
    ///
    /// # Errors
    /// Returns the first nested expression error.
    ///
    /// # Safety
    /// `frame` and `result` satisfy [`Self::step`] and `result` is aligned and initialized as the
    /// selected expression requires.
    pub unsafe fn step_argument(
        &self,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            vm_frame::step_argument_with(frame, result, |opcode, object, frame, result| {
                self.dispatch_operand(opcode, object, frame, result)
            })
        }
    }

    /// Finish argument parsing, preserving the optional live debug expression behavior.
    ///
    /// # Errors
    /// Returns the first debug-expression error.
    ///
    /// # Safety
    /// The frame cursor points to a valid function-parameter terminator and readable next opcode.
    pub unsafe fn finish_arguments(&self, frame: *mut FFrame) -> RuntimeResult<()> {
        unsafe {
            vm_frame::finish_arguments_with(frame, |opcode, object, frame, result| {
                self.dispatch_operand(opcode, object, frame, result)
            })
        }
    }


    unsafe fn read_argument<T: Copy>(&self, frame: *mut FFrame) -> RuntimeResult<T> {
        let mut value = MaybeUninit::<T>::uninit();
        unsafe { self.step_argument(frame, value.as_mut_ptr().cast())? };
        Ok(unsafe { value.assume_init() })
    }

    unsafe fn step_into<T>(&self, frame: *mut FFrame, output: *mut T) -> RuntimeResult<()> {
        unsafe { self.step_argument(frame, output.cast()) }
    }

    unsafe fn finish(&self, frame: *mut FFrame) -> RuntimeResult<()> {
        unsafe { self.finish_arguments(frame) }
    }

    /// Dispatch one canonical catalog identity without consulting legacy `GNatives`.
    ///
    /// # Errors
    /// Returns an exact direct operation failure, or an error for a catalog identity that has no
    /// implementation in the direct owner.
    ///
    /// # Safety
    /// The frame/object/result satisfy the selected native's contract and remain stable across
    /// callbacks. The catalog identity and numeric index are the exact matched entry.
    pub unsafe fn execute_native(
        &self,
        native: CanonicalNativeBinding,
        opcode: u32,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl> {
        // SAFETY: forwarded from this public method's pointer contract.
        let result = match native {
            CanonicalNativeBinding::Core(native) =>
                unsafe { self.dispatch_native(native, opcode, object, frame, result) },
            CanonicalNativeBinding::External(native) =>
                unsafe { self.host.execute_external_native(native, object, frame, result) }
                    .map(|()| StepControl::Continue),
        };
        result.map_err(|error| {
            error.with_context(&format!("Core native {native:?} ({opcode})"))
        })
    }

    fn dispatch(
        &self,
        encoded_opcode: u8,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl> {
        if encoded_opcode == 4 {
            return Ok(StepControl::Return);
        }
        let opcode = if (96..=111).contains(&encoded_opcode) {
            let low = unsafe {
                crate::vm_bytecode::read::<u8>(std::ptr::addr_of_mut!((*frame).code))
            };
            u32::from(encoded_opcode - 96) * 0x100 + u32::from(low)
        } else {
            u32::from(encoded_opcode)
        };
        let native = if let Some(spec) = core_native_catalog::by_opcode(opcode as i32) {
            CanonicalNativeBinding::Core(spec.native)
        } else if let Some(handler) = self.host.external_native(opcode)? {
            CanonicalNativeBinding::External(handler)
        } else {
            return Err(RuntimeError::new("No direct Core VM operation for bytecode opcode")
                .with_context(&format!("opcode {opcode:#06x}")));
        };
        // SAFETY: the immutable catalogs bind this exact numeric bytecode identity.
        unsafe { self.execute_native(native, opcode, object, frame, result) }
    }

    pub(crate) fn dispatch_operand(
        &self,
        opcode: u8,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        match self.dispatch(opcode, object, frame, result)? {
            StepControl::Continue => Ok(()),
            StepControl::Return => Err(RuntimeError::new(
                "Core VM return/stop opcode encountered inside an expression",
            )),
        }
    }

    unsafe fn dispatch_native(
        &self,
        native: CoreNative,
        opcode: u32,
        object: *mut c_void,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<StepControl> {
        use crate::core_native_catalog::CoreNative as N;
        use crate::vm_flow::Hp2VmFlowOp as Flow;

        let control = match native {
            N::intUObjectexecNothing => StepControl::Continue,
            N::intUObjectexecStop => {
                self.plain(frame, result, Flow::VmFlowStop);
                StepControl::Return
            }
            N::intUObjectexecEndFunctionParms => {
                self.plain(frame, result, Flow::VmFlowEndFunctionParms)
            }
            N::intUObjectexecIntConst => self.plain(frame, result, Flow::VmFlowIntConst),
            N::intUObjectexecFloatConst => self.plain(frame, result, Flow::VmFlowFloatConst),
            N::intUObjectexecObjectConst => {
                let index = unsafe {
                    crate::vm_bytecode::read::<i32>(std::ptr::addr_of_mut!((*frame).code))
                };
                let value = self.host.object_at(index)?;
                unsafe { result.cast::<*mut c_void>().write_unaligned(value) };
                StepControl::Continue
            }
            N::intUObjectexecStringConst => {
                self.host.assign_string_constant(self, false, frame, result)?;
                StepControl::Continue
            }
            N::intUObjectexecUnicodeStringConst => {
                self.host.assign_string_constant(self, true, frame, result)?;
                StepControl::Continue
            }
            N::intUObjectexecNameConst => self.plain(frame, result, Flow::VmFlowNameConst),
            N::intUObjectexecByteConst => self.plain(frame, result, Flow::VmFlowByteConst),
            N::intUObjectexecIntZero => self.plain(frame, result, Flow::VmFlowIntZero),
            N::intUObjectexecIntOne => self.plain(frame, result, Flow::VmFlowIntOne),
            N::intUObjectexecTrue => self.plain(frame, result, Flow::VmFlowTrue),
            N::intUObjectexecFalse => self.plain(frame, result, Flow::VmFlowFalse),
            N::intUObjectexecNoObject => self.plain(frame, result, Flow::VmFlowNoObject),
            N::intUObjectexecIntConstByte => {
                self.plain(frame, result, Flow::VmFlowIntConstByte)
            }
            N::intUObjectexecRotationConst | N::intUObjectexecVectorConst => {
                self.vector_constant(frame, result)
            }
            N::intUObjectexecAndAnd_BoolBool => {
                unsafe {
                    vm_scalar::execute_short_circuit_with::<RuntimeError, 130>(
                        frame,
                        result.cast(),
                        |opcode, object, frame, result| {
                            self.dispatch_operand(opcode, object, frame, result)
                        },
                    )?;
                }
                StepControl::Continue
            }
            N::intUObjectexecOrOr_BoolBool => {
                unsafe {
                    vm_scalar::execute_short_circuit_with::<RuntimeError, 132>(
                        frame,
                        result.cast(),
                        |opcode, object, frame, result| {
                            self.dispatch_operand(opcode, object, frame, result)
                        },
                    )?;
                }
                StepControl::Continue
            }
            _ => {
                if let Some(control) = unsafe { self.execute_math(opcode, frame, result)? } {
                    control
                } else if let Some(control) =
                    unsafe { self.execute_assignment(opcode, frame, result)? }
                {
                    control
                } else if let Some(control) =
                    unsafe { self.execute_conversion(native, frame, result)? }
                {
                    control
                } else if matches!(
                    native,
                    N::intUObjectexecEqualEqual_ObjectObject
                        | N::intUObjectexecNotEqual_ObjectObject
                        | N::intUObjectexecEqualEqual_NameName
                        | N::intUObjectexecNotEqual_NameName
                ) {
                    unsafe { self.execute_identity(opcode, frame, result)? };
                    StepControl::Continue
                } else if native == N::intUObjectexecGetAxes {
                    unsafe { self.execute_axes(frame, result, false)? };
                    StepControl::Continue
                } else if native == N::intUObjectexecGetUnAxes {
                    unsafe { self.execute_axes(frame, result, true)? };
                    StepControl::Continue
                } else if native == N::intUObjectexecLessLess_VectorRotator
                    || native == N::intUObjectexecGreaterGreater_VectorRotator
                {
                    unsafe {
                        self.execute_rotate(
                            frame,
                            result,
                            native == N::intUObjectexecGreaterGreater_VectorRotator,
                        )?
                    }
                } else if native == N::intUObjectexecNormalize {
                    let mut rotation = MaybeUninit::<FRotator>::uninit();
                    unsafe { self.step_into(frame, rotation.as_mut_ptr())? };
                    unsafe { self.finish(frame)? };
                    let rotation = unsafe { rotation.assume_init() };
                    let normalized = FRotator {
                        pitch: rotation.pitch as i16 as i32,
                        yaw: rotation.yaw as i16 as i32,
                        roll: rotation.roll as i16 as i32,
                    };
                    unsafe { result.cast::<FRotator>().write_unaligned(normalized) };
                    StepControl::Continue
                } else if native == N::intUObjectexecOrthoRotation {
                    unsafe { self.execute_ortho_rotation(frame, result)? };
                    StepControl::Continue
                } else if native == N::intUObjectexecInvert {
                    unsafe { self.execute_invert(frame)? };
                    StepControl::Continue
                } else if let Some(operation) = property_operation(native) {
                    self.host.execute_property(self, operation, object, frame, result)?
                } else if let Some(operation) = call_operation(native) {
                    self.host.execute_call(self, operation, object, frame, result)?
                } else if let Some(operation) = flow_operation(native) {
                    self.host.execute_flow(self, operation, object, frame, result)?
                } else if let Some(operation) = string_operation(native) {
                    self.host
                        .execute_string(self, operation, opcode, object, frame, result)?
                } else if let Some(operation) = system_operation(native, opcode) {
                    self.host.execute_system(self, operation, opcode, object, frame, result)?
                } else {
                    return Err(
                        RuntimeError::new("Canonical Core native lacks a direct VM operation")
                            .with_context(&format!("{native:?} ({opcode})")),
                    );
                }
            }
        };
        Ok(control)
    }

    unsafe fn execute_identity(
        &self,
        opcode: u32,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        let value = if matches!(opcode, 254 | 255) {
            let mut a = MaybeUninit::<i32>::zeroed();
            let mut b = MaybeUninit::<i32>::zeroed();
            unsafe { self.step_into(frame, a.as_mut_ptr())? };
            unsafe { self.step_into(frame, b.as_mut_ptr())? };
            unsafe { self.finish(frame)? };
            vm_string::identity(
                opcode,
                unsafe { a.assume_init() } as usize,
                unsafe { b.assume_init() } as usize,
            )
        } else {
            let mut a = MaybeUninit::<*mut c_void>::zeroed();
            let mut b = MaybeUninit::<*mut c_void>::zeroed();
            unsafe { self.step_into(frame, a.as_mut_ptr())? };
            unsafe { self.step_into(frame, b.as_mut_ptr())? };
            unsafe { self.finish(frame)? };
            vm_string::identity(
                opcode,
                unsafe { a.assume_init() } as usize,
                unsafe { b.assume_init() } as usize,
            )
        };
        unsafe { result.cast::<u32>().write_unaligned(value) };
        Ok(())
    }

    unsafe fn execute_conversion(
        &self,
        native: CoreNative,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<Option<StepControl>> {
        use crate::core_native_catalog::CoreNative as N;

        macro_rules! convert {
            ($input:ty, $output:ty, $body:expr) => {{
                let input = unsafe { self.read_argument::<$input>(frame)? };
                let output: $output = $body(input);
                unsafe { result.cast::<$output>().write_unaligned(output) };
                return Ok(Some(StepControl::Continue));
            }};
        }

        match native {
            N::intUObjectexecByteToInt => convert!(u8, i32, vm_convert::byte_to_int),
            N::intUObjectexecByteToBool => convert!(u8, u32, vm_convert::byte_to_bool),
            N::intUObjectexecByteToFloat => convert!(u8, f32, vm_convert::byte_to_float),
            N::intUObjectexecIntToByte => convert!(i32, u8, vm_convert::int_to_byte),
            N::intUObjectexecIntToBool => convert!(i32, i32, vm_convert::int_to_bool),
            N::intUObjectexecIntToFloat => convert!(i32, f32, vm_convert::int_to_float),
            N::intUObjectexecBoolToByte => convert!(u32, u8, vm_convert::bool_to_byte),
            N::intUObjectexecBoolToInt => convert!(u32, i32, vm_convert::bool_to_int),
            N::intUObjectexecBoolToFloat => convert!(u32, f32, vm_convert::bool_to_float),
            N::intUObjectexecFloatToByte => convert!(f32, u8, vm_convert::float_to_byte),
            N::intUObjectexecFloatToInt => convert!(f32, i32, vm_convert::float_to_int),
            N::intUObjectexecFloatToBool => convert!(f32, u32, vm_convert::float_to_bool),
            N::intUObjectexecObjectToBool => convert!(*mut c_void, u32, vm_convert::object_to_bool),
            N::intUObjectexecVectorToBool => convert!(FVector, u32, vm_convert::vector_to_bool),
            N::intUObjectexecRotatorToBool => convert!(FRotator, u32, vm_convert::rotator_to_bool),
            N::intUObjectexecNameToBool => {
                let none = self.host.none_name()?;
                let mut index = MaybeUninit::<i32>::zeroed();
                unsafe { self.step_into(frame, index.as_mut_ptr())? };
                let value = vm_convert::name_to_bool(unsafe { index.assume_init() }, none);
                unsafe { result.cast::<u32>().write_unaligned(value) };
                return Ok(Some(StepControl::Continue));
            }
            N::intUObjectexecVectorToRotator => {
                let input = unsafe { self.read_argument::<FVector>(frame)? };
                unsafe {
                    result
                        .cast::<FRotator>()
                        .write_unaligned(math::vector_rotation(input))
                };
                return Ok(Some(StepControl::Continue));
            }
            N::intUObjectexecRotatorToVector => {
                let input = unsafe { self.read_argument::<FRotator>(frame)? };
                unsafe {
                    result
                        .cast::<FVector>()
                        .write_unaligned(math::rotator_vector(input, math::trig_table()))
                };
                return Ok(Some(StepControl::Continue));
            }
            _ => return Ok(None),
        }
    }

    unsafe fn execute_assignment(
        &self,
        opcode: u32,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<Option<StepControl>> {
        if (133..=140).contains(&opcode) {
            let mut scratch = MaybeUninit::<u8>::zeroed();
            let target = self.host.reference_argument(
                self,
                frame,
                scratch.as_mut_ptr().cast(),
                std::mem::size_of::<u8>(),
            )?;
            if target.is_null() {
                return Err(RuntimeError::new("Core VM byte assignment has no lvalue"));
            }
            let mut rhs = MaybeUninit::<u8>::zeroed();
            if opcode <= 136 {
                unsafe { self.step_into(frame, rhs.as_mut_ptr())? };
            }
            unsafe { self.finish(frame)? };
            unsafe {
                vm_scalar::byte_assign(
                    opcode,
                    target.cast(),
                    rhs.assume_init(),
                    result.cast(),
                )
            };
            return Ok(Some(StepControl::Continue));
        }
        if (159..=166).contains(&opcode) {
            let mut scratch = MaybeUninit::<i32>::zeroed();
            let target = self.host.reference_argument(
                self,
                frame,
                scratch.as_mut_ptr().cast(),
                std::mem::size_of::<i32>(),
            )?;
            if target.is_null() {
                return Err(RuntimeError::new("Core VM integer assignment has no lvalue"));
            }
            let mut integer_rhs = MaybeUninit::<i32>::zeroed();
            let mut float_rhs = MaybeUninit::<f32>::zeroed();
            if opcode <= 160 {
                unsafe { self.step_into(frame, float_rhs.as_mut_ptr())? };
            } else if opcode <= 162 {
                unsafe { self.step_into(frame, integer_rhs.as_mut_ptr())? };
            }
            unsafe { self.finish(frame)? };
            unsafe {
                vm_scalar::integer_assign(
                    opcode,
                    target.cast(),
                    integer_rhs.assume_init(),
                    float_rhs.assume_init(),
                    result.cast(),
                )
            };
            return Ok(Some(StepControl::Continue));
        }
        if (182..=185).contains(&opcode) {
            let mut scratch = MaybeUninit::<f32>::zeroed();
            let target = self.host.reference_argument(
                self,
                frame,
                scratch.as_mut_ptr().cast(),
                std::mem::size_of::<f32>(),
            )?;
            if target.is_null() {
                return Err(RuntimeError::new("Core VM float assignment has no lvalue"));
            }
            let mut rhs = MaybeUninit::<f32>::zeroed();
            unsafe { self.step_into(frame, rhs.as_mut_ptr())? };
            unsafe { self.finish(frame)? };
            unsafe { vm_scalar::float_assign(opcode, target.cast(), rhs.assume_init(), result.cast()) };
            return Ok(Some(StepControl::Continue));
        }
        if matches!(opcode, 221..=224 | 297) {
            let mut scratch = MaybeUninit::<FVector>::zeroed();
            let target = self.host.reference_argument(
                self,
                frame,
                scratch.as_mut_ptr().cast(),
                std::mem::size_of::<FVector>(),
            )?;
            if target.is_null() {
                return Err(RuntimeError::new("Core VM vector assignment has no lvalue"));
            }
            let mut rhs = MaybeUninit::<FVector>::zeroed();
            let mut scalar = MaybeUninit::<f32>::zeroed();
            let rhs_ptr = if matches!(opcode, 221 | 222) {
                unsafe { self.step_into(frame, scalar.as_mut_ptr())? };
                std::ptr::null()
            } else {
                unsafe { self.step_into(frame, rhs.as_mut_ptr())? };
                rhs.as_ptr()
            };
            unsafe { self.finish(frame)? };
            unsafe {
                vm_vector::vector_assign(
                    opcode,
                    target.cast(),
                    rhs_ptr,
                    unsafe { scalar.assume_init() },
                    result.cast(),
                )
            };
            return Ok(Some(StepControl::Continue));
        }
        if matches!(opcode, 290 | 291 | 318 | 319) {
            let mut scratch = MaybeUninit::<FRotator>::zeroed();
            let target = self.host.reference_argument(
                self,
                frame,
                scratch.as_mut_ptr().cast(),
                std::mem::size_of::<FRotator>(),
            )?;
            if target.is_null() {
                return Err(RuntimeError::new("Core VM rotator assignment has no lvalue"));
            }
            let mut rhs = MaybeUninit::<FRotator>::zeroed();
            let mut scalar = MaybeUninit::<f32>::zeroed();
            let rhs_ptr = if matches!(opcode, 290 | 291) {
                unsafe { self.step_into(frame, scalar.as_mut_ptr())? };
                std::ptr::null()
            } else {
                unsafe { self.step_into(frame, rhs.as_mut_ptr())? };
                rhs.as_ptr()
            };
            unsafe { self.finish(frame)? };
            unsafe {
                vm_vector::rotator_assign(
                    opcode,
                    target.cast(),
                    rhs_ptr,
                    unsafe { scalar.assume_init() },
                    result.cast(),
                )
            };
            return Ok(Some(StepControl::Continue));
        }
        Ok(None)
    }

    unsafe fn execute_axes(
        &self,
        frame: *mut FFrame,
        _result: *mut c_void,
        unaxes: bool,
    ) -> RuntimeResult<()> {
        let mut rotation = MaybeUninit::<FRotator>::uninit();
        unsafe { self.step_into(frame, rotation.as_mut_ptr())? };
        let mut x_temp = MaybeUninit::<FVector>::zeroed();
        let mut y_temp = MaybeUninit::<FVector>::zeroed();
        let mut z_temp = MaybeUninit::<FVector>::zeroed();
        let x = self.host.reference_argument(
            self,
            frame,
            x_temp.as_mut_ptr().cast(),
            std::mem::size_of::<FVector>(),
        )?;
        let y = self.host.reference_argument(
            self,
            frame,
            y_temp.as_mut_ptr().cast(),
            std::mem::size_of::<FVector>(),
        )?;
        let z = self.host.reference_argument(
            self,
            frame,
            z_temp.as_mut_ptr().cast(),
            std::mem::size_of::<FVector>(),
        )?;
        if x.is_null() || y.is_null() || z.is_null() {
            return Err(RuntimeError::new("Core VM GetAxes has a missing output reference"));
        }
        unsafe { self.finish(frame)? };
        let coordinates = vm_vector::axes(
            unsafe { rotation.assume_init() },
            unaxes,
            math::trig_table(),
        );
        unsafe {
            vm_vector::write_axes(coordinates, x.cast(), y.cast(), z.cast());
        }
        Ok(())
    }

    unsafe fn execute_rotate(
        &self,
        frame: *mut FFrame,
        result: *mut c_void,
        unrotate: bool,
    ) -> RuntimeResult<StepControl> {
        let mut vector = MaybeUninit::<FVector>::uninit();
        let mut rotation = MaybeUninit::<FRotator>::uninit();
        unsafe { self.step_into(frame, vector.as_mut_ptr())? };
        unsafe { self.step_into(frame, rotation.as_mut_ptr())? };
        unsafe { self.finish(frame)? };
        let value = vm_vector::rotate(
            unsafe { vector.assume_init() },
            unsafe { rotation.assume_init() },
            unrotate,
            math::trig_table(),
        );
        unsafe { result.cast::<FVector>().write_unaligned(value) };
        Ok(StepControl::Continue)
    }

    unsafe fn execute_ortho_rotation(
        &self,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        let mut x = MaybeUninit::<FVector>::uninit();
        let mut y = MaybeUninit::<FVector>::uninit();
        let mut z = MaybeUninit::<FVector>::uninit();
        unsafe { self.step_into(frame, x.as_mut_ptr())? };
        unsafe { self.step_into(frame, y.as_mut_ptr())? };
        unsafe { self.step_into(frame, z.as_mut_ptr())? };
        unsafe { self.finish(frame)? };
        let value = vm_vector::ortho_rotation(
            unsafe { x.assume_init() },
            unsafe { y.assume_init() },
            unsafe { z.assume_init() },
            math::trig_table(),
        );
        unsafe { result.cast::<FRotator>().write_unaligned(value) };
        Ok(())
    }

    unsafe fn execute_invert(&self, frame: *mut FFrame) -> RuntimeResult<()> {
        let mut x_temp = MaybeUninit::<FVector>::zeroed();
        let mut y_temp = MaybeUninit::<FVector>::zeroed();
        let mut z_temp = MaybeUninit::<FVector>::zeroed();
        let x = self.host.reference_argument(
            self,
            frame,
            x_temp.as_mut_ptr().cast(),
            std::mem::size_of::<FVector>(),
        )?;
        let y = self.host.reference_argument(
            self,
            frame,
            y_temp.as_mut_ptr().cast(),
            std::mem::size_of::<FVector>(),
        )?;
        let z = self.host.reference_argument(
            self,
            frame,
            z_temp.as_mut_ptr().cast(),
            std::mem::size_of::<FVector>(),
        )?;
        if x.is_null() || y.is_null() || z.is_null() {
            return Err(RuntimeError::new("Core VM Invert has a missing vector reference"));
        }
        unsafe { self.finish(frame)? };
        unsafe { vm_vector::invert(x.cast(), y.cast(), z.cast()) };
        Ok(())
    }

    fn plain(&self, frame: *mut FFrame, result: *mut c_void, op: vm_flow::Hp2VmFlowOp) -> StepControl {
        // SAFETY: the direct executor is called only with a live frame and the selected opcode's
        // serialized operand/result widths. execute_plain does not call an external operation.
        unsafe { vm_flow::execute_plain(op, std::ptr::addr_of_mut!((*frame).code), result) };
        StepControl::Continue
    }

    fn vector_constant(&self, frame: *mut FFrame, result: *mut c_void) -> StepControl {
        unsafe { vm_vector::execute_constant(frame, result.cast()) };
        StepControl::Continue
    }

    unsafe fn execute_math(
        &self,
        opcode: u32,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<Option<StepControl>> {
        let mut execute = |encoded_opcode, object, frame, result| {
            self.dispatch_operand(encoded_opcode, object, frame, result)
        };
        if matches!(opcode, 129 | 242 | 243 | 131) {
            let count = if opcode == 129 { 1 } else { 2 };
            unsafe {
                vm_scalar::execute_boolean_opcode_with(
                    opcode,
                    count,
                    frame,
                    result.cast(),
                    &mut execute,
                )?;
            }
            return Ok(Some(StepControl::Continue));
        }

        if matches!(opcode, 141 | 143 | 144..=158 | 196 | 249..=251) {
            let count = match opcode {
                141 | 143 => 1,
                251 => 3,
                _ => 2,
            };
            unsafe {
                vm_scalar::execute_integer_opcode_with(
                    opcode,
                    count,
                    frame,
                    result.cast(),
                    &mut execute,
                )?;
            }
            return Ok(Some(StepControl::Continue));
        }

        if matches!(opcode, 176..=181 | 210) {
            unsafe {
                vm_scalar::execute_float_compare_opcode_with(
                    opcode,
                    frame,
                    result.cast(),
                    &mut execute,
                )?;
            }
            return Ok(Some(StepControl::Continue));
        }

        if matches!(opcode, 169..=175 | 186..=194 | 244..=248) {
            let count = match opcode {
                169 | 186..=194 => 1,
                246..=248 => 3,
                _ => 2,
            };
            unsafe {
                vm_scalar::execute_float_opcode_with(
                    opcode,
                    count,
                    frame,
                    result.cast(),
                    &mut execute,
                )?;
            }
            return Ok(Some(StepControl::Continue));
        }


        if matches!(opcode, 211..=220 | 225 | 226 | 296 | 300) {
            unsafe {
                vm_vector::execute_vector_opcode_with(
                    opcode,
                    frame,
                    result,
                    &mut execute,
                )?;
            }
            return Ok(Some(StepControl::Continue));
        }

        if matches!(opcode, 142 | 203 | 287..=289 | 316..=317 | 1031) {
            unsafe {
                vm_vector::execute_rotator_opcode_with(
                    opcode,
                    frame,
                    result,
                    &mut execute,
                )?;
            }
            return Ok(Some(StepControl::Continue));
        }

        Ok(None)
    }


}

fn property_operation(native: CoreNative) -> Option<VmPropertyOp> {
    use crate::core_native_catalog::CoreNative as N;
    Some(match native {
        N::intUObjectexecLocalVariable => VmPropertyOp::LocalVariable,
        N::intUObjectexecInstanceVariable => VmPropertyOp::InstanceVariable,
        N::intUObjectexecDefaultVariable => VmPropertyOp::DefaultVariable,
        N::intUObjectexecClassContext => VmPropertyOp::ClassContext,
        N::intUObjectexecArrayElement => VmPropertyOp::ArrayElement,
        N::intUObjectexecDynArrayElement => VmPropertyOp::DynamicArrayElement,
        N::intUObjectexecDynArrayCount => VmPropertyOp::DynamicArrayCount,
        N::intUObjectexecBoolVariable => VmPropertyOp::BoolVariable,
        N::intUObjectexecStructMember => VmPropertyOp::StructMember,
        N::intUObjectexecNativeParm => VmPropertyOp::NativeParameter,
        N::intUObjectexecLet => VmPropertyOp::Let,
        N::intUObjectexecLetBool => VmPropertyOp::LetBool,
        N::intUObjectexecSelf => VmPropertyOp::SelfObject,
        N::intUObjectexecContext => VmPropertyOp::Context,
        N::intUObjectexecStructCmpEq => VmPropertyOp::StructEqual,
        N::intUObjectexecStructCmpNe => VmPropertyOp::StructNotEqual,
        _ => return None,
    })
}

fn call_operation(native: CoreNative) -> Option<VmCallOp> {
    use crate::core_native_catalog::CoreNative as N;
    Some(match native {
        N::intUObjectexecVirtualFunction => VmCallOp::Virtual,
        N::intUObjectexecFinalFunction => VmCallOp::Final,
        N::intUObjectexecGlobalFunction => VmCallOp::Global,
        N::intUObjectexecIterator => VmCallOp::Iterator,
        _ => return None,
    })
}

fn flow_operation(native: CoreNative) -> Option<VmFlowOp> {
    use crate::core_native_catalog::CoreNative as N;
    Some(match native {
        N::intUObjectexecDebugInfo => VmFlowOp::DebugInfo,
        N::intUObjectexecSwitch => VmFlowOp::Switch,
        N::intUObjectexecCase => VmFlowOp::Case,
        N::intUObjectexecJump => VmFlowOp::Jump,
        N::intUObjectexecJumpIfNot => VmFlowOp::JumpIfNot,
        N::intUObjectexecAssert => VmFlowOp::Assert,
        N::intUObjectexecGotoLabel => VmFlowOp::GotoLabel,
        _ => return None,
    })
}

fn string_operation(native: CoreNative) -> Option<VmStringOp> {
    use crate::core_native_catalog::CoreNative as N;
    Some(match native {
        N::intUObjectexecEatString => VmStringOp::Eat,
        N::intUObjectexecConcat_StringString => VmStringOp::Concat,
        N::intUObjectexecAt_StringString => VmStringOp::At,
        N::intUObjectexecLess_StringString
        | N::intUObjectexecGreater_StringString
        | N::intUObjectexecLessEqual_StringString
        | N::intUObjectexecGreaterEqual_StringString
        | N::intUObjectexecEqualEqual_StringString
        | N::intUObjectexecNotEqual_StringString => VmStringOp::Compare,
        N::intUObjectexecComplementEqual_StringString => VmStringOp::CaseInsensitiveEqual,
        N::intUObjectexecLen => VmStringOp::Len,
        N::intUObjectexecInStr => VmStringOp::InStr,
        N::intUObjectexecMid => VmStringOp::Mid,
        N::intUObjectexecLeft => VmStringOp::Left,
        N::intUObjectexecRight => VmStringOp::Right,
        N::intUObjectexecCaps => VmStringOp::Caps,
        N::intUObjectexecChr => VmStringOp::Chr,
        N::intUObjectexecAsc => VmStringOp::Asc,
        N::intUObjectexecByteToString
        | N::intUObjectexecIntToString
        | N::intUObjectexecBoolToString
        | N::intUObjectexecFloatToString
        | N::intUObjectexecStringToByte
        | N::intUObjectexecStringToInt
        | N::intUObjectexecStringToBool
        | N::intUObjectexecStringToFloat
        | N::intUObjectexecObjectToString
        | N::intUObjectexecNameToString
        | N::intUObjectexecStringToName
        | N::intUObjectexecStringToVector
        | N::intUObjectexecStringToRotator
        | N::intUObjectexecVectorToString
        | N::intUObjectexecVectorToRotator
        | N::intUObjectexecRotatorToVector
        | N::intUObjectexecRotatorToString => VmStringOp::Convert,
        _ => return None,
    })
}

fn system_operation(native: CoreNative, opcode: u32) -> Option<VmSystemOp> {
    use crate::core_native_catalog::CoreNative as N;
    Some(match native {
        N::intUObjectexecDynamicCast => VmSystemOp::DynamicCast,
        N::intUObjectexecMetaCast => VmSystemOp::MetaCast,
        N::intUObjectexecClassIsChildOf => VmSystemOp::ClassIsChildOf,
        N::intUObjectexecIsA => VmSystemOp::IsA,
        N::intUObjectexecNew => VmSystemOp::NewObject,
        N::intUObjectexecRand => VmSystemOp::Rand,
        N::intUObjectexecFRand => VmSystemOp::FRand,
        N::intUObjectexecVRand => VmSystemOp::VRand,
        N::intUObjectexecRotRand => VmSystemOp::RotRand,
        N::intUObjectexecRandRange => VmSystemOp::RandRange,
        N::intUObjectexecGotoState => VmSystemOp::GotoState,
        N::intUObjectexecEnable => VmSystemOp::Enable,
        N::intUObjectexecDisable => VmSystemOp::Disable,
        N::intUObjectexecIsInState => VmSystemOp::IsInState,
        N::intUObjectexecGetStateName => VmSystemOp::GetStateName,
        N::intUObjectexecLog => VmSystemOp::Log,
        N::intUObjectexecWarn => VmSystemOp::Warn,
        N::intUObjectexecSaveConfig => VmSystemOp::SaveConfig,
        N::intUObjectexecResetConfig => VmSystemOp::ResetConfig,
        N::intUObjectexecLocalize => VmSystemOp::Localize,
        N::intUObjectexecGetPropertyText => VmSystemOp::GetPropertyText,
        N::intUObjectexecSetPropertyText => VmSystemOp::SetPropertyText,
        N::intUObjectexecStaticSaveConfig => VmSystemOp::StaticSaveConfig,
        N::intUObjectexecGetEnum => VmSystemOp::GetEnum,
        N::intUObjectexecGetLanguage => VmSystemOp::GetLanguage,
        N::intUObjectexecDynamicLoadObject => VmSystemOp::DynamicLoadObject,
        N::intUCommandletexecMain => VmSystemOp::CommandletMain,
        N::intUObjectexecHighNative0
        | N::intUObjectexecHighNative1
        | N::intUObjectexecHighNative2
        | N::intUObjectexecHighNative3
        | N::intUObjectexecHighNative4
        | N::intUObjectexecHighNative5
        | N::intUObjectexecHighNative6
        | N::intUObjectexecHighNative7
        | N::intUObjectexecHighNative8
        | N::intUObjectexecHighNative9
        | N::intUObjectexecHighNative10
        | N::intUObjectexecHighNative11
        | N::intUObjectexecHighNative12
        | N::intUObjectexecHighNative13
        | N::intUObjectexecHighNative14
        | N::intUObjectexecHighNative15 => VmSystemOp::NativeTableSlot(opcode),
        _ => return None,
    })
}

fn preserve_first(first: RuntimeResult<()>, later: RuntimeResult<()>) -> RuntimeResult<()> {
    match (first, later) {
        (Ok(()), later) => later,
        (Err(first), Ok(())) => Err(first),
        (Err(mut first), Err(later)) => {
            first = first.with_utf32_context(later.diagnostic().to_vec());
            for context in later.context() {
                first = first.with_utf32_context(context.clone());
            }
            Err(first)
        }
    }
}
