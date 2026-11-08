//! Abort-on-failure system allocator for `Core/Src/UnUnix.cpp`'s platform
//! bootstrap helpers (`appUnixSystemMalloc`/`appUnixSystemRealloc`). Every
//! other Unix-platform Rust leaf in this port that previously grew a native
//! buffer through those two C++ functions calls these instead.

use std::ffi::c_void;
use std::io::Write;

/// `appUnixSystemMalloc`: `malloc(Size ? Size : 1)`, aborting on failure.
///
/// # Safety
/// None beyond the normal system allocator contract. The returned pointer is
/// non-null and must be released with native libc `free`.
pub unsafe fn system_malloc(size: usize) -> *mut c_void {
    // SAFETY: a zero-sized request becomes a 1-byte request, matching the
    // original `Size ? Size : 1`.
    let result = unsafe { libc::malloc(if size == 0 { 1 } else { size }) };
    if result.is_null() {
        abort_out_of_memory();
    }
    result
}

/// `appUnixSystemRealloc`: `realloc(Pointer, Size ? Size : 1)`, aborting on
/// failure.
///
/// # Safety
/// `pointer` must be null or a pointer previously returned by
/// [`system_malloc`]/`system_realloc` (or native libc `malloc`/`realloc`)
/// that has not yet been freed.
pub unsafe fn system_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    // SAFETY: caller contract; a zero-sized request becomes 1 byte.
    let result = unsafe { libc::realloc(pointer, if size == 0 { 1 } else { size }) };
    if result.is_null() {
        abort_out_of_memory();
    }
    result
}

/// Allocate one block with the alignment of an owned engine record.
///
/// # Safety
/// `layout` describes the complete allocation. Release the result with libc
/// `free`; it must not be passed to `realloc` if its alignment exceeds malloc's
/// fundamental alignment.
pub unsafe fn system_malloc_layout(layout: std::alloc::Layout) -> *mut c_void {
    // Both supported arm64 platforms provide 16-byte-aligned malloc storage.
    if layout.align() <= align_of::<u128>() {
        return unsafe { system_malloc(layout.size()) };
    }
    let mut result = std::ptr::null_mut();
    let status = unsafe {
        libc::posix_memalign(&mut result, layout.align(), layout.size().max(1))
    };
    if status != 0 {
        abort_out_of_memory();
    }
    result
}

/// `fputs("Unreal: out of memory in Unix platform bootstrap\n", stderr); abort();`
fn abort_out_of_memory() -> ! {
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(b"Unreal: out of memory in Unix platform bootstrap\n");
    drop(stderr);
    std::process::abort();
}
