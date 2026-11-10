//! Stable allocations owned by the canonical object table.
//!
//! The allocation starts with the established UObject identity prefix. Rust
//! ownership metadata lives in the slot, never ahead of reflected fields.
//! Class operations first perform virtual Destroy and later finalize native
//! members before the slot releases memory.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::object_bootstrap::{Hp2ObjectHeaderKind, initialize_header};
use crate::core_class_specs::{NativeClassSpec, NativeRecordKind};
use crate::canonical_vm_native::CanonicalVmNative;
use crate::core_native_catalog::CoreNative;
use crate::core_records::ClassRecord;
use crate::object_archive::ObjectArchive;
use hp2_abi::engine::generated::{UFunction, UObject};
use std::alloc::Layout;
use std::ffi::c_void;
use std::mem::{align_of, size_of, MaybeUninit};
use std::ptr::NonNull;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TeardownState {
    Unpublished,
    Live,
    Destroying,
    Destroyed,
    Finalizing,
    Finalized,
}

pub struct OwnedObject {
    allocation: NonNull<UObject>,
    layout: Layout,
    teardown: TeardownState,
    teardown_bucket: i32,
}

impl OwnedObject {
    pub(crate) fn allocate_zeroed(layout: Layout) -> RuntimeResult<Self> {
        if layout.size() < size_of::<UObject>() || layout.align() < align_of::<UObject>() {
            return Err(RuntimeError::new("Invalid UObject allocation layout"));
        }
        let allocation = unsafe {
            let raw = crate::unix_alloc::system_malloc_layout(layout).cast::<UObject>();
            raw.cast::<u8>().write_bytes(0, layout.size());
            NonNull::new_unchecked(raw)
        };
        Ok(Self {
            allocation,
            layout,
            teardown: TeardownState::Unpublished,
            teardown_bucket: 0,
        })
    }

    pub(crate) fn allocate(
        layout: Layout,
        class: *mut c_void,
        outer: *mut c_void,
        name: i32,
        flags: u32,
    ) -> RuntimeResult<Self> {
        let allocation = Self::allocate_zeroed(layout)?;
        unsafe {
            initialize_header(
                allocation.as_ptr().cast(),
                class,
                outer,
                name,
                flags,
                Hp2ObjectHeaderKind::Allocated,
            );
        }
        Ok(allocation)
    }

    pub fn as_ptr(&self) -> *mut UObject {
        self.allocation.as_ptr()
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub(crate) fn publish(&mut self) {
        self.teardown = TeardownState::Live;
        self.teardown_bucket = 0;
    }

    pub(crate) fn needs_teardown(&self) -> bool {
        self.teardown == TeardownState::Live
    }

    pub(crate) fn begin_teardown(&mut self) {
        self.teardown = TeardownState::Destroying;
    }

    pub(crate) fn is_destroying(&self) -> bool {
        self.teardown == TeardownState::Destroying
    }

    pub(crate) fn is_finalizing(&self) -> bool {
        self.teardown == TeardownState::Finalizing
    }

    pub(crate) fn begin_finalization(&mut self) -> bool {
        if self.teardown != TeardownState::Destroyed {
            return false;
        }
        self.teardown = TeardownState::Finalizing;
        true
    }

    pub(crate) fn finish_finalization(&mut self) {
        if self.teardown == TeardownState::Finalizing {
            self.teardown = TeardownState::Finalized;
        }
    }

    pub(crate) fn teardown_complete(&self) -> bool {
        matches!(self.teardown, TeardownState::Unpublished | TeardownState::Finalized)
    }

    /// # Safety
    /// Virtual Destroy completed and the allocation is still retained for final release.
    pub(crate) unsafe fn finish_teardown(&mut self, bucket: i32) {
        self.teardown_bucket = bucket;
        self.teardown = TeardownState::Destroyed;
    }

    pub(crate) fn teardown_bucket(&self) -> Option<i32> {
        matches!(
            self.teardown,
            TeardownState::Destroyed | TeardownState::Finalizing | TeardownState::Finalized
        )
        .then_some(self.teardown_bucket)
    }
}

impl Drop for OwnedObject {
    fn drop(&mut self) {
        if !self.teardown_complete() {
            // A raw table reset must never bypass ordered member finalization.
            // This is an ownership invariant failure, not error transport.
            eprintln!("Core object allocation released before ordered finalization");
            std::process::abort();
        }
        // SAFETY: this unique allocation came from the Core system allocator.
        unsafe { libc::free(self.allocation.as_ptr().cast()) };
    }
}

/// Complete native-identity record followed by Rust-only class operation metadata.
/// The extra metadata is not reflected property storage or a second class table.
#[repr(C)]
pub(crate) struct OwnedClassRecord {
    pub raw: ClassRecord,
    data: MaybeUninit<ClassData>,
    initialized: bool,
}

/// Ordinary Rust operation functions; never an Itanium/C++ vtable.
#[derive(Clone, Copy, Debug)]
pub struct ClassOps {
    pub construct: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    /// Implements native virtual Destroy; automatic members remain live.
    pub destroy: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    /// Runs once after all explicit Destroy work and before identity/allocation release.
    pub finalize_members: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    pub serialize: for<'a, 'b> unsafe fn(CoreAccess<'a>, *mut UObject, &ObjectArchive<'b>) -> RuntimeResult<()>,
    pub static_constructor: for<'a> unsafe fn(CoreAccess<'a>, *mut hp2_abi::engine::generated::UClass, *mut UObject) -> RuntimeResult<()>,
    pub post_load: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    pub initialize_execution: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    pub post_edit_change: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    pub visit_references: for<'a, 'b> unsafe fn(CoreAccess<'a>, *mut UObject, &ObjectArchive<'b>) -> RuntimeResult<()>,
    pub is_pending_kill: for<'a> unsafe fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<bool>,
    pub process_remote_function: for<'a> unsafe fn(
        CoreAccess<'a>, *mut UObject, *mut UFunction, *mut c_void, *mut hp2_abi::engine::generated::FFrame,
    ) -> RuntimeResult<bool>,
    pub execute_event: for<'a> unsafe fn(
        CoreAccess<'a>, *mut UObject, *mut UFunction, *mut c_void, *mut c_void,
    ) -> RuntimeResult<()>,
}

/// Class metadata remains in the class allocation. All members are copied before
/// calling operations; no reference into this record survives reentry.
#[derive(Clone, Copy)]
pub(crate) struct ClassData {
    pub kind: NativeRecordKind,
    pub ops: ClassOps,
    pub constructor_identity: usize,
    pub native_spec: Option<&'static NativeClassSpec>,
}

impl OwnedClassRecord {
    /// # Safety
    /// The complete allocation contains this record and has not been deallocated.
    pub(crate) unsafe fn initialize(record: *mut Self, data: ClassData) {
        unsafe {
            std::ptr::addr_of_mut!((*record).data).write(MaybeUninit::new(data));
            std::ptr::addr_of_mut!((*record).initialized).write(true);
        }
    }

    /// # Safety
    /// The pointer identifies a live canonical class record, not an arbitrary
    /// UObject or a reflected default-value byte buffer.
    pub(crate) unsafe fn metadata(record: *const Self) -> RuntimeResult<ClassData> {
        if !unsafe { (*record).initialized } {
            return Err(RuntimeError::new("Class metadata is not initialized"));
        }
        Ok(unsafe { (*record).data.assume_init_read() })
    }

    /// Reads only the record kind. `metadata` copies the whole `ClassData`, including every
    /// class operation pointer, which showed up in save/load profiles via `record_kind`.
    ///
    /// # Safety
    /// Same contract as [`Self::metadata`].
    pub(crate) unsafe fn kind(record: *const Self) -> RuntimeResult<NativeRecordKind> {
        if !unsafe { (*record).initialized } {
            return Err(RuntimeError::new("Class metadata is not initialized"));
        }
        Ok(unsafe { (*(*record).data.as_ptr()).kind })
    }

    /// # Safety
    /// The pointer is a live class record with an initialized metadata tail.
    pub(crate) unsafe fn replace_metadata(record: *mut Self, data: ClassData) {
        unsafe {
            if (*record).initialized {
                (*record).data.assume_init_drop();
            }
            std::ptr::addr_of_mut!((*record).data).write(MaybeUninit::new(data));
            std::ptr::addr_of_mut!((*record).initialized).write(true);
        }
    }
}

/// A resolved canonical native implementation retained by its owned function.
#[derive(Clone, Copy, Debug)]
pub enum CanonicalNativeBinding {
    /// An intrinsic implementation from Core's immutable catalog.
    Core(CoreNative),
    /// A Rust implementation registered by the canonical owner's subsystem.
    External(CanonicalVmNative),
}

/// A function's direct native binding is owned by that function's stable record.
#[repr(C)]
pub(crate) struct OwnedFunctionRecord {
    pub raw: UFunction,
    binding: MaybeUninit<Option<CanonicalNativeBinding>>,
    initialized: bool,
}

impl OwnedFunctionRecord {
    /// # Safety
    /// The complete allocation contains this function record.
    pub(crate) unsafe fn bind(record: *mut Self, binding: Option<CanonicalNativeBinding>) {
        unsafe {
            std::ptr::addr_of_mut!((*record).binding).write(MaybeUninit::new(binding));
            std::ptr::addr_of_mut!((*record).initialized).write(true);
        }
    }

    /// # Safety
    /// The pointer identifies a live canonical function record.
    pub(crate) unsafe fn binding(record: *const Self) -> RuntimeResult<Option<CanonicalNativeBinding>> {
        if !unsafe { (*record).initialized } {
            return Err(RuntimeError::new("Function metadata is not initialized"));
        }
        Ok(unsafe { (*record).binding.assume_init_read() })
    }
}
