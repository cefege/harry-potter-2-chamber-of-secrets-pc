//! `Engine/MPeg/TbFile` lifecycle decisions.
//!
//! Callback invocation, C++ object storage, allocation, and virtual ABI stay
//! native leaves. This module owns only the legacy decision/state transitions.

#[inline]
pub fn should_open(handle: i32) -> bool {
    handle == 0
}

/// Publish a successful callback handle. A failed open leaves the refcount
/// unchanged, matching `TbFile::Open`.
pub fn finish_open(callback_handle: i32, handle: &mut i32, ref_count: &mut i32) -> i32 {
    *handle = callback_handle;
    if callback_handle != 0 {
        *ref_count = ref_count.wrapping_add(1);
    }
    callback_handle
}

/// Decrement the reference count before the native close callback. The C++
/// shell performs the callback and mapped-buffer cleanup only when this
/// returns true, preserving callback exception boundaries.
pub fn begin_close(handle: i32, has_close_callback: bool, ref_count: &mut i32) -> bool {
    if !has_close_callback || handle == 0 {
        return false;
    }
    *ref_count = ref_count.wrapping_sub(1);
    *ref_count <= 0
}

#[inline]
pub fn should_call(handle: i32, has_callback: bool) -> bool {
    handle != 0 && has_callback
}

#[inline]
pub fn should_map(handle: i32, has_size_callback: bool, has_read_callback: bool) -> bool {
    handle != 0 && has_size_callback && has_read_callback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_is_idempotent_and_failed_open_keeps_the_refcount() {
        let mut handle = 0;
        let mut refs = 0;
        assert!(should_open(handle));
        assert_eq!(finish_open(77, &mut handle, &mut refs), 77);
        assert_eq!((handle, refs), (77, 1));
        assert!(!should_open(handle));
        assert_eq!(finish_open(0, &mut handle, &mut refs), 0);
        assert_eq!((handle, refs), (0, 1));
    }

    #[test]
    fn close_decrements_before_native_cleanup_and_honors_missing_callback() {
        let mut refs = 1;
        assert!(begin_close(77, true, &mut refs));
        assert_eq!(refs, 0);
        let mut refs = 1;
        assert!(!begin_close(77, false, &mut refs));
        assert_eq!(refs, 1);
    }

    #[test]
    fn call_and_map_guards_match_legacy_defaults() {
        assert!(!should_call(0, true));
        assert!(!should_call(77, false));
        assert!(should_call(77, true));
        assert!(!should_map(77, true, false));
        assert!(should_map(77, true, true));
    }
}
