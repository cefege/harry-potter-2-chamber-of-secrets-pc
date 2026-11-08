//! Cache algorithms from `Core/Src/UnCache.cpp` and `Core/Inc/UnCache.h`.
//! C++ SDK shells forward through `hp2_rs_core_cache_*`
//! (`crates/hp2-ffi/src/core_cache.rs`); direct native field accessors remain ABI leaves.
//!
//! The cache is a doubly linked list of items in address order, plus a 16384-entry
//! hash of the used items. `FMemCache` and `FCacheItem` are the generated layouts
//! from `hp2_abi`. The item array and cache memory come from the C++ allocator through
//! [`CacheServices`].
//!
//! Failure convention: a failing C++ call (`check`, `appErrorf`, `appMalloc`, the
//! `Logf` or `ParseCommand` thunks) returns the frame code of the `guard` stack that
//! was open at that point (the `FRAME_*` constants). The C++ shell reopens those
//! frames and rethrows the recorded exception.
//!
//! Divergences from the C++ (see the per-function notes): `CreateNewFreeSpace`'s
//! `End` argument and the `MergeWithNext` checks are `checkSlow`, compiled out in the
//! shipping configuration, so they are not ported. `Tick` has no failure sites. The
//! dead `#if DO_GUARD_SLOW && 0` branch of `Tick` is removed. `ConditionalCheckState`
//! is a no-op (`DO_GUARD_SLOW=0`). Signed overflow in `CreateSize + SafetyPad` and
//! `CreateSize + COST_INFINITE` wraps here, where the C++ is undefined.

use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::ptr::{addr_of_mut, null_mut};

use hp2_abi::engine::generated::{FCacheItem, FMemCache};

use crate::ansi::Tchar;
use crate::mem::{
    FRAME_OWN, Hp2MemServices, MemServices, alloc_checked, check, checked_int_size, raise, wide_z,
};

/// `FMemCache::FlushItem` guard, nested under `Flush`.
pub const FRAME_FLUSH_ITEM: i32 = 2;
/// `FMemCache::CreateNewFreeSpace` guard, nested under `Init` or `Create`.
pub const FRAME_CREATE_NEW_FREE_SPACE: i32 = 3;
/// `FMemCache::Exec` guard, nested under `Create`.
pub const FRAME_EXEC: i32 = 4;
/// `FMemCache::CheckState` own guard.
pub const FRAME_CHECK_STATE: i32 = 5;
/// `CheckState`'s `guard(1)` (linear walk: `GetSize`, pointer and link checks).
pub const FRAME_CHECK_STATE_WALK: i32 = 6;
/// `CheckState`'s `guard(3)` (hash walk).
pub const FRAME_CHECK_STATE_HASH: i32 = 7;

const COST_INFINITE: i32 = 0x0100_0000;
const HASH_COUNT: usize = 16384;
const IGNORE_SIZE: isize = 256;
const ITEM_BYTES: usize = size_of::<FCacheItem>();

/// Layout of the `Hp2CacheServices` table (`Core/Src/Hp2CacheServices.cpp`). Field
/// order matches the C++ initializer. Each entry is a leaf call into the original
/// C++ function; entries that can raise return status 1 after recording the exception.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2CacheServices {
    /// `appMalloc(Size, Tag)`: writes the block to `*out`.
    pub malloc: unsafe extern "C" fn(size: u32, tag: *const c_void, out: *mut *mut c_void) -> i32,
    /// `appFree(Ptr)`.
    pub free: unsafe extern "C" fn(ptr: *mut c_void),
    /// `appErrorf(TEXT("%s"), Msg)`.
    pub errorf: unsafe extern "C" fn(msg: *const c_void) -> i32,
    /// `appFailAssert(Expr, File, Line)`.
    pub fail_assert:
        unsafe extern "C" fn(expr: *const c_char, file: *const c_char, line: i32) -> i32,
    /// `Ar->Logf(TEXT("%s"), Text)`.
    pub logf: unsafe extern "C" fn(ar: *mut c_void, text: *const c_void) -> i32,
    /// `GLog`.
    pub glog: unsafe extern "C" fn() -> *mut c_void,
}

/// `FMemCache::Status`'s `appSprintf` format.
const STATUS_FORMAT: [Tchar; 85] = crate::ansi::wide(
    b"Gets=%04i (%04.1f) Crts=%03i (%04.1f) Fresh=%03iK Stale=%03iK Items=%03i Tick=%04.1f\0",
);

/// `appSprintf`'s implicit destination capacity (`GET_VARARGS_RESULT(Dest,1024,...)`).
const APP_SPRINTF_CAPACITY: usize = 1024;

impl Hp2CacheServices {
    /// The allocation and check subset, with the same layout as [`Hp2MemServices`].
    fn mem(&self) -> Hp2MemServices {
        Hp2MemServices {
            malloc: self.malloc,
            free: self.free,
            errorf: self.errorf,
            fail_assert: self.fail_assert,
        }
    }
}

impl MemServices for Hp2CacheServices {
    unsafe fn alloc(&self, size: u32, tag: *const c_void) -> Result<*mut c_void, i32> {
        // SAFETY: forwarded; `tag` meets the same contract as `MemServices::alloc`.
        unsafe { self.mem().alloc(size, tag) }
    }

    unsafe fn release(&self, ptr: *mut c_void) {
        // SAFETY: forwarded; `ptr` meets the same contract as `MemServices::release`.
        unsafe { self.mem().release(ptr) }
    }

    fn raise_error(&self, msg: &[Tchar]) -> i32 {
        self.mem().raise_error(msg)
    }

    fn fail_assert(&self, expr: &CStr, file: &CStr, line: i32) -> i32 {
        self.mem().fail_assert(expr, file, line)
    }
}

/// `FMemCache::Status` values, in the order `appSprintf` consumes them.
pub struct StatusFigures {
    pub num_gets: i32,
    pub get_ms: f64,
    pub num_creates: i32,
    pub create_ms: f64,
    pub fresh_k: i32,
    pub stale_k: i32,
    pub items: i32,
    pub tick_ms: f64,
}

/// The services the cache port calls: the memory services plus the logging, command
/// parsing, timing, and status formatting leaves.
pub trait CacheServices: MemServices {
    /// Emits a formatted cache line. `Err` carries the thunk's status.
    ///
    /// # Safety
    /// `ar` must be a live handle understood by this service for this synchronous call.
    unsafe fn logf(&self, ar: *mut c_void, text: &[Tchar]) -> Result<(), i32>;
    /// `GLog`.
    fn glog(&self) -> *mut c_void;
    /// `ParseCommand(Stream, Match)`.
    ///
    /// # Safety
    /// `stream` must be a valid out-slot holding a NUL-terminated `TCHAR` string.
    unsafe fn parse_command(&self, stream: *mut *const c_void, matched: &[Tchar]) -> bool;
    /// `appCycles()`.
    fn app_cycles(&self) -> u32;
    /// `GSecondsPerCycle`.
    fn seconds_per_cycle(&self) -> f32;
    /// Formats the `FMemCache::Status` line into `out`.
    ///
    /// # Safety
    /// `out` must be the caller's `TCHAR` buffer, sized for the status line.
    unsafe fn status_format(&self, out: *mut c_void, figures: &StatusFigures);
}

impl CacheServices for Hp2CacheServices {
    unsafe fn logf(&self, ar: *mut c_void, text: &[Tchar]) -> Result<(), i32> {
        // SAFETY: forwarded from the trait contract; `text` is NUL-terminated.
        match unsafe { (self.logf)(ar, text.as_ptr().cast()) } {
            0 => Ok(()),
            status => Err(status),
        }
    }

    fn glog(&self) -> *mut c_void {
        // SAFETY: `GLog` is read-only; the thunk has no preconditions.
        unsafe { (self.glog)() }
    }

    unsafe fn parse_command(&self, stream: *mut *const c_void, matched: &[Tchar]) -> bool {
        // SAFETY: forwarded from the trait contract; `matched` is NUL-terminated.
        unsafe { crate::command::parse_command(stream.cast(), matched.as_ptr()) != 0 }
    }

    fn app_cycles(&self) -> u32 {
        crate::unix_clock::cycles()
    }

    /// `GSecondsPerCycle`, which `appPlatformPreInit` sets from this same
    /// constant before the cache can exist.
    fn seconds_per_cycle(&self) -> f32 {
        crate::unix_clock::seconds_per_cycle()
    }

    /// `appSprintf(Out, STATUS_FORMAT, ...)`. With only `%i`/`%f` conversions,
    /// `appVswprintfCompat` formats each one through `vswprintf` and copies the
    /// literal text, so a single host `swprintf` writes the same characters;
    /// the line stays far below the 1024-character capacity.
    unsafe fn status_format(&self, out: *mut c_void, f: &StatusFigures) {
        // SAFETY: `out` holds `APP_SPRINTF_CAPACITY` TCHARs by the trait contract.
        unsafe { format_status(out, f) }
    }
}

/// Formats the cache status line into a caller-owned TCHAR buffer.
///
/// # Safety
/// `out` must point to writable storage for `APP_SPRINTF_CAPACITY` TCHAR values.
pub(crate) unsafe fn format_status(out: *mut c_void, f: &StatusFigures) {
    // SAFETY: forwarded from this function's buffer and argument contract.
    unsafe {
        crate::ansi::libc::swprintf(
            out.cast(),
            APP_SPRINTF_CAPACITY,
            STATUS_FORMAT.as_ptr(),
            f.num_gets,
            f.get_ms,
            f.num_creates,
            f.create_ms,
            f.fresh_k,
            f.stale_k,
            f.items,
            f.tick_ms,
        );
    }
}

/// `FMemCache::GHash`.
pub fn ghash(id: u64) -> usize {
    let d = id as u32;
    ((d ^ (d >> 12) ^ (d >> 24)) & (HASH_COUNT as u32 - 1)) as usize
}

/// `Align(Ptr, Alignment)` (`UnTemplate.h`).
fn align(ptr: *mut u8, alignment: i32) -> *mut u8 {
    let mask = alignment.wrapping_sub(1) as isize as usize;
    let aligned = (ptr as usize).wrapping_add(mask) & !mask;
    ptr.wrapping_add(aligned.wrapping_sub(ptr as usize))
}

/// `FCacheItem::GetSize`: `appCheckedIntSize(LinearNext->Data - Data)`.
fn get_size<S: MemServices>(svc: &S, item: *mut FCacheItem, frame: i32) -> Result<i32, i32> {
    // SAFETY: `item` and its `linear_next` are live items of the cache.
    let size = unsafe {
        let next = (*item).linear_next;
        ((*next).data as isize).wrapping_sub((*item).data as isize)
    };
    checked_int_size(svc, size as usize, frame)
}

/// Native `FMemCache::Get`, including its independent counter and clock switches.
///
/// The MRU return deliberately does not unclock `GetCycles`, matching the SDK.
/// A miss leaves the caller's item slot unchanged.
///
/// # Safety
/// `cache` is an initialized SDK cache with live item chains; `item` is writable.
/// The legacy MRU ID must name a live item. Clock services are nonthrowing leaves.
pub unsafe fn get<S: CacheServices>(
    cache: *mut FMemCache,
    id: u64,
    item: *mut *mut FCacheItem,
    alignment: i32,
    count_gets: bool,
    clock_gets: bool,
    svc: &S,
) -> *mut u8 {
    if clock_gets {
        let cycles = svc.app_cycles() as i32;
        unsafe { (*cache).get_cycles = (*cache).get_cycles.wrapping_sub(cycles) };
    }
    unsafe {
        if count_gets {
            (*cache).num_gets = (*cache).num_gets.wrapping_add(1);
        }
        if id == (*cache).mru_id {
            item.write((*cache).mru_item);
            let mru = (*cache).mru_item;
            (*mru).cost = (*mru).cost.wrapping_add(COST_INFINITE);
            return align((*mru).data, alignment);
        }
        let mut found = (*cache).hash_items[ghash(id)];
        while !found.is_null() {
            if (*found).id == id {
                (*cache).mru_id = id;
                (*cache).mru_item = found;
                item.write(found);
                (*found).time = (*cache).time as u16;
                (*found).cost = (*found).cost.wrapping_add(COST_INFINITE);
                if clock_gets {
                    let cycles = svc.app_cycles() as i32;
                    (*cache).get_cycles = (*cache).get_cycles.wrapping_add(cycles);
                }
                return align((*found).data, alignment);
            }
            found = (*found).hash_next;
        }
        if clock_gets {
            let cycles = svc.app_cycles() as i32;
            (*cache).get_cycles = (*cache).get_cycles.wrapping_add(cycles);
        }
    }
    null_mut()
}

/// `item_unlock` error statuses; [`raise_unlock_error`] reports them.
pub const UNLOCK_NULL: i32 = 2;
pub const UNLOCK_NOT_LOCKED: i32 = 3;

/// `FCacheItem::Unlock`'s `appErrorf` for an [`item_unlock`] error status,
/// formatted on the stack. Returns the leaf status: non-zero when it raised,
/// `0` for any other `code`.
///
/// # Safety
/// For [`UNLOCK_NOT_LOCKED`], `item` is the live item `item_unlock` rejected.
pub unsafe fn raise_unlock_error<S: MemServices>(
    item: *const FCacheItem,
    code: i32,
    svc: &S,
) -> i32 {
    fn put(text: &mut [Tchar], at: usize, part: &str) -> usize {
        for (slot, byte) in text[at..].iter_mut().zip(part.bytes()) {
            *slot = Tchar::from(byte);
        }
        at + part.len()
    }
    // `%08X`.
    fn put_hex(text: &mut [Tchar], at: usize, value: u32) -> usize {
        for (digit, slot) in text[at..at + 8].iter_mut().enumerate() {
            let nibble = (value >> (28 - 4 * digit)) & 0xF;
            *slot = Tchar::from(b"0123456789ABCDEF"[nibble as usize]);
        }
        at + 8
    }
    // Longest message: 44 characters plus the terminator.
    let mut text: [Tchar; 48] = [0; 48];
    match code {
        UNLOCK_NULL => {
            put(&mut text, 0, "Unlock: Null cache item");
        }
        UNLOCK_NOT_LOCKED => {
            let id = unsafe { (*item).id };
            let at = put(&mut text, 0, "Unlock: Item ");
            let at = put_hex(&mut text, at, (id >> 32) as u32);
            let at = put(&mut text, at, ".");
            let at = put_hex(&mut text, at, id as u32);
            put(&mut text, at, " is not locked");
        }
        _ => return 0,
    }
    svc.raise_error(&text)
}

/// Native lock-cost mutation, with the original optional slow-guard decisions.
///
/// # Safety
/// `item` is live, or null only when `guard_slow` is enabled. Lock arithmetic
/// must remain within the legacy maximum of 126 simultaneous locks.
pub unsafe fn item_unlock(item: *mut FCacheItem, guard_slow: bool) -> i32 {
    if guard_slow && item.is_null() {
        return UNLOCK_NULL;
    }
    unsafe {
        if guard_slow && (*item).cost < COST_INFINITE {
            return UNLOCK_NOT_LOCKED;
        }
        (*item).cost = (*item).cost.wrapping_sub(COST_INFINITE);
    }
    0
}

/// Native checked adjacent-item size, without retaining cache references over a leaf.
///
/// # Safety
/// `item` and its successor are live cache items whose data lie in the same arena.
pub unsafe fn item_size<S: MemServices>(
    item: *mut FCacheItem,
    guard_slow: bool,
    file: &CStr,
    svc: &S,
) -> Result<i32, i32> {
    if guard_slow {
        let ordered = unsafe { (*(*item).linear_next).data as usize >= (*item).data as usize };
        check(svc, ordered, c"LinearNext->Data>=Data", file, 45, FRAME_OWN)?;
    }
    get_size(svc, item, FRAME_OWN)
}

/// `FMemCache::Unhash`. No guard of its own; the failure belongs to the caller's
/// frame, which is `FRAME_OWN` for every caller.
fn unhash<S: MemServices>(cache: &mut FMemCache, id: u64, svc: &S) -> Result<(), i32> {
    let mut prev_link: *mut *mut FCacheItem = addr_of_mut!(cache.hash_items[ghash(id)]);
    // SAFETY: `prev_link` walks the hash chain; every link is a field of a live item
    // or of `hash_items`.
    unsafe {
        while !(*prev_link).is_null() {
            if (**prev_link).id == id {
                *prev_link = (**prev_link).hash_next;
                return Ok(());
            }
            prev_link = addr_of_mut!((**prev_link).hash_next);
        }
    }
    raise(svc, "Unhashed item", FRAME_OWN)
}

/// `FMemCache::MergeWithNext`. Its checks are `checkSlow` and compile out.
fn merge_with_next(cache: &mut FMemCache, first: *mut FCacheItem) -> *mut FCacheItem {
    // SAFETY: `first` and its successor are live items.
    unsafe {
        let second = (*first).linear_next;
        (*first).linear_next = (*second).linear_next;
        (*(*first).linear_next).linear_prev = first;
        (*second).linear_next = cache.unused_items;
        cache.unused_items = second;
    }
    first
}

/// `FMemCache::FlushItem`. Returns the item after the flushed one (or after the merged
/// item). Frame: `FRAME_FLUSH_ITEM`.
fn flush_item<S: MemServices>(
    cache: &mut FMemCache,
    item: *mut FCacheItem,
    ignore_locked: i32,
    svc: &S,
) -> Result<*mut FCacheItem, i32> {
    let mut item = item;
    // SAFETY: `item` is a live item of the cache.
    unsafe {
        if (*item).cost < COST_INFINITE {
            (*item).id = 0;
            (*item).cost = 0;

            let prev = (*item).linear_prev;
            if !prev.is_null() && (*prev).id == 0 && (*item).segment == (*prev).segment {
                item = merge_with_next(cache, prev);
            }

            let next = (*item).linear_next;
            if !next.is_null() && (*next).id == 0 && (*item).segment == (*next).segment {
                item = merge_with_next(cache, item);
            }
        } else if ignore_locked == 0 {
            let id = (*item).id;
            raise(
                svc,
                &format!(
                    "Flushed locked cache object {:08X}.{:08X}",
                    (id >> 32) as u32,
                    id as u32
                ),
                FRAME_FLUSH_ITEM,
            )?;
        }
        Ok((*item).linear_next)
    }
}

/// `FMemCache::CreateNewFreeSpace`. The `End` argument is only used by `checkSlow`
/// and is not ported. Frame: `FRAME_CREATE_NEW_FREE_SPACE`.
fn create_new_free_space<S: MemServices>(
    cache: &mut FMemCache,
    start: *mut u8,
    prev: *mut FCacheItem,
    next: *mut FCacheItem,
    segment: i32,
    file: &CStr,
    svc: &S,
) -> Result<(), i32> {
    // SAFETY: `prev`, `next`, and `unused_items` are live items or null where checked.
    unsafe {
        if !prev.is_null() && (*prev).id == 0 && (*prev).segment as i32 == segment {
            // The previous item is free space: merge with it.
        } else if !next.is_null() && (*next).id == 0 && (*next).segment as i32 == segment {
            // The next item is free space: merge with it.
            (*next).data = start;
        } else {
            check(
                svc,
                !cache.unused_items.is_null(),
                c"UnusedItems != NULL",
                file,
                325,
                FRAME_CREATE_NEW_FREE_SPACE,
            )?;

            let item = cache.unused_items;
            cache.unused_items = (*item).linear_next;

            (*item).data = start;
            (*item).segment = segment as u8;
            (*item).time = 0;
            (*item).id = 0;
            (*item).cost = 0;
            (*item).linear_next = next;
            (*item).linear_prev = prev;
            (*item).hash_next = null_mut();

            if !prev.is_null() {
                (*prev).linear_next = item;
            } else {
                cache.cache_items = item;
            }
            if !next.is_null() {
                (*next).linear_prev = item;
            }
        }
    }
    Ok(())
}

/// `FMemCache::CheckState`. Frames: `FRAME_CHECK_STATE` for the top-level checks,
/// `FRAME_CHECK_STATE_WALK` for `guard(1)`, `FRAME_CHECK_STATE_HASH` for `guard(3)`.
///
/// # Safety
/// `cache`'s item list, hash chains, and cache memory must be the ones built by
/// [`init`]; a corrupt link is a raw dereference, as in the C++.
pub unsafe fn check_state<S: CacheServices>(
    cache: &FMemCache,
    file: *const c_char,
    svc: &S,
) -> Result<(), i32> {
    // SAFETY: `file` is the caller's NUL-terminated `__FILE__`.
    let file = unsafe { CStr::from_ptr(file) };
    check(
        svc,
        cache.initialized == 1,
        c"Initialized == 1",
        file,
        205,
        FRAME_CHECK_STATE,
    )?;
    check(
        svc,
        !cache.cache_items.is_null(),
        c"CacheItems != NULL",
        file,
        208,
        FRAME_CHECK_STATE,
    )?;

    let mut item_count: i32 = 0;
    let mut used_item_count: i32 = 0;
    let mut was_free: i32 = 0;
    let mut hash_count: i32 = 0;
    let mut prev_segment: i32 = -1;
    let mut expected_pointer: *mut u8 = cache.cache_memory;

    // guard(1): walk the linear item list.
    let mut item = cache.cache_items;
    while item != cache.last_item {
        // SAFETY: `item` is a live item; a null link is a raw dereference, as in the C++.
        unsafe {
            check(
                svc,
                (*item).data == expected_pointer,
                c"Item->Data == ExpectedPointer",
                file,
                219,
                FRAME_CHECK_STATE_WALK,
            )?;
            check(
                svc,
                !(*item).linear_next.is_null(),
                c"Item->LinearNext",
                file,
                220,
                FRAME_CHECK_STATE_WALK,
            )?;
            check(
                svc,
                (*(*item).linear_next).linear_prev == item,
                c"Item->LinearNext->LinearPrev == Item",
                file,
                221,
                FRAME_CHECK_STATE_WALK,
            )?;

            let size = get_size(svc, item, FRAME_CHECK_STATE_WALK)?;
            expected_pointer = expected_pointer.wrapping_offset(size as isize);

            item_count = item_count.wrapping_add(1);

            if (*item).id == 0 && (*item).segment as i32 == prev_segment {
                check(
                    svc,
                    was_free == 0,
                    c"!WasFree",
                    file,
                    233,
                    FRAME_CHECK_STATE_WALK,
                )?;
            }
            was_free = ((*item).id == 0) as i32;
            prev_segment = (*item).segment as i32;

            if item != cache.cache_items {
                check(
                    svc,
                    !(*item).linear_prev.is_null(),
                    c"Item->LinearPrev",
                    file,
                    241,
                    FRAME_CHECK_STATE_WALK,
                )?;
                check(
                    svc,
                    (*(*item).linear_prev).linear_next == item,
                    c"Item->LinearPrev->LinearNext == Item",
                    file,
                    242,
                    FRAME_CHECK_STATE_WALK,
                )?;
            }

            if (*item).id != 0 {
                used_item_count = used_item_count.wrapping_add(1);
                let mut hashed_count: i32 = 0;
                let mut hash_item = cache.hash_items[ghash((*item).id)];
                while !hash_item.is_null() {
                    hashed_count += (hash_item == item) as i32;
                    hash_item = (*hash_item).hash_next;
                }
                check(
                    svc,
                    hashed_count != 0,
                    c"HashedCount!=0",
                    file,
                    252,
                    FRAME_CHECK_STATE_WALK,
                )?;
                check(
                    svc,
                    hashed_count == 1,
                    c"HashedCount==1",
                    file,
                    253,
                    FRAME_CHECK_STATE_WALK,
                )?;
            }

            item = (*item).linear_next;
        }
    }
    check(
        svc,
        expected_pointer == cache.cache_memory.wrapping_offset(cache.mem_total as isize),
        c"ExpectedPointer == CacheMemory + MemTotal",
        file,
        256,
        FRAME_CHECK_STATE_WALK,
    )?;

    // guard(2): walk the unused list.
    let mut item = cache.unused_items;
    while !item.is_null() {
        item_count = item_count.wrapping_add(1);
        // SAFETY: `item` is a live unused item.
        item = unsafe { (*item).linear_next };
    }

    check(
        svc,
        item_count.wrapping_add(1) == cache.items_total,
        c"ItemCount+1==ItemsTotal",
        file,
        266,
        FRAME_CHECK_STATE,
    )?;

    // guard(3): every hashed item belongs in its bucket and is not duplicated.
    for (i, &head) in cache.hash_items.iter().enumerate() {
        let mut item = head;
        while !item.is_null() {
            hash_count = hash_count.wrapping_add(1);
            // SAFETY: `item` is a live hashed item.
            unsafe {
                check(
                    svc,
                    ghash((*item).id) == i,
                    c"GHash(Item->Id) == i",
                    file,
                    278,
                    FRAME_CHECK_STATE_HASH,
                )?;
                let mut other = (*item).hash_next;
                while !other.is_null() {
                    check(
                        svc,
                        (*other).id != (*item).id,
                        c"Other->Id != Item->Id",
                        file,
                        282,
                        FRAME_CHECK_STATE_HASH,
                    )?;
                    other = (*other).hash_next;
                }
                item = (*item).hash_next;
            }
        }
    }
    check(
        svc,
        hash_count == used_item_count,
        c"HashCount == UsedItemCount",
        file,
        285,
        FRAME_CHECK_STATE_HASH,
    )?;
    Ok(())
}

/// `FMemCache::Init`. Frame `FRAME_OWN` for its own check and allocations; the
/// `CreateNewFreeSpace` and `CheckState` frames pass through.
///
/// # Safety
/// `start`, when non-null, must point to `bytes_to_allocate` writable bytes that
/// outlive the cache. `file` must be NUL-terminated.
pub unsafe fn init<S: CacheServices>(
    cache: &mut FMemCache,
    bytes_to_allocate: i32,
    max_items: i32,
    start: *mut u8,
    segment_size: i32,
    file: *const c_char,
    svc: &S,
) -> Result<(), i32> {
    // SAFETY: `file` is the caller's NUL-terminated `__FILE__`, per this function's contract.
    let name = unsafe { CStr::from_ptr(file) };
    check(
        svc,
        cache.initialized == 0,
        c"Initialized==0",
        name,
        31,
        FRAME_OWN,
    )?;

    cache.mem_total = bytes_to_allocate;
    cache.items_total = max_items;
    cache.mru_id = 0;
    cache.mru_item = null_mut();

    cache.cache_memory = if start.is_null() {
        // SAFETY: the name is a NUL-terminated `wide_z` literal.
        unsafe {
            alloc_checked(
                svc,
                bytes_to_allocate as usize,
                &wide_z("CacheMemory"),
                FRAME_OWN,
            )?
        }
        .cast()
    } else {
        start
    };
    // SAFETY: the name is a NUL-terminated `wide_z` literal.
    cache.item_memory = unsafe {
        alloc_checked(
            svc,
            (max_items as usize).wrapping_mul(ITEM_BYTES),
            &wide_z("CacheItems"),
            FRAME_OWN,
        )?
    }
    .cast();

    // Build the linked list of items not associated with cache memory.
    let mut prev_link: *mut *mut FCacheItem = addr_of_mut!(cache.unused_items);
    for i in 0..max_items {
        // SAFETY: `item_memory` holds `max_items` items.
        unsafe {
            let this = cache.item_memory.add(i as usize);
            *prev_link = this;
            prev_link = addr_of_mut!((*this).linear_next);
        }
    }
    // SAFETY: `prev_link` addresses the last link written above, or `unused_items`.
    unsafe {
        *prev_link = null_mut();
    }

    // Create one or more segments of free space in the cache memory.
    let mut prev: *mut FCacheItem = null_mut();
    let mut segment: i32 = 0;
    if segment_size == 0 {
        let this_item = cache.unused_items;
        let memory = cache.cache_memory;
        create_new_free_space(cache, memory, null_mut(), null_mut(), segment, name, svc)?;
        prev = this_item;
        segment += 1;
    } else {
        while segment.wrapping_mul(segment_size) < bytes_to_allocate {
            let this_item = cache.unused_items;
            let seg_start = cache
                .cache_memory
                .wrapping_offset(segment.wrapping_mul(segment_size) as isize);
            create_new_free_space(cache, seg_start, prev, null_mut(), segment, name, svc)?;
            prev = this_item;
            segment += 1;
        }
    }

    // Create the last, empty item.
    cache.last_item = cache.unused_items;
    let end = cache
        .cache_memory
        .wrapping_offset(bytes_to_allocate as isize);
    create_new_free_space(cache, end, prev, null_mut(), segment, name, svc)?;

    // Init the hash table to empty since no items are used.
    cache.hash_items = [null_mut(); HASH_COUNT];

    cache.initialized = 1;
    // SAFETY: `init` runs under its contract, so `cache` is fully set up.
    unsafe { check_state(cache, file, svc) }
}

/// `FMemCache::Exit`. Frames: the `CheckState` frames pass through.
///
/// # Safety
/// `cache` must have been set up by [`init`]. `cache_memory` is released only when
/// `free_memory` is non-zero, as in the C++.
pub unsafe fn exit<S: CacheServices>(
    cache: &mut FMemCache,
    free_memory: i32,
    file: *const c_char,
    svc: &S,
) -> Result<(), i32> {
    if cache.initialized != 0 {
        // SAFETY: `exit` runs under its contract; `initialized` is non-zero here.
        unsafe { check_state(cache, file, svc) }?;

        // `delete ItemMemory`: operator delete is a no-op on null.
        if !cache.item_memory.is_null() {
            // SAFETY: `item_memory` was allocated through `svc` by `init`.
            unsafe { svc.release(cache.item_memory.cast()) };
        }
        if free_memory != 0 {
            // SAFETY: per this function's contract, `cache_memory` is owned allocator memory.
            unsafe { svc.release(cache.cache_memory.cast()) };
        }
        cache.initialized = 0;
    }
    Ok(())
}

/// `FMemCache::Flush`. Frames: `FRAME_OWN` for its own checks and `Unhash`,
/// `FRAME_FLUSH_ITEM` for `FlushItem` (passes through).
///
/// # Safety
/// The item list and hash chains in `cache` must be those built by [`init`].
pub unsafe fn flush<S: CacheServices>(
    cache: &mut FMemCache,
    id: u64,
    mask: u32,
    ignore_locked: i32,
    file: *const c_char,
    svc: &S,
) -> Result<(), i32> {
    // SAFETY: `file` is the caller's NUL-terminated `__FILE__`, per this function's contract.
    let file = unsafe { CStr::from_ptr(file) };
    cache.mru_id = 0;
    cache.mru_item = null_mut();
    if cache.initialized == 0 {
        return Ok(());
    }

    // Special case for flushing all items.
    let mask = if id == 0 { 0 } else { mask };

    if mask == u32::MAX {
        // Quickly flush a single element.
        let mut prev_link: *mut *mut FCacheItem = addr_of_mut!(cache.hash_items[ghash(id)]);
        // SAFETY: `prev_link` walks the hash chain of live items.
        unsafe {
            while !(*prev_link).is_null() {
                let item = *prev_link;
                if (*item).id == id {
                    // Remove item from hash.
                    *prev_link = (*item).hash_next;
                    // Flush the item.
                    flush_item(cache, item, ignore_locked, svc)?;
                    break;
                }
                prev_link = addr_of_mut!((*item).hash_next);
            }
        }
    } else {
        // Slow wildcard flush of all in-memory cache items.
        let mut item = cache.cache_items;
        while !item.is_null() {
            // SAFETY: `item` is a live item of the linear list.
            unsafe {
                if (*item).id != 0 && ((*item).id & mask as u64) == (id & mask as u64) {
                    if (*item).cost < COST_INFINITE {
                        unhash(cache, (*item).id, svc)?;
                    }
                    item = flush_item(cache, item, ignore_locked, svc)?;
                } else {
                    item = (*item).linear_next;
                }
            }
        }

        // DO_CHECK is on: make sure the whole cache was flushed and that exactly one
        // free item remains per segment.
        if mask == 0 && ignore_locked == 0 {
            check(
                svc,
                !cache.cache_items.is_null(),
                c"CacheItems!=NULL",
                file,
                415,
                FRAME_OWN,
            )?;
            let mut expect_segment: i32 = 0;
            let mut test = cache.cache_items;
            while test != cache.last_item {
                // SAFETY: `test` is a live item of the linear list.
                unsafe {
                    check(
                        svc,
                        (*test).id == 0,
                        c"TestItem->Id==0",
                        file,
                        419,
                        FRAME_OWN,
                    )?;
                    check(
                        svc,
                        (*test).segment as i32 == expect_segment,
                        c"TestItem->Segment==ExpectSegment++",
                        file,
                        420,
                        FRAME_OWN,
                    )?;
                    test = (*test).linear_next;
                }
                expect_segment += 1;
            }
        }
    }
    Ok(())
}

/// `FMemCache::Create`. Returns the data pointer and the item that now holds it.
/// Frames: `FRAME_OWN` for its own checks, `Unhash`, and the `Bizarre cache alignment`
/// error; `FRAME_CREATE_NEW_FREE_SPACE` and `FRAME_EXEC` pass through.
///
/// # Safety
/// The item list and hash chains in `cache` must be those built by [`init`].
pub unsafe fn create<S: CacheServices>(
    cache: &mut FMemCache,
    id: u64,
    create_size: i32,
    alignment: i32,
    safety_pad: i32,
    file: *const c_char,
    svc: &S,
) -> Result<(*mut u8, *mut FCacheItem), i32> {
    // SAFETY: `file` is the caller's NUL-terminated `__FILE__`, per this function's contract.
    let file = unsafe { CStr::from_ptr(file) };
    // clock(CreateCycles)
    cache.create_cycles = (cache.create_cycles as u32).wrapping_sub(svc.app_cycles()) as i32;
    check(
        svc,
        cache.initialized != 0,
        c"Initialized",
        file,
        452,
        FRAME_OWN,
    )?;
    check(
        svc,
        create_size > 0,
        c"CreateSize > 0",
        file,
        453,
        FRAME_OWN,
    )?;
    check(svc, id != 0, c"Id != 0", file, 454, FRAME_OWN)?;
    cache.num_creates = cache.num_creates.wrapping_add(1);

    // Find the cheapest contiguous run of items with room for the new block.
    let mut best_cost: i64 = COST_INFINITE as i64;
    let mut best_first: *mut FCacheItem = null_mut();
    let mut best_last: *mut FCacheItem = null_mut();
    let need = create_size.wrapping_add(safety_pad) as isize;
    let mut cost: i64 = 0;
    let mut first = cache.cache_items;
    let mut last = cache.cache_items;
    while last != cache.last_item {
        // SAFETY: `first` and `last` walk live items; `last` is never the sentinel
        // when it is dereferenced here.
        unsafe {
            cost += (*last).cost as i64;
            while !first.is_null()
                && ((*(*last).linear_next).data as isize)
                    .wrapping_sub(align((*first).data, alignment) as isize)
                    >= need
            {
                if cost < best_cost && (*first).segment == (*last).segment {
                    best_cost = cost;
                    best_first = first;
                    best_last = last;
                }
                cost -= (*first).cost as i64;
                first = (*first).linear_next;
            }
            last = (*last).linear_next;
        }
    }

    if best_first.is_null() {
        // Critical error: the item can't fit in the cache.
        let mut items_locked: i32 = 0;
        let mut bytes: i32 = 0;
        let mut bytes_locked: i32 = 0;
        let mut walk = cache.cache_items;
        while walk != cache.last_item {
            // SAFETY: `walk` is a live item.
            let (size, locked) = unsafe {
                (
                    get_size(svc, walk, FRAME_OWN)?,
                    (*walk).cost >= COST_INFINITE,
                )
            };
            bytes = bytes.wrapping_add(size);
            if locked {
                items_locked = items_locked.wrapping_add(1);
                bytes_locked = bytes_locked.wrapping_add(size);
            }
            // SAFETY: `walk` is a live item.
            walk = unsafe { (*walk).linear_next };
        }
        let dump = wide_z("DUMPCACHE");
        // SAFETY: `dump` is NUL-terminated; `glog()` is the default output device.
        unsafe { exec(cache, dump.as_ptr().cast(), svc.glog(), svc) }.map_err(|_| FRAME_EXEC)?;
        raise(
            svc,
            &format!(
                "Create {:08x}.{:08X} failed: Size={} Pad={} Align={} NumLocked={} BytesLocked={}/{}",
                (id >> 32) as u32,
                id as u32,
                create_size,
                safety_pad,
                alignment,
                items_locked,
                bytes_locked,
                bytes
            ),
            FRAME_OWN,
        )?;
    }

    // Merge all items from BestFirst to BestLast into one bigger item, unhashing them.
    while best_last != best_first {
        // SAFETY: `best_last` walks back from `best_first` over live items.
        unsafe {
            if (*best_last).id != 0 {
                unhash(cache, (*best_last).id, svc)?;
            }
            best_last = merge_with_next(cache, (*best_last).linear_prev);
        }
    }
    // SAFETY: `best_first` is a live item (see the NULL check above).
    unsafe {
        if (*best_first).id != 0 {
            unhash(cache, (*best_first).id, svc)?;
        }
    }

    // SAFETY: `best_first` and its successor are live items.
    let (result, next_data, segment) = unsafe {
        let result = align((*best_first).data, alignment);
        (
            result,
            (*(*best_first).linear_next).data,
            (*best_first).segment as i32,
        )
    };

    // Claim BestFirst for the block being created, and lock it.
    // SAFETY: `best_first` is live.
    unsafe {
        (*best_first).time = cache.time as u16;
        (*best_first).id = id;
        (*best_first).cost = create_size.wrapping_add(COST_INFINITE);
    }

    // Hash it.
    let slot = addr_of_mut!(cache.hash_items[ghash(id)]);
    // SAFETY: `slot` is a bucket of `hash_items`; `best_first` is live.
    unsafe {
        (*best_first).hash_next = *slot;
        *slot = best_first;
    }

    // Create free space past the end of the newly allocated block.
    if !cache.unused_items.is_null()
        && (result.wrapping_offset(create_size as isize) as usize) < next_data as usize
    {
        // SAFETY: `best_first` is live.
        let next = unsafe { (*best_first).linear_next };
        create_new_free_space(
            cache,
            result.wrapping_offset(create_size as isize),
            best_first,
            next,
            segment,
            file,
            svc,
        )?;
    }

    // Create free space before the beginning of the newly allocated one.
    // SAFETY: `best_first` is live.
    let data = unsafe { (*best_first).data };
    if !cache.unused_items.is_null() && (result as isize).wrapping_sub(data as isize) >= IGNORE_SIZE
    {
        // Not currently used, so this is trapped as an error.
        raise(svc, "Bizarre cache alignment", FRAME_OWN)?;
        // SAFETY: `best_first` is live.
        let prev = unsafe { (*best_first).linear_prev };
        create_new_free_space(cache, data, prev, best_first, segment, file, svc)?;
        // SAFETY: `best_first` is live.
        unsafe {
            (*best_first).data = result;
        }
    }

    // Set the resulting item.
    cache.mru_item = best_first;
    cache.mru_id = id;

    // unclock(CreateCycles)
    cache.create_cycles =
        (cache.create_cycles as u32).wrapping_add(svc.app_cycles().wrapping_sub(34)) as i32;

    Ok((result, best_first))
}

/// `FMemCache::Tick`. Cannot fail.
///
/// # Safety
/// The item list in `cache` must be the one built by [`init`].
pub unsafe fn tick<S: CacheServices>(cache: &mut FMemCache, svc: &S) {
    cache.tick_cycles = (cache.tick_cycles as u32).wrapping_sub(svc.app_cycles()) as i32;
    cache.mru_id = 0;
    cache.mru_item = null_mut();

    // Init memory stats.
    cache.mem_fresh = 0;
    cache.mem_stale = 0;
    cache.items_fresh = 0;
    cache.items_stale = 0;
    cache.item_gaps = 0;

    // Decay the cost of every used item that has aged.
    let mut item = cache.cache_items;
    while item != cache.last_item {
        // SAFETY: `item` is a live item of the linear list.
        unsafe {
            if (*item).id != 0 && cache.time > (*item).time as i32 {
                (*item).cost = (*item).cost.wrapping_sub((*item).cost >> 5);
            }
            item = (*item).linear_next;
        }
    }

    // Update the cache's time.
    cache.time = cache.time.wrapping_add(1);
    cache.tick_cycles =
        (cache.tick_cycles as u32).wrapping_add(svc.app_cycles().wrapping_sub(34)) as i32;
}

/// `FMemCache::Exec`. Only `DUMPCACHE` is handled. Returns 1 when the command matched.
/// Frame: `FRAME_OWN`; `Create` maps it to `FRAME_EXEC`.
///
/// `cmd` must be a NUL-terminated `TCHAR` string; `ar` must be a live handle
/// accepted by `svc.logf` for this synchronous call.
pub unsafe fn exec<S: CacheServices>(
    cache: &FMemCache,
    cmd: *const c_void,
    ar: *mut c_void,
    svc: &S,
) -> Result<i32, i32> {
    let mut stream = cmd;
    let matched = wide_z("DUMPCACHE");
    // SAFETY: `stream` starts at the caller's NUL-terminated command.
    if !unsafe { svc.parse_command(&mut stream, &matched) } {
        return Ok(0);
    }

    let mut item = cache.cache_items;
    while item != cache.last_item {
        // SAFETY: `item` is a live item of the linear list.
        unsafe {
            let descr = if (*item).cost >= COST_INFINITE {
                "Locked"
            } else if (*item).id == 0 {
                "Empty"
            } else if cache.time.wrapping_sub((*item).time as i32) >= 1 {
                "Stale"
            } else {
                "Fresh"
            };
            let size = get_size(svc, item, FRAME_OWN)?;
            let line = format!("{:02X} [{size}]: {descr}", (*item).id as u8);
            svc.logf(ar, &wide_z(&line)).map_err(|_| FRAME_OWN)?;
            item = (*item).linear_next;
        }
    }
    Ok(1)
}

/// `FMemCache::Status`. Formats the line into `text` and resets the per-period
/// counters. Cannot fail.
///
/// # Safety
/// `text` must be the caller's `TCHAR` buffer, sized for the status line.
pub unsafe fn status<S: CacheServices>(cache: &mut FMemCache, text: *mut c_void, svc: &S) {
    let spc = svc.seconds_per_cycle();
    // `GetCycles * GSecondsPerCycle*1000` is a float expression; it widens to double
    // only at the varargs call.
    let figures = StatusFigures {
        num_gets: cache.num_gets,
        get_ms: ((cache.get_cycles as f32) * spc * 1000.0) as f64,
        num_creates: cache.num_creates,
        create_ms: ((cache.create_cycles as f32) * spc * 1000.0) as f64,
        fresh_k: cache.mem_fresh / 1024,
        stale_k: cache.mem_stale / 1024,
        items: cache
            .items_fresh
            .wrapping_add(cache.items_stale)
            .wrapping_add(cache.item_gaps),
        tick_ms: ((cache.tick_cycles as f32) * spc * 1000.0) as f64,
    };
    // SAFETY: forwarded from the function contract.
    unsafe { svc.status_format(text, &figures) };

    // Reinitialize time-variant stats.
    cache.num_gets = 0;
    cache.get_cycles = 0;
    cache.num_creates = 0;
    cache.create_cycles = 0;
    cache.tick_cycles = 0;
}
