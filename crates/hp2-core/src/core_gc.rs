//! Direct object reachability and purge adapters over canonical Core storage.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive};
use crate::object_gc::{
    self, GcCollectOps, GcDecisionOps, GcMarkInitializationOps, GcMarkOps, GcNameOps, GcObjectOps,
    GcRootOps, GcTableOps, MarkValidation, ReferenceArchiveOps, ReferenceMarkOps,
    ReferencePolicy,
};
use crate::object_lifecycle::{self, PurgeOps};
use hp2_abi::engine::generated::{FNameEntry, UObject};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;

/// Owned GC diagnostics contain copied names, so the consumer may reenter Core.
#[derive(Debug)]
pub enum GcLog {
    Collecting,
    PurgeBegin { no_gc: i32 },
    GarbageCollected { index: i32, object: Vec<u32> },
    NameDeleted { index: i32, name: Vec<u32> },
    Counts { before: i32, remaining: i32, references: i32 },
    Referenced { object: Vec<u32>, context: Option<Vec<u32>> },
    Marked { object: Vec<u32>, context: Option<Vec<u32>> },
    SerializeRouteFailure { object: Vec<u32> },
    ValidationNull,
    ValidationInvalidIndex { index: i32, object: Vec<u32> },
    ValidationEmptySlot { index: i32, object: Vec<u32> },
    ValidationOtherObject { index: i32, object: Vec<u32>, other: Vec<u32> },
}

struct MarkState {
    context: Cell<*mut c_void>,
    ref_obj: Cell<*mut c_void>,
    references: Cell<i32>,
    archive: Cell<*const c_void>,
}

impl Default for MarkState {
    fn default() -> Self {
        Self {
            context: Cell::new(ptr::null_mut()),
            ref_obj: Cell::new(ptr::null_mut()),
            references: Cell::new(0),
            archive: Cell::new(ptr::null()),
        }
    }
}

struct ArchiveScope<'a, 'archive> {
    slot: &'a Cell<*const c_void>,
    previous: *const c_void,
    _archive: &'a ObjectArchive<'archive>,
}

impl Drop for ArchiveScope<'_, '_> {
    fn drop(&mut self) {
        self.slot.set(self.previous);
    }
}

/// Private raw-pointer bridge; public CoreAccess never implements GC policy traits.
struct DirectGc<'a, 'b> {
    access: CoreAccess<'a>,
    marks: MarkState,
    output: &'b dyn Fn(GcLog) -> RuntimeResult<()>,
    no_gc: i32,
    /// Emit the per-object edge/destroy events (each copies full names).
    trace: bool,
}

impl DirectGc<'_, '_> {
    fn activate_archive<'scope, 'archive>(
        &'scope self,
        archive: &'scope ObjectArchive<'archive>,
    ) -> ArchiveScope<'scope, 'archive> {
        let previous = self.marks.archive.replace((archive as *const ObjectArchive<'archive>).cast());
        ArchiveScope {
            slot: &self.marks.archive,
            previous,
            _archive: archive,
        }
    }
    /// Visit slots with the one active source archive; native GC recurses on the
    /// same FArchive, so each edge must not allocate another archive on the stack.
    fn visit_reference_slot(&self, slot: *mut *mut UObject) -> RuntimeResult<()> {
        let archive = self.marks.archive.get();
        if archive.is_null() {
            return Err(RuntimeError::new("GC reference archive is not active"));
        }
        let validation = if cfg!(debug_assertions) {
            MarkValidation::Slow
        } else {
            MarkValidation::Fast
        };
        // SAFETY: the active archive is installed by a scope that outlives this
        // synchronous mapping callback and matches the archive invoking it.
        unsafe {
            object_gc::mark_object_with(
                &*archive.cast::<ObjectArchive<'_>>(),
                slot.cast(),
                validation,
                self,
            )
        }
    }

    fn emit(&self, event: GcLog) -> RuntimeResult<()> {
        (self.output)(event)
    }

    fn object_flags_checked(&self, object: *mut c_void) -> RuntimeResult<u32> {
        if object.is_null() {
            return Err(RuntimeError::new("Null object reference"));
        }
        // SAFETY: marker inputs are object slots from canonical storage or a live
        // reflected reference slot; engine-thread access excludes concurrent mutation.
        Ok(unsafe { (*object.cast::<UObject>()).object_flags })
    }

    fn set_object_flags_checked(&self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        if object.is_null() {
            return Err(RuntimeError::new("Null object reference"));
        }
        // SAFETY: see object_flags_checked; only the flags word is changed.
        unsafe { (*object.cast::<UObject>()).object_flags |= flags };
        Ok(())
    }

    fn clear_object_flags_checked(&self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        if object.is_null() {
            return Err(RuntimeError::new("Null object reference"));
        }
        // SAFETY: see object_flags_checked; only the flags word is changed.
        unsafe { (*object.cast::<UObject>()).object_flags &= !flags };
        Ok(())
    }

    fn name_flags_checked(&self, name: *mut c_void) -> RuntimeResult<u32> {
        if name.is_null() {
            return Err(RuntimeError::new("Invalid name entry"));
        }
        // SAFETY: callers obtain this pointer from the canonical name table.
        Ok(unsafe { (*name.cast::<FNameEntry>()).flags })
    }

    fn copied_name(&self, name: *mut c_void) -> RuntimeResult<Vec<u32>> {
        if name.is_null() {
            return Err(RuntimeError::new("Invalid name entry"));
        }
        // SAFETY: callers obtain this pointer from the canonical name table. The
        // copy ends before any logging callback can reenter or remove the entry.
        let name = unsafe { &(*name.cast::<FNameEntry>()).name };
        Ok(name.iter().take_while(|&&unit| unit != 0).copied().collect())
    }

    fn append_name(&self, output: &mut Vec<u32>, index: i32) -> RuntimeResult<()> {
        let name = self.access.name_at(index);
        if name.is_null() {
            return Err(RuntimeError::new("Invalid name index"));
        }
        // SAFETY: name_at returns a live entry from the canonical name table.
        let name = unsafe { &(*name).name };
        output.extend(name.iter().take_while(|&&unit| unit != 0).copied());
        Ok(())
    }

    fn full_name(&self, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
        if object.is_null() {
            return Err(RuntimeError::new("Null object reference"));
        }

        // GetFullName is the class name, a space, then the outer path. Copy each
        // canonical FName before building the owned diagnostic passed to logging.
        let class = unsafe { (*object).class.cast::<UObject>() };
        if class.is_null() {
            return Err(RuntimeError::new("Object has no class"));
        }
        let class_index = unsafe { i32::from_ne_bytes((*class).name) };
        let mut result = Vec::new();
        self.append_name(&mut result, class_index)?;
        result.push(u32::from(' '));

        let limit = self.access.object_count().max(0) as usize;
        let mut chain = Vec::new();
        let mut current = object;
        while !current.is_null() {
            if chain.len() >= limit {
                return Err(RuntimeError::new("Invalid object outer chain"));
            }
            chain.push(unsafe { i32::from_ne_bytes((*current).name) });
            current = unsafe { (*current).outer.cast() };
        }
        for (position, index) in chain.iter().rev().copied().enumerate() {
            if position != 0 {
                result.push(u32::from('.'));
            }
            self.append_name(&mut result, index)?;
        }
        Ok(result)
    }

    fn validate_object(&self, object: *mut c_void) -> RuntimeResult<()> {
        if object.is_null() {
            self.emit(GcLog::ValidationNull)?;
            return Err(RuntimeError::new("Invalid object reference"));
        }

        let object = object.cast::<UObject>();
        let index = unsafe { (*object).index };
        let count = self.access.object_count();
        if index < 0 || index >= count {
            self.emit(GcLog::ValidationInvalidIndex {
                index,
                object: self.full_name(object)?,
            })?;
            return Err(RuntimeError::new("Invalid object index"));
        }
        let found = self.access.object_at(index);
        if found.is_null() {
            self.emit(GcLog::ValidationEmptySlot {
                index,
                object: self.full_name(object)?,
            })?;
            return Err(RuntimeError::new("Empty object slot"));
        }
        if found != object {
            self.emit(GcLog::ValidationOtherObject {
                index,
                object: self.full_name(object)?,
                other: self.full_name(found)?,
            })?;
            return Err(RuntimeError::new("Object slot contains another object"));
        }
        Ok(())
    }

    fn serialize_roots(&self, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
        let mut index = 0;
        while index < self.access.root_count() {
            let mut root = self.access.root_at(index);
            let previous = root;
            // SAFETY: the local is a writable pointer-sized slot. The canonical
            // collection is updated only after callbacks by its conditional helper.
            unsafe { archive.serialize_object_slot(&mut root)? };
            if root != previous {
                // SAFETY: the helper re-fetches this index and commits only if
                // the original pointer is still present after callback reentry.
                unsafe { self.access.replace_root(index, previous, root) };
            }
            index += 1;
        }
        Ok(())
    }

    fn serialize_pending_localized(&self, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
        let mut index = 0;
        while index < self.access.pending_localized_count() {
            let mut root = self.access.pending_localized_at(index);
            let previous = root;
            // SAFETY: same live-local-slot contract as serialize_roots.
            unsafe { archive.serialize_object_slot(&mut root)? };
            if root != previous {
                // SAFETY: the helper conditionally commits by current index/value.
                unsafe { self.access.replace_pending_localized(index, previous, root) };
            }
            index += 1;
        }
        Ok(())
    }

    fn serialize_root_object(
        &self,
        archive: &ObjectArchive<'_>,
        object: *mut c_void,
    ) -> RuntimeResult<()> {
        let mut root = object.cast::<UObject>();
        // Native SerializeRootSet archives a local UObject* copy from each live
        // table lookup; elimination changes that local, not the canonical table slot.
        unsafe { archive.serialize_object_slot(&mut root) }
    }

}


fn gc_archive_flags() -> ArchiveFlags {
    let defaults = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    ArchiveFlags::new(defaults.version as u16, defaults.licensee as u16, false)
}

impl ArchiveMapping for DirectGc<'_, '_> {
    fn load_object(&self, _index: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new("Reference archive cannot load objects"))
    }

    fn save_object(&self, _object: *mut UObject) -> RuntimeResult<i32> {
        Err(RuntimeError::new("Reference archive cannot save objects"))
    }

    fn load_name(&self, _local_index: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new("Reference archive cannot load names"))
    }

    fn save_name(&self, _global_index: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new("Reference archive cannot save names"))
    }

    fn visit_object(&self, object_slot: *mut *mut UObject) -> RuntimeResult<()> {
        self.visit_reference_slot(object_slot)
    }

    fn visit_name(&self, name_index: i32) -> RuntimeResult<()> {
        let name = self.access.name_at(name_index);
        if name.is_null() {
            return Err(RuntimeError::new("Invalid name index"));
        }
        // SAFETY: name_at returns a live mutable entry in canonical name storage.
        unsafe { object_gc::mark_name_with(name.cast(), self) }
    }
}

impl GcObjectOps for DirectGc<'_, '_> {
    type Error = RuntimeError;

    fn object_flags(&self, object: *mut c_void) -> RuntimeResult<u32> {
        self.object_flags_checked(object)
    }

    fn set_object_flags(&self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        self.set_object_flags_checked(object, flags)
    }

    fn clear_object_flags(&self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        self.clear_object_flags_checked(object, flags)
    }
}

impl GcTableOps for DirectGc<'_, '_> {
    fn object_count(&self) -> RuntimeResult<i32> {
        Ok(self.access.object_count())
    }

    fn object_at(&self, index: i32) -> RuntimeResult<*mut c_void> {
        Ok(self.access.object_at(index).cast())
    }
}

impl GcMarkInitializationOps for DirectGc<'_, '_> {
    fn reset_mark_counter(&self) {
        self.marks.references.set(0);
    }

    fn name_count(&self) -> RuntimeResult<i32> {
        Ok(self.access.name_count())
    }

    fn name_at(&self, index: i32) -> RuntimeResult<*mut c_void> {
        Ok(self.access.name_at(index).cast())
    }

    fn set_name_flags(&self, name: *mut c_void, flags: u32) -> RuntimeResult<()> {
        if name.is_null() {
            return Err(RuntimeError::new("Invalid name entry"));
        }
        // SAFETY: name pointers in this adapter come from the canonical name table.
        unsafe { (*name.cast::<FNameEntry>()).flags |= flags };
        Ok(())
    }
}

impl GcNameOps for DirectGc<'_, '_> {
    type Error = RuntimeError;

    fn clear_name_flags(&self, name: *mut c_void, flags: u32) -> RuntimeResult<()> {
        if name.is_null() {
            return Err(RuntimeError::new("Invalid name entry"));
        }
        // SAFETY: mark_name_with is reached only with a canonical name-table entry.
        unsafe { (*name.cast::<FNameEntry>()).flags &= !flags };
        Ok(())
    }
}

impl<'a, 'b, 'archive> GcRootOps<ObjectArchive<'archive>> for DirectGc<'a, 'b> {
    fn serialize_roots(&self, archive: &ObjectArchive<'archive>) -> RuntimeResult<()> {
        DirectGc::serialize_roots(self, archive)
    }

    fn serialize_pending_localized(
        &self,
        archive: &ObjectArchive<'archive>,
    ) -> RuntimeResult<()> {
        DirectGc::serialize_pending_localized(self, archive)
    }

    fn serialize_root_object(
        &self,
        archive: &ObjectArchive<'archive>,
        object: *mut c_void,
    ) -> RuntimeResult<()> {
        DirectGc::serialize_root_object(self, archive, object)
    }
}

impl<'a, 'b, 'archive> GcMarkOps<ObjectArchive<'archive>> for DirectGc<'a, 'b> {
    fn increment_mark_counter(&self) {
        self.marks.references.set(self.marks.references.get().wrapping_add(1));
    }

    fn context(&self) -> *mut c_void {
        self.marks.context.get()
    }

    fn set_context(&self, context: *mut c_void) {
        self.marks.context.set(context);
    }

    fn ref_obj(&self) -> *mut c_void {
        self.marks.ref_obj.get()
    }

    fn set_ref_obj(&self, object: *mut c_void) {
        self.marks.ref_obj.set(object);
    }

    fn validate(&self, object: *mut c_void) -> RuntimeResult<()> {
        self.validate_object(object)
    }

    fn serialize_object(
        &self,
        archive: &ObjectArchive<'archive>,
        object: *mut c_void,
    ) -> RuntimeResult<()> {
        // SAFETY: the object came from a live reference slot and the class operation
        // is copied before it can reenter or mutate canonical storage.
        unsafe { self.access.serialize_object(object.cast(), archive) }
    }

    fn route_mark_error(&self, object: *mut c_void) -> RuntimeResult<()> {
        let name = self.full_name(object.cast())?;
        self.emit(GcLog::SerializeRouteFailure {
            object: name.clone(),
        })?;
        let mut diagnostic = name;
        diagnostic.extend(" failed to route Serialize".chars().map(u32::from));
        Err(RuntimeError::from_utf32(diagnostic))
    }

    fn referenced(&self, object: *mut c_void, context: *mut c_void) -> RuntimeResult<()> {
        if !self.trace { return Ok(()); }
        self.emit(GcLog::Referenced {
            object: self.full_name(object.cast())?,
            context: if context.is_null() {
                None
            } else {
                Some(self.full_name(context.cast())?)
            },
        })
    }

    fn mark_edge(&self, object: *mut c_void, referrer: *mut c_void) -> RuntimeResult<()> {
        if !self.trace { return Ok(()); }
        self.emit(GcLog::Marked {
            object: self.full_name(object.cast())?,
            context: if referrer.is_null() {
                None
            } else {
                Some(self.full_name(referrer.cast())?)
            },
        })
    }
}

impl<'a, 'b, 'archive> ReferenceMarkOps<ObjectArchive<'archive>> for DirectGc<'a, 'b> {
    fn visit_roots(
        &self,
        archive: &ObjectArchive<'archive>,
        keep: u32,
        required: u32,
    ) -> RuntimeResult<()> {
        object_gc::serialize_roots_with(archive, keep, required, self)
    }

    fn referencing(&self, _archive: &ObjectArchive<'archive>) -> *mut c_void {
        self.marks.ref_obj.get()
    }
}

impl ReferenceArchiveOps for DirectGc<'_, '_> {
    type Error = RuntimeError;

    fn with_reference_archive(
        &self,
        original: *mut c_void,
        slot: *mut c_void,
        keep: u32,
        policy: ReferencePolicy,
    ) -> RuntimeResult<i32> {
        self.marks.context.set(ptr::null_mut());
        self.marks.ref_obj.set(original);
        object_gc::initialize_marks_with(self)?;
        let archive = ObjectArchive::reference(gc_archive_flags(), self);
        let _archive_scope = self.activate_archive(&archive);
        // SAFETY: is_referenced_with passes the original live reference target and
        // a writable pointer-sized slot; mutations remain visible on errors.
        unsafe {
            object_gc::reference_marked_with(
                &archive,
                original,
                slot,
                keep,
                policy,
                self,
            )
        }
    }
}

impl<'a, 'b, 'archive> GcCollectOps<ObjectArchive<'archive>> for DirectGc<'a, 'b> {
    type Error = RuntimeError;

    fn serialize_root_set(
        &self,
        archive: &ObjectArchive<'archive>,
        keep: u32,
        required: u32,
    ) -> RuntimeResult<()> {
        object_gc::serialize_roots_with(archive, keep, required, self)
    }

    fn purge_garbage(&self) -> RuntimeResult<()> {
        object_lifecycle::purge_garbage_with(self.no_gc, self)
    }
}

impl PurgeOps for DirectGc<'_, '_> {
    type Error = RuntimeError;

    fn log_begin(&self, no_gc: i32) -> RuntimeResult<()> {
        self.emit(GcLog::PurgeBegin { no_gc })
    }

    fn object_len(&self) -> RuntimeResult<i32> {
        Ok(self.access.object_count())
    }

    fn object_at(&self, index: i32) -> RuntimeResult<*mut c_void> {
        Ok(self.access.object_at(index).cast())
    }

    fn object_flags(&self, object: *mut c_void) -> RuntimeResult<u32> {
        self.object_flags_checked(object)
    }

    fn exit_purge(&self) -> RuntimeResult<bool> {
        Ok(self.access.exit_purge())
    }

    fn log_destroy(&self, index: i32, object: *mut c_void) -> RuntimeResult<()> {
        if !self.trace { return Ok(()); }
        self.emit(GcLog::GarbageCollected {
            index,
            object: self.full_name(object.cast())?,
        })
    }

    fn destroy(&self, index: i32, _object: *mut c_void) -> RuntimeResult<()> {
        // SAFETY: purge has selected the current canonical slot. destroy_slot
        // performs ConditionalDestroy through the complete registered ClassOps.
        unsafe { self.access.destroy_slot(index) }
    }

    fn delete_object(&self, index: i32, _object: *mut c_void) -> RuntimeResult<()> {
        // SAFETY: purge has completed the full destroy pass before this free pass.
        unsafe { self.access.free_slot(index) }
    }

    fn name_len(&self) -> RuntimeResult<i32> {
        Ok(self.access.name_count())
    }

    fn name_at(&self, index: i32) -> RuntimeResult<*mut c_void> {
        Ok(self.access.name_at(index).cast())
    }

    fn name_flags(&self, name: *mut c_void) -> RuntimeResult<u32> {
        self.name_flags_checked(name)
    }

    fn delete_name(&self, index: i32) -> RuntimeResult<()> {
        let name = self.access.name_at(index);
        if name.is_null() {
            return Err(RuntimeError::new("Invalid name entry"));
        }
        self.emit(GcLog::NameDeleted {
            index,
            name: self.copied_name(name.cast())?,
        })?;
        self.access.remove_name(index)
    }

    fn log_counts(&self, before: i32, remaining: i32) -> RuntimeResult<()> {
        self.emit(GcLog::Counts {
            before,
            remaining,
            references: self.marks.references.get(),
        })
    }
}

impl GcDecisionOps for DirectGc<'_, '_> {
    type Error = RuntimeError;

    fn object_flags(&self, object: *mut c_void) -> RuntimeResult<u32> {
        self.object_flags_checked(object)
    }

    fn is_referenced(
        &self,
        slot: *mut c_void,
        keep: u32,
        policy: ReferencePolicy,
    ) -> RuntimeResult<i32> {
        // SAFETY: attempt_delete_with forwards its writable, pointer-sized slot.
        unsafe { object_gc::is_referenced_with(slot, keep, policy, self) }
    }

    fn purge_garbage(&self) -> RuntimeResult<()> {
        object_lifecycle::purge_garbage_with(self.no_gc, self)
    }
}

impl CoreAccess<'_> {
    /// Collects unreachable canonical objects, invoking class hooks and logging copied events.
    ///
    /// # Errors
    /// Returns the first archive, class-hook, purge, or output callback error.
    pub fn collect_garbage(
        self,
        keep: u32,
        no_gc: i32,
        output: &impl Fn(GcLog) -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        self.collect_garbage_with(keep, no_gc, true, output)
    }

    /// Like [`Self::collect_garbage`]; `trace == false` skips the per-object `Referenced`,
    /// `Marked` and `GarbageCollected` events, whose copied full names cost an allocation per
    /// object even when the consumer discards them.
    ///
    /// # Errors
    /// Returns the first archive, class-hook, purge, or output callback error.
    pub fn collect_garbage_with(
        self,
        keep: u32,
        no_gc: i32,
        trace: bool,
        output: &impl Fn(GcLog) -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        let gc = DirectGc {
            access: self,
            marks: MarkState::default(),
            output,
            no_gc,
            trace,
        };
        gc.emit(GcLog::Collecting)?;
        object_gc::initialize_marks_with(&gc)?;
        let archive = ObjectArchive::reference(gc_archive_flags(), &gc);
        let _archive_scope = gc.activate_archive(&archive);
        object_gc::collect_with(&archive, keep, &gc)
    }

    /// Determines whether the object in `slot` is reachable from selected roots.
    ///
    /// # Errors
    /// Returns the first root traversal, class-hook, or output callback error.
    ///
    /// # Safety
    /// `slot` must point to a live, writable, pointer-sized UObject reference slot.
    pub unsafe fn is_referenced(
        self,
        slot: *mut *mut UObject,
        keep: u32,
        policy: ReferencePolicy,
        output: &impl Fn(GcLog) -> RuntimeResult<()>,
    ) -> RuntimeResult<i32> {
        let gc = DirectGc {
            access: self,
            marks: MarkState::default(),
            output,
            no_gc: 0,
            trace: true,
        };
        // SAFETY: forwarded caller contract; generic policy preserves ignored-slot
        // clearing and committed mark/context mutations if recursion returns an error.
        unsafe { object_gc::is_referenced_with(slot.cast(), keep, policy, &gc) }
    }

    /// Deletes a non-native object only when a direct rooted-reference walk misses it.
    ///
    /// # Errors
    /// Returns the first reference, class-hook, purge, or output callback error.
    ///
    /// # Safety
    /// `slot` must point to a live, writable, pointer-sized UObject reference slot.
    pub(crate) unsafe fn attempt_delete(
        self,
        slot: *mut *mut UObject,
        keep: u32,
        policy: ReferencePolicy,
        no_gc: i32,
        output: &impl Fn(GcLog) -> RuntimeResult<()>,
    ) -> RuntimeResult<i32> {
        let gc = DirectGc {
            access: self,
            marks: MarkState::default(),
            output,
            no_gc,
            trace: true,
        };
        // SAFETY: forwarded caller contract; the decision policy only reads and
        // conditionally rewrites the target through the exact supplied slot.
        unsafe { object_gc::attempt_delete_with(slot.cast(), keep, policy, &gc) }
    }
}

