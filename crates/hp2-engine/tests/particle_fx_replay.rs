#![allow(unsafe_op_in_unsafe_fn)]
//! Replay of the C++ particle-FX fixture harness (`Build/particle_fx_fixture_harness`).
//!
//! The C++ harness is the only generator. Each fixture `case` line carries the knobs,
//! the full AParticleFX state and the driver inputs. This test rebuilds the object from
//! those tokens, prints the same case line from the rebuilt state, drives the Rust port
//! through the same call sequence the harness uses, and compares every output line.
//! There is no Rust RNG: libc `srand(seed)` is called per case exactly as the harness
//! does, and the `probe rand` record checks the stream.
//!
//! The C++ shims (`Hp2ParticleFxServices.cpp`) are reproduced by the `m_*` mocks below.
//! ObjectFlags, LODBias and GIsEditor are not in the generated ABI, so they live in the
//! thread-local `ENV` (divergence, documented in the port report).

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::fmt::Write as _;
use std::ptr::null_mut;
use std::sync::LazyLock;

use hp2_abi::engine::generated::{
    AActor, AParticleFX, FBox, FColorParams, FFloatParams, FRotator, UParticle,
};
use hp2_abi::render_scene::FSceneNode;
use hp2_abi::{FColor, FCoords, FPlane, FVector};
use hp2_engine::particle::{Hp2ParticleServices, ParticleList};
use hp2_engine::particle_fx::{self, FParams, Hp2ParticleFxServices};

const EMIT: &str = include_str!("fixtures/particle_fx_emit.txt");
const BBOX: &str = include_str!("fixtures/particle_fx_bbox.txt");
const UPDATE: &str = include_str!("fixtures/particle_fx_update.txt");
const PARAMS: &str = include_str!("fixtures/particle_fx_params.txt");
const TRIG: &str = include_str!("fixtures/particle_trig.txt");
const SQRT: &str = include_str!("fixtures/particle_fx_sqrt.txt");

unsafe extern "C" {
    fn rand() -> i32;
    fn srand(seed: u32);
}

const RAND_MAX_F: f32 = 0x7fff_ffff as f32;
const PI_F: f32 = std::f32::consts::PI;

// Sentinel objects standing in for engine objects. Only their addresses matter.
static LEVEL_S: [u8; 1] = [1];
static ZONE_S: [u8; 1] = [2];
static MESH_S: [u8; 1] = [3];
static GESTURE_S: [u8; 1] = [4];
static TEX_WITH_S: [u8; 1] = [5];
static TEX_WITHOUT_S: [u8; 1] = [6];

fn sentinel(p: &'static [u8; 1]) -> *mut c_void {
    p.as_ptr() as *mut c_void
}
fn level_ptr() -> *mut c_void {
    sentinel(&LEVEL_S)
}
fn zone_ptr() -> *mut c_void {
    sentinel(&ZONE_S)
}
fn mesh_ptr() -> *mut c_void {
    sentinel(&MESH_S)
}
fn gesture_ptr() -> *mut c_void {
    sentinel(&GESTURE_S)
}
fn tex_with() -> *mut c_void {
    sentinel(&TEX_WITH_S)
}
fn tex_without() -> *mut c_void {
    sentinel(&TEX_WITHOUT_S)
}

// ---------------------------------------------------------------------------
// Thread-local environment (the C++ globals the mocks read and write).
// ---------------------------------------------------------------------------

struct Env {
    trace: Cell<bool>,
    cycles: Cell<u64>,
    destroys: Cell<u32>,
    gstat_emitted: Cell<i64>,
    editor: Cell<bool>,
    level_time: Cell<f32>,
    level_zg: Cell<[f32; 3]>,
    lod_bias: Cell<f32>,
    fx_flags: Cell<u32>,
    owner_flags: Cell<u32>,
    hit_kind: Cell<i64>,
    /// The C++ `UParticleList::WorldBox`, refreshed only by `UpdateBox()`.
    world_box: Cell<FBox>,
    parent: Cell<usize>,
    svc: Cell<usize>,
    log: RefCell<Vec<String>>,
    lines: RefCell<Vec<String>>,
    records: Cell<usize>,
    cases: Cell<usize>,
}

fn fbox_zero() -> FBox {
    // FBox(0): empty, IsValid = 0. The C++ UParticleList ctor sets WorldBox = FBox(0).
    unsafe { std::mem::zeroed() }
}

thread_local! {
    static ENV: Env = Env {
        trace: Cell::new(false),
        cycles: Cell::new(0),
        destroys: Cell::new(0),
        gstat_emitted: Cell::new(0),
        editor: Cell::new(false),
        level_time: Cell::new(0.0),
        level_zg: Cell::new([0.0; 3]),
        lod_bias: Cell::new(0.0),
        fx_flags: Cell::new(0),
        owner_flags: Cell::new(0),
        hit_kind: Cell::new(0),
        world_box: Cell::new(fbox_zero()),
        parent: Cell::new(0),
        svc: Cell::new(0),
        log: RefCell::new(Vec::new()),
        lines: RefCell::new(Vec::new()),
        records: Cell::new(0),
        cases: Cell::new(0),
    };
}

fn env<R>(f: impl FnOnce(&Env) -> R) -> R {
    ENV.with(|e| f(e))
}

fn trace_get() -> bool {
    env(|e| e.trace.get())
}

fn trace_set(v: bool) {
    env(|e| e.trace.set(v))
}

fn log_if_trace(msg: String) {
    env(|e| {
        if e.trace.get() {
            e.log.borrow_mut().push(msg);
        }
    })
}

fn log_always(msg: String) {
    env(|e| e.log.borrow_mut().push(msg))
}

/// Counted record: flushes pending log lines, then the record itself.
fn rec(line: String) {
    env(|e| {
        let pending: Vec<String> = e.log.borrow_mut().drain(..).collect();
        let mut out = e.lines.borrow_mut();
        out.extend(pending);
        out.push(line);
        e.records.set(e.records.get() + 1);
    })
}

fn emit_case_line(line: String) {
    env(|e| {
        e.lines.borrow_mut().push(line);
        e.cases.set(e.cases.get() + 1);
    })
}

fn end_case_line(line: String) {
    env(|e| {
        let pending: Vec<String> = e.log.borrow_mut().drain(..).collect();
        let mut out = e.lines.borrow_mut();
        out.extend(pending);
        out.push(line);
    })
}

fn gstat() -> i64 {
    env(|e| e.gstat_emitted.get())
}

fn app_cycles() {
    let n = env(|e| {
        let n = e.cycles.get() + 1;
        e.cycles.set(n);
        n
    });
    log_if_trace(format!("stat cycle {n}"));
}

// ---------------------------------------------------------------------------
// Fixture tables (TrigFLOAT[16384] then SqrtManTbl[2048]).
// ---------------------------------------------------------------------------

struct Tables {
    trig: Vec<f32>,
    sqrt: Vec<u32>,
}

static TABLES: LazyLock<Tables> = LazyLock::new(load_tables);

fn tables() -> &'static Tables {
    &TABLES
}

fn load_tables() -> Tables {
    let trig: Vec<f32> = TRIG
        .lines()
        .map(|l| u32::from_str_radix(l.trim(), 16).expect("hex trig word"))
        .map(f32::from_bits)
        .collect();
    assert_eq!(trig.len(), 16384, "trig fixture word count");
    assert_eq!(trig[0].to_bits(), 0, "TrigFLOAT[0] must be 0");
    assert_eq!(trig[4096], 1.0, "TrigFLOAT[PI/2] must be 1");
    let sqrt: Vec<u32> = SQRT
        .lines()
        .map(|l| u32::from_str_radix(l.trim(), 16).expect("hex sqrt word"))
        .collect();
    Tables { trig, sqrt }
}

// ---------------------------------------------------------------------------
// Token reader and formatting helpers (mirror the harness printers).
// ---------------------------------------------------------------------------

struct Toks<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Toks<'a> {
    fn new(line: &'a str) -> Self {
        Self {
            t: line.split_whitespace().collect(),
            i: 0,
        }
    }
    fn s(&mut self) -> &'a str {
        let v = self.t[self.i];
        self.i += 1;
        v
    }
    fn int(&mut self) -> i64 {
        self.s().parse().expect("integer token")
    }
    fn hex(&mut self) -> u64 {
        u64::from_str_radix(self.s(), 16).expect("hex token")
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.hex() as u32)
    }
    fn fv(&mut self) -> FVector {
        let x = self.f();
        let y = self.f();
        let z = self.f();
        FVector { x, y, z }
    }
    fn ff(&mut self) -> FFloatParams {
        let base = self.f();
        let rand = self.f();
        FFloatParams { base, rand }
    }
    fn col(&mut self) -> [u8; 4] {
        let a = self.int() as u8;
        let b = self.int() as u8;
        let c = self.int() as u8;
        let d = self.int() as u8;
        [a, b, c, d]
    }
}

fn pf(s: &mut String, v: f32) {
    let _ = write!(s, " {:08x}", v.to_bits());
}
fn pv(s: &mut String, v: &FVector) {
    pf(s, v.x);
    pf(s, v.y);
    pf(s, v.z);
}
fn pp(s: &mut String, v: &FPlane) {
    pf(s, v.x);
    pf(s, v.y);
    pf(s, v.z);
    pf(s, v.w);
}
fn pff(s: &mut String, p: &FFloatParams) {
    pf(s, p.base);
    pf(s, p.rand);
}
fn pcol(s: &mut String, c: [u8; 4]) {
    let _ = write!(s, " {} {} {} {}", c[0], c[1], c[2], c[3]);
}
fn pi(s: &mut String, v: i64) {
    let _ = write!(s, " {}", v);
}
fn fcol(c: FColor) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

fn put_particle(s: &mut String, p: &UParticle) {
    pv(s, &p.position);
    pv(s, &p.velocity);
    pf(s, p.lifetime);
    pf(s, p.age);
    pf(s, p.alpha);
    pf(s, p.alpha_full);
    pf(s, p.alpha_delta);
    pp(s, &p.color);
    pp(s, &p.color_delta);
    pf(s, p.width);
    pf(s, p.width_full);
    pf(s, p.width_delta);
    pf(s, p.length);
    pf(s, p.length_full);
    pf(s, p.length_delta);
    pf(s, p.tail);
    pf(s, p.drip_timer);
    pf(s, p.chaos_delay);
    pf(s, p.spin);
    pf(s, p.spin_rate);
    let _ = write!(s, " {} {}", p.id, p.priority_tag);
}

unsafe fn put_found(s: &mut String, p: *mut UParticle) {
    if p.is_null() {
        s.push_str(" -");
    } else {
        s.push(' ');
        put_particle(s, &*p);
    }
}

unsafe fn put_fparams(s: &mut String, p: &FParams) {
    pf(s, p.particles_per_sec);
    pf(s, p.source_width);
    pf(s, p.source_height);
    pf(s, p.source_depth);
    pf(s, p.period);
    pf(s, p.decay);
    pf(s, p.angular_spread_width);
    pf(s, p.angular_spread_height);
    pf(s, p.speed);
    pf(s, p.lifetime);
    pf(s, p.alpha_start);
    pf(s, p.alpha_end);
    pf(s, p.size_width);
    pf(s, p.size_length);
    pf(s, p.size_end_scale);
    pf(s, p.spin_rate);
    pf(s, p.drip_time);
    s.push(' ');
    pcol(s, fcol(p.color_start));
    s.push(' ');
    pcol(s, fcol(p.color_end));
}

unsafe fn put_sys_params(s: &mut String, f: &AParticleFX) {
    pff(s, &f.angular_spread_width);
    pff(s, &f.angular_spread_height);
    pff(s, &f.source_width);
    pff(s, &f.source_depth);
    pff(s, &f.source_height);
    pff(s, &f.speed);
    pff(s, &f.lifetime);
    pff(s, &f.alpha_start);
    pff(s, &f.alpha_end);
    pff(s, &f.spin_rate);
    pff(s, &f.drip_time);
    pff(s, &f.size_width);
    pff(s, &f.size_length);
    pff(s, &f.size_end_scale);
    s.push(' ');
    pcol(s, f.color_start.base);
    s.push(' ');
    pcol(s, f.color_start.rand);
    s.push(' ');
    pcol(s, f.color_end.base);
    s.push(' ');
    pcol(s, f.color_end.rand);
}

/// `put_fx_state` of the harness, in the same field order.
unsafe fn put_state(
    s: &mut String,
    fx: *const AParticleFX,
    fx_flags: u32,
    owner_flags: u32,
    lod_bias: f32,
) {
    let f = &*fx;
    let _ = write!(s, " {:08x}", fx_flags);
    if f.base.owner.is_null() {
        s.push_str(" 0");
    } else {
        s.push_str(" 1");
        let _ = write!(s, " {:08x}", owner_flags);
        pv(s, &(*(f.base.owner as *const AActor)).velocity);
    }
    pf(s, lod_bias);
    pf(s, f.base.draw_scale);
    pi(s, i64::from(f.base.draw_type));
    pi(s, i64::from(f.distribution));
    pff(s, &f.particles_per_sec);
    pff(s, &f.period);
    pff(s, &f.source_width);
    pff(s, &f.source_height);
    pff(s, &f.source_depth);
    pff(s, &f.decay);
    pff(s, &f.angular_spread_width);
    pff(s, &f.angular_spread_height);
    pff(s, &f.speed);
    pff(s, &f.lifetime);
    pff(s, &f.alpha_start);
    pff(s, &f.alpha_end);
    pff(s, &f.size_width);
    pff(s, &f.size_length);
    pff(s, &f.size_end_scale);
    pff(s, &f.spin_rate);
    pff(s, &f.drip_time);
    s.push(' ');
    pcol(s, f.color_start.base);
    s.push(' ');
    pcol(s, f.color_start.rand);
    s.push(' ');
    pcol(s, f.color_end.base);
    s.push(' ');
    pcol(s, f.color_end.rand);
    pv(s, &f.gravity);
    pf(s, f.damping);
    pf(s, f.wind_modifier);
    pf(s, f.gravity_modifier);
    pf(s, f.parent_blend);
    pf(s, f.chaos);
    pf(s, f.chaos_delay);
    pf(s, f.elasticity);
    pf(s, f.alpha_delay);
    pf(s, f.color_delay);
    pf(s, f.size_delay);
    pf(s, f.alpha_grow_period);
    pf(s, f.size_grow_period);
    pf(s, f.emit_delay);
    pf(s, f.emission_residue);
    pi(s, i64::from(f.particles_max));
    let palette = if f.color_palette.is_null() {
        0
    } else if f.color_palette == tex_without() {
        1
    } else {
        2
    };
    pi(s, palette);
    pi(s, i64::from(f.base.region.zone == zone_ptr()));
    pi(s, i64::from(f.pattern == gesture_ptr()));
    pi(s, i64::from(f.b_prime()));
    pi(s, i64::from(f.b_system_relative()));
    pi(s, i64::from(f.b_velocity_relative()));
    pi(s, i64::from(f.b_wind_per_particle()));
    pi(s, i64::from(f.b_emit()));
    pi(s, i64::from(f.b_shell_only()));
    pi(s, i64::from(f.b_steady_state()));
    pi(s, i64::from(f.b_update()));
    pi(s, i64::from(!f.particle_list.is_null()));
    pi(s, i64::from(f.render_primitive));
    pf(s, f.lod);
    pf(s, f.elapsed_time);
    pf(s, f.age);
    pi(s, i64::from(f.particles_emitted));
    pi(s, i64::from(f.particles_alive));
    pi(s, i64::from(f.current_priority_tag));
    pv(s, &f.last_update_location);
    pv(s, &f.last_emit_location);
    pv(s, &f.base.location);
}

// ---------------------------------------------------------------------------
// Case knobs, state parsing and object construction.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct Extra {
    fx_flags: u32,
    owner_flags: u32,
    lod_bias: f32,
}

#[derive(Clone, Copy)]
struct Knobs {
    seed: u64,
    editor: bool,
    level_time: f32,
    zone: FVector,
    hit_kind: i64,
    base_kind: i64,
}

fn parse_knobs(t: &mut Toks) -> Knobs {
    let seed = t.hex();
    let editor = t.int() != 0;
    let level_time = t.f();
    let zone = t.fv();
    let hit_kind = t.int();
    let base_kind = t.int();
    Knobs {
        seed,
        editor,
        level_time,
        zone,
        hit_kind,
        base_kind,
    }
}

fn put_knobs(s: &mut String, k: &Knobs) {
    let _ = write!(s, " {:016x} {}", k.seed, i32::from(k.editor));
    pf(s, k.level_time);
    pv(s, &k.zone);
    let _ = write!(s, " {} {}", k.hit_kind, k.base_kind);
}

struct Mem {
    fx: Box<AParticleFX>,
    owner: Option<Box<AActor>>,
    other: Option<Box<AActor>>,
    list: Option<Box<ParticleList>>,
}

impl Mem {
    fn new() -> Self {
        Self {
            fx: Box::new(unsafe { std::mem::zeroed::<AParticleFX>() }),
            owner: None,
            other: None,
            list: None,
        }
    }
    fn fxp(&mut self) -> *mut AParticleFX {
        &mut *self.fx as *mut AParticleFX
    }
}

fn build_mem(k: &Knobs) -> Mem {
    let mut m = Mem::new();
    if k.base_kind == 2 {
        m.other = Some(Box::new(unsafe { std::mem::zeroed::<AActor>() }));
    }
    m
}

/// Parses the `put_fx_state` tokens into `m`, returning the non-ABI extras.
unsafe fn parse_state(t: &mut Toks, m: &mut Mem, base_kind: i64) -> Extra {
    let mut ex = Extra {
        fx_flags: t.hex() as u32,
        ..Default::default()
    };
    if t.int() == 1 {
        ex.owner_flags = t.hex() as u32;
        let mut owner: Box<AActor> = Box::new(std::mem::zeroed::<AActor>());
        owner.velocity = t.fv();
        owner.mesh = mesh_ptr();
        m.fx.base.owner = (&mut *owner as *mut AActor).cast();
        m.owner = Some(owner);
    }
    ex.lod_bias = t.f();
    let fx = m.fxp();
    (*fx).base.draw_scale = t.f();
    (*fx).base.draw_type = t.int() as u8;
    (*fx).distribution = t.int() as u8;
    (*fx).particles_per_sec = t.ff();
    (*fx).period = t.ff();
    (*fx).source_width = t.ff();
    (*fx).source_height = t.ff();
    (*fx).source_depth = t.ff();
    (*fx).decay = t.ff();
    (*fx).angular_spread_width = t.ff();
    (*fx).angular_spread_height = t.ff();
    (*fx).speed = t.ff();
    (*fx).lifetime = t.ff();
    (*fx).alpha_start = t.ff();
    (*fx).alpha_end = t.ff();
    (*fx).size_width = t.ff();
    (*fx).size_length = t.ff();
    (*fx).size_end_scale = t.ff();
    (*fx).spin_rate = t.ff();
    (*fx).drip_time = t.ff();
    let cs_base = t.col();
    let cs_rand = t.col();
    let ce_base = t.col();
    let ce_rand = t.col();
    (*fx).color_start = FColorParams {
        base: cs_base,
        rand: cs_rand,
    };
    (*fx).color_end = FColorParams {
        base: ce_base,
        rand: ce_rand,
    };
    (*fx).gravity = t.fv();
    (*fx).damping = t.f();
    (*fx).wind_modifier = t.f();
    (*fx).gravity_modifier = t.f();
    (*fx).parent_blend = t.f();
    (*fx).chaos = t.f();
    (*fx).chaos_delay = t.f();
    (*fx).elasticity = t.f();
    (*fx).alpha_delay = t.f();
    (*fx).color_delay = t.f();
    (*fx).size_delay = t.f();
    (*fx).alpha_grow_period = t.f();
    (*fx).size_grow_period = t.f();
    (*fx).emit_delay = t.f();
    (*fx).emission_residue = t.f();
    (*fx).particles_max = t.int() as i32;
    (*fx).color_palette = match t.int() {
        0 => null_mut(),
        1 => tex_without(),
        _ => tex_with(),
    };
    if t.int() == 1 {
        (*fx).base.region.zone = zone_ptr();
    }
    if t.int() == 1 {
        (*fx).pattern = gesture_ptr();
    }
    let b_prime = t.int() != 0;
    let b_system_relative = t.int() != 0;
    let b_velocity_relative = t.int() != 0;
    let b_wind_per_particle = t.int() != 0;
    let b_emit = t.int() != 0;
    let b_shell_only = t.int() != 0;
    let b_steady_state = t.int() != 0;
    let b_update = t.int() != 0;
    (*fx).set_b_prime(b_prime);
    (*fx).set_b_system_relative(b_system_relative);
    (*fx).set_b_velocity_relative(b_velocity_relative);
    (*fx).set_b_wind_per_particle(b_wind_per_particle);
    (*fx).set_b_emit(b_emit);
    (*fx).set_b_shell_only(b_shell_only);
    (*fx).set_b_steady_state(b_steady_state);
    (*fx).set_b_update(b_update);
    if t.int() == 1 {
        let mut l = Box::new(ParticleList::new());
        (*fx).particle_list = (&mut *l as *mut ParticleList).cast();
        m.list = Some(l);
    }
    (*fx).render_primitive = t.int() as u8;
    (*fx).lod = t.f();
    (*fx).elapsed_time = t.f();
    (*fx).age = t.f();
    (*fx).particles_emitted = t.int() as i32;
    (*fx).particles_alive = t.int() as i32;
    (*fx).current_priority_tag = t.int() as i32;
    (*fx).last_update_location = t.fv();
    (*fx).last_emit_location = t.fv();
    (*fx).base.location = t.fv();

    (*fx).base.level = level_ptr();
    (*fx).base.x_level = level_ptr();
    let owner = (*fx).base.owner;
    (*fx).base.base = match base_kind {
        1 => {
            assert!(!owner.is_null(), "base kind 1 requires an owner");
            owner
        }
        2 => m
            .other
            .as_deref_mut()
            .map(|a| (a as *mut AActor).cast())
            .expect("base kind 2 requires the other actor"),
        _ => null_mut(),
    };
    ex
}

unsafe fn case_head(
    fam: &str,
    name: &str,
    k: &Knobs,
    fx: *const AParticleFX,
    ex: &Extra,
) -> String {
    let mut s = format!("case {fam} {name}");
    put_knobs(&mut s, k);
    put_state(&mut s, fx, ex.fx_flags, ex.owner_flags, ex.lod_bias);
    s
}

unsafe fn end_case(fam: &str, name: &str, fx: *const AParticleFX) {
    let owner_flags = if (*fx).base.owner.is_null() {
        0
    } else {
        env(|e| e.owner_flags.get())
    };
    let (destroys, editor, fx_flags) =
        env(|e| (e.destroys.get(), e.editor.get(), e.fx_flags.get()));
    end_case_line(format!(
        "end {fam} {name} destroys={destroys} editor={} fx_flags={fx_flags:08x} owner_flags={owner_flags:08x}",
        i32::from(editor)
    ));
}

fn begin_case(k: &Knobs, ex: &Extra) {
    unsafe { srand(k.seed as u32) };
    env(|e| {
        e.trace.set(false);
        e.destroys.set(0);
        e.world_box.set(fbox_zero());
        e.cycles.set(0);
        e.editor.set(k.editor);
        e.level_time.set(k.level_time);
        e.level_zg.set([k.zone.x, k.zone.y, k.zone.z]);
        e.hit_kind.set(k.hit_kind);
        e.fx_flags.set(ex.fx_flags);
        e.owner_flags.set(ex.owner_flags);
        e.lod_bias.set(ex.lod_bias);
    });
}

fn frame_node() -> FSceneNode {
    let mut f: FSceneNode = unsafe { std::mem::zeroed() };
    f.proj = FVector {
        x: 0.0,
        y: 0.0,
        z: 2.0,
    };
    f.fx = 256.0;
    f.fy = 128.0;
    f
}

// ---------------------------------------------------------------------------
// C++ shim mocks (Hp2ParticleFxServices.cpp / UnAnsi.cpp / UnMath.h).
// ---------------------------------------------------------------------------

unsafe fn svc() -> &'static Hp2ParticleFxServices {
    &*(env(|e| e.svc.get()) as *const Hp2ParticleFxServices)
}

fn rand_unit() -> f32 {
    unsafe { rand() as f32 / RAND_MAX_F }
}

/// `appRandRange(float)` = `fmadd(Max-Min, appFrand(), Min)` (UnAnsi.cpp:1984, fused).
unsafe fn app_rand_range_float(min: f32, max: f32) -> f32 {
    (max - min).mul_add(m_app_frand(), min)
}

unsafe extern "C" fn m_app_frand() -> f32 {
    rand_unit()
}

/// `appFrand(From, To)` = `(To-From)*r + From`.
unsafe extern "C" fn m_app_frand_range(from: f32, to: f32) -> f32 {
    let r = rand_unit();
    (to - from).mul_add(r, from)
}

unsafe extern "C" fn m_app_rand_range_int(min: i32, max: i32) -> i32 {
    let range = max - min;
    let r = if range > 0 { rand() % range } else { 0 };
    r + min
}

unsafe extern "C" fn m_app_exp(x: f64) -> f64 {
    x.exp()
}

unsafe extern "C" fn m_sqrt_approx(f: f32) -> f32 {
    let bits = f.to_bits();
    let a = ((bits >> 13) & 2047) as usize;
    let b = bits & 0x7f00_0000;
    f32::from_bits(tables().sqrt[a].wrapping_add(b >> 1))
}

unsafe extern "C" fn m_sin_tab(i: i32) -> f32 {
    tables().trig[((i >> 2) & 16383) as usize]
}

unsafe extern "C" fn m_cos_tab(i: i32) -> f32 {
    tables().trig[((i.wrapping_add(16384) >> 2) & 16383) as usize]
}

unsafe extern "C" fn m_sin_float(f: f32) -> f32 {
    m_sin_tab(((f * 65536.0_f32) / (2.0_f32 * PI_F)) as i32)
}

unsafe extern "C" fn m_cos_float(f: f32) -> f32 {
    m_cos_tab(((f * 65536.0_f32) / (2.0_f32 * PI_F)) as i32)
}

// `FColor::Plane()` is declared `FVector` and returns `FPlane(...)`; assigning that to an
// `FPlane` goes through `FPlane(const FVector&)`, which sets `W(0)`. So alpha is always 0 here.
fn palette_entry(k: usize) -> FPlane {
    FPlane {
        x: ((7 * k + 1) as u8) as f32 / 255.0,
        y: ((3 * k + 2) as u8) as f32 / 255.0,
        z: ((5 * k + 3) as u8) as f32 / 255.0,
        w: 0.0,
    }
}

unsafe extern "C" fn m_palette_color0(tex: *mut c_void, out: *mut c_void) -> i32 {
    if tex == tex_with() {
        *(out as *mut FPlane) = palette_entry(0);
        1
    } else {
        0
    }
}

unsafe extern "C" fn m_palette_plane(
    tex: *mut c_void,
    age: f32,
    lifetime: f32,
    color_delay: f32,
    out: *mut c_void,
) {
    if tex != tex_with() {
        return;
    }
    let idx = if lifetime > 0.0 && age > color_delay {
        ((age / lifetime) * 255.0_f32) as i32
    } else {
        0
    };
    assert!((0..256).contains(&idx), "palette index {idx} out of range");
    *(out as *mut FPlane) = palette_entry(idx as usize);
}

unsafe extern "C" fn m_total_wind(_level: *mut c_void, loc: *const c_void, out: *mut c_void) {
    let l = *(loc as *const FVector);
    *(out as *mut FVector) = FVector {
        x: l.y * 0.5,
        y: -l.x * 0.25,
        z: 3.0,
    };
}

unsafe extern "C" fn m_fast_line_check(
    _level: *mut c_void,
    end: *const c_void,
    start: *const c_void,
) -> i32 {
    let e = *(end as *const FVector);
    let s = *(start as *const FVector);
    ((((e.x as i32) ^ (s.y as i32)) & 3) != 0) as i32
}

unsafe extern "C" fn m_system_trace_level_hit(
    _system: *mut c_void,
    end: *const c_void,
    start: *const c_void,
    normal: *mut c_void,
    location: *mut c_void,
) -> i32 {
    let e = *(end as *const FVector);
    let s = *(start as *const FVector);
    let sel = ((e.y as i32) ^ (s.z as i32)) & 3;
    let hit = env(|x| x.hit_kind.get()) == 0 && (sel == 1 || sel == 3);
    if !hit {
        return 0;
    }
    let n = match (e.x as i32) & 3 {
        0 => FVector {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        1 => FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
        2 => FVector {
            x: 0.0,
            y: -1.0,
            z: 0.0,
        },
        _ => FVector {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    };
    *(normal as *mut FVector) = n;
    *(location as *mut FVector) = FVector {
        x: e.x * 0.5,
        y: e.y * 0.5,
        z: e.z * 0.5,
    };
    1
}

unsafe extern "C" fn m_level_destroy_actor(_level: *mut c_void) {
    env(|e| e.destroys.set(e.destroys.get() + 1));
    log_if_trace("destroy".to_string());
}

unsafe extern "C" fn m_stat_clock_update() {
    app_cycles();
}

unsafe extern "C" fn m_stat_unclock_update() {
    app_cycles();
}

unsafe extern "C" fn m_stat_particles_emitted(n: i32) {
    env(|e| e.gstat_emitted.set(e.gstat_emitted.get() + i64::from(n)));
}

unsafe extern "C" fn m_actor_to_local(actor: *mut c_void, out: *mut c_void) {
    let a = &*(actor as *const AActor);
    let rot = &a.rotation as *const FRotator as *const u8;
    let bytes = std::slice::from_raw_parts(rot, std::mem::size_of::<FRotator>());
    assert!(
        bytes.iter().all(|b| *b == 0),
        "replay assumes zero actor rotation"
    );
    let c = &mut *(out as *mut FCoords);
    c.origin = a.location;
    c.x_axis = FVector {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };
    c.y_axis = FVector {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    c.z_axis = FVector {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };
}

unsafe extern "C" fn m_actor_zone_gravity(actor: *mut c_void, out: *mut c_void) {
    let a = &*(actor as *const AActor);
    let g = if a.region.zone.is_null() {
        let z = env(|e| e.level_zg.get());
        FVector {
            x: z[0],
            y: z[1],
            z: z[2],
        }
    } else {
        FVector {
            x: 0.0,
            y: 0.0,
            z: -50.0,
        }
    };
    *(out as *mut FVector) = g;
}

unsafe extern "C" fn m_actor_lod_bias(_actor: *mut c_void) -> f32 {
    env(|e| e.lod_bias.get())
}

unsafe extern "C" fn m_level_time_seconds(_level: *mut c_void) -> f64 {
    f64::from(env(|e| e.level_time.get()))
}

unsafe extern "C" fn m_is_editor() -> i32 {
    i32::from(env(|e| e.editor.get()))
}

unsafe extern "C" fn m_sync_owner_transient(fx: *mut c_void) {
    let f = &*(fx as *const AParticleFX);
    let owner = f.base.owner;
    let transient = env(|e| e.owner_flags.get()) & 0x4000 != 0;
    if !owner.is_null() && owner == f.base.base && transient {
        env(|e| e.fx_flags.set(e.fx_flags.get() | 0x4000));
    }
}

unsafe extern "C" fn m_parent_defaults(_fx: *mut c_void) -> *mut c_void {
    env(|e| e.parent.get()) as *mut c_void
}

unsafe extern "C" fn m_mesh_render_box(_mesh: *mut c_void, out: *mut c_void) {
    let c = &mut *(out as *mut FCoords);
    c.origin = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    c.x_axis = FVector {
        x: 30.0,
        y: 0.0,
        z: 0.0,
    };
    c.y_axis = FVector {
        x: 0.0,
        y: 20.0,
        z: 0.0,
    };
    c.z_axis = FVector {
        x: 0.0,
        y: 0.0,
        z: 10.0,
    };
}

unsafe extern "C" fn m_mesh_frame_verts(_mesh: *mut c_void) -> i32 {
    8
}

unsafe extern "C" fn m_mesh_get_frame(_mesh: *mut c_void, out: *mut c_void) {
    let v = out as *mut FVector;
    for k in 0..8 {
        let kf = k as f32;
        *v.add(k) = FVector {
            x: 1.5 * kf,
            y: -0.5 * kf,
            z: 2.0 * kf + 1.0,
        };
    }
}

unsafe extern "C" fn m_mesh_num_tris(_mesh: *mut c_void) -> i32 {
    6
}

unsafe extern "C" fn m_mesh_tri_verts(_mesh: *mut c_void, t: i32, out: *mut i32) {
    *out = t % 8;
    *out.add(1) = (t + 1) % 8;
    *out.add(2) = (t + 2) % 8;
}

unsafe extern "C" fn m_pattern_num_points(_pattern: *mut c_void) -> i32 {
    4
}

unsafe extern "C" fn m_pattern_point(_pattern: *mut c_void, i: i32, out: *mut c_void) {
    let pts = [
        FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        FVector {
            x: 0.5,
            y: 0.0,
            z: 0.0,
        },
        FVector {
            x: 0.5,
            y: 1.0,
            z: 0.0,
        },
        FVector {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
    ];
    *(out as *mut FVector) = pts[i as usize];
}

unsafe extern "C" fn m_list_start(list: *mut c_void) -> *mut c_void {
    (*(list as *mut ParticleList)).start() as *mut c_void
}

unsafe extern "C" fn m_list_next(list: *mut c_void) -> *mut c_void {
    (*(list as *mut ParticleList)).next_particle() as *mut c_void
}

unsafe extern "C" fn m_list_add(list: *mut c_void, p: *const c_void) {
    (*(list as *mut ParticleList)).add(&*(p as *const UParticle));
}

unsafe extern "C" fn m_list_remove_current(list: *mut c_void) {
    (*(list as *mut ParticleList)).remove_current();
}

unsafe extern "C" fn m_list_remove_all(list: *mut c_void) {
    (*(list as *mut ParticleList)).remove_all();
}

unsafe extern "C" fn m_list_remove_from_front(list: *mut c_void) {
    (*(list as *mut ParticleList)).remove_from_front();
}

unsafe extern "C" fn m_list_size(list: *mut c_void) -> i32 {
    (*(list as *mut ParticleList)).size() as i32
}

unsafe extern "C" fn m_list_get_particle(list: *mut c_void, i: i32) -> *mut c_void {
    (*(list as *mut ParticleList)).get_particle(i) as *mut c_void
}

unsafe extern "C" fn m_list_update_box(list: *mut c_void) {
    // `UParticleList::UpdateBox()`: recompute from live particles into the stored WorldBox.
    let mut b = fbox_zero();
    (*(list as *mut ParticleList)).update_box(&mut b);
    env(|e| e.world_box.set(b));
}

unsafe extern "C" fn m_list_world_box(_list: *mut c_void, out: *mut c_void) {
    *(out as *mut FBox) = env(|e| e.world_box.get());
}

/// `AParticleFX::EmitParticles` = clock pair around `hp2_rs_engine_particle_fx_emit_particles`.
unsafe fn emit_shell(fx: *mut AParticleFX, tick: f32) -> i32 {
    (svc().stat_clock_update)();
    let r = particle_fx::emit_particles(fx, tick, svc());
    (svc().stat_unclock_update)();
    r
}

unsafe extern "C" fn m_emit_particles_shell(fx: *mut c_void, tick: f32) -> i32 {
    emit_shell(fx as *mut AParticleFX, tick)
}

unsafe extern "C" fn m_update_shell(fx: *mut c_void, step: f32) -> i32 {
    particle_fx::update(fx as *mut AParticleFX, step, svc()) as i32
}

/// `appRand`-driven probe: 4 raw rand values, then the float helpers, then 1000 `appFrand` samples.
unsafe fn probe_line() -> String {
    srand(1);
    let mut s = String::from("probe rand");
    for _ in 0..4 {
        let _ = write!(s, " {}", rand());
    }
    pf(&mut s, m_app_frand());
    pf(&mut s, m_app_frand_range(-1.0, 2.0));
    pi(&mut s, i64::from(m_app_rand_range_int(3, 11)));
    pf(&mut s, app_rand_range_float(-1.0, 2.0));
    let (mut lo, mut hi) = (1.0_f32, 0.0_f32);
    for _ in 0..1000 {
        let v = m_app_frand();
        if v < lo {
            lo = v;
        }
        if v > hi {
            hi = v;
        }
    }
    pf(&mut s, lo);
    pf(&mut s, hi);
    s
}

// ---------------------------------------------------------------------------
// Drivers, one per family. Each mirrors the corresponding C++ harness driver.
// ---------------------------------------------------------------------------

unsafe fn populate(fx: *mut AParticleFX, calls: usize, tick: f32) {
    let was = trace_get();
    trace_set(false);
    for _ in 0..calls {
        emit_shell(fx, tick);
    }
    trace_set(was);
}

unsafe fn last_particle(list: *mut ParticleList) -> *mut UParticle {
    let mut last = null_mut();
    let mut p = (*list).start();
    while !p.is_null() {
        last = p;
        p = (*list).next_particle();
    }
    last
}

unsafe fn run_emit_case(t: &mut Toks, name: &str, k: &Knobs, m: &mut Mem, ex: &Extra) {
    let fx = m.fxp();
    let ncalls = t.int() as usize;
    let tick = t.f();
    let locs: Vec<FVector> = (0..ncalls).map(|_| t.fv()).collect();
    let mut line = case_head("emit", name, k, fx, ex);
    pi(&mut line, ncalls as i64);
    pf(&mut line, tick);
    for l in &locs {
        pv(&mut line, l);
    }
    emit_case_line(line);

    for (c, loc) in locs.iter().enumerate() {
        (*fx).base.location = *loc;
        let before = (*fx).particles_emitted;
        let gbefore = gstat();
        trace_set(true);
        let ret = emit_shell(fx, tick);
        trace_set(false);
        log_always(format!("stat emitted {}", gstat() - gbefore));
        let nw = i64::from((*fx).particles_emitted - before);
        let list = (*fx).particle_list as *mut ParticleList;
        assert!(!list.is_null(), "emit cases always have a ParticleList");
        let n = (*list).size() as i64;
        let mut idx: i64 = 0;
        let mut p = (*list).start();
        while !p.is_null() {
            if idx >= n - nw {
                let mut s = format!("emit {name} P {c} {idx}");
                put_particle(&mut s, &*p);
                rec(s);
            }
            idx += 1;
            p = (*list).next_particle();
        }
        let mut s = format!("emit {name} S {c} {ret}");
        pf(&mut s, (*fx).emit_delay);
        pf(&mut s, (*fx).emission_residue);
        pi(&mut s, i64::from((*fx).particles_emitted));
        pv(&mut s, &(*fx).last_emit_location);
        let _ = write!(
            s,
            " {} {} {}",
            (*fx).current_priority_tag,
            (*fx).particles_alive,
            n
        );
        rec(s);
    }
    end_case("emit", name, fx);
}

unsafe fn run_bbox_case(t: &mut Toks, name: &str, k: &Knobs, m: &mut Mem, ex: &Extra) {
    let fx = m.fxp();
    let ncalls = t.int() as usize;
    let tick = t.f();
    let mut line = case_head("bbox", name, k, fx, ex);
    pi(&mut line, ncalls as i64);
    pf(&mut line, tick);
    emit_case_line(line);
    populate(fx, ncalls, tick);
    (svc().list_update_box)((*fx).particle_list);
    for exact in 0..2 {
        trace_set(true);
        let mut co: FCoords = std::mem::zeroed();
        particle_fx::get_render_bounding_box(fx, &mut co, svc());
        trace_set(false);
        let mut s = format!("bbox {name} {exact}");
        pv(&mut s, &co.origin);
        pv(&mut s, &co.x_axis);
        pv(&mut s, &co.y_axis);
        pv(&mut s, &co.z_axis);
        pf(&mut s, (*fx).elapsed_time);
        pf(&mut s, (*fx).age);
        pv(&mut s, &(*fx).last_update_location);
        rec(s);
    }
    end_case("bbox", name, fx);
}

unsafe fn rec_update(name: &str, op: &str, ret: &str, arg: f32, fx: *mut AParticleFX) {
    let f = &*fx;
    let list = f.particle_list as *mut ParticleList;
    let mut s = format!("update {name} {op} {ret}");
    pf(&mut s, arg);
    pi(&mut s, (*list).size() as i64);
    pf(&mut s, f.elapsed_time);
    pf(&mut s, f.age);
    pi(&mut s, i64::from(f.particles_emitted));
    let mut p = (*list).start();
    while !p.is_null() {
        let q = &*p;
        pv(&mut s, &q.position);
        pv(&mut s, &q.velocity);
        pf(&mut s, q.age);
        pf(&mut s, q.alpha);
        pf(&mut s, q.width);
        pf(&mut s, q.length);
        pf(&mut s, q.spin);
        pi(&mut s, i64::from(q.id));
        p = (*list).next_particle();
    }
    rec(s);
}

unsafe fn run_update_case(t: &mut Toks, name: &str, k: &Knobs, m: &mut Mem, ex: &Extra) {
    let fx = m.fxp();
    let pop = t.int() as usize;
    let nsteps = t.int() as usize;
    let steps: Vec<(i64, f32, i64)> = (0..nsteps).map(|_| (t.int(), t.f(), t.int())).collect();
    let mut line = case_head("update", name, k, fx, ex);
    let _ = write!(line, " {} {}", pop, nsteps);
    for (kind, arg, tt) in &steps {
        pi(&mut line, *kind);
        pf(&mut line, *arg);
        pi(&mut line, *tt);
    }
    emit_case_line(line);
    populate(fx, pop, 0.1);
    for (kind, arg, tt) in steps {
        trace_set(true);
        let (op, ret) = match kind {
            0 => {
                particle_fx::update_particles(fx, arg, svc());
                ("UP", "-")
            }
            1 => {
                let r = particle_fx::update(fx, arg, svc());
                ("UPD", if r { "1" } else { "0" })
            }
            2 => {
                particle_fx::init_execution(fx, svc());
                ("INIT", "-")
            }
            _ => {
                // AActor::Tick fake returns 1, and AParticleFX::Tick returns Super::Tick.
                particle_fx::tick(fx, arg, tt as i32, svc());
                ("TICK", "1")
            }
        };
        trace_set(false);
        rec_update(name, op, ret, arg, fx);
    }
    end_case("update", name, fx);
}

unsafe fn run_param_mode(name: &str, fx: *mut AParticleFX, mode: i64, arg: i32, arg2: i64) {
    let pos = FVector {
        x: 1.0,
        y: 2.0,
        z: 3.0,
    };
    let frame = frame_node();
    let parent = env(|e| e.parent.get()) as *mut AParticleFX;
    trace_set(true);
    match mode {
        0 => {
            // PM_SYS
            let mut buf: Box<AParticleFX> = Box::new(std::mem::zeroed::<AParticleFX>());
            let bufp: *mut AParticleFX = &mut *buf;
            let r = particle_fx::get_sys_params(fx, bufp, svc());
            trace_set(false);
            let which = if r == fx {
                0
            } else if r == parent {
                1
            } else if r == bufp {
                2
            } else {
                3
            };
            let mut s = format!("params {name} SYS {which} ");
            put_sys_params(&mut s, &*r);
            rec(s);
        }
        1 => {
            // PM_GP
            let mut p: FParams = std::mem::zeroed();
            particle_fx::get_params(fx, &mut p, svc());
            trace_set(false);
            let mut s = format!("params {name} GP ");
            put_fparams(&mut s, &p);
            rec(s);
        }
        2 => {
            // PM_ADD
            let mut p: FParams = std::mem::zeroed();
            particle_fx::get_params(fx, &mut p, svc());
            let added = particle_fx::add_particle(fx, arg, &pos, &p, svc());
            let list = (*fx).particle_list as *mut ParticleList;
            let size = (*list).size();
            trace_set(false);
            let mut s = format!("params {name} ADD {} {size}", i32::from(added));
            put_found(&mut s, last_particle(list));
            rec(s);
            trace_set(true);
            let rr = particle_fx::recompute_deltas(fx, arg, svc());
            trace_set(false);
            let mut s = format!("params {name} RD {} {arg}", i32::from(rr));
            put_found(&mut s, (*list).get_particle(arg));
            rec(s);
        }
        3 => {
            // PM_ADDNULL
            let added = particle_fx::add_particle(fx, arg, &pos, std::ptr::null(), svc());
            let list = (*fx).particle_list as *mut ParticleList;
            let size = (*list).size();
            trace_set(false);
            rec(format!("params {name} ADD {} {size} -", i32::from(added)));
        }
        4 => {
            // PM_RD
            let mut p: FParams = std::mem::zeroed();
            particle_fx::get_params(fx, &mut p, svc());
            particle_fx::add_particle(fx, 1, &pos, &p, svc());
            let list = (*fx).particle_list as *mut ParticleList;
            if arg2 != 0 {
                let p1 = (*list).get_particle(1);
                assert!(!p1.is_null(), "RD mode needs particle 1");
                (*p1).age = (*p1).lifetime + 1.0;
            }
            let rr = particle_fx::recompute_deltas(fx, arg, svc());
            trace_set(false);
            let mut s = format!("params {name} RD {} {arg}", i32::from(rr));
            put_found(&mut s, (*list).get_particle(arg));
            rec(s);
        }
        5 => {
            // PM_LOD
            for fr in [0.0_f32, 0.3, 0.9] {
                let r = particle_fx::lod(fx, fr, svc());
                trace_set(false);
                let mut s = format!("params {name} LOD");
                pf(&mut s, fr);
                pf(&mut s, r);
                rec(s);
                trace_set(true);
            }
            let lp = particle_fx::lod_particles(fx, 0.3, svc());
            trace_set(false);
            let mut s = format!("params {name} LODP");
            pf(&mut s, 0.3);
            pf(&mut s, lp);
            rec(s);
        }
        6 => {
            // PM_DENS
            for fr in [0.0_f32, 0.5] {
                let r = particle_fx::lod_particle_density(fx, &frame, fr, svc());
                let mut s = format!("params {name} DENS");
                pf(&mut s, fr);
                pf(&mut s, r);
                rec(s);
            }
            let lp = particle_fx::lod_particles(fx, 0.5, svc());
            let mut s = format!("params {name} LODP");
            pf(&mut s, 0.5);
            pf(&mut s, lp);
            rec(s);
        }
        7 => {
            // PM_DENSNOLIST
            (*fx).particle_list = null_mut();
            let r = particle_fx::lod_particle_density(fx, &frame, 0.5, svc());
            let mut s = format!("params {name} DENS");
            pf(&mut s, 0.5);
            pf(&mut s, r);
            rec(s);
            let lp = particle_fx::lod_particles(fx, 0.5, svc());
            let mut s = format!("params {name} LODP");
            pf(&mut s, 0.5);
            pf(&mut s, lp);
            rec(s);
        }
        8 => {
            // PM_MAXL
            let r = particle_fx::max_lifetime(fx);
            trace_set(false);
            let mut s = format!("params {name} MAXL");
            pf(&mut s, r);
            rec(s);
        }
        _ => panic!("unknown param mode {mode}"),
    }
    trace_set(false);
}

unsafe fn run_params_case(t: &mut Toks, name: &str, k: &Knobs, m: &mut Mem, ex: &Extra) {
    let fx = m.fxp();
    let mode = t.int();
    let arg = t.int() as i32;
    let arg2 = t.int();
    let pop = t.int() as usize;
    let mut line = case_head("params", name, k, fx, ex);
    let _ = write!(line, " {} {} {} {}", mode, arg, arg2, pop);
    emit_case_line(line);
    populate(fx, pop, 0.1);
    run_param_mode(name, fx, mode, arg, arg2);
    end_case("params", name, fx);
}

// ---------------------------------------------------------------------------
// Family runner and comparison.
// ---------------------------------------------------------------------------

unsafe fn run_case(fam: &str, line: &str) {
    let mut t = Toks::new(line);
    t.s();
    let f = t.s();
    assert_eq!(f, fam, "case family");
    let name = t.s();
    let k = parse_knobs(&mut t);
    let mut m = build_mem(&k);
    let ex = parse_state(&mut t, &mut m, k.base_kind);
    begin_case(&k, &ex);
    match fam {
        "emit" => run_emit_case(&mut t, name, &k, &mut m, &ex),
        "bbox" => run_bbox_case(&mut t, name, &k, &mut m, &ex),
        "update" => run_update_case(&mut t, name, &k, &mut m, &ex),
        "params" => run_params_case(&mut t, name, &k, &mut m, &ex),
        _ => panic!("unknown family {fam}"),
    }
}

/// Replays one family; returns the regenerated output lines (header first).
unsafe fn run_family(fam: &str, fixture: &str) -> Vec<String> {
    let mut parent = Mem::new();
    let pfx = parent.fxp();
    env(|e| {
        e.lines.borrow_mut().clear();
        e.log.borrow_mut().clear();
        e.records.set(0);
        e.cases.set(0);
        e.trace.set(false);
        e.parent.set(pfx as usize);
    });

    let defaults = fixture
        .lines()
        .find(|l| l.starts_with("defaults "))
        .expect("defaults line");
    let mut t = Toks::new(defaults);
    t.s();
    let pex = parse_state(&mut t, &mut parent, 0);
    let mut dl = String::from("defaults");
    put_state(&mut dl, pfx, pex.fx_flags, pex.owner_flags, pex.lod_bias);
    env(|e| e.lines.borrow_mut().push(dl));

    if fam == "emit" {
        rec(probe_line());
    }

    for line in fixture.lines().filter(|l| l.starts_with("case ")) {
        run_case(fam, line);
    }

    let (cases, records, lines) = env(|e| {
        (
            e.cases.get(),
            e.records.get(),
            std::mem::take(&mut *e.lines.borrow_mut()),
        )
    });
    let mut out = vec![format!("header {fam} cases={cases} records={records}")];
    out.extend(lines);
    out
}

fn check(fam: &str, fixture: &str, got: &[String]) {
    let want: Vec<&str> = fixture.lines().collect();
    for (i, g) in got.iter().enumerate() {
        match want.get(i) {
            Some(w) if *w == g.as_str() => {}
            Some(w) => panic!("{fam}: line {} differs\n  want: {w}\n  got:  {g}", i + 1),
            None => panic!("{fam}: extra generated line {}: {g}", i + 1),
        }
    }
    assert_eq!(want.len(), got.len(), "{fam}: line count");
}

#[test]
fn particle_fx_replay_matches_cpp_harness() {
    LazyLock::force(&TABLES);

    let pserv = Hp2ParticleServices {
        app_frand: m_app_frand,
        sin_tab: m_sin_tab,
        cos_tab: m_cos_tab,
        total_wind: m_total_wind,
        fast_line_check: m_fast_line_check,
        system_trace_level_hit: m_system_trace_level_hit,
        palette_plane: m_palette_plane,
    };
    let svc = Hp2ParticleFxServices {
        particle: &pserv,
        app_frand_range: m_app_frand_range,
        app_rand_range_int: m_app_rand_range_int,
        app_exp: m_app_exp,
        sqrt_approx: m_sqrt_approx,
        sin_float: m_sin_float,
        cos_float: m_cos_float,
        actor_to_local: m_actor_to_local,
        actor_zone_gravity: m_actor_zone_gravity,
        actor_lod_bias: m_actor_lod_bias,
        level_time_seconds: m_level_time_seconds,
        level_destroy_actor: m_level_destroy_actor,
        is_editor: m_is_editor,
        sync_owner_transient: m_sync_owner_transient,
        parent_defaults: m_parent_defaults,
        mesh_render_box: m_mesh_render_box,
        mesh_frame_verts: m_mesh_frame_verts,
        mesh_get_frame: m_mesh_get_frame,
        mesh_num_tris: m_mesh_num_tris,
        mesh_tri_verts: m_mesh_tri_verts,
        pattern_num_points: m_pattern_num_points,
        pattern_point: m_pattern_point,
        palette_color0: m_palette_color0,
        list_start: m_list_start,
        list_next: m_list_next,
        list_add: m_list_add,
        list_remove_current: m_list_remove_current,
        list_remove_all: m_list_remove_all,
        list_remove_from_front: m_list_remove_from_front,
        list_size: m_list_size,
        list_get_particle: m_list_get_particle,
        list_update_box: m_list_update_box,
        list_world_box: m_list_world_box,
        emit_particles_shell: m_emit_particles_shell,
        update_shell: m_update_shell,
        stat_clock_update: m_stat_clock_update,
        stat_unclock_update: m_stat_unclock_update,
        stat_particles_emitted: m_stat_particles_emitted,
    };
    env(|e| e.svc.set(&svc as *const Hp2ParticleFxServices as usize));

    unsafe {
        check("emit", EMIT, &run_family("emit", EMIT));
        check("bbox", BBOX, &run_family("bbox", BBOX));
        check("update", UPDATE, &run_family("update", UPDATE));
        check("params", PARAMS, &run_family("params", PARAMS));
    }
}
