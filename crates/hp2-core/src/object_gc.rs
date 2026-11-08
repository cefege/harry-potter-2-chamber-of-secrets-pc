//! Root selection and recursive GC reference policy over live engine storage.
//!
//! Generic operations use the canonical owner and Rust archive/class operations;
//! legacy ABI tables remain adapters for unconverted mixed-runtime callers.

use crate::object_lifecycle::{Hp2ObjectLifecycleServices, RF_NATIVE, RF_UNREACHABLE};
use std::ffi::c_void;

pub const RF_TAG_GARBAGE: u32 = 0x0000_0040;
pub const RF_ELIMINATE_OBJECT: u32 = 0x0000_0400;
pub const RF_DEBUG_SERIALIZE: u32 = 0x4000_0000;

#[repr(C)]
pub struct Hp2ObjectGcRootServices {
    pub roots: unsafe extern "C" fn(*mut c_void) -> i32,
    pub pending_localized: unsafe extern "C" fn(*mut c_void) -> i32,
    pub object: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}

#[repr(C)]
pub struct Hp2ObjectGcMarkState {
    pub context: *mut c_void,
    pub ref_obj: *mut c_void,
}

#[repr(C)]
pub struct Hp2ObjectGcMarkServices {
    pub name_count: unsafe extern "C" fn() -> i32,
    pub name_at: unsafe extern "C" fn(i32) -> *mut c_void,
    pub name_set_flags: unsafe extern "C" fn(*mut c_void, u32),
    pub name_clear_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    pub validate: unsafe extern "C" fn(*mut c_void) -> i32,
    pub serialize: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub route_error: unsafe extern "C" fn(*mut c_void) -> i32,
    pub referenced: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}

#[repr(C)]
pub struct Hp2ObjectGcReferenceServices {
    /// The native stack archive calls reference_marked before its frame returns.
    pub with_archive: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        u32,
        i32,
        *const Hp2ObjectGcReferenceServices,
        *mut i32,
    ) -> i32,
    pub roots: unsafe extern "C" fn(*mut c_void, u32, u32) -> i32,
    pub referencing: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
}

#[repr(C)]
pub struct Hp2ObjectGcDecisionServices {
    pub is_referenced: unsafe extern "C" fn(*mut c_void, u32, i32, *mut i32) -> i32,
    pub purge: unsafe extern "C" fn() -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// Controls whether slow-mode pointer validation runs before marking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkValidation {
    Fast,
    Slow,
}

/// Controls whether the reference slot is excluded from a reachability query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferencePolicy {
    IncludeSlot,
    IgnoreSlot,
}

/// Live canonical object-record operations used by recursive GC marking.
pub trait GcObjectOps {
    type Error;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error>;
    fn set_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error>;
    fn clear_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error>;
}

/// Live canonical object-table scan operations used by GC initialization/root selection.
pub trait GcTableOps: GcObjectOps {
    fn object_count(&self) -> Result<i32, Self::Error>;
    fn object_at(&self, index: i32) -> Result<*mut c_void, Self::Error>;
}

/// Additional operations used while resetting both object and name marks.
pub trait GcMarkInitializationOps: GcTableOps {
    fn reset_mark_counter(&self);
    fn name_count(&self) -> Result<i32, Self::Error>;
    fn name_at(&self, index: i32) -> Result<*mut c_void, Self::Error>;
    fn set_name_flags(&self, name: *mut c_void, flags: u32) -> Result<(), Self::Error>;
}

/// Archive callbacks and table access for the live root-object selection scan.
pub trait GcRootOps<A>: GcTableOps {
    fn serialize_roots(&self, archive: &A) -> Result<(), Self::Error>;
    fn serialize_pending_localized(&self, archive: &A) -> Result<(), Self::Error>;
    fn serialize_root_object(
        &self,
        archive: &A,
        object: *mut c_void,
    ) -> Result<(), Self::Error>;
}

/// Serializes engine roots and then selects matching objects from the live table.
pub fn serialize_roots_with<A, O: GcRootOps<A>>(
    archive: &A,
    keep: u32,
    required: u32,
    ops: &O,
) -> Result<(), O::Error> {
    ops.serialize_roots(archive)?;
    ops.serialize_pending_localized(archive)?;
    let mut index = 0;
    while index < ops.object_count()? {
        let object = ops.object_at(index)?;
        if !object.is_null()
            && ops.object_flags(object)? & keep != 0
            && ops.object_flags(object)? & required == required
        {
            ops.serialize_root_object(archive, object)?;
        }
        index += 1;
    }
    Ok(())
}

/// Initializes object/name reachability flags without snapshotting either table.
pub fn initialize_marks_with<O: GcMarkInitializationOps>(ops: &O) -> Result<(), O::Error> {
    ops.reset_mark_counter();
    let mut index = 0;
    while index < ops.object_count()? {
        let object = ops.object_at(index)?;
        if !object.is_null() {
            ops.set_object_flags(object, RF_UNREACHABLE | RF_TAG_GARBAGE)?;
        }
        index += 1;
    }
    let mut index = 0;
    while index < ops.name_count()? {
        let name = ops.name_at(index)?;
        if !name.is_null() {
            ops.set_name_flags(name, RF_UNREACHABLE)?;
        }
        index += 1;
    }
    Ok(())
}

/// Name-table flag operation needed by the recursive archive visitor.
pub trait GcNameOps {
    type Error;

    fn clear_name_flags(&self, name: *mut c_void, flags: u32) -> Result<(), Self::Error>;
}

/// Clears reachability on one archive-visited name.
///
/// # Safety
/// `name` must be a valid mutable name-entry pointer for the duration of the call.
pub unsafe fn mark_name_with<O: GcNameOps>(
    name: *mut c_void,
    ops: &O,
) -> Result<(), O::Error> {
    ops.clear_name_flags(name, RF_UNREACHABLE)
}

/// Recursive object-marker hooks. Implementations store state in engine-thread
/// cells or raw legacy slots; reference callbacks share the active source archive.
pub trait GcMarkOps<A>: GcObjectOps {
    fn increment_mark_counter(&self);
    fn context(&self) -> *mut c_void;
    fn set_context(&self, context: *mut c_void);
    fn ref_obj(&self) -> *mut c_void;
    fn set_ref_obj(&self, object: *mut c_void);
    fn validate(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn serialize_object(&self, archive: &A, object: *mut c_void) -> Result<(), Self::Error>;
    fn route_mark_error(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn referenced(&self, object: *mut c_void, context: *mut c_void) -> Result<(), Self::Error>;
    /// Debug hook for a first-visit mark edge: `object` was reached from
    /// `referrer`. Default no-op; implementations may log the edge.
    fn mark_edge(&self, _object: *mut c_void, _referrer: *mut c_void) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Marks a possibly unaligned object-reference slot and recursively serializes
/// the first reachability edge using the canonical class operation.
///
/// # Safety
/// `slot` must point to a pointer-sized writable reference slot. Implementations
/// must keep state access callback-safe and must not retain object borrows across
/// validation, serialization, or error-routing operations.
pub unsafe fn mark_object_with<A, O: GcMarkOps<A>>(
    archive: &A,
    slot: *mut c_void,
    validation: MarkValidation,
    ops: &O,
) -> Result<(), O::Error> {
    ops.increment_mark_counter();
    let slot = slot.cast::<*mut c_void>();
    let object = unsafe { slot.read_unaligned() };
    if validation == MarkValidation::Slow && !object.is_null() {
        ops.validate(object)?;
    }
    if !object.is_null() && ops.object_flags(object)? & RF_ELIMINATE_OBJECT != 0 {
        // Source GC marking (`FArchiveTagUsed::operator<<`, stock UE1
        // Core/Src/UnObj.cpp) only reads reference slots — marking never
        // rewrites them. An eliminated object is skipped (stays
        // RF_UNREACHABLE, so the purge frees it) while the caller keeps its
        // value, matching the semantics the game scripts run against.
        return Ok(());
    } else if !object.is_null() && ops.object_flags(object)? & RF_UNREACHABLE != 0 {
        ops.clear_object_flags(object, RF_UNREACHABLE | RF_DEBUG_SERIALIZE)?;
        if ops.object_flags(object)? & RF_TAG_GARBAGE != 0 {
            let previous = ops.context();
            ops.mark_edge(object, previous)?;
            ops.set_context(object);
            ops.serialize_object(archive, object)?;
            if ops.object_flags(object)? & RF_DEBUG_SERIALIZE == 0 {
                ops.route_mark_error(object)?;
            }
            ops.set_context(previous);
        } else {
            let context = ops.context();
            ops.referenced(object, context)?;
            if ops.ref_obj() == object {
                ops.set_ref_obj(context);
            }
        }
    }
    unsafe { slot.write_unaligned(object) };
    Ok(())
}

/// Operations needed by reference-archive recursion.
pub trait ReferenceMarkOps<A>: GcObjectOps {
    fn visit_roots(
        &self,
        archive: &A,
        keep: u32,
        required: u32,
    ) -> Result<(), Self::Error>;
    fn referencing(&self, archive: &A) -> *mut c_void;
}
/// Handles the synchronous recursive callback from a reference archive.
///
/// # Safety
/// `slot` must point to a pointer-sized writable reference slot; `original` must
/// remain valid through root traversal. Errors deliberately retain all slot and
/// mark-state mutations already committed.
pub unsafe fn reference_marked_with<A, O: ReferenceMarkOps<A>>(
    archive: &A,
    original: *mut c_void,
    slot: *mut c_void,
    keep: u32,
    policy: ReferencePolicy,
    ops: &O,
) -> Result<i32, O::Error> {
    ops.clear_object_flags(original, RF_TAG_GARBAGE)?;
    ops.visit_roots(archive, keep, RF_TAG_GARBAGE)?;
    let object_slot = slot.cast::<*mut c_void>();
    unsafe { object_slot.write_unaligned(original) };
    if policy == ReferencePolicy::IncludeSlot {
        let referencing = ops.referencing(archive);
        unsafe { object_slot.write_unaligned(referencing) };
    }
    let object = unsafe { object_slot.read_unaligned() };
    Ok(i32::from(
        ops.object_flags(object)? & RF_UNREACHABLE == 0,
    ))
}

/// Opens the archive used by IsReferenced and runs its recursive reference walk.
///
/// Direct implementations create and destroy their archive in this synchronous
/// operation, then call `reference_marked_with` from its object visitor. The
/// legacy adapter keeps its existing native stack archive and reentrant callback.
pub trait ReferenceArchiveOps {
    type Error;

    fn with_reference_archive(
        &self,
        original: *mut c_void,
        slot: *mut c_void,
        keep: u32,
        policy: ReferencePolicy,
    ) -> Result<i32, Self::Error>;
}

/// Determines whether an object is reachable while preserving ignored-slot
/// clearing even when archive traversal fails.
///
/// # Safety
/// `slot` must point to a pointer-sized writable reference slot.
pub unsafe fn is_referenced_with<O: ReferenceArchiveOps>(
    slot: *mut c_void,
    keep: u32,
    policy: ReferencePolicy,
    ops: &O,
) -> Result<i32, O::Error> {
    let object_slot = slot.cast::<*mut c_void>();
    let original = unsafe { object_slot.read_unaligned() };
    if policy == ReferencePolicy::IgnoreSlot {
        unsafe { object_slot.write_unaligned(std::ptr::null_mut()) };
    }
    ops.with_reference_archive(original, slot, keep, policy)
}

/// Archive and purge operations for the top-level collect policy.
pub trait GcCollectOps<A> {
    type Error;

    fn serialize_root_set(
        &self,
        archive: &A,
        keep: u32,
        required: u32,
    ) -> Result<(), Self::Error>;
    fn purge_garbage(&self) -> Result<(), Self::Error>;
}

/// Serializes collection roots before invoking purge, preserving first failure.
pub fn collect_with<A, O: GcCollectOps<A>>(
    archive: &A,
    keep: u32,
    ops: &O,
) -> Result<(), O::Error> {
    ops.serialize_root_set(archive, keep, RF_TAG_GARBAGE)?;
    ops.purge_garbage()
}

/// Operations used by ConditionalDestroy's GC decision point.
pub trait GcDecisionOps {
    type Error;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error>;
    fn is_referenced(
        &self,
        slot: *mut c_void,
        keep: u32,
        policy: ReferencePolicy,
    ) -> Result<i32, Self::Error>;
    fn purge_garbage(&self) -> Result<(), Self::Error>;
}

/// Keeps native objects, and purges only candidates not found reachable.
///
/// # Safety
/// `slot` must point to a pointer-sized writable object reference.
pub unsafe fn attempt_delete_with<O: GcDecisionOps>(
    slot: *mut c_void,
    keep: u32,
    policy: ReferencePolicy,
    ops: &O,
) -> Result<i32, O::Error> {
    let object = unsafe { slot.cast::<*mut c_void>().read_unaligned() };
    if ops.object_flags(object)? & RF_NATIVE != 0 {
        return Ok(0);
    }
    let referenced = ops.is_referenced(slot, keep, policy)?;
    if referenced == 0 {
        ops.purge_garbage()?;
        Ok(1)
    } else {
        Ok(0)
    }
}

/// # Safety
/// Tables, archives and callbacks must remain valid, including after reentry.
pub unsafe fn serialize_roots(
    archive: *mut c_void,
    objects: *mut c_void,
    keep: u32,
    required: u32,
    life: &Hp2ObjectLifecycleServices,
    services: &Hp2ObjectGcRootServices,
) -> Result<(), i32> {
    let mut archive = archive;
    serialize_roots_with(
        &archive,
        keep,
        required,
        &LegacyRootOps {
            table: LegacyTable { objects, life },
            services,
        },
    )
}

/// # Safety
/// The live tables and counter must be writable and valid for each callback.
pub unsafe fn initialize_marks(
    objects: *mut c_void,
    counter: *mut i32,
    life: &Hp2ObjectLifecycleServices,
    services: &Hp2ObjectGcMarkServices,
) -> Result<(), i32> {
    initialize_marks_with(&LegacyInitOps {
        table: LegacyTable { objects, life },
        counter,
        services,
    })
}

/// # Safety
/// Slot is pointer-sized writable storage, possibly unaligned. Archive/state
/// must survive recursive callbacks. No Rust reference to state spans a callback.
pub unsafe fn mark_object(
    archive: *mut c_void,
    state: *mut Hp2ObjectGcMarkState,
    slot: *mut c_void,
    counter: *mut i32,
    slow: i32,
    life: &Hp2ObjectLifecycleServices,
    services: &Hp2ObjectGcMarkServices,
) -> Result<(), i32> {
    let mut archive = archive;
    unsafe {
        mark_object_with(
            &archive,
            slot,
            if slow != 0 {
                MarkValidation::Slow
            } else {
                MarkValidation::Fast
            },
            &LegacyMarkOps {
                life,
                state,
                counter,
                services,
            },
        )
    }
}

/// # Safety
/// Name and native callbacks must satisfy the FName clear-flags contract.
pub unsafe fn mark_name(name: *mut c_void, services: &Hp2ObjectGcMarkServices) -> Result<(), i32> {
    unsafe { mark_name_with(name, &LegacyNameOps { services }) }
}

/// # Safety
/// Archive must be a live native marking archive; purge must not unwind into Rust.
pub unsafe fn collect(
    archive: *mut c_void,
    keep: u32,
    roots: unsafe extern "C" fn(*mut c_void, u32, u32) -> i32,
    purge: unsafe extern "C" fn() -> i32,
) -> Result<(), i32> {
    let mut archive = archive;
    collect_with(&archive, keep, &LegacyCollectOps { roots, purge })
}

/// # Safety
/// Slot must contain a valid non-null UObject pointer. The archive callback
/// constructs its native archive and reenters reference_marked synchronously.
pub unsafe fn is_referenced(
    slot: *mut c_void,
    keep: u32,
    ignore: i32,
    services: &Hp2ObjectGcReferenceServices,
) -> Result<i32, i32> {
    unsafe {
        is_referenced_with(
            slot,
            keep,
            if ignore != 0 {
                ReferencePolicy::IgnoreSlot
            } else {
                ReferencePolicy::IncludeSlot
            },
            &LegacyReferenceArchive {
                services,
                legacy_ignore: ignore,
            },
        )
    }
}

/// # Safety
/// The original object and slot must remain valid throughout root traversal.
/// On failure the legacy ignored slot/context mutations deliberately remain.
pub unsafe fn reference_marked(
    archive: *mut c_void,
    original: *mut c_void,
    slot: *mut c_void,
    keep: u32,
    ignore: i32,
    life: &Hp2ObjectLifecycleServices,
    services: &Hp2ObjectGcReferenceServices,
) -> Result<i32, i32> {
    let mut archive = archive;
    unsafe {
        reference_marked_with(
            &mut archive,
            original,
            slot,
            keep,
            if ignore != 0 {
                ReferencePolicy::IgnoreSlot
            } else {
                ReferencePolicy::IncludeSlot
            },
            &LegacyReferenceMarkOps { life, services },
        )
    }
}

/// # Safety
/// Slot, services and callbacks must satisfy native IsReferenced/PurgeGarbage.
pub unsafe fn attempt_delete(
    slot: *mut c_void,
    keep: u32,
    ignore: i32,
    life: &Hp2ObjectLifecycleServices,
    services: &Hp2ObjectGcDecisionServices,
) -> Result<i32, i32> {
    unsafe {
        attempt_delete_with(
            slot,
            keep,
            if ignore != 0 {
                ReferencePolicy::IgnoreSlot
            } else {
                ReferencePolicy::IncludeSlot
            },
            &LegacyDecisionOps {
                life,
                services,
                legacy_ignore: ignore,
            },
        )
    }
}

struct LegacyTable<'a> {
    objects: *mut c_void,
    life: &'a Hp2ObjectLifecycleServices,
}

impl GcObjectOps for LegacyTable<'_> {
    type Error = i32;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.life.flags)(object) })
    }

    fn set_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.life.set_flags)(object, flags)) }
    }

    fn clear_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.life.clear_flags)(object, flags)) }
    }
}

impl GcTableOps for LegacyTable<'_> {
    fn object_count(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { (self.life.table_len)(self.objects) })
    }

    fn object_at(&self, index: i32) -> Result<*mut c_void, Self::Error> {
        let mut object = std::ptr::null_mut();
        unsafe { status((self.life.table_at)(self.objects, index, &mut object))? };
        Ok(object)
    }
}

struct LegacyRootOps<'a> {
    table: LegacyTable<'a>,
    services: &'a Hp2ObjectGcRootServices,
}

impl GcObjectOps for LegacyRootOps<'_> {
    type Error = i32;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        self.table.object_flags(object)
    }
    fn set_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        self.table.set_object_flags(object, flags)
    }
    fn clear_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        self.table.clear_object_flags(object, flags)
    }
}

impl GcTableOps for LegacyRootOps<'_> {
    fn object_count(&self) -> Result<i32, Self::Error> {
        self.table.object_count()
    }
    fn object_at(&self, index: i32) -> Result<*mut c_void, Self::Error> {
        self.table.object_at(index)
    }
}

impl GcRootOps<*mut c_void> for LegacyRootOps<'_> {
    fn serialize_roots(&self, archive: &*mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.roots)(*archive)) }
    }

    fn serialize_pending_localized(
        &self,
        archive: &*mut c_void,
    ) -> Result<(), Self::Error> {
        unsafe { status((self.services.pending_localized)(*archive)) }
    }

    fn serialize_root_object(
        &self,
        archive: &*mut c_void,
        object: *mut c_void,
    ) -> Result<(), Self::Error> {
        unsafe { status((self.services.object)(*archive, object)) }
    }
}

struct LegacyInitOps<'a> {
    table: LegacyTable<'a>,
    counter: *mut i32,
    services: &'a Hp2ObjectGcMarkServices,
}

impl GcObjectOps for LegacyInitOps<'_> {
    type Error = i32;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        self.table.object_flags(object)
    }
    fn set_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        self.table.set_object_flags(object, flags)
    }
    fn clear_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        self.table.clear_object_flags(object, flags)
    }
}

impl GcTableOps for LegacyInitOps<'_> {
    fn object_count(&self) -> Result<i32, Self::Error> {
        self.table.object_count()
    }
    fn object_at(&self, index: i32) -> Result<*mut c_void, Self::Error> {
        self.table.object_at(index)
    }
}

impl GcMarkInitializationOps for LegacyInitOps<'_> {
    fn reset_mark_counter(&self) {
        unsafe { self.counter.write(0) }
    }

    fn name_count(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { (self.services.name_count)() })
    }

    fn name_at(&self, index: i32) -> Result<*mut c_void, Self::Error> {
        Ok(unsafe { (self.services.name_at)(index) })
    }

    fn set_name_flags(&self, name: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { (self.services.name_set_flags)(name, flags) };
        Ok(())
    }
}

struct LegacyMarkOps<'a> {
    life: &'a Hp2ObjectLifecycleServices,
    state: *mut Hp2ObjectGcMarkState,
    counter: *mut i32,
    services: &'a Hp2ObjectGcMarkServices,
}

impl GcObjectOps for LegacyMarkOps<'_> {
    type Error = i32;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.life.flags)(object) })
    }
    fn set_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.life.set_flags)(object, flags)) }
    }
    fn clear_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.life.clear_flags)(object, flags)) }
    }
}

impl GcMarkOps<*mut c_void> for LegacyMarkOps<'_> {
    fn increment_mark_counter(&self) {
        unsafe { self.counter.write(self.counter.read().wrapping_add(1)) }
    }

    fn context(&self) -> *mut c_void {
        unsafe { (*self.state).context }
    }

    fn set_context(&self, context: *mut c_void) {
        unsafe { (*self.state).context = context }
    }

    fn ref_obj(&self) -> *mut c_void {
        unsafe { (*self.state).ref_obj }
    }

    fn set_ref_obj(&self, object: *mut c_void) {
        unsafe { (*self.state).ref_obj = object }
    }

    fn validate(&self, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.validate)(object)) }
    }

    fn serialize_object(
        &self,
        archive: &*mut c_void,
        object: *mut c_void,
    ) -> Result<(), Self::Error> {
        unsafe { status((self.services.serialize)(*archive, object)) }
    }

    fn route_mark_error(&self, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.route_error)(object)) }
    }

    fn referenced(
        &self,
        object: *mut c_void,
        context: *mut c_void,
    ) -> Result<(), Self::Error> {
        unsafe { status((self.services.referenced)(object, context)) }
    }
}

struct LegacyNameOps<'a> {
    services: &'a Hp2ObjectGcMarkServices,
}

impl GcNameOps for LegacyNameOps<'_> {
    type Error = i32;

    fn clear_name_flags(&self, name: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.services.name_clear_flags)(name, flags)) }
    }
}

struct LegacyCollectOps {
    roots: unsafe extern "C" fn(*mut c_void, u32, u32) -> i32,
    purge: unsafe extern "C" fn() -> i32,
}

impl GcCollectOps<*mut c_void> for LegacyCollectOps {
    type Error = i32;

    fn serialize_root_set(
        &self,
        archive: &*mut c_void,
        keep: u32,
        required: u32,
    ) -> Result<(), Self::Error> {
        unsafe { status((self.roots)(*archive, keep, required)) }
    }

    fn purge_garbage(&self) -> Result<(), Self::Error> {
        unsafe { status((self.purge)()) }
    }
}

struct LegacyReferenceArchive<'a> {
    services: &'a Hp2ObjectGcReferenceServices,
    legacy_ignore: i32,
}

impl ReferenceArchiveOps for LegacyReferenceArchive<'_> {
    type Error = i32;

    fn with_reference_archive(
        &self,
        original: *mut c_void,
        slot: *mut c_void,
        keep: u32,
        _policy: ReferencePolicy,
    ) -> Result<i32, Self::Error> {
        let mut result = 0;
        unsafe {
            status((self.services.with_archive)(
                original,
                slot,
                keep,
                self.legacy_ignore,
                self.services,
                &mut result,
            ))?;
        }
        Ok(result)
    }
}

struct LegacyReferenceMarkOps<'a> {
    life: &'a Hp2ObjectLifecycleServices,
    services: &'a Hp2ObjectGcReferenceServices,
}

impl GcObjectOps for LegacyReferenceMarkOps<'_> {
    type Error = i32;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.life.flags)(object) })
    }
    fn set_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.life.set_flags)(object, flags)) }
    }
    fn clear_object_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.life.clear_flags)(object, flags)) }
    }
}

impl ReferenceMarkOps<*mut c_void> for LegacyReferenceMarkOps<'_> {
    fn visit_roots(
        &self,
        archive: &*mut c_void,
        keep: u32,
        required: u32,
    ) -> Result<(), Self::Error> {
        unsafe { status((self.services.roots)(*archive, keep, required)) }
    }

    fn referencing(&self, archive: &*mut c_void) -> *mut c_void {
        unsafe { (self.services.referencing)(*archive) }
    }
}

struct LegacyDecisionOps<'a> {
    life: &'a Hp2ObjectLifecycleServices,
    services: &'a Hp2ObjectGcDecisionServices,
    legacy_ignore: i32,
}

impl GcDecisionOps for LegacyDecisionOps<'_> {
    type Error = i32;

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.life.flags)(object) })
    }

    fn is_referenced(
        &self,
        slot: *mut c_void,
        keep: u32,
        _policy: ReferencePolicy,
    ) -> Result<i32, Self::Error> {
        let mut result = 0;
        unsafe {
            status((self.services.is_referenced)(
                slot,
                keep,
                self.legacy_ignore,
                &mut result,
            ))?;
        }
        Ok(result)
    }

    fn purge_garbage(&self) -> Result<(), Self::Error> {
        unsafe { status((self.services.purge)()) }
    }
}
