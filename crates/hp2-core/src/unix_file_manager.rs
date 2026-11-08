//! Unix file-manager policy. All OS, locale, archive and SDK effects are checked
//! native leaves; only invocation-local Rust buffers are borrowed across calls.
use crate::{ansi::Tchar, path, unix_text};
use std::ffi::{CStr, c_void};
use std::ptr;

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2UnixManagerOperation {
    Reader,
    Writer,
    Size,
    Copy,
    Move,
    Delete,
    GetTime,
    SetTime,
    MakeDirectory,
    DeleteDirectory,
    Find,
    SetDirectory,
    GetDirectory,
}

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2UnixManagerLeaf {
    Stat,
    Lstat,
    OpenDirectory,
    ReadDirectory,
    CloseDirectory,
    Lower,
    Match,
    Mkdir,
    Rmdir,
    Unlink,
    Rename,
    Chmod,
    Utime,
    Utimes,
    Chdir,
    Getcwd,
    OpenRead,
    OpenWrite,
    OpenAppend,
    SeekEnd,
    SeekStart,
    Tell,
    CloseFile,
    Unbuffered,
    ConstructReader,
    ConstructWriter,
    FatalFailure,
    LogFailure,
    SizeWarning,
    AmbiguousWarning,
    PublishName,
    PublishDirectory,
    Progress,
    VirtualReader,
    VirtualWriter,
    ArchiveSize,
    ArchiveError,
    ArchiveRead,
    ArchiveWrite,
    ArchiveClose,
    ArchiveDelete,
    VirtualDelete,
    VirtualCopy,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2UnixManagerMetadata {
    pub size: i64,
    pub mode: u32,
    pub directory: i32,
    pub access_seconds: i64,
    pub access_microseconds: i64,
    pub modify_seconds: i64,
    pub modify_microseconds: i64,
}

/// A leaf's input/output packet, always owned by Rust for the duration of a call.
/// Paths and text are NUL terminated. ReadDirectory returns the native name
/// pointer, valid until the next directory operation; Rust copies it immediately.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UnixManagerIo {
    pub path: *const u8,
    pub other_path: *const u8,
    pub text: *const c_void,
    pub other_text: *const c_void,
    pub handle: *mut c_void,
    pub buffer: *mut u8,
    pub length: usize,
    pub number: i64,
    pub value: i64,
    pub error: i32,
    pub fraction: f32,
    pub metadata: Hp2UnixManagerMetadata,
}
impl Default for Hp2UnixManagerIo {
    fn default() -> Self {
        Self {
            path: ptr::null(),
            other_path: ptr::null(),
            text: ptr::null(),
            other_text: ptr::null(),
            handle: ptr::null_mut(),
            buffer: ptr::null_mut(),
            length: 0,
            number: 0,
            value: 0,
            error: 0,
            fraction: 0.0,
            metadata: Hp2UnixManagerMetadata::default(),
        }
    }
}

/// Immutable copied service identity; never an exclusive borrow of native state.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UnixManagerServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, Hp2UnixManagerLeaf, *mut Hp2UnixManagerIo) -> i32,
    pub enoent: i32,
    pub enotdir: i32,
    pub eexist: i32,
    pub exdev: i32,
    pub erange: i32,
    pub directory_mode: u32,
    pub writable_mode: u32,
    pub no_replace: u32,
    pub even_read_only: u32,
    pub append: u32,
    pub unbuffered: u32,
    pub read_no_fail: u32,
    pub write_no_fail: u32,
    pub failure_flags: u32,
    pub has_output: i32,
    pub has_progress: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UnixManagerRequest {
    pub operation: Hp2UnixManagerOperation,
    pub first: *const c_void,
    pub second: *const c_void,
    pub flags: u32,
    pub a: i32,
    pub b: i32,
    pub c: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UnixManagerResult {
    pub value: i64,
    pub handle: *mut c_void,
}

type Checked<T> = Result<T, i32>;
use Hp2UnixManagerLeaf as L;

/// NUL-terminated, not necessarily UTF-8: directory traversal must preserve raw
/// filesystem names even though publication and case comparison are strict.
struct Path(Vec<u8>);
impl Path {
    fn new(bytes: &[u8]) -> Self {
        let mut out = Vec::with_capacity(bytes.len() + 1);
        out.extend_from_slice(bytes);
        out.push(0);
        Self(out)
    }
    fn bytes(&self) -> &[u8] {
        &self.0[..self.0.len() - 1]
    }
    fn pointer(&self) -> *const u8 {
        self.0.as_ptr()
    }
}
fn join(dir: &[u8], name: &[u8]) -> Path {
    let separator = !dir.is_empty() && dir != b"/";
    let mut out = Vec::with_capacity(dir.len() + name.len() + separator as usize + 1);
    out.extend_from_slice(dir);
    if separator {
        out.push(b'/');
    }
    out.extend_from_slice(name);
    out.push(0);
    Path(out)
}
fn parts(input: &[u8]) -> impl Iterator<Item = &[u8]> {
    input.split(|b| *b == b'/').filter(|part| !part.is_empty())
}
unsafe fn to_path(input: *const c_void) -> Option<Path> {
    let input = input.cast::<Tchar>();
    if input.is_null() {
        return None;
    }
    let length = unsafe { unix_text::tchar_length(input) };
    if length == 0 {
        return None;
    }
    let mut buffer = Vec::with_capacity(length * 4 + 1);
    let written = unsafe { path::native_path_to_utf8(input, buffer.as_mut_ptr(), length * 4) }?;
    unsafe {
        buffer.set_len(written);
    }
    buffer.push(0);
    Some(Path(buffer))
}
fn native(bytes: &[u8]) -> Option<Vec<Tchar>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut out = Vec::with_capacity(text.chars().count() + 1);
    out.extend(text.chars().map(|c| c as Tchar));
    out.push(0);
    Some(out)
}

impl Hp2UnixManagerServices {
    fn invoke(self, leaf: L, io: &mut Hp2UnixManagerIo) -> Checked<()> {
        let status = unsafe { (self.call)(self.context, leaf, io) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn leaf(self, leaf: L, io: Hp2UnixManagerIo) -> Checked<Hp2UnixManagerIo> {
        let mut io = io;
        self.invoke(leaf, &mut io)?;
        Ok(io)
    }
    fn path_leaf(self, leaf: L, path: &Path) -> Checked<Hp2UnixManagerIo> {
        self.leaf(
            leaf,
            Hp2UnixManagerIo {
                path: path.pointer(),
                ..Default::default()
            },
        )
    }
    fn handle_leaf(self, leaf: L, handle: *mut c_void) -> Checked<Hp2UnixManagerIo> {
        self.leaf(
            leaf,
            Hp2UnixManagerIo {
                handle,
                ..Default::default()
            },
        )
    }
    fn missing(self, error: i32) -> bool {
        error == self.enoent || error == self.enotdir
    }
    fn next(self, handle: *mut c_void) -> Checked<Option<Path>> {
        let io = self.handle_leaf(L::ReadDirectory, handle)?;
        if io.path.is_null() {
            Ok(None)
        } else {
            Ok(Some(Path::new(unsafe {
                CStr::from_ptr(io.path.cast()).to_bytes()
            })))
        }
    }
    fn case_equal(self, left: &[u8], right: &[u8]) -> Checked<bool> {
        let (Ok(left), Ok(right)) = (std::str::from_utf8(left), std::str::from_utf8(right)) else {
            return Ok(false);
        };
        if left.chars().count() != right.chars().count() {
            return Ok(false);
        }
        for (a, b) in left.chars().zip(right.chars()) {
            let a = self
                .leaf(
                    L::Lower,
                    Hp2UnixManagerIo {
                        number: a as i64,
                        ..Default::default()
                    },
                )?
                .value;
            let b = self
                .leaf(
                    L::Lower,
                    Hp2UnixManagerIo {
                        number: b as i64,
                        ..Default::default()
                    },
                )?
                .value;
            if a != b {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn ambiguous(self, dir: &Path, part: &[u8]) -> Checked<()> {
        let dir = native(dir.bytes());
        let part = native(part);
        self.leaf(
            L::AmbiguousWarning,
            Hp2UnixManagerIo {
                text: dir.as_ref().map_or(ptr::null(), |v| v.as_ptr().cast()),
                other_text: part.as_ref().map_or(ptr::null(), |v| v.as_ptr().cast()),
                ..Default::default()
            },
        )?;
        Ok(())
    }
    fn recover(self, input: &Path, missing_leaf: bool) -> Checked<(Option<Path>, bool)> {
        let mut current = Path::new(if input.bytes().starts_with(b"/") {
            b"/"
        } else {
            b"."
        });
        let mut ps = parts(input.bytes()).peekable();
        while let Some(part) = ps.next() {
            let candidate = join(current.bytes(), part);
            if part == b"." || part == b".." {
                current = candidate;
                continue;
            }
            if self.path_leaf(L::Lstat, &candidate)?.value == 0 {
                current = candidate;
                continue;
            }
            let handle = self.path_leaf(L::OpenDirectory, &current)?.handle;
            if handle.is_null() {
                return Ok((None, false));
            }
            let mut matched = None;
            let mut count = 0usize;
            while let Some(entry) = self.next(handle)? {
                if self.case_equal(entry.bytes(), part)? {
                    matched = Some(entry);
                    count += 1;
                }
            }
            self.handle_leaf(L::CloseDirectory, handle)?;
            if count > 1 {
                self.ambiguous(&current, part)?;
                return Ok((None, true));
            }
            if let Some(matched) = matched {
                current = join(current.bytes(), matched.bytes());
                continue;
            }
            if missing_leaf && ps.peek().is_none() {
                current = candidate;
                continue;
            }
            return Ok((None, false));
        }
        Ok((Some(current), false))
    }
    fn existing(self, input: &Path) -> Checked<(Option<Path>, bool)> {
        if self.path_leaf(L::Lstat, input)?.value == 0 {
            return Ok((Some(Path::new(input.bytes())), false));
        }
        self.recover(input, false)
    }
    fn stat_existing(self, input: &Path) -> Checked<Option<Hp2UnixManagerMetadata>> {
        let info = self.path_leaf(L::Stat, input)?;
        if info.value == 0 {
            return Ok(Some(info.metadata));
        }
        if !self.missing(info.error) {
            return Ok(None);
        }
        let (recovered, _) = self.recover(input, false)?;
        if let Some(recovered) = recovered {
            let info = self.path_leaf(L::Stat, &recovered)?;
            if info.value == 0 {
                return Ok(Some(info.metadata));
            }
        }
        Ok(None)
    }
    fn is_directory(self, input: &Path) -> Checked<(bool, i32)> {
        let info = self.path_leaf(L::Stat, input)?;
        Ok((info.value == 0 && info.metadata.directory != 0, info.error))
    }
    fn make_writable(self, input: &Path) -> Checked<()> {
        let info = self.path_leaf(L::Stat, input)?;
        if info.value == 0 {
            self.leaf(
                L::Chmod,
                Hp2UnixManagerIo {
                    path: input.pointer(),
                    number: (info.metadata.mode | self.writable_mode) as i64,
                    ..Default::default()
                },
            )?;
        }
        Ok(())
    }
    fn mkdir(self, input: &Path) -> Checked<Hp2UnixManagerIo> {
        self.leaf(
            L::Mkdir,
            Hp2UnixManagerIo {
                path: input.pointer(),
                number: self.directory_mode as i64,
                ..Default::default()
            },
        )
    }
    fn mkdir_tree(self, input: &Path) -> Checked<bool> {
        let mut current = Path::new(if input.bytes().starts_with(b"/") {
            b"/"
        } else {
            b"."
        });
        for part in parts(input.bytes()) {
            let candidate = join(current.bytes(), part);
            if part == b"." || part == b".." {
                current = candidate;
                continue;
            }
            let info = self.path_leaf(L::Stat, &candidate)?;
            if info.value == 0 {
                if info.metadata.directory == 0 {
                    return Ok(false);
                }
                current = candidate;
                continue;
            }
            let (recovered, ambiguous) = self.recover(&candidate, false)?;
            if let Some(recovered) = recovered {
                if !self.is_directory(&recovered)?.0 {
                    return Ok(false);
                }
                current = recovered;
                continue;
            }
            if ambiguous || self.mkdir(&candidate)?.value != 0 {
                return Ok(false);
            }
            current = candidate;
        }
        Ok(true)
    }
    fn remove_tree(self, dir: &Path) -> Checked<bool> {
        let handle = self.path_leaf(L::OpenDirectory, dir)?.handle;
        if handle.is_null() {
            return Ok(false);
        }
        let mut ok = true;
        while ok {
            let Some(entry) = self.next(handle)? else {
                break;
            };
            if entry.bytes() == b"." || entry.bytes() == b".." {
                continue;
            }
            let child = join(dir.bytes(), entry.bytes());
            let info = self.path_leaf(L::Lstat, &child)?;
            ok = if info.value != 0 {
                false
            } else if info.metadata.directory != 0 {
                self.remove_tree(&child)?
            } else {
                self.path_leaf(L::Unlink, &child)?.value == 0
            };
        }
        if self.handle_leaf(L::CloseDirectory, handle)?.value != 0 {
            ok = false;
        }
        Ok(ok && self.path_leaf(L::Rmdir, dir)?.value == 0)
    }
    fn failure(self, writer: bool, message: i64) -> Checked<*mut c_void> {
        let no_fail = if writer {
            self.write_no_fail
        } else {
            self.read_no_fail
        };
        if self.failure_flags & no_fail != 0 {
            self.leaf(
                L::FatalFailure,
                Hp2UnixManagerIo {
                    number: message,
                    ..Default::default()
                },
            )?;
        }
        if self.has_output != 0 {
            self.leaf(
                L::LogFailure,
                Hp2UnixManagerIo {
                    number: message,
                    ..Default::default()
                },
            )?;
        }
        Ok(ptr::null_mut())
    }
    unsafe fn reader(self, name: *const c_void) -> Checked<*mut c_void> {
        let Some(path) = (unsafe { to_path(name) }) else {
            return self.failure(false, 0);
        };
        let mut open = self.path_leaf(L::OpenRead, &path)?;
        if open.handle.is_null() && self.missing(open.error) {
            if let (Some(recovered), _) = self.recover(&path, false)? {
                open = self.path_leaf(L::OpenRead, &recovered)?;
            }
        }
        let file = open.handle;
        if file.is_null() {
            return self.failure(false, 1);
        }
        if self.handle_leaf(L::SeekEnd, file)?.value != 0 {
            self.handle_leaf(L::CloseFile, file)?;
            return self.failure(false, 2);
        }
        let size = self.handle_leaf(L::Tell, file)?.value;
        if size < 0 || size > i32::MAX as i64 || self.handle_leaf(L::SeekStart, file)?.value != 0 {
            self.handle_leaf(L::CloseFile, file)?;
            return self.failure(false, 3);
        }
        Ok(self
            .leaf(
                L::ConstructReader,
                Hp2UnixManagerIo {
                    handle: file,
                    number: size,
                    ..Default::default()
                },
            )?
            .handle)
    }
    unsafe fn writer(self, name: *const c_void, flags: u32) -> Checked<*mut c_void> {
        let Some(path) = (unsafe { to_path(name) }) else {
            return self.failure(true, 0);
        };
        let (existing, ambiguous) = self.existing(&path)?;
        if ambiguous {
            return self.failure(true, 4);
        }
        if flags & self.no_replace != 0 && existing.is_some() {
            return Ok(ptr::null_mut());
        }
        if flags & self.even_read_only != 0 {
            if let Some(existing) = &existing {
                self.make_writable(existing)?;
            }
        }
        let write_path = if let Some(existing) = existing {
            existing
        } else {
            let (recovered, ambiguous) = self.recover(&path, true)?;
            if ambiguous {
                return self.failure(true, 5);
            }
            let Some(recovered) = recovered else {
                return self.failure(true, 5);
            };
            recovered
        };
        let append = flags & self.append != 0;
        let file = self
            .path_leaf(
                if append { L::OpenAppend } else { L::OpenWrite },
                &write_path,
            )?
            .handle;
        if file.is_null() {
            return self.failure(true, 6);
        }
        if flags & self.unbuffered != 0 {
            self.handle_leaf(L::Unbuffered, file)?;
        }
        let mut position = 0;
        if append {
            let failed_seek = self.handle_leaf(L::SeekEnd, file)?.value != 0;
            if !failed_seek {
                position = self.handle_leaf(L::Tell, file)?.value;
            }
            if failed_seek || position < 0 || position > i32::MAX as i64 {
                self.handle_leaf(L::CloseFile, file)?;
                return self.failure(true, 7);
            }
        }
        Ok(self
            .leaf(
                L::ConstructWriter,
                Hp2UnixManagerIo {
                    handle: file,
                    number: position,
                    ..Default::default()
                },
            )?
            .handle)
    }
    unsafe fn copy_attributes(self, dest: *const c_void, src: *const c_void) -> Checked<bool> {
        let Some(dest) = (unsafe { to_path(dest) }) else {
            return Ok(false);
        };
        let Some(src) = (unsafe { to_path(src) }) else {
            return Ok(false);
        };
        let (Some(dest), false) = self.existing(&dest)? else {
            return Ok(false);
        };
        let (Some(src), false) = self.existing(&src)? else {
            return Ok(false);
        };
        let info = self.path_leaf(L::Stat, &src)?;
        if info.value != 0 {
            return Ok(false);
        }
        if self
            .leaf(
                L::Chmod,
                Hp2UnixManagerIo {
                    path: dest.pointer(),
                    number: (info.metadata.mode & 0o7777) as i64,
                    ..Default::default()
                },
            )?
            .value
            != 0
        {
            return Ok(false);
        }
        Ok(self
            .leaf(
                L::Utimes,
                Hp2UnixManagerIo {
                    path: dest.pointer(),
                    metadata: info.metadata,
                    ..Default::default()
                },
            )?
            .value
            == 0)
    }
    fn progress(self, fraction: f32) -> Checked<()> {
        if self.has_progress != 0 {
            self.leaf(
                L::Progress,
                Hp2UnixManagerIo {
                    fraction,
                    ..Default::default()
                },
            )?;
        }
        Ok(())
    }
    unsafe fn copy(self, r: Hp2UnixManagerRequest) -> Checked<bool> {
        self.progress(0.0)?;
        let src = self
            .leaf(
                L::VirtualReader,
                Hp2UnixManagerIo {
                    text: r.second,
                    ..Default::default()
                },
            )?
            .handle;
        if src.is_null() {
            self.progress(1.0)?;
            return Ok(false);
        }
        let flags = (if r.a != 0 { 0 } else { self.no_replace })
            | (if r.b != 0 { self.even_read_only } else { 0 });
        let dest = self
            .leaf(
                L::VirtualWriter,
                Hp2UnixManagerIo {
                    text: r.first,
                    number: flags as i64,
                    ..Default::default()
                },
            )?
            .handle;
        if dest.is_null() {
            self.handle_leaf(L::ArchiveClose, src)?;
            self.handle_leaf(L::ArchiveDelete, src)?;
            self.progress(1.0)?;
            return Ok(false);
        }
        let size = self.handle_leaf(L::ArchiveSize, src)?.value as i32;
        let mut total = 0i32;
        let mut buffer = vec![0u8; 64 * 1024];
        while total < size
            && self.handle_leaf(L::ArchiveError, src)?.value == 0
            && self.handle_leaf(L::ArchiveError, dest)?.value == 0
        {
            let count = (size - total).min(buffer.len() as i32);
            self.leaf(
                L::ArchiveRead,
                Hp2UnixManagerIo {
                    handle: src,
                    buffer: buffer.as_mut_ptr(),
                    number: count as i64,
                    ..Default::default()
                },
            )?;
            if self.handle_leaf(L::ArchiveError, src)?.value == 0 {
                self.leaf(
                    L::ArchiveWrite,
                    Hp2UnixManagerIo {
                        handle: dest,
                        buffer: buffer.as_mut_ptr(),
                        number: count as i64,
                        ..Default::default()
                    },
                )?;
            }
            total += count;
            if size != 0 {
                self.progress(total as f32 / size as f32)?;
            }
        }
        let mut success = total == size
            && self.handle_leaf(L::ArchiveError, src)?.value == 0
            && self.handle_leaf(L::ArchiveError, dest)?.value == 0;
        success = self.handle_leaf(L::ArchiveClose, dest)?.value != 0 && success;
        success = self.handle_leaf(L::ArchiveClose, src)?.value != 0 && success;
        self.handle_leaf(L::ArchiveDelete, dest)?;
        self.handle_leaf(L::ArchiveDelete, src)?;
        if !success {
            self.leaf(
                L::VirtualDelete,
                Hp2UnixManagerIo {
                    text: r.first,
                    number: 0,
                    ..Default::default()
                },
            )?;
        } else if r.c != 0 {
            success = unsafe { self.copy_attributes(r.first, r.second) }?;
        }
        self.progress(1.0)?;
        Ok(success)
    }
    unsafe fn move_file(self, r: Hp2UnixManagerRequest) -> Checked<bool> {
        let Some(src) = (unsafe { to_path(r.second) }) else {
            return Ok(false);
        };
        let Some(dest) = (unsafe { to_path(r.first) }) else {
            return Ok(false);
        };
        let (Some(src), false) = self.existing(&src)? else {
            return Ok(false);
        };
        let (existing_dest, ambiguous) = self.existing(&dest)?;
        if ambiguous || (r.a == 0 && existing_dest.is_some()) {
            return Ok(false);
        }
        if r.b != 0 {
            if let Some(existing) = &existing_dest {
                self.make_writable(existing)?;
            }
        }
        let dest = if let Some(existing) = existing_dest {
            existing
        } else {
            let (Some(recovered), false) = self.recover(&dest, true)? else {
                return Ok(false);
            };
            recovered
        };
        let renamed = self.leaf(
            L::Rename,
            Hp2UnixManagerIo {
                path: src.pointer(),
                other_path: dest.pointer(),
                ..Default::default()
            },
        )?;
        if renamed.value == 0 {
            return Ok(true);
        }
        if renamed.error == self.exdev {
            // Native virtual dispatch retains subclass Copy and Delete overrides.
            if self
                .leaf(L::VirtualCopy, Hp2UnixManagerIo::default())?
                .value
                == 0
            {
                return Ok(false);
            }
            return Ok(self
                .leaf(
                    L::VirtualDelete,
                    Hp2UnixManagerIo {
                        text: r.second,
                        number: 1,
                        ..Default::default()
                    },
                )?
                .value
                != 0);
        }
        Ok(false)
    }
    fn delete(self, path: &Path, require: bool, writable: bool) -> Checked<bool> {
        if writable {
            self.make_writable(path)?;
        }
        let removed = self.path_leaf(L::Unlink, path)?;
        if removed.value == 0 {
            return Ok(true);
        }
        if !self.missing(removed.error) {
            return Ok(false);
        }
        let (recovered, ambiguous) = self.recover(path, false)?;
        let Some(recovered) = recovered else {
            return Ok(!require && !ambiguous);
        };
        if writable {
            self.make_writable(&recovered)?;
        }
        let removed = self.path_leaf(L::Unlink, &recovered)?;
        Ok(removed.value == 0 || (!require && removed.error == self.enoent))
    }
    fn recovered_action(self, leaf: L, path: &Path) -> Checked<bool> {
        let result = self.path_leaf(leaf, path)?;
        if result.value == 0 {
            return Ok(true);
        }
        if !self.missing(result.error) {
            return Ok(false);
        }
        let (recovered, _) = self.recover(path, false)?;
        if let Some(recovered) = recovered {
            Ok(self.path_leaf(leaf, &recovered)?.value == 0)
        } else {
            Ok(false)
        }
    }
    fn make_directory(self, path: &Path, tree: bool) -> Checked<bool> {
        if tree {
            return self.mkdir_tree(path);
        }
        let result = self.mkdir(path)?;
        if result.value == 0 {
            return Ok(true);
        }
        let mut error = result.error;
        if error == self.eexist {
            let (directory, observed_error) = self.is_directory(path)?;
            if directory {
                return Ok(true);
            }
            error = observed_error;
        }
        if !self.missing(error) {
            return Ok(false);
        }
        let (recovered, _) = self.recover(path, false)?;
        if let Some(recovered) = recovered {
            Ok(self.is_directory(&recovered)?.0)
        } else {
            Ok(false)
        }
    }
    fn delete_directory(self, path: &Path, require: bool, tree: bool) -> Checked<bool> {
        if !tree {
            let result = self.path_leaf(L::Rmdir, path)?;
            if result.value == 0 {
                return Ok(true);
            }
            if !self.missing(result.error) {
                return Ok(false);
            }
        }
        let (recovered, ambiguous) = self.recover(path, false)?;
        let Some(recovered) = recovered else {
            return Ok(!require && !ambiguous);
        };
        if tree {
            return self.remove_tree(&recovered);
        }
        let result = self.path_leaf(L::Rmdir, &recovered)?;
        Ok(result.value == 0 || (!require && result.error == self.enoent))
    }
    fn find(self, spec: &Path, files: bool, directories: bool) -> Checked<()> {
        let slash = spec.bytes().iter().rposition(|b| *b == b'/');
        let mut dir = Path::new(match slash {
            None => b".",
            Some(0) => b"/",
            Some(i) => &spec.bytes()[..i],
        });
        let pattern = match slash {
            None => spec.bytes(),
            Some(i) => &spec.bytes()[i + 1..],
        };
        let pattern = Path::new(if pattern.is_empty() { b"*" } else { pattern });
        let mut open = self.path_leaf(L::OpenDirectory, &dir)?;
        if open.handle.is_null() && self.missing(open.error) {
            if let (Some(recovered), _) = self.recover(&dir, false)? {
                dir = recovered;
                open = self.path_leaf(L::OpenDirectory, &dir)?;
            }
        }
        if open.handle.is_null() {
            return Ok(());
        }
        while let Some(entry) = self.next(open.handle)? {
            if entry.bytes() == b"." || entry.bytes() == b".." {
                continue;
            }
            if self
                .leaf(
                    L::Match,
                    Hp2UnixManagerIo {
                        path: pattern.pointer(),
                        other_path: entry.pointer(),
                        ..Default::default()
                    },
                )?
                .value
                != 0
            {
                continue;
            }
            let full = join(dir.bytes(), entry.bytes());
            let info = self.path_leaf(L::Lstat, &full)?;
            if info.value != 0 {
                continue;
            }
            let is_dir = info.metadata.directory != 0;
            if (is_dir && !directories) || (!is_dir && !files) {
                continue;
            }
            if let Some(name) = native(entry.bytes()) {
                self.leaf(
                    L::PublishName,
                    Hp2UnixManagerIo {
                        text: name.as_ptr().cast(),
                        ..Default::default()
                    },
                )?;
            }
        }
        self.handle_leaf(L::CloseDirectory, open.handle)?;
        Ok(())
    }
    fn get_directory(self) -> Checked<()> {
        let mut buffer = vec![0u8; 256];
        loop {
            let result = self.leaf(
                L::Getcwd,
                Hp2UnixManagerIo {
                    buffer: buffer.as_mut_ptr(),
                    length: buffer.len(),
                    ..Default::default()
                },
            )?;
            if result.value != 0 {
                let bytes = unsafe { CStr::from_ptr(buffer.as_ptr().cast()).to_bytes() };
                if let Some(text) = native(bytes) {
                    self.leaf(
                        L::PublishDirectory,
                        Hp2UnixManagerIo {
                            text: text.as_ptr().cast(),
                            ..Default::default()
                        },
                    )?;
                }
                return Ok(());
            }
            if result.error != self.erange {
                return Ok(());
            }
            buffer.resize(buffer.len() * 2, 0);
        }
    }
}

/// Execute one complete public manager operation. Native callers retain all
/// typed result scopes and rethrow the first captured exception after return.
///
/// # Safety
/// Input TCHAR strings and immutable service identity remain valid throughout
/// synchronous callbacks. Leaves contain native exceptions and signals, return
/// a nonzero status without publishing further effects, and never unwind Rust.
pub unsafe fn execute(
    s: Hp2UnixManagerServices,
    r: Hp2UnixManagerRequest,
) -> Checked<Hp2UnixManagerResult> {
    use Hp2UnixManagerOperation as O;
    let mut result = Hp2UnixManagerResult {
        value: 0,
        handle: ptr::null_mut(),
    };
    match r.operation {
        O::Reader => {
            result.handle = unsafe { s.reader(r.first) }?;
            return Ok(result);
        }
        O::Writer => {
            result.handle = unsafe { s.writer(r.first, r.flags) }?;
            return Ok(result);
        }
        O::Copy => {
            result.value = unsafe { s.copy(r) }? as i64;
            return Ok(result);
        }
        O::Move => {
            result.value = unsafe { s.move_file(r) }? as i64;
            return Ok(result);
        }
        O::GetDirectory => {
            s.get_directory()?;
            return Ok(result);
        }
        O::Size => result.value = -1,
        _ => {}
    }
    let Some(path) = (unsafe { to_path(r.first) }) else {
        return Ok(result);
    };
    result.value = match r.operation {
        O::Size => {
            if let Some(info) = s.stat_existing(&path)? {
                if info.size < 0 || info.size > i32::MAX as i64 {
                    s.leaf(L::SizeWarning, Hp2UnixManagerIo::default())?;
                    -1
                } else {
                    info.size
                }
            } else {
                -1
            }
        }
        O::Delete => s.delete(&path, r.a != 0, r.b != 0)? as i64,
        O::GetTime => s
            .stat_existing(&path)?
            .map_or(0, |info| info.modify_seconds),
        O::SetTime => s.recovered_action(L::Utime, &path)? as i64,
        O::MakeDirectory => s.make_directory(&path, r.a != 0)? as i64,
        O::DeleteDirectory => s.delete_directory(&path, r.a != 0, r.b != 0)? as i64,
        O::Find => {
            s.find(&path, r.a != 0, r.b != 0)?;
            0
        }
        O::SetDirectory => s.recovered_action(L::Chdir, &path)? as i64,
        _ => unreachable!(),
    };
    Ok(result)
}
