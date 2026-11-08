//! Package transaction policy. All native callbacks are individually guarded;
//! an error is inspected only at the legacy catch boundary. No Rust borrow of
//! engine storage survives a callback (serialization can reenter any phase).
use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use crate::name::{self, PublishedNames};
use hp2_abi::engine::generated::{FName, FObjectExport, TArrayRaw};
#[path = "package_save.rs"]
pub(crate) mod package_save;
use self::package_save as shared_package_save;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

// Own native FString buffers transferred by typed path-formatting leaves.
// Equality and hashing deliberately retain FString/TMap's legacy case folding.
struct ConformPath {
    raw: TArrayRaw,
    allocator: Hp2ArrayServices,
}

impl ConformPath {
    fn new(allocator: Hp2ArrayServices) -> Self {
        Self { raw: TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }, allocator }
    }

    fn text(&self) -> &[u32] {
        if self.raw.num <= 1 { return &[]; }
        // Native FString transfer owns num initialized TCHARs, including NUL.
        unsafe { std::slice::from_raw_parts(self.raw.data.cast(), self.raw.num as usize - 1) }
    }
}

impl PartialEq for ConformPath {
    fn eq(&self, other: &Self) -> bool {
        if self.text().is_empty() || other.text().is_empty() {
            return self.text().is_empty() && other.text().is_empty();
        }
        unsafe { crate::ansi::stricmp(self.raw.data.cast(), other.raw.data.cast()) == 0 }
    }
}
impl Eq for ConformPath {}
impl Hash for ConformPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(name::strihash_text(self.text()));
    }
}
impl Drop for ConformPath {
    fn drop(&mut self) {
        // Preserve a pending transaction error while freeing native storage.
        let slots = Hp2ArraySlots {
            data: &mut self.raw.data,
            num: &mut self.raw.num,
            max: &mut self.raw.max,
        };
        let _ = unsafe { array_storage::release(slots, self.allocator) };
    }
}

// Preserve native padding as potentially uninitialized bytes. Inspect only
// initialized fields; the native append leaf performs the typed record copy.
struct ConformExport(std::mem::MaybeUninit<FObjectExport>);
impl ConformExport {
    fn object(&self) -> usize {
        unsafe { ptr::addr_of!((*self.0.as_ptr())._object).read_unaligned() as usize }
    }
}

type Result<T, E = i32> = std::result::Result<T, E>;

pub(crate) trait NameResolutionOps {
    type Error;
    fn config_prefix(&mut self) -> Result<Option<bool>, Self::Error>;
    fn name_length(&mut self) -> Result<usize, Self::Error>;
    fn has_separator(&mut self) -> Result<bool, Self::Error>;
    fn resolve_config(&mut self, system: bool) -> Result<bool, Self::Error>;
    fn partial_fits(&mut self) -> Result<bool, Self::Error>;
    fn resolve_outer(&mut self) -> Result<Option<usize>, Self::Error>;
    fn publish_outer(&mut self, outer: usize);
    fn advance_name(&mut self) -> Result<(), Self::Error>;
}

/// Resolves only the configured-name prefix, leaving package loading to its caller.
pub(crate) fn resolve_config_name_with<O: NameResolutionOps>(
    ops: &mut O,
) -> Result<bool, O::Error> {
    if let Some(system) = ops.config_prefix()?
        && ops.name_length()? < 1024
        && ops.has_separator()?
        && !ops.resolve_config(system)?
    {
        return Ok(false);
    }
    Ok(true)
}

pub(crate) fn resolve_name_with<O: NameResolutionOps>(
    ops: &mut O,
) -> Result<bool, O::Error> {
    if !resolve_config_name_with(ops)? { return Ok(false); }
    while ops.has_separator()? {
        if !ops.partial_fits()? {
            return Ok(false);
        }
        let Some(outer) = ops.resolve_outer()? else { return Ok(false); };
        ops.publish_outer(outer);
        // Native package creation may reuse the original static name buffer.
        ops.advance_name()?;
    }
    Ok(true)
}

pub(crate) trait ObjectLookupOps {
    type Error;
    fn hash_head(&self, name: usize, outer: usize) -> Result<usize, Self::Error>;
    fn name(&self, object: usize) -> Result<usize, Self::Error>;
    fn outer(&self, object: usize) -> Result<usize, Self::Error>;
    fn matches_class(&self, object: usize) -> Result<bool, Self::Error>;
    fn next(&self, object: usize) -> Result<usize, Self::Error>;
    fn count(&self) -> Result<usize, Self::Error>;
    fn at(&self, index: usize) -> Result<usize, Self::Error>;
}

pub(crate) fn lookup_object_with<O: ObjectLookupOps>(
    ops: &O,
    name: usize,
    outer: usize,
    any_package: bool,
) -> Result<usize, O::Error> {
    if name == 0 { return Ok(0); }
    let mut object = ops.hash_head(name, outer)?;
    while object != 0 {
        if ops.name(object)? == name && ops.outer(object)? == outer && ops.matches_class(object)? {
            return Ok(object);
        }
        object = ops.next(object)?;
    }
    if any_package {
        let mut index = 0;
        while index < ops.count()? {
            let object = ops.at(index)?;
            if object != 0 && ops.name(object)? == name && ops.matches_class(object)? {
                return Ok(object);
            }
            index += 1;
        }
    }
    Ok(0)
}

// Both the mapping and each lookup snapshot own their allocation in Rust. The
// engine allocator remains fallible and records native exceptions in its leaves.
struct RemapBuffer<T: Copy> {
    data: *mut T,
    len: i32,
    capacity: i32,
    arrays: Hp2ArrayServices,
}
impl<T: Copy> RemapBuffer<T> {
    const fn new(arrays: Hp2ArrayServices) -> Self {
        Self { data: ptr::null_mut(), len: 0, capacity: 0, arrays }
    }
    unsafe fn push(this: *mut Self, value: T) -> Result<()> {
        // SAFETY: the owner is live and serialized; no reference to it spans the
        // allocator callback, which can synchronously reenter package loading.
        unsafe {
            let mut pending_len = (*this).len;
            let old_capacity = (*this).capacity;
            let index = array_storage::add(
                Hp2ArraySlots {
                    data: (&raw mut (*this).data).cast(),
                    num: &raw mut pending_len,
                    max: &raw mut (*this).capacity,
                },
                1,
                size_of::<T>() as i32,
                crate::ansi::wide(b"\0").as_ptr().cast(),
                (*this).arrays,
            );
            match index {
                Ok(index) => {
                    (*this).data.add(index as usize).write(value);
                    // Publish only initialized pairs. In particular, an initial
                    // allocation may reenter lookup while Data is still null.
                    (*this).len = pending_len;
                    Ok(())
                }
                Err(error) => {
                    // Allocation failure must not publish pending capacity.
                    (*this).capacity = old_capacity;
                    Err(error)
                }
            }
        }
    }
    unsafe fn at(&self, index: i32) -> T {
        // SAFETY: callers only read the initialized prefix, with no intervening callback.
        unsafe { self.data.add(index as usize).read() }
    }
    unsafe fn close(&mut self) -> Result<()> {
        let data = std::mem::replace(&mut self.data, ptr::null_mut());
        self.len = 0;
        self.capacity = 0;
        if data.is_null() {
            return Ok(());
        }
        // SAFETY: the allocation belongs to the copied allocator. Unpublish it
        // first so Drop cannot retry a free that throws after releasing memory.
        let status = unsafe { (self.arrays.free)(self.arrays.context, data.cast()) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn finish<R>(&mut self, result: Result<R>) -> Result<R> {
        // SAFETY: this uniquely owned buffer is no longer used by the operation.
        let cleanup = unsafe { self.close() };
        result.and_then(|value| cleanup.map(|()| value))
    }
}
impl<T: Copy> Drop for RemapBuffer<T> {
    fn drop(&mut self) {
        // SAFETY: owners never escape engine-thread execution. Native allocator
        // leaves keep the first pending exception if cleanup also throws.
        let _ = unsafe { self.close() };
    }
}
#[derive(Clone, Copy)]
struct NameRemap {
    from: i32,
    to: i32,
}
static mut PACKAGE_REMAPS: Option<RemapBuffer<NameRemap>> = None;

unsafe fn remap_snapshot(
    remaps: *const RemapBuffer<NameRemap>,
    from: i32,
) -> Result<RemapBuffer<i32>> {
    // SAFETY: lookup is serialized; each pair is copied before an allocation.
    // Equal-key TMultiMap chains are newest-first, including after rehash,
    // because rehash visits insertion-order pairs and prepends each one.
    unsafe {
        let mut snapshot = RemapBuffer::new((*remaps).arrays);
        let mut index = (*remaps).len;
        while index > 0 {
            index -= 1;
            let pair = (*remaps).at(index);
            if pair.from == from {
                RemapBuffer::push(&raw mut snapshot, pair.to)?;
            }
        }
        Ok(snapshot)
    }
}

/// Establish the renamed-package mapping using canonical, case-insensitive names.
///
/// # Safety
/// Names and the published array allocator are live and engine-thread serialized.
/// Name callbacks capture exceptions; no reference to remap state spans one.
pub(crate) unsafe fn initialize_remaps(names: PublishedNames) -> Result<()> {
    // SAFETY: initialization/exit and package lookups run on the engine thread.
    unsafe {
        release_remaps()?;
        let arrays = array_storage::published_allocator()?;
        (&raw mut PACKAGE_REMAPS).write(Some(RemapBuffer::new(arrays)));
        let mut from = FName { index: 0 };
        let mut to = FName { index: 0 };
        name::ctor(
            &names.services,
            names.storage,
            &raw mut from,
            crate::ansi::wide(b"UnrealI\0").as_ptr(),
            name::FNAME_ADD,
        ).map_err(|_| 1)?;
        name::ctor(
            &names.services,
            names.storage,
            &raw mut to,
            crate::ansi::wide(b"UnrealShare\0").as_ptr(),
            name::FNAME_ADD,
        ).map_err(|_| 1)?;
        let remaps = (*(&raw mut PACKAGE_REMAPS)).as_mut().ok_or(1)? as *mut _;
        RemapBuffer::push(remaps, NameRemap { from: from.index, to: to.index })
    }
}

/// Release the mapping after object/name purge; a later initialization starts empty.
///
/// # Safety
/// Serialized with initialization and lookups. Active lookup snapshots are
/// independent allocations and remain valid across this release.
pub(crate) unsafe fn release_remaps() -> Result<()> {
    // SAFETY: taking the owner unpublishes it before the allocator can reenter.
    let previous = unsafe { (&raw mut PACKAGE_REMAPS).replace(None) };
    match previous {
        Some(mut remaps) => unsafe { remaps.close() },
        None => Ok(()),
    }
}
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2PackageOperation {
    Lookup,
    LookupChecked,
    SafeError,
    GetLinker,
    Create,
    Resolve,
    LoadObject,
    LoadClass,
    LoadPackage,
    Begin,
    End,
    Preload,
    Postload,
    Dissociate,
    Reset,
    CompareLoaded,
    SortLoaded,
    TagExport,
    TagImport,
    TagName,
    Save,
    Untag,
    TagExports,
    TagImports,
    ExportNames,
    Summary,
    BuildNames,
    SortNames,
    SaveNames,
    BuildImports,
    SortImports,
    BuildExports,
    SortExports,
    Mappings,
    SaveExports,
    SaveImports,
    SaveExportMap,
    RewriteSummary,
    CompareName,
    CompareImport,
    CompareExport,
    ConformExports,
    ConformNames,
}
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2PackageLeaf {
    Check,
    CheckSlow,
    BeginCount,
    SetBeginCount,
    AutoRegister,
    ImportCount,
    SetImportCount,
    LoadedCount,
    LoadedAt,
    LoadedRelease,
    LoadedSort,
    LoaderCount,
    LoaderAt,
    LoaderCheckedAt,
    Root,
    Success,
    Outer,
    Class,
    Flags,
    SetFlags,
    ClearFlags,
    IsIn,
    PendingKill,
    Linker,
    LinkerIndex,
    SerialOffset,
    ExportCount,
    ExportObject,
    ImportCountOf,
    ImportObject,
    SetImportObject,
    ClassFlags,
    Preload,
    Postload,
    FlushLocalized,
    DetachExport,
    DetachLazy,
    DeleteLoad,
    Scope,
    FatalCaught,
    CatchConst,
    ObjectNameText,
    Buffer,
    CopyText,
    TextLength,
    FindDot,
    FindPath,
    FindSlash,
    FindColon,
    AdvanceText,
    TerminateText,
    IniPrefix,
    UserPrefix,
    ConfigGet,
    ConfigMissing,
    FindPackage,
    FindAny,
    NewPackage,
    ResolveCall,
    CreateCall,
    FindPackageFile,
    PackageDll,
    PackageResolveFailed,
    PackageNotFound,
    FileNotFound,
    FilenameToPackage,
    NewFileLog,
    ResetCall,
    NewLoader,
    GuidEqual,
    PackageVersion,
    SafeCaught,
    SandboxSupports,
    SandboxLog,
    BeginCall,
    EndCall,
    GetLinkerCall,
    LinkerCreate,
    FindObject,
    ObjectNotFound,
    LoadObjectCall,
    ClassStatic,
    ChildOf,
    ClassMismatch,
    LoadAll,
    Strict,
    WarnError,
    ThrowError,
    FatalError,
    LogError,
    IsPackage,
    AllowDownload,
    SavingStatus,
    ObjectCount,
    ObjectAt,
    NameCount,
    NameExists,
    NameFlags,
    ClearNameFlags,
    SetNameFlags,
    Serialize,
    ExportArchiveScope,
    ImportArchiveScope,
    TagExportCall,
    TagImportCall,
    DeletedExportLog,
    ObjectUsage,
    SetObjectUsage,
    NameUsage,
    SetNameUsage,
    ObjectName,
    TransientPackage,
    TransientImport,
    PrivateImport,
    SavingObjectLog,
    NewSaver,
    DeleteSaver,
    ConformLog,
    CopyGuid,
    NewGuid,
    CopyGenerations,
    ClearGenerations,
    AddGeneration,
    SerializeSummary,
    Tell,
    Seek,
    SetNameOffset,
    SetNameCount,
    SetImportCountOf,
    SetExportCount,
    SetImportOffset,
    SetExportOffset,
    NameMapCount,
    NameMapAt,
    AddName,
    FindName,
    AppendNameSlot,
    SetNameMap,
    SwapNames,
    SortNameMap,
    SaveName,
    AddImport,
    SortImportMap,
    AddExport,
    SortExportMap,
    ExportRecord,
    ClearExportMap,
    ObjectFullName,
    ConformFullName,
    AppendExportRecord,
    AddEmptyExport,
    SetExportClass,
    SetExportSuper,
    SetExportPackage,
    SetExportOffsetOf,
    SetExportSize,
    IsClass,
    IsStruct,
    Super,
    SetImportPackage,
    SerializeImport,
    SerializeExport,
    ClosingStatus,
    LastGenerationCounts,
    DeleteTemp,
    SaveErrorLog,
    SaveTimeLog,
    MovingLog,
    MoveTemp,
    SaveWarning,
    DebugLoading,
    StartSaveTime,
    FindNameText,
    ObjectIndex,
    HashBucket,
    HashHead,
    HashNext,
    LookupCall,
    LookupMissing,
}
/// Scalar call state; native lifetime-owned scratch is deliberately opaque.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2PackageArgs {
    pub native: usize,
    pub outer: usize,
    pub base: usize,
    pub object_class: usize,
    pub name: usize,
    pub filename: usize,
    pub flags: usize,
    pub sandbox: usize,
    pub guid: usize,
    pub error: usize,
    pub conform: usize,
    pub linker: usize,
    pub archive: usize,
    pub context: usize,
}
#[repr(C)]
#[derive(Default)]
pub struct Hp2PackageConstants {
    pub check: i32,
    pub slow: i32,
    pub quiet: usize,
    pub throw_flag: usize,
    pub no_fail: usize,
    pub no_warn: usize,
    pub allow_dll: usize,
    pub disallow_files: usize,
    pub no_remap: usize,
    pub verify: usize,
    pub need_load: usize,
    pub native: usize,
    pub runtime_static: usize,
    pub transient: usize,
    pub public_flag: usize,
    pub tag_exp: usize,
    pub tag_imp: usize,
    pub load_edit: usize,
    pub load_client: usize,
    pub load_server: usize,
    pub not_edit: usize,
    pub not_client: usize,
    pub not_server: usize,
    pub load_context: usize,
    /// TCHAR capacity of the native text buffers (`Buffer` 0 and 1). UE1 copied
    /// names and filenames into them unchecked; the flow now refuses text that
    /// does not fit instead of overflowing the native stack.
    pub text_capacity: usize,
}
#[repr(C)]
pub struct Hp2PackageServices {
    pub call: unsafe extern "C" fn(
        *mut Hp2PackageArgs,
        Hp2PackageLeaf,
        usize,
        usize,
        usize,
        *mut usize,
    ) -> i32,
    pub constants: Hp2PackageConstants,
}
use Hp2PackageLeaf as L;
use Hp2PackageOperation as O;
pub(crate) trait LoadObjectsOps {
    type Error;
    fn loaded_count(&self) -> std::result::Result<usize, Self::Error>;
    fn loaded_at(&self, index: usize) -> std::result::Result<usize, Self::Error>;
    fn current(&self) -> usize;
    fn set_current(&self, object: usize);
    fn sort_loaded_from(&self, index: usize) -> std::result::Result<(), Self::Error>;
    fn needs_load(&self, object: usize) -> std::result::Result<bool, Self::Error>;
    fn preload_object(&self, object: usize) -> std::result::Result<(), Self::Error>;
    fn postload_object(&self, object: usize) -> std::result::Result<(), Self::Error>;
    fn check_postload_count(&self, original: usize) -> std::result::Result<(), Self::Error>;
    fn release_loaded(&self) -> std::result::Result<(), Self::Error>;
    fn linker(&self, object: usize) -> std::result::Result<usize, Self::Error>;
    fn object_index(&self, object: usize) -> std::result::Result<i32, Self::Error>;
    fn linker_index(&self, object: usize) -> std::result::Result<i32, Self::Error>;
    fn serial_offset(&self, linker: usize, row: i32) -> std::result::Result<i32, Self::Error>;
}

pub(crate) fn preload_loaded_with<O: LoadObjectsOps>(ops: &O) -> std::result::Result<(), O::Error> {
    let mut last_sorted = 0;
    ops.set_current(0);
    let mut index = 0;
    while index < ops.loaded_count()? {
        if ops.current() != 0
            && ops.loaded_count()? > last_sorted
            && index + 1 < ops.loaded_count()?
        {
            ops.sort_loaded_from(index)?;
            last_sorted = ops.loaded_count()?;
        }
        let object = ops.loaded_at(index)?;
        if ops.needs_load(object)? {
            ops.preload_object(object)?;
            ops.set_current(ops.loaded_at(index)?);
        }
        index += 1;
    }
    Ok(())
}

pub(crate) fn postload_loaded_with<O: LoadObjectsOps>(ops: &O) -> std::result::Result<(), O::Error> {
    let original = ops.loaded_count()?;
    let mut index = 0;
    while index < ops.loaded_count()? {
        ops.postload_object(ops.loaded_at(index)?)?;
        index += 1;
    }
    ops.check_postload_count(original)?;
    ops.release_loaded()
}

pub(crate) fn compare_loaded_with<O: LoadObjectsOps>(
    ops: &O, a: usize, b: usize,
) -> std::result::Result<i32, O::Error> {
    let current = ops.current();
    let al = ops.linker(a)?;
    let bl = ops.linker(b)?;
    let cl = ops.linker(current)?;
    if al != bl {
        if al == cl { return Ok(-1); }
        if bl == cl { return Ok(1); }
        // Stable linker object indices, rather than allocation addresses.
        let key = |linker| {
            if linker == 0 { Ok(-1) } else { ops.object_index(linker).map(i64::from) }
        };
        return Ok(if key(al)? > key(bl)? { 1 } else { -1 });
    }
    let ao = ops.serial_offset(al, ops.linker_index(a)?)?;
    let bo = ops.serial_offset(bl, ops.linker_index(b)?)?;
    if al == cl {
        let co = ops.serial_offset(cl, ops.linker_index(current)?)?;
        if ao > co && bo < co { return Ok(-1); }
        if ao < co && bo > co { return Ok(1); }
    }
    Ok(ao.wrapping_sub(bo))
}

// Deliberately process-global, not a saved/restored per-sort context. This is
// the legacy reentrancy behavior of CurObj and GTempSave.
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static TEMP_SAVE: AtomicUsize = AtomicUsize::new(0);
/// The shared source reset walk; callbacks reread live lists at each source
/// sequence point rather than retaining a snapshot across loader teardown.
pub(crate) trait ResetLoadersOps {
    type Error;
    fn loader_count(&self) -> std::result::Result<i32, Self::Error>;
    fn loader_at(&self, index: i32) -> std::result::Result<usize, Self::Error>;
    fn loader_root(&self, linker: usize) -> std::result::Result<usize, Self::Error>;
    fn export_count(&self, linker: usize) -> std::result::Result<usize, Self::Error>;
    fn export_object(&self, linker: usize, index: usize) -> std::result::Result<usize, Self::Error>;
    fn object_class_flags(&self, object: usize) -> std::result::Result<usize, Self::Error>;
    fn detach_export(&self, linker: usize, index: usize) -> std::result::Result<(), Self::Error>;
    fn detach_lazy(&self, linker: usize) -> std::result::Result<(), Self::Error>;
    fn delete_loader(&self, linker: usize) -> std::result::Result<(), Self::Error>;
}

pub(crate) fn reset_loaders_with<O: ResetLoadersOps>(
    ops: &O, outer: usize, dynamic_only: bool, force_lazy_load: bool, runtime_static: usize,
) -> std::result::Result<(), O::Error> {
    let mut index = ops.loader_count()? - 1;
    while index >= 0 {
        let linker = ops.loader_at(index)?;
        if outer == 0 || ops.loader_root(linker)? == outer {
            if dynamic_only {
                let mut export = 0;
                while export < ops.export_count(linker)? {
                    let object = ops.export_object(linker, export)?;
                    if object != 0 && ops.object_class_flags(object)? & runtime_static == 0 {
                        ops.detach_export(linker, export)?;
                    }
                    export += 1;
                }
            } else {
                if force_lazy_load { ops.detach_lazy(linker)?; }
                ops.delete_loader(linker)?;
            }
        }
        index -= 1;
    }
    Ok(())
}

struct Flow<'a> {
    s: &'a Hp2PackageServices,
    p: *mut Hp2PackageArgs,
    names: Option<unsafe fn() -> Result<PublishedNames>>,
}

impl ResetLoadersOps for Flow<'_> {
    type Error = i32;
    fn loader_count(&self) -> Result<i32> { self.leaf(L::LoaderCount).map(|count| count as i32) }
    fn loader_at(&self, index: i32) -> Result<usize> { self.get(L::LoaderCheckedAt, index as usize) }
    fn loader_root(&self, linker: usize) -> Result<usize> { self.get(L::Root, linker) }
    fn export_count(&self, linker: usize) -> Result<usize> { self.get(L::ExportCount, linker) }
    fn export_object(&self, linker: usize, index: usize) -> Result<usize> { self.call(L::ExportObject, linker, index, 0) }
    fn object_class_flags(&self, object: usize) -> Result<usize> { self.get(L::ClassFlags, self.get(L::Class, object)?) }
    fn detach_export(&self, linker: usize, index: usize) -> Result<()> { self.call(L::DetachExport, linker, index, 0).map(|_| ()) }
    fn detach_lazy(&self, linker: usize) -> Result<()> { self.get(L::DetachLazy, linker).map(|_| ()) }
    fn delete_loader(&self, linker: usize) -> Result<()> { self.get(L::DeleteLoad, linker).map(|_| ()) }
}
impl LoadObjectsOps for Flow<'_> {
    type Error = i32;
    fn loaded_count(&self) -> Result<usize> { self.leaf(L::LoadedCount) }
    fn loaded_at(&self, index: usize) -> Result<usize> { self.get(L::LoadedAt, index) }
    fn current(&self) -> usize { CURRENT.load(Ordering::Relaxed) }
    fn set_current(&self, object: usize) { CURRENT.store(object, Ordering::Relaxed); }
    fn sort_loaded_from(&self, index: usize) -> Result<()> {
        self.get(L::LoadedSort, index).map(|_| ())
    }
    fn needs_load(&self, object: usize) -> Result<bool> {
        Ok(self.get(L::Flags, object)? & self.s.constants.need_load != 0)
    }
    fn preload_object(&self, object: usize) -> Result<()> {
        let linker = self.get(L::Linker, object)?;
        if self.s.constants.check != 0 { self.check(linker != 0)?; }
        self.call(L::Preload, linker, object, 0).map(|_| ())
    }
    fn postload_object(&self, object: usize) -> Result<()> {
        self.get(L::Postload, object).map(|_| ())
    }
    fn check_postload_count(&self, original: usize) -> Result<()> {
        if self.s.constants.check != 0 {
            self.check(self.leaf(L::LoadedCount)? == original)?;
        }
        Ok(())
    }
    fn release_loaded(&self) -> Result<()> { self.leaf(L::LoadedRelease).map(|_| ()) }
    fn linker(&self, object: usize) -> Result<usize> { self.get(L::Linker, object) }
    fn object_index(&self, object: usize) -> Result<i32> {
        self.get(L::ObjectIndex, object).map(|index| index as i32)
    }
    fn linker_index(&self, object: usize) -> Result<i32> {
        self.get(L::LinkerIndex, object).map(|index| index as i32)
    }
    fn serial_offset(&self, linker: usize, row: i32) -> Result<i32> {
        self.call(L::SerialOffset, linker, row as usize, 0).map(|offset| offset as i32)
    }
}

impl Flow<'_> {
    fn a(&self) -> Hp2PackageArgs {
        unsafe { *self.p }
    }
    fn update(&self, f: impl FnOnce(&mut Hp2PackageArgs)) {
        unsafe { f(&mut *self.p) }
    }
    fn call(&self, leaf: L, a: usize, b: usize, c: usize) -> Result<usize> {
        let mut out = 0;
        let status = unsafe { (self.s.call)(self.p, leaf, a, b, c, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
    fn get(&self, leaf: L, a: usize) -> Result<usize> {
        self.call(leaf, a, 0, 0)
    }
    fn leaf(&self, leaf: L) -> Result<usize> {
        self.get(leaf, 0)
    }
    fn scope(&self, op: O) -> Result<usize> {
        self.get(L::Scope, op as usize)
    }
    fn check(&self, condition: bool) -> Result<()> {
        self.get(L::Check, condition as usize)?;
        Ok(())
    }
    /// Whether `text` copied at TCHAR `offset` of a native text buffer, plus its
    /// terminator, stays within `text_capacity`.
    fn fits(&self, text: usize, offset: usize) -> Result<bool> {
        let len = self.get(L::TextLength, text)?;
        Ok(offset
            .checked_add(len)
            .is_some_and(|end| end < self.s.constants.text_capacity))
    }
    fn catch(&self, status: i32) -> Result<()> {
        if self.leaf(L::CatchConst)? != 0 {
            Ok(())
        } else {
            Err(status)
        }
    }
    fn each_object(&self, mut f: impl FnMut(usize) -> Result<()>) -> Result<()> {
        let mut i = 0;
        while i < self.leaf(L::ObjectCount)? {
            let o = self.get(L::ObjectAt, i)?;
            if o != 0 {
                f(o)?;
            }
            i += 1;
        }
        Ok(())
    }
    fn each_name(&self, mut f: impl FnMut(usize) -> Result<()>) -> Result<()> {
        let mut i = 0;
        while i < self.leaf(L::NameCount)? {
            if self.get(L::NameExists, i)? != 0 {
                f(i)?;
            }
            i += 1;
        }
        Ok(())
    }
    fn loader_for(&self, outer: usize) -> Result<usize> {
        let mut i = 0;
        while i < self.leaf(L::LoaderCount)? {
            let l = self.get(L::LoaderAt, i)?;
            if self.get(L::Root, l)? == outer {
                return Ok(l);
            }
            i += 1;
        }
        Ok(0)
    }
    fn load_flags(&self, o: usize) -> Result<()> {
        let k = &self.s.constants;
        for (not, yes) in [
            (k.not_edit, k.load_edit),
            (k.not_client, k.load_client),
            (k.not_server, k.load_server),
        ] {
            if self.get(L::Flags, o)? & not == 0 {
                self.call(L::SetFlags, o, yes, 0)?;
            }
        }
        Ok(())
    }
    fn matches_class(&self, object: usize) -> Result<bool> {
        let a = self.a();
        if a.object_class == 0 {
            Ok(true)
        } else if a.flags != 0 {
            Ok(self.get(L::Class, object)? == a.object_class)
        } else {
            Ok(unsafe {
                crate::object_model::is_a(
                    object as *const std::ffi::c_void,
                    a.object_class as *const std::ffi::c_void,
                )
            })
        }
    }
    fn run(&self, op: O) -> Result<usize> {
        let k = &self.s.constants;
        let a = self.a();
        match op {
            O::Lookup => {
                let any = a.outer == usize::MAX;
                if any {
                    self.update(|a| a.outer = 0);
                }
                if self.call(L::ResolveCall, 0, 0, 0)? == 0 {
                    return Ok(0);
                }
                let a = self.a();
                lookup_object_with(self, self.get(L::FindNameText, a.name)?, a.outer, any)
            }
            O::LookupChecked => {
                let result = self.leaf(L::LookupCall)?;
                if result == 0 {
                    self.leaf(L::LookupMissing)?;
                }
                Ok(result)
            }
            O::SafeError => {
                let flags = if self.leaf(L::Strict)? != 0 {
                    k.no_fail
                } else {
                    a.flags
                };
                if flags & k.quiet == 0 {
                    self.leaf(L::WarnError)?;
                }
                if flags & k.throw_flag != 0 {
                    self.leaf(L::ThrowError)?;
                }
                if flags & k.no_fail != 0 {
                    self.leaf(L::FatalError)?;
                }
                if flags & k.no_warn == 0 {
                    self.leaf(L::LogError)?;
                }
                Ok(0)
            }
            O::Create => {
                self.call(L::ResolveCall, 1, 0, 0)?;
                let a = self.a();
                let found = self.call(L::FindPackage, a.outer, a.name, 0)?;
                if found != 0 {
                    Ok(found)
                } else {
                    self.call(L::NewPackage, a.outer, a.name, 0)
                }
            }
            O::Resolve => self.resolve(),
            O::GetLinker => self.get_linker(),
            O::LoadObject => self.load_object(),
            O::LoadClass => {
                let result = (|| {
                    let class = self.leaf(L::ClassStatic)?;
                    let result =
                        self.call(L::LoadObjectCall, class, a.outer, a.flags | k.throw_flag)?;
                    if result != 0 && self.call(L::ChildOf, result, a.object_class, 0)? == 0 {
                        self.call(L::ClassMismatch, result, a.object_class, 0)?;
                    }
                    Ok(result)
                })();
                match result {
                    Ok(r) => Ok(r),
                    Err(e) => {
                        self.catch(e)?;
                        self.get(L::SafeCaught, 2)?;
                        Ok(0)
                    }
                }
            }
            O::LoadPackage => {
                self.leaf(L::BeginCall)?;
                let result = (|| {
                    let filename = if a.filename != 0 {
                        a.filename
                    } else {
                        self.get(L::ObjectNameText, a.outer)?
                    };
                    let linker =
                        self.call(L::GetLinkerCall, a.outer, filename, a.flags | k.throw_flag)?;
                    if a.flags & k.verify == 0 {
                        self.get(L::LoadAll, linker)?;
                    }
                    let result = self.get(L::Root, linker)?;
                    self.leaf(L::EndCall)?;
                    Ok(result)
                })();
                match result {
                    Ok(r) => Ok(r),
                    Err(e) => {
                        self.catch(e)?;
                        self.leaf(L::EndCall)?;
                        self.get(L::SafeCaught, 3)?;
                        Ok(0)
                    }
                }
            }
            O::Begin => {
                let count = (self.leaf(L::BeginCount)? as i32).wrapping_add(1);
                self.get(L::SetBeginCount, count as usize)?;
                if count == 1 && k.check != 0 {
                    self.check(self.leaf(L::LoadedCount)? == 0)?;
                    self.check(self.leaf(L::AutoRegister)? == 0)?;
                    let mut i = 0;
                    while i < self.leaf(L::LoaderCount)? {
                        self.check(self.get(L::Success, self.get(L::LoaderAt, i)?)? != 0)?;
                        i += 1;
                    }
                }
                Ok(0)
            }
            O::End => {
                let count = self.leaf(L::BeginCount)? as i32;
                if k.check != 0 {
                    self.check(count > 0)?;
                }
                let count = count.wrapping_sub(1);
                self.get(L::SetBeginCount, count as usize)?;
                if count == 0 {
                    let result = (|| {
                        self.scope(O::Preload)?;
                        self.scope(O::Postload)?;
                        self.leaf(L::FlushLocalized)?;
                        self.scope(O::Dissociate)?;
                        Ok(0)
                    })();
                    if let Err(e) = result {
                        self.catch(e)?;
                        self.leaf(L::FatalCaught)?;
                    }
                }
                Ok(0)
            }
            O::Preload => {
                self.leaf(L::DebugLoading)?;
                preload_loaded_with(self)?;
                Ok(0)
            }
            O::SortLoaded => self.sort_loaded(a.outer as *mut usize, a.base),
            O::CompareLoaded => self.compare_loaded(a.outer, a.base),
            O::Postload => {
                postload_loaded_with(self)?;
                Ok(0)
            }
            O::Dissociate => {
                if self.leaf(L::ImportCount)? != 0 {
                    let mut i = 0;
                    while i < self.leaf(L::LoaderCount)? {
                        let mut j = 0;
                        while j < self.get(L::ImportCountOf, self.get(L::LoaderAt, i)?)? {
                            let loader = self.get(L::LoaderAt, i)?;
                            let object = self.call(L::ImportObject, loader, j, 0)?;
                            if object != 0 && self.get(L::Flags, object)? & k.native == 0 {
                                self.call(L::SetImportObject, loader, j, 0)?;
                            }
                            j += 1;
                        }
                        i += 1;
                    }
                }
                self.get(L::SetImportCount, 0)
            }
            O::Reset => {
                reset_loaders_with(self, a.outer, a.flags != 0, a.context != 0, k.runtime_static)?;
                Ok(0)
            }
            O::TagExport => {
                let o = a.base;
                if o != 0 && self.get(L::PendingKill, o)? != 0 {
                    self.get(L::DeletedExportLog, o)?;
                }
                if o != 0
                    && self.get(L::PendingKill, o)? == 0
                    && self.call(L::IsIn, o, a.outer, 0)? != 0
                    && self.get(L::Flags, o)? & (k.transient | k.tag_exp) == 0
                {
                    self.call(L::SetFlags, o, k.tag_exp, 0)?;
                    self.load_flags(o)?;
                    let class = self.get(L::Class, o)?;
                    let parent = self.get(L::Outer, o)?;
                    self.call(L::TagExportCall, a.archive, class, 0)?;
                    self.call(L::TagExportCall, a.archive, parent, 0)?;
                    self.call(L::Serialize, o, a.archive, 0)?;
                }
                Ok(0)
            }
            O::TagImport => {
                let o = a.base;
                if o != 0 && self.get(L::PendingKill, o)? == 0 {
                    let flags = self.get(L::Flags, o)?;
                    if flags & k.transient == 0 || flags & k.public_flag != 0 {
                        let count = self.call(L::ObjectUsage, a.linker, o, 0)? as i32;
                        self.call(
                            L::SetObjectUsage,
                            a.linker,
                            o,
                            count.wrapping_add(1) as usize,
                        )?;
                        if self.get(L::Flags, o)? & k.tag_exp == 0 {
                            self.call(L::SetFlags, o, k.tag_imp, 0)?;
                            self.load_flags(o)?;
                            let parent = self.get(L::Outer, o)?;
                            if parent != 0 {
                                self.call(L::TagImportCall, a.archive, parent, 0)?;
                            }
                        }
                    }
                }
                Ok(0)
            }
            O::TagName => {
                self.call(L::SetNameFlags, a.name, k.tag_exp | a.context, 0)?;
                let count = self.call(L::NameUsage, a.linker, a.name, 0)? as i32;
                self.call(
                    L::SetNameUsage,
                    a.linker,
                    a.name,
                    count.wrapping_add(1) as usize,
                )
            }
            O::Save => self.save(),
            O::Untag => {
                let mask = k.tag_imp | k.tag_exp | k.load_edit | k.load_client | k.load_server;
                self.each_object(|o| {
                    self.call(L::ClearFlags, o, mask, 0)?;
                    Ok(())
                })?;
                self.each_name(|n| {
                    self.call(L::ClearNameFlags, n, mask, 0)?;
                    Ok(())
                })?;
                Ok(0)
            }
            O::TagExports => {
                let mut flow = self;
                package_save::tag_exports_with(
                    &mut flow,
                    a.base as *mut hp2_abi::engine::generated::UObject,
                    a.flags as u32,
                    a.outer as *mut hp2_abi::engine::generated::UObject,
                    |flow| flow.leaf(L::ObjectCount),
                    |flow, index| {
                        flow.get(L::ObjectAt, index)
                            .map(|object| object as *mut hp2_abi::engine::generated::UObject)
                    },
                    |flow, object| flow.get(L::Flags, object as usize).map(|flags| flags as u32),
                    |flow, object, outer| {
                        flow.call(L::IsIn, object as usize, outer as usize, 0)
                            .map(|inside| inside != 0)
                    },
                    |flow, object| {
                        flow.call(L::TagExportCall, flow.a().archive, object as usize, 0)
                            .map(|_| ())
                    },
                )?;
                Ok(0)
            }
            O::TagImports => {
                let mut flow = self;
                package_save::tag_imports_with(
                    &mut flow,
                    k.tag_exp as u32,
                    |flow| flow.leaf(L::ObjectCount),
                    |flow, index| {
                        flow.get(L::ObjectAt, index)
                            .map(|object| object as *mut hp2_abi::engine::generated::UObject)
                    },
                    |flow, object| flow.get(L::Flags, object as usize).map(|flags| flags as u32),
                    |flow, object| {
                        let object = object as usize;
                        let flags = flow.get(L::Flags, object)?;
                        flow.call(L::ImportArchiveScope, object, flags & flow.s.constants.load_context, 0)?;
                        if flow.call(L::IsIn, object, flow.leaf(L::TransientPackage)?, 0)? != 0 {
                            flow.get(L::TransientImport, object)?;
                        }
                        Ok(())
                    },
                )?;
                Ok(0)
            }
            O::ExportNames => {
                let mask = (k.tag_exp | k.load_edit | k.load_client | k.load_server) as u32;
                let mut flow = self;
                package_save::export_names_with(
                    &mut flow,
                    k.tag_exp as u32,
                    k.tag_imp as u32,
                    mask,
                    k.public_flag as u32,
                    k.check != 0,
                    |flow| flow.leaf(L::ObjectCount),
                    |flow, index| {
                        flow.get(L::ObjectAt, index)
                            .map(|object| object as *mut hp2_abi::engine::generated::UObject)
                    },
                    |flow, object| flow.get(L::Flags, object as usize).map(|flags| flags as u32),
                    |flow, object| flow.get(L::ObjectName, object as usize).map(|name| name as i32),
                    |flow, object| flow.get(L::Outer, object as usize).map(|outer| outer as *mut _),
                    |flow, object| flow.get(L::Class, object as usize).map(|class| class as *mut _),
                    |flow, name, flags| {
                        flow.call(L::SetNameFlags, name as usize, flags as usize, 0).map(|_| ())
                    },
                    |flow, condition| flow.check(condition),
                    |flow, object| flow.get(L::PrivateImport, object as usize).map(|_| ()),
                    |flow, object| flow.get(L::SavingObjectLog, object as usize).map(|_| ()),
                )?;
                Ok(0)
            }
            O::Summary => {
                if a.conform != 0 {
                    self.leaf(L::ConformLog)?;
                    self.leaf(L::CopyGuid)?;
                    self.leaf(L::CopyGenerations)?;
                } else {
                    self.leaf(L::NewGuid)?;
                    self.leaf(L::ClearGenerations)?;
                }
                self.leaf(L::AddGeneration)?;
                self.leaf(L::SerializeSummary)
            }
            O::BuildNames => {
                let mut flow = self;
                package_save::build_names_with(
                    &mut flow,
                    k.tag_exp as u32,
                    |flow| flow.get(L::Tell, flow.a().linker).map(|offset| offset as u64),
                    |flow, offset| flow.get(L::SetNameOffset, offset as usize).map(|_| ()),
                    |flow| flow.leaf(L::NameCount),
                    |_flow, index| Ok(index as i32),
                    |flow, name| {
                        if flow.get(L::NameExists, name as usize)? == 0 { return Ok(0); }
                        flow.get(L::NameFlags, name as usize).map(|flags| flags as u32)
                    },
                    |flow, name| flow.get(L::AddName, name as usize).map(|_| ()),
                )?;
                Ok(0)
            }
            O::SortNames => {
                TEMP_SAVE.store(a.linker, Ordering::Relaxed);
                let first = if a.conform != 0 {
                    self.run(O::ConformNames)?;
                    self.get(L::NameMapCount, a.conform)?
                } else {
                    0
                };
                self.get(L::SortNameMap, first)
            }
            O::ConformNames => {
                let original = if k.slow != 0 {
                    (0..self.get(L::NameMapCount, a.linker)?)
                        .map(|index| self.call(L::NameMapAt, a.linker, index, 0).map(|name| name as i32))
                        .collect::<Result<Vec<_>>>()?
                } else {
                    Vec::new()
                };
                let mut flow = self;
                package_save::conform_name_prefix_with(
                    &mut flow,
                    |flow| flow.get(L::NameMapCount, flow.a().conform),
                    |flow, index| {
                        flow.call(L::NameMapAt, flow.a().conform, index, 0)
                            .map(|name| name as i32)
                    },
                    |flow, name| {
                        let found = flow.call(L::FindName, flow.a().linker, name as usize, 0)? as i32;
                        Ok((found >= 0).then_some(found as usize))
                    },
                    |flow, index| {
                        flow.call(L::NameMapAt, flow.a().linker, index, 0)
                            .map(|name| name as i32)
                    },
                    |flow| {
                        flow.leaf(L::AppendNameSlot)?;
                        Ok(flow.get(L::NameMapCount, flow.a().linker)? - 1)
                    },
                    |flow, index, name| {
                        flow.call(L::SetNameMap, index, name as usize, 0).map(|_| ())
                    },
                    |flow, left, right| {
                        flow.call(L::SwapNames, left, right, 0).map(|_| ())
                    },
                )?;
                package_save::check_conform_names_with(
                    &mut flow,
                    k.slow != 0,
                    k.check != 0,
                    |flow| flow.get(L::NameMapCount, flow.a().conform),
                    |flow, index| {
                        flow.call(L::NameMapAt, flow.a().conform, index, 0)
                            .map(|name| name as i32)
                    },
                    |flow, index| {
                        flow.call(L::NameMapAt, flow.a().linker, index, 0)
                            .map(|name| name as i32)
                    },
                    |_flow| Ok(original.len()),
                    |_flow, index| Ok(original[index]),
                    |flow, name| {
                        let found = flow.call(L::FindName, flow.a().linker, name as usize, 0)? as i32;
                        Ok((found >= 0).then_some(found as usize))
                    },
                    |flow, condition| flow.check(condition),
                )?;
                Ok(0)
            }
            O::SaveNames => {
                let mut flow = self;
                package_save::save_names_with(
                    &mut flow,
                    |flow| flow.get(L::NameMapCount, flow.a().linker),
                    |flow, index| Ok(flow.call(L::NameMapAt, flow.a().linker, index, 0)? as i32),
                    |flow, count| flow.get(L::SetNameCount, count).map(|_| ()),
                    |flow, name| flow.get(L::SaveName, name as usize).map(|_| ()),
                    |flow, name, usage| {
                        flow.call(
                            L::SetNameUsage,
                            flow.a().linker,
                            name as usize,
                            usage as usize,
                        )
                        .map(|_| ())
                    },
                )?;
                Ok(0)
            }
            O::BuildImports | O::BuildExports => {
                let imports = matches!(op, O::BuildImports);
                let mut flow = self;
                package_save::build_object_map_with(
                    &mut flow,
                    imports,
                    k.tag_imp as u32,
                    k.tag_exp as u32,
                    |flow| flow.leaf(L::ObjectCount),
                    |flow, index| {
                        flow.get(L::ObjectAt, index)
                            .map(|object| object as *mut hp2_abi::engine::generated::UObject)
                    },
                    |flow, object| flow.get(L::Flags, object as usize).map(|flags| flags as u32),
                    |flow, object| flow.get(L::AddImport, object as usize).map(|_| ()),
                    |flow, object| flow.get(L::AddExport, object as usize).map(|_| ()),
                )?;
                if imports {
                    self.get(L::SetImportCountOf, self.get(L::ImportCountOf, a.linker)?)?;
                }
                Ok(0)
            }
            O::SortImports => {
                TEMP_SAVE.store(a.linker, Ordering::Relaxed);
                self.leaf(L::SortImportMap)
            }
            O::SortExports => {
                let first = if a.conform != 0 {
                    self.run(O::ConformExports)?
                } else {
                    0
                };
                self.get(L::SortExportMap, first)?;
                self.get(L::SetExportCount, self.get(L::ExportCount, a.linker)?)
            }
            O::ConformExports => {
                // execute's runtime contract keeps the published native allocator live.
                let allocator = unsafe { array_storage::published_allocator()? };
                let original = (0..self.get(L::ExportCount, a.linker)?)
                    .map(|index| {
                        let record = self.call(L::ExportRecord, a.linker, index, 0)?;
                        // Copy before callbacks can resize the source table.
                        Ok(ConformExport(unsafe {
                            (record as *const std::mem::MaybeUninit<FObjectExport>).read_unaligned()
                        }))
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.get(L::ClearExportMap, original.len())?;
                let mut state = (self, vec![false; original.len()], HashMap::with_capacity(original.len()));
                let first = package_save::conform_exports_with(
                    &mut state,
                    k.check != 0,
                    k.slow != 0,
                    |_| Ok(original.len()),
                    |(flow, _, names), index| {
                        let mut path = ConformPath::new(allocator);
                        flow.call(L::ObjectFullName, original[index].object(), ptr::addr_of_mut!(path.raw) as usize, 0)?;
                        names.insert(path, index);
                        Ok(())
                    },
                    |(flow, _, _)| flow.get(L::ExportCount, flow.a().conform),
                    |(flow, _, names), index| {
                        let mut path = ConformPath::new(allocator);
                        flow.call(L::ConformFullName, index, ptr::addr_of_mut!(path.raw) as usize, 0)?;
                        Ok(names.get(&path).copied())
                    },
                    |(flow, _, _), index| flow.get(L::AppendExportRecord, original[index].0.as_ptr() as usize).map(|_| ()),
                    |(flow, _, _), index| {
                        let last = flow.get(L::ExportCount, flow.a().linker)? - 1;
                        let object = flow.call(L::ExportObject, flow.a().linker, last, 0)?;
                        Ok(object == original[index].object())
                    },
                    |(_, used, _), index| { used[index] = true; Ok(()) },
                    |(flow, _, _)| flow.leaf(L::AddEmptyExport).map(|_| ()),
                    |(_, used, _), index| Ok(used[index]),
                    |(flow, _, _), index| {
                        for export in 0..flow.get(L::ExportCount, flow.a().linker)? {
                            if flow.call(L::ExportObject, flow.a().linker, export, 0)? == original[index].object() {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    },
                    |(flow, _, _), condition| flow.check(condition),
                )?;
                Ok(first)
            }
            O::CompareName | O::CompareImport | O::CompareExport => {
                let linker = TEMP_SAVE.load(Ordering::Relaxed);
                let leaf = if matches!(op, O::CompareName) {
                    L::NameUsage
                } else {
                    L::ObjectUsage
                };
                let first = self.call(leaf, linker, a.outer, 0)? as i32;
                let second = self.call(leaf, linker, a.base, 0)? as i32;
                Ok(package_save::compare_qsort_mapped_values(first, second) as usize)
            }
            O::Mappings => {
                let mut flow = self;
                package_save::build_object_mappings_with(
                    &mut flow,
                    |flow| flow.get(L::ExportCount, flow.a().linker),
                    |flow, index| {
                        flow.call(L::ExportObject, flow.a().linker, index, 0)
                            .map(|object| object as *mut hp2_abi::engine::generated::UObject)
                    },
                    |flow| flow.get(L::ImportCountOf, flow.a().linker),
                    |flow, index| {
                        flow.call(L::ImportObject, flow.a().linker, index, 0)
                            .map(|object| object as *mut hp2_abi::engine::generated::UObject)
                    },
                    |flow, object, usage| {
                        flow.call(
                            L::SetObjectUsage,
                            flow.a().linker,
                            object as usize,
                            usage as usize,
                        )
                        .map(|_| ())
                    },
                )?;
                Ok(0)
            }
            O::SaveExports => self.save_exports(),
            O::SaveImports => {
                let mut flow = self;
                package_save::save_imports_with(
                    &mut flow,
                    |flow| flow.get(L::Tell, flow.a().linker).map(|offset| offset as u64),
                    |flow, offset| flow.get(L::SetImportOffset, offset as usize).map(|_| ()),
                    |flow| flow.get(L::ImportCountOf, flow.a().linker),
                    |flow, index| {
                        let args = flow.a();
                        let object = flow.call(L::ImportObject, args.linker, index, 0)?;
                        let outer = flow.get(L::Outer, object)?;
                        if outer != 0 {
                            if flow.s.constants.check != 0 {
                                flow.check(flow.call(L::IsIn, outer, args.outer, 0)? == 0)?;
                            }
                            let mapped = flow.call(L::ObjectUsage, args.linker, outer, 0)?;
                            flow.call(L::SetImportPackage, index, mapped, 0)?;
                            if flow.s.constants.check != 0 {
                                flow.check((mapped as i32) < 0)?;
                            }
                        }
                        flow.get(L::SerializeImport, index).map(|_| ())
                    },
                )?;
                Ok(0)
            }
            O::SaveExportMap => {
                let mut flow = self;
                package_save::save_export_map_with(
                    &mut flow,
                    |flow| flow.get(L::Tell, flow.a().linker).map(|offset| offset as u64),
                    |flow, offset| flow.get(L::SetExportOffset, offset as usize).map(|_| ()),
                    |flow| flow.get(L::ExportCount, flow.a().linker),
                    |flow, index| flow.get(L::SerializeExport, index).map(|_| ()),
                )?;
                Ok(0)
            }
            O::RewriteSummary => {
                let mut flow = self;
                package_save::rewrite_summary_with(
                    &mut flow,
                    |flow| flow.leaf(L::ClosingStatus).map(|_| ()),
                    |flow| flow.leaf(L::LastGenerationCounts).map(|_| ()),
                    |flow| flow.call(L::Seek, flow.a().linker, 0, 0).map(|_| ()),
                    |flow| flow.leaf(L::SerializeSummary).map(|_| ()),
                )?;
                Ok(0)
            }
        }
    }
    fn resolve(&self) -> Result<usize> {
        let a = self.a();
        if self.s.constants.check != 0 {
            self.check(a.name != 0)?;
        }
        resolve_name_with(&mut &*self).map(usize::from)
    }
    fn get_linker(&self) -> Result<usize> {
        let k = &self.s.constants;
        if k.check != 0 {
            self.check(self.leaf(L::BeginCount)? != 0)?;
        }
        let a = self.a();
        let mut result = if a.outer != 0 {
            self.loader_for(a.outer)?
        } else {
            0
        };
        let attempted = (|| {
            let filename = self.get(L::Buffer, 0)?;
            self.call(L::TerminateText, filename, 0, 0)?;
            if result == 0 {
                if a.filename == 0 {
                    if a.outer == 0 {
                        self.leaf(L::PackageResolveFailed)?;
                    }
                    let name = self.get(L::ObjectNameText, a.outer)?;
                    if self.call(L::FindPackageFile, name, filename, a.guid)? == 0 {
                        if a.flags & k.allow_dll != 0
                            && self.get(L::IsPackage, a.outer)? != 0
                            && self.get(L::PackageDll, a.outer)? != 0
                        {
                            return Ok(true);
                        }
                        self.leaf(L::PackageNotFound)?;
                    }
                } else {
                    if self.call(L::FindPackageFile, a.filename, filename, a.guid)? == 0
                        || !self.fits(a.filename, 0)?
                    {
                        self.leaf(L::FileNotFound)?;
                    }
                    let tmp = self.get(L::Buffer, 1)?;
                    self.call(L::CopyText, tmp, a.filename, 0)?;
                    let mut t = tmp;
                    loop {
                        let path = self.get(L::FindPath, t)?;
                        if path != 0 {
                            t = path;
                            continue;
                        }
                        let slash = self.get(L::FindSlash, t)?;
                        if slash != 0 {
                            t = self.call(L::AdvanceText, slash, 1, 0)?;
                            continue;
                        }
                        let colon = self.get(L::FindColon, t)?;
                        if colon != 0 {
                            t = self.call(L::AdvanceText, colon, 1, 0)?;
                            continue;
                        }
                        break;
                    }
                    let dot = self.get(L::FindDot, t)?;
                    if dot != 0 {
                        self.call(L::TerminateText, dot, 0, 0)?;
                    }
                    let package = self.call(L::CreateCall, 0, t, 0)?;
                    if a.outer == 0 {
                        if package == 0 {
                            self.leaf(L::FilenameToPackage)?;
                        }
                        self.update(|a| a.outer = package);
                        result = self.loader_for(package)?;
                    } else if a.outer != package {
                        self.get(L::NewFileLog, package)?;
                        self.call(L::ResetCall, a.outer, 0, 1)?;
                    }
                }
            }
            if result == 0 {
                result = self.get(L::NewLoader, filename)?;
            }
            if a.guid != 0 && self.call(L::GuidEqual, result, a.guid, 0)? == 0 {
                self.leaf(L::PackageVersion)?;
            }
            Ok(false)
        })();
        match attempted {
            Ok(true) => return Ok(0),
            Ok(false) => {}
            Err(e) => {
                self.catch(e)?;
                self.get(L::SafeCaught, 0)?;
            }
        }
        let a = self.a();
        if a.sandbox != 0
            && a.outer != 0
            && self.call(L::SandboxSupports, a.sandbox, a.outer, 0)? == 0
        {
            self.leaf(L::SandboxLog)?;
            result = 0;
        }
        Ok(result)
    }
    fn load_object(&self) -> Result<usize> {
        let k = &self.s.constants;
        let a = self.a();
        if k.check != 0 {
            self.check(a.object_class != 0)?;
            self.check(a.name != 0)?;
        }
        let mut result = 0;
        self.leaf(L::BeginCall)?;
        let attempted = (|| {
            self.call(L::ResolveCall, 1, 1, 0)?;
            loop {
                let outer = self.a().outer;
                if outer == 0 {
                    break;
                }
                let parent = self.get(L::Outer, outer)?;
                if parent == 0 {
                    break;
                }
                self.update(|a| a.outer = parent);
            }
            let a = self.a();
            let linker = if a.flags & k.disallow_files == 0 {
                self.call(
                    L::GetLinkerCall,
                    a.outer,
                    a.filename,
                    a.flags | k.throw_flag | k.allow_dll,
                )?
            } else {
                0
            };
            if linker != 0 {
                result = self.get(L::LinkerCreate, linker)?;
            }
            if result == 0 {
                result = self.leaf(L::FindObject)?;
            }
            if result == 0 {
                self.leaf(L::ObjectNotFound)?;
            }
            self.leaf(L::EndCall)?;
            Ok(())
        })();
        if let Err(e) = attempted {
            self.catch(e)?;
            self.leaf(L::EndCall)?;
            let a = self.a();
            if a.outer != 0 && a.flags & k.no_remap == 0 && a.filename == 0 {
                let name = self.get(L::ObjectName, a.outer)? as i32;
                // SAFETY: only this serialized lookup accesses the map, and the
                // snapshot is complete before CreateCall/LoadObjectCall reentry.
                let mut remaps = unsafe {
                    let owner = (*(&raw const PACKAGE_REMAPS)).as_ref().ok_or(1)? as *const _;
                    remap_snapshot(owner, name)?
                };
                let remapped = (|| {
                    let mut result = result;
                    for i in 0..remaps.len {
                        // SAFETY: the provider is copied, not a table borrow;
                        // acquire it only on the uncommon remap path.
                        let names = unsafe { (self.names.ok_or(1)?)()? };
                        // SAFETY: canonical indices are resolved at each use,
                        // just as dereferencing each native snapshot FName was.
                        let text = unsafe { names.text(remaps.at(i)) } as usize;
                        let outer = self.call(L::CreateCall, 0, text, 0)?;
                        self.update(|a| a.outer = outer);
                        result = self.call(
                            L::LoadObjectCall,
                            a.object_class,
                            outer,
                            a.flags | k.no_remap,
                        )?;
                        if result != 0 {
                            return Ok(result);
                        }
                    }
                    self.get(L::SafeCaught, 1)?;
                    Ok(result)
                })();
                // SAFETY: snapshot cleanup occurs on success and every error,
                // preserving the first status and native pending exception.
                return unsafe { remaps.finish(remapped) };
            }
            self.get(L::SafeCaught, 1)?;
        }
        Ok(result)
    }
    fn sort_loaded(&self, objects: *mut usize, count: usize) -> Result<usize> {
        crate::sort::quicksort(
            count as isize,
            |invariant_ok| {
                if self.s.constants.slow != 0 {
                    self.get(L::CheckSlow, invariant_ok as usize)?;
                }
                Ok(())
            },
            |i, j| {
                self.compare_loaded(unsafe { *objects.add(i as usize) }, unsafe {
                    *objects.add(j as usize)
                })
                .map(|v| v as i32)
            },
            |i, j| unsafe { ptr::swap(objects.add(i as usize), objects.add(j as usize)) },
        )?;
        Ok(0)
    }
    fn compare_loaded(&self, a: usize, b: usize) -> Result<usize> {
        if self.s.constants.slow != 0 {
            self.get(L::CheckSlow, (CURRENT.load(Ordering::Relaxed) != 0) as usize)?;
        }
        compare_loaded_with(self, a, b).map(|order| order as usize)
    }
    fn save(&self) -> Result<usize> {
        let a = self.a();
        if self.s.constants.check != 0 {
            self.check(a.outer != 0)?;
            self.check(a.filename != 0)?;
        }
        if !self.fits(a.filename, 0)? {
            self.leaf(L::SaveWarning)?;
            return Ok(0);
        }
        self.leaf(L::StartSaveTime)?;
        if self.get(L::IsPackage, a.outer)? != 0 {
            self.get(L::AllowDownload, a.outer)?;
        }
        let tmp = self.get(L::Buffer, 0)?;
        self.call(L::CopyText, tmp, a.filename, 0)?;
        // Native character primitives preserve TCHAR width and the legacy path
        // separator precedence without converting or allocating the filename.
        let mut c = self.get(L::TextLength, tmp)?;
        while c > 0 && self.call(L::FindPath, tmp, c, 1)? == 0 {
            c -= 1;
        }
        self.call(L::TerminateText, tmp, c, 0)?;
        if !self.fits(self.get(L::Buffer, 2)?, c)? {
            self.leaf(L::SaveWarning)?;
            return Ok(0);
        }
        self.call(
            L::CopyText,
            self.call(L::AdvanceText, tmp, c, 0)?,
            self.get(L::Buffer, 2)?,
            0,
        )?;
        self.leaf(L::SavingStatus)?;
        self.call(L::ResetCall, a.outer, 0, 1)?;
        self.scope(O::Untag)?;
        self.leaf(L::ExportArchiveScope)?;
        let attempted = (|| {
            let linker = self.get(L::NewSaver, tmp)?;
            self.update(|a| a.linker = linker);
            let mut flow = self;
            package_save::run_save_phases_with(&mut flow, |flow, phase| {
                let phase = match phase {
                    package_save::SavePhase::TagImports => O::TagImports,
                    package_save::SavePhase::ExportNames => O::ExportNames,
                    package_save::SavePhase::InitializeSummary => O::Summary,
                    package_save::SavePhase::BuildNames => O::BuildNames,
                    package_save::SavePhase::SortNames => O::SortNames,
                    package_save::SavePhase::SaveNames => O::SaveNames,
                    package_save::SavePhase::BuildImports => O::BuildImports,
                    package_save::SavePhase::SortImports => O::SortImports,
                    package_save::SavePhase::BuildExports => O::BuildExports,
                    package_save::SavePhase::SortExports => O::SortExports,
                    package_save::SavePhase::Mappings => O::Mappings,
                    package_save::SavePhase::SaveExports => O::SaveExports,
                    package_save::SavePhase::SaveImports => O::SaveImports,
                    package_save::SavePhase::SaveExportMap => O::SaveExportMap,
                    package_save::SavePhase::RewriteSummary => O::RewriteSummary,
                };
                flow.scope(phase).map(|_| ())
            })?;
            Ok(())
        })();
        let mut success = true;
        if let Err(e) = attempted {
            self.catch(e)?;
            self.leaf(L::DeleteTemp)?;
            self.leaf(L::SaveErrorLog)?;
            success = false;
        }
        if self.a().linker != 0 {
            self.get(L::DeleteSaver, self.a().linker)?;
        }
        self.leaf(L::SaveTimeLog)?;
        if success {
            self.leaf(L::MovingLog)?;
            if self.leaf(L::MoveTemp)? == 0 {
                self.leaf(L::DeleteTemp)?;
                self.leaf(L::SaveWarning)?;
                success = false;
            }
        }
        Ok(success as usize)
    }
    fn save_exports(&self) -> Result<usize> {
        let mut flow = self;
        package_save::save_exports_with(
            &mut flow,
            |flow| flow.get(L::ExportCount, flow.a().linker),
            |flow, index| {
                flow.call(L::ExportObject, flow.a().linker, index, 0)
                    .map(|object| object as *mut hp2_abi::engine::generated::UObject)
            },
            |flow, index, object| {
                let args = flow.a();
                let object = object as usize;
                if flow.get(L::IsClass, object)? == 0 {
                    let class = flow.get(L::Class, object)?;
                    let mapped = flow.call(L::ObjectUsage, args.linker, class, 0)?;
                    flow.call(L::SetExportClass, index, mapped, 0)?;
                    if flow.s.constants.check != 0 { flow.check(mapped != 0)?; }
                }
                if flow.get(L::IsStruct, object)? != 0 {
                    let parent = flow.get(L::Super, object)?;
                    if parent != 0 {
                        let mapped = flow.call(L::ObjectUsage, args.linker, parent, 0)?;
                        flow.call(L::SetExportSuper, index, mapped, 0)?;
                        if flow.s.constants.check != 0 { flow.check(mapped != 0)?; }
                    }
                }
                let outer = flow.get(L::Outer, object)?;
                if outer != args.outer {
                    if flow.s.constants.check != 0 {
                        flow.check(flow.call(L::IsIn, outer, args.outer, 0)? != 0)?;
                    }
                    let mapped = flow.call(L::ObjectUsage, args.linker, outer, 0)?;
                    flow.call(L::SetExportPackage, index, mapped, 0)?;
                    if flow.s.constants.check != 0 { flow.check((mapped as i32) > 0)?; }
                }
                let offset = flow.get(L::Tell, args.linker)?;
                flow.call(L::SetExportOffsetOf, index, offset, 0).map(|_| ())
            },
            |flow, object| {
                flow.call(L::Serialize, object as usize, flow.a().linker, 1)
                    .map(|_| ())
            },
            |flow| flow.get(L::Tell, flow.a().linker).map(|offset| offset as u64),
            |flow, index| {
                flow.call(L::SerialOffset, flow.a().linker, index, 0)
                    .map(|offset| offset as u64)
            },
            |flow, index, size| {
                flow.call(L::SetExportSize, index, size as usize, 0)
                    .map(|_| ())
            },
        )?;
        Ok(0)
    }
}

impl ObjectLookupOps for Flow<'_> {
    type Error = i32;
    fn hash_head(&self, name: usize, outer: usize) -> Result<usize> {
        let index = if outer == 0 { 0 } else { self.get(L::ObjectIndex, outer)? };
        self.get(L::HashHead, self.call(L::HashBucket, name, index, 0)?)
    }
    fn name(&self, object: usize) -> Result<usize> { self.get(L::ObjectName, object) }
    fn outer(&self, object: usize) -> Result<usize> { self.get(L::Outer, object) }
    fn matches_class(&self, object: usize) -> Result<bool> { Flow::matches_class(self, object) }
    fn next(&self, object: usize) -> Result<usize> { self.get(L::HashNext, object) }
    fn count(&self) -> Result<usize> { self.leaf(L::ObjectCount) }
    fn at(&self, index: usize) -> Result<usize> { self.get(L::ObjectAt, index) }
}

impl NameResolutionOps for &Flow<'_> {
    type Error = i32;
    fn config_prefix(&mut self) -> Result<Option<bool>> {
        let system = self.get(L::IniPrefix, self.a().name)? != 0;
        let user = self.get(L::UserPrefix, self.a().name)? != 0;
        Ok(if system || user { Some(system) } else { None })
    }
    fn name_length(&mut self) -> Result<usize> { self.get(L::TextLength, self.a().name) }
    fn has_separator(&mut self) -> Result<bool> { Ok(self.get(L::FindDot, self.a().name)? != 0) }
    fn resolve_config(&mut self, system: bool) -> Result<bool> {
        let section = self.get(L::Buffer, 3)?;
        self.call(L::CopyText, section, self.call(L::AdvanceText, self.a().name, 4, 0)?, 0)?;
        let mut key = section;
        loop {
            let dot = self.get(L::FindDot, key)?;
            if dot == 0 { break; }
            key = self.call(L::AdvanceText, dot, 1, 0)?;
        }
        if self.s.constants.check != 0 { self.check(key != section)?; }
        self.call(L::TerminateText, key, usize::MAX, 0)?;
        let name = self.call(L::ConfigGet, section, key, usize::from(system))?;
        if name == 0 {
            if self.a().context != 0 { self.leaf(L::ConfigMissing)?; }
            return Ok(false);
        }
        self.update(|a| a.name = name);
        Ok(true)
    }
    fn partial_fits(&mut self) -> Result<bool> { self.fits(self.a().name, 0) }
    fn resolve_outer(&mut self) -> Result<Option<usize>> {
        let a = self.a();
        let partial = self.get(L::Buffer, 1)?;
        self.call(L::CopyText, partial, a.name, 0)?;
        self.call(L::TerminateText, self.get(L::FindDot, partial)?, 0, 0)?;
        if a.flags != 0 { return self.call(L::CreateCall, a.outer, partial, 0).map(Some); }
        let mut outer = self.call(L::FindPackage, a.outer, partial, 0)?;
        if outer == 0 { outer = self.call(L::FindAny, a.outer, partial, 0)?; }
        Ok((outer != 0).then_some(outer))
    }
    fn publish_outer(&mut self, outer: usize) { self.update(|a| a.outer = outer); }
    fn advance_name(&mut self) -> Result<()> {
        let dot = self.get(L::FindDot, self.a().name)?;
        let name = self.call(L::AdvanceText, dot, 1, 0)?;
        self.update(|a| a.name = name);
        Ok(())
    }
}
/// # Safety
/// All pointers/handles and callback implementations must remain valid for the
/// synchronous call and recursive native scopes. No callback unwinds into Rust.
/// A supplied canonical name provider remains live through the same reentry.
pub unsafe fn execute(
    s: &Hp2PackageServices,
    op: Hp2PackageOperation,
    args: *mut Hp2PackageArgs,
    names: Option<unsafe fn() -> Result<PublishedNames>>,
) -> Result<usize> {
    Flow { s, p: args, names }.run(op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Production sort context is intentionally process-global.
    static SERIAL: Mutex<()> = Mutex::new(());

    struct RemapAllocatorState {
        allocations: HashMap<usize, usize>,
        fail_alloc: bool,
        allocations_remaining: Option<usize>,
        fail_free: bool,
        frees: usize,
        reenter_lookup: bool,
        reentrant_snapshot_len: Option<i32>,
    }
    // Callback contexts retain the allocation's original raw provenance even
    // when a test changes failure controls between calls.
    struct RemapAllocator(*mut RemapAllocatorState);
    impl RemapAllocator {
        fn new() -> Self {
            Self(Box::into_raw(Box::new(RemapAllocatorState {
                allocations: HashMap::new(),
                fail_alloc: false,
                allocations_remaining: None,
                fail_free: false,
                frees: 0,
                reenter_lookup: false,
                reentrant_snapshot_len: None,
            })))
        }
        fn services(&self) -> Hp2ArrayServices {
            Hp2ArrayServices {
                context: self.0.cast(),
                realloc: remap_realloc,
                checked_int_size: remap_size,
                free: remap_free,
            }
        }
    }
    impl std::ops::Deref for RemapAllocator {
        type Target = RemapAllocatorState;
        fn deref(&self) -> &Self::Target {
            // SAFETY: the fixture owns this allocation until Drop.
            unsafe { &*self.0 }
        }
    }
    impl std::ops::DerefMut for RemapAllocator {
        fn deref_mut(&mut self) -> &mut Self::Target {
            // SAFETY: callbacks are synchronous; no borrow spans a callback.
            unsafe { &mut *self.0 }
        }
    }
    impl Drop for RemapAllocator {
        fn drop(&mut self) {
            // SAFETY: every buffer using this context has already been dropped.
            unsafe { drop(Box::from_raw(self.0)); }
        }
    }
    unsafe extern "C" fn remap_realloc(
        context: *mut std::ffi::c_void,
        old: *mut std::ffi::c_void,
        bytes: usize,
        _max: *const i32,
        _width: i32,
        _tag: *const std::ffi::c_void,
        out: *mut *mut std::ffi::c_void,
    ) -> i32 {
        let state = context.cast::<RemapAllocatorState>();
        if unsafe { (*state).reenter_lookup } {
            unsafe { (*state).reenter_lookup = false };
            let owner = unsafe { (*(&raw const PACKAGE_REMAPS)).as_ref().unwrap() as *const RemapBuffer<NameRemap> };
            // Reject premature publication without reading uninitialized memory.
            if unsafe { (*owner).len != 0 && (*owner).data.is_null() } {
                return 92;
            }
            match unsafe { remap_snapshot(owner, 4) } {
                Ok(mut snapshot) => {
                    unsafe { (*state).reentrant_snapshot_len = Some(snapshot.len) };
                    if let Err(status) = unsafe { snapshot.close() } {
                        return status;
                    }
                }
                Err(status) => return status,
            }
        }
        let allocator = unsafe { &mut *context.cast::<RemapAllocatorState>() };
        if allocator.fail_alloc || allocator.allocations_remaining == Some(0) {
            return 17;
        }
        let replacement = unsafe { std::alloc::alloc(std::alloc::Layout::from_size_align(bytes, 8).unwrap()) };
        if replacement.is_null() {
            return 17;
        }
        if let Some(remaining) = &mut allocator.allocations_remaining {
            *remaining -= 1;
        }
        if !old.is_null() {
            let old_bytes = allocator.allocations.remove(&old.addr()).unwrap();
            unsafe {
                ptr::copy_nonoverlapping(old.cast::<u8>(), replacement, old_bytes.min(bytes));
                std::alloc::dealloc(old.cast(), std::alloc::Layout::from_size_align(old_bytes, 8).unwrap());
            }
        }
        allocator.allocations.insert(replacement.addr(), bytes);
        unsafe { *out = replacement.cast() };
        0
    }
    unsafe extern "C" fn remap_size(
        _context: *mut std::ffi::c_void,
        bytes: usize,
        out: *mut i32,
    ) -> i32 {
        match i32::try_from(bytes) {
            Ok(bytes) => {
                unsafe { *out = bytes };
                0
            }
            Err(_) => 18,
        }
    }
    unsafe extern "C" fn remap_free(
        context: *mut std::ffi::c_void,
        data: *mut std::ffi::c_void,
    ) -> i32 {
        let allocator = unsafe { &mut *context.cast::<RemapAllocatorState>() };
        allocator.frees += 1;
        let bytes = allocator.allocations.remove(&data.addr()).unwrap();
        unsafe { std::alloc::dealloc(data.cast(), std::alloc::Layout::from_size_align(bytes, 8).unwrap()) };
        if allocator.fail_free { 19 } else { 0 }
    }
    #[test]
    fn initial_remap_allocation_reentry_observes_only_committed_pairs() {
        let _serial = SERIAL.lock().unwrap();
        let mut allocator = RemapAllocator::new();
        let arrays = allocator.services();
        allocator.reenter_lookup = true;
        unsafe { (&raw mut PACKAGE_REMAPS).write(Some(RemapBuffer::new(arrays))) };
        let owner = unsafe { (*(&raw mut PACKAGE_REMAPS)).as_mut().unwrap() as *mut RemapBuffer<NameRemap> };
        unsafe { RemapBuffer::push(owner, NameRemap { from: 4, to: 7 }) }.unwrap();
        assert_eq!(allocator.reentrant_snapshot_len, Some(0));
        let mut snapshot = unsafe { remap_snapshot(owner, 4) }.unwrap();
        assert_eq!(snapshot.len, 1);
        assert_eq!(unsafe { snapshot.at(0) }, 7);
        unsafe { snapshot.close() }.unwrap();
        unsafe { release_remaps() }.unwrap();
        assert!(allocator.allocations.is_empty());
    }
    #[test]
    fn remap_snapshot_preserves_duplicates_and_newest_first_order_after_growth() {
        let allocator = RemapAllocator::new();
        let mut remaps = RemapBuffer::new(allocator.services());
        for value in 0..100 {
            unsafe { RemapBuffer::push(&raw mut remaps, NameRemap { from: value % 3, to: value }) }.unwrap();
        }
        let mut snapshot = unsafe { remap_snapshot(&raw const remaps, 1) }.unwrap();
        assert_eq!(
            (0..snapshot.len).map(|i| unsafe { snapshot.at(i) }).collect::<Vec<_>>(),
            (0..100).rev().filter(|value| value % 3 == 1).collect::<Vec<_>>()
        );
        unsafe { remaps.close() }.unwrap();
        let mut replacement = RemapBuffer::new(allocator.services());
        unsafe { RemapBuffer::push(&raw mut replacement, NameRemap { from: 1, to: 999 }) }.unwrap();
        assert_eq!(unsafe { snapshot.at(0) }, 97);
        unsafe { snapshot.close() }.unwrap();
        unsafe { replacement.close() }.unwrap();
        assert!(allocator.allocations.is_empty());
    }
    #[test]
    fn remap_allocation_failure_keeps_published_pairs_and_cleans_snapshot() {
        let mut allocator = RemapAllocator::new();
        let mut remaps = RemapBuffer::new(allocator.services());
        unsafe { RemapBuffer::push(&raw mut remaps, NameRemap { from: 4, to: 7 }) }.unwrap();
        allocator.fail_alloc = true;
        assert!(matches!(unsafe { remap_snapshot(&raw const remaps, 4) }, Err(17)));
        assert_eq!(remaps.len, 1);
        assert_eq!(unsafe { remaps.at(0) }.to, 7);
        allocator.fail_alloc = false;
        let mut snapshot = unsafe { remap_snapshot(&raw const remaps, 99) }.unwrap();
        assert_eq!(snapshot.len, 0);
        unsafe { snapshot.close() }.unwrap();
        unsafe { remaps.close() }.unwrap();
        assert!(allocator.allocations.is_empty());
    }
    #[test]
    fn partial_remap_snapshot_failure_releases_storage_without_replacing_allocation_error() {
        let mut allocator = RemapAllocator::new();
        let mut remaps = RemapBuffer::new(allocator.services());
        for to in 0..100 {
            unsafe { RemapBuffer::push(&raw mut remaps, NameRemap { from: 4, to }) }.unwrap();
        }
        allocator.allocations_remaining = Some(1);
        allocator.fail_free = true;
        assert!(matches!(unsafe { remap_snapshot(&raw const remaps, 4) }, Err(17)));
        assert_eq!(allocator.frees, 1);
        assert_eq!(allocator.allocations.len(), 1);
        assert_eq!(remaps.len, 100);
        allocator.fail_free = false;
        unsafe { remaps.close() }.unwrap();
        assert!(allocator.allocations.is_empty());
    }
    #[test]
    fn remap_cleanup_keeps_the_first_error_and_does_not_retry_a_failed_free() {
        let mut allocator = RemapAllocator::new();
        let mut snapshot = RemapBuffer::new(allocator.services());
        unsafe { RemapBuffer::push(&raw mut snapshot, 42i32) }.unwrap();
        allocator.fail_free = true;
        assert_eq!(unsafe { snapshot.finish::<()>(Err(23)) }, Err(23));
        drop(snapshot);
        assert_eq!(allocator.frees, 1);
        assert!(allocator.allocations.is_empty());
    }
    unsafe extern "C" fn failed_load_call(
        p: *mut Hp2PackageArgs,
        leaf: L,
        _a: usize,
        _b: usize,
        _c: usize,
        out: *mut usize,
    ) -> i32 {
        let trace = unsafe { &mut *((*p).native as *mut Vec<L>) };
        trace.push(leaf);
        unsafe { *out = 0 };
        match leaf {
            L::BeginCall | L::EndCall | L::SafeCaught => 0,
            L::ResolveCall => 7,
            L::CatchConst => {
                unsafe { *out = 1 };
                0
            }
            _ => 90,
        }
    }
    #[test]
    fn failed_load_skips_remaps_for_no_remap_explicit_filename_and_missing_outer() {
        for (outer, filename, flags) in [(9, 0, 4), (9, 10, 0), (0, 0, 0)] {
            let mut trace = Vec::<L>::new();
            let mut args = Hp2PackageArgs {
                native: &mut trace as *mut Vec<L> as usize,
                outer,
                filename,
                flags,
                ..Default::default()
            };
            let services = Hp2PackageServices {
                call: failed_load_call,
                constants: Hp2PackageConstants {
                    no_remap: 4,
                    ..Default::default()
                },
            };
            assert_eq!(unsafe { execute(&services, O::LoadObject, &mut args, None) }, Ok(0));
            assert_eq!(trace, [L::BeginCall, L::ResolveCall, L::CatchConst, L::EndCall, L::SafeCaught]);
        }
    }
    #[derive(Default)]
    struct Model {
        trace: Vec<(L, usize, usize, usize)>,
        count: i32,
        loaded: Vec<usize>,
        preloaded: Vec<usize>,
        postloaded: Vec<usize>,
        grow: bool,
        fail_scope: Option<usize>,
        text_error: bool,
        move_ok: bool,
        fail_new: bool,
        /// Length every `TextLength` probe reports.
        text_len: usize,
        names: Vec<usize>,
        conform_names: Vec<usize>,
        exports: Vec<usize>,
        original: Vec<usize>,
        conform_exports: Vec<usize>,
        used: Vec<bool>,
        mapping: HashMap<usize, usize>,
        usage: HashMap<usize, usize>,
        sorted_from: usize,
        offsets: HashMap<usize, usize>,
        object_flags: HashMap<usize, usize>,
        references: HashMap<usize, Vec<usize>>,
        serialized: Vec<usize>,
        imports: Vec<usize>,
        global_imports: usize,
        error: Option<String>,
        /// Per-object linker handle (default 100) and per-linker object index.
        linker_of: HashMap<usize, usize>,
        linker_index: HashMap<usize, usize>,
    }
    fn services() -> Hp2PackageServices {
        Hp2PackageServices {
            call: callback,
            constants: Hp2PackageConstants {
                need_load: 1,
                native: 2,
                public_flag: 4,
                transient: 8,
                tag_exp: 16,
                tag_imp: 32,
                load_edit: 64,
                load_client: 128,
                load_server: 256,
                not_edit: 512,
                not_client: 1024,
                not_server: 2048,
                quiet: 1,
                throw_flag: 2,
                no_fail: 4,
                no_warn: 8,
                text_capacity: 256,
                ..Default::default()
            },
        }
    }
    fn args(m: &mut Model) -> Hp2PackageArgs {
        Hp2PackageArgs {
            native: m as *mut Model as usize,
            outer: 10,
            filename: 20,
            linker: 100,
            conform: 200,
            ..Default::default()
        }
    }
    unsafe extern "C" fn callback(
        p: *mut Hp2PackageArgs,
        leaf: L,
        a: usize,
        b: usize,
        c: usize,
        out: *mut usize,
    ) -> i32 {
        // Test callbacks also must never unwind through an extern-C frame.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            dispatch(p, leaf, a, b, c)
        }));
        match result {
            Ok(Ok(value)) => {
                unsafe { *out = value };
                0
            }
            Ok(Err(status)) => status,
            Err(_) => 91,
        }
    }
    unsafe fn dispatch(
        p: *mut Hp2PackageArgs,
        leaf: L,
        a: usize,
        b: usize,
        c: usize,
    ) -> Result<usize> {
        let q = unsafe { *p };
        let m = unsafe { &mut *(q.native as *mut Model) };
        m.trace.push((leaf, a, b, c));
        match leaf {
            L::BeginCount => Ok(m.count as usize),
            L::SetBeginCount => {
                m.count = a as i32;
                Ok(0)
            }
            L::LoadedCount => Ok(m.loaded.len()),
            L::LoadedAt => Ok(m.loaded[a]),
            L::LoadedRelease => {
                m.loaded.clear();
                Ok(0)
            }
            L::DebugLoading | L::FlushLocalized => Ok(0),
            L::SetImportCount => {
                m.global_imports = a;
                Ok(0)
            }
            L::ImportCount => Ok(m.global_imports),
            L::LoaderCount => Ok((!m.imports.is_empty()) as usize),
            L::LoaderAt => Ok(100),
            L::Flags => Ok(m
                .object_flags
                .get(&a)
                .copied()
                .unwrap_or((!m.preloaded.contains(&a)) as usize)),
            L::Linker => Ok(m.linker_of.get(&a).copied().unwrap_or(100)),
            L::ObjectIndex => Ok(m.linker_index.get(&a).copied().unwrap_or(a)),
            L::LinkerIndex => Ok(a),
            L::SerialOffset => Ok(m.offsets.get(&b).copied().unwrap_or(b * 10)),
            L::SetFlags => {
                *m.object_flags.entry(a).or_default() |= b;
                Ok(0)
            }
            L::PendingKill => Ok(0),
            L::IsIn => Ok((a != 2) as usize),
            L::Class => Ok(2),
            L::Outer => Ok(if a == 3 { 0 } else { 3 }),
            L::TagExportCall => {
                let mut nested = q;
                nested.base = b;
                unsafe { execute(&services(), O::TagExport, &mut nested, None) }
            }
            L::Serialize => {
                m.serialized.push(a);
                let references = m.references.get(&a).cloned().unwrap_or_default();
                for object in references {
                    let mut nested = q;
                    nested.base = object;
                    unsafe { execute(&services(), O::TagExport, &mut nested, None) }?;
                }
                Ok(0)
            }
            L::Preload => {
                m.preloaded.push(b);
                if m.grow && b == 1 {
                    m.loaded.push(3);
                }
                Ok(0)
            }
            L::Postload => {
                m.postloaded.push(a);
                if m.grow && a == 1 {
                    m.loaded.push(4);
                }
                Ok(0)
            }
            L::LoadedSort => {
                let pointer = unsafe { m.loaded.as_mut_ptr().add(a) };
                let count = m.loaded.len() - a;
                let mut nested = q;
                nested.outer = pointer as usize;
                nested.base = count;
                unsafe { execute(&services(), O::SortLoaded, &mut nested, None) }
            }
            L::Scope => {
                if m.fail_scope == Some(a) {
                    return Err(7);
                }
                // Save phases are separate native guard/lifetime boundaries.
                // EndLoad's scopes execute recursively against the live model.
                let op = if a == O::Preload as usize {
                    Some(O::Preload)
                } else if a == O::Postload as usize {
                    Some(O::Postload)
                } else if a == O::Dissociate as usize {
                    Some(O::Dissociate)
                } else {
                    None
                };
                if let Some(op) = op {
                    unsafe { execute(&services(), op, p, None) }
                } else {
                    Ok(0)
                }
            }
            L::CatchConst => Ok(m.text_error as usize),
            L::FatalCaught => Err(8),
            L::Check | L::CheckSlow => {
                if a != 0 {
                    Ok(0)
                } else {
                    Err(9)
                }
            }
            L::StartSaveTime
            | L::CopyText
            | L::TerminateText
            | L::SavingStatus
            | L::ResetCall
            | L::ExportArchiveScope
            | L::DeleteTemp
            | L::SaveErrorLog
            | L::DeleteSaver
            | L::SaveTimeLog
            | L::MovingLog
            | L::SaveWarning => Ok(0),
            L::IsPackage => Ok(0),
            L::TextLength => Ok(m.text_len),
            L::Buffer => Ok(30 + a),
            L::AdvanceText => Ok(a + b),
            L::NewSaver => {
                if m.fail_new {
                    Err(7)
                } else {
                    Ok(100)
                }
            }
            L::MoveTemp => Ok(m.move_ok as usize),
            L::NameMapCount => Ok(if a == 200 {
                m.conform_names.len()
            } else {
                m.names.len()
            }),
            L::NameMapAt => Ok(if a == 200 {
                m.conform_names[b]
            } else {
                m.names[b]
            }),
            L::FindName => Ok(m.names.iter().position(|n| *n == b).unwrap_or(usize::MAX)),
            L::AppendNameSlot => {
                m.names.push(0);
                Ok(0)
            }
            L::SetNameMap => {
                m.names[a] = b;
                Ok(0)
            }
            L::SwapNames => {
                m.names.swap(a, b);
                Ok(0)
            }
            L::SortNameMap | L::SortExportMap => {
                m.sorted_from = a;
                Ok(0)
            }
            L::AddEmptyExport => {
                m.exports.push(0);
                Ok(0)
            }
            L::ExportCount => Ok(if a == 200 {
                m.conform_exports.len()
            } else {
                m.exports.len()
            }),
            L::ExportObject => Ok(m.exports[b]),
            L::SetExportCount => Ok(0),
            L::ImportCountOf => Ok(m.imports.len()),
            L::ImportObject => Ok(m.imports[b]),
            L::SetImportObject => {
                m.imports[b] = c;
                Ok(0)
            }
            L::SetObjectUsage => {
                m.usage.insert(b, c);
                Ok(0)
            }
            L::ObjectUsage | L::NameUsage => Ok(m.usage.get(&b).copied().unwrap_or(0)),
            L::SortImportMap => Ok(0),
            L::ConformLog
            | L::CopyGuid
            | L::CopyGenerations
            | L::NewGuid
            | L::ClearGenerations
            | L::AddGeneration
            | L::SerializeSummary => Ok(0),
            L::BeginCall => {
                let mut nested = q;
                unsafe { execute(&services(), O::Begin, &mut nested, None) }
            }
            L::EndCall => {
                let mut nested = q;
                unsafe { execute(&services(), O::End, &mut nested, None) }
            }
            L::GetLinkerCall => Err(7),
            L::SafeCaught => Ok(0),
            L::Strict => Ok(0),
            L::WarnError | L::LogError => Ok(0),
            L::ThrowError => Err(7),
            L::FatalError => Err(8),
            _ => {
                m.error = Some(format!("unexpected {leaf:?}({a},{b},{c})"));
                Err(90)
            }
        }
    }
    fn run(m: &mut Model, op: O, a: &mut Hp2PackageArgs) -> Result<usize> {
        a.native = m as *mut Model as usize;
        let result = unsafe { execute(&services(), op, a, None) };
        assert_eq!(m.error, None);
        result
    }
    fn cleanup(m: &Model) -> Vec<L> {
        m.trace
            .iter()
            .map(|e| e.0)
            .filter(|l| {
                matches!(
                    l,
                    L::CatchConst
                        | L::DeleteTemp
                        | L::SaveErrorLog
                        | L::DeleteSaver
                        | L::SaveTimeLog
                        | L::MovingLog
                        | L::MoveTemp
                        | L::SaveWarning
                )
            })
            .collect()
    }
    #[test]
    fn nested_end_defers_and_live_preload_postload_collections_grow() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            loaded: vec![1, 2],
            grow: true,
            ..Default::default()
        };
        let mut a = args(&mut m);
        assert_eq!(run(&mut m, O::Begin, &mut a), Ok(0));
        assert_eq!(run(&mut m, O::Begin, &mut a), Ok(0));
        assert_eq!(run(&mut m, O::End, &mut a), Ok(0));
        assert!(m.preloaded.is_empty());
        assert_eq!(m.count, 1);
        assert_eq!(run(&mut m, O::End, &mut a), Ok(0));
        assert_eq!(m.preloaded, vec![1, 2, 3]);
        assert_eq!(m.postloaded, vec![1, 2, 3, 4]);
        assert!(m.loaded.is_empty());
        assert_eq!(m.count, 0);
        assert!(
            !m.trace
                .iter()
                .any(|e| matches!(e.0, L::Check | L::CheckSlow | L::AutoRegister | L::Success))
        );
    }
    #[test]
    fn const_text_save_failure_deletes_before_log_and_linker_destruction() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            fail_scope: Some(O::SaveExports as usize),
            text_error: true,
            ..Default::default()
        };
        let mut a = args(&mut m);
        a.linker = 0;
        assert_eq!(run(&mut m, O::Save, &mut a), Ok(0));
        assert_eq!(
            cleanup(&m),
            vec![
                L::CatchConst,
                L::DeleteTemp,
                L::SaveErrorLog,
                L::DeleteSaver,
                L::SaveTimeLog
            ]
        );
        let mut m = Model {
            fail_new: true,
            text_error: true,
            ..Default::default()
        };
        let mut a = args(&mut m);
        a.linker = 0;
        assert_eq!(run(&mut m, O::Save, &mut a), Ok(0));
        assert_eq!(
            cleanup(&m),
            vec![
                L::CatchConst,
                L::DeleteTemp,
                L::SaveErrorLog,
                L::SaveTimeLog
            ]
        );
    }
    #[test]
    fn non_text_errors_escape_without_running_the_const_text_cleanup() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            fail_scope: Some(O::SaveExports as usize),
            ..Default::default()
        };
        let mut a = args(&mut m);
        a.linker = 0;
        assert_eq!(run(&mut m, O::Save, &mut a), Err(7));
        assert_eq!(cleanup(&m), vec![L::CatchConst]);
        let mut m = Model::default();
        let mut a = args(&mut m);
        assert_eq!(run(&mut m, O::LoadPackage, &mut a), Err(7));
        assert_eq!(m.count, 1); // no C++ catch, hence no compensating EndLoad
    }
    #[test]
    fn failed_move_deletes_only_after_successful_save_closed_the_linker() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model::default();
        let mut a = args(&mut m);
        a.linker = 0;
        assert_eq!(run(&mut m, O::Save, &mut a), Ok(0));
        assert_eq!(
            cleanup(&m),
            vec![
                L::DeleteSaver,
                L::SaveTimeLog,
                L::MovingLog,
                L::MoveTemp,
                L::DeleteTemp,
                L::SaveWarning
            ]
        );
    }
    #[test]
    fn filenames_that_do_not_fit_the_save_buffer_warn_before_any_save_work() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            text_len: 256,
            ..Default::default()
        };
        let mut a = args(&mut m);
        assert_eq!(run(&mut m, O::Save, &mut a), Ok(0));
        let leaves: Vec<L> = m.trace.iter().map(|e| e.0).collect();
        assert_eq!(leaves, vec![L::TextLength, L::SaveWarning]);
    }
    #[test]
    fn caught_load_failure_balances_transaction_before_reporting() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            text_error: true,
            ..Default::default()
        };
        let mut a = args(&mut m);
        assert_eq!(run(&mut m, O::LoadPackage, &mut a), Ok(0));
        assert_eq!(m.count, 0);
        let calls: Vec<_> = m.trace.iter().map(|e| e.0).collect();
        assert!(
            calls.iter().position(|l| *l == L::EndCall).unwrap()
                < calls.iter().position(|l| *l == L::SafeCaught).unwrap()
        );
    }
    #[test]
    fn conform_names_retain_none_and_missing_slots_and_sort_only_the_suffix() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            names: vec![11, 22, 33],
            conform_names: vec![0, 22, 99],
            ..Default::default()
        };
        let mut a = args(&mut m);
        assert_eq!(run(&mut m, O::SortNames, &mut a), Ok(0));
        assert_eq!(m.names, vec![0, 22, 99, 11, 33]);
        assert_eq!(m.sorted_from, 3);
    }
    #[test]
    fn conform_exports_preserve_holes_duplicates_and_reverse_indices() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            exports: vec![10, 20, 30],
            conform_exports: vec![20, 99, 20],
            ..Default::default()
        };
        let mut a = args(&mut m);
        m.original = std::mem::take(&mut m.exports);
        m.used.resize(m.original.len(), false);
        let first = package_save::conform_exports_with(
            &mut m, true, true,
            |m| Ok(m.original.len()),
            |m, index| { m.mapping.insert(m.original[index], index); Ok(()) },
            |m| Ok(m.conform_exports.len()),
            |m, index| Ok(m.mapping.get(&m.conform_exports[index]).copied()),
            |m, index| { m.exports.push(m.original[index]); Ok(()) },
            |m, index| Ok(m.exports.last() == Some(&m.original[index])),
            |m, index| { m.used[index] = true; Ok(()) },
            |m| { m.exports.push(0); Ok(()) },
            |m, index| Ok(m.used[index]),
            |m, index| Ok(m.exports.contains(&m.original[index])),
            |_, valid| if valid { Ok(()) } else { Err(90) },
        ).expect("conform correspondence");
        assert_eq!(m.exports, vec![20, 0, 20, 10, 30]);
        assert_eq!(first, 3);
        assert_eq!(run(&mut m, O::Mappings, &mut a), Ok(0));
        assert_eq!(m.usage.get(&20), Some(&3));
        assert_eq!(m.usage.get(&10), Some(&4));
        assert_eq!(m.usage.get(&30), Some(&5));
        assert!(!m.usage.contains_key(&0));
    }
    #[test]
    fn loaded_sort_preserves_legacy_equal_key_selection_swaps() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model::default();
        CURRENT.store(1, Ordering::Relaxed);
        // Repeated handles have equal offsets. This also exercises the large
        // partition path and the fixed-stack small-partition descent.
        let mut objects = vec![9usize, 2, 8, 3, 7, 4, 6, 5, 2, 9, 1, 8];
        let mut a = args(&mut m);
        a.outer = objects.as_mut_ptr() as usize;
        a.base = objects.len();
        assert_eq!(run(&mut m, O::SortLoaded, &mut a), Ok(0));
        assert_eq!(objects, vec![1, 2, 2, 3, 4, 5, 6, 7, 8, 8, 9, 9]);
        m.offsets.extend([(1, 10), (2, 10), (3, 10), (4, 10)]);
        let mut equal = [1usize, 2, 3, 4];
        a.outer = equal.as_mut_ptr() as usize;
        a.base = equal.len();
        assert_eq!(run(&mut m, O::SortLoaded, &mut a), Ok(0));
        assert_eq!(equal, [2, 3, 4, 1]);
        let mut a = args(&mut m);
        m.usage.insert(11, i32::MIN as usize);
        m.usage.insert(22, 1);
        TEMP_SAVE.store(100, Ordering::Relaxed);
        a.outer = 11;
        a.base = 22;
        assert_eq!(
            run(&mut m, O::CompareName, &mut a),
            Ok(1i32.wrapping_sub(i32::MIN) as usize)
        );
    }
    #[test]
    fn loaded_sort_orders_other_linkers_by_object_index_not_handle() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model::default();
        CURRENT.store(50, Ordering::Relaxed);
        // Handles (heap addresses in production) run opposite to linker indices.
        m.linker_of
            .extend([(1, 400), (2, 300), (3, 200), (4, 500), (50, 500)]);
        m.linker_index
            .extend([(400, 7), (300, 5), (200, 9), (500, 1)]);
        let mut objects = [1usize, 2, 3, 4];
        let mut a = args(&mut m);
        a.outer = objects.as_mut_ptr() as usize;
        a.base = objects.len();
        assert_eq!(run(&mut m, O::SortLoaded, &mut a), Ok(0));
        // Current linker first, then linker indices 5, 7, 9.
        assert_eq!(objects, [4, 2, 1, 3]);
    }
    #[test]
    fn safe_error_throw_prevents_fatal_and_warning_actions() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model::default();
        let mut a = args(&mut m);
        a.flags = 2 | 4;
        assert_eq!(run(&mut m, O::SafeError, &mut a), Err(7));
        assert_eq!(
            m.trace.iter().map(|e| e.0).collect::<Vec<_>>(),
            vec![L::Strict, L::WarnError, L::ThrowError]
        );
    }
    #[test]
    fn export_tags_break_cycles_before_class_outer_and_child_serialization() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model::default();
        m.object_flags.extend([(1, 0), (3, 0), (4, 0)]);
        m.references.insert(1, vec![4, 1]);
        m.references.insert(4, vec![1]);
        let mut a = args(&mut m);
        a.base = 1;
        a.archive = 1;
        assert_eq!(run(&mut m, O::TagExport, &mut a), Ok(0));
        assert_eq!(m.serialized, vec![3, 1, 4]);
        for object in [1, 3, 4] {
            assert_eq!(m.object_flags[&object], 16 | 64 | 128 | 256);
        }
    }
    #[test]
    fn dissociation_retains_native_imports_but_clears_dynamic_references() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            imports: vec![11, 12, 0],
            global_imports: 2,
            ..Default::default()
        };
        m.object_flags.extend([(11, 2), (12, 0)]);
        let mut a = args(&mut m);
        assert_eq!(run(&mut m, O::Dissociate, &mut a), Ok(0));
        assert_eq!(m.imports, vec![11, 0, 0]);
        assert_eq!(m.global_imports, 0);
    }
    #[test]
    fn end_load_text_errors_become_fatal_without_postload_or_release() {
        let _serial = SERIAL.lock().unwrap();
        let mut m = Model {
            count: 1,
            loaded: vec![1],
            fail_scope: Some(O::Preload as usize),
            text_error: true,
            ..Default::default()
        };
        let mut a = args(&mut m);
        assert_eq!(run(&mut m, O::End, &mut a), Err(8));
        assert_eq!(m.count, 0);
        assert_eq!(m.loaded, vec![1]);
        assert!(
            !m.trace
                .iter()
                .any(|e| matches!(e.0, L::Postload | L::LoadedRelease | L::FlushLocalized))
        );
        assert_eq!(m.trace.last().map(|e| e.0), Some(L::FatalCaught));
    }
    struct ConfigTextFixture {
        scratch: [u16; 1024],
        resolved: Vec<u16>,
        section_len: usize,
        key: Vec<u16>,
    }
    unsafe fn text_len(text: usize) -> usize {
        let mut length = 0;
        while unsafe { *((text as *const u16).add(length)) } != 0 {
            length += 1;
        }
        length
    }
    unsafe extern "C" fn config_text_call(
        p: *mut Hp2PackageArgs,
        leaf: L,
        a: usize,
        b: usize,
        _c: usize,
        out: *mut usize,
    ) -> i32 {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<usize> {
            let fixture = unsafe { &mut *((*p).native as *mut ConfigTextFixture) };
            match leaf {
                L::IniPrefix => Ok(1),
                L::UserPrefix => Ok(0),
                L::TextLength => Ok(unsafe { text_len(a) }),
                L::FindDot => {
                    let length = unsafe { text_len(a) };
                    let text = unsafe { std::slice::from_raw_parts(a as *const u16, length) };
                    Ok(text
                        .iter()
                        .position(|v| *v == b'.' as u16)
                        .map_or(0, |i| unsafe { (a as *const u16).add(i) } as usize))
                }
                // Reject the 256-character filename/partial buffer selector.
                L::Buffer if a == 3 => Ok(fixture.scratch.as_mut_ptr() as usize),
                L::AdvanceText => Ok(unsafe { (a as *const u16).offset(b as isize) } as usize),
                L::CopyText => {
                    let count = unsafe { text_len(b) } + 1;
                    if a != fixture.scratch.as_mut_ptr() as usize || count > fixture.scratch.len() {
                        return Err(90);
                    }
                    unsafe { ptr::copy_nonoverlapping(b as *const u16, a as *mut u16, count) };
                    Ok(0)
                }
                L::TerminateText => {
                    unsafe { *(a as *mut u16).offset(b as isize) = 0 };
                    Ok(0)
                }
                L::ConfigGet => {
                    fixture.section_len = unsafe { text_len(a) };
                    fixture.key =
                        unsafe { std::slice::from_raw_parts(b as *const u16, text_len(b)) }
                            .to_vec();
                    Ok(fixture.resolved.as_ptr() as usize)
                }
                _ => Err(90),
            }
        }));
        match result {
            Ok(Ok(value)) => {
                unsafe { *out = value };
                0
            }
            Ok(Err(status)) => status,
            Err(_) => 91,
        }
    }
    #[test]
    fn config_names_over_256_characters_use_the_1024_character_scratch() {
        let mut fixture = ConfigTextFixture {
            scratch: [0; 1024],
            resolved: vec![82, 101, 115, 111, 108, 118, 101, 100, 0],
            section_len: 0,
            key: Vec::new(),
        };
        let input: Vec<u16> = format!("ini:{}.Key", "S".repeat(900))
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut a = Hp2PackageArgs {
            native: &mut fixture as *mut ConfigTextFixture as usize,
            name: input.as_ptr() as usize,
            ..Default::default()
        };
        let s = Hp2PackageServices {
            call: config_text_call,
            constants: Hp2PackageConstants {
                text_capacity: 256,
                ..Default::default()
            },
        };
        assert_eq!(unsafe { execute(&s, O::Resolve, &mut a, None) }, Ok(1));
        assert_eq!(fixture.section_len, 900);
        assert_eq!(fixture.key, vec![75, 101, 121]);
        assert_eq!(a.name, fixture.resolved.as_ptr() as usize);
    }
    #[test]
    fn names_that_do_not_fit_the_partial_buffer_are_unresolved_without_copying() {
        let resolved: Vec<u16> = format!("{}.Obj", "P".repeat(300))
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut fixture = ConfigTextFixture {
            scratch: [0; 1024],
            resolved,
            section_len: 0,
            key: Vec::new(),
        };
        let input: Vec<u16> = "ini:S.Key"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut a = Hp2PackageArgs {
            native: &mut fixture as *mut ConfigTextFixture as usize,
            name: input.as_ptr() as usize,
            ..Default::default()
        };
        let s = Hp2PackageServices {
            call: config_text_call,
            constants: Hp2PackageConstants {
                text_capacity: 256,
                ..Default::default()
            },
        };
        // The fixture fails any Buffer(1)/CopyText into the 256-unit buffer.
        assert_eq!(unsafe { execute(&s, O::Resolve, &mut a, None) }, Ok(0));
        assert_eq!(a.name, fixture.resolved.as_ptr() as usize);
    }
}
