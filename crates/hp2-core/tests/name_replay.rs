//! Bit-exact replay of the `UnName.cpp` oracle fixture (`Build/name_fixture_harness/gen_name.sh`).
//!
//! The test runs the same 18 scenarios as the harness driver (`name_harness.cpp` `main`), calls the
//! Rust port (`hp2_core::name`) through its services table, and records every hook line in the
//! harness format. The emitted stream must equal the fixture line for line, and the per-kind record
//! counts are asserted exactly so a shrunken fixture fails the test.
//!
//! Boundary model (the C++ leaves are stood in for by the `m_*` functions below):
//! - The real per-instance `NameStorage` owns all mutable table state. The mock owns
//!   only hardcoded entries, `GCRCTable` and native entry allocation.
//! - The `HardcodedNames` rows are parsed from the fixture's `table` records, which the oracle
//!   harness prints from its own table. The table is therefore a data input, and the replay checks
//!   that the port and the mock agree on it.
//! - Error and log text is produced by the leaf, as in the oracle: `appFailAssert` text, `appErrorf`
//!   text, and `debugf`/`Logf` text. Each error records and returns status 1 (C++ throw).
//! - Archive ops: the leaf returns the scripted bytes and positions. The per-case scripts carry the
//!   consumed byte counts the oracle archive observed (the leaf owns the wire format).
//!
//! Compiled on macOS only, like the fixture's recording host.
#![cfg(target_os = "macos")]

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::cell::{RefCell, UnsafeCell};
use std::collections::VecDeque;
use std::ffi::{CStr, c_void};
use std::mem::zeroed;
use std::ptr::{null_mut, without_provenance_mut};

use hp2_abi::engine::generated::{FName, FNameEntry};
use hp2_core::ansi::Tchar;
use hp2_core::name::{
    self, FNAME_ADD, FNAME_DEFINE, FNAME_FIND, FNAME_INTRINSIC, Hp2NameServices, NAME_HASH_BINS,
    NameResult, NameStorage, Thrown,
};

const FIXTURE: &str = include_str!("fixtures/name_unname.txt");
const FILE: &str = "HarryPotter2/Unreal/Core/Src/UnName.cpp";

/// Scripted archive: loads consume `bytes`, `strings` (text plus consumed byte count), and `flags`.
/// `pos` mirrors the oracle's `TestArchive::InPos`.
struct ArScript {
    ver: i32,
    loading: bool,
    bytes: VecDeque<u8>,
    strings: VecDeque<(String, usize)>,
    flags: VecDeque<u32>,
    pos: usize,
}

struct Mock {
    out: Vec<String>,
    hardcoded: Vec<FNameEntry>,
    /// Hardcoded tables replaced by `reset()`, kept alive so stale `names` slots never dangle.
    retired_hardcoded: Vec<Vec<FNameEntry>>,
    rows: Vec<(u32, String)>,
    crc: Vec<u32>,
    /// Name-entry allocations `(address, reported size)`, kept across cases like the oracle.
    tracked: Vec<(usize, u32)>,
    ar: ArScript,
}

thread_local! {
    static MOCK: RefCell<Mock> = RefCell::new(Mock::new());
    static STORAGE: UnsafeCell<NameStorage> = const { UnsafeCell::new(NameStorage::new()) };
}

fn mock<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    MOCK.with(|m| f(&mut m.borrow_mut()))
}

fn storage() -> *mut NameStorage {
    STORAGE.with(UnsafeCell::get)
}

fn emit(line: impl Into<String>) {
    let line = line.into();
    mock(|m| m.out.push(line));
}

/// `FName::HardcodedNames` rows from the fixture's `table index=` records.
fn parse_rows() -> Vec<(u32, String)> {
    FIXTURE
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("table index=")?;
            let (_, rest) = rest.split_once(" flags=0x")?;
            let (flags, name) = rest.split_once(" name=")?;
            Some((u32::from_str_radix(flags, 16).ok()?, name.to_string()))
        })
        .collect()
}

/// Writes `s` as wide text with a NUL terminator.
///
/// # Safety
/// `dst` must have room for `s.len() + 1` elements.
unsafe fn write_wide(dst: *mut Tchar, s: &str) {
    unsafe {
        for (i, b) in s.bytes().enumerate() {
            *dst.add(i) = Tchar::from(b);
        }
        *dst.add(s.len()) = 0;
    }
}

/// Reads NUL-terminated wide text as the oracle's `ascii()` does: low byte per char, at most `cap - 1` chars.
///
/// # Safety
/// `p` must be NUL-terminated within `cap` elements.
unsafe fn read_wide(p: *const Tchar, cap: usize) -> String {
    let mut s = String::new();
    for i in 0..cap - 1 {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            break;
        }
        s.push((c as u8) as char);
    }
    s
}

/// # Safety
/// `e` must point to a live `FNameEntry`.
unsafe fn name_at(e: *const FNameEntry) -> *const Tchar {
    unsafe { (&raw const (*e).name).cast::<Tchar>() }
}

/// # Safety
/// `e` must point to a live `FNameEntry`.
unsafe fn name_mut(e: *mut FNameEntry) -> *mut Tchar {
    unsafe { (&raw mut (*e).name).cast::<Tchar>() }
}

/// # Safety
/// `e` must point to a live `FNameEntry`.
unsafe fn entry_name(e: *const FNameEntry) -> String {
    unsafe { read_wide(name_at(e), 256) }
}

fn fresh_hardcoded(rows: &[(u32, String)]) -> Vec<FNameEntry> {
    rows.iter()
        .map(|(flags, name)| {
            let mut e: FNameEntry = unsafe { zeroed() };
            e.flags = *flags;
            unsafe { write_wide(name_mut(&raw mut e), name) };
            e
        })
        .collect()
}

impl Mock {
    fn new() -> Self {
        // GCRCTable, built as the oracle's main() does.
        let mut crc = vec![0u32; 256];
        for (icrc, slot) in crc.iter_mut().enumerate() {
            let mut c = (icrc as u32) << 24;
            for _ in 0..8 {
                c = if c & 0x8000_0000 != 0 {
                    (c << 1) ^ 0x04c1_1db7
                } else {
                    c << 1
                };
            }
            *slot = c;
        }
        let rows = parse_rows();
        let hardcoded = fresh_hardcoded(&rows);
        Mock {
            out: Vec::new(),
            hardcoded,
            retired_hardcoded: Vec::new(),
            rows,
            crc,
            tracked: Vec::new(),
            ar: ArScript {
                ver: 0,
                loading: false,
                bytes: VecDeque::new(),
                strings: VecDeque::new(),
                flags: VecDeque::new(),
                pos: 0,
            },
        }
    }
}

/// Restores the oracle's `reset()`: table from the fixture rows, empty `Names`/`Available`,
/// cleared `NameHash`, `Initialized = 0`.
fn reset() {
    mock(|m| {
        let fresh = fresh_hardcoded(&m.rows);
        let old = std::mem::replace(&mut m.hardcoded, fresh);
        m.retired_hardcoded.push(old);
        unsafe { (*storage()).reset() };
    });
}

fn set_archive(ver: i32, loading: bool, bytes: &[u8], strings: &[(&str, usize)], flags: &[u32]) {
    mock(|m| {
        m.ar = ArScript {
            ver,
            loading,
            bytes: bytes.iter().copied().collect(),
            strings: strings
                .iter()
                .map(|(s, n)| ((*s).to_string(), *n))
                .collect(),
            flags: flags.iter().copied().collect(),
            pos: 0,
        };
    });
}

fn archive_pos() -> usize {
    mock(|m| m.ar.pos)
}

/// Dummy `FArchive*`; the mock's archive is the current script.
fn ar_ptr() -> *mut c_void {
    without_provenance_mut(1)
}

/// The oracle's `NameLength`/`FromAnsi`-free text helpers read `TCHAR` storage; here the mock
/// keeps the same storage, so this only converts a NUL-terminated source.
fn wide(s: &str) -> Vec<Tchar> {
    s.bytes()
        .map(Tchar::from)
        .chain(std::iter::once(0))
        .collect()
}

fn ctor_into(dst: &mut FName, s: &str, find: i32) -> NameResult {
    let w = wide(s);
    // The C++ constructor builds a temporary that is assigned only when it completes.
    let mut tmp = FName { index: 0 };
    unsafe { name::ctor(&SVC, storage(), &raw mut tmp, w.as_ptr(), find)? };
    *dst = tmp;
    Ok(())
}

/// `FName::GetFlags()`: `GetEntry()->Flags`, where index 0 is `&HardcodedNames[0]`.
fn name_flags(index: i32) -> u32 {
    let p = mock(|m| {
        if index == 0 {
            m.hardcoded.as_mut_ptr()
        } else {
            unsafe { (*storage()).entry(index) }
        }
    });
    unsafe { (*p).flags }
}

fn hardcoded_ptr(i: usize) -> *mut FNameEntry {
    mock(|m| m.hardcoded.as_mut_ptr().wrapping_add(i))
}

fn crc_ptr() -> *const u32 {
    mock(|m| m.crc.as_ptr())
}

/// Copies wide text including its terminator (the oracle's `wcscpy`).
///
/// # Safety
/// Both pointers must be valid, `src` NUL-terminated, `dst` large enough.
unsafe fn copy_wide(dst: *mut Tchar, src: *const Tchar) {
    unsafe {
        let mut i = 0;
        loop {
            let c = *src.add(i);
            *dst.add(i) = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
}

// ---- Leaf services (the C++ side of Hp2NameServices.cpp) -------------------------------------

unsafe extern "C" fn m_check_fail(expr: *const c_void, line: i32) -> i32 {
    unsafe {
        let text = CStr::from_ptr(expr.cast()).to_string_lossy().into_owned();
        emit(format!(
            "error Assertion failed: {text} [File:{FILE}] [Line: {line}]"
        ));
    }
    1
}

unsafe extern "C" fn m_err_ctor_before_init() -> i32 {
    emit("error FName constructed before FName::StaticInit");
    1
}

unsafe extern "C" fn m_err_hardcode_duplicate(index: i32) -> i32 {
    emit(format!("error Hardcoded name {index} was duplicated"));
    1
}

unsafe extern "C" fn m_err_name_duplicated(name: *const c_void) -> i32 {
    unsafe {
        let text = read_wide(name.cast(), 4096);
        emit(format!("error Name '{text}' was duplicated"));
    }
    1
}

unsafe extern "C" fn m_err_unhashed(name: *const c_void) -> i32 {
    unsafe {
        let text = read_wide(name.cast(), 4096);
        emit(format!("error Unhashed name '{text}'"));
    }
    1
}

unsafe extern "C" fn m_err_alloc_size() -> i32 {
    emit("error Allocation size exceeds the 32-bit allocator contract");
    1
}

unsafe extern "C" fn m_debugf_init() -> i32 {
    emit("log Init Name subsystem initialized");
    0
}

unsafe extern "C" fn m_debugf_exit() -> i32 {
    emit("log Exit Name subsystem shut down");
    0
}

unsafe extern "C" fn m_display_hash(
    _ar: *mut c_void,
    names: i32,
    used_bins: i32,
    bins: i32,
) -> i32 {
    emit(format!(
        "log Log Hash: {names} names, {used_bins}/{bins} hash bins"
    ));
    0
}

unsafe extern "C" fn m_hardcoded_count(out: *mut i32) -> i32 {
    unsafe { *out = mock(|m| m.hardcoded.len() as i32) };
    0
}

unsafe extern "C" fn m_hardcoded_entry(index: i32, out: *mut *mut c_void) -> i32 {
    unsafe { *out = hardcoded_ptr(index as usize).cast() };
    0
}

unsafe extern "C" fn m_str_len(s: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        let p = s.cast::<Tchar>();
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        *out = n as i32;
    }
    0
}

/// `appToUpper` (ASCII letters only).
fn to_upper(c: Tchar) -> Tchar {
    if (i32::from(b'a')..=i32::from(b'z')).contains(&c) {
        c - 32
    } else {
        c
    }
}

unsafe extern "C" fn m_str_icmp(a: *const c_void, b: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        let (a, b) = (a.cast::<Tchar>(), b.cast::<Tchar>());
        let mut i = 0;
        loop {
            let (ca, cb) = (to_upper(*a.add(i)), to_upper(*b.add(i)));
            if ca != cb || ca == 0 {
                *out = ca - cb;
                break;
            }
            i += 1;
        }
    }
    0
}

unsafe extern "C" fn m_str_cmp(a: *const c_void, b: *const c_void, out: *mut i32) -> i32 {
    unsafe {
        let (a, b) = (a.cast::<Tchar>(), b.cast::<Tchar>());
        let mut i = 0;
        loop {
            let (ca, cb) = (*a.add(i), *b.add(i));
            if ca != cb || ca == 0 {
                *out = ca - cb;
                break;
            }
            i += 1;
        }
    }
    0
}

unsafe extern "C" fn m_str_cpy(dst: *mut c_void, src: *const c_void) -> i32 {
    unsafe { copy_wide(dst.cast(), src.cast()) };
    0
}

unsafe extern "C" fn m_crc_table(out: *mut *const c_void) -> i32 {
    unsafe { *out = crc_ptr().cast() };
    0
}

unsafe extern "C" fn m_alloc(size: u32, out: *mut *mut c_void) -> i32 {
    unsafe {
        let p = alloc_zeroed(Layout::new::<FNameEntry>());
        mock(|m| m.tracked.push((p as usize, size)));
        emit(format!("alloc size={size} tag=NameEntry"));
        *out = p.cast();
    }
    0
}

unsafe extern "C" fn m_free_entry(entry: *mut c_void) -> i32 {
    let tracked = mock(|m| {
        let pos = m.tracked.iter().position(|t| t.0 == entry as usize)?;
        Some(m.tracked.swap_remove(pos).1)
    });
    if let Some(size) = tracked {
        emit(format!("delete size={size}"));
    }
    unsafe { dealloc(entry.cast(), Layout::new::<FNameEntry>()) };
    0
}

unsafe extern "C" fn m_ar_ver(_ar: *mut c_void, out: *mut i32) -> i32 {
    unsafe { *out = mock(|m| m.ar.ver) };
    0
}

unsafe extern "C" fn m_ar_is_loading(_ar: *mut c_void, out: *mut i32) -> i32 {
    unsafe { *out = mock(|m| i32::from(m.ar.loading)) };
    0
}

unsafe extern "C" fn m_ar_read_byte(_ar: *mut c_void, out: *mut u8) -> i32 {
    let b = mock(|m| {
        m.ar.pos += 1;
        m.ar.bytes
            .pop_front()
            .expect("archive byte script exhausted")
    });
    unsafe { *out = b };
    emit(format!("ar_read_byte value=0x{b:02x}"));
    0
}

unsafe extern "C" fn m_ar_load_string(_ar: *mut c_void, dst: *mut c_void) -> i32 {
    let (text, pos) = mock(|m| {
        let (text, consumed) =
            m.ar.strings
                .pop_front()
                .expect("archive string script exhausted");
        m.ar.pos += consumed;
        (text, m.ar.pos)
    });
    unsafe { write_wide(dst.cast(), &text) };
    emit(format!("ar_load_string value={text} pos={pos}"));
    0
}

unsafe extern "C" fn m_ar_save_string(_ar: *mut c_void, src: *const c_void) -> i32 {
    let text = unsafe { read_wide(src.cast(), 4096) };
    emit(format!("ar_save_string value={text}"));
    0
}

unsafe extern "C" fn m_ar_serialize_flags(_ar: *mut c_void, flags: *mut u32) -> i32 {
    unsafe {
        if mock(|m| m.ar.loading) {
            let v = mock(|m| {
                m.ar.pos += 4;
                m.ar.flags
                    .pop_front()
                    .expect("archive flags script exhausted")
            });
            *flags = v;
        }
        emit(format!("ar_serialize_flags value=0x{:x}", *flags));
    }
    0
}

static SVC: Hp2NameServices = Hp2NameServices {
    check_fail: m_check_fail,
    err_ctor_before_init: m_err_ctor_before_init,
    err_hardcode_duplicate: m_err_hardcode_duplicate,
    err_name_duplicated: m_err_name_duplicated,
    err_unhashed: m_err_unhashed,
    err_alloc_size: m_err_alloc_size,
    debugf_init: m_debugf_init,
    debugf_exit: m_debugf_exit,
    display_hash: m_display_hash,
    hardcoded_count: m_hardcoded_count,
    hardcoded_entry: m_hardcoded_entry,
    str_len: m_str_len,
    str_icmp: m_str_icmp,
    str_cmp: m_str_cmp,
    str_cpy: m_str_cpy,
    crc_table: m_crc_table,
    alloc: m_alloc,
    free_entry: m_free_entry,
    ar_ver: m_ar_ver,
    ar_is_loading: m_ar_is_loading,
    ar_read_byte: m_ar_read_byte,
    ar_load_string: m_ar_load_string,
    ar_save_string: m_ar_save_string,
    ar_serialize_flags: m_ar_serialize_flags,
};

// ---- Driver (mirror of name_harness.cpp) ----------------------------------------------------

fn begin_case(name: &str) {
    reset();
    emit(format!("case {name}"));
}

fn step(label: &str, f: impl FnOnce() -> NameResult) {
    emit(format!("step {label}"));
    match f() {
        Ok(()) => emit("result ok"),
        Err(Thrown) => emit("result threw"),
    }
}

fn dump_state(label: &str) {
    let (init, names, available, bins) = unsafe {
        let state = &*storage();
        (
            i32::from(state.initialized()),
            state.entries().to_vec(),
            state.available().len(),
            *state.bins(),
        )
    };
    emit(format!(
        "state {label} initialized={init} names={} available={available}",
        names.len()
    ));
    for (i, &e) in names.iter().enumerate() {
        if e.is_null() {
            continue;
        }
        let (index, flags, text) = unsafe { ((*e).index, (*e).flags, entry_name(e)) };
        emit(format!(
            "slot {i} index {index} flags 0x{flags:x} name {text}"
        ));
    }
    for (b, &head) in bins.iter().enumerate() {
        if init == 0 {
            break;
        }
        if head.is_null() {
            continue;
        }
        let mut chain = String::new();
        let mut h = head;
        while !h.is_null() {
            let (text, index, next) = unsafe {
                (
                    entry_name(h),
                    (*h).index,
                    (*h).hash_next.cast::<FNameEntry>(),
                )
            };
            chain.push_str(&format!("{text}@{index} "));
            h = next;
        }
        emit(format!("bin {b} {chain}"));
    }
}

fn value(index: i32) {
    emit(format!("value index={index}"));
}

fn case_static_init_exit() {
    begin_case("static_init_exit");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    dump_state("after_init");
    step("static_exit", || unsafe {
        name::static_exit(&SVC, storage())
    });
    dump_state("after_exit");
}

fn case_ctor_find_add_define() {
    begin_case("ctor_find_add_define");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut a = FName { index: 0 };
    let mut b = FName { index: 0 };
    let mut c = FName { index: 0 };
    let mut d = FName { index: 0 };
    let mut e = FName { index: 0 };
    let mut f = FName { index: 0 };
    let mut h = FName { index: 0 };
    step("ctor_add_hello", || {
        ctor_into(&mut a, "Hello", FNAME_ADD)?;
        value(a.index);
        Ok(())
    });
    step("ctor_find_lower", || {
        ctor_into(&mut b, "hello", FNAME_FIND)?;
        value(b.index);
        Ok(())
    });
    step("ctor_find_missing", || {
        ctor_into(&mut c, "Nope", FNAME_FIND)?;
        value(c.index);
        Ok(())
    });
    step("ctor_empty", || {
        ctor_into(&mut d, "", FNAME_ADD)?;
        value(d.index);
        Ok(())
    });
    step("define_no_change", || {
        ctor_into(&mut e, "hello", FNAME_DEFINE)?;
        value(e.index);
        Ok(())
    });
    step("define_uppercase", || {
        ctor_into(&mut f, "HELLO", FNAME_DEFINE)?;
        value(f.index);
        Ok(())
    });
    step("ctor_intrinsic", || {
        ctor_into(&mut h, "Intrinsic1", FNAME_INTRINSIC)?;
        emit(format!(
            "value index={} flags=0x{:x}",
            h.index,
            name_flags(h.index)
        ));
        Ok(())
    });
    dump_state("end");
}

fn case_reuse_available() {
    begin_case("reuse_available");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut a = FName { index: 0 };
    let mut b = FName { index: 0 };
    let mut c = FName { index: 0 };
    step("ctor_alpha", || {
        ctor_into(&mut a, "Alpha", FNAME_ADD)?;
        value(a.index);
        Ok(())
    });
    step("ctor_beta", || {
        ctor_into(&mut b, "Beta", FNAME_ADD)?;
        value(b.index);
        Ok(())
    });
    step("delete_alpha", || unsafe {
        name::delete_entry(&SVC, storage(), a.index)
    });
    step("ctor_reuse", || {
        ctor_into(&mut c, "Reuse", FNAME_ADD)?;
        value(c.index);
        Ok(())
    });
    dump_state("end");
}

fn case_delete_failures() {
    begin_case("delete_failures");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut h = FName { index: 0 };
    let mut g = FName { index: 0 };
    step("ctor_intrinsic", || {
        ctor_into(&mut h, "Intrinsic2", FNAME_INTRINSIC)?;
        value(h.index);
        Ok(())
    });
    step("delete_native", || unsafe {
        name::delete_entry(&SVC, storage(), h.index)
    });
    step("ctor_gone", || {
        ctor_into(&mut g, "Gone", FNAME_ADD)?;
        value(g.index);
        Ok(())
    });
    step("delete_gone", || unsafe {
        name::delete_entry(&SVC, storage(), g.index)
    });
    step("delete_twice", || unsafe {
        name::delete_entry(&SVC, storage(), g.index)
    });
    dump_state("end");
}

fn case_unhashed_delete() {
    begin_case("unhashed_delete");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut o = FName { index: 0 };
    step("ctor_orphan", || {
        ctor_into(&mut o, "Orphan", FNAME_ADD)?;
        value(o.index);
        Ok(())
    });
    step("unhash_orphan", || {
        let w = wide("Orphan");
        let bin =
            (unsafe { name::strihash(crc_ptr(), w.as_ptr()) } as usize) & (NAME_HASH_BINS - 1);
        unsafe { *NameStorage::bins_ptr(storage()).add(bin) = null_mut() };
        emit(format!("mutate unhash bin={bin}"));
        Ok(())
    });
    step("delete_orphan", || unsafe {
        name::delete_entry(&SVC, storage(), o.index)
    });
    dump_state("end");
}

fn case_hardcode_duplicate() {
    begin_case("hardcode_duplicate");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut dup: FNameEntry = unsafe { zeroed() };
    dup.index = 5;
    unsafe { write_wide(name_mut(&raw mut dup), "Dup5") };
    step("hardcode_dup", || unsafe {
        name::hardcode(&SVC, storage(), &raw mut dup)
    });
    dump_state("end");
}

fn case_name_too_long() {
    begin_case("name_too_long");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut x = FName { index: 0 };
    let long = "a".repeat(130);
    step("ctor_too_long", || {
        ctor_into(&mut x, &long, FNAME_ADD)?;
        value(x.index);
        Ok(())
    });
    dump_state("end");
}

fn case_ctor_before_init() {
    begin_case("ctor_before_init");
    let mut x = FName { index: 0 };
    step("ctor_early", || {
        ctor_into(&mut x, "Early", FNAME_ADD)?;
        value(x.index);
        Ok(())
    });
    dump_state("end");
}

fn case_static_init_twice() {
    begin_case("static_init_twice");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    step("static_init_again", || unsafe {
        name::static_init(&SVC, storage())
    });
    dump_state("end");
}

fn case_static_exit_uninit() {
    begin_case("static_exit_uninit");
    step("static_exit", || unsafe {
        name::static_exit(&SVC, storage())
    });
    dump_state("end");
}

fn case_display_hash() {
    begin_case("display_hash");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut a = FName { index: 0 };
    let mut b = FName { index: 0 };
    step("ctor_alpha", || {
        ctor_into(&mut a, "Alpha", FNAME_ADD)?;
        value(a.index);
        Ok(())
    });
    step("ctor_beta", || {
        ctor_into(&mut b, "Beta", FNAME_ADD)?;
        value(b.index);
        Ok(())
    });
    step("display_hash", || unsafe {
        name::display_hash(&SVC, storage(), ar_ptr())
    });
    dump_state("end");
}

fn case_serialize_v63_load() {
    begin_case("serialize_v63_load");
    set_archive(63, true, &[b'A', b'b', 0], &[], &[0x1234_5678]);
    let mut e: FNameEntry = unsafe { zeroed() };
    step("entry_load_v63", || {
        unsafe { name::entry_serialize(&SVC, ar_ptr(), &raw mut e)? };
        let text = unsafe { entry_name(&raw const e) };
        emit(format!(
            "value name={text} flags=0x{:x} pos={}",
            e.flags,
            archive_pos()
        ));
        Ok(())
    });
}

fn case_serialize_v63_save_check() {
    begin_case("serialize_v63_save_check");
    set_archive(63, false, &[], &[], &[]);
    let mut e: FNameEntry = unsafe { zeroed() };
    unsafe { write_wide(name_mut(&raw mut e), "Ab") };
    e.flags = 0x21;
    step("entry_save_v63", || unsafe {
        name::entry_serialize(&SVC, ar_ptr(), &raw mut e)
    });
}

fn case_serialize_v64_load() {
    begin_case("serialize_v64_load");
    set_archive(64, true, &[], &[("Ab", 4)], &[0x1234_5678]);
    let mut e: FNameEntry = unsafe { zeroed() };
    step("entry_load_v64", || {
        unsafe { name::entry_serialize(&SVC, ar_ptr(), &raw mut e)? };
        let text = unsafe { entry_name(&raw const e) };
        emit(format!(
            "value name={text} flags=0x{:x} pos={}",
            e.flags,
            archive_pos()
        ));
        Ok(())
    });
}

fn case_serialize_v64_save() {
    begin_case("serialize_v64_save");
    set_archive(64, false, &[], &[], &[]);
    let mut e: FNameEntry = unsafe { zeroed() };
    unsafe { write_wide(name_mut(&raw mut e), "Hi") };
    e.flags = 0x21;
    step("entry_save_v64", || unsafe {
        name::entry_serialize(&SVC, ar_ptr(), &raw mut e)
    });
}

fn case_serialize_v64_long_roundtrip() {
    begin_case("serialize_v64_long_roundtrip");
    let long: String = (0..100u8).map(|i| char::from(b'a' + i % 26)).collect();
    let mut e: FNameEntry = unsafe { zeroed() };
    unsafe { write_wide(name_mut(&raw mut e), &long) };
    e.flags = 0x8000_0001;
    set_archive(64, false, &[], &[], &[]);
    step("entry_save_long", || unsafe {
        name::entry_serialize(&SVC, ar_ptr(), &raw mut e)
    });
    // Wire length of the saved string record (2-byte compact index + 101 bytes), then 4 flag bytes.
    set_archive(64, true, &[], &[(&long, 103)], &[0x8000_0001]);
    let mut back: FNameEntry = unsafe { zeroed() };
    step("entry_load_long", || {
        unsafe { name::entry_serialize(&SVC, ar_ptr(), &raw mut back)? };
        let text = unsafe { entry_name(&raw const back) };
        emit(format!(
            "value name={text} flags=0x{:x} pos={}",
            back.flags,
            archive_pos()
        ));
        Ok(())
    });
}

fn case_static_exit_frees() {
    begin_case("static_exit_frees");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    let mut a = FName { index: 0 };
    let mut b = FName { index: 0 };
    let mut c = FName { index: 0 };
    step("ctor_free1", || {
        ctor_into(&mut a, "Free1", FNAME_ADD)?;
        value(a.index);
        Ok(())
    });
    step("ctor_free2", || {
        ctor_into(&mut b, "Free2", FNAME_ADD)?;
        value(b.index);
        Ok(())
    });
    step("ctor_free3", || {
        ctor_into(&mut c, "Free3", FNAME_ADD)?;
        value(c.index);
        Ok(())
    });
    step("static_exit", || unsafe {
        name::static_exit(&SVC, storage())
    });
    dump_state("end");
}

fn case_hardcoded_duplicate_name() {
    begin_case("hardcoded_duplicate_name");
    unsafe { copy_wide(name_mut(hardcoded_ptr(3)), name_at(hardcoded_ptr(4))) };
    emit("mutate hardcoded 3 = hardcoded 4");
    step("static_init", || unsafe {
        name::static_init(&SVC, storage())
    });
    dump_state("end");
}
fn run_all_cases() {
    let count = mock(|m| m.hardcoded.len());
    emit(format!("table count={count}"));
    for i in 0..count {
        let p = hardcoded_ptr(i);
        let (flags, text) = unsafe { ((*p).flags, entry_name(p)) };
        emit(format!("table index={i} flags=0x{flags:x} name={text}"));
    }

    case_static_init_exit();
    case_ctor_find_add_define();
    case_reuse_available();
    case_delete_failures();
    case_unhashed_delete();
    case_hardcode_duplicate();
    case_name_too_long();
    case_ctor_before_init();
    case_static_init_twice();
    case_static_exit_uninit();
    case_display_hash();
    case_serialize_v63_load();
    case_serialize_v63_save_check();
    case_serialize_v64_load();
    case_serialize_v64_save();
    case_serialize_v64_long_roundtrip();
    case_static_exit_frees();
    case_hardcoded_duplicate_name();
}

#[test]
fn name_replay_bit_exact() {
    run_all_cases();
    let got: Vec<String> = mock(|m| std::mem::take(&mut m.out));
    let want: Vec<&str> = FIXTURE.lines().collect();

    for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(g.as_str(), *w, "record {} differs", i + 1);
    }
    assert_eq!(got.len(), want.len(), "replay record total");
}

#[test]
fn name_storage_boundaries_lifo_stability_reinit() {
    reset();
    unsafe {
        let state = storage();
        assert!(!(*state).initialized());
        assert_eq!((*state).len(), 0);
        assert!((*state).is_valid(0));
        assert!(!(*state).is_valid(-1));
        assert!(!(*state).is_valid(1));
        assert!(!(*state).is_valid(i32::MAX));

        name::static_init(&SVC, state).unwrap();
        let hardcoded_count = mock(|m| m.hardcoded.len() as i32);
        assert_eq!((*state).len(), hardcoded_count);
        assert_eq!((*state).entry(0), hardcoded_ptr(0));
        assert!((*state).is_valid(hardcoded_count - 1));
        assert!(!(*state).is_valid(hardcoded_count));
        let mut first = FName { index: -1 };
        let mut second = FName { index: -1 };
        ctor_into(&mut first, "StorageFirst", FNAME_ADD).unwrap();
        ctor_into(&mut second, "StorageSecond", FNAME_ADD).unwrap();
        let first_entry = (*state).entry(first.index);
        let second_entry = (*state).entry(second.index);
        for i in 0..2048 {
            let mut added = FName { index: -1 };
            ctor_into(&mut added, &format!("StorageGrowth{i}"), FNAME_ADD).unwrap();
        }
        assert_eq!((*state).entry(first.index), first_entry);
        assert_eq!((*state).entry(second.index), second_entry);
        assert_eq!((*state).entry(0), hardcoded_ptr(0));
        assert_eq!(entry_name(first_entry), "StorageFirst");

        name::delete_entry(&SVC, state, first.index).unwrap();
        name::delete_entry(&SVC, state, second.index).unwrap();
        assert!(!(*state).is_valid(first.index));
        assert!(!(*state).is_valid(second.index));
        assert!((*state).entry(first.index).is_null());
        assert_eq!((*state).available(), &[first.index, second.index]);
        let high_water = (*state).len();
        let mut reused = FName { index: -1 };
        ctor_into(&mut reused, "StorageReuseLast", FNAME_ADD).unwrap();
        assert_eq!(reused.index, second.index);
        ctor_into(&mut reused, "StorageReuseFirst", FNAME_ADD).unwrap();
        assert_eq!(reused.index, first.index);
        assert_eq!((*state).len(), high_water);
        assert!((*state).available().is_empty());

        name::static_exit(&SVC, state).unwrap();
        assert!(!(*state).initialized());
        assert!((*state).is_empty());
        assert!((*state).available().is_empty());
        assert!((*state).is_valid(0));
        assert!(!(*state).is_valid(first.index));
        assert_eq!(entry_name(hardcoded_ptr(0)), "None");
        name::static_init(&SVC, state).unwrap();
        assert_eq!((*state).len(), hardcoded_count);
        assert_eq!((*state).entry(0), hardcoded_ptr(0));
        ctor_into(&mut reused, "StorageFirst", FNAME_FIND).unwrap();
        assert_eq!(reused.index, 0);
        ctor_into(&mut reused, "StorageReinitialized", FNAME_ADD).unwrap();
        assert_eq!(reused.index, hardcoded_count);
        name::static_exit(&SVC, state).unwrap();
    }
}

/// A native allocator can reenter name lookup/creation before reporting its first
/// error. It must see the already-grown table, and nested growth must not invalidate
/// any Rust storage borrow or pointer slot held by the suspended constructor.
unsafe extern "C" fn m_reentrant_alloc_failure(_size: u32, _out: *mut *mut c_void) -> i32 {
    unsafe {
        let state = storage();
        let reserved = (*state).len() - 1;
        assert!((*state).entry(reserved).is_null());
        for i in 0..512 {
            let mut nested = FName { index: -1 };
            ctor_into(&mut nested, &format!("ReentrantName{i}"), FNAME_ADD).unwrap();
            assert_eq!(nested.index, reserved + 1 + i);
        }
    }
    1
}

#[test]
fn name_storage_reentrant_allocation_first_error() {
    reset();
    unsafe {
        let state = storage();
        name::static_init(&SVC, state).unwrap();
        let count = (*state).len();
        let none = (*state).entry(0);
        let failing = Hp2NameServices {
            alloc: m_reentrant_alloc_failure,
            ..SVC
        };
        let text = wide("SuspendedAllocation");
        let mut handle = FName { index: -1 };
        assert_eq!(
            name::ctor(&failing, state, &raw mut handle, text.as_ptr(), FNAME_ADD),
            Err(Thrown)
        );
        assert_eq!(handle.index, count);
        assert_eq!((*state).len(), count + 1 + 512);
        assert!((*state).entry(count).is_null());
        assert!(!(*state).is_valid(count));
        assert!((*state).available().is_empty());
        assert_eq!((*state).entry(0), none);
        ctor_into(&mut handle, "SuspendedAllocation", FNAME_FIND).unwrap();
        assert_eq!(handle.index, 0);
        ctor_into(&mut handle, "ReentrantName511", FNAME_FIND).unwrap();
        assert_eq!(handle.index, count + 512);
        name::static_exit(&SVC, state).unwrap();
    }
}
