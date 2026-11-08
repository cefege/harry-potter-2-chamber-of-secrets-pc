//! Unix archive state, strict filename wire codec and checked native I/O policy.
//! Native objects are accessed only through raw pointers: stdio and archive
//! callbacks may synchronously reenter the same archive and change its position.
use std::ffi::c_void;

pub const UNIX_ARCHIVE_NAME_SIZE: usize = 128;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UnixArchiveState {
    pub file: *mut c_void,
    pub size: i64,
    pub position: i64,
}

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2UnixArchiveAction {
    InitReader,
    InitWriter,
    SeekReader,
    SeekWriter,
    TellReader,
    TellWriter,
    SizeReader,
    CloseReader,
    CloseWriter,
    Read,
    Write,
    Flush,
    ReadName,
    WriteName,
    Destroy,
}

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2UnixArchiveLeaf {
    InitReader,
    InitWriter,
    IsError,
    Fail,
    Seek,
    Read,
    Write,
    FileError,
    Flush,
    Close,
    VirtualFlush,
    VirtualClose,
    ByteOrderSerialize,
    NameText,
    LoadNameScope,
    LoadedNameValid,
    PublishName,
}

/// Exact native messages are selected here, formatted by the native error device.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2UnixArchiveFailure {
    ReaderSeekInvalid,
    ReaderSeek,
    ReaderPosition,
    ReaderSize,
    ReaderClose,
    ReaderRange,
    ReaderEnd,
    ReaderRead,
    UnexpectedEnd,
    NameLength,
    NameText,
    NameInvalid,
    WriterSeekInvalid,
    WriterSeek,
    WriterPosition,
    WriterClose,
    WriterRange,
    WriterWrite,
    WriterFlush,
    NameEncode,
}

/// Invocation-local checked services. Nonzero callback status stops all work.
/// `data` is operation-specific synchronous storage, never retained by a leaf.
/// Every native exception and guard jump is contained inside the native leaf.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UnixArchiveServices {
    pub context: *mut c_void,
    pub call:
        unsafe extern "C" fn(*mut c_void, Hp2UnixArchiveLeaf, i64, *mut c_void, *mut i64) -> i32,
    pub tchar_width: u32,
}

impl Hp2UnixArchiveServices {
    fn invoke(
        self,
        leaf: Hp2UnixArchiveLeaf,
        argument: i64,
        data: *mut c_void,
    ) -> Result<i64, i32> {
        let mut value = 0;
        let status = unsafe { (self.call)(self.context, leaf, argument, data, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
    fn leaf(self, leaf: Hp2UnixArchiveLeaf) -> Result<i64, i32> {
        self.invoke(leaf, 0, std::ptr::null_mut())
    }
    fn fail(self, message: Hp2UnixArchiveFailure) -> Result<(), i32> {
        self.invoke(
            Hp2UnixArchiveLeaf::Fail,
            message as i64,
            std::ptr::null_mut(),
        )?;
        Ok(())
    }
}

unsafe fn source_unit(source: *const c_void, index: usize, width: u32) -> u32 {
    if width == 2 {
        unsafe { *source.cast::<u16>().add(index) as u32 }
    } else {
        unsafe { *source.cast::<u32>().add(index) }
    }
}

unsafe fn dest_unit(dest: *mut c_void, index: usize, width: u32, value: u32) {
    if width == 2 {
        unsafe { *dest.cast::<u16>().add(index) = value as u16 }
    } else {
        unsafe { *dest.cast::<u32>().add(index) = value }
    }
}

/// Strict native TCHAR to UTF-16, retaining the original partial-output rules.
/// # Safety
/// Source is terminated native text; wire and units are writable at their bounds.
/// The host TCHAR width is either two or four bytes.
pub unsafe fn encode_name(
    source: *const c_void,
    width: u32,
    wire: *mut u16,
    capacity: i32,
    units: *mut i32,
) -> bool {
    if source.is_null() || wire.is_null() || capacity <= 0 {
        return false;
    }
    unsafe {
        *units = 0;
    }
    let mut index = 0;
    loop {
        let mut point = unsafe { source_unit(source, index, width) };
        if point == 0 {
            break;
        }
        index += 1;
        if width == 2 && (0xd800..=0xdbff).contains(&point) {
            let low = unsafe { source_unit(source, index, width) };
            index += 1;
            if !(0xdc00..=0xdfff).contains(&low) || unsafe { *units } > capacity - 3 {
                return false;
            }
            unsafe {
                *wire.add(*units as usize) = point as u16;
                *units += 1;
                *wire.add(*units as usize) = low as u16;
                *units += 1;
            }
            continue;
        }
        if (0xd800..=0xdfff).contains(&point) {
            return false;
        }
        if point <= 0xffff {
            if unsafe { *units } > capacity - 2 {
                return false;
            }
            unsafe {
                *wire.add(*units as usize) = point as u16;
                *units += 1;
            }
        } else if point <= 0x10ffff {
            if unsafe { *units } > capacity - 3 {
                return false;
            }
            point -= 0x10000;
            unsafe {
                *wire.add(*units as usize) = (0xd800 + (point >> 10)) as u16;
                *units += 1;
                *wire.add(*units as usize) = (0xdc00 + (point & 0x3ff)) as u16;
                *units += 1;
            }
        } else {
            return false;
        }
    }
    unsafe {
        *wire.add(*units as usize) = 0;
        *units += 1;
    }
    true
}

/// Strict bounded UTF-16 to native TCHAR, including validation after embedded NUL.
/// # Safety
/// Wire holds `units` readable units and dest holds `capacity` native characters.
pub unsafe fn decode_name(
    wire: *const u16,
    units: i32,
    dest: *mut c_void,
    capacity: i32,
    width: u32,
) -> bool {
    if wire.is_null()
        || dest.is_null()
        || units <= 0
        || unsafe { *wire.add((units - 1) as usize) } != 0
        || capacity <= 0
    {
        return false;
    }
    let mut index = 0;
    let mut written = 0;
    while index < units - 1 {
        let mut point = unsafe { *wire.add(index as usize) as u32 };
        index += 1;
        if (0xd800..=0xdbff).contains(&point) {
            if index >= units - 1 {
                return false;
            }
            let low = unsafe { *wire.add(index as usize) as u32 };
            index += 1;
            if !(0xdc00..=0xdfff).contains(&low) {
                return false;
            }
            if width == 2 {
                if written > capacity - 3 {
                    return false;
                }
                unsafe {
                    dest_unit(dest, written as usize, width, point);
                    dest_unit(dest, (written + 1) as usize, width, low);
                }
                written += 2;
                continue;
            }
            point = 0x10000 + ((point - 0xd800) << 10) + (low - 0xdc00);
        } else if (0xdc00..=0xdfff).contains(&point) {
            return false;
        }
        if written > capacity - 2 {
            return false;
        }
        unsafe {
            dest_unit(dest, written as usize, width, point);
        }
        written += 1;
    }
    unsafe {
        dest_unit(dest, written as usize, width, 0);
    }
    true
}

/// Called while a directly constructed native LoadedName remains in lexical scope.
/// # Safety
/// Services refer to that live native FName and the caller's destination name.
pub unsafe fn publish_loaded_name(services: Hp2UnixArchiveServices) -> Result<(), i32> {
    if services.leaf(Hp2UnixArchiveLeaf::LoadedNameValid)? == 0 {
        services.fail(Hp2UnixArchiveFailure::NameInvalid)?;
    } else {
        services.leaf(Hp2UnixArchiveLeaf::PublishName)?;
    }
    Ok(())
}

unsafe fn name(services: Hp2UnixArchiveServices, loading: bool) -> Result<(), i32> {
    use Hp2UnixArchiveFailure as F;
    use Hp2UnixArchiveLeaf as L;
    let mut wire = [0u16; UNIX_ARCHIVE_NAME_SIZE];
    let mut units = 0i32;
    if !loading {
        let mut source: *const c_void = std::ptr::null();
        services.invoke(L::NameText, 0, (&mut source as *mut *const c_void).cast())?;
        if !unsafe {
            encode_name(
                source,
                services.tchar_width,
                wire.as_mut_ptr(),
                UNIX_ARCHIVE_NAME_SIZE as i32,
                &mut units,
            )
        } {
            return services.fail(F::NameEncode);
        }
    }
    let mut bytes = units * 2;
    services.invoke(L::ByteOrderSerialize, 4, (&mut bytes as *mut i32).cast())?;
    if loading {
        if services.leaf(L::IsError)? != 0 {
            return Ok(());
        }
        if bytes < 2 || bytes % 2 != 0 || bytes > (UNIX_ARCHIVE_NAME_SIZE * 2) as i32 {
            return services.fail(F::NameLength);
        }
        units = bytes / 2;
    }
    for index in 0..units as usize {
        // The reader intentionally completes the loop after ArIsError, whereas
        // the writer observes the live flag before each unit.
        if !loading && services.leaf(L::IsError)? != 0 {
            break;
        }
        services.invoke(
            L::ByteOrderSerialize,
            2,
            (&mut wire[index] as *mut u16).cast(),
        )?;
    }
    if loading {
        if services.leaf(L::IsError)? != 0 {
            return Ok(());
        }
        // u32 alignment also accommodates the two-byte TCHAR host contract.
        let mut dest = [0u32; UNIX_ARCHIVE_NAME_SIZE];
        if !unsafe {
            decode_name(
                wire.as_ptr(),
                units,
                dest.as_mut_ptr().cast(),
                UNIX_ARCHIVE_NAME_SIZE as i32,
                services.tchar_width,
            )
        } {
            return services.fail(F::NameText);
        }
        services.invoke(L::LoadNameScope, 0, dest.as_mut_ptr().cast())?;
    }
    Ok(())
}

/// Execute a native archive operation. No reference into `state` survives a leaf.
/// # Safety
/// State is embedded in the live native archive. Services and data satisfy the
/// action's native contract, and callbacks may mutate state through reentry.
pub unsafe fn run(
    state: *mut Hp2UnixArchiveState,
    services: Hp2UnixArchiveServices,
    action: Hp2UnixArchiveAction,
    argument: i64,
    data: *mut c_void,
) -> Result<i32, i32> {
    use Hp2UnixArchiveAction as A;
    use Hp2UnixArchiveFailure as F;
    use Hp2UnixArchiveLeaf as L;
    match action {
        A::InitReader | A::InitWriter => {
            let reader = matches!(action, A::InitReader);
            unsafe {
                state.write(Hp2UnixArchiveState {
                    file: data,
                    size: if reader { argument } else { 0 },
                    position: if reader { 0 } else { argument },
                });
            }
            services.leaf(if reader { L::InitReader } else { L::InitWriter })?;
        }
        A::SeekReader | A::SeekWriter => {
            let reader = matches!(action, A::SeekReader);
            if argument < 0 || (reader && argument > unsafe { (*state).size }) {
                services.fail(if reader {
                    F::ReaderSeekInvalid
                } else {
                    F::WriterSeekInvalid
                })?;
                return Ok(0);
            }
            if !reader {
                services.leaf(L::VirtualFlush)?;
                if services.leaf(L::IsError)? != 0 {
                    return Ok(0);
                }
            }
            if services.invoke(L::Seek, argument, std::ptr::null_mut())? != 0 {
                services.fail(if reader { F::ReaderSeek } else { F::WriterSeek })?;
            } else {
                unsafe {
                    (*state).position = argument;
                }
            }
        }
        A::TellReader | A::TellWriter | A::SizeReader => {
            let (value, failure) = match action {
                A::SizeReader => (unsafe { (*state).size }, F::ReaderSize),
                A::TellReader => (unsafe { (*state).position }, F::ReaderPosition),
                _ => (unsafe { (*state).position }, F::WriterPosition),
            };
            if value < 0 || value > i32::MAX as i64 {
                services.fail(failure)?;
                return Ok(-1);
            }
            return Ok(value as i32);
        }
        A::Read | A::Write => {
            if services.leaf(L::IsError)? != 0 {
                return Ok(0);
            }
            let reader = matches!(action, A::Read);
            let position = unsafe { (*state).position };
            if argument < 0
                || position < 0
                || position > i32::MAX as i64
                || argument > i32::MAX as i64 - position
            {
                services.fail(if reader {
                    F::ReaderRange
                } else {
                    F::WriterRange
                })?;
                return Ok(0);
            }
            if reader && position + argument > unsafe { (*state).size } {
                services.fail(F::ReaderEnd)?;
                return Ok(0);
            }
            if argument == 0 {
                return Ok(0);
            }
            let done = services.invoke(if reader { L::Read } else { L::Write }, argument, data)?;
            // stdio may reenter Seek/Serialize. Add to the position *after* it returns.
            unsafe {
                (*state).position = (*state).position.wrapping_add(done);
            }
            if done != argument {
                let failure = if !reader {
                    F::WriterWrite
                } else if services.leaf(L::FileError)? != 0 {
                    F::ReaderRead
                } else {
                    F::UnexpectedEnd
                };
                services.fail(failure)?;
            }
        }
        A::Flush => {
            if !unsafe { (*state).file }.is_null() && services.leaf(L::Flush)? != 0 {
                services.fail(F::WriterFlush)?;
            }
        }
        A::CloseReader | A::CloseWriter => {
            if !unsafe { (*state).file }.is_null() {
                let reader = matches!(action, A::CloseReader);
                if !reader {
                    services.leaf(L::VirtualFlush)?;
                }
                if services.leaf(L::Close)? != 0 {
                    services.fail(if reader {
                        F::ReaderClose
                    } else {
                        F::WriterClose
                    })?;
                }
                // A throwing close/failure logger leaves File non-null, as natively.
                unsafe {
                    (*state).file = std::ptr::null_mut();
                }
            }
            return Ok((services.leaf(L::IsError)? == 0) as i32);
        }
        A::ReadName => unsafe {
            name(services, true)?;
        },
        A::WriteName => unsafe {
            name(services, false)?;
        },
        A::Destroy => {
            if !unsafe { (*state).file }.is_null() {
                services.leaf(L::VirtualClose)?;
            }
        }
    }
    Ok(0)
}
