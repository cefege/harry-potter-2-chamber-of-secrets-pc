//! Replays the original-C++ `FCollisionHash` oracle fixtures
//! (`fixtures/actcol_{basic,stale,random_a,random_b}.txt`) against
//! `hp2_engine::actcol::CollisionHash`.
//!
//! The fixtures were produced by linking the pristine 8e34caa `UnActCol.cpp`
//! with a harness that drives `FCollisionHash` through the same op script as
//! below. The harness replaces `UPrimitive` with a box stub
//! (`PointCheck`/`LineCheck`/`GetCollisionBoundingBox`) and `appRand` with
//! `state = state * 1103515245 + 12345; (state >> 16) & 0x7fff` seeded at 1.
//!
//! Fixture layout: op lines, then a `=== expected` line, then the expected output.
//! Ops (`#` starts a comment; floats are 8-digit hex bit patterns, ints decimal):
//!   `ACTOR id cx cy cz collide delete`   `SET id lx ly lz minx miny minz maxx maxy maxz`
//!   `ADD id`   `REM id thorough`   `TICK`
//!   `PT lx ly lz ex ey ez`   `RAD lx ly lz r`   `ENC id lx ly lz pitch yaw roll`
//!   `LINE ex ey ez sx sy sz zx zy zz flags`
//! Output: query header (`PT`/`RAD`/`ENC`/`LINE`), `=> n`, then per result
//! `  actor=<id|-1|?> loc=.. nrm=.. prim=0|1 time=.. item=..` (`RAD`: `  actor=<id>`).
//!
//! Every fixture runs in one test, serially: `CollisionHash` tables are built once
//! per process from the first `appRand` sequence, exactly as the C++ process does.

use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_void};
use std::fmt::Write as _;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FBox, FCheckResult, FRotator};
use hp2_engine::actcol::{CollisionHash, Hp2ActColServices};

const INDEX_NONE: i32 = -1;

/// Harness `gRandState`, process-wide, seeded at 1.
static RAND_STATE: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Copy)]
struct Side {
    cyl: FVector,
    bbox: FBox,
}

/// Harness `gSide`: cylinder extent and collision box, keyed by actor address.
static SIDES: Mutex<BTreeMap<usize, Side>> = Mutex::new(BTreeMap::new());

fn zeroed<T>() -> T {
    // SAFETY: every use is a plain-old-data mirror type for which all-zero is valid.
    unsafe { std::mem::zeroed() }
}

fn side(key: usize) -> Side {
    SIDES.lock().unwrap().get(&key).copied().unwrap_or(Side {
        cyl: zeroed(),
        bbox: zeroed(),
    })
}

fn axes(v: FVector) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn from_axes(a: [f32; 3]) -> FVector {
    FVector {
        x: a[0],
        y: a[1],
        z: a[2],
    }
}

fn hexf(tok: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(tok, 16).expect("hex float"))
}

fn hex(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

fn vec_hex(v: FVector) -> String {
    let a = axes(v);
    format!("{} {} {}", hex(a[0]), hex(a[1]), hex(a[2]))
}

fn vec_of(toks: &[&str]) -> FVector {
    from_axes([hexf(toks[0]), hexf(toks[1]), hexf(toks[2])])
}

// ---- Service callbacks (mirror of the harness stubs) ----------------------------

unsafe extern "C" fn t_app_rand() -> i32 {
    let s = RAND_STATE
        .load(Ordering::Relaxed)
        .wrapping_mul(1_103_515_245)
        .wrapping_add(12345);
    RAND_STATE.store(s, Ordering::Relaxed);
    ((s >> 16) & 0x7fff) as i32
}

unsafe extern "C" fn t_collision_box(actor: *mut c_void, out: *mut c_void) {
    let bbox = side(actor as usize).bbox;
    // SAFETY: `out` is a valid `FBox*` supplied by `actcol.rs`.
    unsafe { *(out as *mut FBox) = bbox };
}

unsafe extern "C" fn t_cylinder_extent(actor: *const c_void, out: *mut c_void) {
    let cyl = side(actor as usize).cyl;
    // SAFETY: `out` is a valid `FVector*` supplied by `actcol.rs`.
    unsafe { *(out as *mut FVector) = cyl };
}

unsafe extern "C" fn t_is_moving_brush(_actor: *const c_void) -> i32 {
    0
}

/// Writes the harness hit fields into `res` (`Next` is left to the caller).
unsafe fn write_hit(res: *mut FCheckResult, owner: *mut c_void, location: FVector, time: f32) {
    // SAFETY: `res` is a valid `FCheckResult*` supplied by `actcol.rs`.
    unsafe {
        (*res).actor = owner;
        (*res).location = location;
        (*res).normal = FVector {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };
        (*res).time = time;
        (*res).primitive = ptr::null_mut();
        (*res).item = INDEX_NONE;
    }
}

unsafe extern "C" fn t_point_check(
    owner: *mut c_void,
    res: *mut c_void,
    location: *const c_void,
    extent: *const c_void,
) -> i32 {
    // SAFETY: `location`/`extent` are valid `FVector*` for the call.
    let (loc, ext) = unsafe { (*(location as *const FVector), *(extent as *const FVector)) };
    let bbox = side(owner as usize).bbox;
    let (l, e) = (axes(loc), axes(ext));
    let (lo, hi) = (axes(bbox.min), axes(bbox.max));
    let hit = (0..3).all(|i| l[i] - e[i] <= hi[i] && l[i] + e[i] >= lo[i]);
    if !hit {
        return 1;
    }
    // SAFETY: `res` is a valid `FCheckResult*` supplied by `actcol.rs`.
    unsafe { write_hit(res as *mut FCheckResult, owner, loc, 0.0) };
    0
}

unsafe extern "C" fn t_line_check(
    owner: *mut c_void,
    res: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    size: *const c_void,
    _flags: u32,
) -> i32 {
    // SAFETY: `end`/`start`/`size` are valid `FVector*` for the call.
    let (e_v, s_v, z_v) = unsafe {
        (
            *(end as *const FVector),
            *(start as *const FVector),
            *(size as *const FVector),
        )
    };
    let bbox = side(owner as usize).bbox;
    let (lo_b, hi_b) = (axes(bbox.min), axes(bbox.max));
    let (e, s, z) = (axes(e_v), axes(s_v), axes(z_v));
    let hit = (0..3).all(|i| {
        let lo = (if s[i] < e[i] { s[i] } else { e[i] }) - z[i];
        let hi = (if s[i] > e[i] { s[i] } else { e[i] }) + z[i];
        lo <= hi_b[i] && hi >= lo_b[i]
    });
    if !hit {
        return 1;
    }
    // SAFETY: `res` is a valid `FCheckResult*` supplied by `actcol.rs`.
    unsafe { write_hit(res as *mut FCheckResult, owner, s_v, 0.5) };
    0
}

unsafe extern "C" fn t_alloc(_mem: *mut c_void) -> *mut c_void {
    // Leaked on purpose: the replay is short-lived and results must outlive the query.
    Box::into_raw(Box::new(zeroed::<FCheckResult>())) as *mut c_void
}

unsafe extern "C" fn t_debugf(_actor: *const c_void) {}

unsafe extern "C" fn t_check_failed(expr: *const c_char, file: *const c_char, line: i32) {
    // SAFETY: `actcol.rs` passes NUL-terminated static strings.
    let (expr, file) = unsafe { (CStr::from_ptr(expr), CStr::from_ptr(file)) };
    panic!("check failed: {} {:?}:{line}", expr.to_string_lossy(), file);
}

fn services() -> Hp2ActColServices {
    Hp2ActColServices {
        app_rand: t_app_rand,
        actor_collision_bounding_box: t_collision_box,
        actor_cylinder_extent: t_cylinder_extent,
        actor_is_moving_brush: t_is_moving_brush,
        primitive_point_check: t_point_check,
        primitive_line_check: t_line_check,
        alloc_check_result: t_alloc,
        debugf_moved_without_hashing: t_debugf,
        check_failed: t_check_failed,
    }
}

// ---- Scenario driver ------------------------------------------------------------

#[derive(Default)]
struct Counts {
    ops: usize,
    queries: usize,
    results: usize,
}

struct Scenario {
    svc: Hp2ActColServices,
    hash: CollisionHash,
    /// Harness `gById` / `gIdOf`.
    ptr_by_id: BTreeMap<i32, *mut AActor>,
    id_by_ptr: BTreeMap<usize, i32>,
    out: String,
    counts: Counts,
}

impl Scenario {
    fn new() -> Self {
        let svc = services();
        let hash = CollisionHash::new(&svc);
        Self {
            svc,
            hash,
            ptr_by_id: BTreeMap::new(),
            id_by_ptr: BTreeMap::new(),
            out: String::new(),
            counts: Counts::default(),
        }
    }

    fn find(&self, id: i32) -> *mut AActor {
        *self
            .ptr_by_id
            .get(&id)
            .unwrap_or_else(|| panic!("unknown actor {id}"))
    }

    fn id_str(&self, actor: *const AActor) -> String {
        if actor.is_null() {
            return "-1".to_string();
        }
        match self.id_by_ptr.get(&(actor as usize)) {
            Some(id) => id.to_string(),
            None => "?".to_string(),
        }
    }

    fn print_results(&mut self, head: *mut FCheckResult, full: bool) {
        let mut results = Vec::new();
        let mut r = head;
        while !r.is_null() {
            // SAFETY: list nodes come from `t_alloc` and stay alive for the scenario.
            let node = unsafe { *r };
            results.push(node);
            r = node.next as *mut FCheckResult;
        }
        self.counts.results += results.len();
        writeln!(self.out, "=> {}", results.len()).unwrap();
        for node in results {
            let actor = self.id_str(node.actor as *const AActor);
            if !full {
                writeln!(self.out, "  actor={actor}").unwrap();
                continue;
            }
            writeln!(
                self.out,
                "  actor={actor} loc={} nrm={} prim={} time={} item={}",
                vec_hex(node.location),
                vec_hex(node.normal),
                i32::from(!node.primitive.is_null()),
                hex(node.time),
                node.item,
            )
            .unwrap();
        }
    }

    fn apply(&mut self, line: &str) {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let Some((&op, args)) = toks.split_first() else {
            return;
        };
        self.counts.ops += 1;
        match op {
            "ACTOR" => {
                let id: i32 = args[0].parse().unwrap();
                let cyl = vec_of(&args[1..4]);
                let collide = args[4].parse::<i32>().unwrap() != 0;
                let delete = args[5].parse::<i32>().unwrap() != 0;
                let p = Box::into_raw(Box::new(zeroed::<AActor>()));
                // SAFETY: `p` was just allocated and is exclusively owned here.
                unsafe {
                    (*p).set_b_collide_actors(collide);
                    (*p).set_b_delete_me(delete);
                }
                SIDES.lock().unwrap().insert(
                    p as usize,
                    Side {
                        cyl,
                        bbox: zeroed(),
                    },
                );
                self.ptr_by_id.insert(id, p);
                self.id_by_ptr.insert(p as usize, id);
            }
            "SET" => {
                let id: i32 = args[0].parse().unwrap();
                let p = self.find(id);
                let loc = vec_of(&args[1..4]);
                let (min, max) = (vec_of(&args[4..7]), vec_of(&args[7..10]));
                // SAFETY: `p` is a live actor allocated by `ACTOR`.
                unsafe { (*p).location = loc };
                let mut sides = SIDES.lock().unwrap();
                let entry = sides.entry(p as usize).or_insert(Side {
                    cyl: zeroed(),
                    bbox: zeroed(),
                });
                entry.bbox.min = min;
                entry.bbox.max = max;
                entry.bbox.is_valid = 1;
            }
            "ADD" => {
                let p = self.find(args[0].parse().unwrap());
                // SAFETY: `p` is a live actor.
                unsafe { self.hash.add_actor(&self.svc, p) };
            }
            "REM" => {
                let p = self.find(args[0].parse().unwrap());
                let thorough = args[1].parse::<i32>().unwrap() != 0;
                // SAFETY: `p` is a live actor.
                unsafe { self.hash.remove_actor(&self.svc, p, thorough) };
            }
            "TICK" => self.hash.tick(),
            "PT" => {
                let loc = vec_of(&args[0..3]);
                let ext = vec_of(&args[3..6]);
                // SAFETY: the hash only touches actors registered by `ACTOR`.
                let head = unsafe { self.hash.actor_point_check(&self.svc, loc, ext) };
                self.begin_query("PT");
                self.print_results(head, true);
            }
            "RAD" => {
                let loc = vec_of(&args[0..3]);
                let radius = hexf(args[3]);
                // SAFETY: as above.
                let head = unsafe { self.hash.actor_radius_check(&self.svc, loc, radius) };
                self.begin_query("RAD");
                self.print_results(head, false);
            }
            "ENC" => {
                let p = self.find(args[0].parse().unwrap());
                let loc = vec_of(&args[1..4]);
                let rot = FRotator {
                    pitch: args[4].parse().unwrap(),
                    yaw: args[5].parse().unwrap(),
                    roll: args[6].parse().unwrap(),
                };
                // SAFETY: `p` is a live actor.
                let head = unsafe { self.hash.actor_encroachment_check(&self.svc, p, loc, rot) };
                self.begin_query("ENC");
                self.print_results(head, true);
            }
            "LINE" => {
                let e = vec_of(&args[0..3]);
                let s = vec_of(&args[3..6]);
                let z = vec_of(&args[6..9]);
                let flags: u8 = args[9].parse().unwrap();
                // SAFETY: as above; a null `mem` allocates results from `GMem`.
                let head = unsafe {
                    self.hash.actor_line_check(
                        &self.svc,
                        ptr::null_mut(),
                        e,
                        s,
                        z,
                        u32::from(flags),
                    )
                };
                self.begin_query("LINE");
                self.print_results(head, true);
            }
            other => panic!("unknown op: {other}"),
        }
    }

    fn begin_query(&mut self, tag: &str) {
        self.counts.queries += 1;
        writeln!(self.out, "{tag}").unwrap();
    }
}

/// Compares line by line so a mismatch names the first differing line.
fn check(name: &str, got: &str, want: &str) {
    for (i, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "{name}: output line {}", i + 1);
    }
    assert_eq!(
        got.lines().count(),
        want.lines().count(),
        "{name}: output line count"
    );
}

fn replay(name: &str, text: &str, want: Counts) {
    let (ops, expected) = text
        .split_once("=== expected\n")
        .unwrap_or_else(|| panic!("{name}: missing `=== expected` separator"));
    let mut s = Scenario::new();
    for line in ops.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        s.apply(line);
    }
    check(name, &s.out, expected);
    assert_eq!(s.counts.ops, want.ops, "{name}: op count");
    assert_eq!(s.counts.queries, want.queries, "{name}: query count");
    assert_eq!(s.counts.results, want.results, "{name}: result count");
}

#[test]
fn actcol_fixtures_replay_bit_exact() {
    // Run serially in one test: the shared hash tables and the LCG must follow
    // the same first-hash order as the single-hash C++ oracle process.
    replay(
        "basic",
        include_str!("fixtures/actcol_basic.txt"),
        Counts {
            ops: 39,
            queries: 18,
            results: 13,
        },
    );
    replay(
        "stale",
        include_str!("fixtures/actcol_stale.txt"),
        Counts {
            ops: 21,
            queries: 8,
            results: 2,
        },
    );
    replay(
        "random_a",
        include_str!("fixtures/actcol_random_a.txt"),
        Counts {
            ops: 707,
            queries: 228,
            results: 298,
        },
    );
    replay(
        "random_b",
        include_str!("fixtures/actcol_random_b.txt"),
        Counts {
            ops: 696,
            queries: 214,
            results: 183,
        },
    );
}
