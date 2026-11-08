//! Canonical Rust ownership for Actor object files and GameSaveInfo records.
use crate::actor_files::{self, FileOperation, Hp2ActorFileServices};
use hp2_abi::engine::generated::{TArrayRaw, UGameSaveInfo, UObject};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind},
    name_owner::FindName,
    object_archive::{
        ArchiveFlags, ArchiveMapping, ArchiveMode, BorrowedArchiveCursor, ObjectArchive,
    },
    object_owner::ClassOps,
    runtime_archive::{RuntimeArchive, RuntimeNameMapping},
};
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    ffi::c_void,
    path::PathBuf,
    ptr,
};

const OBJECT_OPERATIONS: ClassOps = NativeRecordKind::Object.operations();
const GAME_SAVE_INFO_PROPERTIES: &[StartupProperty] = &[
    StartupProperty {
        name: "numBeans",
        kind: StartupPropertyKind::Int,
        category: "",
        property_flags: 0,
        object_flags: 4,
        inner_name: None,
        offset: std::mem::offset_of!(UGameSaveInfo, num_beans),
        array_dim: 1,
    },
    StartupProperty {
        name: "numStars",
        kind: StartupPropertyKind::Int,
        category: "",
        property_flags: 0,
        object_flags: 4,
        inner_name: None,
        offset: std::mem::offset_of!(UGameSaveInfo, num_stars),
        array_dim: 1,
    },
    StartupProperty {
        name: "numPoints",
        kind: StartupPropertyKind::Int,
        category: "",
        property_flags: 0,
        object_flags: 4,
        inner_name: None,
        offset: std::mem::offset_of!(UGameSaveInfo, num_points),
        array_dim: 1,
    },
    StartupProperty {
        name: "savePointID",
        kind: StartupPropertyKind::Int,
        category: "",
        property_flags: 0,
        object_flags: 4,
        inner_name: None,
        offset: std::mem::offset_of!(UGameSaveInfo, save_point_id),
        array_dim: 1,
    },
    StartupProperty {
        name: "currentLevelString",
        kind: StartupPropertyKind::String,
        category: "",
        property_flags: 0,
        object_flags: 4,
        inner_name: None,
        offset: std::mem::offset_of!(UGameSaveInfo, current_level_string),
        array_dim: 1,
    },
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "GameSaveInfo",
    package: "Engine",
    super_name: Some("Object"),
    within: "Object",
    flags: 0,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<UGameSaveInfo>()),
    operations: Some(OBJECT_OPERATIONS),
    properties: GAME_SAVE_INFO_PROPERTIES,
    defaults: &[],
}];

struct Names<'a>(CoreAccess<'a>);

impl RuntimeNameMapping for Names<'_> {
    fn name_text(&self, name_index: i32) -> Result<Vec<u32>, i32> {
        self.0.name_text(name_index).map_err(|_| 1)
    }
    fn add_name(&self, text: &[u32]) -> Result<i32, i32> {
        self.0.intern(text, FindName::Add).map_err(|_| 1)
    }
}

/// `FArchiveFileReaderUnix` overrides only FName serialization; base FArchive
/// leaves UObject references untouched. Borrowed ObjectArchive honors that
/// source rule before consulting this required, unreachable mapping interface.
struct FileArchiveMapping;
impl ArchiveMapping for FileArchiveMapping {
    fn load_object(&self, _index: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new(
            "Base file archive does not map UObject references",
        ))
    }
    fn save_object(&self, _object: *mut UObject) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Base file archive does not map UObject references",
        ))
    }
    fn load_name(&self, _index: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Unix file archive names use the FName text codec",
        ))
    }
    fn save_name(&self, _index: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Unix file archive names use the FName text codec",
        ))
    }
    fn visit_object(&self, _slot: *mut *mut UObject) -> RuntimeResult<()> {
        Err(RuntimeError::new(
            "Base file archive does not visit UObject references",
        ))
    }
    fn visit_name(&self, _index: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(
            "Unix file archive names use the FName text codec",
        ))
    }
}

struct FileSession {
    archive: RefCell<RuntimeArchive>,
}
struct ServiceContext<'a> {
    access: CoreAccess<'a>,
    first_error: RefCell<Option<RuntimeError>>,
    active_session: Cell<*mut FileSession>,
}
impl Drop for ServiceContext<'_> {
    fn drop(&mut self) {
        let session = self.active_session.replace(ptr::null_mut());
        if !session.is_null() {
            // On a serializer failure the shared legacy policy returns before
            // its close callback; Rust still releases the owned file handle.
            unsafe {
                drop(Box::from_raw(session));
            }
        }
    }
}

struct FileCursor<'a> {
    archive: &'a RefCell<RuntimeArchive>,
    names: &'a dyn RuntimeNameMapping,
}
impl BorrowedArchiveCursor for FileCursor<'_> {
    fn tell(&self) -> RuntimeResult<usize> {
        usize::try_from(self.archive.borrow_mut().tell().map_err(archive_error)?)
            .map_err(|_| RuntimeError::new("Negative actor-file archive position"))
    }
    fn seek(&self, position: usize) -> RuntimeResult<()> {
        let position = i32::try_from(position)
            .map_err(|_| RuntimeError::new("Actor-file archive position exceeds INT"))?;
        self.archive
            .borrow_mut()
            .seek(position)
            .map_err(archive_error)
    }
    fn total_size(&self) -> RuntimeResult<usize> {
        usize::try_from(
            self.archive
                .borrow_mut()
                .total_size()
                .map_err(archive_error)?,
        )
        .map_err(|_| RuntimeError::new("Negative actor-file archive size"))
    }
    fn is_error(&self) -> RuntimeResult<bool> {
        Ok(self.archive.borrow().error().is_some())
    }
    fn read(&self, output: &mut [u8]) -> RuntimeResult<()> {
        let count = i32::try_from(output.len())
            .map_err(|_| RuntimeError::new("Actor-file read exceeds INT"))?;
        let status = unsafe {
            self.archive
                .borrow_mut()
                .serialize(output.as_mut_ptr().cast(), count)
        };
        if status == 0 {
            Ok(())
        } else {
            Err(archive_error(status))
        }
    }
    fn write(&self, input: &[u8]) -> RuntimeResult<()> {
        let count = i32::try_from(input.len())
            .map_err(|_| RuntimeError::new("Actor-file write exceeds INT"))?;
        let status = unsafe {
            self.archive
                .borrow_mut()
                .serialize(input.as_ptr().cast_mut().cast(), count)
        };
        if status == 0 {
            Ok(())
        } else {
            Err(archive_error(status))
        }
    }
    unsafe fn serialize_name(&self, name_index: *mut i32) -> RuntimeResult<()> {
        unsafe {
            self.archive
                .borrow_mut()
                .serialize_name_with(name_index, self.names)
        }
        .map_err(archive_error)
    }
}

fn archive_error(code: i32) -> RuntimeError {
    RuntimeError::new("Actor file archive operation failed").with_context(&code.to_string())
}
fn source_archive_flags() -> ArchiveFlags {
    let constants = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    ArchiveFlags::new(constants.version as u16, constants.licensee as u16, true)
}
fn fstring_path(raw: *const TArrayRaw) -> RuntimeResult<PathBuf> {
    if raw.is_null() {
        return Err(RuntimeError::new("Null actor-file path FString"));
    }
    let value = unsafe { raw.read_unaligned() };
    if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid actor-file path FString storage"));
    }
    let units = unsafe { hp2_core::native_text::fstring_units(raw) };
    let mut text = String::new();
    for unit in units.iter().copied().take_while(|&unit| unit != 0) {
        text.push(
            char::from_u32(unit as u32)
                .ok_or_else(|| RuntimeError::new("Invalid actor-file path character"))?,
        );
    }
    Ok(PathBuf::from(text))
}
// The service context is borrowed only by `transfer_file`; all callbacks,
// including nested independent transfers, finish before that frame exits.
// No callback stores this reference or exposes it to Core.
fn contextual<'a>(context: *mut c_void) -> RuntimeResult<&'a ServiceContext<'a>> {
    if context.is_null() {
        return Err(RuntimeError::new("Null actor-file service context"));
    }
    Ok(unsafe { &*context.cast::<ServiceContext<'a>>() })
}
fn remember(context: &ServiceContext<'_>, error: RuntimeError) -> i32 {
    let mut first = context.first_error.borrow_mut();
    if first.is_none() {
        *first = Some(error);
    }
    1
}
fn service_result(context: &ServiceContext<'_>, result: RuntimeResult<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => remember(context, error),
    }
}
fn session<'a>(archive: *mut c_void) -> RuntimeResult<&'a FileSession> {
    if archive.is_null() {
        return Err(RuntimeError::new("Null actor-file archive"));
    }
    Ok(unsafe { &*archive.cast::<FileSession>() })
}

unsafe extern "C" fn open(
    context: *mut c_void,
    path: *const c_void,
    writing: i32,
    save_slot: i32,
    out_archive: *mut *mut c_void,
) -> i32 {
    let Ok(context) = contextual(context) else {
        return 1;
    };
    if out_archive.is_null() {
        return remember(context, RuntimeError::new("Null actor-file archive output"));
    }
    unsafe {
        out_archive.write(ptr::null_mut());
    }
    let result = (|| -> RuntimeResult<Option<Box<FileSession>>> {
        let leaf = fstring_path(path.cast())?;
        let path = if save_slot != 0 {
            crate::game_save_owner::save_slot_file_path(context.access, &leaf)?
        } else if leaf.is_absolute() {
            leaf
        } else {
            context.access.data_root().join("System").join(leaf)
        };
        match hp2_core::unix_file_manager_owner::open_archive(context.access, &path, writing != 0)? {
            Some(archive) => Ok(Some(Box::new(FileSession {
                archive: RefCell::new(archive),
            }))),
            None => Ok(None),
        }
    })();
    match result {
        Ok(Some(archive)) if context.active_session.get().is_null() => {
            let archive = Box::into_raw(archive);
            context.active_session.set(archive);
            unsafe {
                out_archive.write(archive.cast());
            }
            0
        }
        Ok(Some(archive)) => {
            drop(archive);
            remember(
                context,
                RuntimeError::new("Actor-file context already owns an archive"),
            )
        }
        Ok(None) => 0,
        Err(error) => remember(context, error),
    }
}

unsafe fn serialize_object(
    context: &ServiceContext<'_>,
    archive: *mut c_void,
    object: *mut c_void,
) -> RuntimeResult<()> {
    let session = unsafe { session(archive)? };
    let names = Names(context.access);
    let mapping = FileArchiveMapping;
    let cursor = FileCursor {
        archive: &session.archive,
        names: &names,
    };
    let loading = session.archive.borrow().is_loading();
    let mode = if loading {
        ArchiveMode::Loading
    } else {
        ArchiveMode::Saving
    };
    let archive = ObjectArchive::borrowed(&cursor, mode, source_archive_flags(), &mapping)?;
    let object = object.cast::<UObject>();
    if object.is_null() {
        return Err(RuntimeError::new("Null actor-file UObject"));
    }
    unsafe {
        context.access.serialize_object(object, &archive)?;
    }
    Ok(())
}
unsafe extern "C" fn serialize_object_callback(
    context: *mut c_void,
    archive: *mut c_void,
    object: *mut c_void,
) -> i32 {
    let Ok(context) = contextual(context) else {
        return 1;
    };
    service_result(context, unsafe {
        serialize_object(context, archive, object)
    })
}

unsafe fn serialize_info_field(
    context: &ServiceContext<'_>,
    archive: *mut c_void,
    info: *mut c_void,
    field: i32,
) -> RuntimeResult<()> {
    let session = unsafe { session(archive)? };
    if info.is_null() {
        return Err(RuntimeError::new("Null GameSaveInfo object"));
    }
    let names = Names(context.access);
    let mapping = FileArchiveMapping;
    let cursor = FileCursor {
        archive: &session.archive,
        names: &names,
    };
    let loading = session.archive.borrow().is_loading();
    let mode = if loading {
        ArchiveMode::Loading
    } else {
        ArchiveMode::Saving
    };
    let archive = ObjectArchive::borrowed(&cursor, mode, source_archive_flags(), &mapping)?;
    let info = info.cast::<UGameSaveInfo>();
    unsafe {
        match field {
            0 => archive.serialize_i32(&mut *ptr::addr_of_mut!((*info).num_beans))?,
            1 => archive.serialize_i32(&mut *ptr::addr_of_mut!((*info).num_stars))?,
            2 => archive.serialize_i32(&mut *ptr::addr_of_mut!((*info).num_points))?,
            3 => archive.serialize_i32(&mut *ptr::addr_of_mut!((*info).save_point_id))?,
            4 => context.access.serialize_owned_fstring(
                &archive,
                ptr::addr_of_mut!((*info).current_level_string),
            )?,
            _ => {
                return Err(RuntimeError::new("Unknown GameSaveInfo archive field")
                    .with_context(&field.to_string()));
            }
        }
    }
    Ok(())
}
unsafe extern "C" fn serialize_info_field_callback(
    context: *mut c_void,
    archive: *mut c_void,
    info: *mut c_void,
    field: i32,
) -> i32 {
    let Ok(context) = contextual(context) else {
        return 1;
    };
    service_result(context, unsafe {
        serialize_info_field(context, archive, info, field)
    })
}
unsafe extern "C" fn close(context: *mut c_void, archive: *mut c_void) -> i32 {
    let Ok(context) = contextual(context) else {
        return 1;
    };
    if archive.is_null() {
        return remember(
            context,
            RuntimeError::new("Null actor-file archive on close"),
        );
    }
    let owned = context.active_session.replace(ptr::null_mut());
    if owned.cast::<c_void>() != archive {
        context.active_session.set(owned);
        return remember(
            context,
            RuntimeError::new("Actor-file close handle does not match its active archive"),
        );
    }
    let mut session = unsafe { Box::from_raw(owned) };
    let result = session.archive.get_mut().close().map_err(archive_error);
    service_result(context, result)
}

/// Executes the existing four-operation policy with Core-owned file and object
/// serialization leaves. The borrowed path is the VM's live FString storage.
///
/// # Safety
/// `path` and `object` must remain valid canonical Core storage for this call.
pub(crate) unsafe fn transfer_file(
    access: CoreAccess<'_>,
    operation: FileOperation,
    path: *const TArrayRaw,
    object: *mut UObject,
) -> RuntimeResult<bool> {
    let context = ServiceContext {
        access,
        first_error: RefCell::new(None),
        active_session: Cell::new(ptr::null_mut()),
    };
    let services = Hp2ActorFileServices {
        context: (&context as *const ServiceContext<'_>).cast_mut().cast(),
        open,
        serialize_object: serialize_object_callback,
        serialize_info_field: serialize_info_field_callback,
        close,
    };
    let result =
        unsafe { actor_files::transfer_file(&services, operation, path.cast(), object.cast()) }
            .map_err(|status| {
                context
                    .first_error
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| archive_error(status))
            })?;
    if let Some(error) = context.first_error.borrow_mut().take() {
        return Err(error);
    }
    Ok(result)
}
