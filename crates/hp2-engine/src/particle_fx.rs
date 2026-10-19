//! Port of `AParticleFX` (`UnParticleFX.cpp`, oracle ref 5782e7f).
//!
//! Bit-exact against the C++ oracle built with clang `-ffp-contract=on`, which
//! contracts only within one source expression. Each fused site is written with
//! `mul_add` and carries a `// Fused:` comment. Header-level sites (Size, dot,
//! transform) live in `vec.rs` and `prim.rs`; `cross` is `fpoly::cross`.
//!
//! Stat clocks and counters are kept (`STATS` is defined in `UnBuild.h`). The
//! emit clock is an RAII scope in the C++ shell; the update clock and the
//! emitted counter go through the `stat_*` services. `UParticle`
//! locals are zero-initialised. The oracle zero-fills them through
//! `-ftrivial-auto-var-init=zero`, so `Tail`, `ChaosDelay` and `Spin` in
//! `AddParticle` are zero where the C++ reads uninitialised memory.
//!
//! Engine access goes through [`Hp2ParticleFxServices`]; the C++ side fills it
//! from `Hp2ParticleFxServices.cpp`. `UParticle` lists are C++ `UParticleList`
//! objects reached through the list thunks.

use crate::fpoly::cross;
use std::ffi::c_void;
use std::mem::MaybeUninit;

use hp2_abi::engine::generated::{
    AActor, AParticleFX, FBox, FColorParams, FFloatParams, FRotator, UParticle,
};
use hp2_abi::render_scene::FSceneNode;
use hp2_abi::{FColor, FCoords, FPlane, FVector};

use crate::coords::{self, Trig};
use crate::math;
use crate::particle::{self, Hp2ParticleServices};
use crate::prim::{self, Aabb};
use crate::vec;

const MAX_TIMESLICE: f32 = 0.06667;
const KINDA_SMALL_NUMBER: f32 = 1.0e-4;
const DEGREES_TO_RADIANS: f32 = 0.017_453_29;
const DIST_UNIFORM: u8 = 1;
const DIST_OWNER_MESH: u8 = 2;
const PPRIM_SHARD: u8 = 3;
const LEVELTICK_VIEWPORTS_ONLY: i32 = 1;
const PRIORITY_TABLE: [i32; 10] = [9, 3, 1, 7, 5, 0, 6, 2, 4, 8];

/// `FParams` (UnParticleFX.h:50–71). 76 bytes; layout pinned by the services
/// `static_assert`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FParams {
    pub particles_per_sec: f32,
    pub source_width: f32,
    pub source_height: f32,
    pub source_depth: f32,
    pub period: f32,
    pub decay: f32,
    pub angular_spread_width: f32,
    pub angular_spread_height: f32,
    pub speed: f32,
    pub lifetime: f32,
    pub color_start: FColor,
    pub color_end: FColor,
    pub alpha_start: f32,
    pub alpha_end: f32,
    pub size_width: f32,
    pub size_length: f32,
    pub size_end_scale: f32,
    pub spin_rate: f32,
    pub drip_time: f32,
}

/// `FParticleParams` (UnParticleFX.h:31–48). 52 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FParticleParams {
    pub position: FVector,
    pub velocity: FVector,
    pub lifetime: f32,
    pub alpha: f32,
    pub color: FColor,
    pub width: f32,
    pub length: f32,
    pub drip_timer: f32,
    pub spin_rate: f32,
}

/// Engine thunks, filled by `Hp2ParticleFxServices.cpp`. Field order matches
/// the C struct exactly; every field is one pointer wide.
#[repr(C)]
pub struct Hp2ParticleFxServices {
    /// `Hp2ParticleServices` (`appFrand`, sin/cos tables, wind, traces).
    pub particle: *const Hp2ParticleServices,
    pub app_frand_range: unsafe extern "C" fn(f32, f32) -> f32,
    pub app_rand_range_int: unsafe extern "C" fn(i32, i32) -> i32,
    pub app_exp: unsafe extern "C" fn(f64) -> f64,
    pub sqrt_approx: unsafe extern "C" fn(f32) -> f32,
    pub sin_float: unsafe extern "C" fn(f32) -> f32,
    pub cos_float: unsafe extern "C" fn(f32) -> f32,
    pub actor_to_local: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub actor_zone_gravity: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub actor_lod_bias: unsafe extern "C" fn(*mut c_void) -> f32,
    pub level_time_seconds: unsafe extern "C" fn(*mut c_void) -> f64,
    pub level_destroy_actor: unsafe extern "C" fn(*mut c_void),
    pub is_editor: unsafe extern "C" fn() -> i32,
    pub sync_owner_transient: unsafe extern "C" fn(*mut c_void),
    pub parent_defaults: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub mesh_render_box: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub mesh_frame_verts: unsafe extern "C" fn(*mut c_void) -> i32,
    pub mesh_get_frame: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub mesh_num_tris: unsafe extern "C" fn(*mut c_void) -> i32,
    pub mesh_tri_verts: unsafe extern "C" fn(*mut c_void, i32, *mut i32),
    pub pattern_num_points: unsafe extern "C" fn(*mut c_void) -> i32,
    pub pattern_point: unsafe extern "C" fn(*mut c_void, i32, *mut c_void),
    pub palette_color0: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub list_start: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub list_next: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub list_add: unsafe extern "C" fn(*mut c_void, *const c_void),
    pub list_remove_current: unsafe extern "C" fn(*mut c_void),
    pub list_remove_all: unsafe extern "C" fn(*mut c_void),
    pub list_remove_from_front: unsafe extern "C" fn(*mut c_void),
    pub list_size: unsafe extern "C" fn(*mut c_void) -> i32,
    pub list_get_particle: unsafe extern "C" fn(*mut c_void, i32) -> *mut c_void,
    pub list_update_box: unsafe extern "C" fn(*mut c_void),
    pub list_world_box: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub emit_particles_shell: unsafe extern "C" fn(*mut c_void, f32) -> i32,
    pub update_shell: unsafe extern "C" fn(*mut c_void, f32) -> i32,
    pub stat_clock_update: unsafe extern "C" fn(),
    pub stat_unclock_update: unsafe extern "C" fn(),
    pub stat_particles_emitted: unsafe extern "C" fn(i32),
}

type Col = [u8; 4];

#[inline]
fn max_c(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

#[inline]
fn min_c(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

#[inline]
fn clamp_c(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x < hi {
        x
    } else {
        hi
    }
}

#[inline]
fn clamp_i(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo {
        lo
    } else if x < hi {
        x
    } else {
        hi
    }
}

/// `MaxVal(P) = P.Base + Max(P.Rand, 0.f)` (TU L37–45).
#[inline]
fn max_val(p: FFloatParams) -> f32 {
    p.base + max_c(p.rand, 0.0)
}

/// `MinVal(P) = P.Base + Min(P.Rand, 0.f)` (TU L37–45).
#[inline]
fn min_val(p: FFloatParams) -> f32 {
    p.base + min_c(p.rand, 0.0)
}

#[inline]
fn fvec(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

#[inline]
fn vec_eq(a: FVector, b: FVector) -> bool {
    a.x == b.x && a.y == b.y && a.z == b.z
}

#[inline]
fn is_zero(a: FVector) -> bool {
    a.x == 0.0 && a.y == 0.0 && a.z == 0.0
}

#[inline]
fn square(a: f32) -> f32 {
    a * a
}

#[inline]
fn deg_to_rad(x: f32) -> f32 {
    x * DEGREES_TO_RADIANS
}

fn zero_coords() -> FCoords {
    FCoords {
        origin: vec::zero(),
        x_axis: vec::zero(),
        y_axis: vec::zero(),
        z_axis: vec::zero(),
    }
}

/// `FColor::Plane()` (UnTex.h:108–111). W is dropped, so the result has W = 0.
#[inline]
fn color_plane(c: Col) -> FPlane {
    FPlane {
        x: f32::from(c[0]) / 255.0,
        y: f32::from(c[1]) / 255.0,
        z: f32::from(c[2]) / 255.0,
        w: 0.0,
    }
}

/// `FColor::FColor(const FPlane&)` (UnTex.h:62–67).
#[inline]
fn color_from_plane(p: FPlane) -> Col {
    [
        clamp_i(math::app_floor(p.x * 256.0), 0, 255) as u8,
        clamp_i(math::app_floor(p.y * 256.0), 0, 255) as u8,
        clamp_i(math::app_floor(p.z * 256.0), 0, 255) as u8,
        clamp_i(math::app_floor(p.w * 256.0), 0, 255) as u8,
    ]
}

/// `FColor * float` (operators at TU L291–311): per channel, clamped and truncated.
#[inline]
fn col_mul(c: Col, f: f32) -> Col {
    c.map(|x| clamp_i(math::app_round(f32::from(x) * f), 0, 255) as u8)
}

/// `FColor + FColor` (TU L291–311): per channel, saturating.
#[inline]
fn col_add(a: Col, b: Col) -> Col {
    std::array::from_fn(|i| clamp_i(i32::from(a[i]) + i32::from(b[i]), 0, 255) as u8)
}

#[inline]
fn col_of(c: FColor) -> Col {
    [c.r, c.g, c.b, c.a]
}

#[inline]
fn fcolor_of(c: Col) -> FColor {
    FColor {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

/// `ExpandBy(FBox&, FVector)` (file-local, TU L105–119).
fn expand_by_vec(b: &mut Aabb, v: FVector) {
    let mn = vec::add(b.min, v);
    let mx = vec::add(b.max, v);
    b.add_point(mn);
    b.add_point(mx);
}

/// `FBox::ExpandBy(float W)`.
fn expand_by_w(b: &Aabb, w: f32) -> Aabb {
    Aabb {
        min: vec::sub(b.min, fvec(w, w, w)),
        max: vec::add(b.max, fvec(w, w, w)),
        valid: b.valid,
    }
}

/// `FBox(const FCoords&)`: the box spanned by the origin plus each axis.
fn aabb_from_coords(c: &FCoords) -> Aabb {
    let mut mn = c.origin;
    let mut mx = c.origin;
    for a in [c.x_axis, c.y_axis, c.z_axis] {
        if a.x >= 0.0 {
            mx.x += a.x;
        } else {
            mn.x += a.x;
        }
        if a.y >= 0.0 {
            mx.y += a.y;
        } else {
            mn.y += a.y;
        }
        if a.z >= 0.0 {
            mx.z += a.z;
        } else {
            mn.z += a.z;
        }
    }
    Aabb {
        min: mn,
        max: mx,
        valid: true,
    }
}

/// `FBox(const FBox& Box, const FCoords& C)`.
fn aabb_transform(b: &Aabb, c: &FCoords) -> Aabb {
    aabb_from_coords(&coords::box_times_coords(
        &Aabb::from_min_max(b.min, b.max),
        c,
    ))
}

/// `FBox::GetCoords()`.
fn box_get_coords(b: &Aabb) -> FCoords {
    FCoords {
        origin: b.min,
        x_axis: fvec(b.max.x - b.min.x, 0.0, 0.0),
        y_axis: fvec(0.0, b.max.y - b.min.y, 0.0),
        z_axis: fvec(0.0, 0.0, b.max.z - b.min.z),
    }
}

// --- Services helpers -------------------------------------------------------

/// # Safety
/// `s.particle` must point to a populated `Hp2ParticleServices`.
unsafe fn frand(s: &Hp2ParticleFxServices) -> f32 {
    unsafe { ((*s.particle).app_frand)() }
}

/// # Safety
/// `s` must be populated.
unsafe fn frand_range(s: &Hp2ParticleFxServices, lo: f32, hi: f32) -> f32 {
    unsafe { (s.app_frand_range)(lo, hi) }
}

/// # Safety
/// `s` must be populated.
unsafe fn sin_f(s: &Hp2ParticleFxServices, x: f32) -> f32 {
    unsafe { (s.sin_float)(x) }
}

/// # Safety
/// `s` must be populated.
unsafe fn cos_f(s: &Hp2ParticleFxServices, x: f32) -> f32 {
    unsafe { (s.cos_float)(x) }
}

/// `FCoords` for `UnitCoords * Location * Rotation`.
///
/// # Safety
/// `svc.particle` must be populated.
unsafe fn fx_to_world(loc: FVector, rot: FRotator, svc: &Hp2ParticleFxServices) -> FCoords {
    unsafe {
        let sp = &*svc.particle;
        let sin = |i: i32| (sp.sin_tab)(i);
        let cos = |i: i32| (sp.cos_tab)(i);
        let trig = Trig {
            sin: &sin,
            cos: &cos,
        };
        let mut c = coords::unit_coords();
        coords::mul_vec(&mut c, loc);
        coords::mul_rot(&mut c, rot, &trig);
        c
    }
}

/// `ToLocal()` for an actor.
///
/// # Safety
/// `fx` must point to a live actor.
unsafe fn to_local(fx: *mut AParticleFX, svc: &Hp2ParticleFxServices) -> FCoords {
    unsafe {
        let mut c = zero_coords();
        (svc.actor_to_local)(fx.cast(), (&mut c as *mut FCoords).cast());
        c
    }
}

/// `Level->Model`-based total wind: `AWind::GetTotalWind(Level, loc)`.
///
/// # Safety
/// `level` must be a live `ULevel` pointer.
unsafe fn total_wind(svc: &Hp2ParticleFxServices, level: *mut c_void, loc: FVector) -> FVector {
    unsafe {
        let mut out = vec::zero();
        ((*svc.particle).total_wind)(
            level,
            (&loc as *const FVector).cast(),
            (&mut out as *mut FVector).cast(),
        );
        out
    }
}

/// `Info->ZoneGravity * GravityModifier + Gravity`, with the zone-or-level
/// fallback handled by the `ActorZoneGravity` thunk.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
unsafe fn system_gravity(fx: *mut AParticleFX, svc: &Hp2ParticleFxServices) -> FVector {
    unsafe {
        let mut zg = vec::zero();
        (svc.actor_zone_gravity)(fx.cast(), (&mut zg as *mut FVector).cast());
        vec::add(vec::scale(zg, (*fx).gravity_modifier), (*fx).gravity)
    }
}

/// `UParticleList` world box (copy of `ParticleList->WorldBox`).
///
/// # Safety
/// `list` must be a live `UParticleList`.
unsafe fn world_box(svc: &Hp2ParticleFxServices, list: *mut c_void) -> Aabb {
    unsafe {
        let mut b: FBox = std::mem::zeroed();
        (svc.list_world_box)(list, (&mut b as *mut FBox).cast());
        Aabb {
            min: b.min,
            max: b.max,
            valid: b.is_valid != 0,
        }
    }
}

/// `Hp2ParticleFxServices` pattern point `Pattern->Points(i)`.
///
/// # Safety
/// `pat` must be a live `UGesture`; `i` must be in range.
unsafe fn pattern_point(svc: &Hp2ParticleFxServices, pat: *mut c_void, i: i32) -> FVector {
    unsafe {
        let mut v = vec::zero();
        (svc.pattern_point)(pat, i, (&mut v as *mut FVector).cast());
        v
    }
}

/// `RandValue(P)`: `Base + Rand*appFrand()`, drawn only when `Rand != 0`.
///
/// # Safety
/// `s` must be populated.
unsafe fn rand_value(p: FFloatParams, s: &Hp2ParticleFxServices) -> f32 {
    unsafe {
        if p.rand != 0.0 {
            // Fused: Base + Rand*appFrand()
            p.rand.mul_add(frand(s), p.base)
        } else {
            p.base
        }
    }
}

/// `ColorStart.Rand.IntValue()` path: `Base + Rand*appFrand()` on colours.
///
/// # Safety
/// `s` must be populated.
unsafe fn rand_color(p: FColorParams, s: &Hp2ParticleFxServices) -> Col {
    unsafe {
        let rand = p.rand;
        let base = p.base;
        if rand != [0; 4] {
            col_add(base, col_mul(rand, frand(s)))
        } else {
            base
        }
    }
}

// --- Blending (GetSysParams) ------------------------------------------------

#[inline]
fn blend_f(d: &mut FFloatParams, p: FFloatParams, inv: f32, pb: f32) {
    // Fused: fma(d, inv, round(p*pb)). Oracle GetSysParams BlendValue (UnParticleFX.cpp:832-853):
    // `NewFX->p * InvBlend + ParentFX->p * ParentBlend` compiles to fmul(Parent, pb) followed by
    // fmla(NewFX, inv) on the sum (vector fmla.4s/fmla.2s, see the fused-op reconciliation).
    d.base = d.base.mul_add(inv, p.base * pb);
    d.rand = d.rand.mul_add(inv, p.rand * pb);
}

#[inline]
fn blend_c(d: &mut FColorParams, p: FColorParams, inv: f32, pb: f32) {
    d.base = col_add(col_mul(d.base, inv), col_mul(p.base, pb));
    d.rand = col_add(col_mul(d.rand, inv), col_mul(p.rand, pb));
}

// --- Exported logic ---------------------------------------------------------

/// `AParticleFX::GetSysParams` (TU L808–857). Returns `fx` itself, the parent
/// defaults when `ParentBlend >= 1`, or `buf` holding the blended copy.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `buf` must be writable and sized for
/// one `AParticleFX`.
pub unsafe fn get_sys_params(
    fx: *mut AParticleFX,
    buf: *mut AParticleFX,
    svc: &Hp2ParticleFxServices,
) -> *mut AParticleFX {
    unsafe {
        let pb = (*fx).parent_blend;
        if pb <= 0.0 {
            return fx;
        }
        let parent = (svc.parent_defaults)(fx.cast()).cast::<AParticleFX>();
        if pb >= 1.0 {
            return parent;
        }
        std::ptr::copy_nonoverlapping(fx, buf, 1);
        let nf = &mut *buf;
        let pf = &*parent;
        let inv = 1.0 - pb;
        blend_f(
            &mut nf.angular_spread_width,
            pf.angular_spread_width,
            inv,
            pb,
        );
        blend_f(
            &mut nf.angular_spread_height,
            pf.angular_spread_height,
            inv,
            pb,
        );
        blend_f(&mut nf.source_width, pf.source_width, inv, pb);
        blend_f(&mut nf.source_depth, pf.source_depth, inv, pb);
        blend_f(&mut nf.source_height, pf.source_height, inv, pb);
        blend_f(&mut nf.speed, pf.speed, inv, pb);
        blend_f(&mut nf.lifetime, pf.lifetime, inv, pb);
        blend_f(&mut nf.alpha_start, pf.alpha_start, inv, pb);
        blend_f(&mut nf.alpha_end, pf.alpha_end, inv, pb);
        blend_f(&mut nf.spin_rate, pf.spin_rate, inv, pb);
        blend_f(&mut nf.drip_time, pf.drip_time, inv, pb);
        blend_c(&mut nf.color_start, pf.color_start, inv, pb);
        blend_c(&mut nf.color_end, pf.color_end, inv, pb);
        blend_f(&mut nf.size_width, pf.size_width, inv, pb);
        blend_f(&mut nf.size_length, pf.size_length, inv, pb);
        blend_f(&mut nf.size_end_scale, pf.size_end_scale, inv, pb);
        buf
    }
}

/// `AParticleFX::GetParams` (TU L861–932). Draw order is part of the contract.
///
/// # Safety
/// `sys` must point to a live `AParticleFX` (or blended copy); `out` must be
/// writable.
pub unsafe fn get_params(sys: *const AParticleFX, out: *mut FParams, svc: &Hp2ParticleFxServices) {
    unsafe {
        let s = &*sys;
        let o = &mut *out;
        o.angular_spread_width = rand_value(s.angular_spread_width, svc);
        o.angular_spread_height = rand_value(s.angular_spread_height, svc);
        o.source_width = rand_value(s.source_width, svc);
        o.source_depth = rand_value(s.source_depth, svc);
        o.source_height = rand_value(s.source_height, svc);
        o.speed = rand_value(s.speed, svc);
        o.lifetime = rand_value(s.lifetime, svc);
        o.alpha_start = rand_value(s.alpha_start, svc);
        o.alpha_end = rand_value(s.alpha_end, svc);
        o.spin_rate = rand_value(s.spin_rate, svc);
        o.drip_time = rand_value(s.drip_time, svc);
        o.color_start = fcolor_of(rand_color(s.color_start, svc));
        o.color_end = fcolor_of(rand_color(s.color_end, svc));
        let size_rand = frand(svc);
        // Fused: SizeWidth.Base + SizeRand*SizeWidth.Rand
        o.size_width = s.size_width.rand.mul_add(size_rand, s.size_width.base);
        // Fused: SizeLength.Base + SizeRand*SizeLength.Rand
        o.size_length = s.size_length.rand.mul_add(size_rand, s.size_length.base);
        o.size_end_scale = rand_value(s.size_end_scale, svc);
    }
}

/// `AParticleFX::AddParticle` (TU L936–1050). Returns `false` when `params` is
/// null or there is no particle list.
///
/// Note: `Tail`, `ChaosDelay` and `Spin` are never assigned in the C++ (UB);
/// they are zero here, matching the oracle's `-ftrivial-auto-var-init=zero`.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `pos` and `params` must be valid
/// when non-null.
pub unsafe fn add_particle(
    fx: *mut AParticleFX,
    id: i32,
    pos: *const FVector,
    params: *const FParams,
    svc: &Hp2ParticleFxServices,
) -> bool {
    unsafe {
        if params.is_null() {
            return false;
        }
        let list = (*fx).particle_list;
        if list.is_null() {
            return false;
        }
        let p = &*params;
        let position = *pos;

        // UParticle(FVector) ctor: LastPosition = Position = Pos, Velocity = 0, Age = 0, Id = -1.
        let mut part: UParticle = std::mem::zeroed();
        part.last_position = position;
        part.position = position;
        part.velocity = vec::zero();
        part.age = 0.0;
        part.id = -1;
        part.id = id;
        part.priority_tag = 0;

        part.width = p.size_width;
        part.width_full = p.size_width;
        part.length = p.size_length;
        part.length_full = p.size_length;
        if (*fx).size_grow_period > 0.0 {
            part.width = 0.0;
            part.length = 0.0;
        }

        part.lifetime = p.lifetime;
        if part.lifetime > 0.01 {
            // Fused: (SizeWidth*SizeEndScale - SizeWidth)
            part.width_delta =
                p.size_width.mul_add(p.size_end_scale, -p.size_width) / part.lifetime;
            part.length_delta =
                p.size_length.mul_add(p.size_end_scale, -p.size_length) / part.lifetime;
        } else {
            part.width_delta = 0.0;
            part.length_delta = 0.0;
        }

        let to_world = fx_to_world((*fx).last_update_location, (*fx).base.rotation, svc);

        // Literal 0.01745f, not DEGREES_TO_RADIANS (TU L975, L978).
        let width_theta = p.angular_spread_width * 0.01745;
        let width_variance = frand_range(svc, -width_theta, width_theta);
        let height_theta = p.angular_spread_height * 0.01745;
        let height_variance = frand_range(svc, -height_theta, height_theta);

        let cos_height = cos_f(svc, height_variance);
        let local_rot = fvec(
            cos_f(svc, width_variance) * cos_height,
            sin_f(svc, width_variance) * cos_height,
            sin_f(svc, height_variance),
        );
        let world_rot = prim::transform_vector(local_rot, &to_world);
        part.velocity = vec::scale(world_rot, p.speed);

        part.color = color_plane(col_of(p.color_start));
        part.alpha = p.alpha_start;
        part.alpha_full = p.alpha_start;
        if (*fx).alpha_grow_period > 0.0 {
            part.alpha = 0.0;
        }

        part.spin_rate = p.spin_rate;
        part.drip_timer = p.drip_time;
        if part.drip_timer > 0.0 {
            part.width_delta = part.width / part.drip_timer;
            part.length_delta = part.length / part.drip_timer;
            part.width = 0.0;
            part.length = 0.0;
        }

        let palette = (*fx).color_palette;
        let mut palette_plane = FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
        if !palette.is_null()
            && (svc.palette_color0)(palette, (&mut palette_plane as *mut FPlane).cast()) != 0
        {
            part.color = palette_plane;
        } else {
            if part.lifetime > (*fx).color_delay {
                let life_recip = 1.0 / (part.lifetime - (*fx).color_delay);
                let color_recip = 1.0f32 / 255.0 * life_recip;
                let cs = col_of(p.color_start);
                let ce = col_of(p.color_end);
                part.color_delta.x = (i32::from(ce[0]) - i32::from(cs[0])) as f32 * color_recip;
                part.color_delta.y = (i32::from(ce[1]) - i32::from(cs[1])) as f32 * color_recip;
                part.color_delta.z = (i32::from(ce[2]) - i32::from(cs[2])) as f32 * color_recip;
            } else {
                part.color_delta.x = 0.0;
                part.color_delta.y = 0.0;
                part.color_delta.z = 0.0;
            }
            part.color_delta.w = 0.0;

            if part.lifetime > (*fx).alpha_delay + 0.001 {
                let life_recip = 1.0 / (part.lifetime - (*fx).alpha_delay);
                part.alpha_delta = (p.alpha_end - p.alpha_start) * life_recip;
            } else {
                part.alpha_delta = 0.0;
            }
        }

        (svc.list_add)(list, (&part as *const UParticle).cast());
        true
    }
}

/// `AParticleFX::EmitParticles` after the C++ `FAppRandTraceNativeObjectScope`
/// (offset 1). Returns the number of particles emitted.
///
/// # Safety
/// `fx` must point to a live `AParticleFX` with a valid particle list.
pub unsafe fn emit_particles(fx: *mut AParticleFX, tick: f32, svc: &Hp2ParticleFxServices) -> i32 {
    unsafe {
        let list = (*fx).particle_list;
        (*fx).emit_delay += tick;

        let location = (*fx).base.location;
        let emit_delta = vec::sub((*fx).last_emit_location, location);
        let parent = (svc.parent_defaults)(fx.cast()).cast::<AParticleFX>();

        // Emissions = PPS.Base + appFrand()*PPS.Rand
        let pps = (*fx).particles_per_sec;
        // Fused: PPS.Base + appFrand()*PPS.Rand
        let mut emissions = pps.rand.mul_add(frand(svc), pps.base);
        if (*fx).parent_blend != 0.0 {
            let ppps = (*parent).particles_per_sec;
            // Fused: ParentFX->PPS.Base + appFrand()*ParentFX->PPS.Rand
            let inner = ppps.rand.mul_add(frand(svc), ppps.base);
            // Fused: Emissions += (inner - Emissions) * ParentBlend
            emissions = (inner - emissions).mul_add((*fx).parent_blend, emissions);
        }

        let dist = (*fx).distribution;
        if dist != DIST_UNIFORM {
            emissions *= clamp_c((*fx).lod * 3.0, 0.1, 1.0);
            emissions *= (*fx).emit_delay;
        } else if !(*fx).pattern.is_null() {
            let pat = (*fx).pattern;
            let num = (svc.pattern_num_points)(pat);
            let period = (*fx).period;
            // Fused: (Period.Base + Period.Rand*0.5f) * (Num-1)
            let f = period.rand.mul_add(0.5, period.base) * ((num - 1) as f32);
            let n = std::cmp::min(f as i32, num - 2);
            let del = vec::sub(pattern_point(svc, pat, n), pattern_point(svc, pat, n + 1));
            emissions = vec::size(del) * (*fx).base.draw_scale * period.rand * ((num - 1) as f32)
                / emissions;
        } else {
            emissions = vec::size(emit_delta) / emissions;
        }

        emissions += (*fx).emission_residue;

        let mut emit_start = -(*fx).emit_delay;

        let pmax = (*fx).particles_max;
        if pmax > 0 && emissions >= (pmax - (*fx).particles_emitted) as f32 {
            (*fx).emit_delay *= ((pmax - (*fx).particles_emitted) as f32) / emissions;
            emissions = (pmax - (*fx).particles_emitted) as f32;
        }

        let max_life = max_lifetime(fx);
        if emit_start < -max_life {
            let new_delay = (*fx).emit_delay + emit_start + max_life;
            emissions *= new_delay / (*fx).emit_delay;
            (*fx).emit_delay = new_delay;
            emit_start = -max_life;
        }

        let mut new_particles = emissions as i32;
        (*fx).emission_residue = emissions - new_particles as f32;
        if new_particles <= 0 {
            return 0;
        }

        let mut new_params: FParams = std::mem::zeroed();
        let to_world = fx_to_world(location, (*fx).base.rotation, svc);

        if (*fx).particles_alive > 0
            && new_particles + (svc.list_size)(list) > (*fx).particles_alive
        {
            if new_particles > (*fx).particles_alive {
                new_particles = (*fx).particles_alive;
            }
            let excess = new_particles + (svc.list_size)(list) - (*fx).particles_alive;
            for _ in 0..excess {
                (svc.list_remove_from_front)(list);
            }
        }

        let level = (*fx).base.x_level;
        let system_gravity = system_gravity(fx, svc);
        let mut system_wind = vec::zero();
        if (*fx).damping * (*fx).wind_modifier > 0.0 {
            system_wind = vec::scale(total_wind(svc, level, location), (*fx).wind_modifier);
        }

        let owner = (*fx).base.owner.cast::<AActor>();
        let mesh_owner = dist == DIST_OWNER_MESH && !owner.is_null() && !(*owner).mesh.is_null();
        let mut verts: Vec<FVector> = Vec::new();
        if mesh_owner {
            let n = (svc.mesh_frame_verts)(owner.cast());
            verts = vec![vec::zero(); n.max(0) as usize];
            (svc.mesh_get_frame)(owner.cast(), verts.as_mut_ptr().cast());
        }

        let mut sys_buf = MaybeUninit::<AParticleFX>::uninit();
        let sys = get_sys_params(fx, sys_buf.as_mut_ptr(), svc);

        let time_slice = !is_zero((*fx).attraction) || (*fx).elasticity > KINDA_SMALL_NUMBER;

        for i in 0..new_particles {
            get_params(sys, &mut new_params, svc);

            let delta = if dist == DIST_UNIFORM {
                if emissions > 0.0 {
                    (i + 1) as f32 / emissions
                } else {
                    0.0
                }
            } else {
                frand(svc)
            };

            // C++ `UParticle Particle;` runs the default ctor: position, velocity
            // and age zero, Id -1. Other fields are uninitialised in the C++ and
            // zero-filled here.
            let mut part: UParticle = std::mem::zeroed();
            part.id = -1;
            part.lifetime = new_params.lifetime;
            // Fused: -EmitStart - EmitDelay*Delta
            let mut time_delta = (-(*fx).emit_delay).mul_add(delta, -emit_start);
            if time_delta >= part.lifetime {
                continue;
            }

            let world_dir;
            if mesh_owner {
                let mut tv = [0i32; 3];
                let tri = (svc.app_rand_range_int)(0, (svc.mesh_num_tris)(owner.cast()) - 1);
                (svc.mesh_tri_verts)(owner.cast(), tri, tv.as_mut_ptr());
                let u = frand(svc);
                let v = frand(svc);
                let v0 = verts[tv[0] as usize];
                let v1 = verts[tv[1] as usize];
                let v2 = verts[tv[2] as usize];
                part.position = vec::add(
                    vec::add(vec::scale(v0, 1.0 - u), vec::scale(v1, u * (1.0 - v))),
                    vec::scale(v2, u * v),
                );
                world_dir = prim::safe_normal(cross(vec::sub(v1, v0), vec::sub(v2, v0)));
            } else {
                if !(*fx).pattern.is_null() {
                    let pat = (*fx).pattern;
                    let num = (svc.pattern_num_points)(pat);
                    let period = (*fx).period;
                    // Fused: (Period.Base + Delta*Period.Rand) * (Num-1)
                    let mut f = delta.mul_add(period.rand, period.base) * ((num - 1) as f32);
                    let n = std::cmp::min(f as i32, num - 2);
                    f -= n as f32;
                    let pos = vec::add(
                        vec::scale(pattern_point(svc, pat, n), 1.0 - f),
                        vec::scale(pattern_point(svc, pat, n + 1), f),
                    );
                    let pos =
                        vec::scale(fvec(0.0, pos.x - 0.5, 0.5 - pos.y), (*fx).base.draw_scale);
                    part.position = prim::transform_point(pos, &to_world);
                } else {
                    part.position = location;
                }

                let width_theta = deg_to_rad(new_params.angular_spread_width);
                let width_variance = frand_range(svc, -width_theta, width_theta);
                let height_theta = deg_to_rad(new_params.angular_spread_height);
                let height_variance = frand_range(svc, -height_theta, height_theta);
                let cos_height = cos_f(svc, height_variance);
                let local_rot = fvec(
                    cos_f(svc, width_variance) * cos_height,
                    sin_f(svc, width_variance) * cos_height,
                    sin_f(svc, height_variance),
                );
                world_dir = prim::transform_vector(local_rot, &to_world);
            }

            let depth = new_params.source_depth;
            let width = new_params.source_width;
            let height = new_params.source_height;
            let offset = fvec(
                frand_range(svc, -depth * 0.5, depth * 0.5),
                frand_range(svc, -width * 0.5, width * 0.5),
                frand_range(svc, -height * 0.5, height * 0.5),
            );
            part.position = vec::add(part.position, prim::transform_vector(offset, &to_world));
            part.position = vec::add(part.position, vec::scale(emit_delta, 1.0 - delta));

            part.width = new_params.size_width;
            part.width_full = new_params.size_width;
            part.length = new_params.size_length;
            part.length_full = new_params.size_length;
            if (*fx).size_grow_period > 0.0 {
                part.width = 0.0;
                part.length = 0.0;
            }

            part.lifetime = new_params.lifetime;
            part.drip_timer = new_params.drip_time;
            if part.drip_timer == 0.0 {
                if part.lifetime > (*fx).size_delay {
                    let t = part.lifetime - (*fx).size_delay;
                    // Fused: (SizeWidth*SizeEndScale - SizeWidth)
                    part.width_delta = new_params
                        .size_width
                        .mul_add(new_params.size_end_scale, -new_params.size_width)
                        / t;
                    part.length_delta = new_params
                        .size_length
                        .mul_add(new_params.size_end_scale, -new_params.size_length)
                        / t;
                } else {
                    part.width_delta = 0.0;
                    part.length_delta = 0.0;
                }
            } else {
                part.width_delta = part.width / part.drip_timer;
                part.length_delta = part.length / part.drip_timer;
                part.width = 0.0;
                part.length = 0.0;
            }

            part.velocity = vec::scale(world_dir, new_params.speed);
            if (*fx).b_velocity_relative() && !owner.is_null() {
                part.velocity = vec::add(part.velocity, (*owner).velocity);
            }

            part.color = color_plane(col_of(new_params.color_start));
            part.alpha = new_params.alpha_start;
            part.alpha_full = new_params.alpha_start;
            if (*fx).alpha_grow_period > 0.0 {
                part.alpha = 0.0;
            }

            part.spin_rate = new_params.spin_rate;
            if (*fx).render_primitive == PPRIM_SHARD {
                part.spin = frand(svc) * std::f32::consts::PI * 2.0;
            }

            let mut palette_plane = FPlane {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            };
            let palette = (*fx).color_palette;
            if !palette.is_null()
                && (svc.palette_color0)(palette, (&mut palette_plane as *mut FPlane).cast()) != 0
            {
                part.color = palette_plane;
            } else {
                if part.lifetime > (*fx).color_delay {
                    let t = part.lifetime - (*fx).color_delay;
                    let cs = col_of(new_params.color_start);
                    let ce = col_of(new_params.color_end);
                    part.color_delta.x = ((i32::from(ce[0]) - i32::from(cs[0])) as f32 / 255.0) / t;
                    part.color_delta.y = ((i32::from(ce[1]) - i32::from(cs[1])) as f32 / 255.0) / t;
                    part.color_delta.z = ((i32::from(ce[2]) - i32::from(cs[2])) as f32 / 255.0) / t;
                    part.color_delta.w = 0.0;
                } else {
                    part.color_delta = FPlane {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        w: 0.0,
                    };
                }

                if part.lifetime > (*fx).alpha_delay {
                    part.alpha_delta = (new_params.alpha_end - new_params.alpha_start)
                        / (part.lifetime - (*fx).alpha_delay);
                } else {
                    part.alpha_delta = 0.0;
                }
            }

            while time_delta > 0.0 {
                let tick_slice = if time_delta > MAX_TIMESLICE && time_slice {
                    MAX_TIMESLICE
                } else {
                    time_delta
                };
                let system_damping = if (*fx).damping > 0.0 {
                    (svc.app_exp)(f64::from(-(*fx).damping * tick_slice)) as f32
                } else {
                    1.0
                };
                let _ = particle::update(
                    &mut part,
                    system_gravity,
                    system_damping,
                    system_wind,
                    level,
                    tick_slice,
                    fx.cast(),
                    &*svc.particle,
                );
                time_delta -= tick_slice;
            }

            (*fx).current_priority_tag += 1;
            if (*fx).current_priority_tag > 9 {
                (*fx).current_priority_tag = 0;
            }
            part.priority_tag = PRIORITY_TABLE[(*fx).current_priority_tag as usize];

            (svc.list_add)(list, (&part as *const UParticle).cast());
        }

        if new_particles > 0 {
            (*fx).emit_delay = 0.0;
            (*fx).last_emit_location = location;
            (*fx).particles_emitted += new_particles;
            (svc.stat_particles_emitted)(new_particles);
        }
        new_particles
    }
}

/// `AParticleFX::UpdateParticles` (TU L223–289).
///
/// # Safety
/// `fx` must point to a live `AParticleFX` with a valid particle list.
pub unsafe fn update_particles(fx: *mut AParticleFX, tick: f32, svc: &Hp2ParticleFxServices) {
    unsafe {
        let list = (*fx).particle_list;
        let max_life = max_lifetime(fx);
        if max_life > 0.0 && tick > max_life {
            (svc.list_remove_all)(list);
            return;
        }

        let mut p = (svc.list_start)(list).cast::<UParticle>();
        while !p.is_null() {
            if (*p).lifetime > 0.0 && (*p).age + tick >= (*p).lifetime {
                (svc.list_remove_current)(list);
            }
            p = (svc.list_next)(list).cast::<UParticle>();
        }

        if (svc.list_size)(list) == 0 {
            return;
        }

        let level = (*fx).base.x_level;
        let system_gravity = system_gravity(fx, svc);
        let mut system_wind = vec::zero();
        if (*fx).damping * (*fx).wind_modifier > 0.0 {
            system_wind = vec::scale(
                total_wind(svc, level, (*fx).base.location),
                (*fx).wind_modifier,
            );
        }

        let time_slice = !is_zero((*fx).attraction) || (*fx).elasticity > KINDA_SMALL_NUMBER;
        let mut tick = tick;
        while tick > 0.0 {
            let tick_slice = if tick > MAX_TIMESLICE && time_slice {
                MAX_TIMESLICE
            } else {
                tick
            };
            let system_damping = if (*fx).damping > 0.0 {
                (svc.app_exp)(f64::from(-(*fx).damping * tick_slice)) as f32
            } else {
                1.0
            };
            let mut p = (svc.list_start)(list).cast::<UParticle>();
            while !p.is_null() {
                let _ = particle::update(
                    &mut *p,
                    system_gravity,
                    system_damping,
                    system_wind,
                    level,
                    tick_slice,
                    fx.cast(),
                    &*svc.particle,
                );
                p = (svc.list_next)(list).cast::<UParticle>();
            }
            tick -= tick_slice;
        }
    }
}

/// `AParticleFX::Update` (TU L701–804). The C++ shell supplies the
/// `FAppRandTraceNativeObjectScope` (offset 0).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn update(fx: *mut AParticleFX, step: f32, svc: &Hp2ParticleFxServices) -> bool {
    unsafe {
        let list = (*fx).particle_list;
        if list.is_null() {
            return false;
        }

        (svc.stat_clock_update)();
        (svc.sync_owner_transient)(fx.cast());

        if (*fx).elapsed_time == 0.0 {
            (*fx).last_update_location = (*fx).base.location;
            (*fx).last_emit_location = (*fx).base.location;
            (*fx).last_update_rotation = (*fx).base.rotation;
        }

        let mut step = step;
        if step == 0.0 {
            step = (*fx).age - (*fx).elapsed_time;
        }
        (*fx).elapsed_time += step;

        if (*fx).b_steady_state()
            && vec_eq((*fx).base.location, (*fx).last_update_location)
            && step > MAX_TIMESLICE
        {
            let max_life = max_lifetime(fx);
            let max_rate = max_val((*fx).particles_per_sec);
            let mut max_particles = max_rate * max_life;
            if (*fx).particles_alive > 0 {
                max_particles = min_c(max_particles, (*fx).particles_alive as f32);
            }
            let max_step_raw =
                (max_particles - (svc.list_size)(list) as f32) / min_val((*fx).particles_per_sec);
            let max_step = max_c(max_step_raw, MAX_TIMESLICE);
            step = min_c(step, max_life);
            step = min_c(step, max_step);
        }

        if step <= 0.0 && !(*fx).b_shell_only() {
            (svc.stat_unclock_update)();
            return true;
        }
        if (*fx).b_shell_only() {
            step = 0.1;
        }

        update_particles(fx, step, svc);
        (svc.stat_unclock_update)();

        if (*fx).particles_max == 0 || (*fx).particles_emitted < (*fx).particles_max {
            if (*fx).b_emit() {
                (svc.emit_particles_shell)(fx.cast(), step);
            } else {
                (*fx).last_emit_location = (*fx).base.location;
            }
        } else if (svc.list_size)(list) == 0 {
            if (svc.is_editor)() != 0 {
                (*fx).particles_emitted = 0;
                (*fx).emit_delay = 0.0;
            } else {
                (svc.level_destroy_actor)(fx.cast());
                return false;
            }
        }

        (svc.list_update_box)(list);

        (*fx).last_update_location = (*fx).base.location;
        (*fx).last_update_rotation = (*fx).base.rotation;
        true
    }
}

/// `AParticleFX::GetRenderBoundingBox` (TU L121–191). `Exact` is unused in the
/// TU and is not part of the export.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `out` must be writable.
pub unsafe fn get_render_bounding_box(
    fx: *mut AParticleFX,
    out: *mut FCoords,
    svc: &Hp2ParticleFxServices,
) {
    unsafe {
        let list = (*fx).particle_list;
        let wb = world_box(svc, list);
        let dloc = vec::sub((*fx).base.location, (*fx).last_update_location);
        let wb_ext = vec::size_squared(vec::sub(wb.max, wb.min));
        if vec::size_squared(dloc) > max_c(wb_ext, 10000.0) {
            let ok = (svc.update_shell)(fx.cast(), 0.0) != 0;
            *out = if ok && !(*fx).particle_list.is_null() {
                box_get_coords(&world_box(svc, (*fx).particle_list))
            } else {
                zero_coords()
            };
            return;
        }

        let mut bx = world_box(svc, list);
        if (*fx).age > (*fx).elapsed_time {
            let start_size = max_c(max_val((*fx).size_width), max_val((*fx).size_length)) * 0.5;
            let b_const =
                (*fx).b_steady_state() && vec_eq((*fx).base.location, (*fx).last_update_location);

            let owner = (*fx).base.owner.cast::<AActor>();
            let source_box = if (*fx).distribution == DIST_OWNER_MESH
                && !owner.is_null()
                && !(*owner).mesh.is_null()
            {
                let mut c = zero_coords();
                (svc.mesh_render_box)(owner.cast(), (&mut c as *mut FCoords).cast());
                aabb_from_coords(&c)
            } else if !(*fx).pattern.is_null() {
                let sb = Aabb::from_extent(vec::scale(fvec(0.0, 0.5, 0.5), (*fx).base.draw_scale));
                aabb_transform(&sb, &to_local(fx, svc))
            } else {
                let ext = vec::scale(
                    fvec(
                        max_val((*fx).source_depth),
                        max_val((*fx).source_width),
                        max_val((*fx).source_height),
                    ),
                    0.5,
                );
                aabb_transform(&Aabb::from_extent(ext), &to_local(fx, svc))
            };
            bx.add_box(&expand_by_w(&source_box, start_size));

            if !b_const || (*fx).elapsed_time < max_val((*fx).lifetime) {
                let delta = min_c((*fx).age - (*fx).elapsed_time, max_val((*fx).lifetime));

                let mut max_speed = max_val((*fx).speed);
                if (*fx).chaos_delay > 0.0 {
                    max_speed += (*fx).chaos / (*fx).chaos_delay;
                }
                let mut expand = max_speed * delta;
                // Fused: StartSize*Max(MaxVal(SizeEndScale),0) - 1
                let mut size_del =
                    start_size.mul_add(max_c(max_val((*fx).size_end_scale), 0.0), -1.0);
                if delta < min_val((*fx).lifetime) {
                    size_del *= delta / min_val((*fx).lifetime);
                }
                expand += size_del;
                bx = expand_by_w(&bx, expand * 1.1);

                let gravity_travel = system_gravity(fx, svc);
                let travel = vec::scale(gravity_travel, square(delta) * 0.5);
                expand_by_vec(&mut bx, vec::scale(travel, 1.1));
            }
        }

        *out = box_get_coords(&bx);
    }
}

/// `AParticleFX::InitExecution` after `Super::InitExecution()`.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn init_execution(fx: *mut AParticleFX, svc: &Hp2ParticleFxServices) {
    unsafe {
        let steady = (*fx).particles_max == 0 && (*fx).distribution != DIST_UNIFORM;
        (*fx).set_b_steady_state(steady);
        if steady
            && ((*fx).b_prime() || (svc.level_time_seconds)(fx.cast()) == 0.0)
            && (*fx).age < max_lifetime(fx)
        {
            (*fx).age = max_lifetime(fx);
        }
        (*fx).elapsed_time = 0.0;
    }
}

/// `AParticleFX::Tick` body before `Super::Tick` (TU L673–697). Returns nothing;
/// the C++ shell calls `Super::Tick`.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn tick(
    fx: *mut AParticleFX,
    delta_seconds: f32,
    tick_type: i32,
    svc: &Hp2ParticleFxServices,
) {
    unsafe {
        let editor = (svc.is_editor)() != 0;
        if tick_type != LEVELTICK_VIEWPORTS_ONLY || editor {
            let list = (*fx).particle_list;
            let has_particles = !list.is_null() && (svc.list_size)(list) > 0;
            if has_particles || max_val((*fx).particles_per_sec) > 0.0 {
                (*fx).age += delta_seconds;
                if !editor && (*fx).particles_max > 0 && (*fx).particles_per_sec.base > 0.0 {
                    let sys_life = (*fx).particles_max as f32 / (*fx).particles_per_sec.base
                        + max_lifetime(fx);
                    if (*fx).age > sys_life {
                        (svc.level_destroy_actor)(fx.cast());
                    }
                }
            }
        }
    }
}

/// `AParticleFX::MaxLifetime` (TU L665–669).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn max_lifetime(fx: *mut AParticleFX) -> f32 {
    unsafe { max_val((*fx).lifetime) }
}

/// `AParticleFX::Lod` (TU L1092–1098).
///
/// # Safety
/// `fx` must point to a live actor.
pub unsafe fn lod(fx: *mut AParticleFX, screen_frac: f32, svc: &Hp2ParticleFxServices) -> f32 {
    unsafe {
        if (*fx).distribution == DIST_UNIFORM {
            1.0
        } else {
            let bias = (svc.actor_lod_bias)(fx.cast());
            min_c(1.0, bias * 2.0 * (svc.sqrt_approx)(screen_frac))
        }
    }
}

/// `AParticleFX::LodParticles` (TU L1102–1104).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn lod_particles(
    fx: *mut AParticleFX,
    screen_frac: f32,
    svc: &Hp2ParticleFxServices,
) -> f32 {
    unsafe {
        let list = (*fx).particle_list;
        let count = if list.is_null() {
            0
        } else {
            (svc.list_size)(list)
        };
        lod(fx, screen_frac, svc) * count as f32
    }
}

/// `AParticleFX::LodParticleDensity` (TU L1109–1140).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `frame` must be a valid scene node.
pub unsafe fn lod_particle_density(
    fx: *mut AParticleFX,
    frame: *const FSceneNode,
    screen_frac: f32,
    svc: &Hp2ParticleFxServices,
) -> f32 {
    unsafe {
        let list = (*fx).particle_list;
        if list.is_null() {
            return 0.0;
        }
        let cur_lod = lod(fx, screen_frac, svc);
        let max_pt = math::app_round(cur_lod * 9.0);
        let size = (svc.list_size)(list);
        let sample_lod = 20.0 / size as f32;
        let sample_pt = std::cmp::min(max_pt, math::app_round(sample_lod * 9.0));
        let factor = ((max_pt as f64 + 1.0) as f32) / ((sample_pt as f64 + 1.0) as f32);
        let max_pt = sample_pt;

        let fr = &*frame;
        let size_mul = fr.proj.z / (fr.fx * fr.fy);
        let mut density = 0.0f32;
        let mut p = (svc.list_start)(list).cast::<UParticle>();
        while !p.is_null() {
            if (*p).priority_tag <= max_pt {
                let d = vec::sub((*p).position, fr.coords.origin);
                let dist = (svc.sqrt_approx)(vec::size_squared(d));
                let sz = (*p).width * (*p).length * size_mul;
                if sz >= dist {
                    density += 1.0;
                } else {
                    density += sz / dist;
                }
            }
            p = (svc.list_next)(list).cast::<UParticle>();
        }
        density * factor
    }
}

/// `AParticleFX::RecomputeDeltas` (TU L1054–1088).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn recompute_deltas(fx: *mut AParticleFX, id: i32, svc: &Hp2ParticleFxServices) -> bool {
    unsafe {
        let list = (*fx).particle_list;
        if !(*fx).color_palette.is_null() || list.is_null() {
            return false;
        }
        let p = (svc.list_get_particle)(list, id).cast::<UParticle>();
        if p.is_null() {
            return false;
        }
        let remaining = (*p).lifetime - (*p).age;
        if remaining > 0.0 {
            let life_recip = 1.0 / remaining;
            let color_recip = 1.0f32 / 255.0 * life_recip;
            let ce = (*fx).color_end.base;
            (*p).color_delta.x = (f32::from(ce[0]) - (*p).color.x) * color_recip;
            (*p).color_delta.y = (f32::from(ce[1]) - (*p).color.y) * color_recip;
            (*p).color_delta.z = (f32::from(ce[2]) - (*p).color.z) * color_recip;
            (*p).color_delta.w = 0.0;
            (*p).alpha_delta = ((*fx).alpha_end.base - (*p).alpha) * life_recip;
            let sesb = (*fx).size_end_scale.base;
            // Fused: SizeEndScale.Base*Width - Width
            (*p).width_delta = sesb.mul_add((*p).width, -(*p).width) * life_recip;
            (*p).length_delta = sesb.mul_add((*p).length, -(*p).length) * life_recip;
            true
        } else {
            false
        }
    }
}

/// `execAddParticle` body after `P_FINISH` (TU L1148–1266).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `loc` must be valid.
pub unsafe fn exec_add_particle(
    fx: *mut AParticleFX,
    id: i32,
    loc: *const FVector,
    svc: &Hp2ParticleFxServices,
) {
    unsafe {
        (*fx).set_b_steady_state(false);
        let mut buf = MaybeUninit::<AParticleFX>::uninit();
        let sys = get_sys_params(fx, buf.as_mut_ptr(), svc);
        let mut np: FParams = std::mem::zeroed();
        get_params(sys, &mut np, svc);
        add_particle(fx, id, loc, &np, svc);
    }
}

/// `execNumParticles` body (TU L1148–1266): `ParticleList ? Size() : -1`.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn num_particles(fx: *mut AParticleFX, svc: &Hp2ParticleFxServices) -> i32 {
    unsafe {
        let list = (*fx).particle_list;
        if list.is_null() {
            -1
        } else {
            (svc.list_size)(list)
        }
    }
}

/// `execGetParticleParams` body (TU L1148–1266).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `out` must be writable.
pub unsafe fn get_particle_params(
    fx: *mut AParticleFX,
    id: i32,
    out: *mut FParticleParams,
    svc: &Hp2ParticleFxServices,
) -> bool {
    unsafe {
        let list = (*fx).particle_list;
        let p = if list.is_null() {
            std::ptr::null_mut()
        } else {
            (svc.list_get_particle)(list, id).cast::<UParticle>()
        };
        if p.is_null() {
            return false;
        }
        let o = &mut *out;
        o.position = (*p).position;
        o.velocity = (*p).velocity;
        o.lifetime = (*p).lifetime;
        o.alpha = (*p).alpha;
        o.color = fcolor_of(color_from_plane((*p).color));
        o.width = (*p).width;
        o.length = (*p).length;
        o.drip_timer = (*p).drip_timer;
        o.spin_rate = (*p).spin_rate;
        true
    }
}

/// `execSetParticleParams` body (TU L1148–1266).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `params` must be valid.
pub unsafe fn set_particle_params(
    fx: *mut AParticleFX,
    id: i32,
    params: *const FParticleParams,
    svc: &Hp2ParticleFxServices,
) -> bool {
    unsafe {
        (*fx).set_b_steady_state(false);
        let list = (*fx).particle_list;
        let p = if list.is_null() {
            std::ptr::null_mut()
        } else {
            (svc.list_get_particle)(list, id).cast::<UParticle>()
        };
        if p.is_null() {
            return false;
        }
        let s = &*params;
        (*p).position = s.position;
        (*p).velocity = s.velocity;
        (*p).lifetime = s.lifetime;
        (*p).alpha = s.alpha;
        (*p).color = color_plane(col_of(s.color));
        (*p).width = s.width;
        (*p).length = s.length;
        (*p).drip_timer = s.drip_timer;
        (*p).spin_rate = s.spin_rate;
        true
    }
}
