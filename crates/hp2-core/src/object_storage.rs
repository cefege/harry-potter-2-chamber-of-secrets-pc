//! Sole owner of object slots, allocations, reusable indices and hash links.
//! Legacy pointers and Rust-owned records use the same publication and lookup
//! algorithms. Engine-thread raw storage permits reentry without retaining a
//! vector or object borrow across a diagnostic callback.

use crate::core_access::{RuntimeError, RuntimeResult};
use crate::object_owner::OwnedObject;
use hp2_abi::engine::generated::UObject;
use crate::fast_hash::FastMap;
use std::ffi::c_void;
use std::hash::BuildHasherDefault;
use std::ptr::{self, addr_of, addr_of_mut};

pub const HASH_BUCKETS: usize = 4096;

pub fn hash_bucket(name: i32, outer: i32) -> i32 {
    (name ^ outer) & (HASH_BUCKETS as i32 - 1)
}

enum ObjectSlot {
    Empty,
    Legacy(*mut c_void),
    Owned(OwnedObject),
}

impl ObjectSlot {
    fn pointer(&self) -> *mut c_void {
        match self {
            Self::Empty => ptr::null_mut(),
            Self::Legacy(pointer) => *pointer,
            Self::Owned(object) => object.as_ptr().cast(),
        }
    }
}

pub struct ObjectStorage {
    objects: Vec<ObjectSlot>,
    /// Published pointer -> slot index, so membership checks need not scan `objects`.
    indices: FastMap<usize, i32>,
    available: Vec<i32>,
    hash: [*mut c_void; HASH_BUCKETS],
}

impl Default for ObjectStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectStorage {
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
            indices: FastMap::with_hasher(BuildHasherDefault::new()),
            available: Vec::new(),
            hash: [ptr::null_mut(); HASH_BUCKETS],
        }
    }

    pub fn len(&self) -> i32 {
        self.objects.len() as i32
    }
    pub fn at(&self, index: i32) -> *mut c_void {
        self.objects
            .get(index as usize)
            .map(ObjectSlot::pointer)
            .unwrap_or(ptr::null_mut())
    }
    /// The slot holding `object`, if it is currently published.
    pub fn index_of(&self, object: *const c_void) -> Option<i32> {
        self.indices.get(&(object as usize)).copied()
    }
    fn forget(&mut self, object: *mut c_void, index: i32) {
        if !object.is_null() && self.indices.get(&(object as usize)) == Some(&index) {
            self.indices.remove(&(object as usize));
        }
    }
    pub fn hash_head(&self, bucket: i32) -> *mut c_void {
        self.hash[bucket as usize]
    }
    pub fn initialize_hash(&mut self) {
        self.hash.fill(ptr::null_mut());
    }
    pub fn release(&mut self) {
        *self = Self::new();
    }
    pub fn remove(&mut self, index: i32) {
        let object = self.objects[index as usize].pointer();
        self.objects[index as usize] = ObjectSlot::Empty;
        self.forget(object, index);
        self.available.push(index);
    }
    pub(crate) fn remove_owned(&mut self, index: i32) -> RuntimeResult<OwnedObject> {
        let Some(slot) = self.objects.get_mut(index as usize) else {
            return Err(RuntimeError::new("Invalid owned object index"));
        };
        if matches!(slot, ObjectSlot::Owned(object) if !object.teardown_complete()) {
            return Err(RuntimeError::new("Object fields have not been destroyed"));
        }
        match std::mem::replace(slot, ObjectSlot::Empty) {
            ObjectSlot::Owned(object) => {
                self.forget(object.as_ptr().cast(), index);
                self.available.push(index);
                Ok(object)
            }
            other => {
                *slot = other;
                Err(RuntimeError::new("Object slot is not Rust-owned"))
            }
        }
    }

    pub fn is_owned(&self, index: i32) -> bool {
        matches!(self.objects.get(index as usize), Some(ObjectSlot::Owned(_)))
    }

    pub(crate) fn needs_teardown(&self, index: i32) -> bool {
        matches!(self.objects.get(index as usize), Some(ObjectSlot::Owned(object)) if object.needs_teardown())
    }

    pub(crate) fn begin_teardown(&mut self, index: i32) {
        if let Some(ObjectSlot::Owned(object)) = self.objects.get_mut(index as usize) {
            object.begin_teardown();
        }
    }

    pub(crate) fn is_destroying(&self, index: i32) -> bool {
        matches!(self.objects.get(index as usize), Some(ObjectSlot::Owned(object)) if object.is_destroying())
    }

    /// # Safety
    /// Virtual Destroy completed; the object remains in this owned slot.
    pub(crate) unsafe fn finish_teardown(&mut self, index: i32, bucket: i32) {
        if let Some(ObjectSlot::Owned(object)) = self.objects.get_mut(index as usize) {
            unsafe { object.finish_teardown(bucket) };
        }
    }

    pub(crate) fn is_finalizing(&self, index: i32) -> bool {
        matches!(self.objects.get(index as usize), Some(ObjectSlot::Owned(object)) if object.is_finalizing())
    }

    pub(crate) fn begin_finalization(&mut self, index: i32) -> bool {
        match self.objects.get_mut(index as usize) {
            Some(ObjectSlot::Owned(object)) => object.begin_finalization(),
            _ => false,
        }
    }

    pub(crate) fn finish_finalization(&mut self, index: i32) {
        if let Some(ObjectSlot::Owned(object)) = self.objects.get_mut(index as usize) {
            object.finish_finalization();
        }
    }

    pub(crate) fn teardown_bucket(&self, index: i32) -> Option<i32> {
        match self.objects.get(index as usize) {
            Some(ObjectSlot::Owned(object)) => object.teardown_bucket(),
            _ => None,
        }
    }

    pub fn occupied_buckets(&self) -> i32 {
        self.hash.iter().filter(|head| !head.is_null()).count() as i32
    }
}

#[repr(C)]
pub struct Hp2ObjectStorageServices {
    /// Native checks in their original order: 0 = free slot, 1 = removed
    /// nonzero, 2 = removed exactly once, 3 = replacement is last free slot.
    pub check: unsafe extern "C" fn(i32, i32) -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

unsafe fn require(
    services: &Hp2ObjectStorageServices,
    kind: i32,
    condition: bool,
) -> Result<(), i32> {
    if condition {
        Ok(())
    } else {
        status(unsafe { (services.check)(kind, 0) })
    }
}

/// # Safety
/// Storage and object are live and engine-thread serialized. Object and any
/// nonnull outer satisfy the compiler-probed UObject layout; named fields are
/// initialized. No whole-object read or native vptr access is performed.
pub unsafe fn hash(storage: *mut ObjectStorage, object: *mut c_void) {
    unsafe {
        let header = object.cast::<UObject>();
        let name = i32::from_ne_bytes(addr_of!((*header).name).read());
        let outer = addr_of!((*header).outer).read().cast::<UObject>();
        let outer_index = if outer.is_null() {
            0
        } else {
            addr_of!((*outer).index).read()
        };
        let bucket = hash_bucket(name, outer_index) as usize;
        let head = (*storage).hash[bucket];
        addr_of_mut!((*header).hash_next).write(head);
        (*storage).hash[bucket] = object;
    }
}

/// # Safety
/// Same contract as hash. Native check failures are returned after all matching
/// links have been removed, and may synchronously reenter storage.
pub unsafe fn unhash(
    storage: *mut ObjectStorage,
    object: *mut c_void,
    bucket: i32,
    services: &Hp2ObjectStorageServices,
) -> Result<(), i32> {
    unsafe { unhash_with(storage, object, bucket, |kind, condition| require(services, kind, condition)) }
}

/// # Safety
/// Same live-table contract as `unhash`; errors retain the removed-link prefix.
pub(crate) unsafe fn unhash_with<E>(
    storage: *mut ObjectStorage,
    object: *mut c_void,
    bucket: i32,
    mut check: impl FnMut(i32, bool) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let mut previous: *mut c_void = ptr::null_mut();
        let mut current = (*storage).hash[bucket as usize];
        let mut removed: i32 = 0;
        while !current.is_null() {
            let next = addr_of!((*current.cast::<UObject>()).hash_next).read();
            if current == object {
                if previous.is_null() {
                    (*storage).hash[bucket as usize] = next;
                } else {
                    addr_of_mut!((*previous.cast::<UObject>()).hash_next).write(next);
                }
                removed = removed.wrapping_add(1);
            } else {
                previous = current;
            }
            current = next;
        }
        check(1, removed != 0)?;
        check(2, removed == 1)
    }
}

/// Select a slot without retaining a storage borrow over a diagnostic callback.
unsafe fn select_slot<E>(
    storage: *mut ObjectStorage,
    mut index: i32,
    check_empty: impl FnOnce(bool) -> Result<(), E>,
) -> Result<i32, E> {
    unsafe {
        if index == -1 {
            if let Some(slot) = (*storage).available.pop() {
                index = slot;
                check_empty((*storage).at(index).is_null())?;
            } else {
                index = (*storage).len();
                (*storage).objects.push(ObjectSlot::Empty);
            }
        }
        Ok(index)
    }
}

unsafe fn publish_slot(storage: *mut ObjectStorage, mut slot: ObjectSlot, index: i32) {
    unsafe {
        if let ObjectSlot::Owned(object) = &mut slot {
            object.publish();
        }
        let object = slot.pointer();
        let storage = &mut *storage;
        let previous = storage.objects[index as usize].pointer();
        storage.forget(previous, index);
        storage.objects[index as usize] = slot;
        storage.indices.insert(object as usize, index);
        addr_of_mut!((*object.cast::<UObject>()).index).write(index);
        hash(storage, object);
    }
}

/// Publish one Rust allocation through the canonical slot/hash algorithms.
///
/// # Safety
/// Storage is engine-thread serialized. The allocation's header and outer are
/// initialized; an explicit index must exist and have been reserved.
pub unsafe fn add_owned(
    storage: *mut ObjectStorage,
    object: OwnedObject,
    index: i32,
) -> RuntimeResult<i32> {
    unsafe {
        let index = select_slot(storage, index, |empty| {
            if empty {
                Ok(())
            } else {
                Err(RuntimeError::new("Objects(Index)==NULL"))
            }
        })?;
        publish_slot(storage, ObjectSlot::Owned(object), index);
        Ok(index)
    }
}

/// # Safety
/// Storage and object satisfy hash; the diagnostic service captures failures and
/// may reenter. Explicit indices must exist. The caller reserves a replacement
/// after its destructor has returned.
pub unsafe fn add(
    storage: *mut ObjectStorage,
    object: *mut c_void,
    index: i32,
    services: &Hp2ObjectStorageServices,
) -> Result<(), i32> {
    unsafe {
        let index = select_slot(storage, index, |empty| require(services, 0, empty))?;
        publish_slot(storage, ObjectSlot::Legacy(object), index);
        Ok(())
    }
}

/// # Safety
/// The native destructor has completed. Reentrant checks may mutate storage;
/// the pop occurs only after the check returns, just as in native replacement.
pub unsafe fn reserve_replacement(
    storage: *mut ObjectStorage,
    index: i32,
    services: &Hp2ObjectStorageServices,
) -> Result<(), i32> {
    unsafe {
        let matches = (&(*storage).available).last().copied() == Some(index);
        require(services, 3, matches)?;
        (*storage).available.pop();
        Ok(())
    }
}
/// Removes a fully finalized owned record from lookup while retaining its
/// allocation for same-address replacement.
///
/// # Safety
/// Final member destruction has completed and the slot is engine-thread serialized.
pub(crate) unsafe fn take_owned_for_replacement(
    storage: *mut ObjectStorage,
    index: i32,
) -> RuntimeResult<OwnedObject> {
    let object = unsafe { (*storage).at(index) };
    if object.is_null() || !unsafe { (*storage).is_owned(index) } {
        return Err(RuntimeError::new("Invalid owned replacement slot"));
    }
    let bucket = unsafe { (*storage).teardown_bucket(index) }
        .ok_or_else(|| RuntimeError::new("Object fields have not been destroyed"))?;
    unsafe {
        unhash_with(storage, object, bucket, |kind, valid| {
            if valid { Ok(()) } else {
                Err(RuntimeError::new(if kind == 1 { "Removed!=0" } else { "Removed==1" }))
            }
        })?;
        (*storage).remove_owned(index)
    }
}

/// Reserves a replacement slot without a foreign diagnostic callback.
///
/// # Safety
/// `index` is the slot whose fully torn-down allocation was just detached.
pub(crate) unsafe fn reserve_owned_replacement(
    storage: *mut ObjectStorage,
    index: i32,
) -> RuntimeResult<()> {
    unsafe {
        if (*storage).available.last().copied() != Some(index) {
            return Err(RuntimeError::new("Objects(LastFreeIndex)==Index"));
        }
        (*storage).available.pop();
        Ok(())
    }
}


/// # Safety
/// The predicate captures native exceptions and may mutate the live table and
/// cursor. Each increment is published before the predicate, including on error.
/// No storage or cursor borrow crosses a callback.
pub unsafe fn next_index(
    storage: *mut ObjectStorage,
    index: *mut i32,
    context: *mut c_void,
    matches: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
) -> Result<(), i32> {
    unsafe {
        loop {
            *index = (*index).wrapping_add(1);
            if *index >= (*storage).len() {
                return Ok(());
            }
            let object = (*storage).at(*index);
            if !object.is_null() {
                let mut found = 0;
                status(matches(object, context, &mut found))?;
                if found != 0 {
                    return Ok(());
                }
            }
        }
    }
}

/// # Safety
/// `matches` is called with each live object and `class`; writes `1` to
/// `found` on an IsA match, `0` otherwise. Never errors.
unsafe extern "C" fn is_a_matches(object: *mut c_void, class: *mut c_void, found: *mut i32) -> i32 {
    unsafe {
        *found = crate::object_model::is_a(object, class) as i32;
        0
    }
}

/// # Safety
/// Same contract as `next_index`, specialized to `UObject::IsA` so the scan
/// never crosses back into C++ per candidate.
pub unsafe fn next_index_is_a(
    storage: *mut ObjectStorage,
    index: *mut i32,
    class: *mut c_void,
) -> Result<(), i32> {
    unsafe { next_index(storage, index, class, is_a_matches) }
}

/// # Safety
/// Visit captures exceptions and preserves the original ascending, live scan.
pub unsafe fn visit(
    storage: *mut ObjectStorage,
    visit: unsafe extern "C" fn(*mut c_void) -> i32,
) -> Result<(), i32> {
    unsafe {
        let mut index = 0;
        while index < (*storage).len() {
            let object = (*storage).at(index);
            if !object.is_null() {
                status(visit(object))?;
            }
            index += 1;
        }
        Ok(())
    }
}
