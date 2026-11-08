//! Borrowed byte readers and owned TArray byte writers, without an FArchive vtable.
//!
//! Cursor, growth and dispatch ordering belong to `array_archive`; compact and
//! FString wire formats belong to their existing codecs. This is only the byte,
//! compact-index and FString API, not an implementation of every archive virtual.
use crate::{
    ansi::Tchar,
    array_archive::{self, action},
    array_storage,
    compact_index::{self, Hp2CompactIndexServices},
    fstring_archive::{self, Hp2FStringArchiveServices},
    runtime_archive::{array_services, array_slots},
};
use hp2_abi::engine::generated::TArrayRaw;
use std::{ffi::c_void, ptr, slice};

pub struct RuntimeBufferArchive<'a> {
    input: Option<&'a [u8]>,
    bytes: TArrayRaw,
    position: i32,
    version: i32,
    error: Option<i32>,
    wire: Vec<u16>,
    counted_num: u64,
    counted_max: u64,
}

impl<'a> RuntimeBufferArchive<'a> {
    pub fn writer() -> Self {
        Self {
            input: None,
            bytes: TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
            position: 0,
            version: 79,
            error: None,
            wire: Vec::new(),
            counted_num: 0,
            counted_max: 0,
        }
    }

    pub fn reader(bytes: &'a [u8]) -> Self {
        let mut archive = Self::writer();
        archive.input = Some(bytes);
        if bytes.len() > i32::MAX as usize {
            archive.error = Some(1);
        }
        archive
    }

    pub fn set_version(&mut self, version: i32) {
        self.version = version;
    }
    pub fn version(&self) -> i32 {
        self.version
    }
    pub fn is_loading(&self) -> bool {
        self.input.is_some()
    }
    pub fn error(&self) -> Option<i32> {
        self.error
    }
    pub fn counted_bytes(&self) -> (u64, u64) {
        (self.counted_num, self.counted_max)
    }

    /// Writer seeks are unrestricted, as in FBufferWriter. A later serialization
    /// must satisfy its native signed-count and storage preconditions.
    pub fn seek(&mut self, position: i32) -> Result<(), i32> {
        let action = if self.is_loading() {
            action::BUFFER_SEEK_READ
        } else {
            action::BUFFER_SEEK_WRITE
        };
        unsafe { run(self, action, position, ptr::null_mut()) }
    }

    pub fn tell(&mut self) -> Result<i32, i32> {
        self.query(action::BUFFER_TELL)
    }
    pub fn total_size(&mut self) -> Result<i32, i32> {
        self.query(action::BUFFER_SIZE)
    }
    pub fn at_end(&mut self) -> Result<bool, i32> {
        self.query(action::BUFFER_AT_END).map(|value| value != 0)
    }

    fn query(&mut self, action: i32) -> Result<i32, i32> {
        let mut value = 0;
        unsafe { run(self, action, 0, (&mut value as *mut i32).cast()) }?;
        Ok(value)
    }

    pub fn as_bytes(&self) -> &[u8] {
        if let Some(input) = self.input {
            return input;
        }
        if self.bytes.num == 0 {
            return &[];
        }
        unsafe { slice::from_raw_parts(self.bytes.data.cast(), self.bytes.num as usize) }
    }

    /// # Safety
    /// Data covers `bytes` readable bytes when saving, writable bytes when loading,
    /// and does not overlap archive storage or state (the native memcpy contract).
    /// `bytes` is nonnegative; writer position, end and TArray growth must fit the
    /// native signed-count/capacity contract. Sources must survive writer realloc.
    /// No concurrent access or external reentry may invalidate these pointers.
    pub unsafe fn serialize(&mut self, data: *mut c_void, bytes: i32) -> i32 {
        unsafe { serialize_raw(self, data, bytes) }
    }

    /// # Safety
    /// Value is aligned, writable i32 storage, disjoint from archive storage/state.
    /// Only saving requires initialization. Writer storage obeys `serialize`'s
    /// signed-count/capacity contract through all codec calls.
    pub unsafe fn compact(&mut self, value: *mut i32) -> i32 {
        unsafe { serialize_index((self as *mut Self).cast(), value) }
    }

    /// # Safety
    /// String is initialized TArrayRaw TCHAR storage owned by `array_services`'s
    /// allocator. Saving requires initialized, terminated text. Its object/data
    /// must not alias archive state/storage, and must remain live across callbacks.
    /// Writer storage obeys `serialize`'s signed-count/capacity contract.
    pub unsafe fn string(&mut self, string: *mut TArrayRaw) -> i32 {
        let raw = self as *mut Self;
        if let Some(error) = unsafe { (*raw).error } {
            return error;
        }
        let services = string_services(raw);
        let status = unsafe { fstring_archive::serialize(raw.cast(), string.cast(), &services) };
        unsafe { record_status(raw, status) }
    }
}

impl Drop for RuntimeBufferArchive<'_> {
    fn drop(&mut self) {
        // Reader bytes are borrowed; only this separate writer allocation is freed.
        // The concrete libc free leaf cannot fail, including after an archive error.
        unsafe {
            array_storage::release(
                array_slots(ptr::addr_of_mut!(self.bytes)),
                array_services(ptr::null_mut()),
            )
            .unwrap();
        }
    }
}

unsafe fn record_status(raw: *mut RuntimeBufferArchive<'_>, status: i32) -> i32 {
    unsafe {
        if status != 0 && (*raw).error.is_none() {
            (*raw).error = Some(status);
        }
        (*raw).error.unwrap_or(0)
    }
}

unsafe fn run(
    raw: *mut RuntimeBufferArchive<'_>,
    action: i32,
    arg: i32,
    value: *mut c_void,
) -> Result<(), i32> {
    if let Some(error) = unsafe { (*raw).error } {
        return Err(error);
    }
    let status =
        unsafe { array_archive::run(raw.cast(), ptr::null_mut(), buffer_leaf, action, arg, value) }
            .err()
            .unwrap_or(0);
    match unsafe { record_status(raw, status) } {
        0 => Ok(()),
        error => Err(error),
    }
}

unsafe extern "C" fn buffer_leaf(
    object: *mut c_void,
    _archive: *mut c_void,
    op: i32,
    arg: i32,
    value: *mut c_void,
) -> i32 {
    let raw = object.cast::<RuntimeBufferArchive<'_>>();
    unsafe {
        match op {
            array_archive::BUFFER_NUM => {
                let count = match (*raw).input {
                    Some(input) => input.len() as i32,
                    None => (*raw).bytes.num,
                };
                value.cast::<i32>().write(count);
            }
            array_archive::POSITION => value.cast::<i32>().write((*raw).position),
            array_archive::SET_POSITION => (*raw).position = arg,
            array_archive::BUFFER_GROW => {
                let slots = array_slots(ptr::addr_of_mut!((*raw).bytes));
                let (num, max) = (slots.num.read(), slots.max.read());
                if let Err(error) =
                    array_storage::add(slots, arg, 1, ptr::null(), array_services(ptr::null_mut()))
                {
                    // The canonical policy publishes counts before realloc. Preserve
                    // the old allocation's valid extent when libc returns failure.
                    slots.num.write(num);
                    slots.max.write(max);
                    return error;
                }
                // Native seek-created gaps are unspecified. Initialize only gaps
                // for safe slice exposure; WRITE_ONE/MANY initializes the payload.
                let gap = (*raw).position - num;
                if gap > 0 {
                    ptr::write_bytes(
                        slots.data.read_unaligned().cast::<u8>().add(num as usize),
                        0,
                        gap as usize,
                    );
                }
            }
            array_archive::WRITE_ONE | array_archive::WRITE_MANY => {
                if arg != 0 {
                    ptr::copy_nonoverlapping(
                        value.cast::<u8>(),
                        (*raw).bytes.data.cast::<u8>().add((*raw).position as usize),
                        arg as usize,
                    );
                }
            }
            array_archive::READ_CHECK => {
                // These are the native reader assertion leaves, not a second cursor
                // policy. Checked arithmetic keeps malformed lengths from wrapping.
                let Some(end) = (*raw).position.checked_add(arg) else {
                    return 1;
                };
                let Some(input) = (*raw).input else {
                    return 1;
                };
                if arg < 0 || (*raw).position < 0 || end as usize > input.len() {
                    return 1;
                }
            }
            array_archive::SEEK_CHECK => {
                let Some(input) = (*raw).input else {
                    return 1;
                };
                if arg < 0 || arg as usize > input.len() {
                    return 1;
                }
            }
            array_archive::READ_ONE | array_archive::READ_MANY => {
                if arg != 0 {
                    let Some(input) = (*raw).input else {
                        return 1;
                    };
                    ptr::copy_nonoverlapping(
                        input.as_ptr().add((*raw).position as usize),
                        value.cast::<u8>(),
                        arg as usize,
                    );
                }
            }
            _ => return 1,
        }
    }
    0
}

unsafe fn serialize_raw(raw: *mut RuntimeBufferArchive<'_>, data: *mut c_void, bytes: i32) -> i32 {
    let action = if unsafe { (*raw).input.is_some() } {
        action::BUFFER_READ
    } else {
        action::BUFFER_WRITE
    };
    unsafe { run(raw, action, bytes, data) }.err().unwrap_or(0)
}
unsafe extern "C" fn loading(ar: *mut c_void) -> i32 {
    unsafe { (*ar.cast::<RuntimeBufferArchive<'_>>()).input.is_some() as i32 }
}
unsafe extern "C" fn saving(ar: *mut c_void) -> i32 {
    unsafe { (*ar.cast::<RuntimeBufferArchive<'_>>()).input.is_none() as i32 }
}
unsafe extern "C" fn serialize_byte(ar: *mut c_void, value: *mut u8) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 1) }
}
unsafe extern "C" fn serialize_word(ar: *mut c_void, value: *mut u16) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 2) }
}
unsafe extern "C" fn serialize_int(ar: *mut c_void, value: *mut i32) -> i32 {
    unsafe { serialize_raw(ar.cast(), value.cast(), 4) }
}
unsafe extern "C" fn serialize_index(ar: *mut c_void, value: *mut i32) -> i32 {
    let raw = ar.cast::<RuntimeBufferArchive<'_>>();
    if let Some(error) = unsafe { (*raw).error } {
        return error;
    }
    let status = unsafe { compact_index::serialize(ar, value, &COMPACT) };
    unsafe { record_status(raw, status) }
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

unsafe extern "C" fn string_num(value: *const c_void) -> i32 {
    unsafe { ptr::addr_of!((*value.cast::<TArrayRaw>()).num).read_unaligned() }
}
unsafe extern "C" fn string_data(value: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*value.cast::<TArrayRaw>()).data).read_unaligned() }
}
unsafe extern "C" fn string_resize(value: *mut c_void, count: i32) -> i32 {
    if count < 0 {
        return 1;
    }
    unsafe {
        let slots = array_slots(value.cast());
        let (num, max) = (slots.num.read(), slots.max.read());
        let result = array_storage::set_exact(
            slots,
            count,
            size_of::<Tchar>() as i32,
            ptr::null(),
            array_services(ptr::null_mut()),
        );
        if result.is_err() {
            slots.num.write(num);
            slots.max.write(max);
        }
        result.err().unwrap_or(0)
    }
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
    let raw = ar.cast::<RuntimeBufferArchive<'_>>();
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
    if count < 0 {
        return 1;
    }
    let raw = context.cast::<RuntimeBufferArchive<'_>>();
    let count = count as usize;
    unsafe {
        if count > (*raw).wire.len() {
            let additional = count - (*raw).wire.len();
            if (*raw).wire.try_reserve(additional).is_err() {
                return 1;
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
fn string_services(raw: *mut RuntimeBufferArchive<'_>) -> Hp2FStringArchiveServices {
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
