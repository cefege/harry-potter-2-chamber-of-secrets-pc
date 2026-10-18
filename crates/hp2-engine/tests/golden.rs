//! Bit-exact check of the Rust `FReachSpec` and `AWind` ports against
//! `tests/fixtures/orig_reach_wind.txt`, which was recorded by running the
//! original `UnReach.cpp` / `UnWind.cpp` (from commit 16650a7) through a
//! driver with the same services, seeds and call order as this file.
//! Floats are printed as IEEE bit patterns, so any difference in rounding,
//! operation order or RNG draws fails the comparison.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APawn, AWind, FReachSpec, FRotator, ULevel};
use hp2_engine::reach;
use hp2_engine::services::Hp2EngineServices;
use hp2_engine::wind;

// ---- services (mirror the driver's fakes) ----

/// Driver's `GRngState`; the driver seeds it with this constant.
static RNG_STATE: AtomicU32 = AtomicU32::new(0x2545_F491);

fn next_rng() -> u32 {
    let s = RNG_STATE
        .load(Ordering::Relaxed)
        .wrapping_mul(1_103_515_245)
        .wrapping_add(12345);
    RNG_STATE.store(s, Ordering::Relaxed);
    s
}

unsafe extern "C" fn app_frand() -> f32 {
    ((next_rng() >> 8) & 0x00FF_FFFF) as f32 / 16_777_216.0
}

unsafe extern "C" fn app_rand() -> i32 {
    ((next_rng() >> 16) & 0x7fff) as i32
}

unsafe extern "C" fn far_move_actor(
    _level: *mut c_void,
    actor: *mut c_void,
    dest: *const c_void,
) -> i32 {
    unsafe {
        (*(actor as *mut AActor)).location = *(dest as *const FVector);
    }
    1
}

unsafe extern "C" fn point_reachable(pawn: *mut c_void, point: *const c_void) -> i32 {
    // Mirrors driver.cpp: `aPoint.X + aPoint.Y + (CollisionRadius - CollisionHeight) > 0.f`.
    let p = unsafe { *(point as *const FVector) };
    let a = unsafe { &*(pawn as *const AActor) };
    (p.x + p.y + (a.collision_radius - a.collision_height) > 0.0) as i32
}

unsafe extern "C" fn set_collision_size(actor: *mut c_void, radius: f32, height: f32) {
    unsafe {
        let a = &mut *(actor as *mut AActor);
        a.collision_radius = radius;
        a.collision_height = height;
    }
}

unsafe extern "C" fn model_fast_line_check(
    _model: *mut c_void,
    end: *const c_void,
    _start: *const c_void,
) -> u8 {
    let e = unsafe { *(end as *const FVector) };
    (e.x < 300.0) as u8
}

/// Mirrors the harness `HarnessRotatorVector` (driver.cpp): each component is the
/// rotator field divided by 10000. This is a test double; production uses
/// `FRotator::Vector()` through the C++ adapter.
unsafe extern "C" fn rotator_vector(rot: *const c_void, out: *mut c_void) {
    unsafe {
        let r = &*(rot as *const FRotator);
        *(out as *mut FVector) = FVector {
            x: r.pitch as f32 / 10000.0,
            y: r.yaw as f32 / 10000.0,
            z: r.roll as f32 / 10000.0,
        };
    }
}

static SVC: Hp2EngineServices = Hp2EngineServices {
    app_frand,
    app_rand,
    far_move_actor,
    point_reachable,
    set_collision_size,
    model_fast_line_check,
    rotator_vector,
};

// ---- output helpers (match the driver's printf formats) ----

fn fb(v: f32) -> String {
    format!(" {:08x}", v.to_bits())
}

fn vb(v: FVector) -> String {
    format!("{}{}{}", fb(v.x), fb(v.y), fb(v.z))
}

fn vec3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

/// `SpecOut`: distance, radius, height, flags, pruned, start index, end index.
fn spec_out(s: &FReachSpec, acts: &[*mut AActor; 4]) -> String {
    let idx = |p: *mut c_void| -> i32 {
        if p.is_null() {
            return -1;
        }
        acts.iter()
            .position(|&a| a as *mut c_void == p)
            .map_or(99, |i| i as i32)
    };
    format!(
        " {} {} {} {} {} {} {}",
        s.distance,
        s.collision_radius,
        s.collision_height,
        s.reach_flags,
        s.b_pruned as i32,
        idx(s.start),
        idx(s.end)
    )
}

// ---- reach section (driver RunReach) ----

fn make_spec(k: i32, acts: &[*mut AActor; 4]) -> FReachSpec {
    // Padding members are private: zero them, then set the mirrored fields.
    let mut s: FReachSpec = unsafe { std::mem::zeroed() };
    s.distance = (k * 37) % 500;
    s.start = acts[(k % 4) as usize] as *mut c_void;
    s.end = acts[((k + 1) % 4) as usize] as *mut c_void;
    s.collision_radius = (k * 13) % 120;
    s.collision_height = (k * 29) % 150;
    s.reach_flags = (k * 7) % 64;
    s.b_pruned = (k % 2) as u8;
    s
}

fn run_reach(out: &mut String, acts: &[*mut AActor; 4], scout: *mut APawn) {
    unsafe {
        for k in 0..24 {
            for pass in 0..2 {
                let inp = make_spec(k, acts);
                let s = inp;
                let label = if pass == 0 { "monster" } else { "bot" };
                let ret = if pass == 0 {
                    reach::monster_path(&s)
                } else {
                    reach::bot_only_path(&s)
                };
                out.push_str(&format!("{label}{} =>", spec_out(&inp, acts)));
                out.push_str(&format!(" {ret}{}\n", spec_out(&s, acts)));
            }
        }

        for a in 0..12 {
            for b in 0..12 {
                let fa = make_spec(a + 3, acts);
                let fbs = make_spec(b * 5 + 1, acts);
                let mut c = fa;
                reach::combine(&fa, &fbs, &mut c);
                out.push_str(&format!(
                    "plus{}{} =>",
                    spec_out(&fa, acts),
                    spec_out(&fbs, acts)
                ));
                out.push_str(&format!(
                    " {} {} {} {}\n",
                    c.distance, c.collision_radius, c.collision_height, c.reach_flags
                ));
                out.push_str(&format!(
                    "le{}{} =>",
                    spec_out(&fa, acts),
                    spec_out(&fbs, acts)
                ));
                out.push_str(&format!(" {}\n", reach::less_equal(&fa, &fbs)));
                out.push_str(&format!(
                    "eq{}{} =>",
                    spec_out(&fa, acts),
                    spec_out(&fbs, acts)
                ));
                out.push_str(&format!(" {}\n", reach::equal(&fa, &fbs)));
            }
        }

        let begins = [
            vec3(0.0, 0.0, 0.0),
            vec3(10.0, 20.0, 30.0),
            vec3(100.0, 50.0, 20.0),
        ];
        let dests = [
            vec3(50.0, 0.0, 0.0),
            vec3(200.0, 100.0, 40.0),
            vec3(-30.0, 10.0, 0.0),
        ];
        let scout_rad = [17.0f32, 40.0];
        let scout_hgt = [44.0f32, 120.0];
        for begin in begins {
            for dest in dests {
                for si in 0..2 {
                    for k in 0..4 {
                        (*acts[0]).location = begin;
                        (*acts[1]).location = dest;
                        let mut s = make_spec(k * 3 + si as i32, acts);
                        (*scout).base.collision_radius = scout_rad[si];
                        (*scout).base.collision_height = scout_hgt[si];
                        (*scout).base.location = vec3(0.0, 0.0, 0.0);

                        out.push_str("define");
                        out.push_str(&vb(begin));
                        out.push_str(&vb(dest));
                        out.push_str(&fb(scout_rad[si]));
                        out.push_str(&fb(scout_hgt[si]));
                        out.push_str(&spec_out(&s, acts));
                        out.push_str(" =>");
                        let ret = reach::define_for(&SVC, &mut s, acts[0], acts[1], scout);
                        out.push_str(&format!(" {ret}"));
                        out.push_str(&spec_out(&s, acts));
                        out.push_str(&fb((*scout).base.collision_radius));
                        out.push_str(&fb((*scout).base.collision_height));
                        out.push_str(&vb((*scout).base.location));
                        out.push('\n');

                        (*scout).base.collision_radius = scout_rad[si];
                        (*scout).base.collision_height = scout_hgt[si];
                        (*scout).base.location = vec3(0.0, 0.0, 0.0);
                        let mut t = make_spec(k * 3 + si as i32, acts);
                        out.push_str("findbest");
                        out.push_str(&vb(begin));
                        out.push_str(&vb(dest));
                        out.push_str(&fb(scout_rad[si]));
                        out.push_str(&fb(scout_hgt[si]));
                        out.push_str(&spec_out(&t, acts));
                        out.push_str(" =>");
                        let ret = reach::find_best_reachable(&SVC, &mut t, &begin, &dest, scout);
                        out.push_str(&format!(" {ret}"));
                        out.push_str(&spec_out(&t, acts));
                        out.push_str(&fb((*scout).base.collision_radius));
                        out.push_str(&fb((*scout).base.collision_height));
                        out.push_str(&vb((*scout).base.location));
                        out.push('\n');
                    }
                }
            }
        }
    }
}

// ---- wind section (driver RunWind) ----

// Boxes keep each object at a stable address while the registries hold raw pointers to it.
#[allow(clippy::vec_box)]
struct WindRig {
    winds: Vec<Box<AWind>>,
    levels: Vec<Box<ULevel>>,
    models: Vec<Box<[u8; 64]>>,
}

impl WindRig {
    fn new() -> Self {
        Self {
            winds: Vec::new(),
            levels: Vec::new(),
            models: Vec::new(),
        }
    }

    fn rot(&self, p: i32, y: i32, r: i32, out: &mut String) {
        let rot = FRotator {
            pitch: p,
            yaw: y,
            roll: r,
        };
        let mut v = vec3(0.0, 0.0, 0.0);
        unsafe {
            rotator_vector(
                &rot as *const _ as *const c_void,
                &mut v as *mut _ as *mut c_void,
            );
        }
        out.push_str(&format!("ROT {p} {y} {r} =>{}\n", vb(v)));
    }

    /// `WindCreate`: builds an AWind on a fresh level and registers it.
    #[allow(clippy::too_many_arguments)]
    fn create(
        &mut self,
        radius: i32,
        inner: i32,
        fluct: i32,
        period: i32,
        source: i32,
        perm: i32,
        loc: FVector,
        rot: (i32, i32, i32),
        fluc: FVector,
        flucvel: FVector,
        speed: f32,
        out: &mut String,
    ) {
        self.rot(rot.0, rot.1, rot.2, out);
        let mut model = Box::new([0u8; 64]);
        let mut level = Box::new(unsafe { std::mem::zeroed::<ULevel>() });
        level.model = &mut *model as *mut [u8; 64] as *mut c_void;
        let mut w = Box::new(unsafe { std::mem::zeroed::<AWind>() });
        w.base.x_level = &mut *level as *mut ULevel as *mut c_void;
        w.base.location = loc;
        w.base.rotation = FRotator {
            pitch: rot.0,
            yaw: rot.1,
            roll: rot.2,
        };
        w.wind_speed = speed;
        w.wind_radius = radius as u8;
        w.wind_radius_inner = inner as u8;
        w.wind_fluctuation = fluct as u8;
        w.wind_fluc_period = period as u8;
        w.wind_source = source as u8;
        w.set_b_permeating(perm != 0);
        w.fluc = fluc;
        w.fluc_vel = flucvel;
        wind::register(&mut *w as *mut AWind);
        let idx = self.winds.len();
        self.winds.push(w);
        self.levels.push(level);
        self.models.push(model);

        out.push_str(&format!("WIND {idx}"));
        out.push_str(&fb(speed));
        out.push_str(&format!(
            " {radius} {inner} {fluct} {period} {source} {perm}"
        ));
        out.push_str(&vb(loc));
        out.push_str(&format!(" {} {} {}", rot.0, rot.1, rot.2));
        out.push_str(&vb(fluc));
        out.push_str(&vb(flucvel));
        out.push('\n');
    }

    fn ptr(&mut self, i: usize) -> *mut AWind {
        &mut *self.winds[i] as *mut AWind
    }

    fn grid(&mut self, i: usize, out: &mut String) {
        // Mirrors driver.cpp WindGrid. Negative coordinates stay above -32 (noise lattice
        // index >= 0); the far negative point is sampled only when WindFluctuation == 0.
        let mut grid = vec![
            vec3(10.0, 10.0, 10.0),
            vec3(120.0, 40.0, 30.0),
            vec3(200.0, 200.0, 200.0),
            vec3(255.0, 128.0, 0.0),
            vec3(40.5, 33.25, 70.75),
            vec3(5.0, 250.0, 100.0),
            vec3(50.0, 60.0, 70.0),
            vec3(-12.5, 3.0, 20.0),
            vec3(-31.75, -0.5, -15.0),
        ];
        let w = self.ptr(i);
        if unsafe { (*w).wind_fluctuation } == 0 {
            grid.push(vec3(-60.0, -40.0, -20.0));
        }
        for g in grid {
            let r = unsafe { wind::get_wind_shared(&SVC, w, g) };
            out.push_str(&format!("WGET{} =>{}\n", vb(g), vb(r)));
        }
    }

    fn tick(&mut self, i: usize, dt: f32, out: &mut String) {
        let w = self.ptr(i);
        unsafe { wind::tick(&SVC, w, dt) };
        let (f, fv) = unsafe { ((*w).fluc, (*w).fluc_vel) };
        out.push_str(&format!("WTICK{} =>{}{}\n", fb(dt), vb(f), vb(fv)));
    }

    fn end(&mut self, out: &mut String) {
        for w in &mut self.winds {
            wind::unregister(&mut **w as *mut AWind);
        }
        self.winds.clear();
        self.levels.clear();
        self.models.clear();
        out.push_str("WEND\n");
    }
}

fn run_wind(out: &mut String) {
    // Scenario 1: point source, fluctuating, 5 ticks.
    let mut rig = WindRig::new();
    let zero = vec3(0.0, 0.0, 0.0);
    rig.create(
        200,
        64,
        128,
        32,
        0,
        0,
        vec3(50.0, 60.0, 70.0),
        (0, 0, 0),
        zero,
        zero,
        100.0,
        out,
    );
    rig.grid(0, out);
    for dt in [0.016f32, 0.033, 0.5, 0.0, 1.25] {
        rig.tick(0, dt, out);
        rig.grid(0, out);
    }
    rig.end(out);

    // Scenario 2: plane source, rotated, one tick.
    let mut rig = WindRig::new();
    rig.create(
        255,
        0,
        0,
        0,
        1,
        1,
        vec3(10.0, 20.0, 30.0),
        (16384, 8192, 0),
        vec3(1.0, 2.0, 3.0),
        vec3(0.5, 0.0, 0.0),
        75.5,
        out,
    );
    rig.grid(0, out);
    rig.tick(0, 0.5, out);
    rig.grid(0, out);
    rig.end(out);

    // Scenario 3: ambient source, strong fluctuation, 5 ticks.
    let mut rig = WindRig::new();
    rig.create(
        120,
        200,
        255,
        1,
        2,
        0,
        vec3(300.0, 40.0, 5.0),
        (0, 0, 0),
        zero,
        zero,
        40.0,
        out,
    );
    rig.grid(0, out);
    for dt in [0.016f32, 0.033, 0.5, 0.0, 1.25] {
        rig.tick(0, dt, out);
        rig.grid(0, out);
    }
    rig.end(out);

    // Scenario 4: three live winds, totals over the grid.
    let mut rig = WindRig::new();
    rig.create(
        200,
        64,
        128,
        32,
        0,
        0,
        vec3(50.0, 60.0, 70.0),
        (0, 0, 0),
        zero,
        zero,
        100.0,
        out,
    );
    rig.create(
        255,
        0,
        0,
        0,
        1,
        1,
        vec3(10.0, 20.0, 30.0),
        (16384, 8192, 0),
        vec3(1.0, 2.0, 3.0),
        vec3(0.5, 0.0, 0.0),
        75.5,
        out,
    );
    rig.create(
        120,
        200,
        255,
        1,
        2,
        0,
        vec3(300.0, 40.0, 5.0),
        (0, 0, 0),
        zero,
        zero,
        40.0,
        out,
    );
    let grid = [
        vec3(10.0, 10.0, 10.0),
        vec3(120.0, 40.0, 30.0),
        vec3(200.0, 200.0, 200.0),
    ];
    for g in grid {
        let t = unsafe { wind::get_total_wind(&SVC, g) };
        out.push_str(&format!("WTOTAL{} =>{}\n", vb(g), vb(t)));
    }
    rig.end(out);
}

#[test]
fn reach_and_wind_match_original_fixture() {
    // Reach objects: four actors, one scout pawn on its own level.
    let mut actors: Vec<Box<AActor>> = (0..4)
        .map(|_| Box::new(unsafe { std::mem::zeroed::<AActor>() }))
        .collect();
    let acts: [*mut AActor; 4] = [
        &mut *actors[0] as *mut AActor,
        &mut *actors[1] as *mut AActor,
        &mut *actors[2] as *mut AActor,
        &mut *actors[3] as *mut AActor,
    ];
    let mut level = Box::new(unsafe { std::mem::zeroed::<ULevel>() });
    let mut scout = Box::new(unsafe { std::mem::zeroed::<APawn>() });
    scout.base.x_level = &mut *level as *mut ULevel as *mut c_void;

    let mut out = String::new();
    run_reach(&mut out, &acts, &mut *scout as *mut APawn);
    run_wind(&mut out);

    let golden = include_str!("fixtures/orig_reach_wind.txt");
    let got: Vec<&str> = out.lines().collect();
    let want: Vec<&str> = golden.lines().collect();
    assert_eq!(got.len(), want.len(), "line count differs");
    for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(g, w, "fixture line {}", i + 1);
    }
}
