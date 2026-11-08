//! Rust-owned file archives for the native-free object/package runtime.
//!
//! Stdio is the OS ABI leaf. Cursor/range/error policy, compact indices,
//! FString wire conversion and linker-name/record ordering use the same Core
//! owners as the native executable. No FArchive vtable is fabricated.
use crate::{
    ansi::Tchar,
    array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
    compact_index::{self, Hp2CompactIndexServices},
    fstring_archive::{self, Hp2FStringArchiveServices},
    linker_name_archive::{self, Hp2LinkerNameArchiveServices},
    linker_record_archive::{self, Hp2LinkerRecordArchiveServices},
    package_summary_archive::{self, Hp2PackageSummaryArchiveServices},
    unix_file_archive::{
        self, Hp2UnixArchiveAction as Action, Hp2UnixArchiveFailure as Failure,
        Hp2UnixArchiveLeaf as Leaf, Hp2UnixArchiveServices, Hp2UnixArchiveState,
    },
};
use hp2_abi::engine::generated::{FName, FPackageFileSummary, TArrayRaw, ULinkerLoad, CORE_LINKER_CONSTANTS};
use std::{
    ffi::{CString, c_char, c_int, c_void},
    io,
    os::{fd::{FromRawFd, IntoRawFd}, unix::ffi::OsStrExt},
    path::Path,
    ptr,
};

unsafe extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void;
    fn fdopen(fd: c_int, mode: *const c_char) -> *mut c_void;
    fn fseeko(file: *mut c_void, offset: i64, origin: c_int) -> c_int;
    fn ftello(file: *mut c_void) -> i64;
    fn fread(data: *mut c_void, width: usize, count: usize, file: *mut c_void) -> usize;
    fn fwrite(data: *const c_void, width: usize, count: usize, file: *mut c_void) -> usize;
    fn ferror(file: *mut c_void) -> c_int;
    fn fflush(file: *mut c_void) -> c_int;
    fn fclose(file: *mut c_void) -> c_int;
    fn realloc(data: *mut c_void, bytes: usize) -> *mut c_void;
    fn free(data: *mut c_void);
}

#[derive(Debug)]
pub enum ArchiveError {
    Policy { code: u32, errno: Option<i32> },
    Size,
    Allocation,
    MissingName,
    Closed,
    Callback(i32),
}

/// Owns one real stdio resource and its canonical Unix archive state.
/// The linker pointer, when bound, addresses the actual compiler-probed maps.
/// Engine-thread serialized; service contexts are invocation-local raw pointers.
pub struct RuntimeArchive {
    state: Hp2UnixArchiveState,
    loading: bool,
    closed: bool,
    version: i32,
    error: Option<ArchiveError>,
    null_error_device: bool,
    linker: *mut ULinkerLoad,
    wire: Vec<u16>,
    counted_num: u64,
    counted_max: u64,
}

/// Core name-table operations for Unix FArchive's UTF-16 name wire.
pub trait RuntimeNameMapping {
    fn name_text(&self, name_index: i32) -> Result<Vec<u32>, i32>;
    fn add_name(&self, text: &[u32]) -> Result<i32, i32>;
}

struct RuntimeNameCall<'a> {
    archive: *mut RuntimeArchive,
    name_index: *mut i32,
    mapping: &'a dyn RuntimeNameMapping,
    text: Vec<u32>,
    loaded: Option<i32>,
}

impl RuntimeArchive {
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_mode(path, true)
    }
    pub fn create(path: &Path) -> io::Result<Self> {
        Self::open_mode(path, false)
    }

    /// Takes the actual file opened by the shared Unix manager, preserving its
    /// captured reader size and initial cursor instead of reopening the path.
    pub fn from_file(file: std::fs::File, loading: bool, size: i64) -> io::Result<Self> {
        let descriptor = file.into_raw_fd();
        let stream = unsafe { fdopen(descriptor, if loading { c"rb".as_ptr() } else { c"wb".as_ptr() }) };
        if stream.is_null() {
            let error = io::Error::last_os_error();
            unsafe { drop(std::fs::File::from_raw_fd(descriptor)); }
            return Err(error);
        }
        Self::from_stdio(stream, loading, size)
    }

    fn open_mode(path: &Path, loading: bool) -> io::Result<Self> {
        let path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in archive path"))?;
        let file = unsafe {
            fopen(
                path.as_ptr(),
                if loading {
                    c"rb".as_ptr()
                } else {
                    c"wb".as_ptr()
                },
            )
        };
        if file.is_null() {
            return Err(io::Error::last_os_error());
        }
        let size = if loading {
            let result = unsafe { fseeko(file, 0, 2) };
            let length = if result == 0 {
                unsafe { ftello(file) }
            } else {
                -1
            };
            if length < 0 || unsafe { fseeko(file, 0, 0) } != 0 {
                let error = io::Error::last_os_error();
                unsafe {
                    fclose(file);
                }
                return Err(error);
            }
            length
        } else {
            0
        };
        Self::from_stdio(file, loading, size)
    }

    fn from_stdio(file: *mut c_void, loading: bool, size: i64) -> io::Result<Self> {
        let mut archive = Self {
            state: Hp2UnixArchiveState {
                file: ptr::null_mut(),
                size: 0,
                position: 0,
            },
            loading,
            closed: false,
            version: CORE_LINKER_CONSTANTS.version,
            error: None,
            null_error_device: false,
            linker: ptr::null_mut(),
            wire: Vec::new(),
            counted_num: 0,
            counted_max: 0,
        };
        // Init has no failing external leaf: it publishes mode and the real resource.
        let raw = &raw mut archive;
        unsafe {
            Self::run(
                raw,
                if loading {
                    Action::InitReader
                } else {
                    Action::InitWriter
                },
                size,
                file,
            )
        }
        .expect("Unix archive initialization has no fallible leaf");
        Ok(archive)
    }

    pub fn version(&self) -> i32 {
        self.version
    }
    pub fn set_version(&mut self, version: i32) {
        self.version = version;
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn error(&self) -> Option<&ArchiveError> {
        self.error.as_ref()
    }
    /// Matches native GNull: ordinary Unix archive failures set ArIsError,
    /// while actual callback/mapping failures still interrupt the operation.
    pub fn use_null_error_device(&mut self) {
        self.null_error_device = true;
    }
    fn has_throwing_error(&self) -> bool {
        self.error.as_ref().is_some_and(|error|
            !self.null_error_device || !matches!(error, ArchiveError::Policy { .. }))
    }
    pub fn counted_bytes(&self) -> (u64, u64) {
        (self.counted_num, self.counted_max)
    }

    /// Serializes a global FName through the source Unix archive's UTF-16 text format.
    ///
    /// # Safety
    /// `name_index` is a live global FName slot; mapping operations remain valid
    /// for the synchronous call and do not reenter this archive.
    pub unsafe fn serialize_name_with(
        &mut self,
        name_index: *mut i32,
        mapping: &dyn RuntimeNameMapping,
    ) -> Result<(), i32> {
        if name_index.is_null() {
            return Err(1);
        }
        let mut context = RuntimeNameCall {
            archive: self,
            name_index,
            mapping,
            text: Vec::new(),
            loaded: None,
        };
        let services = Hp2UnixArchiveServices {
            context: (&mut context as *mut RuntimeNameCall<'_>).cast(),
            call: runtime_name_leaf,
            tchar_width: size_of::<Tchar>() as u32,
        };
        let action = if self.loading { Action::ReadName } else { Action::WriteName };
        let result = unsafe {
            unix_file_archive::run(
                ptr::addr_of_mut!(self.state),
                services,
                action,
                0,
                ptr::null_mut(),
            )
        };
        if let Err(status) = result {
            unsafe { record_status(self, status); }
            return Err(status);
        }
        if self.has_throwing_error() { Err(1) } else { Ok(()) }
    }

    /// # Safety
    /// Linker and its canonical arrays remain live through every archive operation.
    pub unsafe fn bind_linker(&mut self, linker: *mut ULinkerLoad) {
        self.linker = linker;
    }

    pub fn seek(&mut self, position: i32) -> Result<(), i32> {
        unsafe {
            Self::run(
                self,
                if self.loading {
                    Action::SeekReader
                } else {
                    Action::SeekWriter
                },
                position as i64,
                ptr::null_mut(),
            )
        }
        .map(|_| ())
    }
    pub fn tell(&mut self) -> Result<i32, i32> {
        unsafe {
            Self::run(
                self,
                if self.loading {
                    Action::TellReader
                } else {
                    Action::TellWriter
                },
                0,
                ptr::null_mut(),
            )
        }
    }
    pub fn total_size(&mut self) -> Result<i32, i32> {
        unsafe { Self::run(self, Action::SizeReader, 0, ptr::null_mut()) }
    }
    pub fn flush(&mut self) -> Result<(), i32> {
        unsafe { Self::run(self, Action::Flush, 0, ptr::null_mut()) }.map(|_| ())
    }
    pub fn close(&mut self) -> Result<(), i32> {
        unsafe {
            Self::run(
                self,
                if self.loading {
                    Action::CloseReader
                } else {
                    Action::CloseWriter
                },
                0,
                ptr::null_mut(),
            )
        }
        .map(|_| ())
    }

    /// # Safety
    /// Data covers `bytes` readable bytes when saving, writable bytes when loading.
    /// Archive access and all pointed-to storage are engine-thread serialized.
    pub unsafe fn serialize(&mut self, data: *mut c_void, bytes: i32) -> i32 {
        unsafe { serialize_raw(self, data, bytes as i64) }
    }
    /// # Safety
    /// Value is writable i32 storage; only saving requires an initialized value.
    pub unsafe fn compact(&mut self, value: *mut i32) -> i32 {
        unsafe { serialize_index((self as *mut Self).cast(), value) }
    }
    /// # Safety
    /// String is initialized TArrayRaw TCHAR storage owned by this allocator.
    pub unsafe fn string(&mut self, string: *mut TArrayRaw) -> i32 {
        let raw = self as *mut Self;
        let services = unsafe { string_services(raw) };
        let status = unsafe { fstring_archive::serialize(raw.cast(), string.cast(), &services) };
        unsafe { record_status(raw, status) }
    }
    /// # Safety
    /// Summary is initialized compiler-probed storage, including empty generations.
    pub unsafe fn summary(&mut self, summary: *mut FPackageFileSummary) -> i32 {
        let arrays = array_services(self);
        let services = summary_services(&raw const arrays);
        let status = unsafe {
            package_summary_archive::serialize((self as *mut Self).cast(), summary.cast(), &services)
        };
        if status == 0 {
            self.version =
                unsafe { ptr::addr_of!((*summary).file_version).read_unaligned() } & 0xffff;
        }
        unsafe { record_status(self, status) }
    }
    /// # Safety
    /// Record is writable compiler-probed storage; bound NameMap is canonical/live.
    pub unsafe fn export(&mut self, record: *mut c_void) -> i32 {
        let status = unsafe {
            linker_record_archive::serialize_export((self as *mut Self).cast(), record, &RECORD)
        };
        unsafe { record_status(self, status) }
    }
    /// # Safety
    /// Record is writable compiler-probed storage; bound NameMap is canonical/live.
    pub unsafe fn import(&mut self, record: *mut c_void) -> i32 {
        let status = unsafe {
            linker_record_archive::serialize_import((self as *mut Self).cast(), record, &RECORD)
        };
        unsafe { record_status(self, status) }
    }

    unsafe fn run(
        raw: *mut Self,
        action: Action,
        argument: i64,
        data: *mut c_void,
    ) -> Result<i32, i32> {
        let closing = matches!(action, Action::CloseReader | Action::CloseWriter);
        if unsafe { (*raw).closed }
            && matches!(
                action,
                Action::SeekReader | Action::SeekWriter | Action::Read | Action::Write
            )
        {
            return Err(unsafe { fail(raw, ArchiveError::Closed) });
        }
        let services = Hp2UnixArchiveServices {
            context: raw.cast(),
            call: unix_leaf,
            tchar_width: size_of::<Tchar>() as u32,
        };
        let result = unsafe {
            unix_file_archive::run(
                ptr::addr_of_mut!((*raw).state),
                services,
                action,
                argument,
                data,
            )
        };
        // A failed virtual flush stops the shared policy before fclose. Ownership
        // still ends on explicit close, including when an earlier error is sticky.
        if closing && unsafe { !(*raw).state.file.is_null() } {
            let file = unsafe { (*raw).state.file };
            unsafe {
                (*raw).state.file = ptr::null_mut();
                (*raw).closed = true;
            }
            if unsafe { fclose(file) } != 0 {
                let code = if unsafe { (*raw).loading } {
                    Failure::ReaderClose
                } else {
                    Failure::WriterClose
                };
                unsafe {
                    fail(
                        raw,
                        ArchiveError::Policy {
                            code: code as u32,
                            errno: io::Error::last_os_error().raw_os_error(),
                        },
                    );
                }
            }
        }
        if unsafe { (*raw).has_throwing_error() } {
            Err(1)
        } else {
            result
        }
    }
}

impl Drop for RuntimeArchive {
    fn drop(&mut self) {
        // Explicit close reports errors. Destruction releases the resource even
        // after an earlier archive failure, and never closes a failed fclose twice.
        if !self.closed && !self.state.file.is_null() {
            unsafe {
                fclose(self.state.file);
            }
            self.closed = true;
            self.state.file = ptr::null_mut();
        }
    }
}

unsafe fn fail(raw: *mut RuntimeArchive, error: ArchiveError) -> i32 {
    if raw.is_null() { return 1; }
    // A genuine thrown callback must not be hidden behind an earlier GNull
    // I/O flag. Preserve the first exception independently of soft failures.
    let soft = unsafe { (*raw).null_error_device } && matches!(&error, ArchiveError::Policy { .. });
    let replace = unsafe { (*raw).error.is_none()
        || (!soft && (*raw).null_error_device && matches!(&(*raw).error, Some(ArchiveError::Policy { .. }))) };
    if replace { unsafe { (*raw).error = Some(error); } }
    i32::from(unsafe { (*raw).has_throwing_error() })
}

unsafe fn record_status(raw: *mut RuntimeArchive, status: i32) -> i32 {
    if status != 0 {
        unsafe {
            fail(raw, ArchiveError::Callback(status));
        }
    }
    if status == 0 && unsafe { (*raw).has_throwing_error() } {
        1
    } else {
        status
    }
}

unsafe extern "C" fn runtime_name_leaf(
    context: *mut c_void,
    leaf: Leaf,
    argument: i64,
    data: *mut c_void,
    output: *mut i64,
) -> i32 {
    if context.is_null() || output.is_null() {
        return 1;
    }
    let call = context.cast::<RuntimeNameCall<'_>>();
    let archive = unsafe { (*call).archive };
    unsafe { output.write(0); }
    match leaf {
        Leaf::IsError => unsafe { output.write(i64::from((*archive).error.is_some())); },
        Leaf::Fail => {
            return unsafe {
                fail(
                    archive,
                    ArchiveError::Policy { code: argument as u32, errno: None },
                )
            };
        }
        Leaf::ByteOrderSerialize => return unsafe { serialize_raw(archive, data, argument) },
        Leaf::NameText => {
            let name = unsafe { (*call).name_index.read_unaligned() };
            match unsafe { (*call).mapping.name_text(name) } {
                Ok(mut text) => {
                    if text.contains(&0) {
                        return unsafe { fail(archive, ArchiveError::MissingName) };
                    }
                    text.push(0);
                    if data.is_null() {
                        return unsafe { fail(archive, ArchiveError::MissingName) };
                    }
                    unsafe {
                        (*call).text = text;
                        data.cast::<*const c_void>().write((*call).text.as_ptr().cast());
                    }
                }
                Err(status) => return unsafe { record_status(archive, status) },
            }
        }
        Leaf::LoadNameScope => {
            if data.is_null() {
                return unsafe { fail(archive, ArchiveError::MissingName) };
            }
            let mut length = 0usize;
            while length < crate::unix_file_archive::UNIX_ARCHIVE_NAME_SIZE
                && unsafe { *data.cast::<u32>().add(length) } != 0
            {
                length += 1;
            }
            if length == crate::unix_file_archive::UNIX_ARCHIVE_NAME_SIZE {
                return unsafe { fail(archive, ArchiveError::MissingName) };
            }
            let text = unsafe { std::slice::from_raw_parts(data.cast::<u32>(), length) };
            match unsafe { (*call).mapping.add_name(text) } {
                Ok(name) => {
                    unsafe { (*call).loaded = Some(name); }
                    let services = Hp2UnixArchiveServices {
                        context, call: runtime_name_leaf, tchar_width: size_of::<Tchar>() as u32,
                    };
                    let published = unsafe { unix_file_archive::publish_loaded_name(services) };
                    unsafe { (*call).loaded = None; }
                    if let Err(status) = published { return unsafe { record_status(archive, status) }; }
                }
                Err(status) => return unsafe { record_status(archive, status) },
            }
        }
        Leaf::LoadedNameValid => unsafe { output.write(i64::from((*call).loaded.is_some())); },
        Leaf::PublishName => {
            let Some(name) = (unsafe { (*call).loaded }) else {
                return unsafe { fail(archive, ArchiveError::MissingName) };
            };
            unsafe { (*call).name_index.write_unaligned(name); }
        }
        _ => return unsafe { fail(archive, ArchiveError::MissingName) },
    }
    0
}

unsafe extern "C" fn unix_leaf(
    context: *mut c_void,
    leaf: Leaf,
    argument: i64,
    data: *mut c_void,
    output: *mut i64,
) -> i32 {
    let raw = context.cast::<RuntimeArchive>();
    unsafe {
        *output = 0;
        match leaf {
            Leaf::InitReader => (*raw).loading = true,
            Leaf::InitWriter => (*raw).loading = false,
            Leaf::IsError => *output = (*raw).error.is_some() as i64,
            Leaf::Fail => {
                // Range/EOF policy failures do not own errno; reporting the
                // thread's last unrelated OS error would invent a cause.
                let os_failure = [
                    Failure::ReaderSeek,
                    Failure::ReaderClose,
                    Failure::ReaderRead,
                    Failure::WriterSeek,
                    Failure::WriterClose,
                    Failure::WriterWrite,
                    Failure::WriterFlush,
                ]
                .iter()
                .any(|failure| *failure as i64 == argument);
                let errno = if os_failure {
                    io::Error::last_os_error().raw_os_error()
                } else {
                    None
                };
                return fail(
                    raw,
                    ArchiveError::Policy {
                        code: argument as u32,
                        errno,
                    },
                );
            }
            Leaf::Seek => *output = fseeko((*raw).state.file, argument, 0) as i64,
            Leaf::Read => *output = fread(data, 1, argument as usize, (*raw).state.file) as i64,
            Leaf::Write => *output = fwrite(data, 1, argument as usize, (*raw).state.file) as i64,
            Leaf::FileError => *output = ferror((*raw).state.file) as i64,
            Leaf::Flush => *output = fflush((*raw).state.file) as i64,
            Leaf::Close => {
                *output = fclose((*raw).state.file) as i64;
                // fclose consumes FILE even when it reports an error.
                (*raw).state.file = ptr::null_mut();
                (*raw).closed = true;
            }
            Leaf::VirtualFlush => {
                return RuntimeArchive::run(raw, Action::Flush, 0, ptr::null_mut())
                    .err()
                    .unwrap_or(0);
            }
            Leaf::VirtualClose => {
                return RuntimeArchive::run(
                    raw,
                    if (*raw).loading {
                        Action::CloseReader
                    } else {
                        Action::CloseWriter
                    },
                    0,
                    ptr::null_mut(),
                )
                .err()
                .unwrap_or(0);
            }
            Leaf::ByteOrderSerialize => return serialize_raw(raw, data, argument),
            // File archives use linker-local compact FNames, not the unrelated
            // Unix standalone name wire operation. That operation is not exposed.
            Leaf::NameText | Leaf::LoadNameScope | Leaf::LoadedNameValid | Leaf::PublishName => {
                unreachable!("standalone name operation on package archive")
            }
        }
    }
    0
}

unsafe fn serialize_raw(raw: *mut RuntimeArchive, data: *mut c_void, bytes: i64) -> i32 {
    unsafe {
        RuntimeArchive::run(
            raw,
            if (*raw).loading {
                Action::Read
            } else {
                Action::Write
            },
            bytes,
            data,
        )
    }
    .err()
    .unwrap_or(0)
}
unsafe extern "C" fn loading(ar: *mut c_void) -> i32 {
    unsafe { (*ar.cast::<RuntimeArchive>()).loading as i32 }
}
unsafe extern "C" fn saving(ar: *mut c_void) -> i32 {
    unsafe { (!(*ar.cast::<RuntimeArchive>()).loading) as i32 }
}
pub unsafe extern "C" fn serialize_byte(ar: *mut c_void, value: *mut u8) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 1) }
}
pub unsafe extern "C" fn serialize_word(ar: *mut c_void, value: *mut u16) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 2) }
}
pub unsafe extern "C" fn serialize_int(ar: *mut c_void, value: *mut i32) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 4) }
}
pub unsafe extern "C" fn serialize_dword(ar: *mut c_void, value: *mut u32) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 4) }
}
pub unsafe extern "C" fn serialize_qword(ar: *mut c_void, value: *mut u64) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 8) }
}
pub unsafe extern "C" fn serialize_index(ar: *mut c_void, value: *mut i32) -> i32 {
    let status = unsafe { compact_index::serialize(ar, value, &COMPACT) };
    unsafe { record_status(ar.cast(), status) }
}
unsafe extern "C" fn compact_mismatch(_value: i32, _original: i32) -> i32 {
    1
}
const COMPACT: Hp2CompactIndexServices = Hp2CompactIndexServices {
    archive_is_loading: loading,
    archive_is_saving: saving,
    archive_serialize_int: serialize_int,
    archive_serialize_byte: serialize_byte,
    archive_mismatch: compact_mismatch,
};

/// # Safety
/// Array is initialized compiler-probed raw storage; no typed element is dropped here.
pub unsafe fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}
unsafe extern "C" fn heap_realloc(
    context: *mut c_void,
    data: *mut c_void,
    bytes: usize,
    _max: *const i32,
    _width: i32,
    _tag: *const c_void,
    output: *mut *mut c_void,
) -> i32 {
    if bytes > i32::MAX as usize {
        return unsafe { fail(context.cast(), ArchiveError::Size) };
    }
    if bytes == 0 {
        unsafe {
            free(data);
            output.write(ptr::null_mut());
        }
        return 0;
    }
    let next = unsafe { realloc(data, bytes) };
    if next.is_null() {
        return unsafe { fail(context.cast(), ArchiveError::Allocation) };
    }
    unsafe {
        output.write(next);
    }
    0
}
unsafe extern "C" fn checked_size(context: *mut c_void, bytes: usize, output: *mut i32) -> i32 {
    if bytes > i32::MAX as usize {
        return unsafe { fail(context.cast(), ArchiveError::Size) };
    }
    unsafe {
        output.write(bytes as i32);
    }
    0
}
unsafe extern "C" fn heap_free(_context: *mut c_void, data: *mut c_void) -> i32 {
    unsafe {
        free(data);
    }
    0
}
pub fn array_services(context: *mut RuntimeArchive) -> Hp2ArrayServices {
    Hp2ArrayServices {
        context: context.cast(),
        realloc: heap_realloc,
        checked_int_size: checked_size,
        free: heap_free,
    }
}

unsafe fn resize_array(array: *mut TArrayRaw, count: i32, width: i32) -> i32 {
    if count < 0 {
        return 1;
    }
    let slots = unsafe { array_slots(array) };
    let (num, max) = unsafe { (slots.num.read(), slots.max.read()) };
    let result = unsafe {
        array_storage::set_exact(
            slots,
            count,
            width,
            ptr::null(),
            array_services(ptr::null_mut()),
        )
    };
    if result.is_err() {
        // The canonical policy publishes Num/Max before realloc. Unlike a native
        // fatal allocator, this backend returns failure, so retain valid storage.
        unsafe {
            slots.num.write(num);
            slots.max.write(max);
        }
    }
    result.err().unwrap_or(0)
}

unsafe extern "C" fn string_num(value: *const c_void) -> i32 {
    unsafe { ptr::addr_of!((*value.cast::<TArrayRaw>()).num).read_unaligned() }
}
unsafe extern "C" fn string_data(value: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*value.cast::<TArrayRaw>()).data).read_unaligned() }
}
unsafe extern "C" fn string_resize(value: *mut c_void, count: i32) -> i32 {
    unsafe { resize_array(value.cast(), count, size_of::<Tchar>() as i32) }
}
unsafe extern "C" fn string_set_num(value: *mut c_void, count: i32) {
    unsafe {
        ptr::addr_of_mut!((*value.cast::<TArrayRaw>()).num).write_unaligned(count);
    }
}
unsafe extern "C" fn string_empty(value: *mut c_void) -> i32 {
    unsafe {
        array_storage::empty(
            array_slots(value.cast()),
            0,
            size_of::<Tchar>() as i32,
            ptr::null(),
            array_services(ptr::null_mut()),
        )
    }
    .err()
    .unwrap_or(0)
}
unsafe extern "C" fn count_string(ar: *mut c_void, value: *mut c_void) -> i32 {
    let raw = ar.cast::<RuntimeArchive>();
    let string = value.cast::<TArrayRaw>();
    unsafe {
        (*raw).counted_num +=
            ptr::addr_of!((*string).num).read_unaligned() as u64 * size_of::<Tchar>() as u64;
        (*raw).counted_max +=
            ptr::addr_of!((*string).max).read_unaligned() as u64 * size_of::<Tchar>() as u64;
    }
    0
}
unsafe extern "C" fn wire_allocate(context: *mut c_void, count: i32, output: *mut *mut u16) -> i32 {
    let raw = context.cast::<RuntimeArchive>();
    if count < 0 {
        return unsafe { fail(raw, ArchiveError::Size) };
    }
    let count = count as usize;
    unsafe {
        if count > (*raw).wire.len() {
            let additional = count - (*raw).wire.len();
            if (*raw).wire.try_reserve(additional).is_err() {
                return fail(raw, ArchiveError::Allocation);
            }
            (*raw).wire.resize(count, 0);
        }
        output.write((*raw).wire.as_mut_ptr());
    }
    0
}
unsafe extern "C" fn to_ansi(value: i32) -> u8 {
    if (0..=255).contains(&value) {
        value as u8
    } else {
        b'?'
    }
}
unsafe extern "C" fn from_ansi(value: u8) -> i32 {
    value as i32
}
unsafe extern "C" fn string_overflow() -> i32 {
    1
}
unsafe fn string_services(raw: *mut RuntimeArchive) -> Hp2FStringArchiveServices {
    Hp2FStringArchiveServices {
        context: raw.cast(),
        archive_is_saving: saving,
        archive_is_loading: loading,
        archive_count_string: count_string,
        archive_serialize_index: serialize_index,
        archive_serialize_byte: serialize_byte,
        archive_serialize_unicode: serialize_word,
        string_num,
        string_data,
        string_resize_for_load: string_resize,
        string_set_num,
        string_empty,
        wire_allocate,
        tchar_to_ansi: to_ansi,
        ansi_to_tchar: from_ansi,
        string_length_overflow: string_overflow,
    }
}

unsafe extern "C" fn name_count(linker: *mut c_void) -> i32 {
    unsafe { ptr::addr_of!((*linker.cast::<ULinkerLoad>()).base.name_map.num).read_unaligned() }
}
unsafe extern "C" fn name_at(linker: *mut c_void, index: i32) -> *mut c_void {
    unsafe {
        ptr::addr_of!((*linker.cast::<ULinkerLoad>()).base.name_map.data)
            .read_unaligned()
            .cast::<FName>()
            .add(index as usize)
            .cast()
    }
}
unsafe extern "C" fn save_name_index(linker: *mut c_void, name: i32, output: *mut i32) -> i32 {
    let count = unsafe { name_count(linker) };
    for index in 0..count {
        if unsafe { (*name_at(linker, index).cast::<FName>()).index } == name {
            unsafe {
                output.write(index);
            }
            return 0;
        }
    }
    1
}
unsafe extern "C" fn bad_name_index(_linker: *mut c_void, _index: i32, _count: i32) -> i32 {
    1
}
pub unsafe extern "C" fn serialize_name(ar: *mut c_void, value: *mut c_void) -> i32 {
    let raw = ar.cast::<RuntimeArchive>();
    let linker = unsafe { (*raw).linker };
    if linker.is_null() {
        return unsafe { fail(raw, ArchiveError::MissingName) };
    }
    let status = unsafe {
        if (*raw).loading {
            linker_name_archive::load_name(ar, linker.cast(), value.cast(), &NAMES)
        } else {
            linker_name_archive::save_name(ar, linker.cast(), value.cast(), &NAMES)
        }
    };
    unsafe { record_status(raw, status) }
}
const NAMES: Hp2LinkerNameArchiveServices = Hp2LinkerNameArchiveServices {
    archive_serialize_index: serialize_index,
    name_map_count: name_count,
    name_map_at: name_at,
    save_name_index,
    bad_name_index,
};
const RECORD: Hp2LinkerRecordArchiveServices = Hp2LinkerRecordArchiveServices {
    archive_is_loading: loading,
    archive_compact_index: serialize_index,
    archive_int: serialize_int,
    archive_dword: serialize_dword,
    archive_name: serialize_name,
};

unsafe extern "C" fn serialize_guid(ar: *mut c_void, guid: *mut c_void) -> i32 {
    unsafe { package_summary_archive::serialize_guid(ar, guid, &summary_services(ptr::null())) }
}
unsafe extern "C" fn archive_tell(ar: *mut c_void, position: *mut i32) -> i32 {
    let result = unsafe {
        RuntimeArchive::run(
            ar.cast(),
            if (*ar.cast::<RuntimeArchive>()).loading {
                Action::TellReader
            } else {
                Action::TellWriter
            },
            0,
            ptr::null_mut(),
        )
    };
    match result {
        Ok(value) => {
            // SAFETY: The summary owner supplies writable position storage.
            unsafe { position.write(value) };
            0
        }
        Err(code) => code,
    }
}
unsafe extern "C" fn archive_seek(ar: *mut c_void, position: i32) -> i32 {
    unsafe {
        RuntimeArchive::run(
            ar.cast(),
            if (*ar.cast::<RuntimeArchive>()).loading {
                Action::SeekReader
            } else {
                Action::SeekWriter
            },
            position as i64,
            ptr::null_mut(),
        )
    }
    .err()
    .unwrap_or(0)
}
fn summary_services(arrays: *const Hp2ArrayServices) -> Hp2PackageSummaryArchiveServices {
    Hp2PackageSummaryArchiveServices {
        archive_is_loading: loading,
        archive_serialize_int: serialize_int,
        archive_serialize_dword: serialize_dword,
        archive_serialize_guid: serialize_guid,
        archive_tell,
        archive_seek,
        arrays,
        array_tag: ptr::null(),
    }
}
