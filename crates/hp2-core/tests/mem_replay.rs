//! Bit-exact replay of the `UnMem.cpp` oracle fixture (`Build/mem_fixture_harness/gen_mem.sh`).
//!
//! The test runs the same operation sequence as the harness driver (`mem_harness.cpp`),
//! drives the Rust port with recording services in place of the C++ stand-ins, and
//! re-encodes each case and its hook lines in the harness format. The emitted text must
//! equal the fixture line for line. Record counts are asserted exactly so a silently
//! shrunken fixture fails the test.
//!
//! Two notes on the mirror:
//! - `UnusedChunks` is a private static in `mem.rs`, so it cannot be read from here. The
//!   test keeps a shadow of the unused list, updated only from the port's observable
//!   transitions (the order `free_chunks` releases chunks in, the first-fit reuse choice,
//!   and `exit` clearing the list). Each first-fit choice is asserted against the chunk the
//!   port actually returned, and the frees `exit` performs are asserted by the hook text.
//! - Chunk labels mirror the driver's registry: `chunk#N` counts successful mallocs, and a
//!   freed header stops matching so a reused address gets its new label.
//!
//! Compiled on macOS only, like the fixture's recording host.
#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, c_void};
use std::mem::{offset_of, zeroed};
use std::ptr::null_mut;

use hp2_abi::engine::generated::{FMemStack, FTaggedMemory};
use hp2_core::ansi::Tchar;
use hp2_core::mem::{self, FRAME_INIT_STACK_UNDER_EXIT, FRAME_OWN, MemServices, wide_z};

const FIXTURE: &str = include_str!("fixtures/mem_unmem.txt");
const EXPECTED_CASES: usize = 31;
const EXPECTED_HOOKS: usize = 24;

/// Offset of `FTaggedMemory::data`, as the harness's `offsetof`.
const DATA_OFFSET: usize = offset_of!(FTaggedMemory, data);

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

/// Mirrors the harness `Narrow`: copies characters up to `limit - 1`, stopping at NUL.
///
/// # Safety
/// `p` must point to a NUL-terminated `Tchar` string, or to at least `limit` readable
/// `Tchar`s.
unsafe fn narrow(p: *const Tchar, limit: usize) -> String {
    let mut s = String::new();
    let mut k = 0;
    while k + 1 < limit {
        // SAFETY: the caller guarantees a readable NUL-terminated string.
        let c = unsafe { *p.add(k) };
        if c == 0 {
            break;
        }
        s.push((c as u8) as char);
        k += 1;
    }
    s
}

/// Narrows a stack's tag pointer (`TCHAR` string) to at most 63 characters.
fn narrow_tag(p: *const c_void) -> String {
    if p.is_null() {
        return "null".into();
    }
    // SAFETY: tags are NUL-terminated `TCHAR` strings set by `init`.
    unsafe { narrow(p.cast(), 64) }
}

/// Unwind names for each failing op and frame code, in the order the C++ guards unwind.
fn unwind_names(op: &str, frame: i32) -> &'static [&'static str] {
    match (op, frame) {
        ("tick", FRAME_OWN) => &["FMemStack::InitStack"],
        ("exit", FRAME_INIT_STACK_UNDER_EXIT) => &["FMemStack::InitStack", "FMemStack::Exit"],
        ("get_byte_count", FRAME_OWN) => &["FMemStack::GetByteCount"],
        ("allocate_new_chunk", FRAME_OWN) => &["FMemStack::AllocateNewChunk"],
        _ => panic!("no unwind mapping for {op} frame {frame}"),
    }
}

struct Entry {
    ptr: usize,
    base: usize,
    span: usize,
    label: String,
}

/// Recording services in place of `Hp2MemServices` / the harness stand-ins.
struct Recorder {
    /// `true` is `mode=raise`: a failing callback reports non-zero.
    raise: Cell<bool>,
    /// `malloc=fail`: the next `alloc` reports an allocator error.
    malloc_fail: Cell<bool>,
    next_chunk: Cell<usize>,
    chunks: RefCell<Vec<Entry>>,
    hooks: RefCell<Vec<String>>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            raise: Cell::new(true),
            malloc_fail: Cell::new(false),
            next_chunk: Cell::new(0),
            chunks: RefCell::new(Vec::new()),
            hooks: RefCell::new(Vec::new()),
        }
    }

    fn hook(&self, line: String) {
        self.hooks.borrow_mut().push(line);
    }

    fn take_hooks(&self) -> Vec<String> {
        std::mem::take(&mut *self.hooks.borrow_mut())
    }

    fn register(&self, ptr: *const c_void, bytes: usize, label: String) {
        let base = ptr as usize + DATA_OFFSET;
        let span = bytes.wrapping_sub(DATA_OFFSET);
        self.chunks.borrow_mut().push(Entry {
            ptr: ptr as usize,
            base,
            span,
            label,
        });
    }

    fn retire(&self, ptr: *const c_void) {
        for e in self.chunks.borrow_mut().iter_mut() {
            if e.ptr == ptr as usize {
                e.ptr = 0;
                e.base = 0;
                e.span = 0;
            }
        }
    }

    /// Header pointer label: `null`, the registered label, or `unknown`.
    fn chunk_label(&self, p: *const c_void) -> String {
        if p.is_null() {
            return "null".into();
        }
        let p = p as usize;
        self.chunks
            .borrow()
            .iter()
            .find(|e| e.ptr == p)
            .map_or_else(|| "unknown".into(), |e| e.label.clone())
    }

    /// Data pointer label: `label+offset` inside a registered chunk.
    fn data_label(&self, p: *const u8) -> String {
        if p.is_null() {
            return "null".into();
        }
        let a = p as usize;
        for e in self.chunks.borrow().iter() {
            if a >= e.base && a <= e.base + e.span {
                return format!("{}+{}", e.label, a - e.base);
            }
        }
        "unknown".into()
    }

    /// `>`-joined header labels along `Next`, at most 16; `-` when empty.
    fn chain(&self, first: *mut FTaggedMemory) -> String {
        let mut parts = Vec::new();
        let mut c = first;
        while !c.is_null() && parts.len() < 16 {
            parts.push(self.chunk_label(c as *const c_void));
            // SAFETY: `c` is a live header on the chain.
            c = unsafe { (*c).next };
        }
        if parts.is_empty() {
            "-".into()
        } else {
            parts.join(">")
        }
    }
}

impl MemServices for Recorder {
    unsafe fn alloc(&self, size: u32, tag: *const c_void) -> Result<*mut c_void, i32> {
        let tag_text = narrow_tag(tag);
        if self.malloc_fail.get() {
            self.hook(format!("malloc|size={size}|tag={tag_text}|result=fail"));
            return Err(1);
        }
        // SAFETY: plain libc allocation, as the oracle's appMalloc uses.
        let p = unsafe { malloc(size as usize) };
        let n = self.next_chunk.get() + 1;
        self.next_chunk.set(n);
        let label = format!("chunk#{n}");
        self.register(p, size as usize, label.clone());
        self.hook(format!("malloc|size={size}|tag={tag_text}|result={label}"));
        Ok(p)
    }

    unsafe fn release(&self, ptr: *mut c_void) {
        let label = self.chunk_label(ptr);
        self.hook(format!("free|ptr={label}"));
        self.retire(ptr);
        // SAFETY: `ptr` came from `alloc`.
        unsafe { free(ptr) };
    }

    fn raise_error(&self, msg: &[Tchar]) -> i32 {
        // SAFETY: `wide_z` output is NUL-terminated.
        let text = unsafe { narrow(msg.as_ptr(), 256) };
        self.hook(format!("errorf|msg={text}"));
        self.raise.get() as i32
    }

    fn fail_assert(&self, expr: &CStr, file: &CStr, line: i32) -> i32 {
        let file = file.to_str().unwrap();
        let base = file.rsplit('/').next().unwrap();
        let expr = expr.to_str().unwrap();
        self.hook(format!("fail_assert|expr={expr}|file={base}|line={line}"));
        self.raise.get() as i32
    }
}

struct Replay {
    rec: Recorder,
    out: String,
    cases: usize,
    /// Shadow of `UnusedChunks`, head first (see the module note).
    unused: Vec<*mut FTaggedMemory>,
}

impl Replay {
    fn new() -> Self {
        Self {
            rec: Recorder::new(),
            out: String::new(),
            cases: 0,
            unused: Vec::new(),
        }
    }

    /// Adds `unwind|` hook lines for a failing op. Returns whether it threw.
    fn settle(&self, op: &str, frame: Option<i32>) -> bool {
        let Some(frame) = frame else {
            return false;
        };
        for name in unwind_names(op, frame) {
            self.rec.hook(format!("unwind|{name}"));
        }
        true
    }

    fn emit(&mut self, op: &str, stack: &str, args: &str, st: &FMemStack, ret: &str, threw: bool) {
        self.cases += 1;
        let ret = if threw { "none" } else { ret };
        let top = self.rec.data_label(st.top);
        let end_off = if st.top.is_null() || st.end.is_null() {
            "na".to_string()
        } else {
            (st.end as isize - st.top as isize).to_string()
        };
        let tag = narrow_tag(st.tag.cast());
        let top_chunk = self.rec.chunk_label(st.top_chunk as *const c_void);
        let chain = self.rec.chain(st.top_chunk);
        let unused = if self.unused.is_empty() {
            "-".to_string()
        } else {
            self.unused
                .iter()
                .map(|&c| self.rec.chunk_label(c as *const c_void))
                .collect::<Vec<_>>()
                .join(">")
        };
        let mode = if self.rec.raise.get() {
            "raise"
        } else {
            "return"
        };
        let malloc = if self.rec.malloc_fail.get() {
            "fail"
        } else {
            "ok"
        };
        self.out.push_str(&format!(
            "case|{}|{op}|stack={stack}|{args}|mode={mode}|malloc={malloc}|ret={ret}|threw={}|top={top}|end_off={end_off}|default={}|tag={tag}|top_chunk={top_chunk}|chain={chain}|unused={unused}\n",
            self.cases,
            threw as u8,
            st.default_chunk_size,
        ));
        for line in self.rec.take_hooks() {
            self.out.push_str(&format!("hook|{line}\n"));
        }
    }

    fn init_case(
        &mut self,
        name: &str,
        st: &mut FMemStack,
        default: i32,
        tag_text: &str,
        tag: *const c_void,
    ) {
        mem::init(st, default, tag);
        self.emit(
            "init",
            name,
            &format!("arg_default={default}|arg_tag={tag_text}"),
            st,
            "void",
            false,
        );
    }

    fn tick_case(&mut self, name: &str, st: &FMemStack) {
        let file = c"UnMem.cpp";
        // SAFETY: `file` is a NUL-terminated literal that outlives the call.
        let r = unsafe { mem::tick(st, &self.rec, file.as_ptr()) };
        let threw = self.settle("tick", r.err());
        self.emit("tick", name, "-", st, "void", threw);
    }

    fn exit_case(&mut self, name: &str, st: &FMemStack) {
        let file = c"UnMem.cpp";
        // SAFETY: `file` is a NUL-terminated literal; every unused chunk came from `alloc`.
        let r = unsafe { mem::exit(st, &self.rec, file.as_ptr()) };
        let threw = self.settle("exit", r.err());
        if !threw {
            self.unused.clear();
        }
        self.emit("exit", name, "-", st, "void", threw);
    }

    fn count_case(&mut self, name: &str, st: &FMemStack, args: &str) {
        // SAFETY: the chain is the `TopChunk` list built by the fixture or by `alloc`.
        let r = unsafe { mem::get_byte_count(st, &self.rec) };
        let threw = self.settle("get_byte_count", r.as_ref().err().copied());
        let ret = r.map_or_else(|_| String::new(), |v| v.to_string());
        self.emit("get_byte_count", name, args, st, &ret, threw);
    }

    fn alloc_case(&mut self, name: &str, st: &mut FMemStack, min: i32) {
        // First-fit choice the port must make: the first unused chunk with DataSize >= min.
        let chosen = self
            .unused
            .iter()
            .position(|&c| unsafe { (*c).data_size } >= min);
        // SAFETY: `st` is a live stack; the unused list holds only chunks from `alloc`.
        let r = unsafe { mem::allocate_new_chunk(st, min, &self.rec) };
        let threw = self.settle("allocate_new_chunk", r.as_ref().err().copied());
        let ret = match r {
            Ok(p) => self.rec.data_label(p),
            Err(_) => String::new(),
        };
        if let Some(i) = chosen
            && !threw
        {
            let expect = self.unused.remove(i);
            assert_eq!(
                st.top_chunk, expect,
                "first-fit reuse picked a different chunk"
            );
        }
        self.emit(
            "allocate_new_chunk",
            name,
            &format!("min={min}"),
            st,
            &ret,
            threw,
        );
    }

    fn free_case(&mut self, name: &str, st: &mut FMemStack, newtop: *mut FTaggedMemory) {
        let label = self.rec.chunk_label(newtop as *const c_void);
        // Chunks above `newtop` move to the unused list, each to the head.
        let mut c = st.top_chunk;
        while !c.is_null() && c != newtop {
            self.unused.insert(0, c);
            // SAFETY: `c` is a live header on the chain.
            c = unsafe { (*c).next };
        }
        // SAFETY: `newtop` is null or on the current chain (fixture contract).
        unsafe { mem::free_chunks(st, newtop) };
        self.emit(
            "free_chunks",
            name,
            &format!("newtop={label}"),
            st,
            "void",
            false,
        );
    }
}

/// Header-sized buffer aligned like a chunk; fake chunks for `GetByteCount`.
#[repr(C, align(16))]
struct FakeBuf([u8; 512]);

/// Top chunk (7 bytes used, DataSize 50) over `f1` (DataSize `f1_size`), as the harness.
fn setup_fake_chain(st: &mut FMemStack, f1: &mut FakeBuf, f2: &mut FakeBuf, f1_size: i32) {
    let h1 = f1.0.as_mut_ptr().cast::<FTaggedMemory>();
    let h2 = f2.0.as_mut_ptr().cast::<FTaggedMemory>();
    // SAFETY: both buffers are 512 bytes, 16-aligned, and hold the header and data.
    unsafe {
        (*h1).next = null_mut();
        (*h1).data_size = f1_size;
        (*h2).next = h1;
        (*h2).data_size = 50;
        let data2 = f2.0.as_mut_ptr().add(DATA_OFFSET);
        st.top_chunk = h2;
        st.top = data2.add(7);
        st.end = data2.add(50);
    }
}

#[test]
fn mem_unmem_replay_matches_fixture() {
    let mut h = Replay::new();
    let mem_tag = wide_z("MemTest");
    let fake_tag = wide_z("FakeTag");

    let mut f1 = Box::new(FakeBuf([0; 512]));
    let mut f2 = Box::new(FakeBuf([0; 512]));
    h.rec.register(f1.0.as_ptr().cast(), 512, "fake#1".into());
    h.rec.register(f2.0.as_ptr().cast(), 512, "fake#2".into());

    // SAFETY: `FMemStack` is plain old data; zero is a valid initial state before `init`.
    let mut s1: FMemStack = unsafe { zeroed() };
    let mut s2: FMemStack = unsafe { zeroed() };
    let mut s3: FMemStack = unsafe { zeroed() };
    let next_of_top = |s: &FMemStack| unsafe { (*s.top_chunk).next };
    let next2_of_top = |s: &FMemStack| unsafe { (*(*s.top_chunk).next).next };

    // Stack S1: init, reuse, malloc failure, Exit raise/return, release-list order.
    h.init_case("S1", &mut s1, 4096, "MemTest", mem_tag.as_ptr().cast());
    h.tick_case("S1", &s1);
    h.count_case("S1", &s1, "-");
    h.alloc_case("S1", &mut s1, 100);
    h.alloc_case("S1", &mut s1, 10);
    let n = next_of_top(&s1);
    h.free_case("S1", &mut s1, n);
    h.alloc_case("S1", &mut s1, 10);
    h.rec.malloc_fail.set(true);
    h.alloc_case("S1", &mut s1, 100);
    h.rec.malloc_fail.set(false);
    h.tick_case("S1", &s1);
    h.rec.raise.set(false);
    h.tick_case("S1", &s1);
    h.rec.raise.set(true);
    h.alloc_case("S1", &mut s1, 10);
    let n = next_of_top(&s1);
    h.free_case("S1", &mut s1, n);
    h.exit_case("S1", &s1);
    h.rec.raise.set(false);
    h.exit_case("S1", &s1);
    h.rec.raise.set(true);
    h.free_case("S1", &mut s1, null_mut());
    h.alloc_case("S1", &mut s1, 10);
    h.alloc_case("S1", &mut s1, 10);
    h.alloc_case("S1", &mut s1, 10);
    let n = next2_of_top(&s1);
    h.free_case("S1", &mut s1, n);
    h.free_case("S1", &mut s1, null_mut());
    h.exit_case("S1", &s1);

    // Stack S2: GetByteCount over a fake chain, including the 32-bit contract error.
    h.init_case("S2", &mut s2, 4096, "FakeTag", fake_tag.as_ptr().cast());
    setup_fake_chain(&mut s2, &mut f1, &mut f2, 100);
    h.count_case("S2", &s2, "f1_size=100");
    setup_fake_chain(&mut s2, &mut f1, &mut f2, i32::MAX);
    h.count_case("S2", &s2, "f1_size=2147483647");
    h.rec.raise.set(false);
    setup_fake_chain(&mut s2, &mut f1, &mut f2, i32::MAX);
    h.count_case("S2", &s2, "f1_size=2147483647");
    h.rec.raise.set(true);

    // Stack S3: negative MinSize wraps DataSize to -4; first-fit skips that undersized unused chunk.
    h.init_case("S3", &mut s3, 12, "MemTest", mem_tag.as_ptr().cast());
    h.alloc_case("S3", &mut s3, -100);
    h.free_case("S3", &mut s3, null_mut());
    h.alloc_case("S3", &mut s3, 10);
    h.free_case("S3", &mut s3, null_mut());
    h.exit_case("S3", &s3);

    let hooks = h.out.lines().filter(|l| l.starts_with("hook|")).count();
    assert_eq!(h.cases, EXPECTED_CASES, "case record count");
    assert_eq!(hooks, EXPECTED_HOOKS, "hook record count");
    assert_eq!(h.out, FIXTURE, "replay output differs from fixture");
}
