//! Binary/text file persistence policy from `Core/Src/UnMisc.cpp`.
//! Native adapters retain archive dispatch and typed storage. Canonical text saves
//! use the same encoding/close/delete policy with a Rust filesystem provider.

use crate::ansi::Tchar;
use std::ffi::c_void;
use std::ptr::null_mut;

/// Every fallible callback returns zero or a captured native exception status.
/// The context owns the filename, manager, current archive, and temporary byte
/// array. `destroy` deletes the archive without an explicit Close. `binary`
/// applies TArray::Set only when resize is nonzero, then returns element zero
/// (including the native bounds behavior for empty arrays). `text` empties and
/// adds exactly count TCHARs. Returned storage stays live through the call.
#[repr(C)]
pub struct Hp2FilePersistenceServices {
    pub open: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub size: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub serialize: unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32,
    pub is_error: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub close: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub destroy: unsafe extern "C" fn(*mut c_void) -> i32,
    pub delete_file: unsafe extern "C" fn(*mut c_void) -> i32,
    pub scratch: unsafe extern "C" fn(*mut c_void, i32, *mut *mut u8) -> i32,
    pub binary: unsafe extern "C" fn(*mut c_void, i32, i32, *mut *mut u8) -> i32,
    pub text: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub from_ansi: unsafe extern "C" fn(u8) -> u32,
    pub from_unicode: unsafe extern "C" fn(u16) -> u32,
    pub to_ansi: unsafe extern "C" fn(u32) -> u8,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// Load a binary array, preserving the legacy absence of size/error checks.
/// # Safety
/// Services and context must satisfy Hp2FilePersistenceServices for a mutable
/// native byte array. Native exceptions must be captured, never unwind into Rust.
pub unsafe fn load_array(s: &Hp2FilePersistenceServices, c: *mut c_void) -> Result<i32, i32> {
    unsafe { load_array_with(&mut NativeLoadArray { services: s, context: c }) }
}

/// Typed leaves for the same binary-load transaction over an owning archive.
pub trait LoadArrayOps {
    type Error;
    fn open(&mut self) -> Result<i32, Self::Error>;
    fn size(&mut self) -> Result<i32, Self::Error>;
    /// Returns count writable bytes retained until serialize completes.
    unsafe fn binary(&mut self, count: i32) -> Result<*mut u8, Self::Error>;
    /// Initializes the requested bytes or preserves the archive's error state.
    unsafe fn serialize(&mut self, bytes: *mut u8, count: i32) -> Result<(), Self::Error>;
    fn close(&mut self) -> Result<i32, Self::Error>;
    fn destroy(&mut self) -> Result<(), Self::Error>;
}

/// Runs the native open/size/resize/read/close/delete sequence without replacing
/// an owning Rust archive with an FArchive vtable.
///
/// # Safety
/// Operations must retain binary storage through serialize and the archive
/// through destroy; callbacks may not reenter this same adapter.
pub unsafe fn load_array_with<O: LoadArrayOps>(ops: &mut O) -> Result<i32, O::Error> {
    if ops.open()? == 0 { return Ok(0); }
    let size = ops.size()?;
    let bytes = unsafe { ops.binary(size)? };
    unsafe { ops.serialize(bytes, size)?; }
    let closed = ops.close()?;
    ops.destroy()?;
    Ok(closed)
}

struct NativeLoadArray<'a> { services: &'a Hp2FilePersistenceServices, context: *mut c_void }
impl LoadArrayOps for NativeLoadArray<'_> {
    type Error = i32;
    fn open(&mut self) -> Result<i32, i32> {
        let mut value=0;status(unsafe{(self.services.open)(self.context,0,&mut value)})?;Ok(value)
    }
    fn size(&mut self) -> Result<i32, i32> {
        let mut value=0;status(unsafe{(self.services.size)(self.context,&mut value)})?;Ok(value)
    }
    unsafe fn binary(&mut self, count:i32) -> Result<*mut u8,i32> {
        let mut value=null_mut();status(unsafe{(self.services.binary)(self.context,count,1,&mut value)})?;Ok(value)
    }
    unsafe fn serialize(&mut self, bytes:*mut u8, count:i32) -> Result<(),i32> {
        status(unsafe{(self.services.serialize)(self.context,bytes,count)})
    }
    fn close(&mut self) -> Result<i32,i32> {
        let mut value=0;status(unsafe{(self.services.close)(self.context,&mut value)})?;Ok(value)
    }
    fn destroy(&mut self) -> Result<(),i32> { status(unsafe{(self.services.destroy)(self.context)}) }
}

/// Save a binary array. As in native code, writer errors and Close are not queried.
/// # Safety
/// Services/context must reference a byte array with the supplied native length.
pub unsafe fn save_array(
    s: &Hp2FilePersistenceServices,
    c: *mut c_void,
    length: i32,
) -> Result<i32, i32> {
    unsafe {
        let mut opened = 0;
        status((s.open)(c, 1, &mut opened))?;
        if opened == 0 {
            return Ok(0);
        }
        let mut bytes = null_mut();
        status((s.binary)(c, length, 0, &mut bytes))?;
        status((s.serialize)(c, bytes, length))?;
        status((s.destroy)(c))?;
        Ok(1)
    }
}
pub(crate) fn decode_ini_text(bytes: &[u8]) -> Option<Vec<Tchar>> {
    let encoding = ini_encoding(bytes)?;
    let mut output = Vec::with_capacity(encoding.1);
    visit_decoded_ini_text(bytes, encoding.0, u32::from, u32::from, |_, unit| output.push(unit as Tchar));
    Some(output)
}

pub fn decode_text_file(bytes: &[u8]) -> Option<Vec<Tchar>> {
    decode_ini_text(bytes)
}

type IniEncoding = (Option<bool>, usize);

fn ini_encoding(bytes: &[u8]) -> Option<IniEncoding> {
    if bytes.starts_with(&[0xff, 0xfe]) {
        Some((Some(true), decoded_length(&bytes[2..], true)? as usize))
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        Some((Some(false), decoded_length(&bytes[2..], false)? as usize))
    } else {
        Some((None, bytes.len()))
    }
}

fn visit_decoded_ini_text(
    bytes: &[u8],
    endian: Option<bool>,
    mut from_ansi: impl FnMut(u8) -> u32,
    mut from_unicode: impl FnMut(u16) -> u32,
    mut emit: impl FnMut(usize, u32),
) {
    if let Some(little) = endian {
        let body = &bytes[2..];
        let mut offset = 0;
        let mut index = 0;
        while offset < body.len() {
            let first = unit(body, offset, little);
            offset += 2;
            let scalar = if (0xd800..=0xdbff).contains(&first) {
                let second = unit(body, offset, little);
                offset += 2;
                0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
            } else {
                from_unicode(first)
            };
            emit(index, scalar);
            index += 1;
        }
    } else {
        for (index, &byte) in bytes.iter().enumerate() {
            emit(index, from_ansi(byte));
        }
    }
}

pub(crate) fn encode_ini_text(text: &[Tchar]) -> Option<Vec<u8>> {
    let (units, unicode) = encoded_unit_count(text)?;
    let size = if unicode { 2 + units as usize * 2 } else { text.len() };
    let mut output = Vec::with_capacity(size);
    encode_ini_text_into(text, units, unicode, |unit| unit as u8, |byte| output.push(byte));
    Some(output)
}

fn encode_ini_text_into(
    text: &[Tchar],
    units: i32,
    unicode: bool,
    mut to_ansi: impl FnMut(Tchar) -> u8,
    mut emit: impl FnMut(u8),
) {
    if unicode {
        emit(0xff);
        emit(0xfe);
        for &unit in text {
            let mut encoded = [0; 2];
            for value in char::from_u32(unit as u32).expect("validated TCHAR scalar").encode_utf16(&mut encoded).iter() {
                for byte in value.to_le_bytes() { emit(byte); }
            }
        }
    } else {
        for &unit in text.iter().take(units as usize) {
            emit(to_ansi(unit));
        }
    }
}

fn unit(bytes: &[u8], offset: usize, little: bool) -> u16 {
    let pair = [bytes[offset], bytes[offset + 1]];
    if little {
        u16::from_le_bytes(pair)
    } else {
        u16::from_be_bytes(pair)
    }
}

// The complete validation pass precedes any mutation of the destination FString.
fn decoded_length(bytes: &[u8], little: bool) -> Option<i32> {
    if bytes.len() & 1 != 0 {
        return None;
    }
    let mut offset = 0;
    let mut count = 0i32;
    while offset < bytes.len() {
        let first = unit(bytes, offset, little);
        offset += 2;
        if (0xd800..=0xdbff).contains(&first) {
            if offset == bytes.len() || !(0xdc00..=0xdfff).contains(&unit(bytes, offset, little)) {
                return None;
            }
            offset += 2;
        } else if (0xdc00..=0xdfff).contains(&first) {
            return None;
        }
        if count == i32::MAX - 1 {
            return None;
        }
        count += 1;
    }
    Some(count)
}

/// Load ANSI or BOM-selected UTF-16, leaving the destination intact on I/O or
/// UTF-16 validation failure. UTF-8 is intentionally treated as ANSI bytes.
/// # Safety
/// Services/context must reference a mutable FString and valid native storage.
pub unsafe fn load_string(s: &Hp2FilePersistenceServices, c: *mut c_void) -> Result<i32, i32> {
    unsafe {
        let mut opened = 0;
        status((s.open)(c, 0, &mut opened))?;
        if opened == 0 {
            return Ok(0);
        }
        let mut size = 0;
        status((s.size)(c, &mut size))?;
        let mut closed = 0;
        if !(0..=i32::MAX - 2).contains(&size) {
            status((s.close)(c, &mut closed))?;
            status((s.destroy)(c))?;
            return Ok(0);
        }
        let mut buffer = null_mut();
        status((s.scratch)(c, size + 2, &mut buffer))?;
        if size != 0 {
            status((s.serialize)(c, buffer, size))?;
        }
        let mut error = 0;
        status((s.is_error)(c, &mut error))?;
        status((s.close)(c, &mut closed))?;
        status((s.destroy)(c))?;
        if error != 0 || closed == 0 {
            return Ok(0);
        }
        *buffer.add(size as usize) = 0;
        *buffer.add(size as usize + 1) = 0;
        let bytes = std::slice::from_raw_parts(buffer, size as usize);
        let Some((endian, count)) = ini_encoding(bytes) else {
            return Ok(0);
        };
        let count = i32::try_from(count).map_err(|_| 1)?;
        let mut output = null_mut();
        status((s.text)(c, count + 1, &mut output))?;
        let output = output.cast::<Tchar>();
        visit_decoded_ini_text(
            bytes,
            endian,
            // SAFETY: callbacks satisfy their documented conversion contracts;
            // output is the writable FString buffer allocated above.
            |byte| unsafe { (s.from_ansi)(byte) },
            |unit| unsafe { (s.from_unicode)(unit) },
            |index, unit| unsafe { *output.add(index) = unit as Tchar },
        );
        *output.add(count as usize) = 0;
        Ok(1)
    }
}

// MAXINT-sized serialized storage includes its two-byte BOM, even for inputs
// which ultimately select ANSI (the original validates this before selection).
fn encoded_unit_count(text: &[Tchar]) -> Option<(i32, bool)> {
    let mut units = 0;
    let mut unicode = false;
    for &ch in text {
        let scalar = ch as u32;
        if scalar > 0x10ffff || (0xd800..=0xdfff).contains(&scalar) {
            return None;
        }
        let added = if scalar > 0xffff { 2 } else { 1 };
        if units > (i32::MAX - 2) / 2 - added {
            return None;
        }
        units += added;
        unicode |= scalar > 0xff;
    }
    Some((units, unicode))
}

/// Validate and encode TCHAR32 text before opening the writer. Failed writes
/// explicitly Close, destroy, then delete the destination, in that order.
/// # Safety
/// Text must cover length native TCHARs (including embedded NULs if present).
/// Services/context must retain the temporary array until this call returns.
pub unsafe fn save_string(
    s: &Hp2FilePersistenceServices,
    c: *mut c_void,
    text: *const Tchar,
    length: i32,
) -> Result<i32, i32> {
    unsafe { save_string_with(&mut NativeSaveString { services: s, context: c }, text, length) }
}

trait SaveStringOps {
    type Error;
    unsafe fn scratch(&mut self, count: i32) -> Result<*mut u8, Self::Error>;
    fn to_ansi(&mut self, value: u32) -> u8;
    fn open(&mut self) -> Result<bool, Self::Error>;
    unsafe fn serialize(&mut self, data: *mut u8, count: i32) -> Result<(), Self::Error>;
    fn is_error(&mut self) -> Result<bool, Self::Error>;
    fn close(&mut self) -> Result<bool, Self::Error>;
    fn destroy(&mut self) -> Result<(), Self::Error>;
    fn delete_file(&mut self) -> Result<(), Self::Error>;
}

unsafe fn save_string_with<O: SaveStringOps>(
    ops: &mut O, text: *const Tchar, length: i32,
) -> Result<i32, O::Error> {
    if length == 0 { return Ok(0); }
    unsafe {
        let text = std::slice::from_raw_parts(text, length as usize);
        let Some((units, unicode)) = encoded_unit_count(text) else { return Ok(0); };
        let size = if unicode { 2 + units * 2 } else { length };
        let buffer = ops.scratch(size)?;
        let mut offset = 0;
        encode_ini_text_into(text, units, unicode, |unit| ops.to_ansi(unit as u32), |byte| {
            buffer.add(offset).write(byte);
            offset += 1;
        });
        if !ops.open()? { return Ok(0); }
        ops.serialize(buffer, size)?;
        let error = ops.is_error()?;
        let closed = ops.close()?;
        ops.destroy()?;
        let success = !error && closed;
        if !success { ops.delete_file()?; }
        Ok(i32::from(success))
    }
}

struct NativeSaveString<'a> { services: &'a Hp2FilePersistenceServices, context: *mut c_void }
impl SaveStringOps for NativeSaveString<'_> {
    type Error = i32;
    unsafe fn scratch(&mut self, count: i32) -> Result<*mut u8, i32> {
        let mut data = null_mut();
        status(unsafe { (self.services.scratch)(self.context, count, &mut data) })?;
        Ok(data)
    }
    fn to_ansi(&mut self, value: u32) -> u8 { unsafe { (self.services.to_ansi)(value) } }
    fn open(&mut self) -> Result<bool, i32> {
        let mut opened = 0; status(unsafe { (self.services.open)(self.context, 1, &mut opened) })?;
        Ok(opened != 0)
    }
    unsafe fn serialize(&mut self, data: *mut u8, count: i32) -> Result<(), i32> {
        status(unsafe { (self.services.serialize)(self.context, data, count) })
    }
    fn is_error(&mut self) -> Result<bool, i32> {
        let mut error = 0; status(unsafe { (self.services.is_error)(self.context, &mut error) })?;
        Ok(error != 0)
    }
    fn close(&mut self) -> Result<bool, i32> {
        let mut closed = 0; status(unsafe { (self.services.close)(self.context, &mut closed) })?;
        Ok(closed != 0)
    }
    fn destroy(&mut self) -> Result<(), i32> { status(unsafe { (self.services.destroy)(self.context) }) }
    fn delete_file(&mut self) -> Result<(), i32> { status(unsafe { (self.services.delete_file)(self.context) }) }
}

/// Saves counted TCHAR32 text with appSaveStringToFile's ANSI/UTF-16LE encoding,
/// empty/invalid-input preservation and failed-write deletion. Ordinary filesystem
/// failures return false, just like FFileManager and FArchive's boolean/error flags.
pub fn save_text_to_path(path: &std::path::Path, text: &[Tchar]) -> bool {
    let Ok(length) = i32::try_from(text.len()) else { return false; };
    let mut ops = FileSaveString { path, scratch: Vec::new(), file: None, failed: false };
    match unsafe { save_string_with(&mut ops, text.as_ptr(), length) } {
        Ok(value) => value != 0,
        Err(error) => match error {},
    }
}

struct FileSaveString<'a> {
    path: &'a std::path::Path,
    scratch: Vec<u8>,
    file: Option<std::fs::File>,
    failed: bool,
}
impl SaveStringOps for FileSaveString<'_> {
    type Error = std::convert::Infallible;
    unsafe fn scratch(&mut self, count: i32) -> Result<*mut u8, Self::Error> {
        // The encoder initializes every byte before serialize reads it; do not
        // zero-fill a temporary array that is immediately overwritten.
        self.scratch.reserve_exact(count as usize);
        Ok(self.scratch.as_mut_ptr())
    }
    fn to_ansi(&mut self, value: u32) -> u8 { value as u8 }
    fn open(&mut self) -> Result<bool, Self::Error> {
        match std::fs::File::create(self.path) {
            Ok(file) => { self.file = Some(file); Ok(true) }
            Err(_) => { self.failed = true; Ok(false) }
        }
    }
    unsafe fn serialize(&mut self, data: *mut u8, count: i32) -> Result<(), Self::Error> {
        use std::io::Write;
        let bytes = unsafe { std::slice::from_raw_parts(data, count as usize) };
        if self.file.as_mut().expect("open writer").write_all(bytes).is_err() { self.failed = true; }
        Ok(())
    }
    fn is_error(&mut self) -> Result<bool, Self::Error> { Ok(self.failed) }
    fn close(&mut self) -> Result<bool, Self::Error> {
        use std::os::fd::IntoRawFd;
        let descriptor = self.file.take().expect("open writer").into_raw_fd();
        // File writes are unbuffered. Observe close failure instead of losing it
        // in File::drop, so the shared policy deletes a failed destination.
        if unsafe { libc::close(descriptor) } != 0 { self.failed = true; }
        Ok(!self.failed)
    }
    fn destroy(&mut self) -> Result<(), Self::Error> { drop(self.file.take()); Ok(()) }
    fn delete_file(&mut self) -> Result<(), Self::Error> {
        // FFileManager::Delete's boolean result is ignored by the source caller.
        if std::fs::remove_file(self.path).is_err() { self.failed = true; }
        Ok(())
    }
}
