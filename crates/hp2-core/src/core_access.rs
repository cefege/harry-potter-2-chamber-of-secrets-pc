//! Engine-thread Core access and owned diagnostic transport.
//!
//! Diagnostics retain the engine's UTF-32 text and append context in unwind
//! order. Direct Rust callers carry the first failure in `Result`; they do not
//! publish an exception in foreign thread-local storage.

use crate::name_owner::{FindName, NameOwner};
use crate::canonical_vm_native::VmNativeSpec;
use crate::object_collections::ObjectCollection;
use crate::object_owner::OwnedObject;
use hp2_abi::engine::generated::{FFrame, FNameEntry, TArrayRaw, UClass, UFunction, UObject, UProperty, UStruct};
use std::alloc::Layout;
use std::any::Any;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use crate::object_storage::{self, ObjectStorage};

pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Parent-installed bridge to the actual shared EngineState cache owner.
pub type CacheCommandHandler = for<'owner, 'output> unsafe fn(
    CoreAccess<'owner>,
    *const u32,
    &'output mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
) -> RuntimeResult<bool>;

#[derive(Debug, Eq, PartialEq)]
pub struct RuntimeError {
    diagnostic: Vec<u32>,
    context: Vec<Vec<u32>>,
    source_path: Option<PathBuf>,
}
impl RuntimeError {
    pub fn from_utf32(diagnostic: Vec<u32>) -> Self {
        Self {
            diagnostic,
            context: Vec::new(),
            source_path: None,
        }
    }


    pub fn new(diagnostic: &str) -> Self {
        Self::from_utf32(diagnostic.chars().map(u32::from).collect())
    }

    pub fn diagnostic(&self) -> &[u32] {
        &self.diagnostic
    }

    pub fn context(&self) -> &[Vec<u32>] {
        &self.context
    }

    pub fn with_context(mut self, context: &str) -> Self {
        self.context.push(context.chars().map(u32::from).collect());
        self
    }

    /// Attaches the package path as structured source detail without changing history output.
    pub fn with_source_path(mut self, path: impl AsRef<Path>) -> Self {
        self.source_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Returns the source path when the failure originated in a filesystem package.
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    pub fn with_utf32_context(mut self, context: Vec<u32>) -> Self {
        self.context.push(context);
        self
    }
}

fn display_utf32(text: &[u32], output: &mut fmt::Formatter<'_>) -> fmt::Result {
    for &unit in text {
        write!(output, "{}", char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))?;
    }
    Ok(())
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_utf32(&self.diagnostic, output)?;
        if !self.context.is_empty() {
            output.write_str("\n\nHistory: ")?;
            for (index, context) in self.context.iter().enumerate() {
                if index != 0 {
                    output.write_str(" <- ")?;
                }
                display_utf32(context, output)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeError {}

/// Direct output routing. The category is a canonical FName index; handlers may
/// reenter Core and receive no borrowed object or store view.
pub type CoreOutput = for<'a> fn(CoreAccess<'a>, i32, &[u32]) -> RuntimeResult<()>;

/// Rust debugger capability. Its frame borrow lasts only for this invocation.
pub trait ScriptDebugger {
    /// # Safety
    /// Object and frame are live canonical records for the synchronous call.
    /// Implementations may reenter Core but must not retain either raw borrow.
    unsafe fn debug_info(
        &self,
        access: CoreAccess<'_>,
        object: *mut UObject,
        frame: *mut FFrame,
        text: &[u32],
        line: i32,
        input: i32,
    ) -> RuntimeResult<()>;
}

/// Source VM observations over actual live objects, frames and committed results.
#[derive(Clone, Copy)]
pub enum VmObservation {
    ScriptCallEnter { object: *mut UObject, function: *mut UFunction, frame: *mut FFrame },
    ScriptCallExit { object: *mut UObject, function: *mut UFunction, frame: *mut FFrame, result: *mut std::ffi::c_void },
    FunctionEnter { object: *mut UObject, function: *mut UFunction, frame: *mut FFrame },
    FunctionExit { object: *mut UObject, function: *mut UFunction, frame: *mut FFrame, result: *mut std::ffi::c_void },
    Rand { object: *mut UObject, raw: i32, bound: i32, index: i32 },
    RandRange { object: *mut UObject, raw: i32, min: f32, max: f32, value: f32 },
}

/// A synchronous observer may reenter Core, but must not retain live frame borrows.
pub type VmObserver = fn(CoreAccess<'_>, VmObservation) -> RuntimeResult<()>;

/// Optional transaction owner, corresponding to the native `GUndo` resource.
/// SaveArray operation kind used by source `TTransArray` mutation methods.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectArrayTransactionOperation {
    /// `ModifyItem`: record the existing element before it is changed.
    Modify,
    /// `Add`: record the appended range after resizing, before publication.
    Add,
}
pub trait ObjectTransactions {
    /// # Safety
    /// The object is live for this synchronous invocation. Implementations may
    /// reenter Core, but must not retain a borrowed object or storage view.
    unsafe fn save_object(&self, access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()>;

    /// Source-equivalent `FTransactionBase::SaveArray` for a canonical object
    /// array range and operation.
    ///
    /// # Safety
    /// `owner` is the live array owner; `array` is the live `TTransArray`;
    /// `index..index + count` is a valid range, and element width matches its
    /// storage.
    unsafe fn save_object_array_range(
        &self,
        access: CoreAccess<'_>,
        owner: *mut UObject,
        array: *mut hp2_abi::engine::generated::TTransArrayRaw,
        index: i32,
        count: i32,
        operation: ObjectArrayTransactionOperation,
        element_size: usize,
    ) -> RuntimeResult<()>;
}

/// Paths and process inputs owned by one Core runtime.
#[derive(Debug)]
pub struct CoreOptions {
    pub data_root: PathBuf,
    pub system_ini: PathBuf,
    pub user_ini: PathBuf,
    pub command_line: Vec<u32>,
    pub output: CoreOutput,
    pub log_file: Option<std::fs::File>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CoreProcessMode {
    pub is_client: bool,
    pub is_server: bool,
    pub is_editor: bool,
    pub is_scriptable: bool,
    pub lazy_load: bool,
}

struct StartupStructBinding {
    structure: *mut UObject,
    fields: &'static [crate::core_class_specs::StartupStructField],
}


struct CoreState {
    vm_natives: Cell<Option<&'static [VmNativeSpec]>>,
    objects: UnsafeCell<ObjectStorage>,
    names: UnsafeCell<NameOwner>,
    roots: UnsafeCell<ObjectCollection>,
    pending_localized: UnsafeCell<ObjectCollection>,
    metadata: UnsafeCell<crate::class_metadata::MetadataStorage>,
    startup_structs: UnsafeCell<Vec<StartupStructBinding>>,
    cpp_property_offsets: UnsafeCell<std::collections::HashMap<(usize, i32), i32>>,
    loaded_native_modules: UnsafeCell<std::collections::HashSet<usize>>,

    loader: UnsafeCell<crate::core_loader::LoaderState>,
    config: UnsafeCell<crate::core_config::CoreConfig>,
    data_root: PathBuf,
    core_package: Cell<*mut UObject>,
    meta_class: Cell<*mut UClass>,
    transient_package: Cell<*mut UObject>,
    system: Cell<*mut crate::core_records::SystemRecord>,
    process_mode: Cell<CoreProcessMode>,
    no_gc: Cell<i32>,
    running: Cell<bool>,
    command_line: Vec<u32>,
    strict: bool,
    check_conflicts: bool,
    debugger: RefCell<Option<Rc<dyn ScriptDebugger>>>,
    vm_observer: Cell<Option<VmObserver>>,
    transactions: RefCell<Option<Rc<dyn ObjectTransactions>>>,
    cache_command: Cell<Option<CacheCommandHandler>>,
    exit_purge: Cell<bool>,
    output: CoreOutput,
    log_to_stdout: Cell<bool>,
    file_log: RefCell<Option<std::io::BufWriter<std::fs::File>>>,
    warning_name: i32,
    error_name: i32,
    warnings: Cell<i32>,
    errors: Cell<i32>,
    destroy_record: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    owner_context: Option<Box<dyn Any>>,
    memory: crate::mem_owner::MemStackOwner,
}

/// Owns canonical storage; moving this handle never moves an engine record.
pub struct CoreOwner {
    state: Box<CoreState>,
    objects_closed: Cell<bool>,
    closed: Cell<bool>,
}

impl CoreOwner {
    /// Initializes the native Core graph and its actual constructors/defaults.
    /// No access capability escapes while bootstrap or registration is incomplete.
    pub fn new(options: CoreOptions) -> RuntimeResult<Self> {
        Self::initialize_native_classes(options, &[], None)
    }

    /// Initializes Core and additional immutable native class declarations in one canonical graph.
    ///
    /// # Safety
    /// Every declaration must outlive this owner and describe a complete ABI:
    /// layouts, property offsets, lifecycle operations, serialization, GC visits,
    /// post-load behavior, and callback pointer obligations must match each type.
    /// Operation functions must obey the same access, reentrancy, and destruction
    /// contracts as Core's Rust `ClassOps`; layout must include the UObject prefix.
    pub unsafe fn with_native_classes(
        options: CoreOptions, declarations: &'static [crate::core_class_specs::NativeClassSpec],
    ) -> RuntimeResult<Self> {
        Self::initialize_native_classes(options, declarations, None)
    }

    /// Initializes native classes with immutable higher-layer state available to callbacks.
    ///
    /// The context is retained until after bootstrap-failure cleanup, class operation
    /// destruction, and all object freeing have completed.
    ///
    /// # Safety
    /// Every declaration must outlive this owner and describe a complete ABI:
    /// layouts, property offsets, lifecycle operations, serialization, GC visits,
    /// post-load behavior, and callback pointer obligations must match each type.
    /// Operation functions must obey the same access, reentrancy, and destruction
    /// contracts as Core's Rust `ClassOps`; layout must include the UObject prefix.
    /// Context destruction must not access this owner or invoke callbacks into it.
    pub unsafe fn with_native_classes_and_context<T: 'static>(
        options: CoreOptions,
        declarations: &'static [crate::core_class_specs::NativeClassSpec],
        context: T,
    ) -> RuntimeResult<Self> {
        Self::initialize_native_classes(options, declarations, Some(Box::new(context)))
    }

    fn initialize_native_classes(
        options: CoreOptions,
        declarations: &'static [crate::core_class_specs::NativeClassSpec],
        owner_context: Option<Box<dyn Any>>,
    ) -> RuntimeResult<Self> {
        let owner = unsafe {
            Self::empty(crate::core_reflection::destroy_record, options, owner_context)?
        };
        let access = owner.access();
        unsafe { crate::core_bootstrap::publish_native_classes(access, declarations)?; }
        for declaration in crate::core_class_specs::NATIVE_CLASSES.iter().chain(declarations) {
            let class = access.native_class_in_package(declaration.package, declaration.short_name)?;
            if unsafe { (*class).defaults.data.is_null() } {
                unsafe { access.register_class(class)?; }
            }
        }
        Ok(owner)
    }

    /// # Safety
    /// `destroy_record` must visit the actual class operation metadata and tear
    /// down every initialized field even when it returns the first hook error.
    /// It must not deallocate records; all destruction precedes all freeing.
    pub(crate) unsafe fn empty(
        destroy_record: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
        mut options: CoreOptions,
        owner_context: Option<Box<dyn Any>>,
    ) -> RuntimeResult<Self> {
        // Native file leaves model a stable working directory; retain that
        // identity once instead of prepending a relative data root repeatedly.
        if options.data_root.is_relative() {
            options.data_root = std::path::absolute(&options.data_root).map_err(|error| {
                RuntimeError::new("Data root could not be made absolute")
                    .with_context(&error.to_string())
            })?;
        }
        if options.command_line.last() != Some(&0) { options.command_line.push(0); }
        // SAFETY: the command line and fixed parameter are terminated UTF-32/TCHAR data.
        let strict = unsafe {
            crate::search::parse_param(options.command_line.as_ptr().cast(), [83, 84, 82, 73, 67, 84, 0].as_ptr())
        };
        let check_conflicts = unsafe {
            crate::search::parse_param(
                options.command_line.as_ptr().cast(), crate::lit!("CONFLICTS").as_ptr().cast(),
            )
        };
        let mut names = NameOwner::new();
        names.initialize()?;
        let warning_name = names.find(&[87, 97, 114, 110, 105, 110, 103])?.index;
        let error_name = names.find(&[69, 114, 114, 111, 114])?.index;
        let config = crate::core_config::CoreConfig::new(
            options.data_root.clone(), options.system_ini, options.user_ini,
        );
        Ok(Self {
            state: Box::new(CoreState {
                objects: UnsafeCell::new(ObjectStorage::new()),
                names: UnsafeCell::new(names),
                roots: UnsafeCell::new(ObjectCollection::new()),
                pending_localized: UnsafeCell::new(ObjectCollection::new()),
                metadata: UnsafeCell::new(crate::class_metadata::MetadataStorage::new()),
                startup_structs: UnsafeCell::new(Vec::new()),
                cpp_property_offsets: UnsafeCell::new(std::collections::HashMap::new()),
                loaded_native_modules: UnsafeCell::new(std::collections::HashSet::new()),

                loader: UnsafeCell::new(crate::core_loader::LoaderState::new()),
                config: UnsafeCell::new(config),
                data_root: options.data_root,
                core_package: Cell::new(std::ptr::null_mut()),
                meta_class: Cell::new(std::ptr::null_mut()),
                transient_package: Cell::new(std::ptr::null_mut()),
                system: Cell::new(std::ptr::null_mut()),
                process_mode: Cell::new(CoreProcessMode::default()),
                no_gc: Cell::new(0),
                running: Cell::new(false),
                command_line: options.command_line,
                strict,
                check_conflicts,
                debugger: RefCell::new(None),
                vm_observer: Cell::new(None),
                transactions: RefCell::new(None),
                cache_command: Cell::new(None),
                exit_purge: Cell::new(false),
                output: options.output,
                log_to_stdout: Cell::new(false),
                file_log: RefCell::new(options.log_file.map(std::io::BufWriter::new)),
                warning_name,
                error_name,
                warnings: Cell::new(0),
                errors: Cell::new(0),
                vm_natives: Cell::new(None),
                destroy_record,
                owner_context,
                memory: crate::mem_owner::MemStackOwner::new(65_536, &[71, 77, 101, 109, 0]),
            }),
            objects_closed: Cell::new(false),
            closed: Cell::new(false),
        })
    }

    pub fn access(&self) -> CoreAccess<'_> {
        CoreAccess {
            state: &self.state,
            objects_closed: &self.objects_closed,
            _engine_thread: PhantomData,
        }
    }

    /// Runs virtual Destroy for all live slots in reverse order. A separate reverse
    /// pass finalizes members and releases slots; publication closes at pre-exit start.
    pub fn shutdown(self) -> RuntimeResult<()> {
        self.shutdown_inner()
    }

    /// Performs source `appPreExit` destruction before final object/storage release.
    pub fn pre_exit(&self) -> RuntimeResult<()> {
        if self.objects_closed.replace(true) {
            return Ok(());
        }
        self.state.exit_purge.set(true);
        let access = self.access();
        let mut first_error = None;
        loop {
            let mut destroyed = false;
            let mut index = access.object_count() - 1;
            while index >= 0 {
                if unsafe { (*access.storage()).needs_teardown(index) } {
                    destroyed = true;
                    let result = unsafe { access.destroy_slot(index) };
                    if first_error.is_none() {
                        first_error = result.err();
                    }
                }
                index -= 1;
            }
            if !destroyed {
                break;
            }
        }
        let mut index = access.object_count() - 1;
        while index >= 0 {
            if unsafe { (*access.storage()).is_owned(index) } {
                let result = unsafe { access.free_slot(index) };
                if first_error.is_none() {
                    first_error = result.err();
                }
            }
            index -= 1;
        }
        // No user operation runs while the canonical collections are released.
        unsafe {
            (*access.storage()).release();
            (*access.state.roots.get()).release();
            (*access.state.pending_localized.get()).release();
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn shutdown_inner(&self) -> RuntimeResult<()> {
        if self.closed.replace(true) {
            return Ok(());
        }
        let mut first_error = self.pre_exit().err();
        let access = self.access();
        if let Err(error) = access.scratch_memory().exit() {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
        // Native appExit flushes configuration after object pre-exit and before
        // releasing names. Preserve that ordering even after a hook failed.
        if let Err(error) = unsafe { (&mut *access.config_storage()).flush(None, false) } {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
        access.flush_vm_trace();
        unsafe { (&mut *access.state.names.get()).memory_cleanup(); }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for CoreOwner {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_inner() {
            eprintln!("{error}");
        }
    }
}

/// Engine-thread access without an exclusive borrow of the runtime.
///
/// Methods copy state or finish a short interior mutation before returning.
/// Hooks receive another capability, never a live store or object-body borrow.
#[derive(Clone, Copy)]
pub struct CoreAccess<'a> {
    state: &'a CoreState,
    objects_closed: &'a Cell<bool>,
    _engine_thread: PhantomData<Rc<()>>,
}

impl<'a> CoreAccess<'a> {
    pub(crate) fn loader_state(self) -> *mut crate::core_loader::LoaderState {
        self.state.loader.get()
    }

    pub(crate) fn config_storage(self) -> *mut crate::core_config::CoreConfig {
        self.state.config.get()
    }

    pub(crate) fn set_log_to_stdout(self, enabled: bool) {
        self.state.log_to_stdout.set(enabled);
    }

    pub(crate) fn log_to_stdout(self) -> bool {
        self.state.log_to_stdout.get()
    }

    /// Writes a complete encoded record without invoking an engine callback.
    pub(crate) fn write_log_file(self, bytes: &[u8]) -> RuntimeResult<()> {
        use std::io::Write;
        if let Some(file) = self.state.file_log.borrow_mut().as_mut() {
            file.write_all(bytes)
                .and_then(|()| file.flush())
                .map_err(|error| RuntimeError::new(&format!("Error writing log: {error}")))?;
        }
        Ok(())
    }

    /// Writes one UTF-16LE category/text record through the owned file sink.
    pub fn write_log_record(self, event: &[u32], text: &[u32]) -> RuntimeResult<()> {
        let mut bytes = Vec::with_capacity((event.len() + text.len() + 4) * 4);
        for unit in event.iter().copied().chain([b':' as u32, b' ' as u32])
            .chain(text.iter().copied().take_while(|&unit| unit != 0))
            .chain([b'\r' as u32, b'\n' as u32])
        {
            let scalar = char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER);
            let mut encoded = [0u16; 2];
            for value in scalar.encode_utf16(&mut encoded).iter().copied() {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        self.write_log_file(&bytes)
    }

    pub fn set_debugger(self, debugger: Option<Rc<dyn ScriptDebugger>>) {
        // A retiring handler's Drop may reenter Core; release the cell borrow.
        let previous = self.state.debugger.replace(debugger);
        drop(previous);
    }

    /// Installs the actual canonical VM's optional synchronous observation receiver.
    pub fn set_vm_observer(self, observer: Option<VmObserver>) {
        self.state.vm_observer.set(observer);
    }

    /// Detaches only this receiver, without clearing a subsequent replacement.
    pub fn clear_vm_observer(self, observer: VmObserver) {
        if self.state.vm_observer.get().is_some_and(|current| std::ptr::fn_addr_eq(current, observer)) {
            self.state.vm_observer.set(None);
        }
    }

    pub(crate) fn observe_vm(self, event: VmObservation) -> RuntimeResult<()> {
        match self.state.vm_observer.get() {
            Some(observer) => observer(self, event),
            None => Ok(()),
        }
    }

    pub(crate) fn with_interpreted_call(
        self, object: *mut UObject, function: *mut UFunction, frame: *mut FFrame,
        result: *mut std::ffi::c_void, from_script_call: bool,
        execute: impl FnOnce() -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        use crate::vm_dispatch::{InternalEntryQuery, FUNC_SINGULAR};
        // Capture once, like GScriptDispatchObserver around ProcessInternal.
        let observer = self.state.vm_observer.get();
        if from_script_call && let Some(observer) = observer {
            observer(self, VmObservation::ScriptCallEnter { object, function, frame })?;
        }
        let singular = unsafe { (*function).function_flags } & FUNC_SINGULAR;
        let accepted = crate::vm_dispatch::accept_internal_with(|query| {
            match query {
                InternalEntryQuery::RemoteConsumed => unsafe {
                    self.process_remote_function(object, function, (*frame).locals.cast(), std::ptr::null_mut())
                },
                InternalEntryQuery::Probing => unsafe {
                    self.is_probing(object, i32::from_ne_bytes((*function.cast::<UObject>()).name))
                },
                InternalEntryQuery::SingularActive => Ok(unsafe { (*object).object_flags } & singular != 0),
            }
        })?;
        if accepted {
            if let Some(observer) = observer {
                observer(self, VmObservation::FunctionEnter { object, function, frame })?;
            }
            unsafe { (*object).object_flags |= singular; }
            execute()?;
            if let Some(observer) = observer {
                observer(self, VmObservation::FunctionExit { object, function, frame, result })?;
            }
            unsafe { (*object).object_flags &= !singular; }
        }
        if from_script_call && let Some(observer) = observer {
            observer(self, VmObservation::ScriptCallExit { object, function, frame, result })?;
        }
        Ok(())
    }

    pub fn set_transactions(self, transactions: Option<Rc<dyn ObjectTransactions>>) {
        let previous = self.state.transactions.replace(transactions);
        drop(previous);
    }

    /// Whether object modification can invoke a transaction callback.
    pub(crate) fn has_object_transactions(self) -> bool {
        self.state.transactions.borrow().is_some()
    }

    /// # Safety
    /// The object is live and remains valid through transaction callback reentry.
    pub unsafe fn modify_object(self, object: *mut UObject) -> RuntimeResult<()> {
        let transactions = self.state.transactions.borrow().clone();
        if let Some(transactions) = transactions {
            if unsafe { (*object).object_flags & 0x0000_0001 != 0 } {
                unsafe { transactions.save_object(self, object)?; }
            }
        }
        Ok(())
    }

    /// `TTransArray<UObject*>::ModifyItem(index)` with native one-element
    /// `SaveArray` modify semantics.
    ///
    /// # Safety
    /// `array` is a live object-reference `TTransArray` and `index` is valid.
    pub unsafe fn modify_trans_array_item(
        self,
        array: *mut hp2_abi::engine::generated::TTransArrayRaw,
        index: i32,
    ) -> RuntimeResult<()> {
        unsafe {
            self.trans_array_transaction(
                array,
                index,
                1,
                ObjectArrayTransactionOperation::Modify,
            )
        }
    }

    /// Records the source `TTransArray::Add(count)` transaction for an already
    /// resized object-reference array, matching `SaveArray(index,count,1)`.
    ///
    /// # Safety
    /// `array` is live, contains the appended range, and `index..index + count`
    /// identifies that newly-added range.
    pub unsafe fn add_trans_array_items(
        self,
        array: *mut hp2_abi::engine::generated::TTransArrayRaw,
        index: i32,
        count: i32,
    ) -> RuntimeResult<()> {
        unsafe {
            self.trans_array_transaction(array, index, count, ObjectArrayTransactionOperation::Add)
        }
    }

    unsafe fn trans_array_transaction(
        self,
        array: *mut hp2_abi::engine::generated::TTransArrayRaw,
        index: i32,
        count: i32,
        operation: ObjectArrayTransactionOperation,
    ) -> RuntimeResult<()> {
        if array.is_null() {
            return Err(RuntimeError::new("Cannot transact a null trans-array"));
        }
        if index < 0
            || count <= 0
            || index
                .checked_add(count)
                .map_or(true, |end| end > unsafe { (*array).array_num })
        {
            return Err(RuntimeError::new("Invalid trans-array transaction range"));
        }
        let owner = unsafe { (*array).owner.cast::<UObject>() };
        if owner.is_null() {
            return Err(RuntimeError::new("Trans-array has no transaction owner"));
        }
        let transactions = self.state.transactions.borrow().clone();
        if let Some(transactions) = transactions {
            unsafe {
                transactions.save_object_array_range(
                    self,
                    owner,
                    array,
                    index,
                    count,
                    operation,
                    core::mem::size_of::<*mut UObject>(),
                )?;
            }
        }
        Ok(())
    }

    /// # Safety
    /// Object and frame remain live for synchronous debugger reentry.
    pub(crate) unsafe fn debug_info(
        self, object: *mut UObject, frame: *mut FFrame, text: &[u32], line: i32, input: i32,
    ) -> RuntimeResult<()> {
        let debugger = self.state.debugger.borrow().clone();
        if let Some(debugger) = debugger {
            unsafe { debugger.debug_info(self, object, frame, text, line, input)?; }
        }
        Ok(())
    }

    /// Returns the owner context when its concrete type is exactly `T`.
    ///
    /// The reference is bounded by this access capability and cannot outlive the
    /// owning Core runtime.
    pub fn owner_context<T: 'static>(self) -> Option<&'a T> {
        self.state.owner_context.as_deref()?.downcast_ref::<T>()
    }

    /// Installs an immutable external native catalog before reflected Engine metadata is loaded.
    ///
    /// Core native identities always take precedence over this catalog.
    ///
    /// # Errors
    /// Returns an error if a catalog was already installed.
    pub fn register_vm_natives(self, natives: &'static [VmNativeSpec]) -> RuntimeResult<()> {
        if self.state.vm_natives.get().is_some() {
            return Err(RuntimeError::new("Canonical VM native catalog is already registered"));
        }
        self.state.vm_natives.set(Some(natives));
        Ok(())
    }

    pub(crate) fn vm_natives(self) -> Option<&'static [VmNativeSpec]> {
        self.state.vm_natives.get()
    }

    /// Returns the current process-mode flags used by Core.
    pub fn process_mode(self) -> CoreProcessMode {
        self.state.process_mode.get()
    }

    /// Replaces the process-mode flags used by Core.
    pub fn set_process_mode(self, mode: CoreProcessMode) {
        self.state.process_mode.set(mode);
    }

    pub(crate) fn check_name_conflicts(self) -> bool {
        self.state.check_conflicts
    }

    /// Returns Core's current global garbage-collection suppression flag.
    pub fn no_gc(self) -> i32 {
        self.state.no_gc.get()
    }

    /// Sets Core's global garbage-collection suppression flag.
    pub fn set_no_gc(self, no_gc: i32) {
        self.state.no_gc.set(no_gc);
    }

    /// Returns GIsRunning for this application owner, independent of process-mode flags.
    pub fn is_running(self) -> bool {
        self.state.running.get()
    }

    /// Sets GIsRunning at application-loop entry/exit, not at Engine construction.
    pub fn set_running(self, running: bool) {
        self.state.running.set(running);
    }

    pub(crate) fn is_client(self) -> bool { self.process_mode().is_client }
    pub(crate) fn is_server(self) -> bool { self.process_mode().is_server }
    pub(crate) fn is_editor(self) -> bool { self.process_mode().is_editor }
    pub(crate) fn is_scriptable(self) -> bool { self.process_mode().is_scriptable }
    pub(crate) fn lazy_load(self) -> bool { self.process_mode().lazy_load }
    /// Returns whether Core is inside a package BeginLoad scope.
    pub fn is_loading(self) -> bool {
        unsafe { (*self.loader_state()).begin_load_count != 0 }
    }
    pub(crate) fn is_strict(self) -> bool { self.state.strict }

    /// Returns the canonical FName slot's RF_Suppress flag (FName::SafeSuppressed).
    pub fn name_suppressed(self, name: i32) -> bool {
        // RF_Suppress is mirrored in native UnName.h and UnObjBas.h.
        unsafe { (&*self.state.names.get()).flags(name).is_some_and(|flags| flags & 0x0000_1000 != 0) }
    }

    /// Returns the owning application's command line without copying its storage.
    pub fn command_line(&self) -> &[u32] {
        &self.state.command_line
    }

    pub(crate) fn vm_runaway_step(self) -> bool {
        // VM execution, including guard reset, is process-wide and serialized.
        unsafe { crate::vm_guard::step_runaway() }
    }

    pub(crate) fn vm_reset_runaway(self) {
        unsafe { crate::vm_guard::reset_runaway() };
    }

    /// Advances the canonical RNG while updating the process-wide trace owner.
    pub fn rand(self) -> i32 {
        crate::ansi_trace::with_process_state(crate::ansi_trace::direct_rand_value)
    }

    /// Advances the canonical RNG once and applies the native float range formula.
    pub fn rand_range_float(self, min: f32, max: f32) -> f32 {
        crate::ansi_trace::with_process_state(|trace| {
            crate::ansi_trace::direct_rand_range_float(trace, min, max)
        })
    }

    pub(crate) fn vm_rand(self) -> i32 {
        self.rand()
    }


    /// Flushes trace output during orderly Core teardown.
    pub(crate) fn flush_vm_trace(self) {
        crate::ansi_trace::flush_process_state();
    }
    pub(crate) fn vm_trace_enabled(self) -> bool {
        crate::ansi_trace::with_process_state(|trace| trace.enabled && !trace.flushed)
    }

    pub(crate) fn vm_enter_event(self) -> bool {
        unsafe { crate::vm_guard::enter_recursion() }
    }

    pub(crate) fn vm_exit_event(self) {
        unsafe { crate::vm_guard::leave_recursion() };
    }

    pub(crate) fn reset_vm_guards(self) {
        unsafe { crate::vm_guard::reset(1) };
    }

    /// Returns Core's configured data root for synchronous path policy.
    pub fn data_root(&self) -> &Path {
        &self.state.data_root
    }

    /// Returns the one rooted System configuration object owned by this Core.
    pub fn system(self) -> RuntimeResult<*mut crate::core_records::SystemRecord> {
        let system = self.state.system.get();
        if !system.is_null() { return Ok(system); }
        let object = unsafe {
            self.construct_object(
                self.native_class("Core.System")?, self.transient_package(),
                0, 0, std::ptr::null_mut(),
            )?
        };
        // Native GSys is process-lived; this identity is scoped to its Core owner.
        unsafe { self.add_root(object); }
        let system = object.cast();
        self.state.system.set(system);
        Ok(system)
    }

    /// Performs appPlatformInit's rooted System configuration, name flags, and shared
    /// save/map/cache path initialization phase.
    ///
    /// # Errors
    /// Rejects missing platform search paths, invalid configured names, or failed
    /// native save/map/cache initialization.
    pub fn initialize_application_system(self) -> RuntimeResult<*mut crate::core_records::SystemRecord> {
        let system = self.system()?;
        // SAFETY: system() returns this owner's live, complete native System record.
        let (paths, suppress) = unsafe { ((*system).paths, (*system).suppress) };
        if paths.num <= 5 {
            return Err(RuntimeError::new("Core.System Paths did not load from System.ini"));
        }
        if suppress.num < 0 || (suppress.num > 0 && suppress.data.is_null()) {
            return Err(RuntimeError::new("Invalid Core.System Suppress array"));
        }
        for index in 0..suppress.num as usize {
            // SAFETY: reflected ArrayOfName storage has num live four-byte FNames.
            let name = unsafe { suppress.data.cast::<i32>().add(index).read() };
            let entry = self.name_at(name);
            if entry.is_null() {
                return Err(RuntimeError::new("Invalid Core.System Suppress name"));
            }
            // RF_Suppress is applied to the real interned entry, preserving other flags.
            unsafe { (*entry).flags |= 0x0000_1000; }
        }
        // SAFETY: `system` came from this owner's `system()` above and was just
        // checked for its Paths and Suppress storage; nothing else mutates it
        // during this synchronous call.
        unsafe { crate::native_runtime_init::initialize(self, system)? };
        Ok(system)
    }

    /// Reads a value from the canonical configuration cache.
    pub fn config_get_units(
        self,
        section: &[u32],
        key: &[u32],
    ) -> RuntimeResult<Option<Vec<u32>>> {
        self.config_get(section, key)
    }

    /// Reads a value from a selected canonical configuration file.
    pub fn config_get_units_from(
        self,
        filename: Option<&[u32]>,
        section: &[u32],
        key: &[u32],
    ) -> RuntimeResult<Option<Vec<u32>>> {
        unsafe { &mut *self.config_storage() }.get(filename, section, key)
    }

    /// Reads a configuration section in the cache's native duplicate order.
    pub fn config_section_units(
        self,
        section: &[u32],
        filename: Option<&[u32]>,
    ) -> RuntimeResult<Vec<(Vec<u32>, Vec<u32>)>> {
        unsafe { &mut *self.config_storage() }.section_pairs(filename, section)
    }

    /// Sets a value in the canonical configuration cache.
    pub fn config_set_units(
        self,
        section: &[u32],
        key: &[u32],
        value: &[u32],
    ) -> RuntimeResult<()> {
        self.config_set(section, key, value)
    }

    /// Sets a value in a selected canonical configuration file.
    pub fn config_set_units_from(
        self,
        filename: Option<&[u32]>,
        section: &[u32],
        key: &[u32],
        value: &[u32],
    ) -> RuntimeResult<()> {
        unsafe { &mut *self.config_storage() }.set(filename, section, key, value)
    }

    /// Flushes a selected canonical configuration file to its configured path.
    pub fn config_flush_units_from(
        self,
        filename: Option<&[u32]>,
        reload: bool,
    ) -> RuntimeResult<()> {
        unsafe { &mut *self.config_storage() }.flush(filename, reload)
    }


    pub(crate) fn metadata_storage(self) -> *mut crate::class_metadata::MetadataStorage {
        self.state.metadata.get()
    }

    /// Adds a native C++ string/name/struct property to a canonical class and
    /// records it for the ordinary Core property linker.
    ///
    /// `property_class` is one of the registered `StrProperty`, `NameProperty`,
    /// or `StructProperty` classes. The returned property is a canonical object.
    ///
    /// # Safety
    /// `structure` and any `inner_struct` must remain live for the class lifetime;
    /// `offset` and `array_dim` must match its compiler-probed native layout.
    pub unsafe fn add_cpp_property(
        self,
        structure: *mut UStruct,
        property_class: &str,
        name: i32,
        category: i32,
        offset: usize,
        array_dim: usize,
        property_flags: u32,
        object_flags: u32,
        inner_struct: *mut UStruct,
    ) -> RuntimeResult<*mut UProperty> {
        if structure.is_null() { return Err(RuntimeError::new("Null native property owner")); }
        let class = self.native_class(property_class)?;
        let property = unsafe { self.construct_object(class, structure.cast(), name, object_flags, std::ptr::null_mut())? }
            .cast::<UProperty>();
        unsafe {
            crate::native_property_offsets::initialize_cpp_prefix_data(property, property_flags);
            (*property).category = category.to_ne_bytes();
            (*property).offset = i32::try_from(offset).map_err(|_| RuntimeError::new("Native property offset overflow"))?;
            (*property).array_dim = i32::try_from(array_dim).map_err(|_| RuntimeError::new("Native property array dimension overflow"))?;
            (*property).cpp_property = 1;
            if property_class == "StructProperty" {
                if inner_struct.is_null() { return Err(RuntimeError::new("Null struct property target")); }
                (*property.cast::<hp2_abi::engine::generated::UStructProperty>()).r#struct = inner_struct.cast();
            }
            crate::native_property_offsets::add_cpp_property(structure, property.cast());
        }
        // Keep constructor-authored layout separate from serialized property slots.
        // NAME_None requests receive a generated constructor name: register that
        // actual identity and preserve the first registration, as native code does.
        let constructed_name = i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name });
        unsafe { &mut *self.state.cpp_property_offsets.get() }
            .entry((structure as usize, constructed_name)).or_insert(offset as i32);
        Ok(property)
    }

    pub(crate) fn cpp_property_offset(self, owner: *mut UObject, name: i32) -> Option<i32> {
        // No registry borrow survives this synchronous metadata lookup.
        unsafe { &*self.state.cpp_property_offsets.get() }.get(&(owner as usize, name)).copied()
    }

    /// Creates a canonical native UStruct record for a dynamic C++ property layout.
    ///
    /// # Safety
    /// `name` and `size` must match the native declaration exactly.
    pub unsafe fn create_cpp_struct(
        self,
        outer: *mut UObject,
        name: i32,
        size: usize,
        object_flags: u32,
    ) -> RuntimeResult<*mut UStruct> {
        let class = self.native_class("Struct")?;
        let structure = unsafe { self.construct_object(class, outer, name, object_flags, std::ptr::null_mut())? }
            .cast::<UStruct>();
        unsafe {
            (*structure).properties_size = i32::try_from(size)
                .map_err(|_| RuntimeError::new("Native struct size overflow"))?;
            (*structure).properties_align = std::mem::align_of::<usize>() as i32;
        }
        Ok(structure)
    }

    /// Links dynamic native fields without recalculating compiler-authored struct storage.
    ///
    /// # Safety
    /// `structure` and all property children must be canonical and fully initialized.
    pub unsafe fn link_cpp_struct(self, structure: *mut UStruct) -> RuntimeResult<()> {
        unsafe { self.link_struct(structure, None, false) }
    }
    pub(crate) fn register_startup_struct(
        self,
        structure: *mut UObject,
        fields: &'static [crate::core_class_specs::StartupStructField],
    ) -> RuntimeResult<()> {
        let declarations = unsafe { &mut *self.state.startup_structs.get() };
        if let Some(existing) = declarations.iter().find(|entry| entry.structure == structure) {
            if existing.fields != fields {
                return Err(RuntimeError::new("Conflicting native struct declaration"));
            }
            return Ok(());
        }
        declarations.push(StartupStructBinding { structure, fields });
        Ok(())
    }

    pub(crate) fn startup_struct_fields(
        self,
        structure: *mut UObject,
    ) -> Option<&'static [crate::core_class_specs::StartupStructField]> {
        unsafe { &*self.state.startup_structs.get() }
            .iter()
            .find(|entry| entry.structure == structure)
            .map(|entry| entry.fields)
    }

    pub fn core_package(self) -> *mut UObject {
        self.state.core_package.get()
    }

    pub fn transient_package(self) -> *mut UObject {
        self.state.transient_package.get()
    }

    /// # Safety
    /// All three identities are published canonical records with initialized
    /// class metadata. This is called once before exposing the runtime.
    pub(crate) unsafe fn set_bootstrap_roots(
        self, core: *mut UObject, meta: *mut UClass, transient: *mut UObject,
    ) {
        self.state.core_package.set(core);
        self.state.meta_class.set(meta);
        self.state.transient_package.set(transient);
    }

    /// UTF-8 boundary for startup declarations and command-line names.
    /// The established name limit also bounds temporary conversion storage.
    pub fn intern_str(self, text: &str, mode: FindName) -> RuntimeResult<i32> {
        let mut units = [0; crate::name::NAME_SIZE];
        let mut count = 0;
        for character in text.chars().take_while(|&character| character != '\0') {
            if count == units.len() {
                return if mode == FindName::Find {
                    Ok(0)
                } else {
                    Err(RuntimeError::new("NameLength<NAME_SIZE"))
                };
            }
            units[count] = u32::from(character);
            count += 1;
        }
        self.intern(&units[..count], mode)
    }

    pub fn native_class(self, name: &str) -> RuntimeResult<*mut UClass> {
        let (package_name, short_name) = name.split_once('.').unwrap_or(("Core", name));
        self.native_class_in_package(package_name, short_name)
            .map_err(|error| if package_name == "Core" { error.with_context(name) } else { error })
    }

    fn native_class_in_package(self, package_name: &str, short_name: &str) -> RuntimeResult<*mut UClass> {
        let meta = self.state.meta_class.get();
        if meta.is_null() {
            return Err(RuntimeError::new("Core native classes are not initialized"));
        }
        let package_class_name = self.intern_str("Package", FindName::Find)?;
        let package_class = unsafe { self.find_object(meta, self.core_package(), package_class_name, true) };
        if package_class.is_null() {
            return Err(RuntimeError::new("Core Package class is not initialized"));
        }
        let package = if package_name == "Core" {
            self.core_package()
        } else {
            let package_index = self.intern_str(package_name, FindName::Find)?;
            unsafe { self.find_object(package_class.cast(), std::ptr::null_mut(), package_index, true) }
        };
        if package.is_null() {
            return Err(RuntimeError::new("Native class package not found").with_context(package_name));
        }
        let name_index = self.intern_str(short_name, FindName::Find)?;
        let object = unsafe { self.find_object(meta, package, name_index, true) };
        if object.is_null() {
            return Err(RuntimeError::new("Native class not found")
                .with_context(&format!("{package_name}.{short_name}")));
        }
        Ok(object.cast())
    }

    /// Records an actual compiled Rust module's package, the canonical counterpart
    /// of a loaded package DLL. Native class roots retain the package's lifetime.
    pub(crate) fn register_loaded_native_module(self, package: *mut UObject) {
        unsafe { (&mut *self.state.loaded_native_modules.get()).insert(package as usize); }
    }

    pub(crate) fn has_loaded_native_module(self, package: *mut UObject) -> bool {
        unsafe { (&*self.state.loaded_native_modules.get()).contains(&(package as usize)) }
    }

    /// Assigns NUL-terminated text to a live FString using Core's Rust-owned allocator.
    ///
    /// # Errors
    /// Returns an error if `terminated_text` lacks a terminal NUL or storage allocation fails.
    ///
    /// # Safety
    /// `destination` must be an initialized FString exclusively borrowed for the
    /// call. The terminated input must remain readable and must not overlap its
    /// storage.
    pub unsafe fn assign_fstring(
        self, destination: *mut TArrayRaw, terminated_text: &[u32],
    ) -> RuntimeResult<()> {
        if terminated_text.last() != Some(&0) {
            return Err(RuntimeError::new("FString assignment text is not terminated"));
        }
        let slots = crate::array_storage::Hp2ArraySlots {
            data: unsafe { std::ptr::addr_of_mut!((*destination).data) },
            num: unsafe { std::ptr::addr_of_mut!((*destination).num) },
            max: unsafe { std::ptr::addr_of_mut!((*destination).max) },
        };
        unsafe {
            crate::string_storage::assign_with_allocator(
                slots,
                None,
                terminated_text.as_ptr().cast(),
                false,
                crate::array_storage::system_allocator(),
                std::ptr::null(),
            )
        }
        .map_err(|_| RuntimeError::new("Unable to assign FString"))
    }

    /// Writes a native FString result with the allocator used by Core VM string expressions.
    ///
    /// # Errors
    /// Returns an error if the length exceeds the engine's signed array limit or allocation fails.
    ///
    /// # Safety
    /// `destination` must be an initialized FString result slot exclusively borrowed for this
    /// call. `text` must remain readable and must not overlap the destination allocation.
    pub unsafe fn write_vm_fstring_result(
        self,
        destination: *mut TArrayRaw,
        text: &[crate::ansi::Tchar],
    ) -> RuntimeResult<()> {
        unsafe { self.write_vm_fstring_parts(destination, &[text]) }
    }

    /// Writes concatenated C-string fragments directly into the final FString allocation.
    ///
    /// # Safety
    /// Destination is initialized Core FString storage; fragments are readable and
    /// do not overlap its allocation. Each fragment stops at its first NUL.
    pub unsafe fn write_vm_fstring_parts<const N: usize>(
        self, destination: *mut TArrayRaw, parts: &[&[crate::ansi::Tchar]; N],
    ) -> RuntimeResult<()> {
        let lengths = parts.map(|part| part.iter().position(|unit| *unit == 0).unwrap_or(part.len()));
        let len = lengths.iter().try_fold(0usize, |total, length| total.checked_add(*length))
            .ok_or_else(|| RuntimeError::new("Core VM string exceeds INT size"))?;
        let count = if len == 0 {
            0
        } else {
            i32::try_from(len + 1)
                .map_err(|_| RuntimeError::new("Core VM string exceeds INT size"))?
        };
        let slots = crate::array_storage::Hp2ArraySlots {
            data: unsafe { std::ptr::addr_of_mut!((*destination).data) },
            num: unsafe { std::ptr::addr_of_mut!((*destination).num) },
            max: unsafe { std::ptr::addr_of_mut!((*destination).max) },
        };
        let allocator = crate::array_storage::system_allocator();
        unsafe {
            crate::array_storage::set_exact(
                slots,
                count,
                std::mem::size_of::<crate::ansi::Tchar>() as i32,
                b"FString\0".as_ptr().cast(),
                allocator,
            )
        }
        .map_err(|_| RuntimeError::new("Unable to allocate Core VM FString result"))?;
        if count != 0 {
            let data = unsafe { slots.data.read_unaligned().cast::<crate::ansi::Tchar>() };
            unsafe {
                let mut offset = 0;
                for (part, length) in parts.iter().zip(lengths) {
                    std::ptr::copy_nonoverlapping(part.as_ptr(), data.add(offset), length);
                    offset += length;
                }
                data.add(len).write(0);
            }
        }
        Ok(())
    }

    /// Appends one C-string payload to Core-owned FString storage, retaining its capacity.
    ///
    /// # Safety
    /// Destination is initialized Core FString storage; text is readable and does
    /// not overlap its allocation. The existing FString has its trailing NUL.
    pub unsafe fn append_vm_fstring_result(
        self, destination: *mut TArrayRaw, text: &[crate::ansi::Tchar],
    ) -> RuntimeResult<()> {
        let length = text.iter().position(|unit| *unit == 0).unwrap_or(text.len());
        if length == 0 { return Ok(()); }
        let old = unsafe { destination.read_unaligned() };
        let previous = if old.num > 0 { old.num as usize - 1 } else { 0 };
        let total = previous.checked_add(length).and_then(|value| value.checked_add(1))
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| RuntimeError::new("Core VM string exceeds INT size"))?;
        let slots = crate::array_storage::Hp2ArraySlots {
            data: unsafe { std::ptr::addr_of_mut!((*destination).data) },
            num: unsafe { std::ptr::addr_of_mut!((*destination).num) },
            max: unsafe { std::ptr::addr_of_mut!((*destination).max) },
        };
        unsafe {
            crate::array_storage::add(slots, total - old.num, std::mem::size_of::<crate::ansi::Tchar>() as i32,
                b"FString\0".as_ptr().cast(), crate::array_storage::system_allocator())
        }.map_err(|_| RuntimeError::new("Unable to grow Core VM FString result"))?;
        let data = unsafe { slots.data.read_unaligned().cast::<crate::ansi::Tchar>() };
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), data.add(previous), length);
            data.add(previous + length).write(0);
        }
        Ok(())
    }

    /// Releases an FString argument stepped by the canonical Core VM.
    ///
    /// # Errors
    /// Returns an error if Core's VM array storage cannot release the allocation.
    ///
    /// # Safety
    /// `value` must be an initialized FString stepped by the canonical Core VM.
    pub unsafe fn release_vm_fstring_argument(self, value: *mut TArrayRaw) -> RuntimeResult<()> {
        let slots = crate::array_storage::Hp2ArraySlots {
            data: unsafe { std::ptr::addr_of_mut!((*value).data) },
            num: unsafe { std::ptr::addr_of_mut!((*value).num) },
            max: unsafe { std::ptr::addr_of_mut!((*value).max) },
        };
        unsafe {
            crate::array_storage::release(slots, crate::array_storage::system_allocator())
        }
        .map_err(|_| RuntimeError::new("Unable to release Core VM FString argument"))
    }
    /// Serializes an owned FString through canonical Core storage.
    ///
    /// # Safety
    /// `value` is an initialized FString allocated by Core's system allocator.
    pub unsafe fn serialize_owned_fstring(
        self,
        archive: &crate::object_archive::ObjectArchive<'_>,
        value: *mut TArrayRaw,
    ) -> RuntimeResult<()> {
        unsafe { crate::core_array::serialize_owned_fstring(self, archive, value) }
    }

    /// Releases an owned FString using the allocator paired with its storage.
    ///
    /// # Safety
    /// `value` is an initialized FString allocated by Core's system allocator.
    pub unsafe fn destroy_owned_fstring(self, value: *mut TArrayRaw) -> RuntimeResult<()> {
        unsafe { crate::core_array::destroy_owned_fstring(value) }
    }

    /// Serializes a Core-owned `TArray<FString>`.
    ///
    /// # Safety
    /// `value` is an initialized array of Core-owned FStrings.
    pub unsafe fn serialize_owned_fstring_array(
        self,
        archive: &crate::object_archive::ObjectArchive<'_>,
        value: *mut TArrayRaw,
    ) -> RuntimeResult<()> {
        unsafe { crate::core_array::serialize_owned_fstring_array(self, archive, value) }
    }

    /// Destroys a Core-owned `TArray<FString>` and each live string element.
    ///
    /// # Safety
    /// `value` is an initialized array of Core-owned FStrings.
    pub unsafe fn destroy_owned_fstring_array(self, value: *mut TArrayRaw) -> RuntimeResult<()> {
        unsafe { crate::core_array::destroy_owned_fstring_array(value) }
    }

    pub fn emit_log(self, category: i32, message: &[u32]) -> RuntimeResult<()> {
        if category == self.state.warning_name {
            self.state.warnings.set(self.state.warnings.get().wrapping_add(1));
        }
        if category == self.state.error_name {
            self.state.errors.set(self.state.errors.get().wrapping_add(1));
        }
        let output = self.state.output;
        output(self, category, message)
    }

    pub(crate) fn emit_warning(self, message: &[u32]) -> RuntimeResult<()> {
        self.emit_log(self.state.warning_name, message)
    }

    pub fn warning_count(self) -> i32 {
        self.state.warnings.get()
    }

    pub fn error_count(self) -> i32 {
        self.state.errors.get()
    }

    pub fn object_count(self) -> i32 {
        // SAFETY: the capability is engine-thread confined; no hook is invoked.
        unsafe { (*self.state.objects.get()).len() }
    }

    pub fn object_at(self, index: i32) -> *mut UObject {
        // SAFETY: at bounds-checks the index and copies the stable pointer.
        unsafe { (*self.state.objects.get()).at(index).cast() }
    }

    pub fn intern(self, text: &[u32], mode: FindName) -> RuntimeResult<i32> {
        // SAFETY: name operations are direct and cannot invoke external hooks.
        unsafe { (&mut *self.state.names.get()).intern(text, mode).map(|name| name.index) }
    }

    pub fn find_name(self, text: &[u32]) -> RuntimeResult<i32> {
        // SAFETY: this only reads engine-thread-owned names without callbacks.
        unsafe { (&*self.state.names.get()).find(text).map(|name| name.index) }
    }

    pub fn name_text(self, index: i32) -> RuntimeResult<Vec<u32>> {
        // A copy is required when the caller retains text over a reentrant hook.
        unsafe { (&*self.state.names.get()).text(index) }
            .ok_or_else(|| RuntimeError::new("Invalid name index"))
    }

    /// Writes a name into caller-owned scratch without allocating a name snapshot.
    pub fn write_name_text(self, index: i32, output: &mut Vec<u32>) -> RuntimeResult<()> {
        output.clear();
        self.append_name_text(index, output)
    }

    pub(crate) fn append_name_text(self, index: i32, output: &mut Vec<u32>) -> RuntimeResult<()> {
        // No reentrant hook runs while this engine-thread-local name is borrowed.
        let names = unsafe { &*self.state.names.get() };
        let text = names.text_slice(index).ok_or_else(|| RuntimeError::new("Invalid name index"))?;
        output.extend_from_slice(text);
        Ok(())
    }

    /// Core-owned `GMem` scratch storage; callers bound query results with a mark.
    pub fn scratch_memory(self) -> &'a crate::mem_owner::MemStackOwner {
        &self.state.memory
    }

    /// Formats the actual canonical class name and outer path for diagnostics.
    ///
    /// # Safety
    /// `object`, its class and every outer are live identities in this owner.
    pub unsafe fn object_full_name(self, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
        crate::core_lifetime::object_full_name(self, object)
    }

    /// Formats the canonical object path (no class prefix) for a live object.
    ///
    /// # Safety
    /// `object` and every outer are live identities in this owner.
    pub unsafe fn object_path_text(self, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
        crate::core_reflection::object_path_text_with(object, &|name| self.name_text(name))
    }

    /// Executes `USystem::Exec` using Core-owned process/configuration state.
    ///
    /// # Safety
    /// `system` must be a live canonical `USystem`; `command` must be a readable,
    /// NUL-terminated TCHAR buffer for the synchronous call.
    pub unsafe fn execute_system_command(
        self,
        system: *mut UObject,
        command: *const u32,
        output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
    ) -> RuntimeResult<bool> {
        unsafe { crate::system_command::execute_system_command(self, system, command, output) }
    }


    /// Executes UObject static console commands against canonical Core metadata and state.
    ///
    /// # Safety
    /// `command` must be readable and NUL-terminated for the synchronous call.
    pub unsafe fn execute_static_command(
        self,
        command: *const u32,
        output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
    ) -> RuntimeResult<bool> {
        unsafe { crate::core_command_owner::execute_static_command(self, command, output) }
    }
    /// Installs the callback for the actual shared cache owner.
    ///
    /// Engine integration installs one leaf that delegates into its existing
    /// `EngineState.cache`; this does not allocate or mirror cache state.
    pub fn set_cache_command_handler(self, handler: CacheCommandHandler) {
        self.state.cache_command.set(Some(handler));
    }

    /// Executes the cache command against the parent-installed shared cache owner.
    ///
    /// # Errors
    /// Returns an error if Engine has not installed its live cache owner or the
    /// cache command/output operation fails.
    ///
    /// # Safety
    /// `command` must be a readable, NUL-terminated TCHAR buffer for this call.
    pub unsafe fn execute_cache_command(
        self,
        command: *const u32,
        output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
    ) -> RuntimeResult<bool> {
        let handler = self.state.cache_command.get().ok_or_else(|| {
            RuntimeError::new("The shared EngineState cache owner is not installed")
        })?;
        // SAFETY: forwarded command lifetime and owner-thread constraints.
        unsafe { handler(self, command, output) }
    }

    pub(crate) fn storage(self) -> *mut ObjectStorage {
        self.state.objects.get()
    }

    pub(crate) fn name_count(self) -> i32 {
        unsafe { (*(&mut *self.state.names.get()).storage_ptr()).len() }
    }

    pub(crate) fn name_hash_stats(self) -> (i32, i32, i32) {
        unsafe { (&*self.state.names.get()).hash_stats() }
    }

    pub(crate) fn name_at(self, index: i32) -> *mut FNameEntry {
        unsafe {
            let storage = (&mut *self.state.names.get()).storage_ptr();
            if index < 0 || index >= (*storage).len() {
                std::ptr::null_mut()
            } else {
                (*storage).entry(index)
            }
        }
    }

    pub(crate) fn remove_name(self, index: i32) -> RuntimeResult<()> {
        unsafe { (&mut *self.state.names.get()).remove(index) }
    }

    pub(crate) fn root_count(self) -> i32 {
        unsafe { (*self.state.roots.get()).len() }
    }

    pub(crate) fn object_creation_closed(self) -> bool {
        self.objects_closed.get()
    }

    pub(crate) fn exit_purge(self) -> bool {
        self.state.exit_purge.get()
    }

    pub(crate) fn root_at(self, index: i32) -> *mut UObject {
        unsafe { (*self.state.roots.get()).at(index).cast() }
    }

    pub(crate) fn loader_count(self) -> i32 {
        unsafe { i32::try_from((*self.state.loader.get()).linkers.len()).unwrap_or(i32::MAX) }
    }

    pub(crate) fn loader_at(self, index: i32) -> *mut UObject {
        if index < 0 { return std::ptr::null_mut(); }
        unsafe { (&(*self.state.loader.get()).linkers).get(index as usize).copied().unwrap_or(std::ptr::null_mut()) }
    }

    pub(crate) fn pending_localized_count(self) -> i32 {
        unsafe { (*self.state.pending_localized.get()).len() }
    }

    pub(crate) fn pending_localized_at(self, index: i32) -> *mut UObject {
        unsafe { (*self.state.pending_localized.get()).at(index).cast() }
    }

    pub(crate) fn take_pending_localized(self, index: i32, expected: *mut UObject) -> bool {
        unsafe { (&mut *self.state.pending_localized.get()).take_if(index, expected.cast()) }
    }

    /// # Safety
    /// `object` remains a live canonical object until it is removed from roots.
    pub unsafe fn add_root(self, object: *mut UObject) {
        unsafe {
            let roots = &mut *self.state.roots.get();
            if !roots.contains(object.cast()) {
                roots.append(object.cast());
            }
        }
    }

    pub fn remove_root(self, object: *mut UObject) {
        unsafe { (&mut *self.state.roots.get()).remove_all(object.cast()) };
    }

    /// # Safety
    /// A non-null replacement is a live canonical object. This conditional
    /// commit must follow a snapshot, not retain a vector element over a hook.
    pub(crate) unsafe fn replace_root(
        self, index: i32, previous: *mut UObject, replacement: *mut UObject,
    ) {
        unsafe {
            (&mut *self.state.roots.get()).replace_if(index, previous.cast(), replacement.cast());
        }
    }

    /// # Safety
    /// The object remains live until localized loading drains this collection.
    pub(crate) unsafe fn queue_localized(self, object: *mut UObject) {
        unsafe {
            let pending = &mut *self.state.pending_localized.get();
            if !pending.contains(object.cast()) {
                pending.append(object.cast());
            }
        }
    }

    /// # Safety
    /// A non-null replacement is live. Re-fetches the current collection after
    /// callbacks rather than retaining a potentially moved entry address.
    pub(crate) unsafe fn replace_pending_localized(
        self, index: i32, previous: *mut UObject, replacement: *mut UObject,
    ) {
        unsafe {
            (&mut *self.state.pending_localized.get())
                .replace_if(index, previous.cast(), replacement.cast());
        }
    }

    pub(crate) fn clear_pending_localized(self) {
        unsafe { (&mut *self.state.pending_localized.get()).release() };
    }

    /// # Safety
    /// The index denotes a live owned slot. Runs only virtual Destroy and
    /// captures the base identity snapshot before any outer can become stale.
    pub(crate) unsafe fn destroy_slot(self, index: i32) -> RuntimeResult<()> {
        let object = self.object_at(index);
        if object.is_null() || !unsafe { (*self.storage()).is_owned(index) } {
            return Err(RuntimeError::new("Invalid owned object index"));
        }
        if !unsafe { (*self.storage()).needs_teardown(index) } {
            return Ok(());
        }
        unsafe { (*self.storage()).begin_teardown(index) };
        let result = unsafe { (self.state.destroy_record)(self, object) };
        // Re-fetch even for same-address replacement: re-publication is Live,
        // whereas the exact allocation being destroyed remains Destroying.
        if self.object_at(index) == object
            && unsafe { (*self.storage()).is_destroying(index) }
        {
            // Base Destroy records this before Outer can become stale.
            let outer_index = unsafe { (*object)._linker_index };
            let name = unsafe { i32::from_ne_bytes((*object).name) };
            let bucket = object_storage::hash_bucket(name, outer_index);
            unsafe { (*self.storage()).finish_teardown(index, bucket) };
        }
        result
    }

    /// Runs final member destruction once, after virtual Destroy and before release.
    ///
    /// # Safety
    /// `index` identifies an owned slot whose virtual Destroy phase completed.
    pub(crate) unsafe fn finalize_slot(self, index: i32) -> RuntimeResult<()> {
        let object = self.object_at(index);
        if object.is_null() || !unsafe { (*self.storage()).is_owned(index) } {
            return Err(RuntimeError::new("Invalid owned object index"));
        }
        if unsafe { (*self.storage()).teardown_bucket(index) }.is_none() {
            return Err(RuntimeError::new("Object fields have not been destroyed"));
        }
        if !unsafe { (*self.storage()).begin_finalization(index) } {
            // A nested finalization is owned by the outer invocation; completed
            // finalization is also idempotent.
            return Ok(());
        }
        let result = match unsafe { self.class_ops(object) } {
            Ok(operations) => unsafe { (operations.finalize_members)(self, object) },
            Err(error) => Err(error),
        };
        if self.object_at(index) == object && unsafe { (*self.storage()).is_finalizing(index) } {
            unsafe { (*self.storage()).finish_finalization(index) };
        }
        result
    }

    /// # Safety
    /// Virtual destruction completed. This pass finalizes native members once,
    /// then removes the object identity and releases its allocation.
    pub(crate) unsafe fn free_slot(self, index: i32) -> RuntimeResult<()> {
        let object = self.object_at(index);
        if object.is_null() || !unsafe { (*self.storage()).is_owned(index) } {
            return Err(RuntimeError::new("Invalid owned object index"));
        }
        if unsafe { (*self.storage()).is_finalizing(index) } {
            return Ok(());
        }
        let bucket = unsafe { (*self.storage()).teardown_bucket(index) }
            .ok_or_else(|| RuntimeError::new("Object fields have not been destroyed"))?;
        let finalized = unsafe { self.finalize_slot(index) };
        let result = unsafe {
            object_storage::unhash_with(self.storage(), object.cast(), bucket, |kind, valid| {
                if valid {
                    Ok(())
                } else {
                    Err(RuntimeError::new(if kind == 1 { "Removed!=0" } else { "Removed==1" }))
                }
            })
        };
        let allocation = unsafe { (*self.storage()).remove_owned(index)? };
        unsafe { &mut *self.state.startup_structs.get() }
            .retain(|entry| entry.structure != object);
        let frame = unsafe { (*object).state_frame };
        unsafe {
            crate::vm_frame::free_state(frame.cast());
            (*object).state_frame = std::ptr::null_mut();
        }
        drop(allocation);
        finalized.and(result)
    }

    /// # Safety
    /// The selected record layout contains the complete class body; class and
    /// outer are null bootstrap handles or live canonical owned objects.
    pub(crate) unsafe fn publish_record(
        self,
        layout: Layout,
        class: *mut UObject,
        outer: *mut UObject,
        name: i32,
        flags: u32,
    ) -> RuntimeResult<*mut UObject> {
        if self.objects_closed.get() {
            return Err(RuntimeError::new("Cannot publish Core objects after pre-exit begins"));
        }
        let allocation = OwnedObject::allocate(layout, class.cast(), outer.cast(), name, flags)?;
        let pointer = allocation.as_ptr();
        // SAFETY: freshly initialized header, canonical storage and live handles.
        unsafe { object_storage::add_owned(self.storage(), allocation, -1)? };
        Ok(pointer)
    }
}
