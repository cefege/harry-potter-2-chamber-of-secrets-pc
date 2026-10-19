//! Replays the `persist.*` scenarios in `tests/fixtures/level_oracle.txt` against
//! the Rust port of `ULevel::SavePersistentActors` / `LoadPersistentActors`.
//!
//! The services are mocks that emit the same trace records as the C++ oracle
//! (`archive_serialize_bytes\t<len>\t<W|R>` for every int, sqword, and wire unit).
//! Each scenario's trace must match exactly, and its return value and saved byte
//! count must match too. Non-`persist.*` scenarios (`core.*`, `net.*`) are skipped.

use std::ffi::c_void;
use std::sync::Mutex;

use hp2_engine::level_persist::{self, Hp2LevelPersistServices};

const FIXTURE: &str = include_str!("fixtures/level_oracle.txt");

#[derive(Clone)]
struct ActorRec {
    idx: i32,
    name: Vec<u32>,
    persistent: bool,
}

struct Ar {
    writer: bool,
    buf: Vec<u8>,
    pos: usize,
}

#[derive(Default)]
struct Mock {
    trace: Vec<String>,
    /// Actor handles are `index + 1`; 0 is null.
    actors: Vec<ActorRec>,
    /// Level actor handles, in level order.
    level: Vec<usize>,
    /// TMultiMap handles are `index + 1`; entries are `(name, actor handle)`.
    maps: Vec<Vec<(Vec<u32>, usize)>>,
    /// Iterator handles are `index + 1`; entries are `(map index, position)`.
    iters: Vec<(usize, usize)>,
    /// Archive handles are `index + 1`.
    ars: Vec<Ar>,
    pa_in: Vec<u8>,
    file_size: i32,
    global_time: i64,
    reader_missing: bool,
    last_close_size: Option<usize>,
}

static MOCK: Mutex<Option<Mock>> = Mutex::new(None);

fn install(m: Mock) {
    *MOCK.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
}

fn take() -> Mock {
    MOCK.lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .expect("mock installed")
}

fn with<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    let mut g = MOCK.lock().unwrap_or_else(|e| e.into_inner());
    f(g.as_mut().expect("mock installed"))
}

fn rec(m: &mut Mock, line: String) {
    m.trace.push(line);
}

/// Reads a NUL-terminated `TCHAR` string.
unsafe fn read_z(p: *const u32) -> Vec<u32> {
    let mut v = Vec::new();
    let mut i = 0usize;
    loop {
        let c = unsafe { *p.add(i) };
        if c == 0 {
            return v;
        }
        v.push(c);
        i += 1;
    }
}

fn to_s(v: &[u32]) -> String {
    v.iter().map(|&c| char::from_u32(c).unwrap()).collect()
}

fn t(s: &str) -> Vec<u32> {
    s.chars().map(|c| c as u32).collect()
}

/// Copy-out with `(out, cap)`: returns the full length including the NUL.
unsafe fn put(out: *mut u32, cap: i32, s: &[u32]) -> i32 {
    let n = s.len() as i32 + 1;
    if n <= cap {
        for (i, &c) in s.iter().enumerate() {
            unsafe { *out.add(i) = c };
        }
        unsafe { *out.add(s.len()) = 0 };
    }
    n
}

unsafe extern "C" fn is_editor() -> i32 {
    0
}

unsafe extern "C" fn gsys_map_path(out: *mut u32, cap: i32) -> i32 {
    unsafe { put(out, cap, &t("Maps\\")) }
}

unsafe extern "C" fn gsys_save_slot_path(out: *mut u32, cap: i32) -> i32 {
    unsafe { put(out, cap, &t("Save\\")) }
}

unsafe extern "C" fn url_default_map_ext(out: *mut u32, cap: i32) -> i32 {
    unsafe { put(out, cap, &t("unr")) }
}

unsafe extern "C" fn url_default_save_ext(out: *mut u32, cap: i32) -> i32 {
    unsafe { put(out, cap, &t("usa")) }
}

unsafe extern "C" fn app_path_leaf(path: *const u32, out: *mut u32, cap: i32) -> i32 {
    let p = unsafe { read_z(path) };
    let slash = '/' as u32;
    let start = p.iter().rposition(|&c| c == slash).map_or(0, |i| i + 1);
    unsafe { put(out, cap, &p[start..]) }
}

unsafe extern "C" fn fstring_in_str(s: *const u32, sub: *const u32) -> i32 {
    let (s, sub) = unsafe { (read_z(s), read_z(sub)) };
    s.windows(sub.len().max(1))
        .position(|w| w == sub.as_slice())
        .map_or(-1, |i| i as i32)
}

unsafe extern "C" fn fstring_left(s: *const u32, n: i32, out: *mut u32, cap: i32) -> i32 {
    let s = unsafe { read_z(s) };
    let k = (n.max(0) as usize).min(s.len());
    unsafe { put(out, cap, &s[..k]) }
}

unsafe extern "C" fn fstring_path_join(
    a: *const u32,
    b: *const u32,
    out: *mut u32,
    cap: i32,
) -> i32 {
    let (mut r, b) = unsafe { (read_z(a), read_z(b)) };
    if !r.is_empty() && r.last() != Some(&('/' as u32)) {
        r.push('/' as u32);
    }
    r.extend(b);
    unsafe { put(out, cap, &r) }
}

unsafe extern "C" fn fstring_append(a: *const u32, b: *const u32, out: *mut u32, cap: i32) -> i32 {
    let (mut r, b) = unsafe { (read_z(a), read_z(b)) };
    r.extend(b);
    unsafe { put(out, cap, &r) }
}

unsafe extern "C" fn fstring_ne(a: *const u32, b: *const u32) -> i32 {
    let (a, b) = unsafe { (read_z(a), read_z(b)) };
    (a != b) as i32
}

unsafe extern "C" fn app_strcmp(a: *const u32, b: *const u32) -> i32 {
    let (a, b) = unsafe { (read_z(a), read_z(b)) };
    a.cmp(&b) as i32
}

unsafe extern "C" fn level_actors_num(_level: *mut c_void) -> i32 {
    with(|m| m.level.len() as i32)
}

unsafe extern "C" fn level_actor_at(_level: *mut c_void, i: i32) -> *mut c_void {
    with(|m| m.level[i as usize] as *mut c_void)
}

unsafe extern "C" fn actor_b_persistent(actor: *mut c_void) -> i32 {
    with(|m| m.actors[actor as usize - 1].persistent as i32)
}

unsafe extern "C" fn actor_name(actor: *mut c_void, out: *mut u32, cap: i32) -> i32 {
    with(|m| {
        let name = m.actors[actor as usize - 1].name.clone();
        unsafe { put(out, cap, &name) }
    })
}

unsafe extern "C" fn actor_set_b_script_initialized(_actor: *mut c_void, _v: i32) {}

unsafe extern "C" fn load_map_new() -> *mut c_void {
    with(|m| {
        m.maps.push(Vec::new());
        m.maps.len() as *mut c_void
    })
}

unsafe extern "C" fn load_map_delete(_map: *mut c_void) {}

unsafe extern "C" fn load_map_add(map: *mut c_void, name: *const u32, actor: *mut c_void) {
    let name = unsafe { read_z(name) };
    with(|m| m.maps[map as usize - 1].push((name, actor as usize)));
}

unsafe extern "C" fn load_map_find(map: *mut c_void, name: *const u32) -> *mut c_void {
    let name = unsafe { read_z(name) };
    with(|m| {
        m.maps[map as usize - 1]
            .iter()
            .find(|(k, _)| *k == name)
            .map_or(std::ptr::null_mut(), |(_, a)| *a as *mut c_void)
    })
}

unsafe extern "C" fn load_map_remove(map: *mut c_void, name: *const u32) {
    let name = unsafe { read_z(name) };
    with(|m| m.maps[map as usize - 1].retain(|(k, _)| *k != name));
}

unsafe extern "C" fn load_map_iter_begin(map: *mut c_void) -> *mut c_void {
    with(|m| {
        m.iters.push((map as usize - 1, 0));
        m.iters.len() as *mut c_void
    })
}

unsafe extern "C" fn load_map_iter_valid(it: *mut c_void) -> i32 {
    with(|m| {
        let (mi, pos) = m.iters[it as usize - 1];
        (pos < m.maps[mi].len()) as i32
    })
}

unsafe extern "C" fn load_map_iter_value(it: *mut c_void) -> *mut c_void {
    with(|m| {
        let (mi, pos) = m.iters[it as usize - 1];
        m.maps[mi][pos].1 as *mut c_void
    })
}

unsafe extern "C" fn load_map_iter_next(it: *mut c_void) {
    with(|m| m.iters[it as usize - 1].1 += 1);
}

unsafe extern "C" fn load_map_iter_end(_it: *mut c_void) {}

unsafe extern "C" fn archive_writer_create(path: *const u32) -> *mut c_void {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("archive_writer_create\t{path}"));
        m.ars.push(Ar {
            writer: true,
            buf: Vec::new(),
            pos: 0,
        });
        m.ars.len() as *mut c_void
    })
}

unsafe extern "C" fn archive_reader_create(path: *const u32) -> *mut c_void {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("archive_reader_create\t{path}"));
        if m.reader_missing {
            return std::ptr::null_mut();
        }
        let buf = m.pa_in.clone();
        m.ars.push(Ar {
            writer: false,
            buf,
            pos: 0,
        });
        m.ars.len() as *mut c_void
    })
}

unsafe extern "C" fn archive_close(ar: *mut c_void) {
    with(|m| {
        let (writer, size) = {
            let a = &m.ars[ar as usize - 1];
            (a.writer, a.buf.len())
        };
        if writer {
            rec(m, format!("archive_close\t{size}"));
            m.last_close_size = Some(size);
        }
    });
}

unsafe extern "C" fn archive_tell(ar: *mut c_void) -> i32 {
    with(|m| m.ars[ar as usize - 1].pos as i32)
}

unsafe extern "C" fn archive_seek(ar: *mut c_void, pos: i32) {
    with(|m| {
        rec(m, format!("archive_seek\t{pos}"));
        m.ars[ar as usize - 1].pos = pos as usize;
    });
}

unsafe extern "C" fn archive_total_size(ar: *mut c_void) -> i32 {
    with(|m| m.ars[ar as usize - 1].buf.len() as i32)
}

unsafe extern "C" fn archive_get_error(_ar: *mut c_void) -> i32 {
    0
}

/// Moves `len` bytes between the archive and `data`, recording one
/// `archive_serialize_bytes` line. Returns `true` for a writer.
unsafe fn serialize(ar: *mut c_void, data: *mut u8, len: usize) -> bool {
    with(|m| {
        let a = &mut m.ars[ar as usize - 1];
        let writer = a.writer;
        let dir = if writer { "W" } else { "R" };
        if writer {
            if a.buf.len() < a.pos + len {
                a.buf.resize(a.pos + len, 0);
            }
            let src = unsafe { std::slice::from_raw_parts(data, len) };
            a.buf[a.pos..a.pos + len].copy_from_slice(src);
        } else {
            let dst = unsafe { std::slice::from_raw_parts_mut(data, len) };
            for (i, d) in dst.iter_mut().enumerate() {
                *d = a.buf.get(a.pos + i).copied().unwrap_or(0);
            }
        }
        a.pos += len;
        rec(m, format!("archive_serialize_bytes\t{len}\t{dir}"));
        writer
    })
}

unsafe extern "C" fn archive_serialize_bytes(ar: *mut c_void, data: *mut c_void, len: i32) {
    unsafe { serialize(ar, data as *mut u8, len as usize) };
}

unsafe extern "C" fn archive_serialize_int(ar: *mut c_void, v: *mut i32) {
    let mut b = unsafe { *v }.to_le_bytes();
    let writer = unsafe { serialize(ar, b.as_mut_ptr(), 4) };
    if !writer {
        unsafe { *v = i32::from_le_bytes(b) };
    }
}

unsafe extern "C" fn archive_serialize_sqword(ar: *mut c_void, v: *mut i64) {
    let mut b = unsafe { *v }.to_le_bytes();
    let writer = unsafe { serialize(ar, b.as_mut_ptr(), 8) };
    if !writer {
        unsafe { *v = i64::from_le_bytes(b) };
    }
}

unsafe extern "C" fn file_manager_file_size(path: *const u32) -> i32 {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("file_size\t{path}\t{}", m.file_size));
        m.file_size
    })
}

unsafe extern "C" fn file_manager_global_time(path: *const u32) -> i64 {
    let path = to_s(&unsafe { read_z(path) });
    with(|m| {
        rec(m, format!("global_time\t{path}"));
        m.global_time
    })
}

unsafe extern "C" fn actor_serialize_persistence(ar: *mut c_void, actor: *mut c_void) {
    // Mirrors harness `AActor::SerializePersistence`: trace the call, then write
    // one INT `0x1000 + idx` through the archive (recorded as serialize_bytes 4).
    let idx = with(|m| {
        let idx = m.actors[actor as usize - 1].idx;
        rec(m, format!("actor_serialize_persistence\t{idx}"));
        idx
    });
    let mut v = (0x1000 + idx).to_le_bytes();
    unsafe { serialize(ar, v.as_mut_ptr(), 4) };
}

unsafe extern "C" fn actor_destroy(_level: *mut c_void, actor: *mut c_void) {
    with(|m| {
        let idx = m.actors[actor as usize - 1].idx;
        rec(m, format!("level_destroy_actor\t{idx}"));
    });
}

unsafe extern "C" fn debugf(name: i32, msg: *const u32) {
    // Mirrors harness `EngSink::Serialize` category names.
    let name = if name == 0 { "log" } else { "error" };
    let msg = escape_fixture(&to_s(&unsafe { read_z(msg) }));
    with(|m| rec(m, format!("debugf\t{name}\t{msg}")));
}

/// Escapes `\n`, `\r`, `\t` only, as the fixture writer does. Backslashes are
/// left alone so `Save\/cache` stays literal.
fn escape_fixture(s: &str) -> String {
    s.replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn services() -> Hp2LevelPersistServices {
    Hp2LevelPersistServices {
        is_editor,
        gsys_map_path,
        gsys_save_slot_path,
        url_default_map_ext,
        url_default_save_ext,
        app_path_leaf,
        fstring_in_str,
        fstring_left,
        fstring_path_join,
        fstring_append,
        fstring_ne,
        app_strcmp,
        level_actors_num,
        level_actor_at,
        actor_b_persistent,
        actor_name,
        actor_set_b_script_initialized,
        load_map_new,
        load_map_delete,
        load_map_add,
        load_map_find,
        load_map_remove,
        load_map_iter_begin,
        load_map_iter_valid,
        load_map_iter_value,
        load_map_iter_next,
        load_map_iter_end,
        archive_writer_create,
        archive_reader_create,
        archive_close,
        archive_tell,
        archive_seek,
        archive_total_size,
        archive_get_error,
        archive_serialize_bytes,
        archive_serialize_int,
        archive_serialize_sqword,
        file_manager_file_size,
        file_manager_global_time,
        actor_serialize_persistence,
        actor_destroy,
        debug_log: debugf,
    }
}

struct Scenario {
    name: String,
    inputs: Vec<Vec<String>>,
    trace: Vec<String>,
    ret: Option<i32>,
    saved: Option<usize>,
}

/// Parses the fixture verbatim. Paths keep the oracle's literal `Save\/cache` form.
fn parse(text: &str) -> Vec<Scenario> {
    let mut out: Vec<Scenario> = Vec::new();
    for raw in text.lines() {
        if raw.is_empty() {
            continue;
        }
        let line = raw.to_string();
        let f: Vec<&str> = line.split('\t').collect();
        match f[0] {
            "case" => continue,
            "scenario" => out.push(Scenario {
                name: f[1].to_string(),
                inputs: Vec::new(),
                trace: Vec::new(),
                ret: None,
                saved: None,
            }),
            "input" => out
                .last_mut()
                .unwrap()
                .inputs
                .push(f[1..].iter().map(|s| s.to_string()).collect()),
            "=>" => {
                let sc = out.last_mut().unwrap();
                if sc.name.starts_with("persist.") {
                    sc.ret = Some(f[1].parse().unwrap());
                }
            }
            "saved_bytes" => out.last_mut().unwrap().saved = Some(f[1].parse().unwrap()),
            _ => out.last_mut().unwrap().trace.push(line.clone()),
        }
    }
    out
}

fn decode_hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn run(sc: &Scenario) {
    let mut m = Mock {
        file_size: -1,
        ..Mock::default()
    };
    let mut map_name = String::new();
    for inp in &sc.inputs {
        match inp[0].as_str() {
            "map" => map_name = inp[1].clone(),
            "actor" => {
                m.actors.push(ActorRec {
                    idx: inp[1].parse().unwrap(),
                    name: t(&inp[2]),
                    persistent: inp[3] == "1",
                });
                if inp[4] == "1" {
                    m.level.push(m.actors.len());
                }
            }
            "pa_bytes" => {
                let n: usize = inp[1].parse().unwrap();
                m.pa_in = decode_hex(inp.get(2).map_or("", |s| s.as_str()));
                assert_eq!(m.pa_in.len(), n, "scenario {}: pa_bytes length", sc.name);
            }
            "file_size" => m.file_size = inp[1].parse().unwrap(),
            "global_time" => m.global_time = inp[1].parse().unwrap(),
            "reader_null" => m.reader_missing = inp[1] == "1",
            other => panic!("scenario {}: unknown input {other}", sc.name),
        }
    }
    install(m);

    let svc = services();
    let map: Vec<u32> = t(&map_name).into_iter().chain(std::iter::once(0)).collect();
    let mut level_byte = 0u8;
    let level = &mut level_byte as *mut u8 as *mut c_void;
    let ret = unsafe {
        if sc.name.starts_with("persist.save_") {
            level_persist::save_persistent_actors(&svc, level, map.as_ptr())
        } else {
            level_persist::load_persistent_actors(&svc, level, map.as_ptr())
        }
    };
    let m = take();

    assert_eq!(m.trace, sc.trace, "scenario {}: trace", sc.name);
    assert_eq!(
        ret as i32,
        sc.ret.expect("scenario has =>"),
        "scenario {}: return",
        sc.name
    );
    if let Some(n) = sc.saved {
        assert_eq!(
            m.last_close_size,
            Some(n),
            "scenario {}: saved bytes",
            sc.name
        );
    }
}

/// Expected record counts per persist scenario: (name, trace records, return).
/// Pinned from the oracle fixture so a silent regeneration fails here.
const EXPECTED: &[(&str, usize, i32)] = &[
    ("persist.save_two_persistent", 56, 0),
    ("persist.load_roundtrip", 49, 0),
    ("persist.load_missing_file", 2, 0),
    ("persist.load_empty_input", 2, 0),
    ("persist.load_bad_identifier", 7, 1),
    ("persist.load_map_mismatch", 17, 0),
    ("persist.load_size_mismatch", 19, 0),
    ("persist.load_missing_actor", 49, 0),
    ("persist.load_leftover_destroy", 50, 0),
    ("persist.load_bad_eof", 45, 1),
    ("persist.load_wire_error", 7, 1),
];

#[test]
fn persist_scenarios_replay() {
    let scenarios = parse(FIXTURE);
    let mut ran = 0;
    for sc in scenarios.iter().filter(|s| s.name.starts_with("persist.")) {
        let &(_, count, ret) = EXPECTED
            .iter()
            .find(|(n, _, _)| *n == sc.name)
            .unwrap_or_else(|| panic!("unexpected persist scenario {}", sc.name));
        assert_eq!(sc.trace.len(), count, "scenario {}: record count", sc.name);
        assert_eq!(sc.ret, Some(ret), "scenario {}: expected return", sc.name);
        run(sc);
        ran += 1;
    }
    assert_eq!(
        ran,
        EXPECTED.len(),
        "expected 11 persist scenarios in the fixture"
    );
}
