//! Native TMap policy over live packed hash slots and typed primitive callbacks.
//!
//! No Rust owner duplicates native pairs or buckets. Calls are serialized on the
//! engine thread. Callbacks capture exceptions, and the first error stops all
//! callbacks (including cleanup), preserving native partial publication. In
//! particular a failed rehash after allocation leaves its unpublished allocation
//! and any already rewritten pair links, as did the native implementation.

use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use std::{ffi::c_void, ptr};

const _: () = {
    use crate::core_records::MapStorageRaw;
    use hp2_abi::engine::generated::TMapIntInt;
    assert!(std::mem::size_of::<MapStorageRaw>() == std::mem::size_of::<TMapIntInt>());
    assert!(std::mem::offset_of!(MapStorageRaw, pairs) == std::mem::offset_of!(TMapIntInt, pairs));
    assert!(std::mem::offset_of!(MapStorageRaw, hash) == std::mem::offset_of!(TMapIntInt, hash));
    assert!(std::mem::offset_of!(MapStorageRaw, hash_count) == std::mem::offset_of!(TMapIntInt, hash_count));
};

/// Constructs an owned native TMap header with eight empty hash buckets.
///
/// # Safety
/// `storage` is writable compiler-probed TMap storage with no live allocations.
/// Its pair keys and values must be POD; this API does not run element destructors.
///
/// # Errors
/// Returns a hash-bucket allocation failure, leaving safely destructible storage.
pub unsafe fn initialize_owned_pod(storage: *mut c_void) -> crate::core_access::RuntimeResult<()> {
    let map = storage.cast::<crate::core_records::MapStorageRaw>();
    unsafe {
        map.write_unaligned(std::mem::zeroed());
        ptr::addr_of_mut!((*map).hash_count).write_unaligned(8);
        crate::core_array::initialize_map_storage(map)
    }
}

/// Releases an owned POD map's paired array and bucket allocations.
///
/// # Safety
/// `storage` is a live native TMap header constructed with Core's paired
/// allocator. Every pair key/value is POD, and no caller retains its storage.
pub unsafe fn destroy_owned_pod(storage: *mut c_void) {
    unsafe { crate::core_lifetime::release_map_storage(storage.cast()) }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2MapSlots {
    pub hash: *mut *mut i32,
    pub count: *mut i32,
}

/// Primitive operations shared by all key/value instantiations.
pub mod leaf {
    pub const NUM: i32 = 0;
    pub const NEXT: i32 = 1;
    pub const LINK: i32 = 2;
    pub const HASH: i32 = 3;
    pub const REMOVE: i32 = 4;
    pub const EMPTY: i32 = 5;
}
/// Request-specific typed operations; unused primitives need not be instantiated.
pub mod query {
    pub const HASH: i32 = 0;
    pub const KEY_EQ: i32 = 1;
    pub const VALUE_EQ: i32 = 2;
    pub const CONSTRUCT: i32 = 3;
    pub const ASSIGN: i32 = 4;
    pub const EMIT: i32 = 5;
    pub const SUMMARY: i32 = 6;
    pub const BUCKET: i32 = 7;
}
/// Entry operations, also declared in Hp2MapStorage.h.
pub mod operation {
    pub const REHASH: i32 = 0;
    pub const RELAX: i32 = 1;
    pub const ADD: i32 = 2;
    pub const SET: i32 = 3;
    pub const FIND: i32 = 4;
    pub const REMOVE: i32 = 5;
    pub const MULTI_FIND: i32 = 6;
    pub const UNIQUE: i32 = 7;
    pub const REMOVE_PAIR: i32 = 8;
    pub const FIND_PAIR: i32 = 9;
    pub const DUMP: i32 = 10;
    pub const EMPTY: i32 = 11;
    pub const RELEASE: i32 = 12;
}

pub type MapPrimitive = unsafe extern "C" fn(
    context: *mut c_void,
    operation: i32,
    index: i32,
    argument: i32,
    out: *mut i32,
) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2MapServices {
    pub context: *mut c_void,
    pub leaf: MapPrimitive,
    pub query: MapPrimitive,
    pub allocator: Hp2ArrayServices,
    pub tag: *const c_void,
}

fn status(code: i32) -> Result<(), i32> {
    if code == 0 { Ok(()) } else { Err(code) }
}

unsafe fn call(s: Hp2MapServices, query: bool, op: i32, i: i32, arg: i32) -> Result<i32, i32> {
    let mut out = 0;
    unsafe {
        status((if query { s.query } else { s.leaf })(
            s.context, op, i, arg, &mut out,
        ))?;
    }
    Ok(out)
}

unsafe fn num(s: Hp2MapServices) -> Result<i32, i32> {
    unsafe { call(s, false, leaf::NUM, 0, 0) }
}

/// # Safety
/// Slots must remain live, with a power-of-two count >= 8 and valid chain indices.
/// Every callback must capture exceptions. Reentry may change pair storage; no
/// pointer/reference to a pair is retained across callbacks. Reentry must leave
/// currently visited indices valid, and must not destroy this map or invalidate
/// input operands. During rehash it must not grow the live hash count beyond the
/// allocated table. As with native TMap, structural mutation during chain walks
/// must preserve a terminating, valid chain. Concurrent access is forbidden.
/// Shared TMap rehash policy over owned slots and typed key/hash leaves.
///
/// # Errors
/// Returns the first allocator, key-hash, link, count, or free error.
pub(crate) unsafe fn rehash_with<E>(
    old_hash: *mut i32,
    hash_count: i32,
    mut allocate: impl FnMut(i32) -> Result<*mut i32, E>,
    mut free: impl FnMut(*mut i32) -> Result<(), E>,
    mut num: impl FnMut() -> Result<i32, E>,
    mut key_hash: impl FnMut(i32) -> Result<i32, E>,
    mut set_next: impl FnMut(i32, i32) -> Result<(), E>,
) -> Result<*mut i32, E> {
    let fresh = allocate(hash_count)?;
    for bucket in 0..hash_count as usize {
        unsafe { fresh.add(bucket).write_unaligned(-1); }
    }
    let count = num()?;
    let mut index = 0;
    while index < count {
        let hash = key_hash(index)?;
        let bucket = (hash & (hash_count - 1)) as usize;
        let next = unsafe { fresh.add(bucket).read_unaligned() };
        set_next(index, next)?;
        unsafe { fresh.add(bucket).write_unaligned(index); }
        index += 1;
    }
    if !old_hash.is_null() { free(old_hash)?; }
    Ok(fresh)
}

/// # Safety
/// Slots must remain live, with a power-of-two count >= 8 and valid chain indices.
/// Every callback must capture exceptions. Reentry may change pair storage; no
/// pointer/reference to a pair is retained across callbacks. Reentry must leave
/// currently visited indices valid, and must not destroy this map or invalidate
/// input operands. During rehash it must not grow the live hash count beyond the
/// allocated table. As with native TMap, structural mutation during chain walks
/// must preserve a terminating, valid chain. Concurrent access is forbidden.
pub unsafe fn rehash(slots: Hp2MapSlots, s: Hp2MapServices) -> Result<(), i32> {
    unsafe {
        let mut data: *mut c_void = ptr::null_mut();
        let mut n = 0;
        let mut max = 0;
        let count = slots.count.read_unaligned();
        let old = slots.hash.read_unaligned();
        let fresh = rehash_with(
            old, count,
            |count| {
                array_storage::set_exact(
                    Hp2ArraySlots { data: &mut data, num: &mut n, max: &mut max },
                    count, 4, s.tag, s.allocator,
                )?;
                Ok(data.cast::<i32>())
            },
            |hash| status((s.allocator.free)(s.allocator.context, hash.cast())),
            || num(s),
            |index| call(s, false, leaf::HASH, index, 0),
            |index, next| call(s, false, leaf::LINK, index, next).map(|_| ()),
        )?;
        slots.hash.write_unaligned(fresh);
        Ok(())
    }
}

unsafe fn relax(slots: Hp2MapSlots, s: Hp2MapServices) -> Result<(), i32> {
    unsafe {
        while slots.count.read_unaligned() > num(s)?.wrapping_mul(2).wrapping_add(8) {
            slots
                .count
                .write_unaligned(slots.count.read_unaligned() / 2);
        }
        rehash(slots, s)
    }
}

unsafe fn first(slots: Hp2MapSlots, s: Hp2MapServices) -> Result<i32, i32> {
    unsafe {
        let hash = call(s, true, query::HASH, 0, 0)?;
        let bucket = (hash & (slots.count.read_unaligned() - 1)) as usize;
        Ok(slots.hash.read_unaligned().add(bucket).read_unaligned())
    }
}

unsafe fn matches(s: Hp2MapServices, i: i32, pair: bool) -> Result<bool, i32> {
    unsafe {
        Ok(call(s, true, query::KEY_EQ, i, 0)? != 0
            && (!pair || call(s, true, query::VALUE_EQ, i, 0)? != 0))
    }
}

/// Finds a key through the live intrusive hash chain without allocating or copying pairs.
pub fn find_index_with<E>(
    mut index: i32,
    mut matches: impl FnMut(i32) -> Result<bool, E>,
    mut next: impl FnMut(i32) -> Result<i32, E>,
) -> Result<i32, E> {
    while index != -1 {
        if matches(index)? { return Ok(index); }
        index = next(index)?;
    }
    Ok(-1)
}

unsafe fn find(slots: Hp2MapSlots, s: Hp2MapServices, pair: bool) -> Result<i32, i32> {
    unsafe {
        find_index_with(first(slots, s)?,
            |index| matches(s, index, pair),
            |index| call(s, false, leaf::NEXT, index, 0))
    }
}

unsafe fn add(slots: Hp2MapSlots, s: Hp2MapServices) -> Result<i32, i32> {
    unsafe {
        let index = call(s, true, query::CONSTRUCT, 0, 0)?;
        let hash = call(s, false, leaf::HASH, index, 0)?;
        let bucket = (hash & (slots.count.read_unaligned() - 1)) as usize;
        let next = slots.hash.read_unaligned().add(bucket).read_unaligned();
        call(s, false, leaf::LINK, index, next)?;
        // Read the live count after the constructor/hash callbacks, just as native.
        let last = num(s)? - 1;
        slots
            .hash
            .read_unaligned()
            .add(bucket)
            .write_unaligned(last);
        if slots.count.read_unaligned().wrapping_mul(2).wrapping_add(8) < num(s)? {
            slots
                .count
                .write_unaligned(slots.count.read_unaligned().wrapping_mul(2));
            rehash(slots, s)?;
        }
        Ok(index)
    }
}

/// Execute one complete native map operation, returning a pair index or count.
/// Find returns -1 on absence; wrappers retain typed pointer/value return ABIs.
///
/// # Safety
/// See [`rehash`]. Request callbacks must implement only the primitives reached by
/// the selected operation. Output callbacks may reenter under the same validity
/// contract; subsequent chain links and slot values are always read live.
pub unsafe fn run(slots: Hp2MapSlots, s: Hp2MapServices, op: i32) -> Result<i32, i32> {
    unsafe {
        match op {
            operation::REHASH => {
                rehash(slots, s)?;
                Ok(0)
            }
            operation::RELAX => {
                relax(slots, s)?;
                Ok(0)
            }
            operation::ADD => add(slots, s),
            operation::SET | operation::UNIQUE => {
                let i = find(slots, s, op == operation::UNIQUE)?;
                if i == -1 {
                    add(slots, s)
                } else {
                    if op == operation::SET {
                        call(s, true, query::ASSIGN, i, 0)?;
                    }
                    Ok(i)
                }
            }
            operation::FIND | operation::FIND_PAIR => find(slots, s, op == operation::FIND_PAIR),
            operation::REMOVE | operation::REMOVE_PAIR => {
                let mut count = 0;
                let mut i = num(s)? - 1;
                while i >= 0 {
                    if matches(s, i, op == operation::REMOVE_PAIR)? {
                        call(s, false, leaf::REMOVE, i, 0)?;
                        count += 1;
                    }
                    i -= 1;
                }
                if count != 0 {
                    relax(slots, s)?;
                }
                Ok(count)
            }
            operation::MULTI_FIND => {
                let mut i = first(slots, s)?;
                while i != -1 {
                    if matches(s, i, false)? {
                        call(s, true, query::EMIT, i, 0)?;
                    }
                    i = call(s, false, leaf::NEXT, i, 0)?;
                }
                Ok(0)
            }
            operation::DUMP => {
                let n = num(s)?;
                call(s, true, query::SUMMARY, n, slots.count.read_unaligned())?;
                let mut bucket = 0;
                while bucket < slots.count.read_unaligned() {
                    let mut count = 0;
                    let mut i = slots
                        .hash
                        .read_unaligned()
                        .add(bucket as usize)
                        .read_unaligned();
                    while i != -1 {
                        count += 1;
                        i = call(s, false, leaf::NEXT, i, 0)?;
                    }
                    call(s, true, query::BUCKET, bucket, count)?;
                    bucket += 1;
                }
                Ok(0)
            }
            operation::EMPTY => {
                call(s, false, leaf::EMPTY, 0, 0)?;
                slots.count.write_unaligned(8);
                rehash(slots, s)?;
                Ok(0)
            }
            operation::RELEASE => {
                let hash = slots.hash.read_unaligned();
                if !hash.is_null() {
                    status((s.allocator.free)(s.allocator.context, hash.cast()))?;
                }
                slots.hash.write_unaligned(ptr::null_mut());
                slots.count.write_unaligned(0);
                Ok(0)
            }
            _ => Err(1),
        }
    }
}
