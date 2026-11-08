//! UObject registration, subsystem lifetime and property-broadcast policy.
//! Engine-thread access is serialized. Native services retain typed storage,
//! placement memory tags, virtual hooks and exception/diagnostic boundaries.
use crate::object_collections::Hp2ObjectCollectionKind as Collection;
use crate::object_lifecycle::RF_NATIVE;
use hp2_abi::engine::generated::UObject;
use std::ffi::c_void;
use std::ptr::{addr_of_mut, null_mut};
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};

/// Performs canonical `UObject::StaticTick` checks using this Core runtime.
///
/// The process-wide duplicate-native slot is shared with the native ABI.
pub fn static_tick(access: CoreAccess<'_>) -> RuntimeResult<()> {
    if access.is_loading() {
        return Err(RuntimeError::new("UObject::StaticTick during BeginLoad"));
    }
    let duplicate = crate::vm_registry::duplicate_index();
    if duplicate != 0 {
        return Err(RuntimeError::new(&format!("Duplicate native registered: {duplicate}")));
    }
    Ok(())
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectHeaderKind {
    Allocated,
    Native,
    Static,
    ClassDefault,
}

/// Initialize only the base fields established by the matching SDK constructor.
///
/// # Safety
/// Storage is writable and aligned for the compiler-probed UObject layout.
/// Handles are stored, not dereferenced. Native vptrs remain untouched. Ordinary
/// constructors preserve padding and the static constructor also preserves Class;
/// class defaults deliberately zero data padding and retain a zero linker index.
/// Never form a reference to or copy the whole mirror before native construction.
pub unsafe fn initialize_header(
    storage: *mut c_void,
    class: *mut c_void,
    outer: *mut c_void,
    name: i32,
    flags: u32,
    kind: Hp2ObjectHeaderKind,
) {
    let storage = storage.cast::<UObject>();
    let (outer, name, flags) = match kind {
        Hp2ObjectHeaderKind::Allocated => (outer, name, flags),
        Hp2ObjectHeaderKind::Native | Hp2ObjectHeaderKind::Static => {
            (null_mut(), 0, flags | RF_NATIVE)
        }
        Hp2ObjectHeaderKind::ClassDefault => {
            // Match the SDK's header memset without rewriting its native vptr,
            // then establish the two nonzero fields without redundant stores.
            let start = std::mem::offset_of!(UObject, index) + size_of::<i32>();
            let end = std::mem::offset_of!(UObject, class);
            unsafe {
                storage.cast::<u8>().add(start).write_bytes(0, end - start);
                addr_of_mut!((*storage).index).write(-1);
                addr_of_mut!((*storage).class).write(class);
            }
            return;
        }
    };
    unsafe {
        addr_of_mut!((*storage).index).write(-1);
        addr_of_mut!((*storage).hash_next).write(null_mut());
        addr_of_mut!((*storage).state_frame).write(null_mut());
        addr_of_mut!((*storage)._linker).write(null_mut());
        addr_of_mut!((*storage)._linker_index).write(-1);
        addr_of_mut!((*storage).outer).write(outer);
        addr_of_mut!((*storage).object_flags).write(flags);
        addr_of_mut!((*storage).name).write(name.to_ne_bytes());
        if !matches!(kind, Hp2ObjectHeaderKind::Static) {
            addr_of_mut!((*storage).class).write(class);
        }
    }
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectBootstrapOperation {
    Initialize,
    ProcessRegistrants,
    Exit,
    Tick,
    ConditionalErrorShutdown,
    StaticErrorShutdown,
    BindPackage,
    Register,
    NativeConstructor,
    StaticConstructor,
    ValidateCopy,
    ValidateAssign,
    GlobalSetProperty,
}
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectBootstrapLeaf {
    SetNoRegister,
    CheckAbi,
    EnterMemoryTag,
    ParseConflicts,
    ParseNoGc,
    SetCheckConflicts,
    SetNoGc,
    InitializeStorage,
    InitializeRegistrants,
    SetInitialized,
    Initialized,
    CallProcessRegistrants,
    RegisterCount,
    SetRegisterCount,
    AutoRegister,
    AutoNext,
    SetAutoRegister,
    SetHashNext,
    AppendRegistrant,
    DrainRegistrants,
    AssertNoAutoRegister,
    CreateTransient,
    RootTransient,
    LogInitialized,
    AssertCollectionEmpty,
    ProfileEnabled,
    ParseProfile,
    ObjectLen,
    ObjectAt,
    IsFunction,
    FunctionCalls,
    FunctionCycles,
    SortProfile,
    Ticks,
    SecondsPerCycleBits,
    LogProfileTitle,
    LogProfileColumns,
    LogProfileSeparator,
    UnrootTransient,
    SetObjectFlags,
    NameLen,
    NameAt,
    TagNameUnreachable,
    SetExitPurge,
    PurgeGarbage,
    ReleaseStorage,
    ReleaseCollection,
    LogClosed,
    CheckBeginLoad,
    NativeDuplicate,
    DuplicateError,
    ObjectFlags,
    VirtualShutdownAfterError,
    DiscardCapturedException,
    LogObjectDoubleFault,
    LogStaticErrorShutdown,
    ConditionalShutdownAfterError,
    LogManagerDoubleFault,
    PackageDllHandle,
    ObjectOuter,
    PackageAttemptedBind,
    SetPackageAttemptedBind,
    FormatDllPath,
    LoadDll,
    SetPackageDllHandle,
    StaticLinked,
    LogBoundPackage,
    CheckRegistrationAllowed,
    CheckRegistrationSlotWidths,
    StoreRegistrationStrings,
    ObjectClass,
    ObjectStaticClass,
    CallRegister,
    CheckInitialized,
    RegistrationOuter,
    RegistrationName,
    CreatePackage,
    SetObjectOuter,
    SetObjectName,
    ClearLinker,
    NameIsNone,
    FindRegisteredDuplicate,
    UnpackagedError,
    InvalidNameError,
    ExistingNameError,
    AddObject,
    CopyClassError,
    AssignClassError,
    ImportObjectProperty,
    PostEditChange,
    ImportDefaultProperty,
    SaveDefaultConfig,
}
#[repr(C)]
pub struct Hp2ObjectBootstrapArgs {
    pub object: *mut c_void,
    pub other: *mut c_void,
    pub class_object: *mut c_void,
    pub property: *mut c_void,
    pub text: *const c_void,
    pub package: *const c_void,
    pub offset: i32,
    pub mode: i32,
}
#[repr(C)]
pub struct Hp2ObjectBootstrapServices {
    pub call:
        unsafe extern "C" fn(*mut c_void, Hp2ObjectBootstrapLeaf, usize, usize, *mut usize) -> i32,
    pub profile_row: unsafe extern "C" fn(*mut c_void, *const Hp2ObjectBootstrapProfileRow) -> i32,
}
#[repr(C)]
pub struct Hp2ObjectBootstrapProfileRow {
    pub function: *mut c_void,
    pub microseconds_per_tick: f64,
    pub cycles_per_call: f64,
    pub calls_per_tick: f64,
    pub calls: f64,
    pub percent: f64,
}
struct Flow<'a> {
    context: *mut c_void,
    services: &'a Hp2ObjectBootstrapServices,
}
impl Flow<'_> {
    unsafe fn call(&self, leaf: Hp2ObjectBootstrapLeaf, a: usize, b: usize) -> Result<usize, i32> {
        let mut out = 0;
        let status = unsafe { (self.services.call)(self.context, leaf, a, b, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
}
use Hp2ObjectBootstrapLeaf as L;

/// Descending signed cycles, without subtracting and overflowing.
pub fn compare_profile_cycles(a: i64, b: i64) -> i32 {
    if b > a {
        1
    } else if b == a {
        0
    } else {
        -1
    }
}

// Matches the native function-local one-way latch, including exit/reinit.
static mut ERROR_SHUTDOWN_STARTED: bool = false;
const RF_UNREACHABLE: usize = 0x0000_0002;
const RF_TAG_GARBAGE: usize = 0x0000_0040;
const RF_ERROR_SHUTDOWN: usize = 0x1000_0000;

unsafe fn profile(flow: &Flow<'_>) -> Result<(), i32> {
    unsafe {
        if flow.call(L::ProfileEnabled, 0, 0)? == 0 || flow.call(L::ParseProfile, 0, 0)? == 0 {
            return Ok(());
        }
        let mut functions = Vec::<*mut c_void>::new();
        let mut index = 0;
        while index < flow.call(L::ObjectLen, 0, 0)? {
            let object = flow.call(L::ObjectAt, index, 0)?;
            if object != 0
                && flow.call(L::IsFunction, object, 0)? != 0
                && flow.call(L::FunctionCalls, object, 0)? != 0
            {
                functions.push(object as *mut c_void);
            }
            index += 1;
        }
        flow.call(L::LogProfileTitle, functions.len(), 0)?;
        flow.call(L::LogProfileColumns, 0, 0)?;
        flow.call(L::LogProfileSeparator, 0, 0)?;
        flow.call(
            L::SortProfile,
            functions.as_mut_ptr() as usize,
            functions.len(),
        )?;
        let mut total = 0.0f32;
        for &object in &functions {
            total += flow.call(L::FunctionCycles, object as usize, 0)? as i64 as f32;
        }
        for &object in &functions {
            let cycles = flow.call(L::FunctionCycles, object as usize, 0)? as i64 as f32;
            let calls = flow.call(L::FunctionCalls, object as usize, 0)? as i64 as f32;
            let ticks = flow.call(L::Ticks, 0, 0)? as i64 as f32;
            let seconds = f64::from_bits(flow.call(L::SecondsPerCycleBits, 0, 0)? as u64);
            let row = Hp2ObjectBootstrapProfileRow {
                function: object,
                microseconds_per_tick: 1_000_000.0 * seconds * f64::from(cycles) / f64::from(ticks),
                cycles_per_call: f64::from(cycles / calls),
                calls_per_tick: f64::from(calls / ticks),
                calls: f64::from(calls),
                percent: f64::from(cycles / total * 100.0),
            };
            let status = (flow.services.profile_row)(flow.context, &row);
            if status != 0 {
                return Err(status);
            }
        }
        Ok(())
    }
}

/// # Safety
/// Native context/args/services remain live; callbacks capture exceptions and
/// never unwind through Rust. All table entries are reloaded after callbacks.
pub unsafe fn run(
    operation: Hp2ObjectBootstrapOperation,
    args: &Hp2ObjectBootstrapArgs,
    context: *mut c_void,
    services: &Hp2ObjectBootstrapServices,
    names: unsafe fn() -> Result<crate::name::PublishedNames, i32>,
) -> Result<(), i32> {
    use Hp2ObjectBootstrapOperation as O;
    let f = Flow { context, services };
    unsafe {
        match operation {
            O::Initialize => {
                f.call(L::SetNoRegister, 1, 0)?;
                f.call(L::CheckAbi, 0, 0)?;
                f.call(L::EnterMemoryTag, 0, 0)?;
                let conflicts = f.call(L::ParseConflicts, 0, 0)?;
                f.call(L::SetCheckConflicts, conflicts, 0)?;
                let no_gc = f.call(L::ParseNoGc, 0, 0)?;
                f.call(L::SetNoGc, no_gc, 0)?;
                f.call(L::InitializeStorage, 0, 0)?;
                f.call(L::InitializeRegistrants, 0, 0)?;
                f.call(L::SetInitialized, 1, 0)?;
                f.call(L::CallProcessRegistrants, 0, 0)?;
                f.call(L::CreateTransient, 0, 0)?;
                f.call(L::RootTransient, 0, 0)?;
                crate::package_flow::initialize_remaps(names()?)?;
                f.call(L::LogInitialized, 0, 0)?;
            }
            O::ProcessRegistrants => {
                if f.call(L::RegisterCount, 0, 0)? as isize >= 0 {
                    f.call(L::SetRegisterCount, (-1isize) as usize, 0)?;
                    f.call(L::EnterMemoryTag, 1, 0)?;
                    loop {
                        let object = f.call(L::AutoRegister, 0, 0)?;
                        if object == 0 {
                            break;
                        }
                        f.call(L::AppendRegistrant, object, 0)?;
                        // Read the live head after append, as in the native for increment.
                        let head = f.call(L::AutoRegister, 0, 0)?;
                        let next = f.call(L::AutoNext, head, 0)?;
                        f.call(L::SetAutoRegister, next, 0)?;
                    }
                    f.call(L::DrainRegistrants, 0, 0)?;
                    f.call(L::AssertNoAutoRegister, 0, 0)?;
                    f.call(L::SetRegisterCount, 1, 0)?;
                }
            }
            O::Exit => {
                // Roots still own the transient package until purge.
                for kind in [
                    Collection::Hp2ObjectLoaded,
                    Collection::Hp2ObjectRegistrants,
                ] {
                    f.call(L::AssertCollectionEmpty, kind as usize, 0)?;
                }
                f.call(L::AssertNoAutoRegister, 0, 0)?;
                profile(&f)?;
                f.call(L::UnrootTransient, 0, 0)?;
                let mut i = 0;
                while i < f.call(L::ObjectLen, 0, 0)? {
                    let object = f.call(L::ObjectAt, i, 0)?;
                    if object != 0 {
                        f.call(L::SetObjectFlags, object, RF_UNREACHABLE | RF_TAG_GARBAGE)?;
                    }
                    i += 1;
                }
                let mut i = 0;
                while i < f.call(L::NameLen, 0, 0)? {
                    let name = f.call(L::NameAt, i, 0)?;
                    if name != 0 {
                        f.call(L::TagNameUnreachable, name, 0)?;
                    }
                    i += 1;
                }
                f.call(L::SetExitPurge, 1, 0)?;
                f.call(L::PurgeGarbage, 0, 0)?;
                f.call(L::ReleaseStorage, 0, 0)?;
                for kind in [
                    Collection::Hp2ObjectLoaded,
                    Collection::Hp2ObjectLoaders,
                    Collection::Hp2ObjectRoots,
                    Collection::Hp2ObjectRegistrants,
                ] {
                    f.call(L::ReleaseCollection, kind as usize, 0)?;
                }
                crate::object_registry::release()?;
                crate::package_flow::release_remaps()?;
                f.call(L::SetInitialized, 0, 0)?;
                f.call(L::LogClosed, 0, 0)?;
            }
            O::Tick => {
                f.call(L::CheckBeginLoad, 0, 0)?;
                let duplicate = f.call(L::NativeDuplicate, 0, 0)?;
                if duplicate != 0 {
                    f.call(L::DuplicateError, duplicate, 0)?;
                }
            }
            O::ConditionalErrorShutdown => {
                let object = args.object as usize;
                if f.call(L::ObjectFlags, object, 0)? & RF_ERROR_SHUTDOWN == 0 {
                    f.call(L::SetObjectFlags, object, RF_ERROR_SHUTDOWN)?;
                    if f.call(L::VirtualShutdownAfterError, object, 0).is_err() {
                        f.call(L::DiscardCapturedException, 0, 0)?;
                        f.call(L::LogObjectDoubleFault, 0, 0)?;
                    }
                }
            }
            O::StaticErrorShutdown => {
                if f.call(L::Initialized, 0, 0)? != 0 && !ERROR_SHUTDOWN_STARTED {
                    ERROR_SHUTDOWN_STARTED = true;
                    f.call(L::LogStaticErrorShutdown, 0, 0)?;
                    let result = (|| -> Result<(), i32> {
                        let mut i = 0;
                        while i < f.call(L::ObjectLen, 0, 0)? {
                            let object = f.call(L::ObjectAt, i, 0)?;
                            if object != 0 {
                                f.call(L::ConditionalShutdownAfterError, object, 0)?;
                            }
                            i += 1;
                        }
                        Ok(())
                    })();
                    if result.is_err() {
                        f.call(L::DiscardCapturedException, 0, 0)?;
                        f.call(L::LogManagerDoubleFault, 0, 0)?;
                    }
                }
            }
            O::BindPackage => {
                let package = args.object as usize;
                if f.call(L::PackageDllHandle, package, 0)? == 0
                    && f.call(L::ObjectOuter, package, 0)? == 0
                    && f.call(L::PackageAttemptedBind, package, 0)? == 0
                {
                    f.call(L::FormatDllPath, package, 0)?;
                    f.call(L::SetPackageAttemptedBind, package, 1)?;
                    f.call(L::SetNoRegister, 0, 0)?;
                    let handle = f.call(L::LoadDll, 0, 0)?;
                    f.call(L::SetPackageDllHandle, package, handle)?;
                    f.call(L::SetNoRegister, 1, 0)?;
                    if handle != 0 {
                        if f.call(L::StaticLinked, 0, 0)? == 0 {
                            f.call(L::LogBoundPackage, package, 0)?;
                        }
                        f.call(L::CallProcessRegistrants, 0, 0)?;
                    }
                }
            }
            O::Register => {
                let object = args.object as usize;
                f.call(L::CheckInitialized, 0, 0)?;
                let outer_name = f.call(L::RegistrationOuter, object, 0)?;
                let name = f.call(L::RegistrationName, object, 0)?;
                let outer = f.call(L::CreatePackage, outer_name, 0)?;
                f.call(L::SetObjectOuter, object, outer)?;
                f.call(L::SetObjectName, object, name)?;
                f.call(L::ClearLinker, object, 0)?;
                if f.call(L::ObjectOuter, object, 0)? == 0 {
                    f.call(L::UnpackagedError, object, 0)?;
                }
                if f.call(L::NameIsNone, object, 0)? != 0 {
                    f.call(L::InvalidNameError, object, 0)?;
                }
                if f.call(L::FindRegisteredDuplicate, object, 0)? != 0 {
                    f.call(L::ExistingNameError, object, 0)?;
                }
                f.call(L::AddObject, object, 0)?;
            }
            O::NativeConstructor | O::StaticConstructor => {
                if matches!(operation, O::NativeConstructor) {
                    f.call(L::CheckRegistrationAllowed, 0, 0)?;
                }
                f.call(L::CheckRegistrationSlotWidths, 0, 0)?;
                f.call(L::StoreRegistrationStrings, args.object as usize, 0)?;
                let initialized = f.call(L::Initialized, 0, 0)? != 0;
                if matches!(operation, O::NativeConstructor) || !initialized {
                    let head = f.call(L::AutoRegister, 0, 0)?;
                    f.call(L::SetHashNext, args.object as usize, head)?;
                    f.call(L::SetAutoRegister, args.object as usize, 0)?;
                }
                if matches!(operation, O::NativeConstructor)
                    && initialized
                    && f.call(L::ObjectClass, args.object as usize, 0)?
                        == f.call(L::ObjectStaticClass, 0, 0)?
                {
                    f.call(L::CallRegister, args.object as usize, 0)?;
                }
            }
            O::ValidateCopy | O::ValidateAssign => {
                if f.call(L::ObjectClass, args.other as usize, 0)?
                    != f.call(L::ObjectClass, args.object as usize, 0)?
                {
                    f.call(
                        if matches!(operation, O::ValidateCopy) {
                            L::CopyClassError
                        } else {
                            L::AssignClassError
                        },
                        0,
                        0,
                    )?;
                }
            }
            O::GlobalSetProperty => {
                if args.mode != 0 {
                    let mut i = 0;
                    while i < f.call(L::ObjectLen, 0, 0)? {
                        let object = f.call(L::ObjectAt, i, 0)?;
                        if object != 0
                            && crate::object_model::is_a(
                                object as *const c_void,
                                args.class_object as *const c_void,
                            )
                        {
                            f.call(L::ImportObjectProperty, object, 0)?;
                            let live_object = f.call(L::ObjectAt, i, 0)?;
                            f.call(L::PostEditChange, live_object, 0)?;
                        }
                        i += 1;
                    }
                }
                f.call(L::ImportDefaultProperty, 0, 0)?;
                f.call(L::SaveDefaultConfig, 0, 0)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::compare_profile_cycles;
    #[test]
    fn profile_order_handles_signed_cycle_extremes_without_subtraction_overflow() {
        assert_eq!(compare_profile_cycles(i64::MIN, i64::MAX), 1);
        assert_eq!(compare_profile_cycles(i64::MAX, i64::MIN), -1);
        assert_eq!(compare_profile_cycles(i64::MIN, i64::MIN), 0);
        assert_eq!(compare_profile_cycles(1i64 << 48, 1), -1);
    }
}
