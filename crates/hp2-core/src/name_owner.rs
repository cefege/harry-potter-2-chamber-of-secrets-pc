//!
//! Entries use stable allocations while [`NameStorage`] remains the sole name index.

use std::marker::PhantomData;
use std::rc::Rc;

use hp2_abi::engine::generated::{FName, FNameEntry};

use crate::ansi::Tchar;
use crate::core_access::{RuntimeError, RuntimeResult};
use crate::name::{self, NameStorage, NAME_HASH_BINS, NAME_SIZE, RF_NATIVE};

/// Name insertion behavior corresponding to `EFindName`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindName {
    /// Return index zero without inserting when the name is absent.
    Find,
    /// Insert a dynamic name when absent.
    Add,
    /// Insert as a native name when absent.
    Intrinsic,
    /// Find case-insensitively, preferring uppercase spelling on conflict.
    Define,
}

/// Owns one canonical name table and all name-entry allocations it references.
///
/// This type is confined to its engine thread; it deliberately does not implement
/// `Send` or `Sync`. Entry pointers remain stable until removal or shutdown.
pub struct NameOwner {
    storage: NameStorage,
    intrinsic_slab: Option<Box<[FNameEntry]>>,
    _engine_thread: PhantomData<Rc<()>>,
}

impl Default for NameOwner {
    fn default() -> Self {
        Self::new()
    }
}

impl NameOwner {
    /// Creates an uninitialized owner.
    pub const fn new() -> Self {
        Self {
            storage: NameStorage::new(),
            intrinsic_slab: None,
            _engine_thread: PhantomData,
        }
    }

    /// Initializes the canonical table with independently owned intrinsic entries.
    ///
    /// # Errors
    /// Returns an error if already initialized or the intrinsic catalog is invalid.
    pub fn initialize(&mut self) -> RuntimeResult<()> {
        if self.storage.initialized {
            return Err(RuntimeError::new("Initialized==0"));
        }
        let count = name::hardcoded_count() as usize;
        let mut slab = Vec::with_capacity(count);
        for index in 0..count {
            let Some(spec) = name::intrinsic_spec(index) else {
                return Err(RuntimeError::new("Invalid intrinsic name index"));
            };
            slab.push(spec);
        }
        let mut slab = slab.into_boxed_slice();
        for bin in 0..NAME_HASH_BINS {
            self.storage.bins[bin] = std::ptr::null_mut();
        }
        self.storage.names_len = 0;
        self.storage.available.clear();
        for (index, entry) in slab.iter_mut().enumerate() {
            entry.index = index as i32;
            let entry_ptr = entry as *mut FNameEntry;
            let hash = name::strihash_text(&entry.name) as usize & (NAME_HASH_BINS - 1);
            entry.hash_next = self.storage.bins[hash].cast();
            self.storage.bins[hash] = entry_ptr;
            while self.storage.names_len <= index {
                let slot = self.storage.add_slot();
                self.storage.set_entry(slot, std::ptr::null_mut());
            }
            self.storage.set_entry(index as i32, entry_ptr);
        }
        self.intrinsic_slab = Some(slab);
        self.storage.initialized = true;
        Ok(())
    }

    /// Returns whether initialization has completed.
    pub fn is_initialized(&self) -> bool {
        self.storage.initialized()
    }

    /// Returns the number of canonical slots, including removed slots.
    pub fn len(&self) -> i32 {
        self.storage.len()
    }

    /// Returns whether the table contains no slots.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Interns UTF-32 text using the requested legacy lookup mode.
    /// Add-like modes require text length (excluding NUL) less than `NAME_SIZE`;
    /// `Find` checks longer text without attempting insertion.
    ///
    /// # Errors
    /// Returns an error when uninitialized or when insertion exceeds `NAME_SIZE`.
    pub fn intern(&mut self, text: &[u32], mode: FindName) -> RuntimeResult<FName> {
        if !self.storage.initialized {
            return Err(RuntimeError::new("FName constructed before FName::StaticInit"));
        }
        let length = text.iter().position(|&unit| unit == 0).unwrap_or(text.len());
        if length == 0 {
            return Ok(FName { index: 0 });
        }
        let text = &text[..length];
        let (hash, entry) = lookup(&self.storage, text)?;
        if !entry.is_null() {
            // SAFETY: lookup returned a live table entry owned by this NameOwner.
            if mode == FindName::Intrinsic {
                // FName::SetFlags(RF_Native) also updates an already interned entry.
                unsafe { (*entry).flags |= RF_NATIVE; }
            }
            let index = unsafe { (*entry).index };
            if mode == FindName::Define
                // SAFETY: the live entry returned by lookup remains owned during comparison.
                && compare_units_text(text, unsafe { &(*entry).name }) < 0
            {
                // SAFETY: the live entry is uniquely borrowed through this mutable owner.
                copy_name_text(unsafe { &mut (*entry).name }, text);
            }
            return Ok(FName { index });
        }
        if mode == FindName::Find {
            return Ok(FName { index: 0 });
        }
        let index = match self.storage.available.pop() {
            Some(index) => index,
            None => self.storage.add_slot(),
        };
        if length >= NAME_SIZE {
            return Err(RuntimeError::new("NameLength<NAME_SIZE"));
        }
        let mut name = [0_u32; NAME_SIZE];
        name[..length].copy_from_slice(text);
        let storage = &mut self.storage as *mut NameStorage;
        // SAFETY: this owner has exclusive engine-thread access to its stable table.
        unsafe {
            name::commit_entry(storage, index, hash, |previous| {
                let entry = Box::into_raw(Box::new(FNameEntry {
                    index,
                    flags: if mode == FindName::Intrinsic { RF_NATIVE } else { 0 },
                    hash_next: previous.cast(),
                    name,
                }));
                Ok::<*mut FNameEntry, RuntimeError>(entry)
            })?
        };
        Ok(FName { index })
    }

    /// Finds a canonical name without inserting it.
    ///
    /// # Errors
    /// Returns an error if initialization has not completed.
    pub fn find(&self, text: &[u32]) -> RuntimeResult<FName> {
        if !self.storage.initialized {
            return Err(RuntimeError::new("FName constructed before FName::StaticInit"));
        }
        let length = text.iter().position(|&unit| unit == 0).unwrap_or(text.len());
        if length == 0 {
            return Ok(FName { index: 0 });
        }
        let (_, entry) = lookup(&self.storage, &text[..length])?;
        if entry.is_null() {
            Ok(FName { index: 0 })
        } else {
            Ok(FName {
                // SAFETY: lookup returned a live entry in this owner's canonical table.
                index: unsafe { (*entry).index },
            })
        }
    }


    /// Reports whether a slot index denotes a valid name (zero is always valid).
    pub fn is_valid(&self, index: i32) -> bool {
        self.storage.is_valid(index)
    }
    /// Returns a copied UTF-32 name for a live slot, or `None` if absent.
    pub fn text(&self, index: i32) -> Option<Vec<u32>> {
        self.text_slice(index).map(<[u32]>::to_vec)
    }

    /// Borrows a live slot only while its owner is immutably borrowed.
    pub(crate) fn text_slice(&self, index: i32) -> Option<&[u32]> {
        if index < 0 || index >= self.storage.len() {
            return None;
        }
        let entry = self.storage.entry(index);
        if entry.is_null() {
            return None;
        }
        // SAFETY: this valid non-null slot points to a live entry.
        let text = unsafe { &(*entry).name };
        let len = text.iter().position(|&unit| unit == 0).unwrap_or(text.len());
        Some(&text[..len])
    }

    /// Returns the flags for a live slot.
    pub fn flags(&self, index: i32) -> Option<u32> {
        if index < 0 || index >= self.storage.len() {
            return None;
        }
        let entry = self.storage.entry(index);
        // SAFETY: this valid non-null slot points to a live entry.
        (!entry.is_null()).then(|| unsafe { (*entry).flags })
    }

    /// Returns the live name-entry and occupied-bucket counts used by DisplayHash.
    pub fn hash_stats(&self) -> (i32, i32, i32) {
        let mut names = 0i32;
        let mut used = 0i32;
        for head in self.storage.bins() {
            if head.is_null() { continue; }
            used += 1;
            let mut entry = *head;
            while !entry.is_null() {
                names = names.wrapping_add(1);
                entry = unsafe { (*entry).hash_next.cast() };
            }
        }
        (names, used, crate::name::NAME_HASH_BINS as i32)
    }


    /// Removes a non-intrinsic entry and makes its index the next reusable slot.
    ///
    /// # Errors
    /// Returns an error for an invalid, intrinsic, or unhashed entry.
    pub fn remove(&mut self, index: i32) -> RuntimeResult<()> {
        if index < 0 || index >= self.storage.len() {
            return Err(RuntimeError::new("NameEntry"));
        }
        let entry = self.storage.entry(index);
        if entry.is_null() {
            return Err(RuntimeError::new("NameEntry"));
        }
        // SAFETY: this valid non-null slot points to a live entry.
        if unsafe { (*entry).flags & RF_NATIVE != 0 } {
            return Err(RuntimeError::new("!(NameEntry->Flags & RF_Native)"));
        }
        // SAFETY: this valid slot owns an entry whose UTF-32 name is terminated.
        let hash = name::strihash_text(unsafe { &(*entry).name }) as usize
            & (NAME_HASH_BINS - 1);
        let mut link = &mut self.storage.bins[hash] as *mut *mut FNameEntry;
        // SAFETY: this mutable owner exclusively controls the bucket links and entry.
        unsafe {
            while !(*link).is_null() && *link != entry {
                link = std::ptr::addr_of_mut!((*(*link)).hash_next).cast();
            }
            if (*link).is_null() {
                return Err(RuntimeError::new("NameEntry is not hashed"));
            }
            *link = (*entry).hash_next.cast();
        }
        // SAFETY: a non-intrinsic entry has exactly one Box allocation owned by its slot.
        drop(unsafe { Box::from_raw(entry) });
        self.storage.set_entry(index, std::ptr::null_mut());
        self.storage.available.push(index);
        Ok(())
    }

    /// Shuts down the initialized table and releases every owned entry.
    ///
    /// # Errors
    /// Returns an error if the table is not initialized.
    pub fn shutdown(&mut self) -> RuntimeResult<()> {
        if !self.storage.initialized {
            return Err(RuntimeError::new("Initialized"));
        }
        self.memory_cleanup();
        Ok(())
    }

    /// Releases every owned entry and resets the canonical table without failing.
    pub fn memory_cleanup(&mut self) {
        self.storage.bins.fill(std::ptr::null_mut());
        let intrinsic_count = name::hardcoded_count() as usize;
        for index in intrinsic_count..self.storage.names_len {
            let entry = self.storage.names[index];
            if !entry.is_null() {
                // SAFETY: every non-null slot beyond the intrinsic prefix owns one Box allocation.
                drop(unsafe { Box::from_raw(entry) });
                self.storage.names[index] = std::ptr::null_mut();
            }
        }
        self.storage.names.clear();
        self.storage.names_len = 0;
        self.storage.available.clear();
        self.intrinsic_slab = None;
        self.storage.initialized = false;
    }

    /// Returns a raw canonical-table pointer for internal `CoreAccess` use.
    ///
    /// The pointer is valid only while this owner remains alive and unmoved. Callers
    /// must not reset the table or mutate its slots outside this owner's APIs.
    pub(crate) fn storage_ptr(&mut self) -> *mut NameStorage {
        &mut self.storage
    }



}
impl Drop for NameOwner {
    fn drop(&mut self) {
        self.memory_cleanup();
    }
}
fn lookup(storage: &NameStorage, text: &[u32]) -> RuntimeResult<(usize, *mut FNameEntry)> {
    let hash = name::strihash_text(text) as usize & (NAME_HASH_BINS - 1);
    let head = storage.bins[hash];
    // SAFETY: NameOwner exclusively owns all non-null entries in this table.
    let found = unsafe {
        name::find_entry::<RuntimeError>(head, |entry| {
            // SAFETY: the chain's entry remains live for this synchronous comparison.
            Ok(equal_folded_text(text, unsafe { &(*entry).name }))
        })
    }?;
    Ok((hash, found.unwrap_or(std::ptr::null_mut())))
}

fn equal_folded_text(text: &[u32], entry: &[u32; NAME_SIZE]) -> bool {
    for (index, &unit) in text.iter().enumerate() {
        let left = unit as Tchar;
        let right = entry.get(index).copied().unwrap_or(0) as Tchar;
        if name::to_upper(left) != name::to_upper(right) || right == 0 {
            return false;
        }
    }
    entry.get(text.len()).copied().unwrap_or(0) == 0
}

fn compare_units_text(text: &[u32], entry: &[u32; NAME_SIZE]) -> i32 {
    let mut index = 0;
    loop {
        let left = text.get(index).copied().unwrap_or(0) as Tchar;
        let right = entry.get(index).copied().unwrap_or(0) as Tchar;
        if left != right {
            return if left < right { -1 } else { 1 };
        }
        if left == 0 {
            return 0;
        }
        index += 1;
    }
}

fn copy_name_text(entry: &mut [u32; NAME_SIZE], text: &[u32]) {
    entry[..text.len()].copy_from_slice(text);
    entry[text.len()] = 0;
}


