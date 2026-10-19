//! Replays the original-C++ `AMover` oracle fixtures
//! (`fixtures/mover_{spawned_postload,edit,raytrace}.txt`) against
//! `hp2_engine::mover`.
//!
//! The fixtures were produced by linking the pristine 8e34caa `UnMover.cpp`
//! (compiled with the release flags plus `-g`) with a harness that calls the
//! real `AMover` member functions on zeroed objects. The harness stubs only the
//! base-class bodies outside the ported surface (`AActor::Spawned`,
//! `AActor::PostLoad`, `AActor::PostEditChange`) as no-ops.
//!
//! Services: the C++ `Hp2MoverServices.cpp` hash/tracker/renumber bodies are
//! replaced here by recording mocks. The renumber mock performs the same loop
//! as `MoverRenumberBrushPolys` (`if Model && Model->Polys: iLink = i`), so the
//! `oIL` field checks the port's call contract, not that loop's C++ source.
//! Provenance gap: the generator (`gen_mover.py`) and harness (`mover_harness.cpp`)
//! were in the deleted `out/agent-EngActCol/oracle/` and are not in the tree. The
//! fixtures are checked in as-is; regenerating them requires rewriting that harness.
//! Case counts (51/112/150) are asserted below.
//! Fixture line: `KEY=VAL` input tokens, ` |`, then `oKEY=VAL` output tokens.
//! Floats are 8-digit hex bit patterns, ints decimal, vectors `h,h,h`, arrays
//! `;`-separated, `-` for empty lists. Lines starting with `#` are comments.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AMover, FRotator, ULevel};
use hp2_engine::mover::{self, Hp2MoverServices};

thread_local! {
    static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn record(ev: &'static str) {
    EVENTS.with(|e| e.borrow_mut().push(ev));
}

/// Stand-in brush: `np` is -2 (brush without Polys) or the Polys element count.
struct MockBrush {
    np: i32,
    links: Vec<i32>,
}

/// Non-null sentinels for the collision hash and brush tracker; the mocks never dereference them.
const HASH_PTR: *mut c_void = 0x1000 as *mut c_void;
const TRACKER_PTR: *mut c_void = 0x2000 as *mut c_void;

unsafe extern "C" fn mock_hash_add(_hash: *mut c_void, _actor: *mut c_void) {
    record("AD");
}
unsafe extern "C" fn mock_hash_remove(_hash: *mut c_void, _actor: *mut c_void) {
    record("RM");
}
unsafe extern "C" fn mock_tracker_update(_tracker: *mut c_void, _actor: *mut c_void) {
    record("TU");
}
unsafe extern "C" fn mock_tracker_flush(_tracker: *mut c_void, _actor: *mut c_void) {
    record("TF");
}
unsafe extern "C" fn mock_renumber(brush: *mut c_void) {
    if brush.is_null() {
        return;
    }
    let b = unsafe { &mut *brush.cast::<MockBrush>() };
    if b.np >= 0 {
        for (i, l) in b.links.iter_mut().enumerate() {
            *l = i as i32;
        }
    }
}

// The class-table services are not reached by the body functions under test.
unsafe extern "C" fn unused_ptr(_name: *const c_char) -> *mut c_void {
    std::process::abort()
}
unsafe extern "C" fn unused_ptr_size(_name: *const c_char, _size: i32) -> *mut c_void {
    std::process::abort()
}
unsafe extern "C" fn unused_void_ptr(_st: *mut c_void) {
    std::process::abort()
}
unsafe extern "C" fn unused_enum_name(_e: *mut c_void, _name: *const c_char) {
    std::process::abort()
}
unsafe extern "C" fn unused_class_ref(_id: i32) -> *mut c_void {
    std::process::abort()
}
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn unused_add_property(
    _owner: *mut c_void,
    _kind: i32,
    _name: *const c_char,
    _category: *const c_char,
    _offset: i32,
    _array_dim: i32,
    _bit_mask: u32,
    _ref_obj: *mut c_void,
) {
    std::process::abort()
}

fn services() -> Hp2MoverServices {
    Hp2MoverServices {
        find_struct: unused_ptr,
        new_struct: unused_ptr_size,
        link_struct: unused_void_ptr,
        new_enum: unused_ptr,
        enum_add_name: unused_enum_name,
        class_ref: unused_class_ref,
        add_property: unused_add_property,
        hash_add: mock_hash_add,
        hash_remove: mock_hash_remove,
        brush_tracker_update: mock_tracker_update,
        brush_tracker_flush: mock_tracker_flush,
        renumber_brush_polys: mock_renumber,
    }
}

fn parse_f(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).expect("hex float"))
}

fn fmt_f(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

fn parse_v(s: &str) -> FVector {
    let p: Vec<&str> = s.split(',').collect();
    assert_eq!(p.len(), 3, "vector {s}");
    FVector {
        x: parse_f(p[0]),
        y: parse_f(p[1]),
        z: parse_f(p[2]),
    }
}

fn fmt_v(v: FVector) -> String {
    format!("{},{},{}", fmt_f(v.x), fmt_f(v.y), fmt_f(v.z))
}

fn parse_r(s: &str) -> FRotator {
    let p: Vec<i32> = s
        .split(',')
        .map(|t| t.parse().expect("rotator int"))
        .collect();
    assert_eq!(p.len(), 3, "rotator {s}");
    FRotator {
        pitch: p[0],
        yaw: p[1],
        roll: p[2],
    }
}

fn fmt_r(r: FRotator) -> String {
    format!("{},{},{}", r.pitch, r.yaw, r.roll)
}

/// One replayed case, with its expected output tail (` oKN=...`).
struct Case {
    name: String,
    func: String,
    expected: String,
    actual: String,
}

fn run_line(line: &str) -> Case {
    let (input, expected) = line.split_once(" |").expect("fixture separator");
    let kv: BTreeMap<&str, &str> = input.split(' ').filter_map(|t| t.split_once('=')).collect();
    let get = |k: &str| *kv.get(k).unwrap_or_else(|| panic!("missing {k}: {line}"));
    let name = get("N").to_string();
    let func = get("F").to_string();
    let np: i32 = get("NP").parse().unwrap();

    // Brush: NP=-1 null, NP=-2 brush without Polys, NP>=0 Polys with NP elements.
    let mut brush = MockBrush {
        np,
        links: Vec::new(),
    };
    if np >= 0 {
        brush.links = get("IL")
            .split(',')
            .take(np as usize)
            .map(|t| t.parse().unwrap())
            .collect();
    }
    let brush_ptr: *mut c_void = if np == -1 {
        std::ptr::null_mut()
    } else {
        ptr::addr_of_mut!(brush).cast()
    };

    // The level is zeroed; only the hash/tracker pointers are set.
    let mut level: ULevel = unsafe { std::mem::zeroed() };
    if get("HASH") == "1" {
        level.hash = HASH_PTR;
    }
    if get("TRK") == "1" {
        level.brush_tracker = TRACKER_PTR;
    }

    let mut mv: AMover = unsafe { std::mem::zeroed() };
    mv.base.x_level = ptr::addr_of_mut!(level).cast();
    mv.base.brush = brush_ptr;
    mv.base.set_b_collide_actors(get("CA") == "1");
    mv.key_num = get("KN").parse().unwrap();
    mv.world_raytrace_key = get("WK").parse().unwrap();
    mv.brush_raytrace_key = get("BK").parse().unwrap();
    mv.base.location = parse_v(get("LOC"));
    mv.base.rotation = parse_r(get("ROT"));
    mv.base_pos = parse_v(get("BP"));
    mv.base_rot = parse_r(get("BR"));
    mv.old_pos = parse_v(get("OP"));
    mv.old_rot = parse_r(get("OR"));
    mv.saved_pos = parse_v(get("SP"));
    mv.saved_rot = parse_r(get("SR"));
    let kp: Vec<&str> = get("KP").split(';').collect();
    let kr: Vec<&str> = get("KR").split(';').collect();
    assert_eq!(kp.len(), 8);
    assert_eq!(kr.len(), 8);
    for i in 0..8 {
        mv.key_pos[i] = parse_v(kp[i]);
        mv.key_rot[i] = parse_r(kr[i]);
    }

    EVENTS.with(|e| e.borrow_mut().clear());
    let s = services();
    unsafe {
        match func.as_str() {
            "Spawned" => mover::spawned(&mut mv),
            "PostLoad" => mover::post_load(&mut mv, &s),
            "PostEditMove" => mover::post_edit_move(&mut mv),
            "PostEditChange" => mover::post_edit_change(&mut mv),
            "PreRaytrace" => mover::pre_raytrace(&mut mv),
            "PostRaytrace" => mover::post_raytrace(&mut mv, &s),
            "SetWorldRaytraceKey" => mover::set_world_raytrace_key(&mut mv, &s),
            "SetBrushRaytraceKey" => mover::set_brush_raytrace_key(&mut mv, &s),
            other => panic!("unknown fn {other}"),
        }
    }

    let kps: Vec<String> = mv.key_pos.iter().map(|v| fmt_v(*v)).collect();
    let krs: Vec<String> = mv.key_rot.iter().map(|r| fmt_r(*r)).collect();
    let il = if np >= 0 && !brush.links.is_empty() {
        brush
            .links
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join(",")
    } else {
        "-".to_string()
    };
    let ev = EVENTS.with(|e| e.borrow().join(","));
    let ev = if ev.is_empty() { "-".to_string() } else { ev };
    let actual = format!(
        " oKN={} oWK={} oBK={} oLOC={} oROT={} oBP={} oBR={} oOP={} oOR={} oSP={} oSR={} oKP={} oKR={} oIL={} oEV={}",
        mv.key_num,
        mv.world_raytrace_key,
        mv.brush_raytrace_key,
        fmt_v(mv.base.location),
        fmt_r(mv.base.rotation),
        fmt_v(mv.base_pos),
        fmt_r(mv.base_rot),
        fmt_v(mv.old_pos),
        fmt_r(mv.old_rot),
        fmt_v(mv.saved_pos),
        fmt_r(mv.saved_rot),
        kps.join(";"),
        krs.join(";"),
        il,
        ev,
    );
    Case {
        name,
        func,
        expected: expected.to_string(),
        actual,
    }
}

fn replay(file: &str, expected_cases: usize) {
    let path = format!("{}/tests/fixtures/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut n = 0;
    let mut failures = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        n += 1;
        let c = run_line(line);
        if c.expected != c.actual {
            failures.push(format!(
                "{file} {} ({}):\n  expected:{}\n  actual:  {}",
                c.name, c.func, c.expected, c.actual
            ));
        }
    }
    assert_eq!(n, expected_cases, "{file}: case count");
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn mover_spawned_postload_matches_oracle() {
    replay("mover_spawned_postload.txt", 51);
}

#[test]
fn mover_edit_matches_oracle() {
    replay("mover_edit.txt", 112);
}

#[test]
fn mover_raytrace_matches_oracle() {
    replay("mover_raytrace.txt", 150);
}
