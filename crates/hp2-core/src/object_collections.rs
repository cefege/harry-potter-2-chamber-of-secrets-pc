//! Rust-owned UObject pointer collections. Engine-thread access is serialized.
//! Callback traversals use raw owner pointers and reacquire each slot after reentry.
use std::ffi::c_void;
use std::ptr;

/// Stable C ABI ordinals shared by native collection services and Rust policy.
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectCollectionKind {
    Hp2ObjectRoots,
    Hp2ObjectLoaders,
    Hp2ObjectLoaded,
    Hp2ObjectRegistrants,
}

pub struct ObjectCollection {
    objects: Vec<*mut c_void>,
}

impl Default for ObjectCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectCollection {
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }
    pub fn len(&self) -> i32 {
        self.objects.len() as i32
    }
    pub fn capacity(&self) -> usize {
        self.objects.capacity()
    }
    pub fn at(&self, index: i32) -> *mut c_void {
        self.objects[index as usize]
    }
    pub fn contains(&self, object: *mut c_void) -> bool {
        self.objects.contains(&object)
    }
    pub fn append(&mut self, object: *mut c_void) {
        self.objects.push(object);
    }
    pub fn remove_all(&mut self, object: *mut c_void) {
        self.objects.retain(|&entry| entry != object);
        if self.objects.is_empty() {
            self.release();
        }
    }
    pub(crate) fn replace_if(
        &mut self,
        index: i32,
        previous: *mut c_void,
        replacement: *mut c_void,
    ) {
        if let Some(slot) = self.objects.get_mut(index as usize) {
            if *slot == previous {
                *slot = replacement;
            }
        }
    }
    pub(crate) fn take_if(&mut self, index: i32, expected: *mut c_void) -> bool {
        if self.objects.get(index as usize).copied() != Some(expected) {
            return false;
        }
        self.objects.remove(index as usize);
        if self.objects.is_empty() {
            self.release();
        }
        true
    }
    pub fn release(&mut self) {
        self.objects = Vec::new();
    }
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// Visit the live list, including additions made by registration hooks. Release
/// backing only after successful completion, preserving partial state on error.
/// # Safety
/// Owner/context stay live; callbacks capture native exceptions. Reentry is
/// serialized and may grow the collection; no element address escapes a hook.
pub unsafe fn drain(
    collection: *mut ObjectCollection,
    context: *mut c_void,
    visit: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
) -> Result<(), i32> {
    unsafe {
        let mut index = 0;
        while index < (*collection).len() {
            let object = (*collection).at(index);
            status(visit(context, object))?;
            index += 1;
        }
        (*collection).release();
        Ok(())
    }
}

/// Invoke the original native Sort primitive, preserving its exact tie order.
/// # Safety
/// The callback may only reorder the supplied suffix and read object fields.
/// It must not grow, release or otherwise mutate collection storage or unwind.
/// Neither a Rust reference nor the span survives the callback.
pub unsafe fn sort_suffix(
    collection: *mut ObjectCollection,
    first: i32,
    sort: unsafe extern "C" fn(*mut *mut c_void, i32) -> i32,
) -> Result<(), i32> {
    unsafe {
        let count = (*collection).len() - first;
        assert!(first >= 0 && count >= 0);
        if count == 0 {
            return Ok(());
        }
        let data = (*collection).objects.as_mut_ptr().add(first as usize);
        status(sort(data, count))
    }
}

#[repr(C)]
pub struct Hp2ObjectCollectionArchiveServices {
    pub count_bytes: unsafe extern "C" fn(*mut c_void, usize, usize) -> i32,
    pub is_loading: unsafe extern "C" fn(*mut c_void) -> i32,
    pub count: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub object: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
}

/// Archive the root list using native compact-count and UObject archive leaves.
/// Loading publishes one slot before each object leaf, including its written
/// value on failure. Saving/reference visitation follows the live list.
/// # Safety
/// Owner/archive/services stay live. Native leaves capture exceptions. A leaf
/// receives the actual slot: writes are visible to reentry immediately. It may
/// grow storage after its final slot access, but must not access that old slot
/// after growth or remove it while in use. No Rust reference spans a callback.
pub unsafe fn serialize(
    collection: *mut ObjectCollection,
    archive: *mut c_void,
    services: &Hp2ObjectCollectionArchiveServices,
) -> Result<(), i32> {
    unsafe {
        let width = std::mem::size_of::<*mut c_void>();
        let used = (*collection).len() as usize * width;
        let allocated = (*collection).capacity() * width;
        status((services.count_bytes)(archive, used, allocated))?;
        let loading = (services.is_loading)(archive) != 0;
        let mut count = (*collection).len();
        status((services.count)(archive, &mut count))?;
        if loading {
            (*collection).release();
            assert!(count >= 0);
            (*collection).objects.reserve_exact(count as usize);
        }
        let mut index = 0;
        while index < if loading { count } else { (*collection).len() } {
            let slot = if loading { (*collection).len() } else { index };
            if loading {
                (*collection).append(ptr::null_mut());
            }
            let object = (*collection).objects.as_mut_ptr().add(slot as usize);
            // Native archives historically received the actual array slot.
            // A leaf may grow the collection after writing; never reuse this
            // address after returning, even on error.
            status((services.object)(archive, object))?;
            index += 1;
        }
        Ok(())
    }
}

/// Rust-owned TArray-compatible pointer storage for deferred localization.
/// `num` is a distinct live header slot because compact-count archive callbacks
/// can publish it to synchronous reentry before returning. The allocation holds
/// uninitialized pointer slots beyond that prefix, just as the SDK array does.
pub struct PendingLocalized {
    slots: Vec<std::mem::MaybeUninit<*mut c_void>>,
    num: i32,
}

impl Default for PendingLocalized {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingLocalized {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            num: 0,
        }
    }
    pub fn len(&self) -> i32 {
        self.num
    }
    pub fn contains(&self, object: *mut c_void) -> bool {
        (0..self.num).any(|i| unsafe { self.slots[i as usize].assume_init() == object })
    }
    fn capacity(&self) -> usize {
        self.slots.len()
    }
    fn resize(&mut self, capacity: usize) {
        if capacity > self.slots.len() {
            self.slots.reserve_exact(capacity - self.slots.len());
            // MaybeUninit has no initialization or destruction work.
            unsafe {
                self.slots.set_len(capacity);
            }
        } else {
            self.slots.truncate(capacity);
            self.slots.shrink_to_fit();
        }
    }
    fn add_slot(&mut self) -> usize {
        let index = self.num as usize;
        self.num += 1;
        let count = self.num as usize;
        if count > self.capacity() {
            self.resize(count + count * 3 / 8 + 32);
        }
        index
    }
    pub fn append(&mut self, object: *mut c_void) {
        let index = self.add_slot();
        self.slots[index].write(object);
    }
    pub fn remove_first(&mut self) -> *mut c_void {
        let object = unsafe { self.slots[0].assume_init() };
        self.slots.copy_within(1..self.num as usize, 0);
        self.num -= 1;
        let count = self.num as usize;
        let capacity = self.capacity();
        let slack = capacity - count;
        if (count * 3 < capacity * 2 || slack * std::mem::size_of::<*mut c_void>() >= 16384)
            && (slack > 64 || count == 0)
        {
            self.resize(count);
        }
        object
    }
}

/// # Safety
/// Engine-thread access is serialized. Archive services receive the actual
/// header/element slot and capture exceptions. A callback may append after its
/// last access to an element, but must not use that pointer after reallocation.
/// No Rust owner/Vec reference survives a callback, including on failure.
pub unsafe fn serialize_pending(
    pending: *mut PendingLocalized,
    archive: *mut c_void,
    services: &Hp2ObjectCollectionArchiveServices,
) -> Result<(), i32> {
    unsafe {
        let width = std::mem::size_of::<*mut c_void>();
        status((services.count_bytes)(
            archive,
            (*pending).len() as usize * width,
            (*pending).capacity() * width,
        ))?;
        if (services.is_loading)(archive) != 0 {
            let mut count = 0;
            status((services.count)(archive, &mut count))?;
            (*pending).num = 0;
            (*pending).resize(count as usize);
            for _ in 0..count {
                let index = (*pending).add_slot();
                let slot = (*pending).slots.as_mut_ptr().add(index).cast();
                status((services.object)(archive, slot))?;
            }
        } else {
            status((services.count)(archive, &raw mut (*pending).num))?;
            let mut index = 0;
            while index < (*pending).len() {
                let slot = (*pending).slots.as_mut_ptr().add(index as usize).cast();
                status((services.object)(archive, slot))?;
                index += 1;
            }
        }
        Ok(())
    }
}
