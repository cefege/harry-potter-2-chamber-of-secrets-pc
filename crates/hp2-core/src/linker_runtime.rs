//! Linker runtime policy. Native services expose individual typed fields and
//! engine leaves, never references into maps. Every callback may recursively
//! enter this module; snapshots are scalar and no mutable borrow spans a call.
use std::ffi::c_void;

type Result<T, E = i32> = std::result::Result<T, E>;
type Handle = usize;

pub(crate) fn load_context_flags(
    editor: bool,
    client: bool,
    server: bool,
    load_edit: u32,
    load_client: u32,
    load_server: u32,
) -> u32 {
    let mut flags = 0;
    if editor { flags |= load_edit; }
    if client { flags |= load_client; }
    if server { flags |= load_server; }
    flags
}

pub(crate) fn export_matches_context(object_flags: u32, context_flags: u32) -> bool {
    object_flags & context_flags != 0
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2LinkerField {
    Root,
    Context,
    Success,
    ImportCount,
    ExportCount,
    ImportPackage,
    ImportClassPackage,
    ImportClassName,
    ImportName,
    ImportObject,
    ImportSource,
    ImportSourceIndex,
    ExportClass,
    ExportSuper,
    ExportPackage,
    ExportName,
    ExportFlags,
    ExportSize,
    ExportOffset,
    ExportObject,
    ExportNext,
    LoadFlags,
    Verified,
    Hash,
    Loader,
    AltLoader,
    Saver,
    SummaryTag,
    SummaryVersion,
    SummaryLicensee,
    SummaryFlags,
    SummaryNames,
    SummaryNameOffset,
    SummaryImports,
    SummaryImportOffset,
    SummaryExports,
    SummaryExportOffset,
    ObjectName,
    ObjectOuter,
    ObjectClass,
    ObjectFlags,
    ObjectLinker,
    ObjectLinkerIndex,
    ObjectSuper,
    ObjectValid,
    ObjectIsStruct,
    ObjectIsClass,
    ClassFlags,
    PackageFlags,
    IsPackage,
    LazyCount,
    LazyAt,
    LazyArchive,
    LazyPosition,
    Editor,
    Client,
    Server,
    LoaderCount,
    LoaderAt,
    CheckConflicts,
    ArchiveVersion,
    ArchiveLicensee,
    ArchiveLoading,
    ArchiveSaving,
    ArchivePersistent,
    ArchiveEdit,
    ArchiveClient,
    ArchiveServer,
    SaveArchivePersistent,
    SaveArchiveEdit,
    SaveArchiveClient,
    SaveArchiveServer,
    ObjectCount,
    NameCount,
    NameIndex,
    ObjectIndex,
    Trace,
    TraceCaptured,
    TraceIndex,
    TraceOffset,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2LinkerAction {
    Check,
    CheckSlow,
    OpenReader,
    OpenWriter,
    OpenFailed,
    LinkerExists,
    LoadingStatus,
    ReadSummary,
    BinaryFormat,
    Aborted,
    OldVersion,
    EmptyImports,
    EmptyExports,
    EmptyNames,
    ReadName,
    AddName,
    ReadImport,
    ReadExport,
    AddImport,
    AddExport,
    LoadObjectIndex,
    SaveObjectIndexArchive,
    NameEntryFlags,
    NameEntryName,
    LoadScope,
    InitArchiveScope,
    SummaryScope,
    CheckTagScope,
    CheckVersionScope,
    NamesScope,
    ImportsScope,
    ExportsScope,
    Register,
    Unregister,
    VerifyCall,
    VerifyImportCall,
    VerifyImportsScope,
    ExceptionMutableText,
    ExceptionConstText,
    RethrowAsText,
    ClearException,
    CreatePackage,
    PackageLinker,
    IsMesh,
    LodMeshName,
    FindPackage,
    FindClass,
    FindObject,
    FindObjectAny,
    BrokenImport,
    FailedImport,
    PrivateImport,
    MissingImport,
    FailedImportLog,
    ImportIncrement,
    ClassStatic,
    Construct,
    SetLinker,
    PreloadCall,
    CreateExportCall,
    CreateImportCall,
    IndexObjectCall,
    FailedCreate,
    PreloadScope,
    PreloadTagScope,
    ExportTagScope,
    ArchiveTell,
    ArchiveSeek,
    ArchivePrecache,
    LinkerTell,
    LinkerSeek,
    SerializeObject,
    SerialSizeError,
    ClearObjectFlags,
    SetObjectFlags,
    AppendLoaded,
    CreatedLog,
    Bind,
    ExportIndexError,
    ImportIndexError,
    InvalidExport,
    MislinkedExport,
    MisindexedExport,
    DetachExportCall,
    DetachAllCall,
    UnloadingLog,
    DeleteArchive,
    SuperDestroy,
    LazyFind,
    LazyAdd,
    LazyRemove,
    LazyEmpty,
    LazyLoad,
    LazyDetachError,
    SuperSerialize,
    CountImports,
    CountExports,
    SerializeNames,
    SerializeRoot,
    SerializeExportName,
    SerializeImportSource,
    SerializeImportPackage,
    SerializeImportClass,
    StringDot,
    StringImportName,
    StringExportName,
    StringImportFinish,
    StringExportFinish,
    SetSaveVersions,
    RootPath,
    AllocateObjectIndices,
    AllocateNameIndices,
    SaveNameIndex,
    SaveObjectIndex,
    LinkerSerialize,
    CountLazy,
    ClassNameCall,
    ClassPackageCall,
    FindExportCall,
}

#[repr(C)]
pub struct Hp2LinkerConstants {
    pub none: i32,
    pub core: i32,
    pub class_name: i32,
    pub package: i32,
    pub unreal_i: i32,
    pub unreal_share: i32,
    pub camera: i32,
    pub load_edit: u32,
    pub load_client: u32,
    pub load_server: u32,
    pub public_flag: u32,
    pub native: u32,
    pub transient: u32,
    pub need_load: u32,
    pub preloading: u32,
    pub need_post_load: u32,
    pub load_mask: u32,
    pub no_verify: u32,
    pub verify: u32,
    pub forgiving: u32,
    pub throw_flag: u32,
    pub propagate: u32,
    pub broken_links: u32,
    pub safe_replace: u32,
    pub file_tag: i32,
    pub version: i32,
    pub licensee: i32,
    pub min_version: i32,
    pub checks: i32,
    pub slow_checks: i32,
}

#[repr(C)]
pub struct Hp2LinkerRuntimeServices {
    pub field: unsafe extern "C" fn(usize, Hp2LinkerField, i32, *mut usize, i32) -> i32,
    pub action:
        unsafe extern "C" fn(Hp2LinkerAction, usize, usize, usize, usize, *mut usize) -> i32,
    pub constants: Hp2LinkerConstants,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2LinkerOperation {
    Context,
    Serialize,
    ImportFullName,
    ExportFullName,
    Load,
    LoadTagged,
    InitArchive,
    Summary,
    CheckTag,
    CheckVersion,
    Names,
    Imports,
    Exports,
    Verify,
    VerifyImports,
    ClassPackage,
    ClassName,
    VerifyImport,
    LoadAll,
    FindExport,
    Create,
    Preload,
    PreloadBody,
    PreloadTagged,
    CreateExport,
    ExportTagged,
    CreateImport,
    IndexObject,
    DetachExport,
    DestroyLoad,
    AttachLazy,
    DetachLazy,
    DetachAll,
    Stream,
    Save,
    DestroySave,
    MapName,
    MapObject,
    LoadObjectArchive,
    SaveObjectArchive,
    SerializeLoad,
    ImportConstructed,
}

pub fn hash_names(a: i32, b: i32, c: i32, unreal_i: i32, unreal_share: i32) -> i32 {
    let c = if c == unreal_share { unreal_i } else { c };
    a.wrapping_add(b.wrapping_mul(7))
        .wrapping_add(c.wrapping_mul(31))
}

fn parent_matches(source_package: i32, parent_source: i32) -> bool {
    source_package == 0 || (parent_source != -1 && parent_source.wrapping_add(1) == source_package)
}

/// Typed operations required to apply the native `UObject::SetLinker` mutation.
pub(crate) trait SetLinkerOps {
    type Error;

    fn current_linker(&mut self, object: usize) -> Result<usize, Self::Error>;
    fn current_index(&mut self, object: usize) -> Result<i32, Self::Error>;
    fn export_object(&mut self, linker: usize, index: i32) -> Result<usize, Self::Error>;
    fn set_export_object(
        &mut self,
        linker: usize,
        index: i32,
        object: usize,
    ) -> Result<(), Self::Error>;
    fn set_object_linker(
        &mut self,
        object: usize,
        linker: usize,
    ) -> Result<(), Self::Error>;
    fn set_object_index(&mut self, object: usize, index: i32) -> Result<(), Self::Error>;
    fn invalid_export_owner(&mut self, object: usize, linker: usize, index: i32) -> Self::Error;
}

/// Applies the native SetLinker detach-then-assign sequence through typed owner operations.
pub(crate) fn set_linker_with<O: SetLinkerOps>(
    ops: &mut O,
    object: usize,
    linker: usize,
    index: i32,
) -> Result<(), O::Error> {
    let previous_linker = ops.current_linker(object)?;
    if previous_linker != 0 {
        let previous_index = ops.current_index(object)?;
        if ops.export_object(previous_linker, previous_index)? != object {
            return Err(ops.invalid_export_owner(object, previous_linker, previous_index));
        }
        ops.set_export_object(previous_linker, previous_index, 0)?;
    }
    ops.set_object_linker(object, linker)?;
    ops.set_object_index(object, index)
}
#[derive(Clone, Copy)]
pub(crate) enum DestroyLoadArchive {
    Loader,
    AltLoader,
}

/// Typed operations for the canonical `ULinkerLoad::Destroy` sequence.
pub(crate) trait DestroyLoadOps {
    type Error;

    fn unloading_log(&mut self, linker: usize) -> Result<(), Self::Error>;
    fn detach_all_lazy(&mut self, linker: usize) -> Result<(), Self::Error>;
    fn export_count(&mut self, linker: usize) -> Result<i32, Self::Error>;
    fn export_object(&mut self, linker: usize, index: i32) -> Result<usize, Self::Error>;
    fn detach_export(&mut self, linker: usize, index: i32) -> Result<(), Self::Error>;
    fn finish_export_detachment(&mut self, _linker: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn unregister(&mut self, linker: usize) -> Result<(), Self::Error>;
    fn archive(&mut self, linker: usize, field: DestroyLoadArchive) -> Result<usize, Self::Error>;
    fn close_archive(&mut self, archive: usize) -> Result<(), Self::Error>;
    fn clear_archive(
        &mut self,
        linker: usize,
        field: DestroyLoadArchive,
    ) -> Result<(), Self::Error>;
    fn super_destroy(&mut self, linker: usize) -> Result<(), Self::Error>;
}


/// Applies `ULinkerLoad::Destroy` in its observable mutation and callback order.
pub(crate) fn destroy_load_with<O: DestroyLoadOps>(
    ops: &mut O,
    linker: usize,
) -> Result<(), O::Error> {
    ops.unloading_log(linker)?;
    ops.detach_all_lazy(linker)?;
    let mut index = 0;
    while index < ops.export_count(linker)? {
        if ops.export_object(linker, index)? != 0 {
            ops.detach_export(linker, index)?;
        }
        index += 1;
    }
    ops.unregister(linker)?;
    for field in [DestroyLoadArchive::Loader, DestroyLoadArchive::AltLoader] {
        let archive = ops.archive(linker, field)?;
        if archive != 0 {
            ops.close_archive(archive)?;
        }
        ops.clear_archive(linker, field)?;
    }
    ops.super_destroy(linker)
}
/// Runs Rust-owned LinkerLoad teardown to completion while preserving its first failure.
///
/// The direct owner must not release its tail while exports or the linker registry
/// can still refer to it, even when logging or another cleanup operation fails.
pub(crate) fn destroy_load_direct_with<O: DestroyLoadOps>(
    ops: &mut O,
    linker: usize,
) -> Result<(), O::Error> {
    let mut first_error = None;
    let remember = |first: &mut Option<O::Error>, result: Result<(), O::Error>| {
        if first.is_none() {
            *first = result.err();
        }
    };

    remember(&mut first_error, ops.unloading_log(linker));
    remember(&mut first_error, ops.detach_all_lazy(linker));
    let export_count = match ops.export_count(linker) {
        Ok(count) => count,
        Err(error) => {
            if first_error.is_none() {
                first_error = Some(error);
            }
            0
        }
    };
    let mut index = 0;
    while index < export_count {
        match ops.export_object(linker, index) {
            Ok(0) => {}
            Ok(_) => remember(&mut first_error, ops.detach_export(linker, index)),
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        index += 1;
    }
    remember(&mut first_error, ops.finish_export_detachment(linker));
    remember(&mut first_error, ops.unregister(linker));


    for field in [DestroyLoadArchive::Loader, DestroyLoadArchive::AltLoader] {
        let archive = match ops.archive(linker, field) {
            Ok(archive) => archive,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
                0
            }
        };
        if archive != 0 {
            remember(&mut first_error, ops.close_archive(archive));
        }
        remember(&mut first_error, ops.clear_archive(linker, field));
    }
    remember(&mut first_error, ops.super_destroy(linker));
    first_error.map_or(Ok(()), Err)
}


struct RuntimeDestroyLoadOps<'a, 'b> {
    runtime: &'a Runtime<'b>,
}

impl DestroyLoadOps for RuntimeDestroyLoadOps<'_, '_> {
    type Error = i32;

    fn unloading_log(&mut self, linker: usize) -> Result<()> {
        self.runtime.call(A::UnloadingLog, linker, 0, 0, 0).map(|_| ())
    }

    fn detach_all_lazy(&mut self, linker: usize) -> Result<()> {
        self.runtime.call(A::DetachAllCall, linker, 0, 0, 0).map(|_| ())
    }

    fn export_count(&mut self, linker: usize) -> Result<i32> {
        self.runtime.int(linker, F::ExportCount, 0)
    }

    fn export_object(&mut self, linker: usize, index: i32) -> Result<usize> {
        self.runtime.get(linker, F::ExportObject, index)
    }

    fn detach_export(&mut self, linker: usize, index: i32) -> Result<()> {
        self.runtime.call(A::DetachExportCall, linker, index as usize, 0, 0).map(|_| ())
    }

    fn unregister(&mut self, linker: usize) -> Result<()> {
        self.runtime.call(A::Unregister, linker, 0, 0, 0).map(|_| ())
    }

    fn archive(&mut self, linker: usize, field: DestroyLoadArchive) -> Result<usize> {
        let field = match field {
            DestroyLoadArchive::Loader => F::Loader,
            DestroyLoadArchive::AltLoader => F::AltLoader,
        };
        self.runtime.get(linker, field, 0)
    }

    fn close_archive(&mut self, archive: usize) -> Result<()> {
        self.runtime.call(A::DeleteArchive, archive, 0, 0, 0).map(|_| ())
    }

    fn clear_archive(&mut self, linker: usize, field: DestroyLoadArchive) -> Result<()> {
        let field = match field {
            DestroyLoadArchive::Loader => F::Loader,
            DestroyLoadArchive::AltLoader => F::AltLoader,
        };
        self.runtime.set(linker, field, 0, 0)
    }

    fn super_destroy(&mut self, linker: usize) -> Result<()> {
        self.runtime.call(A::SuperDestroy, linker, 0, 0, 0).map(|_| ())
    }
}

struct Runtime<'a> {
    s: &'a Hp2LinkerRuntimeServices,
}

struct RuntimeSetLinkerOps<'a, 'b> {
    runtime: &'a Runtime<'b>,
}

impl SetLinkerOps for RuntimeSetLinkerOps<'_, '_> {
    type Error = i32;

    fn current_linker(&mut self, object: usize) -> Result<usize> {
        self.runtime.get(object, F::ObjectLinker, 0)
    }

    fn current_index(&mut self, object: usize) -> Result<i32> {
        self.runtime.int(object, F::ObjectLinkerIndex, 0)
    }

    fn export_object(&mut self, linker: usize, index: i32) -> Result<usize> {
        self.runtime.get(linker, F::ExportObject, index)
    }

    fn set_export_object(
        &mut self,
        linker: usize,
        index: i32,
        object: usize,
    ) -> Result<()> {
        self.runtime.set(linker, F::ExportObject, index, object)
    }

    fn set_object_linker(&mut self, object: usize, linker: usize) -> Result<()> {
        self.runtime.set(object, F::ObjectLinker, 0, linker)
    }

    fn set_object_index(&mut self, object: usize, index: i32) -> Result<()> {
        self.runtime.set(object, F::ObjectLinkerIndex, 0, index as usize)
    }

    fn invalid_export_owner(&mut self, _object: usize, linker: usize, index: i32) -> i32 {
        match self.runtime.call(A::MislinkedExport, linker, index as usize, 0, 0) {
            Err(status) => status,
            Ok(_) => -1,
        }
    }
}

impl Runtime<'_> {
    fn set_linker(&self, object: Handle, linker: Handle, index: i32) -> Result<()> {
        let mut ops = RuntimeSetLinkerOps { runtime: self };
        set_linker_with(&mut ops, object, linker, index)
    }
}
use Hp2LinkerAction as A;
use Hp2LinkerField as F;
use Hp2LinkerOperation as O;
impl Runtime<'_> {
    fn get(&self, p: Handle, f: F, i: i32) -> Result<Handle> {
        let mut value = 0;
        let status = unsafe { (self.s.field)(p, f, i, &mut value, 0) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
    fn int(&self, p: Handle, f: F, i: i32) -> Result<i32> {
        Ok(self.get(p, f, i)? as i32)
    }
    fn set(&self, p: Handle, f: F, i: i32, mut value: Handle) -> Result<()> {
        let status = unsafe { (self.s.field)(p, f, i, &mut value, 1) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn call(&self, op: A, p: Handle, a: Handle, b: Handle, c: Handle) -> Result<Handle> {
        if let A::SetLinker = op {
            self.set_linker(p, a, b as i32)?;
            return Ok(0);
        }
        let mut value = 0;
        let status = unsafe { (self.s.action)(op, p, a, b, c, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
    fn check(&self, slow: bool, value: impl FnOnce() -> Result<bool>) -> Result<()> {
        let enabled = if slow {
            self.s.constants.slow_checks
        } else {
            self.s.constants.checks
        };
        // Native check/checkSlow do not evaluate their expression when disabled.
        // In particular, release lazy-loader attachment must not scan the list.
        if enabled != 0 && !value()? {
            self.call(if slow { A::CheckSlow } else { A::Check }, 0, 0, 0, 0)?;
        }
        Ok(())
    }
    fn hash(&self, name: i32, class: i32, package: i32) -> usize {
        hash_names(
            name,
            class,
            package,
            self.s.constants.unreal_i,
            self.s.constants.unreal_share,
        ) as usize
            & 255
    }
    fn class_name_value(&self, p: Handle, i: i32) -> Result<i32> {
        let class = self.int(p, F::ExportClass, i)?;
        if class < 0 {
            self.int(p, F::ImportName, -class - 1)
        } else if class > 0 {
            self.int(p, F::ExportName, class - 1)
        } else {
            Ok(self.s.constants.class_name)
        }
    }
    fn class_package_value(&self, p: Handle, i: i32) -> Result<i32> {
        let class = self.int(p, F::ExportClass, i)?;
        if class < 0 {
            let parent = self.int(p, F::ImportPackage, -class - 1)?;
            self.check(true, || Ok(parent < 0))?;
            self.int(p, F::ImportName, -parent - 1)
        } else if class > 0 {
            self.int(self.get(p, F::Root, 0)?, F::ObjectName, 0)
        } else {
            Ok(self.s.constants.core)
        }
    }
    fn class_name(&self, p: Handle, i: i32) -> Result<i32> {
        Ok(self.call(A::ClassNameCall, p, i as usize, 0, 0)? as i32)
    }
    fn class_package(&self, p: Handle, i: i32) -> Result<i32> {
        Ok(self.call(A::ClassPackageCall, p, i as usize, 0, 0)? as i32)
    }
    fn broken(&self, p: Handle) -> Result<()> {
        let root = self.get(p, F::Root, 0)?;
        if self.get(root, F::IsPackage, 0)? != 0 {
            self.set(
                root,
                F::PackageFlags,
                0,
                self.get(root, F::PackageFlags, 0)? | self.s.constants.broken_links as usize,
            )?;
        }
        Ok(())
    }
    fn verify_import(&self, p: Handle, i: i32) -> Result<()> {
        let k = &self.s.constants;
        'shareware: loop {
            if self.int(p, F::ImportSourceIndex, i)? != -1
                || self.int(p, F::ImportClassPackage, i)? == k.none
                || self.int(p, F::ImportClassName, i)? == k.none
                || self.int(p, F::ImportName, i)? == k.none
            {
                return Ok(());
            }
            let mut depth = 0;
            let mut package = 0;
            let parent = self.int(p, F::ImportPackage, i)?;
            if parent == 0 {
                self.check(false, || {
                    Ok(self.int(p, F::ImportClassName, i)? == k.package)
                })?;
                self.check(false, || {
                    Ok(self.int(p, F::ImportClassPackage, i)? == k.core)
                })?;
                let mut tmp =
                    self.call(A::CreatePackage, p, self.get(p, F::ImportName, i)?, 0, 0)?;
                loop {
                    let flags = k.throw_flag | (self.get(p, F::LoadFlags, 0)? as u32 & k.propagate);
                    match self.call(A::PackageLinker, tmp, flags as usize, 0, 0) {
                        Ok(source) => {
                            self.set(p, F::ImportSource, i, source)?;
                            break;
                        }
                        Err(status) => {
                            // Only the original const TCHAR* catch handles this error.
                            if self.call(A::ExceptionConstText, 0, 0, 0, 0)? == 0 {
                                return Err(status);
                            }
                            self.call(A::ClearException, 0, 0, 0, 0)?;
                            if self.int(tmp, F::ObjectName, 0)? == k.unreal_i {
                                tmp =
                                    self.call(A::CreatePackage, p, k.unreal_share as usize, 0, 0)?;
                            } else {
                                self.call(A::RethrowAsText, 0, 0, 0, 0)?;
                                return Err(status);
                            }
                        }
                    }
                }
            } else {
                self.check(false, || Ok(parent < 0))?;
                self.call(A::VerifyImportCall, p, (-parent - 1) as usize, 0, 0)?;
                let parent = self.int(p, F::ImportPackage, i)?;
                let source = self.get(p, F::ImportSource, -parent - 1)?;
                self.set(p, F::ImportSource, i, source)?;
                self.check(false, || Ok(source != 0))?;
                let mut top = i;
                loop {
                    let parent = self.int(p, F::ImportPackage, top)?;
                    if parent >= 0 {
                        break;
                    }
                    top = -parent - 1;
                    depth += 1;
                }
                package = self.call(A::CreatePackage, p, self.get(p, F::ImportName, top)?, 0, 0)?;
            }
            let mut safe_replace = false;
            loop {
                let source = self.get(p, F::ImportSource, i)?;
                let name = self.int(p, F::ImportName, i)?;
                let class = self.int(p, F::ImportClassName, i)?;
                let class_package = self.int(p, F::ImportClassPackage, i)?;
                let hash = self.hash(name, class, class_package);
                let mut j = self.int(source, F::Hash, hash as i32)?;
                while j != -1 {
                    let source_package = self.class_package(source, j)?;
                    let class_hack =
                        class_package == k.unreal_i && source_package == k.unreal_share;
                    if self.int(source, F::ExportName, j)? == name
                        && self.class_name(source, j)? == class
                        && (source_package == class_package || class_hack)
                    {
                        let parent = self.int(p, F::ImportPackage, i)?;
                        let compatible = parent >= 0
                            || self.get(p, F::ImportSource, -parent - 1)? == 0
                            || parent_matches(
                                self.int(source, F::ExportPackage, j)?,
                                self.int(p, F::ImportSourceIndex, -parent - 1)?,
                            );
                        if compatible {
                            if self.get(source, F::ExportFlags, j)? as u32 & k.public_flag == 0 {
                                if self.get(p, F::LoadFlags, 0)? as u32 & k.forgiving != 0 {
                                    self.broken(p)?;
                                    self.call(A::BrokenImport, p, i as usize, 0, 0)?;
                                    return Ok(());
                                }
                                self.call(A::PrivateImport, p, i as usize, 0, 0)?;
                            }
                            self.set(p, F::ImportSourceIndex, i, j as usize)?;
                            break;
                        }
                    }
                    j = self.int(source, F::ExportNext, j)?;
                }
                // Legacy changes Mesh even after a successful first lookup.
                if self.call(A::IsMesh, 0, self.get(p, F::ImportClassName, i)?, 0, 0)? == 0 {
                    break;
                }
                self.set(
                    p,
                    F::ImportClassName,
                    i,
                    self.call(A::LodMeshName, 0, 0, 0, 0)?,
                )?;
            }
            if self.int(p, F::ImportSourceIndex, i)? == -1 && package != 0 {
                let cp = self.call(
                    A::FindPackage,
                    0,
                    self.get(p, F::ImportClassPackage, i)?,
                    0,
                    0,
                )?;
                if cp != 0 {
                    let class =
                        self.call(A::FindClass, cp, self.get(p, F::ImportClassName, i)?, 0, 0)?;
                    if class != 0 {
                        let object = self.call(
                            A::FindObject,
                            class,
                            package,
                            self.get(p, F::ImportName, i)?,
                            0,
                        )?;
                        let required = k.public_flag | k.native | k.transient;
                        if object != 0
                            && self.get(object, F::ObjectFlags, 0)? as u32 & required == required
                        {
                            self.set(p, F::ImportObject, i, object)?;
                            self.call(A::ImportIncrement, 0, 0, 0, 0)?;
                        } else if self.get(class, F::ClassFlags, 0)? as u32 & k.safe_replace != 0 {
                            if self.get(0, F::CheckConflicts, 0)? != 0 {
                                self.call(A::MissingImport, p, i as usize, class, 0)?;
                            }
                            safe_replace = true;
                        } else {
                            let object = self.call(
                                A::FindObjectAny,
                                class,
                                self.get(p, F::ImportName, i)?,
                                0,
                                0,
                            )?;
                            if object != 0 {
                                self.set(p, F::ImportObject, i, object)?;
                                self.call(A::ImportIncrement, 0, 0, 0, 0)?;
                            }
                        }
                    }
                }
                if self.get(p, F::ImportObject, i)? == 0
                    && self.int(package, F::ObjectName, 0)? == k.unreal_i
                    && depth == 1
                {
                    let count = self.int(p, F::ImportCount, 0)?;
                    self.set(p, F::ImportPackage, i, (-count - 1) as usize)?;
                    self.call(A::AddImport, p, 0, 0, 0)?;
                    self.set(p, F::ImportClassPackage, count, k.core as usize)?;
                    self.set(p, F::ImportClassName, count, k.package as usize)?;
                    self.set(p, F::ImportPackage, count, 0)?;
                    self.set(p, F::ImportName, count, k.unreal_share as usize)?;
                    self.set(p, F::ImportObject, count, 0)?;
                    self.set(p, F::ImportSource, count, 0)?;
                    self.set(p, F::ImportSourceIndex, count, (-1i32) as usize)?;
                    self.call(
                        A::VerifyImportCall,
                        p,
                        (self.int(p, F::ImportCount, 0)? - 1) as usize,
                        0,
                        0,
                    )?;
                    continue 'shareware;
                }
                if self.get(p, F::ImportObject, i)? == 0 && !safe_replace {
                    self.call(A::FailedImportLog, p, i as usize, 0, 0)?;
                    if self.get(p, F::LoadFlags, 0)? as u32 & k.forgiving != 0 {
                        self.broken(p)?;
                        self.call(A::BrokenImport, p, i as usize, 0, 0)?;
                        return Ok(());
                    }
                    self.call(A::FailedImport, p, i as usize, 0, 0)?;
                }
            }
            return Ok(());
        }
    }
    fn execute(
        &self,
        op: O,
        p: Handle,
        a: Handle,
        b: Handle,
        c: Handle,
        d: Handle,
    ) -> Result<Handle> {
        let k = &self.s.constants;
        match op {
            O::ImportConstructed => {
                if a != 0 {
                    self.call(A::ImportIncrement, 0, 0, 0, 0)?;
                }
            }
            O::SerializeLoad => {
                self.call(A::LinkerSerialize, p, a, 0, 0)?;
                self.call(A::CountLazy, p, a, 0, 0)?;
            }
            O::Context => {
                self.check(false, || Ok(self.get(p, F::Root, 0)? != 0))?;
                self.check(false, || Ok(a != 0))?;
                let flags = load_context_flags(
                    self.get(0, F::Editor, 0)? != 0,
                    self.get(0, F::Client, 0)? != 0,
                    self.get(0, F::Server, 0)? != 0,
                    k.load_edit, k.load_client, k.load_server,
                );
                self.set(p, F::Context, 0, flags as usize)?;
            }
            O::Serialize => {
                self.call(A::SuperSerialize, p, a, 0, 0)?;
                self.call(A::CountImports, p, a, 0, 0)?;
                self.call(A::CountExports, p, a, 0, 0)?;
                self.call(A::SerializeNames, p, a, 0, 0)?;
                self.call(A::SerializeRoot, p, a, 0, 0)?;
                let mut i = 0;
                while i < self.int(p, F::ExportCount, 0)? {
                    self.call(A::SerializeExportName, p, a, i as usize, 0)?;
                    i += 1;
                }
                i = 0;
                while i < self.int(p, F::ImportCount, 0)? {
                    self.call(A::SerializeImportSource, p, a, i as usize, 0)?;
                    self.call(A::SerializeImportPackage, p, a, i as usize, 0)?;
                    self.call(A::SerializeImportClass, p, a, i as usize, 0)?;
                    i += 1;
                }
            }
            O::ImportFullName => {
                let first = -(a as i32) - 1;
                let mut j = first;
                while j != 0 {
                    if j != first {
                        self.call(A::StringDot, b, 0, 0, 0)?;
                    }
                    self.call(A::StringImportName, p, (-j - 1) as usize, b, 0)?;
                    j = self.int(p, F::ImportPackage, -j - 1)?;
                }
                self.call(A::StringImportFinish, p, a, b, 0)?;
            }
            O::ExportFullName => {
                let first = a as i32 + 1;
                let mut j = first;
                while j != 0 {
                    if j != first {
                        self.call(A::StringDot, b, 0, 0, 0)?;
                    }
                    self.call(A::StringExportName, p, (j - 1) as usize, b, 0)?;
                    j = self.int(p, F::ExportPackage, j - 1)?;
                }
                let class = self.class_name_value(p, a as i32)?;
                let root = if c != 0 {
                    c
                } else {
                    self.call(A::RootPath, self.get(p, F::Root, 0)?, 0, 0, 0)?
                };
                self.call(A::StringExportFinish, p, class as usize, b, root)?;
            }
            O::Load => {
                self.set(p, F::Loader, 0, self.call(A::OpenReader, p, 0, 0, 0)?)?;
                if self.get(p, F::Loader, 0)? == 0 {
                    self.call(A::OpenFailed, p, 0, 0, 0)?;
                }
                self.set(p, F::AltLoader, 0, 0)?;
                return self.call(A::LoadScope, p, 0, 0, 0);
            }
            O::LoadTagged => {
                let mut i = 0;
                while i < self.int(0, F::LoaderCount, 0)? {
                    let loader = self.get(0, F::LoaderAt, i)?;
                    if self.get(loader, F::Root, 0)? == self.get(p, F::Root, 0)? {
                        self.call(A::LinkerExists, p, 0, 0, 0)?;
                    }
                    i += 1;
                }
                self.call(A::LoadingStatus, p, 0, 0, 0)?;
                for stage in [
                    A::InitArchiveScope,
                    A::SummaryScope,
                    A::CheckTagScope,
                    A::CheckVersionScope,
                ] {
                    self.call(stage, p, 0, 0, 0)?;
                }
                self.call(A::EmptyImports, p, self.get(p, F::SummaryImports, 0)?, 0, 0)?;
                self.call(A::EmptyExports, p, self.get(p, F::SummaryExports, 0)?, 0, 0)?;
                self.call(A::EmptyNames, p, self.get(p, F::SummaryNames, 0)?, 0, 0)?;
                for stage in [A::NamesScope, A::ImportsScope, A::ExportsScope] {
                    self.call(stage, p, 0, 0, 0)?;
                }
                for i in 0..256 {
                    self.set(p, F::Hash, i, (-1i32) as usize)?;
                }
                i = 0;
                while i < self.int(p, F::ExportCount, 0)? {
                    let hash = self.hash(
                        self.int(p, F::ExportName, i)?,
                        self.class_name(p, i)?,
                        self.class_package(p, i)?,
                    );
                    self.set(p, F::ExportNext, i, self.get(p, F::Hash, hash as i32)?)?;
                    self.set(p, F::Hash, hash as i32, i as usize)?;
                    i += 1;
                }
                self.call(A::Register, p, 0, 0, 0)?;
                if self.get(p, F::LoadFlags, 0)? as u32 & k.no_verify == 0 {
                    self.call(A::VerifyCall, p, 0, 0, 0)?;
                }
                self.set(p, F::Success, 0, 1)?;
            }
            O::InitArchive => {
                self.set(p, F::ArchiveVersion, 0, k.version as usize)?;
                self.set(p, F::ArchiveLicensee, 0, k.licensee as usize)?;
                self.set(p, F::ArchivePersistent, 0, 1)?;
                self.set(p, F::ArchiveLoading, 0, 1)?;
                self.set(p, F::ArchiveEdit, 0, self.get(0, F::Editor, 0)?)?;
                self.set(p, F::ArchiveClient, 0, 1)?;
                self.set(p, F::ArchiveServer, 0, 1)?;
            }
            O::Summary => {
                self.call(A::ReadSummary, p, 0, 0, 0)?;
                self.set(p, F::ArchiveVersion, 0, self.get(p, F::SummaryVersion, 0)?)?;
                self.set(
                    p,
                    F::ArchiveLicensee,
                    0,
                    self.get(p, F::SummaryLicensee, 0)?,
                )?;
                let root = self.get(p, F::Root, 0)?;
                if self.get(root, F::IsPackage, 0)? != 0 {
                    self.set(root, F::PackageFlags, 0, self.get(p, F::SummaryFlags, 0)?)?;
                }
            }
            O::CheckTag => {
                if self.int(p, F::SummaryTag, 0)? != k.file_tag {
                    self.call(A::BinaryFormat, p, 0, 0, 0)?;
                    self.call(A::Aborted, p, 0, 0, 0)?;
                }
            }
            O::CheckVersion => {
                if self.int(p, F::SummaryVersion, 0)? < k.min_version
                    && self.call(A::OldVersion, p, 0, 0, 0)? == 0
                {
                    self.call(A::Aborted, p, 0, 0, 0)?;
                }
            }
            O::Names | O::Imports | O::Exports => {
                let (count, offset, read) = match op {
                    O::Names => (F::SummaryNames, F::SummaryNameOffset, A::ReadName),
                    O::Imports => (F::SummaryImports, F::SummaryImportOffset, A::ReadImport),
                    _ => (F::SummaryExports, F::SummaryExportOffset, A::ReadExport),
                };
                if self.int(p, count, 0)? > 0 {
                    self.call(A::LinkerSeek, p, self.get(p, offset, 0)?, 0, 0)?;
                    let mut i = 0;
                    while i < self.int(p, count, 0)? {
                        // The native LoadNames scope owns one stack FNameEntry.
                        let record = match op {
                            O::Imports => self.call(A::AddImport, p, 0, 0, 0)?,
                            O::Exports => self.call(A::AddExport, p, 0, 0, 0)?,
                            _ => a,
                        };
                        self.call(read, p, record, 0, 0)?;
                        if matches!(op, O::Names) {
                            let name = if self.call(A::NameEntryFlags, a, 0, 0, 0)?
                                & self.get(p, F::Context, 0)?
                                != 0
                            {
                                self.call(A::NameEntryName, a, 0, 0, 0)?
                            } else {
                                k.none as usize
                            };
                            self.call(A::AddName, p, name, 0, 0)?;
                        }
                        i += 1;
                    }
                }
            }
            O::Verify => {
                if self.get(p, F::Verified, 0)? == 0 {
                    let root = self.get(p, F::Root, 0)?;
                    if self.get(root, F::IsPackage, 0)? != 0 {
                        self.set(
                            root,
                            F::PackageFlags,
                            0,
                            self.get(root, F::PackageFlags, 0)? & !(k.broken_links as usize),
                        )?;
                    }
                    if let Err(status) = self.call(A::VerifyImportsScope, p, 0, 0, 0) {
                        if self.call(A::ExceptionMutableText, 0, 0, 0, 0)? != 0 {
                            self.call(A::Unregister, p, 0, 0, 0)?;
                        }
                        return Err(status);
                    }
                }
                self.set(p, F::Verified, 0, 1)?;
            }
            O::VerifyImports => {
                let mut i = 0;
                while i < self.int(p, F::SummaryImports, 0)? {
                    self.call(A::VerifyImportCall, p, i as usize, 0, 0)?;
                    i += 1;
                }
            }
            O::ClassPackage => return Ok(self.class_package_value(p, a as i32)? as usize),
            O::ClassName => return Ok(self.class_name_value(p, a as i32)? as usize),
            O::VerifyImport => self.verify_import(p, a as i32)?,
            O::LoadAll => {
                let mut i = 0;
                while i < self.int(p, F::SummaryExports, 0)? {
                    self.call(A::CreateExportCall, p, i as usize, 0, 0)?;
                    i += 1;
                }
            }
            O::FindExport => {
                let mut class = a as i32;
                loop {
                    let hash = self.hash(c as i32, class, b as i32);
                    let mut i = self.int(p, F::Hash, hash as i32)?;
                    while i != -1 {
                        if self.int(p, F::ExportName, i)? == c as i32
                            && (self.int(p, F::ExportPackage, i)? == d as i32 || d as i32 == -1)
                            && self.class_package(p, i)? == b as i32
                            && self.class_name(p, i)? == class
                        {
                            return Ok(i as usize);
                        }
                        i = self.int(p, F::ExportNext, i)?;
                    }
                    if self.call(A::IsMesh, 0, class as usize, 0, 0)? == 0 {
                        return Ok((-1i32) as usize);
                    }
                    class = self.call(A::LodMeshName, 0, 0, 0, 0)? as i32;
                }
            }
            O::Create => {
                let index = self.call(
                    A::FindExportCall,
                    p,
                    self.get(a, F::ObjectName, 0)?,
                    self.get(self.get(a, F::ObjectOuter, 0)?, F::ObjectName, 0)?,
                    b,
                )?;
                if index as i32 != -1 {
                    return if c as u32 & k.verify != 0 {
                        Ok(usize::MAX)
                    } else {
                        self.call(A::CreateExportCall, p, index, 0, 0)
                    };
                }
                if d != 0 {
                    self.call(A::FailedCreate, a, b, 0, 0)?;
                }
            }
            O::Preload => {
                self.check(false, || Ok(self.get(p, F::ObjectValid, 0)? != 0))?;
                self.check(false, || Ok(a != 0))?;
                let linker = self.get(a, F::ObjectLinker, 0)?;
                if linker == p {
                    if self.get(a, F::ObjectFlags, 0)? as u32 & k.need_load != 0 {
                        if self.get(a, F::ObjectIsStruct, 0)? != 0 {
                            let super_field = self.get(a, F::ObjectSuper, 0)?;
                            if super_field != 0 {
                                self.call(A::PreloadCall, p, super_field, 0, 0)?;
                            }
                        }
                        self.call(A::PreloadScope, p, a, 0, 0)?;
                    }
                } else if linker != 0 {
                    self.call(A::PreloadCall, linker, a, 0, 0)?;
                }
            }
            O::PreloadBody => {
                let index = self.int(a, F::ObjectLinkerIndex, 0)?;
                self.check(false, || Ok(self.get(p, F::ExportObject, index)? == a))?;
                let loader = self.get(p, F::Loader, 0)?;
                let saved = self.call(A::ArchiveTell, loader, 0, 0, 0)?;
                self.call(
                    A::ArchiveSeek,
                    loader,
                    self.get(p, F::ExportOffset, index)?,
                    0,
                    0,
                )?;
                self.call(
                    A::ArchivePrecache,
                    loader,
                    self.get(p, F::ExportSize, index)?,
                    0,
                    0,
                )?;
                self.call(A::PreloadTagScope, p, a, saved, index as usize)?;
            }
            O::PreloadTagged => {
                self.call(A::ClearObjectFlags, a, k.need_load as usize, 0, 0)?;
                self.call(A::SetObjectFlags, a, k.preloading as usize, 0, 0)?;
                self.call(A::SerializeObject, p, a, 0, 0)?;
                self.call(A::ClearObjectFlags, a, k.preloading as usize, 0, 0)?;
                let position = self.call(A::LinkerTell, p, 0, 0, 0)? as i32;
                if position.wrapping_sub(self.int(p, F::ExportOffset, c as i32)?)
                    != self.int(p, F::ExportSize, c as i32)?
                {
                    self.call(A::SerialSizeError, p, a, c, 0)?;
                }
                self.call(A::ArchiveSeek, self.get(p, F::Loader, 0)?, b, 0, 0)?;
            }
            O::CreateExport => {
                let index = a as i32;
                if self.get(p, F::ExportObject, index)? == 0
                    && export_matches_context(
                        self.get(p, F::ExportFlags, index)? as u32,
                        self.get(p, F::Context, 0)? as u32,
                    )
                {
                    self.check(false, || {
                        Ok(self.int(p, F::ExportName, index)? != k.none
                            || self.get(p, F::ExportFlags, index)? as u32 & k.public_flag == 0)
                    })?;
                    let mut class = self.call(
                        A::IndexObjectCall,
                        p,
                        self.get(p, F::ExportClass, index)?,
                        0,
                        0,
                    )?;
                    if class == 0 {
                        class = self.call(A::ClassStatic, 0, 0, 0, 0)?;
                    }
                    self.check(false, || Ok(class != 0))?;
                    self.check(false, || {
                        Ok(self.get(class, F::ObjectClass, 0)?
                            == self.call(A::ClassStatic, 0, 0, 0, 0)?)
                    })?;
                    if self.int(class, F::ObjectName, 0)? == k.camera {
                        return Ok(0);
                    }
                    self.call(A::PreloadCall, p, class, 0, 0)?;
                    let parent_index = self.int(p, F::ExportPackage, index)?;
                    let parent = if parent_index != 0 {
                        self.call(A::IndexObjectCall, p, parent_index as usize, 0, 0)?
                    } else {
                        self.get(p, F::Root, 0)?
                    };
                    let existing = self.get(p, F::ExportObject, index)?;
                    if existing != 0 {
                        return Ok(existing);
                    }
                    let flags = (self.get(p, F::ExportFlags, index)? as u32 & k.load_mask)
                        | k.need_load
                        | k.need_post_load;
                    // Construct consumes a scalar name/flags pair, not an export reference.
                    let object = self.call(
                        A::Construct,
                        class,
                        parent,
                        self.get(p, F::ExportName, index)?,
                        flags as usize,
                    )?;
                    self.set(p, F::ExportObject, index, object)?;
                    self.call(A::SetLinker, object, p, a, 0)?;
                    return self.call(A::ExportTagScope, p, a, 0, 0);
                }
                return self.get(p, F::ExportObject, index);
            }
            O::ExportTagged => {
                let index = a as i32;
                self.call(
                    A::AppendLoaded,
                    self.get(p, F::ExportObject, index)?,
                    0,
                    0,
                    0,
                )?;
                self.call(A::CreatedLog, self.get(p, F::ExportObject, index)?, 0, 0, 0)?;
                if self.get(self.get(p, F::ExportObject, index)?, F::ObjectIsStruct, 0)? != 0
                    && self.int(p, F::ExportSuper, index)? != 0
                {
                    let super_field = self.call(
                        A::IndexObjectCall,
                        p,
                        self.get(p, F::ExportSuper, index)?,
                        0,
                        0,
                    )?;
                    self.set(
                        self.get(p, F::ExportObject, index)?,
                        F::ObjectSuper,
                        0,
                        super_field,
                    )?;
                }
                let object = self.get(p, F::ExportObject, index)?;
                if self.get(object, F::ObjectIsClass, 0)? != 0 {
                    self.call(A::Bind, object, 0, 0, 0)?;
                }
                return self.get(p, F::ExportObject, index);
            }
            O::CreateImport => {
                let index = a as i32;
                if self.get(p, F::ImportObject, index)? == 0
                    && self.int(p, F::ImportSourceIndex, index)? >= 0
                {
                    let source = self.get(p, F::ImportSource, index)?;
                    self.check(false, || Ok(source != 0))?;
                    let object = self.call(
                        A::CreateExportCall,
                        source,
                        self.get(p, F::ImportSourceIndex, index)?,
                        0,
                        0,
                    )?;
                    self.set(p, F::ImportObject, index, object)?;
                    self.call(A::ImportIncrement, 0, 0, 0, 0)?;
                }
                return self.get(p, F::ImportObject, index);
            }
            O::IndexObject => {
                let index = a as i32;
                if index > 0 {
                    let i = index - 1;
                    let count = self.int(p, F::ExportCount, 0)?;
                    if i < 0 || i >= count {
                        self.call(A::ExportIndexError, p, i as usize, count as usize, 0)?;
                    }
                    return self.call(A::CreateExportCall, p, i as usize, 0, 0);
                } else if index < 0 {
                    let i = index.wrapping_neg().wrapping_sub(1);
                    let count = self.int(p, F::ImportCount, 0)?;
                    if i < 0 || i >= count {
                        self.call(A::ImportIndexError, p, i as usize, count as usize, 0)?;
                    }
                    return self.call(A::CreateImportCall, p, i as usize, 0, 0);
                }
            }
            O::DetachExport => {
                let object = self.get(p, F::ExportObject, a as i32)?;
                self.check(false, || Ok(object != 0))?;
                if self.get(object, F::ObjectValid, 0)? == 0 {
                    self.call(A::InvalidExport, p, a, 0, 0)?;
                }
                if self.get(object, F::ObjectLinker, 0)? != p {
                    self.call(A::MislinkedExport, p, a, 0, 0)?;
                }
                if self.int(object, F::ObjectLinkerIndex, 0)? != a as i32 {
                    self.call(A::MisindexedExport, p, a, 0, 0)?;
                }
                self.call(
                    A::SetLinker,
                    self.get(p, F::ExportObject, a as i32)?,
                    0,
                    (-1i32) as usize,
                    0,
                )?;
            }
            O::DestroyLoad => {
                let mut ops = RuntimeDestroyLoadOps { runtime: self };
                destroy_load_with(&mut ops, p)?;
            }
            O::AttachLazy => {
                self.check(true, || Ok(self.get(a, F::LazyArchive, 0)? == 0))?;
                self.check(
                    true,
                    || Ok(self.call(A::LazyFind, p, a, 0, 0)? as i32 == -1),
                )?;
                self.call(A::LazyAdd, p, a, 0, 0)?;
                self.set(a, F::LazyArchive, 0, p)?;
                self.set(a, F::LazyPosition, 0, self.call(A::LinkerTell, p, 0, 0, 0)?)?;
            }
            O::DetachLazy => {
                self.check(true, || Ok(self.get(a, F::LazyArchive, 0)? == p))?;
                let removed = self.call(A::LazyRemove, p, a, 0, 0)?;
                if removed != 1 {
                    self.call(A::LazyDetachError, p, removed, 0, 0)?;
                }
                self.set(a, F::LazyArchive, 0, 0)?;
                self.set(a, F::LazyPosition, 0, 0)?;
            }
            O::DetachAll => {
                let mut i = 0;
                while i < self.int(p, F::LazyCount, 0)? {
                    let lazy = self.get(p, F::LazyAt, i)?;
                    if a != 0 {
                        self.call(A::LazyLoad, lazy, 0, 0, 0)?;
                    }
                    self.set(lazy, F::LazyArchive, 0, 0)?;
                    self.set(lazy, F::LazyPosition, 0, 0)?;
                    i += 1;
                }
                self.call(A::LazyEmpty, p, 0, 0, 0)?;
            }
            O::Stream => {
                if self.get(p, F::AltLoader, 0)? == 0 {
                    self.set(p, F::AltLoader, 0, self.call(A::OpenReader, p, 0, 0, 0)?)?;
                }
                return self.get(p, F::AltLoader, 0);
            }
            O::Save => {
                self.set(p, F::Saver, 0, self.call(A::OpenWriter, p, 0, 0, 0)?)?;
                if self.get(p, F::Saver, 0)? == 0 {
                    self.call(A::OpenFailed, p, 0, 0, 0)?;
                }
                self.set(p, F::SummaryTag, 0, k.file_tag as usize)?;
                self.call(
                    A::SetSaveVersions,
                    p,
                    k.version as usize,
                    k.licensee as usize,
                    0,
                )?;
                let root = self.get(p, F::Root, 0)?;
                let flags = if self.get(root, F::IsPackage, 0)? != 0 {
                    self.get(root, F::PackageFlags, 0)?
                } else {
                    0
                };
                self.set(p, F::SummaryFlags, 0, flags)?;
                self.set(p, F::ArchiveSaving, 0, 1)?;
                self.set(p, F::SaveArchivePersistent, 0, 1)?;
                self.set(p, F::SaveArchiveEdit, 0, self.get(0, F::Editor, 0)?)?;
                self.set(p, F::SaveArchiveClient, 0, 1)?;
                self.set(p, F::SaveArchiveServer, 0, 1)?;
                self.call(
                    A::AllocateObjectIndices,
                    p,
                    self.get(0, F::ObjectCount, 0)?,
                    0,
                    0,
                )?;
                self.call(
                    A::AllocateNameIndices,
                    p,
                    self.get(0, F::NameCount, 0)?,
                    0,
                    0,
                )?;
                self.set(p, F::Success, 0, 1)?;
            }
            O::DestroySave => {
                let saver = self.get(p, F::Saver, 0)?;
                if saver != 0 {
                    self.call(A::DeleteArchive, saver, 0, 0, 0)?;
                }
                self.set(p, F::Saver, 0, 0)?;
                self.call(A::SuperDestroy, p, 0, 0, 0)?;
            }
            O::MapName => {
                return self.call(A::SaveNameIndex, p, self.get(a, F::NameIndex, 0)?, 0, 0);
            }
            O::MapObject => {
                if a != 0 {
                    return self.call(A::SaveObjectIndex, p, self.get(a, F::ObjectIndex, 0)?, 0, 0);
                }
            }
            O::LoadObjectArchive => {
                let index = self.call(A::LoadObjectIndex, p, 0, 0, 0)?;
                let trace = self.get(p, F::Trace, 0)?;
                if trace != 0 && self.get(trace, F::TraceCaptured, 0)? == 0 {
                    self.set(trace, F::TraceIndex, 0, index)?;
                    self.set(
                        trace,
                        F::TraceOffset,
                        0,
                        self.call(A::ArchiveTell, self.get(p, F::Loader, 0)?, 0, 0, 0)?,
                    )?;
                    self.set(trace, F::TraceCaptured, 0, 1)?;
                }
                return self.call(A::IndexObjectCall, p, index, 0, 0);
            }
            O::SaveObjectArchive => {
                let index = self.execute(O::MapObject, p, a, 0, 0, 0)?;
                self.call(A::SaveObjectIndexArchive, p, index, 0, 0)?;
            }
        }
        Ok(0)
    }
}

/// # Safety
/// All handles must have the native type selected by the operation. Services
/// must catch every native exception, retaining its exact type until the outer
/// native shell rethrows. Only exception inspection/cleanup callbacks run after
/// an error, matching the two historical native catch sites.
pub unsafe fn execute(
    s: &Hp2LinkerRuntimeServices,
    op: O,
    p: *mut c_void,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
) -> Result<usize> {
    Runtime { s }.execute(op, p as usize, a, b, c, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatibility_hash_normalizes_only_the_class_package() {
        assert_eq!(hash_names(11, 9, 37, 23, 37), hash_names(11, 9, 23, 23, 37));
        assert_ne!(hash_names(37, 9, 23, 23, 37), hash_names(23, 9, 23, 23, 37));
        assert_eq!(
            hash_names(i32::MAX, i32::MAX, i32::MAX, 23, 37),
            i32::MAX.wrapping_mul(39)
        );
    }
    #[test]
    fn legacy_root_export_is_accepted_for_any_resolved_parent() {
        assert!(parent_matches(0, -1));
        assert!(parent_matches(0, 9));
        assert!(parent_matches(10, 9));
        assert!(!parent_matches(10, -1));
        assert!(!parent_matches(11, 9));
    }

    struct PreloadState {
        flags: usize,
        position: usize,
        serialized: usize,
        failure: i32,
        events: Vec<&'static str>,
    }

    unsafe extern "C" fn preload_field(p: usize, f: F, _: i32, out: *mut usize, write: i32) -> i32 {
        if write != 0 {
            return 90;
        }
        let value = match f {
            F::Loader => p,
            F::ExportOffset => 100,
            F::ExportSize => 12,
            _ => return 91,
        };
        unsafe { *out = value };
        0
    }

    unsafe extern "C" fn preload_action(
        op: A,
        p: usize,
        a: usize,
        _: usize,
        _: usize,
        out: *mut usize,
    ) -> i32 {
        let state = unsafe { &mut *(p as *mut PreloadState) };
        match op {
            A::ClearObjectFlags => {
                state.flags &= !a;
                state.events.push("clear");
            }
            A::SetObjectFlags => {
                state.flags |= a;
                state.events.push("set");
            }
            A::SerializeObject => {
                state.events.push("serialize");
                state.position += state.serialized;
                if state.failure != 0 {
                    return state.failure;
                }
            }
            A::LinkerTell => {
                unsafe { *out = state.position };
                state.events.push("tell");
            }
            A::ArchiveSeek => {
                state.position = a;
                state.events.push("restore");
            }
            A::SerialSizeError => {
                state.events.push("size-error");
                return 17;
            }
            _ => return 92,
        }
        0
    }

    fn preload_contract(serialized: usize, failure: i32) -> (Result<usize>, PreloadState) {
        // Every constants field is an integer; zero is valid for unused fields.
        let mut constants: Hp2LinkerConstants = unsafe { std::mem::zeroed() };
        constants.checks = 1;
        constants.need_load = 1;
        constants.preloading = 2;
        let services = Hp2LinkerRuntimeServices {
            field: preload_field,
            action: preload_action,
            constants,
        };
        let mut state = PreloadState {
            flags: 5,
            position: 100,
            serialized,
            failure,
            events: Vec::new(),
        };
        let pointer = &mut state as *mut PreloadState as usize;
        let result = Runtime { s: &services }.execute(O::PreloadTagged, pointer, pointer, 37, 0, 0);
        (result, state)
    }

    #[test]
    fn preload_throw_preserves_partial_flags_and_serial_position() {
        let (result, state) = preload_contract(4, 23);
        assert_eq!(result, Err(23));
        assert_eq!(state.flags, 6); // NeedLoad cleared; Preloading intentionally remains.
        assert_eq!(state.position, 104);
        assert_eq!(state.events, ["clear", "set", "serialize"]);
    }

    #[test]
    fn serial_size_error_does_not_restore_archive_position() {
        let (result, state) = preload_contract(11, 0);
        assert_eq!(result, Err(17));
        assert_eq!(state.flags, 4);
        assert_eq!(state.position, 111);
        assert_eq!(
            state.events,
            ["clear", "set", "serialize", "clear", "tell", "size-error"]
        );
    }

    #[test]
    fn successful_preload_restores_only_after_size_validation() {
        let (result, state) = preload_contract(12, 0);
        assert_eq!(result, Ok(0));
        assert_eq!(state.flags, 4);
        assert_eq!(state.position, 37);
        assert_eq!(
            state.events,
            ["clear", "set", "serialize", "clear", "tell", "restore"]
        );
    }

    struct ReentrantExports {
        objects: Vec<usize>,
        services: *const Hp2LinkerRuntimeServices,
        recursive_result: usize,
    }

    unsafe extern "C" fn export_field(p: usize, f: F, i: i32, out: *mut usize, write: i32) -> i32 {
        if write != 0 {
            return 80;
        }
        let value = match f {
            F::ExportObject => unsafe { (&(*(p as *const ReentrantExports)).objects)[i as usize] },
            F::ExportFlags | F::Context | F::ExportPackage => 1,
            F::ExportName => 10,
            F::ExportClass => 0,
            F::ObjectClass => 123,
            F::ObjectName => 3,
            _ => return 81,
        };
        unsafe { *out = value };
        0
    }

    unsafe extern "C" fn export_action(
        op: A,
        p: usize,
        a: usize,
        _: usize,
        _: usize,
        out: *mut usize,
    ) -> i32 {
        match op {
            A::Check => {
                if a == 0 {
                    return 82;
                }
            }
            A::ClassStatic => unsafe { *out = 123 },
            A::PreloadCall => {}
            A::IndexObjectCall if a == 0 => unsafe { *out = 123 },
            A::IndexObjectCall => {
                let state = p as *mut ReentrantExports;
                // Model a native outer-object callback that grows the map and
                // publishes this export while recursively loading its parent.
                unsafe {
                    (&mut (*state).objects).extend(std::iter::repeat_n(0, 64));
                    (&mut (*state).objects)[0] = 456;
                }
                // No &mut state or vector element survives this recursive call.
                let services = unsafe { &*(*state).services };
                match (Runtime { s: services }).execute(O::CreateExport, p, 0, 0, 0, 0) {
                    Ok(object) => unsafe {
                        (*state).recursive_result = object;
                        *out = 789;
                    },
                    Err(status) => return status,
                }
            }
            // In particular, constructing a second instance is a failure.
            _ => return 83,
        }
        0
    }

    #[test]
    fn recursive_parent_loading_publishes_export_before_outer_construction() {
        let mut constants: Hp2LinkerConstants = unsafe { std::mem::zeroed() };
        constants.checks = 1;
        constants.camera = 9;
        let services = Hp2LinkerRuntimeServices {
            field: export_field,
            action: export_action,
            constants,
        };
        let mut state = ReentrantExports {
            objects: vec![0],
            services: &services,
            recursive_result: 0,
        };
        let pointer = &mut state as *mut ReentrantExports as usize;
        assert_eq!(
            Runtime { s: &services }.execute(O::CreateExport, pointer, 0, 0, 0, 0),
            Ok(456)
        );
        assert_eq!(state.recursive_result, 456);
    }

    struct VerifyState {
        package_flags: usize,
        verified: usize,
        events: Vec<&'static str>,
    }

    unsafe extern "C" fn verify_field(p: usize, f: F, _: i32, out: *mut usize, write: i32) -> i32 {
        let state = unsafe { &mut *(p as *mut VerifyState) };
        match f {
            F::Root => unsafe { *out = p },
            F::IsPackage => unsafe { *out = 1 },
            F::Verified if write == 0 => unsafe { *out = state.verified },
            F::Verified => {
                state.verified = unsafe { *out };
                state.events.push("verified");
            }
            F::PackageFlags if write == 0 => unsafe { *out = state.package_flags },
            F::PackageFlags => {
                state.package_flags = unsafe { *out };
                state.events.push("clear-broken");
            }
            _ => return 70,
        }
        0
    }

    unsafe extern "C" fn verify_action<const MUTABLE_TEXT: bool>(
        op: A,
        p: usize,
        _: usize,
        _: usize,
        _: usize,
        out: *mut usize,
    ) -> i32 {
        if matches!(op, A::ExceptionMutableText) {
            unsafe { *out = MUTABLE_TEXT as usize };
            return 0;
        }
        let state = unsafe { &mut *(p as *mut VerifyState) };
        match op {
            A::VerifyImportsScope => {
                state.events.push("imports-fail");
                return 19;
            }
            A::Unregister => state.events.push("unregister"),
            _ => return 71,
        }
        0
    }

    fn verify_contract<const MUTABLE_TEXT: bool>() -> VerifyState {
        let mut constants: Hp2LinkerConstants = unsafe { std::mem::zeroed() };
        constants.broken_links = 1;
        let services = Hp2LinkerRuntimeServices {
            field: verify_field,
            action: verify_action::<MUTABLE_TEXT>,
            constants,
        };
        let mut state = VerifyState {
            package_flags: 5,
            verified: 0,
            events: Vec::new(),
        };
        let pointer = &mut state as *mut VerifyState as usize;
        assert_eq!(
            Runtime { s: &services }.execute(O::Verify, pointer, 0, 0, 0, 0),
            Err(19)
        );
        assert_eq!(state.package_flags, 4);
        assert_eq!(state.verified, 0);
        state
    }

    #[test]
    fn mutable_text_verification_error_unregisters_without_publishing_success() {
        assert_eq!(
            verify_contract::<true>().events,
            ["clear-broken", "imports-fail", "unregister"]
        );
    }

    #[test]
    fn const_text_verification_error_does_not_enter_mutable_text_cleanup() {
        assert_eq!(
            verify_contract::<false>().events,
            ["clear-broken", "imports-fail"]
        );
    }
}
