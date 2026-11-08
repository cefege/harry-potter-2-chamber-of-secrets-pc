//! Bit-exact replay of the `UnCache.cpp` oracle fixture (`Build/cache_fixture_harness/gen_cache.sh`).
//!
//! The test runs the same 52 scenarios as the harness driver (`cache_harness.cpp` `main`),
//! drives the Rust port with recording services in place of the C++ hooks, and re-encodes
//! each op, its hook lines, and the observable cache state in the harness format. The emitted
//! text must equal the fixture line for line. Record counts are asserted exactly so a silently
//! shrunken fixture fails the test.
//!
//! Notes on the mirror:
//! - Hook text (`errorf`, `logf`, `status_format`) is produced by the port and only narrowed
//!   here, so the fixture checks the port's messages and figures, not the recorder's own text.
//! - `ParseCommand` runs through the production Rust parser; the recorder only
//!   records its result and original input before the cache port continues.
//! - Allocations are 4096-aligned and zeroed, matching the harness. `Align` in the port uses
//!   absolute addresses, so the offsets it produces are identical to the oracle's.
//! - Pointers print as item indexes (`ixs`) and byte offsets from CacheMemory (`offs`), never
//!   as addresses, so the transcript is deterministic.
//!
//! Compiled on macOS only, like the fixture's recording host.
#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_char, c_void};
use std::fmt::Write as _;
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;

use hp2_abi::engine::generated::{FCacheItem, FMemCache};
use hp2_core::ansi::Tchar;
use hp2_core::cache::{self, CacheServices, StatusFigures, ghash};
use hp2_core::mem::{MemServices, wide_z};

const FIXTURE: &str = include_str!("fixtures/cache_ops.txt");
const EXPECTED_LINES: usize = 2368;
const EXPECTED_SCENARIOS: usize = 52;
const EXPECTED_OPS: usize = 195;
const EXPECTED_UNWIND: usize = 40;

/// Matches `FMemCache::COST_INFINITE` in the oracle.
const COST_INFINITE: i32 = 0x0100_0000;
/// Matches `FMemCache::HASH_COUNT` in the oracle.
const HASH_COUNT: usize = 16384;
/// Matches the harness's `g_cycles` reset and its 100-cycle step per `AppCycles` call.
const START_CYCLES: u32 = 1_000_000;
/// Bytes in the harness start buffer (`g_startbuf`); the port only ever reads `init`'s argument.
const START_BYTES: usize = 4096;

unsafe extern "C" {
    fn posix_memalign(memptr: *mut *mut c_void, alignment: usize, size: usize) -> i32;
    fn free(ptr: *mut c_void);
}

fn file() -> *const c_char {
    c"UnCache.cpp".as_ptr()
}

/// Narrows a `Tchar` string, stopping at NUL or after `limit` characters.
///
/// # Safety
/// `p` must point to a NUL-terminated `Tchar` string, or to at least `limit` readable `Tchar`s.
unsafe fn narrow_ptr(p: *const Tchar, limit: usize) -> String {
    let mut s = String::new();
    for k in 0..limit {
        // SAFETY: the caller guarantees the string is readable up to its NUL or `limit`.
        let c = unsafe { *p.add(k) };
        if c == 0 {
            break;
        }
        s.push((c as u8) as char);
    }
    s
}

/// Narrows a `Tchar` slice, stopping at its first NUL.
fn narrow_slice(s: &[Tchar]) -> String {
    let mut out = String::new();
    for &c in s {
        if c == 0 {
            break;
        }
        out.push((c as u8) as char);
    }
    out
}

/// Unwind names for each failing (op, frame) pair, innermost first, as the oracle's guard chain
/// prints them. Pairs not observed in the fixture panic so a new path fails loudly.
fn unwind_names(kind: &str, frame: i32) -> &'static [&'static str] {
    match (kind, frame) {
        ("init", 1) => &["FMemCache::Init"],
        ("init", 3) => &["FMemCache::CreateNewFreeSpace", "FMemCache::Init"],
        ("create", 1) => &["FMemCache::Create"],
        ("create", 4) => &["FMemCache::Exec", "FMemCache::Create"],
        ("exec", 1) => &["FMemCache::Exec"],
        ("exit", 5) => &["FMemCache::CheckState", "FMemCache::Exit"],
        ("flush", 1) => &["FMemCache::Flush"],
        ("flush", 2) => &["FMemCache::FlushItem", "FMemCache::Flush"],
        ("check", 5) => &["FMemCache::CheckState"],
        ("check", 6) => &["1", "FMemCache::CheckState"],
        ("check", 7) => &["3", "FMemCache::CheckState"],
        _ => panic!("no unwind mapping for {kind} frame {frame}"),
    }
}

/// Recording services standing in for `Hp2MemServices` and the harness's `Hp2CacheServices`
/// stand-ins. Each hook appends the same line the harness prints.
struct Recorder {
    out: RefCell<String>,
    cycles: Cell<u32>,
    /// `g_fail_alloc`: the next allocation with this tag reports failure.
    fail_alloc: Cell<Option<&'static str>>,
    /// `g_fail_logf`: `Logf` to the device with this name reports failure.
    fail_logf: Cell<Option<&'static str>>,
    /// Live allocations, `(address, tag)`, released by `drop_allocs` or by the port's `Free`.
    allocs: RefCell<Vec<(usize, String)>>,
    /// Sentinel output devices; their addresses identify `glog` and `user` in `logf`.
    glog_dev: u8,
    user_dev: u8,
}

impl Recorder {
    fn new() -> Self {
        Self {
            out: RefCell::new(String::new()),
            cycles: Cell::new(START_CYCLES),
            fail_alloc: Cell::new(None),
            fail_logf: Cell::new(None),
            allocs: RefCell::new(Vec::new()),
            glog_dev: 0,
            user_dev: 0,
        }
    }

    fn line(&self, s: String) {
        let mut out = self.out.borrow_mut();
        out.push_str(&s);
        out.push('\n');
    }

    fn glog_ptr(&self) -> *mut c_void {
        &self.glog_dev as *const u8 as *mut c_void
    }

    fn user_ptr(&self) -> *mut c_void {
        &self.user_dev as *const u8 as *mut c_void
    }

    /// Frees every live allocation without recording, like the harness's `drop_allocs`.
    fn drop_allocs(&self) {
        for (p, _) in self.allocs.borrow_mut().drain(..) {
            // SAFETY: every entry came from `posix_memalign` and is removed as it is freed.
            unsafe { free(p as *mut c_void) };
        }
    }
}

impl MemServices for Recorder {
    unsafe fn alloc(&self, size: u32, tag: *const c_void) -> Result<*mut c_void, i32> {
        // SAFETY: tags are NUL-terminated `Tchar` strings from the port's static names.
        let tag = unsafe { narrow_ptr(tag.cast::<Tchar>(), 256) };
        let fail = self.fail_alloc.get() == Some(tag.as_str());
        self.line(format!(
            "malloc {tag} {size} {}",
            if fail { "fail" } else { "ok" }
        ));
        if fail {
            return Err(1);
        }
        let bytes = (size.max(1) as usize).next_multiple_of(4096);
        let mut p: *mut c_void = null_mut();
        // SAFETY: `p` is a valid out-pointer; a zero return yields `bytes` bytes at 4096 alignment.
        let rc = unsafe { posix_memalign(&mut p, 4096, bytes) };
        if rc != 0 {
            return Err(1);
        }
        // SAFETY: `p` addresses `bytes` writable bytes.
        unsafe { std::ptr::write_bytes(p.cast::<u8>(), 0, bytes) };
        self.allocs.borrow_mut().push((p as usize, tag));
        Ok(p)
    }

    unsafe fn release(&self, ptr: *mut c_void) {
        let mut allocs = self.allocs.borrow_mut();
        let pos = allocs.iter().position(|(p, _)| *p == ptr as usize);
        let Some(i) = pos else {
            drop(allocs);
            self.line("free unknown".into());
            return;
        };
        let (p, tag) = allocs.swap_remove(i);
        drop(allocs);
        self.line(format!("free {tag}"));
        // SAFETY: `p` was returned by `posix_memalign` and has just left the registry.
        unsafe { free(p as *mut c_void) };
    }

    fn raise_error(&self, msg: &[Tchar]) -> i32 {
        self.line(format!("errorf {}", narrow_slice(msg)));
        1
    }

    fn fail_assert(&self, expr: &CStr, file: &CStr, line: i32) -> i32 {
        let file = file.to_string_lossy().into_owned();
        self.line(format!(
            "fail_assert {line} {file} {}",
            expr.to_string_lossy()
        ));
        1
    }
}

impl CacheServices for Recorder {
    unsafe fn logf(&self, ar: *mut c_void, text: &[Tchar]) -> Result<(), i32> {
        let name = if ar == self.glog_ptr() {
            "glog"
        } else if ar == self.user_ptr() {
            "user"
        } else {
            "other"
        };
        let fail = self.fail_logf.get() == Some(name);
        self.line(format!(
            "logf {name} {} {}",
            if fail { "fail" } else { "ok" },
            narrow_slice(text)
        ));
        if fail { Err(1) } else { Ok(()) }
    }

    fn glog(&self) -> *mut c_void {
        self.line("glog".into());
        self.glog_ptr()
    }

    unsafe fn parse_command(&self, stream: *mut *const c_void, matched: &[Tchar]) -> bool {
        let want = narrow_slice(matched);
        // SAFETY: this recorder is called with a valid cursor and the matched
        // command is a NUL-terminated TCHAR string.
        let typed_stream = stream.cast::<*const Tchar>();
        let input = unsafe { narrow_ptr(*typed_stream, 512) };
        let result =
            unsafe { hp2_core::command::parse_command(typed_stream, matched.as_ptr()) != 0 };
        self.line(format!("parse_command {} {want} {input}", result as i32));
        result
    }

    fn app_cycles(&self) -> u32 {
        let v = self.cycles.get();
        self.cycles.set(v.wrapping_add(100));
        self.line(format!("app_cycles {v}"));
        v
    }

    fn seconds_per_cycle(&self) -> f32 {
        let v = 4.1666667e-10_f32;
        self.line(format!("seconds_per_cycle {:08x}", v.to_bits()));
        v
    }

    unsafe fn status_format(&self, out: *mut c_void, f: &StatusFigures) {
        let text = format!(
            "Gets={:04} ({:04.1}) Crts={:03} ({:04.1}) Fresh={:03}K Stale={:03}K Items={:03} Tick={:04.1}",
            f.num_gets,
            f.get_ms,
            f.num_creates,
            f.create_ms,
            f.fresh_k,
            f.stale_k,
            f.items,
            f.tick_ms,
        );
        let dest = out.cast::<Tchar>();
        for (i, b) in text.bytes().enumerate() {
            // SAFETY: `out` is the status buffer, sized for the text and its NUL.
            unsafe { *dest.add(i) = b as Tchar };
        }
        // SAFETY: the NUL lands one past the text, inside the same buffer.
        unsafe { *dest.add(text.len()) = 0 };
        self.line(format!(
            "status_format {} {:016x} {} {:016x} {} {} {} {:016x} {text}",
            f.num_gets,
            f.get_ms.to_bits(),
            f.num_creates,
            f.create_ms.to_bits(),
            f.fresh_k,
            f.stale_k,
            f.items,
            f.tick_ms.to_bits(),
        ));
    }
}

/// `start` buffer for `init`'s caller-supplied memory, aligned like the oracle's `g_startbuf`.
#[repr(C, align(4096))]
struct StartBuf([u8; START_BYTES]);

/// What an op returns on success, printed after `result ok` the way the harness's `g_ret` is.
enum Ret {
    Done,
    Create(*mut u8, *mut FCacheItem),
    Exec(i32),
}

fn done(res: Result<(), i32>) -> Result<Ret, i32> {
    res.map(|()| Ret::Done)
}

struct Replay {
    rec: Box<Recorder>,
    cache: Box<FMemCache>,
    start: Box<StartBuf>,
    /// `g_items_alloc`: bounds the `ixs` index range.
    items_alloc: i32,
}

impl Drop for Replay {
    fn drop(&mut self) {
        self.rec.drop_allocs();
    }
}

impl Replay {
    fn new() -> Self {
        Self {
            rec: Box::new(Recorder::new()),
            // SAFETY: `FMemCache` is plain data (integers and raw pointers); all-zero is valid.
            cache: Box::new(unsafe { zeroed() }),
            start: Box::new(StartBuf([0; START_BYTES])),
            items_alloc: 0,
        }
    }

    /// Mirrors the harness `scenario`: a scenario line, silent release of leftovers, and a reset
    /// of the cache, start buffer, cycle counter, item bound, and failure switches.
    fn scenario(&mut self, name: &str) {
        self.rec.line(format!("scenario {name}"));
        self.rec.drop_allocs();
        // SAFETY: as in `new`, all-zero is a valid `FMemCache`.
        *self.cache = unsafe { zeroed() };
        self.start.0 = [0; START_BYTES];
        self.rec.cycles.set(START_CYCLES);
        self.items_alloc = 0;
        self.rec.fail_alloc.set(None);
        self.rec.fail_logf.set(None);
    }

    /// Mirrors the harness `run`: an `op` line, the op, `result` (with unwind lines on failure),
    /// then the state dump.
    fn run<F>(&mut self, desc: &str, kind: &str, op: F)
    where
        F: FnOnce(&mut FMemCache, &Recorder) -> Result<Ret, i32>,
    {
        self.rec.line(format!("op {desc}"));
        match op(&mut self.cache, &self.rec) {
            Ok(ret) => {
                let text = self.ret_text(&ret);
                self.rec.line(format!("result ok{text}"));
            }
            Err(frame) => {
                for name in unwind_names(kind, frame) {
                    self.rec.line(format!("unwind {name}"));
                }
                self.rec.line(format!("result err {frame}"));
            }
        }
        self.dump_state();
    }

    fn ret_text(&self, ret: &Ret) -> String {
        match ret {
            Ret::Done => String::new(),
            Ret::Create(data, item) => format!(" {} {}", self.offs(*data), self.ixs(*item)),
            Ret::Exec(v) => format!(" {v}"),
        }
    }

    /// Item index of a pointer: `-` for NULL, `x` for a pointer outside ItemMemory.
    fn ixs(&self, p: *const FCacheItem) -> String {
        if p.is_null() {
            return "-".into();
        }
        let base = self.cache.item_memory as usize;
        let v = p as usize;
        let end =
            base.wrapping_add(size_of::<FCacheItem>().wrapping_mul(self.items_alloc as usize));
        if v < base || v >= end {
            return "x".into();
        }
        ((v - base) / size_of::<FCacheItem>()).to_string()
    }

    /// Byte offset of a data pointer from CacheMemory, `-` for NULL.
    fn offs(&self, p: *const u8) -> String {
        if p.is_null() {
            return "-".into();
        }
        let base = self.cache.cache_memory as usize;
        ((p as usize).wrapping_sub(base) as isize).to_string()
    }

    /// Mirrors the harness `dump_state`. Item and list lines are only printed while
    /// `initialized` is non-zero, so no freed or never-initialized memory is walked.
    fn dump_state(&self) {
        let c: &FMemCache = &self.cache;
        self.rec.line(format!(
            "state initialized={} mem_total={} items_total={} time={} mru_id={:016x} mru_item={} cache_items={} last_item={} unused_items={} cache_memory={} item_memory={}",
            c.initialized,
            c.mem_total,
            c.items_total,
            c.time,
            c.mru_id,
            self.ixs(c.mru_item),
            self.ixs(c.cache_items),
            self.ixs(c.last_item),
            self.ixs(c.unused_items),
            if c.cache_memory.is_null() { "-" } else { "set" },
            if c.item_memory.is_null() { "-" } else { "set" },
        ));
        self.rec.line(format!(
            "state counters num_gets={} get_cycles={} num_creates={} create_cycles={} tick_cycles={} items_fresh={} items_stale={} item_gaps={} mem_fresh={} mem_stale={}",
            c.num_gets,
            c.get_cycles,
            c.num_creates,
            c.create_cycles,
            c.tick_cycles,
            c.items_fresh,
            c.items_stale,
            c.item_gaps,
            c.mem_fresh,
            c.mem_stale,
        ));
        if c.initialized == 0 {
            return;
        }
        let mut steps = 0usize;
        let mut p = c.cache_items;
        while !p.is_null() && steps < 4096 {
            // SAFETY: an initialized cache's linked items live in ItemMemory until exit.
            let it = unsafe { &*p };
            self.rec.line(format!(
                "item {} id={:016x} off={} time={} seg={} extra={} cost={} next={} prev={} hash={}",
                self.ixs(p),
                it.id,
                self.offs(it.data),
                it.time,
                it.segment,
                it.extra,
                it.cost,
                self.ixs(it.linear_next),
                self.ixs(it.linear_prev),
                self.ixs(it.hash_next),
            ));
            p = it.linear_next;
            steps += 1;
        }
        self.rec.line(format!("items walked={steps}"));
        let mut unused = String::from("unused");
        let mut p = c.unused_items;
        let mut u = 0usize;
        while !p.is_null() && u < 4096 {
            // SAFETY: as above; the unused list is walked only while initialized.
            let it = unsafe { &*p };
            write!(unused, " {}", self.ixs(p)).expect("writing to a String");
            p = it.linear_next;
            u += 1;
        }
        self.rec.line(unused);
        for (b, head) in c.hash_items.iter().enumerate().take(HASH_COUNT) {
            if head.is_null() {
                continue;
            }
            let mut s = format!("hash {b}");
            let mut p = *head;
            let mut h = 0usize;
            while !p.is_null() && h < 4096 {
                // SAFETY: as above; hash chains only hold live items.
                let it = unsafe { &*p };
                write!(s, " {}", self.ixs(p)).expect("writing to a String");
                p = it.hash_next;
                h += 1;
            }
            self.rec.line(s);
        }
    }

    fn op_init(&mut self, bytes: i32, max: i32, start: bool, seg: i32) {
        self.items_alloc = max;
        let start_ptr = if start {
            self.start.0.as_mut_ptr()
        } else {
            null_mut()
        };
        let desc = format!("init {bytes} {max} {} {seg}", start as i32);
        self.run(&desc, "init", |c, r| {
            // SAFETY: `start_ptr` is null or the 4096-byte aligned start buffer, live for the call.
            let res = unsafe { cache::init(c, bytes, max, start_ptr, seg, file(), r) };
            done(res)
        });
    }

    fn op_exit(&mut self, free_memory: i32) {
        let desc = format!("exit {free_memory}");
        self.run(&desc, "exit", |c, r| {
            // SAFETY: `c` and `r` are live for the call.
            let res = unsafe { cache::exit(c, free_memory, file(), r) };
            done(res)
        });
    }

    fn op_create(&mut self, id: u64, size: i32, align: i32, pad: i32) {
        let desc = format!("create {id:016x} {size} {align} {pad}");
        self.run(&desc, "create", |c, r| {
            // SAFETY: `c` and `r` are live for the call.
            let res = unsafe { cache::create(c, id, size, align, pad, file(), r) };
            res.map(|(data, item)| Ret::Create(data, item))
        });
    }

    fn op_flush(&mut self, id: u64, mask: u32, ignore_locked: i32) {
        let desc = format!("flush {id:016x} {mask:08x} {ignore_locked}");
        self.run(&desc, "flush", |c, r| {
            // SAFETY: `c` and `r` are live for the call.
            let res = unsafe { cache::flush(c, id, mask, ignore_locked, file(), r) };
            done(res)
        });
    }

    fn op_tick(&mut self) {
        self.run("tick", "tick", |c, r| {
            // SAFETY: `c` and `r` are live for the call.
            unsafe { cache::tick(c, r) };
            Ok(Ret::Done)
        });
    }

    fn op_check(&mut self) {
        self.run("check", "check", |c, r| {
            // SAFETY: `c` and `r` are live for the call.
            let res = unsafe { cache::check_state(c, file(), r) };
            done(res)
        });
    }

    fn op_exec(&mut self, cmd: &str, glog: bool) {
        let desc = format!("exec {cmd} {}", if glog { "glog" } else { "user" });
        let wide = wide_z(cmd);
        let ar = if glog {
            self.rec.glog_ptr()
        } else {
            self.rec.user_ptr()
        };
        self.run(&desc, "exec", |c, r| {
            // SAFETY: `wide` is NUL-terminated and live for the call; `ar` is a sentinel device.
            let res = unsafe { cache::exec(c, wide.as_ptr().cast(), ar, r) };
            res.map(Ret::Exec)
        });
    }

    fn op_status(&mut self) {
        let mut buf = [0 as Tchar; 512];
        let buf_ptr = buf.as_mut_ptr().cast::<c_void>();
        self.run("status", "status", |c, r| {
            // SAFETY: `buf` is a zero-filled 512-character buffer, live for the call.
            unsafe { cache::status(c, buf_ptr, r) };
            Ok(Ret::Done)
        });
    }

    fn op_poke_unlock(&mut self, idx: i32) {
        self.run(&format!("poke unlock {idx}"), "poke", |c, _| {
            // SAFETY: scenarios only poke items below the allocated count.
            unsafe {
                let it = c.item_memory.add(idx as usize);
                (*it).cost = (*it).cost.wrapping_sub(COST_INFINITE);
            }
            Ok(Ret::Done)
        });
    }

    fn op_poke_data(&mut self, idx: i32, delta: i32) {
        self.run(&format!("poke data {idx} {delta}"), "poke", |c, _| {
            // SAFETY: as in `op_poke_unlock`.
            unsafe {
                let it = c.item_memory.add(idx as usize);
                (*it).data = (*it).data.wrapping_offset(delta as isize);
            }
            Ok(Ret::Done)
        });
    }

    fn op_poke_id(&mut self, idx: i32, id: u64) {
        self.run(&format!("poke id {idx} {id:016x}"), "poke", |c, _| {
            // SAFETY: as in `op_poke_unlock`.
            unsafe { (*c.item_memory.add(idx as usize)).id = id };
            Ok(Ret::Done)
        });
    }

    fn op_poke_unhash(&mut self, idx: i32) {
        self.run(&format!("poke unhash {idx}"), "poke", |c, _| {
            // SAFETY: as in `op_poke_unlock`; the bucket index is below `HASH_COUNT`, and the
            // link walk stops at NULL or at the item.
            unsafe {
                let item = c.item_memory.add(idx as usize);
                let mut link: *mut *mut FCacheItem =
                    c.hash_items.as_mut_ptr().add(ghash((*item).id));
                while !(*link).is_null() && *link != item {
                    link = &mut (**link).hash_next;
                }
                if !(*link).is_null() {
                    *link = (*item).hash_next;
                }
            }
            Ok(Ret::Done)
        });
    }

    fn op_poke_items_total(&mut self, n: i32) {
        self.run(&format!("poke items_total {n}"), "poke", |c, _| {
            c.items_total = n;
            Ok(Ret::Done)
        });
    }

    fn op_poke_initialized(&mut self, n: i32) {
        self.run(&format!("poke initialized {n}"), "poke", |c, _| {
            c.initialized = n;
            Ok(Ret::Done)
        });
    }

    fn op_poke_cache_items_null(&mut self) {
        self.run("poke cache_items_null", "poke", |c, _| {
            c.cache_items = null_mut();
            Ok(Ret::Done)
        });
    }

    fn op_poke_hash_push(&mut self, bucket: i32, idx: i32) {
        self.run(&format!("poke hash_push {bucket} {idx}"), "poke", |c, _| {
            // SAFETY: as in `op_poke_unlock`; the bucket index is below `HASH_COUNT`.
            unsafe {
                let item = c.item_memory.add(idx as usize);
                (*item).hash_next = c.hash_items[bucket as usize];
                c.hash_items[bucket as usize] = item;
            }
            Ok(Ret::Done)
        });
    }

    fn op_poke_dup_id(&mut self, idx: i32, src: i32) {
        self.run(&format!("poke dup_id {idx} {src}"), "poke", |c, _| {
            // SAFETY: as in `op_poke_unlock`; the unlink walk stops at NULL or at the item.
            unsafe {
                let item = c.item_memory.add(idx as usize);
                let mut link: *mut *mut FCacheItem =
                    c.hash_items.as_mut_ptr().add(ghash((*item).id));
                while !(*link).is_null() && *link != item {
                    link = &mut (**link).hash_next;
                }
                if !(*link).is_null() {
                    *link = (*item).hash_next;
                }
                (*item).id = (*c.item_memory.add(src as usize)).id;
                let b = ghash((*item).id);
                (*item).hash_next = c.hash_items[b];
                c.hash_items[b] = item;
            }
            Ok(Ret::Done)
        });
    }
}

/// Drives the same 52 scenarios as the harness `main`, in the same order.
fn run_all(h: &mut Replay) {
    // init
    h.scenario("init_basic");
    h.op_init(4096, 16, false, 0);
    h.op_exit(1);

    h.scenario("init_segments");
    h.op_init(8192, 32, false, 2048);
    h.op_exit(1);

    h.scenario("init_partial_segment");
    h.op_init(5000, 16, false, 2048);
    h.op_exit(1);

    h.scenario("init_start_given");
    h.op_init(4096, 16, true, 0);
    h.op_exit(0);

    h.scenario("init_cache_memory_fail");
    h.rec.fail_alloc.set(Some("CacheMemory"));
    h.op_init(4096, 16, false, 0);

    h.scenario("init_cache_items_fail");
    h.rec.fail_alloc.set(Some("CacheItems"));
    h.op_init(4096, 16, false, 0);

    h.scenario("init_negative_bytes");
    h.op_init(-16, 16, false, 0);

    h.scenario("init_twice");
    h.op_init(4096, 16, false, 0);
    h.op_init(4096, 16, false, 0);

    h.scenario("init_one_item");
    h.op_init(4096, 1, false, 0);

    // create
    h.scenario("create_basic");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 200, 16, 0);
    h.op_create(3, 300, 64, 8);

    h.scenario("create_pad_and_align");
    h.op_init(4096, 16, false, 0);
    h.op_create(0x10, 64, 256, 16);
    h.op_create(0x11, 64, 256, 16);

    h.scenario("create_bizarre_alignment");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 50, 512, 0);

    h.scenario("create_nofit");
    h.op_init(1024, 8, false, 0);
    h.op_create(1, 1000, 16, 0);
    h.op_create(2, 500, 16, 0);

    h.scenario("create_nofit_logf_fail");
    h.op_init(1024, 8, false, 0);
    h.op_create(1, 1000, 16, 0);
    h.rec.fail_logf.set(Some("glog"));
    h.op_create(2, 500, 16, 0);

    h.scenario("create_zero_id");
    h.op_init(4096, 16, false, 0);
    h.op_create(0, 100, 16, 0);

    h.scenario("create_zero_size");
    h.op_init(4096, 16, false, 0);
    h.op_create(5, 0, 16, 0);

    h.scenario("create_uninit");
    h.op_create(5, 100, 16, 0);

    h.scenario("create_evict");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 1000, 16, 0);
    h.op_create(2, 1000, 16, 0);
    h.op_poke_unlock(0);
    h.op_poke_unlock(2);
    h.op_create(3, 2000, 16, 0);

    h.scenario("create_segments");
    h.op_init(4096, 32, false, 1024);
    h.op_create(1, 900, 16, 0);
    h.op_create(2, 900, 16, 0);
    h.op_create(3, 1000, 16, 0);
    h.op_create(4, 1000, 16, 0);
    h.op_create(5, 1000, 16, 0);

    h.scenario("create_many");
    h.op_init(1024, 32, false, 0);
    for i in 1..=10u64 {
        h.op_create(0x100 + i, 90, 16, 0);
    }

    h.scenario("create_after_exit");
    h.op_init(4096, 16, false, 0);
    h.op_exit(0);
    h.op_create(1, 100, 16, 0);

    // flush
    h.scenario("flush_uninit");
    h.op_flush(1, u32::MAX, 0);

    h.scenario("flush_by_id");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 100, 16, 0);
    h.op_poke_unlock(0);
    h.op_flush(1, u32::MAX, 0);
    h.op_flush(99, u32::MAX, 0);

    h.scenario("flush_locked");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_flush(1, u32::MAX, 0);

    h.scenario("flush_ignore_locked");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_flush(1, u32::MAX, 1);

    h.scenario("flush_wildcard");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 100, 16, 0);
    h.op_create(3, 100, 16, 0);
    h.op_poke_unlock(0);
    h.op_poke_unlock(2);
    h.op_poke_unlock(4);
    h.op_flush(0, 0, 0);

    h.scenario("flush_wildcard_locked");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 100, 16, 0);
    h.op_flush(0, 0, 0);

    h.scenario("flush_partial_mask");
    h.op_init(4096, 16, false, 0);
    h.op_create(0x10001, 100, 16, 0);
    h.op_create(0x20001, 100, 16, 0);
    h.op_create(0x20002, 100, 16, 0);
    h.op_poke_unlock(0);
    h.op_poke_unlock(2);
    h.op_poke_unlock(4);
    h.op_flush(0x1, 0xFFFF, 0);

    h.scenario("flush_unhash_missing");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 100, 16, 0);
    h.op_poke_unhash(0);
    h.op_poke_unlock(0);
    h.op_flush(0, 0, 0);

    // check
    h.scenario("check_clean");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_check();

    h.scenario("check_data_mismatch");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_poke_data(0, 1);
    h.op_check();

    h.scenario("check_hash_removed");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_poke_unhash(0);
    h.op_check();

    h.scenario("check_ghash_mismatch");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_poke_id(0, 2);
    h.op_check();

    h.scenario("check_items_total");
    h.op_init(4096, 16, false, 0);
    h.op_poke_items_total(99);
    h.op_check();

    h.scenario("check_initialized2");
    h.op_init(4096, 16, false, 0);
    h.op_poke_initialized(2);
    h.op_check();

    h.scenario("check_cache_items_null");
    h.op_init(4096, 16, false, 0);
    h.op_poke_cache_items_null();
    h.op_check();

    h.scenario("check_hash_bucket_mismatch");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_poke_hash_push(5, 1);
    h.op_check();

    h.scenario("check_hash_duplicate_id");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 100, 16, 0);
    h.op_poke_dup_id(1, 0);
    h.op_check();

    h.scenario("check_hash_count_mismatch");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_poke_hash_push(0, 1);
    h.op_check();

    // tick
    h.scenario("tick_ageing");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 1000, 16, 0);
    h.op_tick();
    h.op_tick();
    h.op_tick();
    h.op_poke_unlock(0);
    h.op_tick();
    h.op_tick();

    h.scenario("status_after_tick");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 1000, 16, 0);
    h.op_tick();
    h.op_tick();
    h.op_status();

    h.scenario("status_grouping");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 1000, 16, 0);
    for _ in 0..15 {
        h.op_tick();
    }
    h.op_status();

    h.scenario("tick_uninit");
    h.op_tick();

    // exec / status
    h.scenario("exec_dumpcache_user");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_create(2, 200, 16, 0);
    h.op_tick();
    h.op_exec("DUMPCACHE", false);

    h.scenario("exec_unknown");
    h.op_init(4096, 16, false, 0);
    h.op_exec("FOO", false);
    h.op_exec("DUMPCACHEX", false);

    h.scenario("exec_logf_fail");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.rec.fail_logf.set(Some("user"));
    h.op_exec("DUMPCACHE", false);

    h.scenario("status_figures");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_status();
    h.op_status();

    h.scenario("status_uninit");
    h.op_status();

    // exit
    h.scenario("exit_no_free");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_exit(0);

    h.scenario("exit_free");
    h.op_init(4096, 16, false, 0);
    h.op_create(1, 100, 16, 0);
    h.op_exit(1);

    h.scenario("exit_uninit");
    h.op_exit(1);

    h.scenario("exit_after_corruption");
    h.op_init(4096, 16, false, 0);
    h.op_poke_items_total(99);
    h.op_exit(1);
}

#[test]
fn cache_replay_matches_fixture() {
    let mut h = Replay::new();
    run_all(&mut h);
    let out = h.rec.out.borrow().clone();

    assert_eq!(
        FIXTURE.lines().count(),
        EXPECTED_LINES,
        "fixture line count"
    );
    let count = |text: &str, prefix: &str| text.lines().filter(|l| l.starts_with(prefix)).count();
    assert_eq!(
        count(&out, "scenario "),
        EXPECTED_SCENARIOS,
        "scenario records"
    );
    assert_eq!(count(&out, "op "), EXPECTED_OPS, "op records");
    assert_eq!(count(&out, "unwind "), EXPECTED_UNWIND, "unwind records");
    assert_eq!(out.lines().count(), EXPECTED_LINES, "replay line count");
    assert_eq!(out, FIXTURE);
}
