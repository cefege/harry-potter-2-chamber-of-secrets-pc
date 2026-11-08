//! Replays `tests/fixtures/bits_unbits.txt` against the Rust port of `UnBits.cpp`
//! (`bits.rs`).
//!
//! Fixture: `Build/bits_fixture_harness/gen_bits.sh`, which extracts
//! `HarryPotter2/Unreal` at oracle ref `4f69478`, compiles the oracle with product
//! flags and `DO_GUARD_SLOW=0`, runs the driver twice, and `cmp`s the output. The
//! driver lives in `bits_harness.cpp`; its op mix is seeded with a fixed xorshift.
//!
//! Constants mirrored from the engine headers (the fixture is the authority):
//! - `PACKAGE_FILE_VERSION` 6699 (0x1A2B): `bits_harness.cpp:18`.
//! - `ENGINE_NEGOTIATION_VERSION` 400: `UnObjVer.h:17`.
//! - `PACKAGE_FILE_VERSION_LICENSEE` 0: `UnObjVer.h:30`.
//! - `NumWriters` 4, `NumReaders` 4, `NumMarks` 8: `bits_harness.cpp:131`.
//!
//! Each fixture record contains inputs, results and archive/byte state. The
//! replay checks results and state, not the removed implementation's callback
//! trace. The oracle fixture itself remains unchanged.
//!
//! The allocator mirrors the oracle's zero-fill and `PAD` slack. Buffer
//! construction/replacement runs the production Rust raw-array kernel, not a
//! second array implementation.
//!
//! NULL-source readers (`rnull`) leave their buffer zero-filled, because the
//! harness allocator is calloc-based. The product engine leaves the buffer
//! uninitialised there; that divergence is documented, not replicated.
//!
//! The fixture uses `DO_GUARD_SLOW=0`. Live `checkSlow` paths are unit-tested
//! below, not in the fixture.

use std::ffi::c_void;
use std::mem::zeroed;
use std::ptr;
use std::slice;
use std::str::FromStr;
use std::sync::LazyLock;

use hp2_abi::engine::generated::{FBitReader, FBitWriter, TArrayRaw};
use hp2_core::array_storage::Hp2ArrayServices;
use hp2_core::bits::{self, Hp2BitsServices, Threw};

const FIXTURE: &str = include_str!("fixtures/bits_unbits.txt");

const PAD: usize = 16;
const PACKAGE_FILE_VERSION: i32 = 6699;
const ENGINE_NEGOTIATION_VERSION: i32 = 400;
const PACKAGE_FILE_VERSION_LICENSEE: i32 = 0;
const NUM_WRITERS: usize = 4;
const NUM_READERS: usize = 4;
const NUM_MARKS: usize = 8;


// ---------------------------------------------------------------- allocator

unsafe extern "C" fn array_realloc(
    _context: *mut c_void,
    data: *mut c_void,
    bytes: usize,
    _max: *const i32,
    _width: i32,
    _tag: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    // libc owns allocation sizes, including after a failed operation has
    // already published new Num/Max values into the destination header.
    unsafe {
        if bytes == 0 {
            libc::free(data);
            out.write(ptr::null_mut());
            return 0;
        }
        let allocation = libc::realloc(data, bytes + PAD);
        if allocation.is_null() { return 1; }
        if data.is_null() {
            ptr::write_bytes(allocation.cast::<u8>(), 0, bytes + PAD);
        }
        out.write(allocation);
    }
    0
}

unsafe extern "C" fn checked_size(
    _context: *mut c_void, bytes: usize, out: *mut i32,
) -> i32 {
    let Ok(bytes) = i32::try_from(bytes) else { return 1; };
    unsafe { out.write(bytes); }
    0
}

unsafe extern "C" fn array_free(_context: *mut c_void, data: *mut c_void) -> i32 {
    unsafe { libc::free(data); }
    0
}

fn arrays() -> Hp2ArrayServices {
    Hp2ArrayServices {
        context: ptr::null_mut(),
        realloc: array_realloc,
        checked_int_size: checked_size,
        free: array_free,
    }
}

fn arr_free(array: &mut TArrayRaw) {
    unsafe { libc::free(array.data); }
    *array = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
}

// ---------------------------------------------------------------- slots

/// A writer in a stable heap slot, so mark and reader pointers stay valid.
struct WSlot(Box<FBitWriter>);

impl WSlot {
    fn ptr(&mut self) -> *mut FBitWriter {
        &mut *self.0 as *mut FBitWriter
    }
}

impl Drop for WSlot {
    fn drop(&mut self) {
        arr_free(&mut self.0.buffer);
    }
}

/// `FBitWriter(InMaxBits)`: the FArchive base fields, then `writer_init`.
fn new_writer(max: i32) -> WSlot {
    // SAFETY: `FBitWriter` is plain integers and pointers; all-zero is valid.
    let mut w: Box<FBitWriter> = Box::new(unsafe { zeroed() });
    w.ar_ver = PACKAGE_FILE_VERSION;
    w.ar_net_ver = ENGINE_NEGOTIATION_VERSION;
    w.ar_licensee_ver = PACKAGE_FILE_VERSION_LICENSEE;
    w.ar_is_saving = 1;
    // SAFETY: `w` is live with an initialized empty buffer.
    unsafe { bits::writer_init(&mut *w, max, arrays()) }.expect("writer allocation");
    WSlot(w)
}

/// A reader in a stable heap slot.
struct RSlot(Box<FBitReader>);

impl RSlot {
    fn ptr(&mut self) -> *mut FBitReader {
        &mut *self.0 as *mut FBitReader
    }
}

impl Drop for RSlot {
    fn drop(&mut self) {
        arr_free(&mut self.0.buffer);
    }
}

/// `FBitReader(Src, CountBits)`: the FArchive base fields, then `reader_init`.
/// `src` is `None` for the NULL-source constructor.
fn new_reader(src: Option<&[u8; 64]>, count_bits: i32) -> RSlot {
    // SAFETY: `FBitReader` is plain integers and pointers; all-zero is valid.
    let mut r: Box<FBitReader> = Box::new(unsafe { zeroed() });
    r.ar_ver = PACKAGE_FILE_VERSION;
    r.ar_net_ver = ENGINE_NEGOTIATION_VERSION;
    r.ar_licensee_ver = PACKAGE_FILE_VERSION_LICENSEE;
    r.ar_is_saving = 0;
    let src_ptr = src.map_or(ptr::null(), |s| s.as_ptr());
    // SAFETY: `r` is initialized and empty; the source covers the requested bits.
    unsafe { bits::reader_init(&mut *r, src_ptr, count_bits, arrays()) }
        .expect("reader allocation");
    RSlot(r)
}

// ---------------------------------------------------------------- oracle services

/// `appCeilLogTwo` table (`GLogs` in `bits_harness.cpp`).
static GLOGS: LazyLock<[u8; 257]> = LazyLock::new(|| {
    let mut g = [0u8; 257];
    let (mut e, mut c) = (-1i64, 0i64);
    for (i, slot) in g.iter_mut().enumerate() {
        *slot = (e + 1) as u8;
        if i == 0 {
            c = 0;
            e += 1;
        } else {
            c += 1;
            if c >= (1i64 << e) {
                c = 0;
                e += 1;
            }
        }
    }
    g
});

/// `appCeilLogTwo(Arg)` (`UnMisc.cpp:497-506`), as `BitsHookCeilLogTwo`.
fn ceil_log_two_ref(arg: u32) -> u8 {
    let a = arg.wrapping_sub(1);
    if a == u32::MAX {
        return 0;
    }
    let shift: u32 = if a <= 0x10000 {
        if a <= 0x100 { 0 } else { 8 }
    } else if a <= 0x1000000 {
        16
    } else {
        24
    };
    (shift + u32::from(GLOGS[(a >> shift) as usize])) as u8
}

/// Capture a live native assertion failure without unwinding through Rust.
unsafe extern "C" fn svc_check_fail(_expr: *const c_void, _line: i32) -> i32 {
    1
}

/// The fixture table: `DO_GUARD_SLOW=0`.
fn services() -> Hp2BitsServices {
    Hp2BitsServices {
        do_guard_slow: 0,
        check_fail: svc_check_fail,
    }
}

// ---------------------------------------------------------------- formatting

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn buf_hex(a: &TArrayRaw) -> String {
    if a.num <= 0 || a.data.is_null() {
        return String::new();
    }
    // SAFETY: `data` holds `num` valid bytes.
    hex(unsafe { slice::from_raw_parts(a.data.cast::<u8>(), a.num as usize) })
}

fn arg<T: FromStr>(t: &[&str], i: usize) -> T {
    t[i].parse()
        .unwrap_or_else(|_| panic!("field {i} of {t:?} is not a number"))
}

/// The 64-byte source the harness fills (`FillSrc`): the first bytes from the
/// hex token, zero after.
fn src_bytes(t: &[&str], i: usize) -> [u8; 64] {
    let mut out = [0u8; 64];
    let text = t.get(i).copied().unwrap_or("");
    for (k, pair) in text.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(pair).expect("ascii hex");
        out[k] = u8::from_str_radix(s, 16).expect("hex byte");
    }
    out
}

// ---------------------------------------------------------------- replay driver

#[derive(Clone, Copy, Default)]
struct Mark {
    num: i32,
    overflowed: i32,
    slot: usize,
}

struct Replay {
    writers: Vec<Option<WSlot>>,
    readers: Vec<Option<RSlot>>,
    marks: Vec<Mark>,
}

impl Replay {
    fn new() -> Self {
        Self {
            writers: (0..NUM_WRITERS).map(|_| None).collect(),
            readers: (0..NUM_READERS).map(|_| None).collect(),
            marks: vec![Mark::default(); NUM_MARKS],
        }
    }

    fn w(&mut self, s: usize) -> *mut FBitWriter {
        self.writers[s]
            .as_mut()
            .expect("writer used before wnew")
            .ptr()
    }

    fn rp(&mut self, s: usize) -> *mut FBitReader {
        self.readers[s]
            .as_mut()
            .expect("reader used before rnew/rnull")
            .ptr()
    }

    fn writer_state(&mut self, s: usize) -> String {
        let p = self.w(s);
        // SAFETY: `p` is the live writer in slot `s`.
        unsafe {
            let w = &*p;
            format!(
                "state W{s} num={} max={} buffer={} err={} ver={} netver={:08x} saving={} loading={} persistent={} nbytes={} nbits={} buf={}",
                w.num,
                w.max,
                w.buffer.num,
                w.ar_is_error,
                w.ar_ver,
                w.ar_net_ver as u32,
                w.ar_is_saving,
                w.ar_is_loading,
                w.ar_is_persistent,
                bits::writer_get_num_bytes(p),
                bits::writer_get_num_bits(p),
                buf_hex(&w.buffer),
            )
        }
    }

    fn reader_state(&mut self, s: usize) -> String {
        let p = self.rp(s);
        // SAFETY: `p` is the live reader in slot `s`.
        unsafe {
            let r = &*p;
            format!(
                "state R{s} num={} pos={} buffer={} err={} ver={} netver={:08x} saving={} loading={} persistent={} nbytes={} nbits={} atend={} buf={}",
                r.num,
                r.pos,
                r.buffer.num,
                r.ar_is_error,
                r.ar_ver,
                r.ar_net_ver as u32,
                r.ar_is_saving,
                r.ar_is_loading,
                r.ar_is_persistent,
                bits::reader_get_num_bytes(p),
                bits::reader_get_num_bits(p),
                bits::reader_at_end(p),
                buf_hex(&r.buffer),
            )
        }
    }

    /// Run one input record and return its result and archive/byte states.
    fn run(&mut self, line: &str) -> Vec<String> {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(t[0], "op", "not an op line: {line:?}");
        let svc = services();

        let (result, states): (String, Vec<String>) = match t[1] {
            "wnew" => {
                let s: usize = arg(&t, 2);
                let max: i32 = arg(&t, 3);
                self.writers[s] = Some(new_writer(max));
                (
                    "result -".into(),
                    vec![self.writer_state(s)],
                )
            }
            "wbits" => {
                let s: usize = arg(&t, 2);
                let count: i32 = arg(&t, 3);
                let src = src_bytes(&t, 4);
                // SAFETY: the writer is live; `src` covers the requested bits.
                unsafe { bits::writer_serialize_bits(self.w(s), src.as_ptr(), count) };
                ("result -".into(), vec![self.writer_state(s)])
            }
            "wserial" => {
                let s: usize = arg(&t, 2);
                let bytes: i32 = arg(&t, 3);
                let src = src_bytes(&t, 4);
                // SAFETY: the writer is live; `src` covers `bytes` bytes.
                unsafe { bits::writer_serialize(self.w(s), src.as_ptr(), bytes) };
                ("result -".into(), vec![self.writer_state(s)])
            }
            "wint" => {
                let s: usize = arg(&t, 2);
                let max: u32 = arg(&t, 3);
                let v: u32 = arg(&t, 4);
                // SAFETY: the writer is live.
                unsafe { bits::writer_serialize_int(self.w(s), v, max, &svc) }
                    .expect("no live check with DO_GUARD_SLOW=0");
                ("result -".into(), vec![self.writer_state(s)])
            }
            "wwriteint" => {
                let s: usize = arg(&t, 2);
                let max: u32 = arg(&t, 3);
                let v: u32 = arg(&t, 4);
                // SAFETY: the writer is live.
                unsafe { bits::writer_write_int(self.w(s), v, max, &svc) }
                    .expect("no live check with DO_GUARD_SLOW=0");
                ("result -".into(), vec![self.writer_state(s)])
            }
            "wbit" => {
                let s: usize = arg(&t, 2);
                let b: u8 = arg(&t, 3);
                // SAFETY: the writer is live.
                unsafe { bits::writer_write_bit(self.w(s), b) };
                (
                    "result -".into(),
                    vec![self.writer_state(s)],
                )
            }
            "wsetoverflow" => {
                let s: usize = arg(&t, 2);
                // SAFETY: the writer is live.
                unsafe { bits::writer_set_overflowed(self.w(s)) };
                (
                    "result -".into(),
                    vec![self.writer_state(s)],
                )
            }
            "mark" => {
                let m: usize = arg(&t, 2);
                let s: usize = arg(&t, 3);
                let p = self.w(s);
                // SAFETY: the writer is live.
                let (num, overflowed) = unsafe { ((*p).num, (*p).ar_is_error) };
                self.marks[m] = Mark {
                    num,
                    overflowed,
                    slot: s,
                };
                (
                    "result -".into(),
                    vec![self.writer_state(s)],
                )
            }
            "pop" => {
                let m: usize = arg(&t, 2);
                let mk = self.marks[m];
                let p = self.w(mk.slot);
                // SAFETY: the mark's writer is live and the mark is valid (the
                // harness emits `pop` only for valid marks).
                unsafe { bits::writer_mark_pop(p, mk.overflowed, mk.num, &svc) }
                    .expect("no live check with DO_GUARD_SLOW=0");
                (
                    "result -".into(),
                    vec![self.writer_state(mk.slot)],
                )
            }
            "rnew" => {
                let s: usize = arg(&t, 2);
                let count: i32 = arg(&t, 3);
                let src = src_bytes(&t, 4);
                self.readers[s] = Some(new_reader(Some(&src), count));
                ("result -".into(), vec![self.reader_state(s)])
            }
            "rnull" => {
                let s: usize = arg(&t, 2);
                let count: i32 = arg(&t, 3);
                self.readers[s] = Some(new_reader(None, count));
                (
                    "result -".into(),
                    vec![self.reader_state(s)],
                )
            }
            "rsetdata" => {
                let d: usize = arg(&t, 2);
                let s: usize = arg(&t, 3);
                let count: i32 = arg(&t, 4);
                let dp = self.rp(d);
                let sp = self.rp(s);
                // SAFETY: both readers are live; `dp` and `sp` are distinct slots
                // or the same slot, as the harness allows.
                unsafe { bits::reader_set_data(dp, sp, count, arrays()) }
                    .expect("reader replacement allocation");
                (
                    "result -".into(),
                    vec![self.reader_state(d), self.reader_state(s)],
                )
            }
            "rbits" => {
                let s: usize = arg(&t, 2);
                let count: i32 = arg(&t, 3);
                let mut dest = [0x5Au8; 64];
                // SAFETY: the reader is live; `dest` covers `(count+7)>>3` bytes.
                unsafe { bits::reader_serialize_bits(self.rp(s), dest.as_mut_ptr(), count) };
                let n = ((count + 7) >> 3) as usize;
                (
                    format!("result out={}", hex(&dest[..n])),
                    vec![self.reader_state(s)],
                )
            }
            "rserial" => {
                let s: usize = arg(&t, 2);
                let bytes: i32 = arg(&t, 3);
                let mut dest = [0x5Au8; 64];
                // SAFETY: the reader is live; `dest` covers `bytes` bytes.
                unsafe { bits::reader_serialize(self.rp(s), dest.as_mut_ptr(), bytes) };
                (
                    format!("result out={}", hex(&dest[..bytes as usize])),
                    vec![self.reader_state(s)],
                )
            }
            "rint" => {
                let s: usize = arg(&t, 2);
                let max: u32 = arg(&t, 3);
                let mut v = 0x1234_5678u32;
                // SAFETY: the reader is live; `v` is writable.
                unsafe { bits::reader_serialize_int(self.rp(s), &mut v, max) };
                (
                    format!("result value={v}"),
                    vec![self.reader_state(s)],
                )
            }
            "rreadint" => {
                let s: usize = arg(&t, 2);
                let max: u32 = arg(&t, 3);
                // SAFETY: the reader is live.
                let ret = unsafe { bits::reader_read_int(self.rp(s), max) };
                (
                    format!("result ret={ret}"),
                    vec![self.reader_state(s)],
                )
            }
            "rreadbit" => {
                let s: usize = arg(&t, 2);
                // SAFETY: the reader is live.
                let ret = unsafe { bits::reader_read_bit(self.rp(s)) };
                (
                    format!("result ret={ret}"),
                    vec![self.reader_state(s)],
                )
            }
            "rsetoverflow" => {
                let s: usize = arg(&t, 2);
                // SAFETY: the reader is live.
                unsafe { bits::reader_set_overflowed(self.rp(s)) };
                (
                    "result -".into(),
                    vec![self.reader_state(s)],
                )
            }
            other => panic!("unknown op kind {other:?}"),
        };

        let mut out = Vec::with_capacity(1 + states.len());
        out.push(result);
        out.extend(states);
        out
    }
}


// ---------------------------------------------------------------- tests

#[test]
fn bits_fixture_preserves_wire_results_and_archive_state() {
    let mut expected = FIXTURE.lines().filter(|line| !line.starts_with("hook "));
    let mut replay = Replay::new();
    while let Some(op) = expected.next() {
        assert!(op.starts_with("op "), "expected input record, got {op:?}");
        for actual in replay.run(op) {
            assert_eq!(expected.next(), Some(actual.as_str()), "after {op}");
        }
    }
}

fn live_services() -> Hp2BitsServices {
    Hp2BitsServices {
        do_guard_slow: 1,
        ..services()
    }
}

#[test]
fn ceil_log_two_matches_the_glogs_reference_at_boundaries() {
    assert_eq!(bits::ceil_log_two(0), 0);
    assert_eq!(bits::ceil_log_two(1), 0);
    for exponent in 1..32 {
        let power = 1u32 << exponent;
        assert_eq!(
            bits::ceil_log_two(power),
            ceil_log_two_ref(power),
            "exact power 2^{exponent}"
        );
        assert_eq!(
            bits::ceil_log_two(power - 1),
            ceil_log_two_ref(power - 1),
            "below power 2^{exponent}"
        );
        assert_eq!(
            bits::ceil_log_two(power + 1),
            ceil_log_two_ref(power + 1),
            "above power 2^{exponent}"
        );
    }
    assert_eq!(bits::ceil_log_two(u32::MAX), ceil_log_two_ref(u32::MAX));
}
#[test]
fn live_serialize_int_rejects_value_at_max() {
    let mut w = new_writer(64);
    let p = w.ptr();
    let res = unsafe { bits::writer_serialize_int(p, 5, 3, &live_services()) };
    assert_eq!(res, Err(Threw));
    assert_eq!(unsafe { (*p).num }, 0);
}

#[test]
fn live_write_int_rejects_value_at_max() {
    let mut w = new_writer(64);
    let p = w.ptr();
    let res = unsafe { bits::writer_write_int(p, 5, 3, &live_services()) };
    assert_eq!(res, Err(Threw));
    assert_eq!(unsafe { (*p).num }, 0);
}

#[test]
fn live_mark_pop_rejects_num_past_writer_num() {
    let mut w = new_writer(64);
    let p = w.ptr();
    let res = unsafe { bits::writer_mark_pop(p, 0, 6, &live_services()) };
    assert_eq!(res, Err(Threw));
    assert_eq!(unsafe { (*p).num }, 0);
}

#[test]
fn live_mark_pop_rejects_num_past_writer_max() {
    let mut w = new_writer(4);
    let p = w.ptr();
    unsafe { (*p).num = 10 };
    let res = unsafe { bits::writer_mark_pop(p, 0, 6, &live_services()) };
    assert_eq!(res, Err(Threw));
    assert_eq!(unsafe { (*p).num }, 10);
}

unsafe extern "C" fn fail_realloc<const NONZERO: bool>(
    context: *mut c_void, data: *mut c_void, bytes: usize,
    max: *const i32, width: i32, tag: *const c_void, out: *mut *mut c_void,
) -> i32 {
    if (bytes != 0) == NONZERO { return 1; }
    unsafe { array_realloc(context, data, bytes, max, width, tag, out) }
}

fn source_at_three() -> RSlot {
    let mut bytes = [0; 64];
    bytes[..2].copy_from_slice(&[0xad, 0x7e]);
    let mut reader = new_reader(Some(&bytes), 16);
    let mut discard = 0;
    unsafe { bits::reader_serialize_bits(reader.ptr(), &mut discard, 3); }
    reader
}

#[test]
fn reader_replacement_consumes_unaligned_bits_and_clears_tail() {
    let mut source = source_at_three();
    let mut destination = new_reader(None, 8);
    destination.0.pos = 8;
    destination.0.ar_is_error = 1;
    unsafe { bits::reader_set_data(destination.ptr(), source.ptr(), 9, arrays()) }
        .expect("replacement");
    assert_eq!(source.0.pos, 12);
    assert_eq!((destination.0.num, destination.0.pos, destination.0.ar_is_error), (9, 0, 0));
    let mut output = [0xff; 2];
    unsafe { bits::reader_serialize_bits(destination.ptr(), output.as_mut_ptr(), 9); }
    assert_eq!(output, [0xd5, 1]);
    assert_eq!(destination.0.pos, 9);
}

#[test]
fn empty_reader_replacement_retires_storage_without_consuming_source() {
    let mut source = source_at_three();
    let mut destination = new_reader(None, 16);
    unsafe { bits::reader_set_data(destination.ptr(), source.ptr(), 0, arrays()) }
        .expect("empty replacement");
    assert_eq!(source.0.pos, 3);
    assert_eq!((destination.0.num, destination.0.pos), (0, 0));
    assert!(destination.0.buffer.data.is_null());
    assert_eq!(destination.0.buffer.num, 0);
}

#[test]
fn failed_reader_allocation_does_not_consume_source_and_can_recover() {
    let mut source = source_at_three();
    let mut destination = new_reader(None, 8);
    let failing = Hp2ArrayServices { realloc: fail_realloc::<true>, ..arrays() };
    assert_eq!(
        unsafe { bits::reader_set_data(destination.ptr(), source.ptr(), 9, failing) },
        Err(Threw),
    );
    assert_eq!(source.0.pos, 3);
    assert!(destination.0.buffer.data.is_null());
    unsafe { bits::reader_set_data(destination.ptr(), source.ptr(), 9, arrays()) }
        .expect("recovery");
    assert_eq!(source.0.pos, 12);
    assert_eq!(buf_hex(&destination.0.buffer), "d501");
}

#[test]
fn failed_reader_release_keeps_storage_and_does_not_consume_source() {
    let mut source = source_at_three();
    let mut destination = new_reader(None, 8);
    let retained = destination.0.buffer.data;
    let failing = Hp2ArrayServices { realloc: fail_realloc::<false>, ..arrays() };
    assert_eq!(
        unsafe { bits::reader_set_data(destination.ptr(), source.ptr(), 9, failing) },
        Err(Threw),
    );
    assert_eq!(source.0.pos, 3);
    assert_eq!(destination.0.buffer.data, retained);
    unsafe { bits::reader_set_data(destination.ptr(), source.ptr(), 9, arrays()) }
        .expect("recovery");
    assert_eq!(source.0.pos, 12);
    assert_eq!(buf_hex(&destination.0.buffer), "d501");
}

#[test]
fn packed_bunch_archives_preserve_bit_payloads_and_overflow_boundaries() {
    // FOutBunch/FInBunch have four-byte SDK packing even though their archive
    // bases contain eight-byte pointers. Force the legal, non-eight-byte case.
    let mut writer_storage = [0_u64; (size_of::<FBitWriter>() + 15) / 8];
    let mut reader_storage = [0_u64; (size_of::<FBitReader>() + 15) / 8];
    unsafe {
        let writer = writer_storage.as_mut_ptr().cast::<u8>().add(4).cast::<FBitWriter>();
        let reader = reader_storage.as_mut_ptr().cast::<u8>().add(4).cast::<FBitReader>();
        assert_eq!(writer.addr() & 7, 4);
        assert_eq!(reader.addr() & 7, 4);
        bits::writer_init(writer, 16, arrays()).unwrap();
        let payload = [0xa5, 0x3c];
        bits::writer_serialize_bits(writer, payload.as_ptr(), 13);
        let data = ptr::addr_of!((*writer).buffer.data).read_unaligned().cast::<u8>();
        assert_eq!(slice::from_raw_parts(data, 2), &[0xa5, 0x1c]);
        bits::writer_serialize_bits(writer, payload.as_ptr(), 4);
        assert_eq!(ptr::addr_of!((*writer).num).read_unaligned(), 13);
        assert_eq!(ptr::addr_of!((*writer).ar_is_error).read_unaligned(), 1);
        bits::reader_init(reader, data, 13, arrays()).unwrap();
        let mut decoded = [0; 2];
        bits::reader_serialize_bits(reader, decoded.as_mut_ptr(), 13);
        assert_eq!(decoded, [0xa5, 0x1c]);
        assert_eq!(ptr::addr_of!((*reader).pos).read_unaligned(), 13);
        assert_eq!(ptr::addr_of!((*reader).ar_is_error).read_unaligned(), 0);
        for array in [ptr::addr_of_mut!((*writer).buffer), ptr::addr_of_mut!((*reader).buffer)] {
            hp2_core::array_storage::release(hp2_core::array_storage::Hp2ArraySlots {
                data: ptr::addr_of_mut!((*array).data),
                num: ptr::addr_of_mut!((*array).num),
                max: ptr::addr_of_mut!((*array).max),
            }, arrays()).unwrap();
        }
    }
}
