//! Narrow raw archive ABI leaves shared by native model serializers.
use std::ffi::c_void;

/// Raw FArchive operations and compact-index services provided by the native boundary.
#[repr(C)]
pub struct Hp2ModelArchiveIoServices {
    pub compact: hp2_core::compact_index::Hp2CompactIndexServices,
    pub bytes: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub scalar: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub object: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub name: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub count_bytes: unsafe extern "C" fn(*mut c_void, usize, usize) -> i32,
}

/// Synchronous access to a live native archive and immutable raw I/O services.
pub(crate) struct NativeModelArchive<'s> {
    archive: *mut c_void,
    services: &'s Hp2ModelArchiveIoServices,
}

impl<'s> NativeModelArchive<'s> {
    /// Creates a borrowed archive view.
    ///
    /// # Safety
    /// `archive` and all callbacks in `services` must remain valid for every
    /// operation performed through the returned view.
    pub(crate) unsafe fn new(
        archive: *mut c_void,
        services: &'s Hp2ModelArchiveIoServices,
    ) -> Self {
        Self { archive, services }
    }

    pub(crate) fn is_loading(&self) -> bool {
        // SAFETY: compact archive query callback is valid for this synchronous archive call.
        unsafe { (self.services.compact.archive_is_loading)(self.archive) != 0 }
    }

    pub(crate) fn is_saving(&self) -> bool {
        // SAFETY: compact archive query callback is valid for this synchronous archive call.
        unsafe { (self.services.compact.archive_is_saving)(self.archive) != 0 }
    }

    /// Archives one typed scalar using the archive's byte-order operation.
    ///
    /// # Safety
    /// `value` must point to writable storage of `size_of::<T>()` bytes for this call.
    pub(crate) unsafe fn scalar<T>(&self, value: *mut T) -> i32 {
        // SAFETY: caller provides a valid single T, and the leaf does not retain its pointer.
        unsafe { (self.services.scalar)(self.archive, value.cast(), std::mem::size_of::<T>() as i32) }
    }

    /// Archives `count` bytes from a contiguous native span.
    ///
    /// # Safety
    /// `value` must span `count` writable bytes when loading, initialized bytes
    /// otherwise; a zero count permits an empty span.
    pub(crate) unsafe fn raw<T>(&self, value: *mut T, count: i32) -> i32 {
        // SAFETY: caller supplies the advertised byte span; no pointer is retained.
        unsafe { (self.services.bytes)(self.archive, value.cast(), count) }
    }

    /// Archives one signed value through Unreal's canonical compact-index implementation.
    ///
    /// # Safety
    /// `value` must point to a valid writable `i32` for this call.
    pub(crate) unsafe fn index(&self, value: *mut i32) -> i32 {
        // SAFETY: caller provides writable i32 storage and services satisfy compact-index ABI.
        unsafe { hp2_core::compact_index::serialize(self.archive, value, &self.services.compact) }
    }

    /// Archives an object reference through FArchive's reference-aware operator.
    ///
    /// # Safety
    /// `value` must point to a writable object pointer for this call.
    pub(crate) unsafe fn object(&self, value: *mut *mut c_void) -> i32 {
        // SAFETY: caller provides a writable pointer and callback is synchronous.
        unsafe { (self.services.object)(self.archive, value) }
    }

    /// Archives a name value through FArchive's reference-aware name operation.
    ///
    /// # Safety
    /// `value` must point to a valid native name representation for this call.
    pub(crate) unsafe fn name(&self, value: *mut c_void) -> i32 {
        // SAFETY: caller provides valid native name storage and callback is synchronous.
        unsafe { (self.services.name)(self.archive, value) }
    }

    pub(crate) fn count_bytes(&self, used_bytes: usize, capacity_bytes: usize) -> i32 {
        // SAFETY: raw archive leaf receives only scalar values and is synchronous.
        unsafe { (self.services.count_bytes)(self.archive, used_bytes, capacity_bytes) }
    }
}
