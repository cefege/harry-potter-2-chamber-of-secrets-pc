//! Port of `FMemStack` and `FMemMark` in `Core/Inc/UnMem.h` and the retired
//! `Core/Src/UnMem.cpp`. The SDK methods are defined in Rust by
//! `crates/hp2-ffi/src/core_mem.rs`.
//!
//! C++ calls that can fail (`check`, `appErrorf`, `appMalloc`) go through
//! [`MemServices`], filled by `Core/Src/Hp2MemServices.cpp`. A failing call returns
//! the frame code of the `guard` stack that was open at that point; the SDK
//! method rethrows the recorded exception inside those frames. Frame codes are
//! documented per function.
//!
//! `UnusedChunks` is the static list of chunks released by `FreeChunks` and freed
//! by `Exit`; it is process-wide, as the C++ static was.

use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::atomic::AtomicPtr;

use hp2_abi::engine::generated::{FMemStack, FTaggedMemory};
use hp2_abi::render_span::FMemMark;

use crate::ansi::Tchar;

/// Swap two disjoint byte ranges without allocating.
///
/// # Safety
/// When `size` is nonzero, both pointers must be valid for `size` writable bytes
/// and the ranges must not overlap. Null pointers are permitted when `size == 0`.
pub unsafe fn memswap(left: *mut u8, right: *mut u8, size: u32) {
    if size == 0 {
        return;
    }
    let size = size as usize;
    let left = unsafe { std::slice::from_raw_parts_mut(left, size) };
    let right = unsafe { std::slice::from_raw_parts_mut(right, size) };
    left.swap_with_slice(right);
}

#[cfg(test)]
mod memswap_tests {
    use super::memswap;
    use std::ptr::null_mut;

    #[test]
    fn swaps_only_requested_bytes_and_accepts_empty_ranges() {
        let mut left = [1u8, 2, 3, 4];
        let mut right = [5u8, 6, 7, 8];
        // SAFETY: these arrays are separate writable ranges of four bytes.
        unsafe { memswap(left.as_mut_ptr(), right.as_mut_ptr(), 3) };
        assert_eq!(left, [5, 6, 7, 4]);
        assert_eq!(right, [1, 2, 3, 8]);

        // SAFETY: empty ranges do not dereference either pointer.
        unsafe { memswap(null_mut(), null_mut(), 0) };
    }
}

/// Frame code: the function's own `guard`.
pub const FRAME_OWN: i32 = 1;

/// Frame code: `FMemStack::Tick` (`guard(FMemStack::InitStack)`) nested under
/// `FMemStack::Exit`.
pub const FRAME_INIT_STACK_UNDER_EXIT: i32 = 2;

/// Frame code: `FMemStack::AllocateNewChunk` nested under `PushBytes`.
pub const FRAME_ALLOCATE_UNDER_PUSH: i32 = 2;

/// `FMemStack::UnusedChunks`.
static UNUSED_CHUNKS: AtomicPtr<FTaggedMemory> = AtomicPtr::new(null_mut());

/// C++ callbacks the mem port needs. Filled in `Core/Src/Hp2MemServices.cpp`; each
/// entry is a leaf call into the original C++ function. A callback that raises a
/// C++ exception returns status 1 and the exception is recorded on the C++ side.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2MemServices {
    /// `appMalloc(Size, Tag)`: writes the block to `*out`.
    pub malloc: unsafe extern "C" fn(size: u32, tag: *const c_void, out: *mut *mut c_void) -> i32,
    /// `appFree(Ptr)`.
    pub free: unsafe extern "C" fn(ptr: *mut c_void),
    /// `appErrorf(TEXT("%s"), Msg)`: `Msg` is a NUL-terminated `TCHAR` string.
    pub errorf: unsafe extern "C" fn(msg: *const c_void) -> i32,
    /// `appFailAssert(Expr, File, Line)`.
    pub fail_assert:
        unsafe extern "C" fn(expr: *const c_char, file: *const c_char, line: i32) -> i32,
}

/// The service calls the mem port makes. Each returns `Err(1)` or a non-zero status
/// when the C++ side raised.
pub trait MemServices {
    /// `appMalloc(size, tag)`.
    ///
    /// # Safety
    /// `tag` must be null or a NUL-terminated `TCHAR` string valid for the call.
    unsafe fn alloc(&self, size: u32, tag: *const c_void) -> Result<*mut c_void, i32>;
    /// `appFree(ptr)`.
    ///
    /// # Safety
    /// `ptr` must be a block obtained from `alloc` and not yet released.
    unsafe fn release(&self, ptr: *mut c_void);
    /// `appErrorf` with a NUL-terminated `TCHAR` message. Non-zero when it raised.
    fn raise_error(&self, msg: &[Tchar]) -> i32;
    /// `appFailAssert`. Non-zero when it raised.
    fn fail_assert(&self, expr: &CStr, file: &CStr, line: i32) -> i32;
}

impl MemServices for Hp2MemServices {
    unsafe fn alloc(&self, size: u32, tag: *const c_void) -> Result<*mut c_void, i32> {
        let mut out = null_mut();
        // SAFETY: the table is filled by the C++ side; `out` is a valid out-slot.
        match unsafe { (self.malloc)(size, tag, &mut out) } {
            0 => Ok(out),
            status => Err(status),
        }
    }

    unsafe fn release(&self, ptr: *mut c_void) {
        // SAFETY: the table is filled by the C++ side; `ptr` came from `alloc`.
        unsafe { (self.free)(ptr) }
    }

    fn raise_error(&self, msg: &[Tchar]) -> i32 {
        // SAFETY: `msg` comes from `wide_z`, so it is NUL-terminated.
        unsafe { (self.errorf)(msg.as_ptr().cast()) }
    }

    fn fail_assert(&self, expr: &CStr, file: &CStr, line: i32) -> i32 {
        // SAFETY: `expr` and `file` are NUL-terminated `CStr`s, valid for the call.
        unsafe { (self.fail_assert)(expr.as_ptr(), file.as_ptr(), line) }
    }
}

/// Encodes `s` as a NUL-terminated `TCHAR` string.
pub fn wide_z(s: &str) -> Vec<Tchar> {
    s.chars()
        .map(|c| c as u32 as Tchar)
        .chain(std::iter::once(0))
        .collect()
}

/// `appErrorf(Msg)`: `Err(frame)` when the service raised.
pub(crate) fn raise<S: MemServices>(svc: &S, msg: &str, frame: i32) -> Result<(), i32> {
    if svc.raise_error(&wide_z(msg)) != 0 {
        return Err(frame);
    }
    Ok(())
}

/// `check(Expr)`: calls `appFailAssert` when `ok` is false.
pub(crate) fn check<S: MemServices>(
    svc: &S,
    ok: bool,
    expr: &CStr,
    file: &CStr,
    line: i32,
    frame: i32,
) -> Result<(), i32> {
    if ok {
        return Ok(());
    }
    if svc.fail_assert(expr, file, line) != 0 {
        return Err(frame);
    }
    Ok(())
}

/// `appCheckedIntSize(Size)`: above `MAXINT` it reports the contract violation,
/// then truncates to `INT` as the original does.
///
/// # Errors
/// Returns `frame` if the error service rejects an oversized allocation.
pub fn checked_int_size<S: MemServices>(svc: &S, size: usize, frame: i32) -> Result<i32, i32> {
    if size > i32::MAX as usize {
        raise(svc, "Size exceeds the signed 32-bit API contract", frame)?;
    }
    Ok(size as i32)
}

/// `appCheckedAllocSize(Size)`: above `MAXDWORD` it reports the contract violation,
/// then truncates to `DWORD` as the original does.
pub(crate) fn checked_alloc_size<S: MemServices>(
    svc: &S,
    size: usize,
    frame: i32,
) -> Result<u32, i32> {
    if size > u32::MAX as usize {
        raise(
            svc,
            "Allocation size exceeds the 32-bit allocator contract",
            frame,
        )?;
    }
    Ok(size as u32)
}

/// `operator new[](Size, Tag)`: checked allocation through `appMalloc`. `tag` is a
/// NUL-terminated `TCHAR` string, as `TEXT("...")` is.
///
/// # Safety
/// `tag` must be NUL-terminated; it is passed to the allocator as-is.
pub(crate) unsafe fn alloc_checked<S: MemServices>(
    svc: &S,
    size: usize,
    tag: &[Tchar],
    frame: i32,
) -> Result<*mut c_void, i32> {
    let size = checked_alloc_size(svc, size, frame)?;
    // SAFETY: `tag` is NUL-terminated per this function's contract.
    unsafe { svc.alloc(size, tag.as_ptr().cast()) }.map_err(|_| frame)
}

/// `FMemStack::Init`. Stores the settings; cannot fail.
pub fn init(stack: &mut FMemStack, default_chunk_size: i32, tag: *const c_void) {
    stack.default_chunk_size = default_chunk_size;
    stack.top_chunk = null_mut();
    stack.end = null_mut();
    stack.top = null_mut();
    stack.tag = tag.cast();
}

/// `FMemStack::Tick` (`guard(FMemStack::InitStack)`): `check(TopChunk==NULL)`.
/// Returns [`FRAME_OWN`] when the check raised.
///
/// # Safety
/// `file` must be a NUL-terminated string that outlives the call.
pub unsafe fn tick(
    stack: &FMemStack,
    svc: &impl MemServices,
    file: *const c_char,
) -> Result<(), i32> {
    // SAFETY: `file` is the caller's NUL-terminated `__FILE__`, per this function's contract.
    let file = unsafe { CStr::from_ptr(file) };
    check(
        svc,
        stack.top_chunk.is_null(),
        c"TopChunk==NULL",
        file,
        43,
        FRAME_OWN,
    )
}

/// `FMemStack::Exit` (`guard(FMemStack::Exit)`): `Tick()`, then frees every released
/// chunk. A failing `Tick` returns [`FRAME_INIT_STACK_UNDER_EXIT`].
///
/// # Safety
/// `file` must be a NUL-terminated string. Every chunk on `UnusedChunks` must have
/// been allocated through `svc`.
pub unsafe fn exit(
    stack: &FMemStack,
    svc: &impl MemServices,
    file: *const c_char,
) -> Result<(), i32> {
    unsafe { exit_with_pool(stack, svc, file, UNUSED_CHUNKS.as_ptr()) }
}

pub(crate) unsafe fn exit_with_pool(
    stack: &FMemStack,
    svc: &impl MemServices,
    file: *const c_char,
    unused: *mut *mut FTaggedMemory,
) -> Result<(), i32> {
    // SAFETY: forwarded from this function's contract.
    unsafe { tick(stack, svc, file) }.map_err(|_| FRAME_INIT_STACK_UNDER_EXIT)?;
    loop {
        let old = unsafe { *unused };
        if old.is_null() {
            return Ok(());
        }
        // SAFETY: `old` is a live chunk on the unused list (see the contract).
        unsafe { *unused = (*old).next; }
        // SAFETY: `old` was just unlinked from the unused list and came from `alloc`.
        unsafe { svc.release(old.cast()) };
    }
}

/// `FMemStack::GetByteCount` (`guard(FMemStack::GetByteCount)`). Frame 1 on the
/// `appCheckedIntSize` contract error.
///
/// # Safety
/// The chunk chain from `top_chunk` must be the list maintained by this module.
pub unsafe fn get_byte_count(stack: &FMemStack, svc: &impl MemServices) -> Result<i32, i32> {
    let mut count: usize = 0;
    let mut chunk = stack.top_chunk;
    // SAFETY: the chain is the `TopChunk` list, linked by `allocate_new_chunk`.
    unsafe {
        while !chunk.is_null() {
            if chunk != stack.top_chunk {
                count = count.wrapping_add((*chunk).data_size as usize);
            } else {
                let data = (&raw mut (*chunk).data) as usize;
                count = count.wrapping_add((stack.top as usize).wrapping_sub(data));
            }
            chunk = (*chunk).next;
        }
    }
    checked_int_size(svc, count, FRAME_OWN)
}

/// `FMemStack::PushBytes`: align within the current chunk, or grow before
/// publishing the new top. `slow_checks` is the native `DO_GUARD_SLOW` policy.
///
/// # Safety
/// The stack and its chunk chain must be live, and calls using the process-wide
/// unused list must be serialized as in the native implementation. When slow
/// checks are disabled (or their diagnostic returns), the caller must satisfy
/// the native nonnegative size, power-of-two alignment and stack invariants.
pub unsafe fn push_bytes(
    stack: &mut FMemStack,
    alloc_size: i32,
    alignment: i32,
    slow_checks: bool,
    file: &CStr,
    svc: &impl MemServices,
) -> Result<*mut u8, i32> {
    unsafe { push_bytes_with_pool(stack, alloc_size, alignment, slow_checks, file, svc, UNUSED_CHUNKS.as_ptr()) }
}

pub(crate) unsafe fn push_bytes_with_pool(
    stack: &mut FMemStack,
    alloc_size: i32,
    alignment: i32,
    slow_checks: bool,
    file: &CStr,
    svc: &impl MemServices,
    unused: *mut *mut FTaggedMemory,
) -> Result<*mut u8, i32> {
    if slow_checks {
        check(svc, alloc_size >= 0, c"AllocSize>=0", file, 44, FRAME_OWN)?;
        check(
            svc,
            alignment > 0 && (alignment & alignment.wrapping_sub(1)) == 0,
            c"Alignment>0 && (Alignment&(Alignment-1))==0",
            file,
            45,
            FRAME_OWN,
        )?;
        check(
            svc,
            stack.top.is_null() == stack.end.is_null(),
            c"(Top==NULL)==(End==NULL)",
            file,
            46,
            FRAME_OWN,
        )?;
    }

    let mask = (alignment as usize).wrapping_sub(1);
    let align = |ptr: *mut u8| ptr.map_addr(|addr| addr.wrapping_add(mask) & !mask);
    let mut result = null_mut();
    if !stack.top.is_null() {
        if slow_checks {
            check(
                svc,
                stack.top as usize <= stack.end as usize,
                c"reinterpret_cast<UPTRINT>(Top)<=reinterpret_cast<UPTRINT>(End)",
                file,
                51,
                FRAME_OWN,
            )?;
        }
        let candidate = align(stack.top);
        if candidate as usize <= stack.end as usize
            && alloc_size as usize <= (stack.end as usize).wrapping_sub(candidate as usize)
        {
            result = candidate;
        }
    }
    if result.is_null() {
        let min_size = checked_int_size(
            svc,
            (alloc_size as usize).wrapping_add(alignment as usize),
            FRAME_OWN,
        )?;
        // SAFETY: this stack and the shared unused list satisfy the contract.
        unsafe { allocate_new_chunk_with_pool(stack, min_size, svc, unused) }
            .map_err(|_| FRAME_ALLOCATE_UNDER_PUSH)?;
        result = align(stack.top);
        if slow_checks {
            check(
                svc,
                result as usize <= stack.end as usize,
                c"reinterpret_cast<UPTRINT>(Result)<=reinterpret_cast<UPTRINT>(End)",
                file,
                64,
                FRAME_OWN,
            )?;
            check(
                svc,
                alloc_size as usize <= (stack.end as usize).wrapping_sub(result as usize),
                c"static_cast<UPTRINT>(AllocSize)<=reinterpret_cast<UPTRINT>(End)-reinterpret_cast<UPTRINT>(Result)",
                file,
                65,
                FRAME_OWN,
            )?;
        }
    }
    stack.top = result.wrapping_offset(alloc_size as isize);
    Ok(result)
}

/// Capture an `FMemMark`, including a previously uninitialized native mark.
///
/// # Safety
/// `mark` must be writable and disjoint from the live `stack`; the stack must
/// outlive use of the mark. No previous value of the mark is read.
pub unsafe fn mark_init(mark: *mut FMemMark, stack: *mut FMemStack) {
    // SAFETY: field writes do not create a reference to uninitialized mark data.
    unsafe {
        (&raw mut (*mark).mem).write(stack.cast());
        (&raw mut (*mark).top).write((*stack).top);
        (&raw mut (*mark).saved_chunk).write((*stack).top_chunk.cast());
    }
}

/// Restore a mark. Leave `end` untouched when no chunks have been added.
///
/// # Safety
/// The mark must refer to a live stack, and its saved chunk must still be on
/// that stack's chain (or null). Shared unused-list access must be serialized.
pub unsafe fn mark_pop(mark: &FMemMark) {
    unsafe { mark_pop_with_pool(mark, UNUSED_CHUNKS.as_ptr()) }
}

pub(crate) unsafe fn mark_pop_with_pool(mark: &FMemMark, unused: *mut *mut FTaggedMemory) {
    // SAFETY: the saved pointers refer to the live stack and its chunk chain.
    unsafe {
        let stack = &mut *mark.mem.cast::<FMemStack>();
        if mark.saved_chunk.cast::<FTaggedMemory>() != stack.top_chunk {
            free_chunks_with_pool(stack, mark.saved_chunk.cast(), unused);
        }
        stack.top = mark.top;
    }
}

/// `FMemStack::AllocateNewChunk(MinSize)` (`guard(FMemStack::AllocateNewChunk)`).
/// Reuses the first unused chunk with `DataSize >= MinSize`, otherwise allocates a
/// new one. Returns the new top, or [`FRAME_OWN`] on a contract or allocator error.
///
/// # Safety
/// `stack` must be a live `FMemStack`; the unused list must hold only chunks this
/// module allocated.
pub unsafe fn allocate_new_chunk(
    stack: &mut FMemStack,
    min_size: i32,
    svc: &impl MemServices,
) -> Result<*mut u8, i32> {
    unsafe { allocate_new_chunk_with_pool(stack, min_size, svc, UNUSED_CHUNKS.as_ptr()) }
}

pub(crate) unsafe fn allocate_new_chunk_with_pool(
    stack: &mut FMemStack,
    min_size: i32,
    svc: &impl MemServices,
    unused: *mut *mut FTaggedMemory,
) -> Result<*mut u8, i32> {
    // SAFETY: the unused list links are walked and updated in place, as in C++.
    unsafe {
        let mut chunk: *mut FTaggedMemory = null_mut();
        let mut link = unused;
        while !(*link).is_null() {
            if (**link).data_size >= min_size {
                chunk = *link;
                *link = (*chunk).next;
                break;
            }
            link = &raw mut (**link).next;
        }
        if chunk.is_null() {
            let header = checked_int_size(svc, size_of::<FTaggedMemory>(), FRAME_OWN)?;
            let data_size = min_size.max(stack.default_chunk_size.wrapping_sub(header));
            let bytes = checked_alloc_size(
                svc,
                (data_size as usize).wrapping_add(size_of::<FTaggedMemory>()),
                FRAME_OWN,
            )?;
            chunk = svc
                .alloc(bytes, stack.tag.cast())
                .map_err(|_| FRAME_OWN)?
                .cast();
            (*chunk).data_size = data_size;
        }
        (*chunk).next = stack.top_chunk;
        stack.top_chunk = chunk;
        stack.top = (&raw mut (*chunk).data).cast();
        stack.end = stack.top.wrapping_offset((*chunk).data_size as isize);
        Ok(stack.top)
    }
}

/// `FMemStack::FreeChunks(NewTopChunk)` (`guard(FMemStack::FreeChunks)`). Moves
/// chunks above `new_top_chunk` onto the unused list and resets `top`/`end`.
///
/// # Safety
/// `new_top_chunk` must be null or a chunk on the current `TopChunk` list.
pub unsafe fn free_chunks(stack: &mut FMemStack, new_top_chunk: *mut FTaggedMemory) {
    unsafe { free_chunks_with_pool(stack, new_top_chunk, UNUSED_CHUNKS.as_ptr()) }
}

pub(crate) unsafe fn free_chunks_with_pool(
    stack: &mut FMemStack, new_top_chunk: *mut FTaggedMemory,
    unused: *mut *mut FTaggedMemory,
) {
    // SAFETY: the chain is walked as in C++; a `new_top_chunk` off the chain is UB
    // here exactly as the original null dereference would be.
    unsafe {
        while stack.top_chunk != new_top_chunk {
            let remove = stack.top_chunk;
            stack.top_chunk = (*remove).next;
            (*remove).next = *unused;
            *unused = remove;
        }
        stack.top = null_mut();
        stack.end = null_mut();
        if !stack.top_chunk.is_null() {
            stack.top = (&raw mut (*stack.top_chunk).data).cast();
            stack.end = stack
                .top
                .wrapping_offset((*stack.top_chunk).data_size as isize);
        }
    }
}
