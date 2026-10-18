#![allow(unsafe_op_in_unsafe_fn)]

//! Replays `fixtures/span_reference.txt` (captured by compiling the ORIGINAL
//! `UnSpan.cpp` standalone with clang++) against the Rust span port.
//!
//! Each `OP` block's `IN` sections are rebuilt into arena-backed span buffers,
//! the operation is run, and the result is re-emitted in the harness's text
//! format. The whole re-emitted text must equal the fixture line for line.

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;

use hp2_abi::render_span::{FMemMark, FRasterSpan, FSpan, FSpanBuffer};
use hp2_render::span::{
    SpanAlloc, alloc_index, alloc_index_for_screen, box_is_visible, copy_from_raster,
    copy_from_raster_update, copy_index_from, get_valid_range, merge_with,
};

const FIXTURE: &str = include_str!("fixtures/span_reference.txt");

#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct Chunk([u8; 16]);

/// Per-op arena: every push is its own zeroed, 16-aligned, never-moved box.
#[derive(Default)]
struct Arena {
    chunks: RefCell<Vec<Box<[Chunk]>>>,
    warnings: RefCell<Vec<String>>,
}

impl SpanAlloc for Arena {
    unsafe fn push(&self, _mem: *mut c_void, size: usize, align: usize) -> *mut u8 {
        assert!(align <= 16);
        let mut chunk = vec![Chunk([0; 16]); size.div_ceil(16).max(1)].into_boxed_slice();
        let p = chunk.as_mut_ptr() as *mut u8;
        self.chunks.borrow_mut().push(chunk);
        p
    }
    unsafe fn mark_save(&self, _mem: *mut c_void, mark: *mut FMemMark) {
        ptr::write(
            mark,
            FMemMark {
                mem: ptr::null_mut(),
                top: ptr::null_mut(),
                saved_chunk: ptr::null_mut(),
            },
        );
    }
    unsafe fn mark_pop(&self, _mark: *mut FMemMark) {}
    fn warn(&self, text: &str) {
        self.warnings.borrow_mut().push(text.to_owned());
    }
}

fn empty_buffer() -> FSpanBuffer {
    FSpanBuffer {
        start_y: 0,
        end_y: 0,
        valid_lines: 0,
        index: ptr::null_mut(),
        mem: ptr::null_mut(),
        mark: FMemMark {
            mem: ptr::null_mut(),
            top: ptr::null_mut(),
            saved_chunk: ptr::null_mut(),
        },
    }
}

struct Section<'a> {
    tok: Vec<&'a str>,
    data: Vec<&'a str>,
}

/// Split an op body into headed sections. `L` and `R` lines are data of the
/// preceding header; every other line starts a new section.
fn sections<'a>(body: &[&'a str]) -> Vec<Section<'a>> {
    let mut out: Vec<Section<'a>> = Vec::new();
    for &line in body {
        let tok: Vec<&str> = line.split_whitespace().collect();
        match tok.first() {
            Some(&"L") | Some(&"R") => out
                .last_mut()
                .expect("data line before any header")
                .data
                .push(line),
            _ => out.push(Section {
                tok,
                data: Vec::new(),
            }),
        }
    }
    out
}

fn find<'s, 'a>(secs: &'s [Section<'a>], name: &[&str]) -> &'s Section<'a> {
    secs.iter()
        .find(|s| s.tok.starts_with(name))
        .unwrap_or_else(|| panic!("missing section {name:?}"))
}

fn num(s: &str) -> i32 {
    s.parse().expect("integer")
}

/// `L c a0 b0 a1 b1 ...` -> spans of one line.
fn parse_spans(line: &str) -> Vec<(i32, i32)> {
    let vals: Vec<i32> = line.split_whitespace().skip(1).map(num).collect();
    let count = vals[0] as usize;
    let pairs = &vals[1..];
    assert_eq!(pairs.len(), count * 2, "bad L line: {line}");
    pairs.chunks(2).map(|c| (c[0], c[1])).collect()
}

/// `R x0 x1` -> raster span.
fn parse_raster(line: &str) -> FRasterSpan {
    let tok: Vec<&str> = line.split_whitespace().collect();
    FRasterSpan {
        x: [num(tok[1]), num(tok[2])],
    }
}

/// Build a linear buffer `[start_y, end_y)` whose lines are the given span lists.
unsafe fn build_from_lines(
    a: &Arena,
    b: &mut FSpanBuffer,
    start_y: i32,
    end_y: i32,
    lines: &[Vec<(i32, i32)>],
) {
    assert_eq!(
        lines.len(),
        (end_y - start_y) as usize,
        "line count vs header"
    );
    alloc_index(a, b, start_y, end_y, ptr::null_mut());
    for (i, spans) in lines.iter().enumerate() {
        let mut prev = b.index.add(i);
        for &(s, e) in spans {
            let node = a.push(ptr::null_mut(), size_of::<FSpan>(), 4) as *mut FSpan;
            (*node).start = s;
            (*node).end = e;
            *prev = node;
            prev = ptr::addr_of_mut!((*node).next);
            b.valid_lines += 1;
        }
        *prev = ptr::null_mut();
    }
}

/// Input section -> buffer. `start_tok`/`end_tok` index the header's `[sy, ey)` tokens.
unsafe fn load(a: &Arena, sec: &Section, start_tok: usize, end_tok: usize) -> FSpanBuffer {
    let mut b = empty_buffer();
    let lines: Vec<Vec<(i32, i32)>> = sec.data.iter().map(|l| parse_spans(l)).collect();
    build_from_lines(
        a,
        &mut b,
        num(sec.tok[start_tok]),
        num(sec.tok[end_tok]),
        &lines,
    );
    b
}

/// Harness `DumpBuffer`: header `tag sy ey vl` (or `-`) then one `L` line per row.
unsafe fn dump_buffer(out: &mut Vec<String>, tag: &str, b: &FSpanBuffer, with_valid: bool) {
    let vl = if with_valid {
        b.valid_lines.to_string()
    } else {
        "-".to_owned()
    };
    out.push(format!("{tag} {} {} {vl}", b.start_y, b.end_y));
    for i in 0..(b.end_y - b.start_y) as usize {
        let mut n = 0;
        let mut body = String::new();
        let mut span = *b.index.add(i);
        while !span.is_null() {
            n += 1;
            body.push_str(&format!(" {} {}", (*span).start, (*span).end));
            span = (*span).next;
        }
        out.push(format!("L {n}{body}"));
    }
}

fn dump_range(out: &mut Vec<String>, a: i16, b: i16) {
    out.push(format!("RANGE {a} {b}"));
}

/// Update / copyraster: screen + raster in, dest + screen out.
unsafe fn replay_raster(a: &Arena, name: &str, secs: &[Section], out: &mut Vec<String>) {
    // `IN screen 0 ey vl`: start token 2, end token 3.
    let mut screen = load(a, find(secs, &["IN", "screen"]), 2, 3);
    dump_buffer(out, "IN screen", &screen, true);

    let raster_sec = find(secs, &["IN", "raster"]);
    let rs = num(raster_sec.tok[2]);
    let re = num(raster_sec.tok[3]);
    let raster: Vec<FRasterSpan> = raster_sec.data.iter().map(|l| parse_raster(l)).collect();
    out.push(format!("IN raster {rs} {re}"));
    for r in &raster {
        out.push(format!("R {} {}", r.x[0], r.x[1]));
    }

    let mut dest = empty_buffer();
    alloc_index(a, &mut dest, rs, re, ptr::null_mut());
    let ret = if name == "update" {
        copy_from_raster_update(a, &mut dest, &mut screen, rs, re, raster.as_ptr())
    } else {
        copy_from_raster(a, &mut dest, &screen, rs, re, raster.as_ptr())
    };
    out.push(format!("RET {ret}"));

    dump_buffer(out, "OUT dest", &dest, true);
    dump_buffer(out, "OUT screen", &screen, true);
    if name == "update" {
        let (s0, s1) = get_valid_range(&screen);
        dump_range(out, s0, s1);
    }
    let (d0, d1) = get_valid_range(&dest);
    dump_range(out, d0, d1);
}

unsafe fn replay_box(a: &Arena, secs: &[Section], out: &mut Vec<String>) {
    let screen = load(a, find(secs, &["IN", "screen"]), 2, 3);
    dump_buffer(out, "IN screen", &screen, true);

    let bx = find(secs, &["BOX"]);
    let (x1, y1, x2, y2) = (
        num(bx.tok[1]),
        num(bx.tok[2]),
        num(bx.tok[3]),
        num(bx.tok[4]),
    );
    out.push(format!("BOX {x1} {y1} {x2} {y2}"));
    let ret = box_is_visible(&screen, x1, y1, x2, y2);
    out.push(format!("RET {ret}"));
}

unsafe fn replay_merge(a: &Arena, secs: &[Section], out: &mut Vec<String>) {
    let mut this = load(a, find(secs, &["IN", "this"]), 2, 3);
    dump_buffer(out, "IN this", &this, true);
    let other = load(a, find(secs, &["IN", "other"]), 2, 3);
    dump_buffer(out, "IN other", &other, true);

    merge_with(a, &mut this, &other);
    dump_buffer(out, "OUT this", &this, true);
    let (r0, r1) = get_valid_range(&this);
    dump_range(out, r0, r1);
}

unsafe fn replay_copyindex(a: &Arena, secs: &[Section], out: &mut Vec<String>) {
    let source = load(a, find(secs, &["IN", "source"]), 2, 3);
    dump_buffer(out, "IN source", &source, true);

    let mut this = empty_buffer();
    copy_index_from(a, &mut this, &source, ptr::null_mut());
    dump_buffer(out, "OUT this", &this, false);
}

unsafe fn replay_block(name: &str, body: &[&str]) -> Vec<String> {
    let a = Arena::default();
    let secs = sections(body);
    let mut out = vec![format!("OP {name}")];
    match name {
        "update" | "copyraster" => replay_raster(&a, name, &secs, &mut out),
        "box" => replay_box(&a, &secs, &mut out),
        "merge" => replay_merge(&a, &secs, &mut out),
        "copyindex" => replay_copyindex(&a, &secs, &mut out),
        other => panic!("unknown op {other}"),
    }
    out
}

/// `AllocIndexForScreen` has no C++ reference case in the fixture; this pins the
/// rows it produces: `SYR` full-width `[0, SXR)` lines and `ValidLines == SYR`.
#[test]
fn alloc_index_for_screen_builds_full_width_lines() {
    let a = Arena::default();
    let mut b = empty_buffer();
    unsafe {
        alloc_index_for_screen(&a, &mut b, 9, 3, ptr::null_mut());
        assert_eq!((b.start_y, b.end_y, b.valid_lines), (0, 3, 3));
        for i in 0..3 {
            let span = *b.index.add(i);
            assert_eq!(
                ((*span).start, (*span).end, (*span).next.is_null()),
                (0, 9, true)
            );
        }
    }
}

#[test]
fn replays_original_unspan_cpp_through_rust_port() {
    let lines: Vec<&str> = FIXTURE.lines().collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("OP "))
        .map(|(i, _)| i)
        .collect();
    // 150 cases x (update, copyraster, 4 box, merge, copyindex).
    assert_eq!(starts.len(), 150 * 8, "fixture op count");

    let mut actual: Vec<String> = Vec::with_capacity(lines.len());
    for (k, &s) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(lines.len());
        let name = lines[s].strip_prefix("OP ").expect("OP line");
        actual.extend(unsafe { replay_block(name, &lines[s + 1..end]) });
    }

    assert_eq!(actual.len(), lines.len(), "line count differs from fixture");
    for (idx, (got, want)) in actual.iter().zip(lines.iter()).enumerate() {
        assert_eq!(got, want, "first mismatch at fixture line {}", idx + 1);
    }
}
