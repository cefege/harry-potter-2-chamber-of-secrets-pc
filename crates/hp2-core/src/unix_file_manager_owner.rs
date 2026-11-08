use crate::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
    unix_file_manager::{self, Hp2UnixManagerIo, Hp2UnixManagerLeaf as L, Hp2UnixManagerOperation as O, Hp2UnixManagerRequest as R, Hp2UnixManagerServices as S},
};
use std::{ffi::{CStr, CString, c_void}, io::{Read, Seek, Write}, os::{fd::IntoRawFd, unix::{ffi::OsStrExt, fs::{MetadataExt, OpenOptionsExt, PermissionsExt}}}, path::{Path, PathBuf}, ptr};
unsafe extern "C" { fn towlower(character: crate::ansi::Tchar) -> crate::ansi::Tchar; }
const FILEREAD_NO_FAIL: u32 = 0x01;
const FILEWRITE_NO_FAIL: u32 = 0x01;
const FILEWRITE_NO_REPLACE_EXISTING: u32 = 0x02;
const FILEWRITE_EVEN_IF_READ_ONLY: u32 = 0x04;
const FILEWRITE_UNBUFFERED: u32 = 0x08;
const FILEWRITE_APPEND: u32 = 0x10;

struct Directory { entries: std::fs::ReadDir, current: Option<CString> }
enum Archive { File(Option<std::fs::File>), Reader(Option<std::fs::File>, bool, i64), Writer(Option<std::fs::File>, bool) }
// The File -> Reader/Writer transition reuses this allocation.
struct Context<'a> { access: CoreAccess<'a>, request: R, dirs: Vec<*mut Directory>, handles: Vec<*mut Archive>, found: Vec<Vec<u32>>, resolved_path: Option<PathBuf>, failure: Option<RuntimeError> }
impl Drop for Context<'_> {
    fn drop(&mut self) {
        for p in self.dirs.drain(..) { unsafe { drop(Box::from_raw(p)); } }
        for p in self.handles.drain(..) { unsafe { drop(Box::from_raw(p)); } }
    }
}
impl<'a> Context<'a> {
    fn new(access: CoreAccess<'a>, request: R) -> Self {
        Self { access, request, dirs: Vec::new(), handles: Vec::new(), found: Vec::new(), resolved_path: None, failure: None }
    }
    fn execute(&mut self, failure: &str) -> RuntimeResult<unix_file_manager::Hp2UnixManagerResult> {
        match unsafe { unix_file_manager::execute(services(self), self.request) } {
            Ok(result) => match self.failure.take() {
                Some(error) => Err(error),
                None => Ok(result),
            },
            Err(status) => Err(self.failure.take().unwrap_or_else(|| {
                RuntimeError::new(failure).with_context(&status.to_string())
            })),
        }
    }
    fn fail(&mut self, e: RuntimeError) -> i32 { if self.failure.is_none() { self.failure = Some(e); } libc::EIO }
}

fn services(context: &mut Context<'_>) -> S { let flags=context.request.flags; unsafe { services_raw(ptr::from_mut(context).cast(), flags) } }
unsafe fn services_raw(raw: *mut c_void, flags: u32) -> S {
    S { context: raw, call: leaf, enoent: libc::ENOENT, enotdir: libc::ENOTDIR, eexist: libc::EEXIST, exdev: libc::EXDEV, erange: libc::ERANGE,
        directory_mode: (libc::S_IRUSR | libc::S_IWUSR | libc::S_IXUSR | libc::S_IRGRP | libc::S_IXGRP | libc::S_IROTH | libc::S_IXOTH) as u32,
        writable_mode: (libc::S_IRUSR | libc::S_IWUSR) as u32, no_replace: FILEWRITE_NO_REPLACE_EXISTING, even_read_only: FILEWRITE_EVEN_IF_READ_ONLY, append: FILEWRITE_APPEND, unbuffered: FILEWRITE_UNBUFFERED, read_no_fail: FILEREAD_NO_FAIL, write_no_fail: FILEWRITE_NO_FAIL, failure_flags: flags, has_output: 1, has_progress: 0 }
}

pub fn find_files(access: CoreAccess<'_>, pattern: &Path) -> RuntimeResult<Vec<Vec<u32>>> {
    let pattern = resolved_path_units(access, pattern)?;
    let request = R { operation: O::Find, first: pattern.as_ptr().cast(), second: ptr::null(), flags: 0, a: 1, b: 0, c: 0 };
    let mut ctx = Context::new(access, request);
    ctx.execute("Canonical file enumeration failed")?;
    Ok(std::mem::take(&mut ctx.found))
}


/// Runs the shared Unix Move policy with the caller's exact native flags.
pub fn move_file_paths(
    access: CoreAccess<'_>,
    dest: &Path,
    src: &Path,
    replace: bool,
    even_if_read_only: bool,
    copy_attributes: bool,
) -> RuntimeResult<bool> {
    let mut dest = resolved_path_units(access, dest)?;
    let mut src = resolved_path_units(access, src)?;
    let request = R {
        operation: O::Move,
        first: dest.as_mut_ptr().cast(),
        second: src.as_mut_ptr().cast(),
        flags: 0,
        a: i32::from(replace),
        b: i32::from(even_if_read_only),
        c: i32::from(copy_attributes),
    };
    Ok(execute_request(access, request, "Canonical file move failed")?.value != 0)
}

/// Runs Copy with native destination/source ordering and the caller's readonly policy.
pub fn copy_file_paths(
    access: CoreAccess<'_>,
    dest: &Path,
    src: &Path,
    even_if_read_only: bool,
) -> RuntimeResult<bool> {
    let dest = resolved_path_units(access, dest)?;
    let src = resolved_path_units(access, src)?;
    let request = R {
        operation: O::Copy, first: dest.as_ptr().cast(), second: src.as_ptr().cast(),
        flags: 0, a: 1, b: i32::from(even_if_read_only), c: 0,
    };
    Ok(execute_request(access, request, "Canonical file copy failed")?.value != 0)
}

/// Runs Delete with the caller's EvenReadOnly flag and native non-required default.
pub fn delete_file_path(
    access: CoreAccess<'_>,
    path: &Path,
    even_if_read_only: bool,
) -> RuntimeResult<bool> {
    let path = resolved_path_units(access, path)?;
    let request = R {
        operation: O::Delete, first: path.as_ptr().cast(), second: ptr::null(),
        flags: 0, a: 0, b: i32::from(even_if_read_only), c: 0,
    };
    Ok(execute_request(access, request, "Canonical file deletion failed")?.value != 0)
}

pub fn file_size(access: CoreAccess<'_>, path: &Path) -> RuntimeResult<i32> {
    file_size_resolved(access, path).map(|(size, _)| size)
}

/// Retains the actual path used by the shared case-recovery/stat policy.
pub(crate) fn file_size_resolved(
    access: CoreAccess<'_>, path: &Path,
) -> RuntimeResult<(i32, Option<PathBuf>)> {
    let mut path = resolved_path_units(access, path)?;
    let request = R {
        operation: O::Size,
        first: path.as_mut_ptr().cast(),
        second: ptr::null(),
        flags: 0,
        a: 0,
        b: 0,
        c: 0,
    };
    let mut context = Context::new(access, request);
    let size = context.execute("Canonical file size query failed")?.value;
    let size = i32::try_from(size).map_err(|_| RuntimeError::new("Canonical file size exceeds INT"))?;
    Ok((size, if size >= 0 { context.resolved_path.take() } else { None }))
}

pub fn make_directory(access: CoreAccess<'_>, path: &Path) -> RuntimeResult<bool> {
    let mut path = resolved_path_units(access, path)?;
    let request = R {
        operation: O::MakeDirectory,
        first: path.as_mut_ptr().cast(),
        second: ptr::null(),
        flags: 0,
        a: 0,
        b: 0,
        c: 0,
    };
    Ok(execute_request(access, request, "Canonical directory creation failed")?.value != 0)
}

fn resolved_path_units(access: CoreAccess<'_>, path: &Path) -> RuntimeResult<Vec<u32>> {
    let mut units = if path.is_absolute() {
        crate::native_path::path_to_units(path)?
    } else {
        crate::native_path::path_to_units(&access.data_root().join(path))?
    };
    units.push(0);
    Ok(units)
}

fn execute_request(
    access: CoreAccess<'_>,
    request: R,
    failure: &str,
) -> RuntimeResult<crate::unix_file_manager::Hp2UnixManagerResult> {
    Context::new(access, request).execute(failure)
}

/// Runs the actual Unix reader/writer transaction (case recovery, ambiguity,
/// permissions and INT-size checks), then transfers its OS file into Core's
/// stdio archive without copying bytes or reopening a resolved pathname.
pub fn open_archive(
    access: CoreAccess<'_>,
    path: &Path,
    writing: bool,
) -> RuntimeResult<Option<crate::runtime_archive::RuntimeArchive>> {
    open_archive_with_flags(access, path, writing, 0)
}

/// Shared archive open with the exact Unix writer flags required by the caller.
pub fn open_archive_with_flags(
    access: CoreAccess<'_>,
    path: &Path,
    writing: bool,
    flags: u32,
) -> RuntimeResult<Option<crate::runtime_archive::RuntimeArchive>> {
    let mut units = resolved_path_units(access, path)?;
    let request = R {
        operation: if writing { O::Writer } else { O::Reader },
        first: units.as_mut_ptr().cast(),
        second: ptr::null(),
        flags,
        a: 0,
        b: 0,
        c: 0,
    };
    let mut context = Context::new(access, request);
    let result = context.execute("Canonical file archive open failed")?;
    if result.handle.is_null() {
        return Ok(None);
    }
    let handle = result.handle.cast::<Archive>();
    let index = context.handles.iter().position(|&owned| owned == handle)
        .ok_or_else(|| RuntimeError::new("Unix manager returned an unowned file archive"))?;
    context.handles.swap_remove(index);
    let archive = unsafe { Box::from_raw(handle) };
    let (file, size) = match *archive {
        Archive::Reader(Some(file), false, size) if !writing => (file, size),
        Archive::Writer(Some(file), false) if writing => (file, 0),
        _ => return Err(RuntimeError::new("Unix manager returned the wrong archive mode")),
    };
    // Like the native fopen leaf, failure to create the stdio resource is an
    // ordinary no-archive result with the caller's default GNull error device.
    Ok(crate::runtime_archive::RuntimeArchive::from_file(file, !writing, size).ok().map(|mut archive| {
        archive.use_null_error_device();
        archive
    }))
}

unsafe extern "C" fn leaf(raw: *mut c_void, op: L, io: *mut Hp2UnixManagerIo) -> i32 {
    let io = unsafe { &mut *io };
    if matches!(op, L::VirtualCopy | L::VirtualDelete | L::VirtualReader | L::VirtualWriter) {
        let mut request = unsafe { (*raw.cast::<Context<'_>>()).request };
        match op {
            L::VirtualCopy => request.operation = O::Copy,
            L::VirtualDelete => { request.operation = O::Delete; request.first = io.text; request.second = ptr::null(); request.a = io.number as i32; request.b = 1; request.c = 0; }
            L::VirtualReader => { request.operation = O::Reader; request.first = io.text; request.second = ptr::null(); request.flags = io.number as u32; }
            L::VirtualWriter => { request.operation = O::Writer; request.first = io.text; request.second = ptr::null(); request.flags = io.number as u32; }
            _ => unreachable!(),
        }
        let context = raw.cast::<Context<'_>>();
        let previous_request = unsafe { (*context).request };
        unsafe { (*context).request = request; }
        let result = unsafe { unix_file_manager::execute(services_raw(raw, request.flags), request) };
        unsafe { (*context).request = previous_request; }
        return match result {
            Ok(v) => { io.value = v.value; io.handle = v.handle; 0 }
            Err(_) => { io.value = 0; io.handle = ptr::null_mut(); 1 }
        };
    }
    let c = unsafe { &mut *raw.cast::<Context<'_>>() };
    match op {
        L::OpenDirectory => {
            let path = unsafe { bytes_path(io.path) };
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    let handle = Box::into_raw(Box::new(Directory { entries, current: None }));
                    c.dirs.push(handle);
                    io.handle = handle.cast();
                    io.error = 0;
                }
                Err(error) => {
                    io.handle = ptr::null_mut();
                    io.error = errno(&error);
                }
            }
            0
        }
        L::ReadDirectory => {
            let directory = unsafe { &mut *io.handle.cast::<Directory>() };
            match directory.entries.next() {
                Some(Ok(entry)) => match CString::new(entry.file_name().as_bytes()) {
                    Ok(name) => {
                        directory.current = Some(name);
                        io.path = directory.current.as_ref().unwrap().as_ptr().cast();
                        io.error = 0;
                        0
                    }
                    Err(_) => c.fail(RuntimeError::new("Directory entry contains an embedded NUL")),
                },
                Some(Err(error)) => {
                    io.error = errno(&error);
                    c.fail(RuntimeError::new("Unable to read canonical map search directory").with_context(&error.to_string()))
                }
                None => {
                    directory.current = None;
                    io.path = ptr::null();
                    io.error = 0;
                    0
                }
            }
        }
        L::CloseDirectory => {
            let handle = io.handle.cast::<Directory>();
            c.dirs.retain(|open| *open != handle);
            unsafe { drop(Box::from_raw(handle)); }
            0
        }
        L::Lstat | L::Stat => {
            let path = unsafe { bytes_path(io.path) };
            let metadata = if matches!(op, L::Lstat) { std::fs::symlink_metadata(&path) } else { std::fs::metadata(&path) };
            match metadata {
                Ok(metadata) => {
                    io.value = 0;
                    io.error = 0;
                    io.metadata.size = metadata.size() as i64;
                    io.metadata.mode = metadata.mode();
                    io.metadata.directory = i32::from(metadata.file_type().is_dir());
                    io.metadata.access_seconds = metadata.atime();
                    io.metadata.access_microseconds = metadata.atime_nsec() / 1000;
                    io.metadata.modify_seconds = metadata.mtime();
                    io.metadata.modify_microseconds = metadata.mtime_nsec() / 1000;
                    if matches!(c.request.operation, O::Size) { c.resolved_path = Some(path); }
                }
                Err(error) => { io.value = -1; io.error = errno(&error); }
            }
            0
        }
        L::Utimes => {
            let path = unsafe { bytes_path(io.path) };
            let filename = match CString::new(path.as_os_str().as_bytes()) {
                Ok(filename) => filename,
                Err(_) => { io.value = -1; io.error = libc::EINVAL; return 0; }
            };
            let times = [
                libc::timeval { tv_sec: io.metadata.access_seconds as _, tv_usec: io.metadata.access_microseconds as _ },
                libc::timeval { tv_sec: io.metadata.modify_seconds as _, tv_usec: io.metadata.modify_microseconds as _ },
            ];
            let status = unsafe { libc::utimes(filename.as_ptr(), times.as_ptr()) };
            io.value = status as i64;
            io.error = if status == 0 { 0 } else { errno(&std::io::Error::last_os_error()) };
            0
        }
        L::Match => {
            io.value = unsafe { libc::fnmatch(io.path.cast(), io.other_path.cast(), 0) } as i64;
            0
        }
        L::Lower => {
            io.value = unsafe { towlower(io.number as crate::ansi::Tchar) } as i64;
            0
        }
        L::AmbiguousWarning => {
            let name = unsafe { tchar_string(io.other_text) };
            let path = unsafe { tchar_string(io.text) };
            match name.and_then(|name| path.map(|path| (name, path))) {
                Ok((name, path)) => {
                    let message = format!("Ambiguous case-insensitive match for '{name}' in '{path}'");
                    let units = message.chars().map(u32::from).collect::<Vec<_>>();
                    match c.access.intern_str("Warning", FindName::Add).and_then(|category| c.access.emit_log(category, &units)) {
                        Ok(()) => 0,
                        Err(error) => c.fail(error),
                    }
                }
                Err(error) => c.fail(error),
            }
        }
        L::PublishName => match unsafe { tchar_units(io.text) } {
            Ok(name) => { c.found.push(name); 0 }
            Err(error) => c.fail(error),
        },
        L::Rename => {
            let source = unsafe { bytes_path(io.path) };
            let destination = unsafe { bytes_path(io.other_path) };
            match std::fs::rename(source, destination) {
                Ok(()) => { io.value = 0; io.error = 0; }
                Err(error) => { io.value = -1; io.error = errno(&error); }
            }
            0
        }
        L::Mkdir => {
            let path = unsafe { bytes_path(io.path) };
            match CString::new(path.as_os_str().as_bytes()) {
                Ok(path) => {
                    let status = unsafe { libc::mkdir(path.as_ptr(), io.number as libc::mode_t) };
                    io.value = status as i64;
                    io.error = if status == 0 { 0 } else { errno(&std::io::Error::last_os_error()) };
                }
                Err(_) => {
                    io.value = -1;
                    io.error = libc::EINVAL;
                }
            }
            0
        }

        L::Chmod => {
            let path = unsafe { bytes_path(io.path) };
            match std::fs::set_permissions(path, std::fs::Permissions::from_mode(io.number as u32)) {
                Ok(()) => { io.value = 0; io.error = 0; }
                Err(error) => { io.value = -1; io.error = errno(&error); }
            }
            0
        }
        L::Unlink => {
            let path = unsafe { bytes_path(io.path) };
            match std::fs::remove_file(path) {
                Ok(()) => { io.value = 0; io.error = 0; }
                Err(error) => { io.value = -1; io.error = errno(&error); }
            }
            0
        }
        L::OpenRead | L::OpenWrite | L::OpenAppend => {
            let path = unsafe { bytes_path(io.path) };
            let mut options = std::fs::OpenOptions::new();
            match op {
                L::OpenRead => { options.read(true); }
                L::OpenWrite => { options.write(true).create(true).truncate(true).mode(0o666); }
                _ => { options.append(true).create(true).mode(0o666); }
            }
            match options.open(path) {
                Ok(file) => { let handle = Box::into_raw(Box::new(Archive::File(Some(file)))); c.handles.push(handle); io.handle = handle.cast(); io.error = 0; }
                Err(error) => { io.handle = ptr::null_mut(); io.error = errno(&error); }
            }
            0
        }
        L::SeekEnd | L::SeekStart => {
            let handle = unsafe { &mut *io.handle.cast::<Archive>() };
            let result = match handle {
                Archive::File(Some(file)) => if matches!(op, L::SeekEnd) { file.seek(std::io::SeekFrom::End(0)) } else { file.seek(std::io::SeekFrom::Start(0)) },
                _ => Err(std::io::Error::from_raw_os_error(libc::EBADF)),
            };
            io.value = i64::from(result.is_err()); io.error = result.err().map_or(0, |error| errno(&error)); 0
        }
        L::Tell => {
            let handle = unsafe { &mut *io.handle.cast::<Archive>() };
            match handle { Archive::File(Some(file)) => match file.stream_position() { Ok(position) => { io.value = position as i64; io.error = 0; }, Err(error) => { io.value = -1; io.error = errno(&error); } }, _ => { io.value = -1; io.error = libc::EBADF; } }
            0
        }
        L::CloseFile => {
            let handle = io.handle.cast::<Archive>();
            c.handles.retain(|open| *open != handle);
            let file = match unsafe { &mut *handle } { Archive::File(file) => file.take(), _ => None };
            match file.map(close_file).unwrap_or(Err(libc::EBADF)) { Ok(()) => { io.value = 0; io.error = 0; }, Err(error) => { io.value = -1; io.error = error; } }
            unsafe { drop(Box::from_raw(handle)); }
            0
        }
        L::Unbuffered => { io.value = 0; 0 }
        L::ConstructReader | L::ConstructWriter => {
            let handle = unsafe { &mut *io.handle.cast::<Archive>() };
            let file = match std::mem::replace(handle, Archive::File(None)) { Archive::File(Some(file)) => file, _ => return c.fail(RuntimeError::new("Invalid Unix archive construction handle")) };
            *handle = if matches!(op, L::ConstructReader) { Archive::Reader(Some(file), false, io.number) } else { Archive::Writer(Some(file), false) };
            0
        }
        L::ArchiveSize => {
            let archive = unsafe { &mut *io.handle.cast::<Archive>() };
            io.value = match archive { Archive::Reader(_, _, size) => *size, _ => -1 };
            0
        }
        L::ArchiveError => {
            let archive = unsafe { &mut *io.handle.cast::<Archive>() };
            io.value = i64::from(match archive { Archive::Reader(_, error, _) | Archive::Writer(_, error) => *error, Archive::File(_) => true });
            0
        }
        L::ArchiveRead => {
            let archive = unsafe { &mut *io.handle.cast::<Archive>() };
            if let Archive::Reader(Some(file), error, _) = archive {
                let buffer = unsafe { std::slice::from_raw_parts_mut(io.buffer, io.number.max(0) as usize) };
                if file.read_exact(buffer).is_err() { *error = true; }
            } else { io.value = 1; }
            0
        }
        L::ArchiveWrite => {
            let archive = unsafe { &mut *io.handle.cast::<Archive>() };
            if let Archive::Writer(Some(file), error) = archive {
                let buffer = unsafe { std::slice::from_raw_parts(io.buffer, io.number.max(0) as usize) };
                if file.write_all(buffer).is_err() { *error = true; }
            } else { io.value = 1; }
            0
        }
        L::ArchiveClose => {
            let archive = unsafe { &mut *io.handle.cast::<Archive>() };
            let result = match archive {
                Archive::Reader(file, failed, _) => close_archive_file(file, *failed, false),
                Archive::Writer(file, failed) => close_archive_file(file, *failed, true),
                Archive::File(_) => Err(libc::EBADF),
            };
            io.value = i64::from(result.is_ok()); io.error = result.err().unwrap_or(0); 0
        }
        L::ArchiveDelete => {
            let handle = io.handle.cast::<Archive>();
            c.handles.retain(|open| *open != handle);
            unsafe { drop(Box::from_raw(handle)); }
            0
        }
        // VirtualReader/VirtualWriter explicitly receive GNull in the source.
        // Its Logf output is intentionally discarded, not routed to GLog.
        L::LogFailure => 0,
        L::FatalFailure => {
            let message = failure_message(io.number);
            let filename = unsafe { tchar_string(unsafe { (*raw.cast::<Context<'_>>()).request.first }) };
            match filename {
                Ok(filename) => {
                    let full = format!("{message}: {filename}");
                    c.fail(RuntimeError::new(&full))
                }
                Err(error) => c.fail(error),
            }
        }
        L::SizeWarning => {
            let filename = unsafe { tchar_string(unsafe { (*raw.cast::<Context<'_>>()).request.first }) };
            match filename {
                Ok(filename) => {
                    let message = format!("File size exceeds archive INT contract: {filename}");
                    let units = message.chars().map(u32::from).collect::<Vec<_>>();
                    match c.access.intern_str("Warning", FindName::Add).and_then(|category| c.access.emit_log(category, &units)) { Ok(()) => 0, Err(error) => c.fail(error) }
                }
                Err(error) => c.fail(error),
            }
        }
        _ => c.fail(RuntimeError::new("Unexpected canonical Unix file-manager operation")),
    }
}
fn failure_message(code: i64) -> &'static str {
    match code {
        0 => "Invalid Unicode file name",
        1 => "Failed to open file for reading",
        2 => "Failed to size file",
        3 => "File exceeds the archive INT contract",
        4 => "Ambiguous case-insensitive file name",
        5 => "Failed to resolve output file name",
        6 => "Failed to open file for writing",
        7 => "Append position exceeds archive INT contract",
        _ => "",
    }
}

fn errno(e:&std::io::Error)->i32{e.raw_os_error().unwrap_or(libc::EIO)}
unsafe fn bytes_path(p:*const u8)->PathBuf{PathBuf::from(std::ffi::OsStr::from_bytes(unsafe{CStr::from_ptr(p.cast())}.to_bytes()))}
unsafe fn tchar_slice<'a>(p: *const c_void) -> RuntimeResult<&'a [u32]> {
    if p.is_null() { return Err(RuntimeError::new("Null filesystem text")); }
    let p = p.cast::<crate::ansi::Tchar>();
    let n = unsafe { crate::unix_text::tchar_length(p) };
    Ok(crate::native_text::code_units(unsafe { std::slice::from_raw_parts(p, n) }))
}
unsafe fn tchar_units(p: *const c_void) -> RuntimeResult<Vec<u32>> {
    Ok(unsafe { tchar_slice(p) }?.to_vec())
}
unsafe fn tchar_string(p: *const c_void) -> RuntimeResult<String> {
    crate::native_path::from_utf32(unsafe { tchar_slice(p) }?)
}
fn close_file(file: std::fs::File) -> Result<(), i32> {
    let fd = file.into_raw_fd();
    if unsafe { libc::close(fd) } == 0 { Ok(()) }
    else { Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO)) }
}

fn close_archive_file(file: &mut Option<std::fs::File>, failed: bool, writer: bool) -> Result<(), i32> {
    let mut file = file.take().ok_or(libc::EBADF)?;
    let flush_error = if writer { file.flush().err().map(|error| errno(&error)) } else { None };
    let close_error = close_file(file).err();
    if let Some(error) = flush_error.or(close_error) { return Err(error); }
    if failed { return Err(libc::EIO); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_access::CoreOptions;

    #[test]
    fn canonical_copy_preserves_chunks_and_retains_a_rejected_destination() {
        let root=PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("unix-copy-{}",std::process::id()));
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::write(root.join("System/Default.ini"),"").unwrap();
        std::fs::write(root.join("System/DefUser.ini"),"").unwrap();
        let owner=crate::core_access::CoreOwner::new(CoreOptions{
            data_root:root.clone(),system_ini:root.join("System/Default.ini"),
            user_ini:root.join("System/DefUser.ini"),command_line:Vec::new(),
            output:|_,_,_|Ok(()),log_file:None,
        }).unwrap();
        let access=owner.access();
        let source=root.join("source.bin");let destination=root.join("destination.bin");
        let bytes:Vec<u8>=(0..70_001).map(|index|(index%251) as u8).collect();
        std::fs::write(&source,&bytes).unwrap();
        let copy=||{
            let encode=|path:&std::path::Path|crate::native_path::path_to_units(path)
                .unwrap().into_iter().map(|unit|unit as crate::ansi::Tchar).chain([0]).collect::<Vec<_>>();
            let source=encode(&source);let destination=encode(&destination);
            let request=R{operation:O::Copy,first:destination.as_ptr().cast(),second:source.as_ptr().cast(),flags:0,a:1,b:0,c:0};
            let mut context=Context::new(access,request);
            let result=unsafe{unix_file_manager::execute(services(&mut context),request)};
            if let Some(error)=context.failure.take(){panic!("{error:?}");}
            result.unwrap().value!=0
        };
        assert!(copy());
        assert_eq!(std::fs::read(&destination).unwrap(),bytes,"copy spans the 64 KiB archive boundary without truncating the tail");
        std::fs::write(&destination,b"preserve on writer failure").unwrap();
        std::fs::set_permissions(&destination,std::fs::Permissions::from_mode(0o400)).unwrap();
        assert!(!copy(),"the default Copy does not make a read-only destination writable");
        assert_eq!(std::fs::read(&destination).unwrap(),b"preserve on writer failure");
        assert_eq!(std::fs::read(&source).unwrap(),bytes,"copy failure retains the original source");
        std::fs::set_permissions(&destination,std::fs::Permissions::from_mode(0o600)).unwrap();
        owner.shutdown().unwrap();
    }
}
