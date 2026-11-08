//! FString value policy over the existing packed native array slots.
//!
//! Engine-thread only. Slots stay live during callbacks; no Rust references to
//! native state survive reentry. Inputs must remain valid, terminated strings
//! unless explicitly counted. Reentrant callbacks may relocate slot-backed
//! storage but must preserve the pending operation's readable/writable ranges.
//! Raw external input buffers must remain valid until return. Overlapping pointer
//! assignment/append (other than exact pointer assignment identity) was undefined
//! in the native memcpy/strcpy implementation and is not supported. Split's
//! output aliasing is defined separately and deliberately observes live values.

use crate::ansi::{self, Tchar};
use crate::array_storage::{self as array, Hp2ArrayServices, Hp2ArraySlots};
use std::ffi::c_void;
use std::ptr;

const WIDTH: i32 = std::mem::size_of::<Tchar>() as i32;
static EMPTY: [Tchar; 1] = [0];

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2FStringStorageServices {
    pub context: *mut c_void,
    pub arrays: Hp2ArrayServices,
    pub string_tag: *const c_void,
    pub array_tag: *const c_void,
    pub case_char: unsafe extern "C" fn(*mut c_void, u32, i32, *mut u32) -> i32,
    pub bounds_error: unsafe extern "C" fn(*mut c_void) -> i32,
    pub set_tag: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub slots: unsafe extern "C" fn(*const c_void) -> Hp2ArraySlots,
    /// Native temporary lifetime + counted assignment, not substring policy.
    pub assign_range: unsafe extern "C" fn(*mut c_void, *const c_void, i32, i32) -> i32,
}

fn status(code: i32) -> Result<(), i32> {
    if code == 0 { Ok(()) } else { Err(code) }
}
unsafe fn data(s: Hp2ArraySlots) -> *mut Tchar {
    unsafe { s.data.read_unaligned().cast() }
}
unsafe fn text(s: Hp2ArraySlots) -> *const Tchar {
    unsafe {
        if s.num.read() == 0 {
            EMPTY.as_ptr()
        } else {
            data(s)
        }
    }
}
unsafe fn len(s: Hp2ArraySlots) -> i32 {
    unsafe {
        if s.num.read() == 0 {
            0
        } else {
            s.num.read() - 1
        }
    }
}
unsafe fn terminated_len(p: *const Tchar, arrays: Hp2ArrayServices) -> Result<i32, i32> {
    let mut n = 0;
    unsafe {
        while p.add(n).read() != 0 {
            n += 1;
        }
    }
    // appStrlen reports the INT contract only on overflow.
    if n > i32::MAX as usize {
        let mut out = 0;
        unsafe {
            status((arrays.checked_int_size)(arrays.context, n, &mut out))?;
        }
        Ok(out)
    } else {
        Ok(n as i32)
    }
}
unsafe fn checked_copy_size(s: Hp2ArraySlots, arrays: Hp2ArrayServices) -> Result<usize, i32> {
    let mut bytes = 0;
    unsafe {
        status((arrays.checked_int_size)(
            arrays.context,
            (s.num.read() as usize).wrapping_mul(WIDTH as usize),
            &mut bytes,
        ))?;
    }
    Ok(bytes as usize)
}

/// Construct/assign from terminated input (source null) or counted live slots.
/// Constructor calls also reallocate zero-count strings; assignments preserve
/// exact pointer/object identity and otherwise always use SetExact.
/// # Safety
/// See module contract. Destination is initialized except that object
/// self-assignment accepts uninitialized slots and does not read their payload.
pub unsafe fn assign(
    dst: Hp2ArraySlots,
    source: Option<Hp2ArraySlots>,
    input: *const Tchar,
    construct: bool,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe { assign_with_allocator(dst, source, input, construct, svc.arrays, svc.string_tag) }
}

/// The same FString assignment policy with only its actual allocation leaves.
/// # Safety
/// See `assign`. Allocations and existing destination storage belong to arrays.
pub unsafe fn assign_with_allocator(
    dst: Hp2ArraySlots,
    source: Option<Hp2ArraySlots>,
    input: *const Tchar,
    construct: bool,
    arrays: Hp2ArrayServices,
    string_tag: *const c_void,
) -> Result<(), i32> {
    unsafe {
        if !construct
            && source.map_or_else(|| data(dst).cast_const() == input, |s| s.data == dst.data)
        {
            return Ok(());
        }
        let count = if let Some(s) = source {
            s.num.read()
        } else {
            if input.read() == 0 {
                0
            } else {
                terminated_len(input, arrays)?.wrapping_add(1)
            }
        };
        array::set_exact(dst, count, WIDTH, string_tag, arrays)?;
        if dst.num.read() != 0 {
            let bytes = checked_copy_size(dst, arrays)?;
            // Resolve both buffers only after the potentially reentrant check.
            let src = source.map_or(input, |s| text(s));
            ptr::copy_nonoverlapping(src.cast::<u8>(), data(dst).cast::<u8>(), bytes);
        }
        Ok(())
    }
}

/// # Safety
/// See module contract; input cannot overlap destination storage.
pub unsafe fn append(
    dst: Hp2ArraySlots,
    input: *const Tchar,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe {
        let count = dst.num.read();
        if count != 0 {
            let n = terminated_len(input, svc.arrays)?;
            array::add(dst, n, WIDTH, svc.array_tag, svc.arrays)?;
            // strcpy reads through its terminator, not the destination count.
            ansi::strcpy(data(dst).add((count - 1) as usize), input);
        } else if input.read() != 0 {
            let n = terminated_len(input, svc.arrays)?;
            array::add(dst, n.wrapping_add(1), WIDTH, svc.array_tag, svc.arrays)?;
            ansi::strcpy(data(dst), input);
        }
        Ok(())
    }
}

/// # Safety
/// See [`append`]. Separator is the native platform's terminated PATH_SEPARATOR.
pub unsafe fn path_append(
    dst: Hp2ArraySlots,
    input: *const Tchar,
    separator: *const Tchar,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe {
        let count = dst.num.read();
        if count > 1 && data(dst).add((count - 2) as usize).read() != separator.read() {
            append(dst, separator, svc)?;
        }
        append(dst, input, svc)
    }
}

/// Native unsigned Mid policy, including negative Start/Count and wrapping End.
pub fn mid_bounds(length: i32, start: i32, count: i32) -> (i32, i32) {
    let end = start.wrapping_add(count) as u32;
    let start = (start as u32).min(length as u32);
    let end = end.clamp(start, length as u32);
    (start as i32, (end - start) as i32)
}

/// Construct a counted, NUL-padded substring (zero count remains unallocated).
/// # Safety
/// Destination starts empty. Input is terminated or readable for count chars.
pub unsafe fn counted(
    dst: Hp2ArraySlots,
    input: *const Tchar,
    count: i32,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe {
        array::set_exact(
            dst,
            if count == 0 { 0 } else { count.wrapping_add(1) },
            WIDTH,
            svc.string_tag,
            svc.arrays,
        )?;
        if dst.num.read() != 0 {
            ansi::strncpy(data(dst), input, count.wrapping_add(1));
        }
        Ok(())
    }
}

// Slot-backed sources are resolved again after allocation/check callbacks.
unsafe fn range_value(
    dst: Hp2ArraySlots,
    src: Hp2ArraySlots,
    start: i32,
    count: Option<i32>,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe {
        let length = if let Some(count) = count {
            count
        } else {
            terminated_len(text(src).add(start as usize), svc.arrays)?
        };
        let total = if length == 0 {
            0
        } else {
            length.wrapping_add(1)
        };
        array::set_exact(dst, total, WIDTH, svc.string_tag, svc.arrays)?;
        if dst.num.read() != 0 {
            if count.is_some() {
                ansi::strncpy(
                    data(dst),
                    text(src).add(start as usize),
                    length.wrapping_add(1),
                );
            } else {
                let bytes = checked_copy_size(dst, svc.arrays)?;
                ptr::copy_nonoverlapping(
                    text(src).add(start as usize).cast::<u8>(),
                    data(dst).cast::<u8>(),
                    bytes,
                );
            }
        }
        Ok(())
    }
}

unsafe fn append_live(
    dst: Hp2ArraySlots,
    src: Hp2ArraySlots,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe {
        status((svc.set_tag)(svc.context, svc.string_tag))?;
        let result = (|| {
            let count = dst.num.read();
            let n = terminated_len(text(src), svc.arrays)?;
            if count != 0 || n != 0 {
                let index = if count == 0 { 0 } else { count - 1 };
                array::add(
                    dst,
                    if count == 0 { n.wrapping_add(1) } else { n },
                    WIDTH,
                    svc.array_tag,
                    svc.arrays,
                )?;
                // strcpy uses the live source after the allocator returns.
                ansi::strcpy(data(dst).add(index as usize), text(src));
            }
            Ok(())
        })();
        let cleanup = status((svc.set_tag)(svc.context, ptr::null()));
        result.and(cleanup)
    }
}

unsafe fn pad_value(
    dst: Hp2ArraySlots,
    src: Hp2ArraySlots,
    pad: i32,
    left: bool,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe {
        // Preserve the native space-string temporary and its allocation/check/
        // destruction order, without a second scratch allocation for appAlloca.
        let mut buffer = ptr::null_mut();
        let mut num = 0;
        let mut max = 0;
        let spaces = Hp2ArraySlots {
            data: &mut buffer,
            num: &mut num,
            max: &mut max,
        };
        array::set_exact(
            spaces,
            pad.wrapping_add(1),
            WIDTH,
            svc.string_tag,
            svc.arrays,
        )?;
        let mut work_buffer = ptr::null_mut();
        let mut work_num = 0;
        let mut work_max = 0;
        let work = Hp2ArraySlots {
            data: &mut work_buffer,
            num: &mut work_num,
            max: &mut work_max,
        };
        let mut work_constructed = false;
        let result = (|| {
            checked_copy_size(spaces, svc.arrays)?;
            for i in 0..pad {
                data(spaces).add(i as usize).write(b' ' as Tchar);
            }
            data(spaces).add(pad as usize).write(0);
            assign(
                work,
                Some(if left { spaces } else { src }),
                ptr::null(),
                true,
                svc,
            )?;
            work_constructed = true;
            append_live(work, if left { src } else { spaces }, svc)?;
            // operator+ returns its += lvalue by counted copy, not by moving
            // the grown array; its returned capacity is therefore exact.
            assign(dst, Some(work), ptr::null(), true, svc)
        })();
        let work_cleanup = if work_constructed {
            array::empty(work, 0, WIDTH, svc.array_tag, svc.arrays)
                .and_then(|()| array::release(work, svc.arrays))
        } else {
            array::release(work, svc.arrays)
        };
        let space_cleanup = array::empty(spaces, 0, WIDTH, svc.array_tag, svc.arrays)
            .and_then(|()| array::release(spaces, svc.arrays));
        result.and(work_cleanup).and(space_cleanup)
    }
}

/// Value operations: 0 Left, 1 LeftChop, 2 Right, 3 Mid, 4 Caps, 5 Locs,
/// 6 LeftPad, 7 RightPad, 8 Chr. Output is a separate, initially empty string.
/// # Safety
/// See module contract. Source is required except for Chr.
pub unsafe fn value(
    dst: Hp2ArraySlots,
    src: Hp2ArraySlots,
    op: i32,
    a: i32,
    b: i32,
    svc: Hp2FStringStorageServices,
) -> Result<(), i32> {
    unsafe {
        if op == 8 {
            let chars = [a as Tchar, 0];
            return assign(dst, None, chars.as_ptr(), true, svc);
        }
        let length = len(src);
        match op {
            0 => range_value(dst, src, 0, Some(a.clamp(0, length)), svc),
            1 => range_value(
                dst,
                src,
                0,
                Some(length.wrapping_sub(a).clamp(0, length)),
                svc,
            ),
            2 => range_value(dst, src, length - a.clamp(0, length), None, svc),
            3 => {
                let (start, count) = mid_bounds(length, a, b);
                range_value(dst, src, start, Some(count), svc)
            }
            4 | 5 => {
                range_value(dst, src, 0, None, svc)?;
                let mut i = 0;
                while i < src.num.read() {
                    if i >= dst.num.read() {
                        status((svc.bounds_error)(svc.context))?;
                        return Err(1); // never reproduce an out-of-bounds native access
                    }
                    let mut ch = 0;
                    status((svc.case_char)(
                        svc.context,
                        data(dst).add(i as usize).read() as u32,
                        i32::from(op == 4),
                        &mut ch,
                    ))?;
                    if i >= dst.num.read() {
                        status((svc.bounds_error)(svc.context))?;
                        return Err(1);
                    }
                    data(dst).add(i as usize).write(ch as Tchar);
                    i += 1;
                }
                Ok(())
            }
            6 | 7 => {
                let pad = a.wrapping_sub(length);
                if pad <= 0 {
                    return assign(dst, Some(src), ptr::null(), true, svc);
                }
                pad_value(dst, src, pad, op == 6, svc)
            }
            _ => Err(1),
        }
    }
}

/// # Safety
/// Source is a valid counted, terminated string and needle is NUL-terminated.
pub unsafe fn search(src: Hp2ArraySlots, needle: *const Tchar, right: bool) -> i32 {
    unsafe {
        let hay = text(src);
        if !right {
            let found = ansi::strstr(hay, needle);
            return if found.is_null() {
                -1
            } else {
                found.offset_from(hay) as i32
            };
        }
        let mut i = len(src) - 1;
        while i >= 0 {
            let mut j = 0;
            while needle.add(j as usize).read() != 0 {
                // The original indexed access cannot match beyond the live end.
                if i + j >= src.num.read()
                    || hay.add((i + j) as usize).read() != needle.add(j as usize).read()
                {
                    break;
                }
                j += 1;
            }
            if needle.add(j as usize).read() == 0 {
                return i;
            }
            i -= 1;
        }
        -1
    }
}

/// Split deliberately resolves the source/separator again after Left assignment.
/// # Safety
/// Object pointers identify live FString objects; outputs may alias either input
/// or each other. Callbacks own native temporaries and capture all exceptions.
pub unsafe fn split(
    src: *const c_void,
    separator: *const c_void,
    left: *mut c_void,
    right: *mut c_void,
    reverse: bool,
    svc: Hp2FStringStorageServices,
) -> Result<bool, i32> {
    unsafe {
        let pos = search((svc.slots)(src), text((svc.slots)(separator)), reverse);
        if pos < 0 {
            return Ok(false);
        }
        if !left.is_null() {
            status((svc.assign_range)(left, src, 0, pos))?;
        }
        if !right.is_null() {
            let start = pos.wrapping_add(len((svc.slots)(separator)));
            status((svc.assign_range)(right, src, start, i32::MAX))?;
        }
        Ok(true)
    }
}
