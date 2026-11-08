//! Raw TArray/FArray storage policy. C++ retains typed lifetimes and assertions.
//!
//! Slots are addressed independently: no base/derived layout assumption and no
//! Rust reference to live native state survives a service call. Services keep
//! the native allocator, size diagnostics, allocation tags and exception guards.
//! The engine packs pointer fields to four bytes; pointer-slot accesses are
//! explicitly unaligned, while native INT slots retain their four-byte alignment.

use std::ffi::c_void;
use std::ptr;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ArraySlots {
    pub data: *mut *mut c_void,
    pub num: *mut i32,
    pub max: *mut i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ArrayServices {
    pub context: *mut c_void,
    /// Captures checked allocation size, appRealloc and its native guard.
    pub realloc: unsafe extern "C" fn(
        context: *mut c_void,
        data: *mut c_void,
        bytes: usize,
        max: *const i32,
        width: i32,
        tag: *const c_void,
        out: *mut *mut c_void,
    ) -> i32,
    pub checked_int_size:
        unsafe extern "C" fn(context: *mut c_void, bytes: usize, out: *mut i32) -> i32,
    pub free: unsafe extern "C" fn(context: *mut c_void, data: *mut c_void) -> i32,
}

// A resource publication, not an allocator implementation or a second heap.
// VM handlers use the same leaves as the host's existing array/string owners.
static mut ALLOCATOR: Option<Hp2ArrayServices> = None;

/// Publish the process's array allocation resource before direct VM execution.
///
/// # Safety
/// Engine-thread serialized with all consumers. The copied callbacks/context
/// must remain valid until replacement. Replacement is permitted only with no
/// outstanding allocations or calls using the previous resource, unless both
/// publications use the identical allocator. Callbacks never unwind.
pub unsafe fn publish_allocator(services: Hp2ArrayServices) {
    unsafe {
        (&raw mut ALLOCATOR).write(Some(services));
    }
}

/// # Safety
/// Serialized with publication. The returned resource satisfies the lifetime
/// and allocator-pairing contract of `publish_allocator`.
pub unsafe fn published_allocator() -> Result<Hp2ArrayServices, i32> {
    unsafe { (&raw const ALLOCATOR).read().ok_or(1) }
}

/// Stateless SDK allocator for Rust-owned Core storage. Unlike the legacy
/// publication, these operations cannot dispatch through a C++ heap owner.
pub const fn system_allocator() -> Hp2ArrayServices {
    Hp2ArrayServices {
        context: ptr::null_mut(),
        realloc: system_realloc,
        checked_int_size: system_checked_int_size,
        free: system_free,
    }
}

unsafe extern "C" fn system_realloc(
    _context: *mut c_void, data: *mut c_void, bytes: usize,
    _max: *const i32, _width: i32, _tag: *const c_void, out: *mut *mut c_void,
) -> i32 {
    if u32::try_from(bytes).is_err() { return 1; }
    let result = if bytes == 0 {
        unsafe { libc::free(data); }
        ptr::null_mut()
    } else {
        let result = unsafe { libc::realloc(data, bytes) };
        if result.is_null() { return 1; }
        result
    };
    unsafe { out.write(result); }
    0
}

unsafe extern "C" fn system_checked_int_size(
    _context: *mut c_void, bytes: usize, out: *mut i32,
) -> i32 {
    let Ok(size) = i32::try_from(bytes) else { return 1; };
    unsafe { out.write(size); }
    0
}

unsafe extern "C" fn system_free(_context: *mut c_void, data: *mut c_void) -> i32 {
    unsafe { libc::free(data); }
    0
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

// C++ promotes the count to SIZE_T before multiplying by the signed width.
fn bytes(count: i32, width: i32) -> usize {
    (count as usize).wrapping_mul(width as usize)
}

unsafe fn checked_bytes(svc: Hp2ArrayServices, count: i32, width: i32) -> Result<usize, i32> {
    let mut out = 0;
    unsafe {
        status((svc.checked_int_size)(
            svc.context,
            bytes(count, width),
            &mut out,
        ))?;
    }
    Ok(out as usize)
}

/// # Safety
/// Slots must address live, independent native fields. The buffer must satisfy
/// the native array's capacity/width contract. Callbacks may reenter but must
/// preserve validity of those slots and the buffer used after they return.
/// Every service must capture native exceptions rather than unwind through Rust.
pub unsafe fn realloc(
    slots: Hp2ArraySlots,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let mut data = ptr::null_mut();
        status((svc.realloc)(
            svc.context,
            slots.data.read_unaligned(),
            bytes(slots.max.read(), width),
            slots.max,
            width,
            tag,
            &mut data,
        ))?;
        // Num/Max were already published by the caller; Data changes only on success.
        slots.data.write_unaligned(data);
    }
    Ok(())
}

/// # Safety
/// See [`realloc`]. Native precondition assertions remain in the C++ shell.
pub unsafe fn set(
    slots: Hp2ArraySlots,
    count: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        slots.num.write(count);
        if count > slots.max.read() {
            slots.max.write(count);
            realloc(slots, width, tag, svc)?;
        }
    }
    Ok(())
}

/// # Safety
/// See [`realloc`]. Count and the resulting native signed count must be valid.
pub unsafe fn add(
    slots: Hp2ArraySlots,
    count: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<i32, i32> {
    unsafe {
        let index = slots.num.read();
        let num = index.wrapping_add(count);
        slots.num.write(num);
        if num > slots.max.read() {
            slots
                .max
                .write(num.wrapping_add(num.wrapping_mul(3) / 8).wrapping_add(32));
            realloc(slots, width, tag, svc)?;
        }
        Ok(index)
    }
}

unsafe fn zero(
    slots: Hp2ArraySlots,
    index: i32,
    count: i32,
    width: i32,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let len = checked_bytes(svc, count, width)?;
        // In particular, don't offset a null pointer for zero-width elements.
        if len != 0 {
            ptr::write_bytes(
                slots
                    .data
                    .read_unaligned()
                    .cast::<u8>()
                    .add(bytes(index, width)),
                0,
                len,
            );
        }
    }
    Ok(())
}

/// # Safety
/// See [`add`].
pub unsafe fn add_zeroed(
    slots: Hp2ArraySlots,
    count: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<i32, i32> {
    unsafe {
        let index = add(slots, count, width, tag, svc)?;
        zero(slots, index, count, width, svc)?;
        Ok(index)
    }
}

/// # Safety
/// See [`add`]. Index must be within the original live element range or its end.
pub unsafe fn insert(
    slots: Hp2ArraySlots,
    index: i32,
    count: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let old_num = add(slots, count, width, tag, svc)?;
        let len = checked_bytes(svc, old_num.wrapping_sub(index), width)?;
        if len != 0 {
            let data = slots.data.read_unaligned().cast::<u8>();
            ptr::copy(
                data.add(bytes(index, width)),
                data.add(bytes(index.wrapping_add(count), width)),
                len,
            );
        }
    }
    Ok(())
}

/// # Safety
/// See [`insert`].
pub unsafe fn insert_zeroed(
    slots: Hp2ArraySlots,
    index: i32,
    count: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        insert(slots, index, count, width, tag, svc)?;
        zero(slots, index, count, width, svc)?;
    }
    Ok(())
}

/// # Safety
/// See [`realloc`].
pub unsafe fn shrink(
    slots: Hp2ArraySlots,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        if slots.max.read() != slots.num.read() {
            slots.max.write(slots.num.read());
            realloc(slots, width, tag, svc)?;
        }
    }
    Ok(())
}

/// # Safety
/// See [`realloc`]. C++ must already have destroyed any typed live elements.
pub unsafe fn empty(
    slots: Hp2ArraySlots,
    slack: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        slots.num.write(0);
        slots.max.write(slack);
        realloc(slots, width, tag, svc)
    }
}

/// # Safety
/// See [`realloc`]. The removed range must be live, and typed elements in that
/// range must already have been destroyed by C++.
pub unsafe fn remove(
    slots: Hp2ArraySlots,
    index: i32,
    count: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        if count != 0 {
            let len = checked_bytes(
                svc,
                slots.num.read().wrapping_sub(index).wrapping_sub(count),
                width,
            )?;
            if len != 0 {
                let data = slots.data.read_unaligned().cast::<u8>();
                ptr::copy(
                    data.add(bytes(index.wrapping_add(count), width)),
                    data.add(bytes(index, width)),
                    len,
                );
            }
            // Size-check callbacks may reenter: decrement the live count.
            let num = slots.num.read().wrapping_sub(count);
            slots.num.write(num);
            let max = slots.max.read();
            let slack = max.wrapping_sub(num);
            if (num.wrapping_mul(3) < max.wrapping_mul(2) || slack.wrapping_mul(width) >= 16384)
                && (slack > 64 || num == 0)
            {
                slots.max.write(num);
                realloc(slots, width, tag, svc)?;
            }
        }
    }
    Ok(())
}

/// Final destructor release, including residual slack after a zero-count remove.
/// # Safety
/// See [`empty`]. On a failing free the live fields remain unchanged.
pub unsafe fn release(slots: Hp2ArraySlots, svc: Hp2ArrayServices) -> Result<(), i32> {
    unsafe {
        let data = slots.data.read_unaligned();
        if !data.is_null() {
            status((svc.free)(svc.context, data))?;
        }
        slots.data.write_unaligned(ptr::null_mut());
        slots.num.write(0);
        slots.max.write(0);
    }
    Ok(())
}

/// Exact-count replacement used by byte archive loading and string assignment.
/// Unlike `set`, it always reallocates, even when capacity already equals count.
/// # Safety
/// See [`realloc`]. C++ retains archive reads and typed/string value copying.
pub unsafe fn set_exact(
    slots: Hp2ArraySlots,
    count: i32,
    width: i32,
    tag: *const c_void,
    svc: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        slots.num.write(count);
        slots.max.write(count);
        realloc(slots, width, tag, svc)
    }
}
/// Releases a Rust-owned raw array whose live elements have already been
/// explicitly destroyed.
///
/// # Safety
/// The slots identify one live libc-backed array allocation. Every nontrivial
/// element has already been destroyed in native observable order.
pub(crate) unsafe fn release_owned(
    data: *mut *mut c_void,
    num: *mut i32,
    max: *mut i32,
) {
    unsafe {
        let allocation = data.read_unaligned();
        num.write(0);
        max.write(0);
        if !allocation.is_null() {
            libc::free(allocation);
        }
        data.write_unaligned(ptr::null_mut());
    }
}
/// Reserves a Rust-owned raw TArray allocation using the shared engine growth kernel.
///
/// # Safety
/// `data`, `num`, and `max` identify a live libc-backed array. Existing live
/// elements remain initialized and the width is their exact native stride.
pub(crate) unsafe fn reserve_owned(
    data: *mut *mut c_void,
    num: *mut i32,
    max: *mut i32,
    required: i32,
    width: i32,
) -> Result<(), i32> {
    if required < 0 || width <= 0 { return Err(1); }
    let old_num = unsafe { num.read_unaligned() };
    let old_max = unsafe { max.read_unaligned() };
    let old_data = unsafe { data.read_unaligned() };
    if old_num < 0 || old_max < old_num || required < old_num || (old_num > 0 && old_data.is_null()) {
        return Err(1);
    }
    if required <= old_max { return Ok(()); }
    let capacity = old_max.max(1).checked_mul(2).unwrap_or(i32::MAX).max(required);
    let bytes = (capacity as usize).checked_mul(width as usize).ok_or(1)?;
    let grown = unsafe { libc::realloc(old_data, bytes) };
    if grown.is_null() { return Err(1); }
    unsafe {
        data.write_unaligned(grown);
        max.write_unaligned(capacity);
    }
    Ok(())
}
