//! Port of `AParticleFX` (`UnParticleFX.cpp`, oracle ref 5782e7f).
//!
//! Bit-exact against the C++ oracle built with clang `-ffp-contract=on`, which
//! contracts only within one source expression. Each fused site is written with
//! `mul_add` and carries a `// Fused:` comment. Header-level sites (Size, dot,
//! transform) live in `vec.rs` and `prim.rs`; `cross` is `fpoly::cross`.
//!
//! Native and canonical owners run the same [`ParticleFxOps`] policy. The SDK
//! adapter captures foreign exceptions; the canonical adapter uses Rust-owned
//! lists, world queries, resource owners and statistics without native dispatch.
//! Both retain scoped mesh vertices and the original particle constructor's
//! defined scalar defaults; unused particle bytes remain opaque.

use crate::fpoly::cross;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr;

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
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Hp2ParticleFxServices {
    /// `Hp2ParticleServices` (`appFrand`, sin/cos tables, wind, traces).
    pub particle: *const Hp2ParticleServices,
    /// Native typed stack particle scope; invokes the Rust continuation once.
    pub with_particle: unsafe extern "C" fn(
        *const FVector,
        unsafe extern "C" fn(*mut c_void, *mut UParticle) -> i32,
        *mut c_void,
    ) -> i32,
    pub app_frand_range: unsafe extern "C" fn(a0: f32, a1: f32, result: *mut f32) -> i32,
    pub app_rand_range_int: unsafe extern "C" fn(a0: i32, a1: i32, result: *mut i32) -> i32,
    pub app_exp: unsafe extern "C" fn(a0: f64, result: *mut f64) -> i32,
    pub sqrt_approx: unsafe extern "C" fn(a0: f32, result: *mut f32) -> i32,
    pub sin_float: unsafe extern "C" fn(a0: f32, result: *mut f32) -> i32,
    pub cos_float: unsafe extern "C" fn(a0: f32, result: *mut f32) -> i32,
    pub actor_to_local: unsafe extern "C" fn(a0: *mut c_void, a1: *mut c_void) -> i32,
    pub actor_zone_gravity: unsafe extern "C" fn(a0: *mut c_void, a1: *mut c_void) -> i32,
    pub actor_lod_bias: unsafe extern "C" fn(a0: *mut c_void, result: *mut f32) -> i32,
    pub level_time_seconds: unsafe extern "C" fn(a0: *mut c_void, result: *mut f64) -> i32,
    pub level_destroy_actor: unsafe extern "C" fn(a0: *mut c_void) -> i32,
    pub is_editor: unsafe extern "C" fn(result: *mut i32) -> i32,
    pub sync_owner_transient: unsafe extern "C" fn(a0: *mut c_void) -> i32,
    pub parent_defaults: unsafe extern "C" fn(a0: *mut c_void, result: *mut *mut c_void) -> i32,
    pub mesh_render_box: unsafe extern "C" fn(a0: *mut c_void, a1: *mut c_void) -> i32,

    pub mesh_num_tris: unsafe extern "C" fn(a0: *mut c_void, result: *mut i32) -> i32,
    pub mesh_tri_verts: unsafe extern "C" fn(a0: *mut c_void, a1: i32, a2: *mut i32) -> i32,
    pub pattern_num_points: unsafe extern "C" fn(a0: *mut c_void, result: *mut i32) -> i32,
    pub pattern_point: unsafe extern "C" fn(a0: *mut c_void, a1: i32, a2: *mut c_void) -> i32,
    pub palette_color0:
        unsafe extern "C" fn(a0: *mut c_void, a1: *mut c_void, result: *mut i32) -> i32,
    pub list_start: unsafe extern "C" fn(a0: *mut c_void, result: *mut *mut c_void) -> i32,
    pub list_next: unsafe extern "C" fn(a0: *mut c_void, result: *mut *mut c_void) -> i32,
    pub list_add: unsafe extern "C" fn(a0: *mut c_void, a1: *const c_void) -> i32,
    pub list_remove_current: unsafe extern "C" fn(a0: *mut c_void) -> i32,
    pub list_remove_all: unsafe extern "C" fn(a0: *mut c_void) -> i32,
    pub list_remove_from_front: unsafe extern "C" fn(a0: *mut c_void) -> i32,
    pub list_size: unsafe extern "C" fn(a0: *mut c_void, result: *mut i32) -> i32,
    pub list_get_particle:
        unsafe extern "C" fn(a0: *mut c_void, a1: i32, result: *mut *mut c_void) -> i32,
    pub list_update_box: unsafe extern "C" fn(a0: *mut c_void) -> i32,
    pub list_world_box: unsafe extern "C" fn(a0: *mut c_void, a1: *mut c_void) -> i32,
    pub emit_particles_shell:
        unsafe extern "C" fn(a0: *mut c_void, a1: f32, result: *mut i32) -> i32,
    pub update_shell: unsafe extern "C" fn(a0: *mut c_void, a1: f32, result: *mut i32) -> i32,
    pub stat_clock_update: unsafe extern "C" fn() -> i32,
    pub stat_unclock_update: unsafe extern "C" fn() -> i32,
    pub update_particles_shell: unsafe extern "C" fn(fx: *mut c_void, tick: f32) -> i32,
    pub get_sys_params_shell:
        unsafe extern "C" fn(fx: *mut c_void, buffer: *mut c_void, result: *mut *mut c_void) -> i32,
    pub get_params_shell: unsafe extern "C" fn(sys: *const c_void, params: *mut c_void) -> i32,
    pub add_particle_shell: unsafe extern "C" fn(
        fx: *mut c_void,
        id: i32,
        pos: *const c_void,
        params: *const c_void,
        result: *mut i32,
    ) -> i32,
    pub mesh_vertex:
        unsafe extern "C" fn(vertices: *mut c_void, index: i32, out: *mut FVector) -> i32,
    pub with_mesh_vertices: unsafe extern "C" fn(
        *mut AParticleFX,
        unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
        *mut c_void,
    ) -> i32,
    pub stat_particles_emitted: unsafe extern "C" fn(a0: i32) -> i32,
}

/// Source particle-system operations shared by the SDK adapter and canonical owner.
/// Pointer arguments remain live for the synchronous call; no backend retains them.
pub trait ParticleFxOps {
    unsafe fn app_frand(&self) -> Result<f32, i32>;
    fn sin_tab(&self, index: i32) -> f32;
    fn cos_tab(&self, index: i32) -> f32;
    unsafe fn total_wind(&self, level: *mut c_void, location: FVector) -> Result<FVector, i32>;
    unsafe fn update_particle(&self, particle: *mut UParticle, gravity: FVector, damping: f32, wind: FVector, level: *mut c_void, tick: f32, system: *mut c_void) -> Result<bool, i32>;
    unsafe fn with_particle_scope<T, F: FnMut(*mut UParticle) -> Result<T, i32>>(&self, position: *const FVector, body: F) -> Result<T, i32>;
    unsafe fn with_mesh_scope<T, F: FnMut(*mut c_void) -> Result<T, i32>>(&self, fx: *mut AParticleFX, body: F) -> Result<T, i32>;
    unsafe fn update_particles_shell(&self, fx: *mut c_void, tick: f32) -> Result<(), i32>;
    unsafe fn get_sys_params_shell(&self, fx: *mut c_void, buffer: *mut c_void) -> Result<*mut c_void, i32>;
    unsafe fn get_params_shell(&self, sys: *const c_void, params: *mut c_void) -> Result<(), i32>;
    unsafe fn add_particle_shell(&self, fx: *mut c_void, id: i32, pos: *const c_void, params: *const c_void) -> Result<i32, i32>;
    unsafe fn mesh_vertex(&self, vertices: *mut c_void, index: i32, out: *mut FVector) -> Result<(), i32>;
    unsafe fn app_frand_range(&self, from: f32, to: f32) -> Result<f32, i32>;
    unsafe fn app_rand_range_int(&self, from: i32, to: i32) -> Result<i32, i32>;
    unsafe fn app_exp(&self, value: f64) -> Result<f64, i32>;
    unsafe fn sqrt_approx(&self, value: f32) -> Result<f32, i32>;
    unsafe fn sin_float(&self, value: f32) -> Result<f32, i32>;
    unsafe fn cos_float(&self, value: f32) -> Result<f32, i32>;
    unsafe fn actor_to_local(&self, actor: *mut c_void, out: *mut c_void) -> Result<(), i32>;
    unsafe fn actor_zone_gravity(&self, actor: *mut c_void, out: *mut c_void) -> Result<(), i32>;
    unsafe fn actor_lod_bias(&self, actor: *mut c_void) -> Result<f32, i32>;
    unsafe fn level_time_seconds(&self, actor: *mut c_void) -> Result<f64, i32>;
    unsafe fn level_destroy_actor(&self, actor: *mut c_void) -> Result<(), i32>;
    unsafe fn is_editor(&self) -> Result<i32, i32>;
    unsafe fn sync_owner_transient(&self, actor: *mut c_void) -> Result<(), i32>;
    unsafe fn parent_defaults(&self, actor: *mut c_void) -> Result<*mut c_void, i32>;
    unsafe fn mesh_render_box(&self, actor: *mut c_void, out: *mut c_void) -> Result<(), i32>;
    unsafe fn mesh_num_tris(&self, actor: *mut c_void) -> Result<i32, i32>;
    unsafe fn mesh_tri_verts(&self, actor: *mut c_void, index: i32, out: *mut i32) -> Result<(), i32>;
    unsafe fn pattern_num_points(&self, pattern: *mut c_void) -> Result<i32, i32>;
    unsafe fn pattern_point(&self, pattern: *mut c_void, index: i32, out: *mut c_void) -> Result<(), i32>;
    unsafe fn palette_color0(&self, texture: *mut c_void, out: *mut c_void) -> Result<i32, i32>;
    unsafe fn list_start(&self, list: *mut c_void) -> Result<*mut c_void, i32>;
    unsafe fn list_next(&self, list: *mut c_void) -> Result<*mut c_void, i32>;
    unsafe fn list_add(&self, list: *mut c_void, particle: *const c_void) -> Result<(), i32>;
    unsafe fn list_remove_current(&self, list: *mut c_void) -> Result<(), i32>;
    unsafe fn list_remove_all(&self, list: *mut c_void) -> Result<(), i32>;
    unsafe fn list_remove_from_front(&self, list: *mut c_void) -> Result<(), i32>;
    unsafe fn list_size(&self, list: *mut c_void) -> Result<i32, i32>;
    unsafe fn list_get_particle(&self, list: *mut c_void, id: i32) -> Result<*mut c_void, i32>;
    unsafe fn list_update_box(&self, list: *mut c_void) -> Result<(), i32>;
    unsafe fn list_world_box(&self, list: *mut c_void, out: *mut c_void) -> Result<(), i32>;
    unsafe fn emit_particles_shell(&self, fx: *mut c_void, tick: f32) -> Result<i32, i32>;
    unsafe fn update_shell(&self, fx: *mut c_void, tick: f32) -> Result<i32, i32>;
    unsafe fn stat_clock_update(&self) -> Result<(), i32>;
    unsafe fn stat_unclock_update(&self) -> Result<(), i32>;
    unsafe fn stat_particles_emitted(&self, count: i32) -> Result<(), i32>;
}

impl ParticleFxOps for Hp2ParticleFxServices {
    unsafe fn app_frand(&self) -> Result<f32, i32> {
        unsafe { (*self.particle).app_frand() }
    }
    fn sin_tab(&self, index: i32) -> f32 { unsafe { ((*self.particle).sin_tab)(index) } }
    fn cos_tab(&self, index: i32) -> f32 { unsafe { ((*self.particle).cos_tab)(index) } }
    unsafe fn total_wind(&self, level: *mut c_void, location: FVector) -> Result<FVector, i32> {
        let mut out = vec::zero();
        unsafe { (*self.particle).total_wind(level, ptr::from_ref(&location).cast(), ptr::from_mut(&mut out).cast())?; }
        Ok(out)
    }
    unsafe fn update_particle(&self, p: *mut UParticle, gravity: FVector, damping: f32, wind: FVector, level: *mut c_void, tick: f32, system: *mut c_void) -> Result<bool, i32> {
        unsafe { particle::update(p, gravity, damping, wind, level, tick, system, &*self.particle) }
    }
    unsafe fn with_particle_scope<T, F: FnMut(*mut UParticle) -> Result<T, i32>>(&self, position: *const FVector, body: F) -> Result<T, i32> {
        unsafe { with_native_particle(self, position, body) }
    }
    unsafe fn with_mesh_scope<T, F: FnMut(*mut c_void) -> Result<T, i32>>(&self, fx: *mut AParticleFX, body: F) -> Result<T, i32> {
        unsafe { with_mesh_vertices(self, fx, body) }
    }
    unsafe fn update_particles_shell(&self, fx: *mut c_void, tick: f32) -> Result<(), i32> {
        let status = unsafe { (self.update_particles_shell)(fx, tick) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn get_sys_params_shell(
        &self,
        fx: *mut c_void,
        buffer: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        let mut result = MaybeUninit::<*mut c_void>::uninit();
        let status = unsafe { (self.get_sys_params_shell)(fx, buffer, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn get_params_shell(&self, sys: *const c_void, params: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.get_params_shell)(sys, params) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn add_particle_shell(
        &self,
        fx: *mut c_void,
        id: i32,
        pos: *const c_void,
        params: *const c_void,
    ) -> Result<i32, i32> {
        let mut result = MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.add_particle_shell)(fx, id, pos, params, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn mesh_vertex(
        &self,
        vertices: *mut c_void,
        index: i32,
        out: *mut FVector,
    ) -> Result<(), i32> {
        let status = unsafe { (self.mesh_vertex)(vertices, index, out) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    #[inline]
    unsafe fn app_frand_range(&self, a0: f32, a1: f32) -> Result<f32, i32> { let mut result = MaybeUninit::<f32>::uninit();
    let status = unsafe { (self.app_frand_range)(a0, a1, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn app_rand_range_int(&self, a0: i32, a1: i32) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.app_rand_range_int)(a0, a1, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn app_exp(&self, a0: f64) -> Result<f64, i32> { let mut result = MaybeUninit::<f64>::uninit();
    let status = unsafe { (self.app_exp)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn sqrt_approx(&self, a0: f32) -> Result<f32, i32> { let mut result = MaybeUninit::<f32>::uninit();
    let status = unsafe { (self.sqrt_approx)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn sin_float(&self, a0: f32) -> Result<f32, i32> { let mut result = MaybeUninit::<f32>::uninit();
    let status = unsafe { (self.sin_float)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn cos_float(&self, a0: f32) -> Result<f32, i32> { let mut result = MaybeUninit::<f32>::uninit();
    let status = unsafe { (self.cos_float)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn actor_to_local(&self, a0: *mut c_void,
    a1: *mut c_void,) -> Result<(), i32> { let status = unsafe { (self.actor_to_local)(a0, a1) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn actor_zone_gravity(&self, a0: *mut c_void,
    a1: *mut c_void,) -> Result<(), i32> { let status = unsafe { (self.actor_zone_gravity)(a0, a1) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn actor_lod_bias(&self, a0: *mut c_void) -> Result<f32, i32> { let mut result = MaybeUninit::<f32>::uninit();
    let status = unsafe { (self.actor_lod_bias)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn level_time_seconds(&self, a0: *mut c_void) -> Result<f64, i32> { let mut result = MaybeUninit::<f64>::uninit();
    let status = unsafe { (self.level_time_seconds)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn level_destroy_actor(&self, a0: *mut c_void) -> Result<(), i32> { let status = unsafe { (self.level_destroy_actor)(a0) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn is_editor(&self) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.is_editor)(result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn sync_owner_transient(&self, a0: *mut c_void) -> Result<(), i32> { let status = unsafe { (self.sync_owner_transient)(a0) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn parent_defaults(&self, a0: *mut c_void) -> Result<*mut c_void, i32> { let mut result = MaybeUninit::<*mut c_void>::uninit();
    let status = unsafe { (self.parent_defaults)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn mesh_render_box(&self, a0: *mut c_void,
    a1: *mut c_void,) -> Result<(), i32> { let status = unsafe { (self.mesh_render_box)(a0, a1) };
    if status == 0 { Ok(()) } else { Err(status) } }

    #[inline]
    unsafe fn mesh_num_tris(&self, a0: *mut c_void) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.mesh_num_tris)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn mesh_tri_verts(&self, a0: *mut c_void,
    a1: i32,
    a2: *mut i32,) -> Result<(), i32> { let status = unsafe { (self.mesh_tri_verts)(a0, a1, a2) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn pattern_num_points(&self, a0: *mut c_void) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.pattern_num_points)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn pattern_point(&self, a0: *mut c_void,
    a1: i32,
    a2: *mut c_void,) -> Result<(), i32> { let status = unsafe { (self.pattern_point)(a0, a1, a2) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn palette_color0(&self, a0: *mut c_void,
    a1: *mut c_void,) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.palette_color0)(a0, a1, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn list_start(&self, a0: *mut c_void) -> Result<*mut c_void, i32> { let mut result = MaybeUninit::<*mut c_void>::uninit();
    let status = unsafe { (self.list_start)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn list_next(&self, a0: *mut c_void) -> Result<*mut c_void, i32> { let mut result = MaybeUninit::<*mut c_void>::uninit();
    let status = unsafe { (self.list_next)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn list_add(&self, a0: *mut c_void, a1: *const c_void) -> Result<(), i32> { let status = unsafe { (self.list_add)(a0, a1) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn list_remove_current(&self, a0: *mut c_void) -> Result<(), i32> { let status = unsafe { (self.list_remove_current)(a0) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn list_remove_all(&self, a0: *mut c_void) -> Result<(), i32> { let status = unsafe { (self.list_remove_all)(a0) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn list_remove_from_front(&self, a0: *mut c_void) -> Result<(), i32> { let status = unsafe { (self.list_remove_from_front)(a0) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn list_size(&self, a0: *mut c_void) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.list_size)(a0, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn list_get_particle(&self, a0: *mut c_void,
    a1: i32,) -> Result<*mut c_void, i32> { let mut result = MaybeUninit::<*mut c_void>::uninit();
    let status = unsafe { (self.list_get_particle)(a0, a1, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn list_update_box(&self, a0: *mut c_void) -> Result<(), i32> { let status = unsafe { (self.list_update_box)(a0) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn list_world_box(&self, a0: *mut c_void,
    a1: *mut c_void,) -> Result<(), i32> { let status = unsafe { (self.list_world_box)(a0, a1) };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn emit_particles_shell(&self, a0: *mut c_void, a1: f32) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.emit_particles_shell)(a0, a1, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn update_shell(&self, a0: *mut c_void, a1: f32) -> Result<i32, i32> { let mut result = MaybeUninit::<i32>::uninit();
    let status = unsafe { (self.update_shell)(a0, a1, result.as_mut_ptr()) };
    if status == 0 {
        Ok(unsafe { result.assume_init() })
    } else {
        Err(status)
    } }
    #[inline]
    unsafe fn stat_clock_update(&self) -> Result<(), i32> { let status = unsafe { (self.stat_clock_update)() };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn stat_unclock_update(&self) -> Result<(), i32> { let status = unsafe { (self.stat_unclock_update)() };
    if status == 0 { Ok(()) } else { Err(status) } }
    #[inline]
    unsafe fn stat_particles_emitted(&self, a0: i32) -> Result<(), i32> { let status = unsafe { (self.stat_particles_emitted)(a0) };
    if status == 0 { Ok(()) } else { Err(status) } }
}

static mut PARTICLE_FX_SERVICES: Option<Hp2ParticleFxServices> = None;

/// # Safety
/// `value` is the real, immutable `GHp2ParticleFxServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2ParticleFxServices) {
    unsafe { (&raw mut PARTICLE_FX_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Result<Hp2ParticleFxServices, i32> {
    unsafe { (&raw const PARTICLE_FX_SERVICES).read().ok_or(1) }
}

/// Construct the actual native local, keeping its lifetime inside the checked leaf.
unsafe fn with_native_particle<T, F: FnMut(*mut UParticle) -> Result<T, i32>>(
    svc: &Hp2ParticleFxServices,
    position: *const FVector,
    body: F,
) -> Result<T, i32> {
    struct Context<T, F> {
        body: F,
        result: MaybeUninit<T>,
    }
    unsafe extern "C" fn invoke<T, F: FnMut(*mut UParticle) -> Result<T, i32>>(
        context: *mut c_void,
        particle: *mut UParticle,
    ) -> i32 {
        let context = unsafe { &mut *context.cast::<Context<T, F>>() };
        match (context.body)(particle) {
            Ok(value) => {
                context.result.write(value);
                0
            }
            Err(status) => status,
        }
    }
    let mut context = Context {
        body,
        result: MaybeUninit::uninit(),
    };
    let status = unsafe {
        (svc.with_particle)(
            position,
            invoke::<T, F>,
            (&mut context as *mut Context<T, F>).cast(),
        )
    };
    if status == 0 {
        Ok(unsafe { context.result.assume_init() })
    } else {
        Err(status)
    }
}
unsafe fn with_mesh_vertices<T, F: FnMut(*mut c_void) -> Result<T, i32>>(
    svc: &Hp2ParticleFxServices,
    fx: *mut AParticleFX,
    body: F,
) -> Result<T, i32> {
    struct Context<T, F> {
        body: F,
        result: MaybeUninit<T>,
    }
    unsafe extern "C" fn invoke<T, F: FnMut(*mut c_void) -> Result<T, i32>>(
        context: *mut c_void,
        particle: *mut c_void,
    ) -> i32 {
        let context = unsafe { &mut *context.cast::<Context<T, F>>() };
        match (context.body)(particle) {
            Ok(value) => {
                context.result.write(value);
                0
            }
            Err(status) => status,
        }
    }
    let mut context = Context {
        body,
        result: MaybeUninit::uninit(),
    };
    let status = unsafe {
        (svc.with_mesh_vertices)(
            fx,
            invoke::<T, F>,
            (&mut context as *mut Context<T, F>).cast(),
        )
    };
    if status == 0 {
        Ok(unsafe { context.result.assume_init() })
    } else {
        Err(status)
    }
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
/// `s` must implement the synchronous particle operation contract.
unsafe fn frand(s: &impl ParticleFxOps) -> Result<f32, i32> {
    unsafe { s.app_frand() }
}

/// # Safety
/// `s` must be populated.
unsafe fn frand_range(s: &impl ParticleFxOps, lo: f32, hi: f32) -> Result<f32, i32> {
    Ok(unsafe { unsafe { s.app_frand_range(lo, hi)? } })
}

/// # Safety
/// `s` must be populated.
unsafe fn sin_f(s: &impl ParticleFxOps, x: f32) -> Result<f32, i32> {
    Ok(unsafe { unsafe { s.sin_float(x)? } })
}

/// # Safety
/// `s` must be populated.
unsafe fn cos_f(s: &impl ParticleFxOps, x: f32) -> Result<f32, i32> {
    Ok(unsafe { unsafe { s.cos_float(x)? } })
}

/// `FCoords` for `UnitCoords * Location * Rotation`.
///
/// # Safety
/// `svc` must implement the synchronous particle operation contract.
unsafe fn fx_to_world(loc: FVector, rot: FRotator, svc: &impl ParticleFxOps) -> FCoords {
    unsafe {
        let sin = |i: i32| svc.sin_tab(i);
        let cos = |i: i32| svc.cos_tab(i);
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
unsafe fn to_local(fx: *mut AParticleFX, svc: &impl ParticleFxOps) -> Result<FCoords, i32> {
    Ok(unsafe {
        unsafe {
            let mut c = zero_coords();
            svc.actor_to_local(fx.cast(), (&mut c as *mut FCoords).cast())?;
            c
        }
    })
}

/// `Level->Model`-based total wind: `AWind::GetTotalWind(Level, loc)`.
///
/// # Safety
/// `level` must be a live `ULevel` pointer.
unsafe fn total_wind(
    svc: &impl ParticleFxOps,
    level: *mut c_void,
    loc: FVector,
) -> Result<FVector, i32> {
    unsafe { svc.total_wind(level, loc) }
}

/// `Info->ZoneGravity * GravityModifier + Gravity`, with the zone-or-level
/// fallback handled by the `ActorZoneGravity` thunk.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
unsafe fn system_gravity(
    fx: *mut AParticleFX,
    svc: &impl ParticleFxOps,
) -> Result<FVector, i32> {
    Ok(unsafe {
        unsafe {
            let mut zg = vec::zero();
            svc.actor_zone_gravity(fx.cast(), (&mut zg as *mut FVector).cast())?;
            vec::add(vec::scale(zg, (*fx).gravity_modifier), (*fx).gravity)
        }
    })
}

/// `UParticleList` world box (copy of `ParticleList->WorldBox`).
///
/// # Safety
/// `list` must be a live `UParticleList`.
unsafe fn world_box(svc: &impl ParticleFxOps, list: *mut c_void) -> Result<Aabb, i32> {
    Ok(unsafe {
        unsafe {
            let mut b: FBox = std::mem::zeroed();
            svc.list_world_box(list, (&mut b as *mut FBox).cast())?;
            Aabb {
                min: b.min,
                max: b.max,
                valid: b.is_valid != 0,
            }
        }
    })
}

/// `Hp2ParticleFxServices` pattern point `Pattern->Points(i)`.
///
/// # Safety
/// `pat` must be a live `UGesture`; `i` must be in range.
unsafe fn pattern_point(
    svc: &impl ParticleFxOps,
    pat: *mut c_void,
    i: i32,
) -> Result<FVector, i32> {
    Ok(unsafe {
        unsafe {
            let mut v = vec::zero();
            svc.pattern_point(pat, i, (&mut v as *mut FVector).cast())?;
            v
        }
    })
}

/// `RandValue(P)`: `Base + Rand*appFrand()`, drawn only when `Rand != 0`.
///
/// # Safety
/// `s` must be populated.
unsafe fn rand_value(p: FFloatParams, s: &impl ParticleFxOps) -> Result<f32, i32> {
    Ok(unsafe {
        unsafe {
            if p.rand != 0.0 {
                // Fused: Base + Rand*appFrand()
                p.rand.mul_add(frand(s)?, p.base)
            } else {
                p.base
            }
        }
    })
}

/// `ColorStart.Rand.IntValue()` path: `Base + Rand*appFrand()` on colours.
///
/// # Safety
/// `s` must be populated.
unsafe fn rand_color(p: FColorParams, s: &impl ParticleFxOps) -> Result<Col, i32> {
    Ok(unsafe {
        unsafe {
            let rand = p.rand;
            let base = p.base;
            if rand != [0; 4] {
                col_add(base, col_mul(rand, frand(s)?))
            } else {
                base
            }
        }
    })
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
    svc: &impl ParticleFxOps,
) -> Result<*mut AParticleFX, i32> {
    Ok(unsafe {
        unsafe {
            let pb = (*fx).parent_blend;
            if pb <= 0.0 {
                return Ok(fx);
            }
            let parent = svc.parent_defaults(fx.cast())?.cast::<AParticleFX>();
            if pb >= 1.0 {
                return Ok(parent);
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
    })
}

/// `AParticleFX::GetParams` (TU L861–932). Draw order is part of the contract.
///
/// # Safety
/// `sys` must point to a live `AParticleFX` (or blended copy); `out` must be
/// writable.
pub unsafe fn get_params(
    sys: *const AParticleFX,
    out: *mut FParams,
    svc: &impl ParticleFxOps,
) -> Result<(), i32> {
    Ok(unsafe {
        unsafe {
            let s = sys;
            let o = out;
            (*o).angular_spread_width = rand_value((*s).angular_spread_width, svc)?;
            (*o).angular_spread_height = rand_value((*s).angular_spread_height, svc)?;
            (*o).source_width = rand_value((*s).source_width, svc)?;
            (*o).source_depth = rand_value((*s).source_depth, svc)?;
            (*o).source_height = rand_value((*s).source_height, svc)?;
            (*o).speed = rand_value((*s).speed, svc)?;
            (*o).lifetime = rand_value((*s).lifetime, svc)?;
            (*o).alpha_start = rand_value((*s).alpha_start, svc)?;
            (*o).alpha_end = rand_value((*s).alpha_end, svc)?;
            (*o).spin_rate = rand_value((*s).spin_rate, svc)?;
            (*o).drip_time = rand_value((*s).drip_time, svc)?;
            (*o).color_start = fcolor_of(rand_color((*s).color_start, svc)?);
            (*o).color_end = fcolor_of(rand_color((*s).color_end, svc)?);
            let size_rand = frand(svc)?;
            // Fused: SizeWidth.Base + SizeRand*SizeWidth.Rand
            (*o).size_width = (*s)
                .size_width
                .rand
                .mul_add(size_rand, (*s).size_width.base);
            // Fused: SizeLength.Base + SizeRand*SizeLength.Rand
            (*o).size_length = (*s)
                .size_length
                .rand
                .mul_add(size_rand, (*s).size_length.base);
            (*o).size_end_scale = rand_value((*s).size_end_scale, svc)?;
        }
    })
}

/// `AParticleFX::AddParticle` (TU L936–1050). Returns `false` when `params` is
/// null or there is no particle list.
///
/// Native construction initializes consumed default fields without normalizing unused bytes.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `pos` and `params` must be valid
/// when non-null.
pub unsafe fn add_particle(
    fx: *mut AParticleFX,
    id: i32,
    pos: *const FVector,
    params: *const FParams,
    svc: &impl ParticleFxOps,
) -> Result<bool, i32> {
    Ok(unsafe {
        unsafe {
            if params.is_null() {
                return Ok(false);
            }
            let list = (*fx).particle_list;
            if list.is_null() {
                return Ok(false);
            }
            let p = params;
            let position = *pos;

            svc.with_particle_scope(&position, |part| {
                (*part).id = id;
                (*part).priority_tag = 0;

                (*part).width = (*p).size_width;
                (*part).width_full = (*p).size_width;
                (*part).length = (*p).size_length;
                (*part).length_full = (*p).size_length;
                if (*fx).size_grow_period > 0.0 {
                    (*part).width = 0.0;
                    (*part).length = 0.0;
                }

                (*part).lifetime = (*p).lifetime;
                if (*part).lifetime > 0.01 {
                    // Fused: (SizeWidth*SizeEndScale - SizeWidth)
                    (*part).width_delta = (*p)
                        .size_width
                        .mul_add((*p).size_end_scale, -(*p).size_width)
                        / (*part).lifetime;
                    (*part).length_delta = (*p)
                        .size_length
                        .mul_add((*p).size_end_scale, -(*p).size_length)
                        / (*part).lifetime;
                } else {
                    (*part).width_delta = 0.0;
                    (*part).length_delta = 0.0;
                }

                let to_world = fx_to_world((*fx).last_update_location, (*fx).base.rotation, svc);

                // Literal 0.01745f, not DEGREES_TO_RADIANS (TU L975, L978).
                let width_theta = (*p).angular_spread_width * 0.01745;
                let width_variance = frand_range(svc, -width_theta, width_theta)?;
                let height_theta = (*p).angular_spread_height * 0.01745;
                let height_variance = frand_range(svc, -height_theta, height_theta)?;

                let cos_height = cos_f(svc, height_variance)?;
                let local_rot = fvec(
                    cos_f(svc, width_variance)? * cos_height,
                    sin_f(svc, width_variance)? * cos_height,
                    sin_f(svc, height_variance)?,
                );
                let world_rot = prim::transform_vector(local_rot, &to_world);
                (*part).velocity = vec::scale(world_rot, (*p).speed);

                (*part).color = color_plane(col_of((*p).color_start));
                (*part).alpha = (*p).alpha_start;
                (*part).alpha_full = (*p).alpha_start;
                if (*fx).alpha_grow_period > 0.0 {
                    (*part).alpha = 0.0;
                }

                (*part).spin_rate = (*p).spin_rate;
                (*part).drip_timer = (*p).drip_time;
                if (*part).drip_timer > 0.0 {
                    (*part).width_delta = (*part).width / (*part).drip_timer;
                    (*part).length_delta = (*part).length / (*part).drip_timer;
                    (*part).width = 0.0;
                    (*part).length = 0.0;
                }

                let palette = (*fx).color_palette;
                let mut palette_plane = FPlane {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                };
                if !palette.is_null()
                    && svc.palette_color0(palette, (&mut palette_plane as *mut FPlane).cast())? != 0
                {
                    (*part).color = palette_plane;
                } else {
                    if (*part).lifetime > (*fx).color_delay {
                        let life_recip = 1.0 / ((*part).lifetime - (*fx).color_delay);
                        let color_recip = 1.0f32 / 255.0 * life_recip;
                        let cs = col_of((*p).color_start);
                        let ce = col_of((*p).color_end);
                        (*part).color_delta.x =
                            (i32::from(ce[0]) - i32::from(cs[0])) as f32 * color_recip;
                        (*part).color_delta.y =
                            (i32::from(ce[1]) - i32::from(cs[1])) as f32 * color_recip;
                        (*part).color_delta.z =
                            (i32::from(ce[2]) - i32::from(cs[2])) as f32 * color_recip;
                    } else {
                        (*part).color_delta.x = 0.0;
                        (*part).color_delta.y = 0.0;
                        (*part).color_delta.z = 0.0;
                    }
                    (*part).color_delta.w = 0.0;

                    if (*part).lifetime > (*fx).alpha_delay + 0.001 {
                        let life_recip = 1.0 / ((*part).lifetime - (*fx).alpha_delay);
                        (*part).alpha_delta = ((*p).alpha_end - (*p).alpha_start) * life_recip;
                    } else {
                        (*part).alpha_delta = 0.0;
                    }
                }

                svc.list_add((*fx).particle_list, part.cast())?;
                Ok(true)
            })?
        }
    })
}

/// `AParticleFX::EmitParticles` after the C++ `FAppRandTraceNativeObjectScope`
/// (offset 1). Returns the number of particles emitted.
///
/// # Safety
/// `fx` must point to a live `AParticleFX` with a valid particle list.
pub unsafe fn emit_particles(
    fx: *mut AParticleFX,
    tick: f32,
    svc: &impl ParticleFxOps,
) -> Result<i32, i32> {
    Ok(unsafe {
        unsafe {
            (*fx).emit_delay += tick;

            let location = (*fx).base.location;
            let emit_delta = vec::sub((*fx).last_emit_location, location);
            let parent = svc.parent_defaults(fx.cast())?.cast::<AParticleFX>();

            // Emissions = PPS.Base + appFrand()*PPS.Rand
            let pps = (*fx).particles_per_sec;
            // Fused: PPS.Base + appFrand()*PPS.Rand
            let mut emissions = pps.rand.mul_add(frand(svc)?, pps.base);
            if (*fx).parent_blend != 0.0 {
                let ppps = (*parent).particles_per_sec;
                // Fused: ParentFX->PPS.Base + appFrand()*ParentFX->PPS.Rand
                let inner = ppps.rand.mul_add(frand(svc)?, ppps.base);
                // Fused: Emissions += (inner - Emissions) * ParentBlend
                emissions = (inner - emissions).mul_add((*fx).parent_blend, emissions);
            }

            let dist = (*fx).distribution;
            if dist != DIST_UNIFORM {
                emissions *= clamp_c((*fx).lod * 3.0, 0.1, 1.0);
                emissions *= (*fx).emit_delay;
            } else if !(*fx).pattern.is_null() {
                let pat = (*fx).pattern;
                let num = svc.pattern_num_points(pat)?;
                let period = (*fx).period;
                // Fused: (Period.Base + Period.Rand*0.5f) * (Num-1)
                let f = period.rand.mul_add(0.5, period.base) * ((num - 1) as f32);
                let n = std::cmp::min(f as i32, num - 2);
                let del = vec::sub(pattern_point(svc, pat, n)?, pattern_point(svc, pat, n + 1)?);
                emissions =
                    vec::size(del) * (*fx).base.draw_scale * period.rand * ((num - 1) as f32)
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
                return Ok(0);
            }

            let mut new_params: FParams = std::mem::zeroed();
            let to_world = fx_to_world((*fx).base.location, (*fx).base.rotation, svc);

            if (*fx).particles_alive > 0
                && new_particles + svc.list_size((*fx).particle_list)? > (*fx).particles_alive
            {
                if new_particles > (*fx).particles_alive {
                    new_particles = (*fx).particles_alive;
                }
                let excess =
                    new_particles + svc.list_size((*fx).particle_list)? - (*fx).particles_alive;
                for _ in 0..excess {
                    svc.list_remove_from_front((*fx).particle_list)?;
                }
            }

            let level = (*fx).base.x_level;
            let system_gravity = system_gravity(fx, svc)?;
            let mut system_wind = vec::zero();
            if (*fx).damping * (*fx).wind_modifier > 0.0 {
                system_wind = vec::scale(
                    total_wind(svc, level, (*fx).base.location)?,
                    (*fx).wind_modifier,
                );
            }

            svc.with_mesh_scope(fx, |verts| {
                let mut sys_buf = MaybeUninit::<AParticleFX>::uninit();
                let sys = svc
                    .get_sys_params_shell(fx.cast(), sys_buf.as_mut_ptr().cast())?
                    .cast::<AParticleFX>();

                for i in 0..new_particles {
                    svc.get_params_shell(sys.cast(), (&mut new_params as *mut FParams).cast())?;

                    let delta = if (*fx).distribution == DIST_UNIFORM {
                        if emissions > 0.0 {
                            (i + 1) as f32 / emissions
                        } else {
                            0.0
                        }
                    } else {
                        frand(svc)?
                    };

                    svc.with_particle_scope(std::ptr::null(), |part| {
                        (*part).lifetime = new_params.lifetime;
                        // Fused: -EmitStart - EmitDelay*Delta
                        let mut time_delta = (-(*fx).emit_delay).mul_add(delta, -emit_start);
                        if time_delta >= (*part).lifetime {
                            return Ok(());
                        }

                        let world_dir;
                        if (*fx).distribution == DIST_OWNER_MESH
                            && !(*fx).base.owner.is_null()
                            && !(*(*fx).base.owner.cast::<AActor>()).mesh.is_null()
                        {
                            let mut tv = [0i32; 3];
                            let tri = svc
                                .app_rand_range_int(0, svc.mesh_num_tris((*fx).base.owner)? - 1)?;
                            svc.mesh_tri_verts((*fx).base.owner, tri, tv.as_mut_ptr())?;
                            let u = frand(svc)?;
                            let v = frand(svc)?;
                            let mut v0 = vec::zero();
                            svc.mesh_vertex(verts, tv[0], &mut v0)?;
                            let mut v1 = vec::zero();
                            svc.mesh_vertex(verts, tv[1], &mut v1)?;
                            let mut v2 = vec::zero();
                            svc.mesh_vertex(verts, tv[2], &mut v2)?;
                            (*part).position = vec::add(
                                vec::add(vec::scale(v0, 1.0 - u), vec::scale(v1, u * (1.0 - v))),
                                vec::scale(v2, u * v),
                            );
                            world_dir =
                                prim::safe_normal(cross(vec::sub(v1, v0), vec::sub(v2, v0)));
                        } else {
                            if !(*fx).pattern.is_null() {
                                let pat = (*fx).pattern;
                                let num = svc.pattern_num_points(pat)?;
                                let period = (*fx).period;
                                // Fused: (Period.Base + Delta*Period.Rand) * (Num-1)
                                let mut f =
                                    delta.mul_add(period.rand, period.base) * ((num - 1) as f32);
                                let n = std::cmp::min(f as i32, num - 2);
                                f -= n as f32;
                                let pos = vec::add(
                                    vec::scale(pattern_point(svc, pat, n)?, 1.0 - f),
                                    vec::scale(pattern_point(svc, pat, n + 1)?, f),
                                );
                                let pos = vec::scale(
                                    fvec(0.0, pos.x - 0.5, 0.5 - pos.y),
                                    (*fx).base.draw_scale,
                                );
                                (*part).position = prim::transform_point(pos, &to_world);
                            } else {
                                (*part).position = (*fx).base.location;
                            }

                            let width_theta = deg_to_rad(new_params.angular_spread_width);
                            let width_variance = frand_range(svc, -width_theta, width_theta)?;
                            let height_theta = deg_to_rad(new_params.angular_spread_height);
                            let height_variance = frand_range(svc, -height_theta, height_theta)?;
                            let cos_height = cos_f(svc, height_variance)?;
                            let local_rot = fvec(
                                cos_f(svc, width_variance)? * cos_height,
                                sin_f(svc, width_variance)? * cos_height,
                                sin_f(svc, height_variance)?,
                            );
                            world_dir = prim::transform_vector(local_rot, &to_world);
                        }

                        let depth = new_params.source_depth;
                        let width = new_params.source_width;
                        let height = new_params.source_height;
                        let offset = fvec(
                            frand_range(svc, -depth * 0.5, depth * 0.5)?,
                            frand_range(svc, -width * 0.5, width * 0.5)?,
                            frand_range(svc, -height * 0.5, height * 0.5)?,
                        );
                        (*part).position =
                            vec::add((*part).position, prim::transform_vector(offset, &to_world));
                        (*part).position =
                            vec::add((*part).position, vec::scale(emit_delta, 1.0 - delta));

                        (*part).width = new_params.size_width;
                        (*part).width_full = new_params.size_width;
                        (*part).length = new_params.size_length;
                        (*part).length_full = new_params.size_length;
                        if (*fx).size_grow_period > 0.0 {
                            (*part).width = 0.0;
                            (*part).length = 0.0;
                        }

                        (*part).lifetime = new_params.lifetime;
                        (*part).drip_timer = new_params.drip_time;
                        if (*part).drip_timer == 0.0 {
                            if (*part).lifetime > (*fx).size_delay {
                                let t = (*part).lifetime - (*fx).size_delay;
                                // Fused: (SizeWidth*SizeEndScale - SizeWidth)
                                (*part).width_delta = new_params
                                    .size_width
                                    .mul_add(new_params.size_end_scale, -new_params.size_width)
                                    / t;
                                (*part).length_delta = new_params
                                    .size_length
                                    .mul_add(new_params.size_end_scale, -new_params.size_length)
                                    / t;
                            } else {
                                (*part).width_delta = 0.0;
                                (*part).length_delta = 0.0;
                            }
                        } else {
                            (*part).width_delta = (*part).width / (*part).drip_timer;
                            (*part).length_delta = (*part).length / (*part).drip_timer;
                            (*part).width = 0.0;
                            (*part).length = 0.0;
                        }

                        (*part).velocity = vec::scale(world_dir, new_params.speed);
                        if (*fx).b_velocity_relative() && !(*fx).base.owner.is_null() {
                            (*part).velocity = vec::add(
                                (*part).velocity,
                                (*(*fx).base.owner.cast::<AActor>()).velocity,
                            );
                        }

                        (*part).color = color_plane(col_of(new_params.color_start));
                        (*part).alpha = new_params.alpha_start;
                        (*part).alpha_full = new_params.alpha_start;
                        if (*fx).alpha_grow_period > 0.0 {
                            (*part).alpha = 0.0;
                        }

                        (*part).spin_rate = new_params.spin_rate;
                        if (*fx).render_primitive == PPRIM_SHARD {
                            (*part).spin = frand(svc)? * std::f32::consts::PI * 2.0;
                        }

                        let mut palette_plane = FPlane {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                            w: 0.0,
                        };
                        let palette = (*fx).color_palette;
                        if !palette.is_null()
                            && svc.palette_color0(
                                palette,
                                (&mut palette_plane as *mut FPlane).cast(),
                            )? != 0
                        {
                            (*part).color = palette_plane;
                        } else {
                            if (*part).lifetime > (*fx).color_delay {
                                let t = (*part).lifetime - (*fx).color_delay;
                                let cs = col_of(new_params.color_start);
                                let ce = col_of(new_params.color_end);
                                (*part).color_delta.x =
                                    ((i32::from(ce[0]) - i32::from(cs[0])) as f32 / 255.0) / t;
                                (*part).color_delta.y =
                                    ((i32::from(ce[1]) - i32::from(cs[1])) as f32 / 255.0) / t;
                                (*part).color_delta.z =
                                    ((i32::from(ce[2]) - i32::from(cs[2])) as f32 / 255.0) / t;
                                (*part).color_delta.w = 0.0;
                            } else {
                                (*part).color_delta = FPlane {
                                    x: 0.0,
                                    y: 0.0,
                                    z: 0.0,
                                    w: 0.0,
                                };
                            }

                            if (*part).lifetime > (*fx).alpha_delay {
                                (*part).alpha_delta = (new_params.alpha_end
                                    - new_params.alpha_start)
                                    / ((*part).lifetime - (*fx).alpha_delay);
                            } else {
                                (*part).alpha_delta = 0.0;
                            }
                        }

                        let time_slice =
                            !is_zero((*fx).attraction) || (*fx).elasticity > KINDA_SMALL_NUMBER;
                        while time_delta > 0.0 {
                            let tick_slice = if time_delta > MAX_TIMESLICE && time_slice {
                                MAX_TIMESLICE
                            } else {
                                time_delta
                            };
                            let system_damping = if (*fx).damping > 0.0 {
                                svc.app_exp(f64::from(-(*fx).damping * tick_slice))? as f32
                            } else {
                                1.0
                            };
                            let _ = svc.update_particle(
                                part, system_gravity, system_damping, system_wind,
                                (*fx).base.x_level, tick_slice, fx.cast(),
                            )?;
                            time_delta -= tick_slice;
                        }

                        (*fx).current_priority_tag += 1;
                        if (*fx).current_priority_tag > 9 {
                            (*fx).current_priority_tag = 0;
                        }
                        (*part).priority_tag = PRIORITY_TABLE[(*fx).current_priority_tag as usize];

                        svc.list_add((*fx).particle_list, part.cast())?;
                        Ok(())
                    })?;
                }

                if new_particles > 0 {
                    (*fx).emit_delay = 0.0;
                    (*fx).last_emit_location = (*fx).base.location;
                    (*fx).particles_emitted += new_particles;
                    svc.stat_particles_emitted(new_particles)?;
                }
                Ok(new_particles)
            })?
        }
    })
}
/// Typed Engine operations needed by the callback-free particle update and density policies.
pub trait ParticleLifecycleOps: particle::ParticleUpdateOps {

    /// Returns the first particle in a list, or null when it is empty.
    fn list_start(&mut self, list: *mut c_void) -> Result<*mut UParticle, Self::Error>;
    /// Advances the list's current iterator.
    fn list_next(&mut self, list: *mut c_void) -> Result<*mut UParticle, Self::Error>;
    /// Returns the number of particles in a list.
    fn list_size(&mut self, list: *mut c_void) -> Result<usize, Self::Error>;
    /// Removes the current list item without advancing the iterator.
    fn list_remove_current(&mut self, list: *mut c_void) -> Result<(), Self::Error>;
    /// Removes all particles from a list.
    fn list_remove_all(&mut self, list: *mut c_void) -> Result<(), Self::Error>;
    /// Computes the source system gravity.
    fn system_gravity(&mut self, fx: *mut AParticleFX) -> Result<FVector, Self::Error>;
    /// Reads total system wind at a world location.
    fn system_total_wind(
        &mut self,
        level: *mut c_void,
        location: FVector,
    ) -> Result<FVector, Self::Error>;
    /// Evaluates the engine exponential function.
    fn app_exp(&mut self, value: f64) -> Result<f64, Self::Error>;
    /// Returns the actor LOD bias used by non-uniform particle systems.
    fn actor_lod_bias(&mut self, fx: *mut AParticleFX) -> Result<f32, Self::Error>;
    /// Evaluates the engine's approximate square root.
    fn sqrt_approx(&mut self, value: f32) -> Result<f32, Self::Error>;
}

/// Advances particles using typed Engine operations instead of the legacy callback table.
///
/// # Safety
/// `fx` must point to a live particle system with a valid list, and each operation
/// implementation must honor the pointer contracts of its Engine objects.
pub unsafe fn update_particles_with_ops<O: ParticleLifecycleOps>(
    ops: &mut O,
    fx: *mut AParticleFX,
    tick: f32,
) -> Result<(), O::Error> {
    unsafe {
        let list = (*fx).particle_list.cast();
        let max_life = max_lifetime(fx);
        if max_life > 0.0 && tick > max_life {
            ops.list_remove_all(list)?;
            return Ok(());
        }

        let mut particle = ops.list_start(list)?;
        while !particle.is_null() {
            if (*particle).lifetime > 0.0 && (*particle).age + tick >= (*particle).lifetime {
                ops.list_remove_current(list)?;
            }
            particle = ops.list_next(list)?;
        }

        if ops.list_size(list)? == 0 {
            return Ok(());
        }

        let level = (*fx).base.x_level;
        let gravity = ops.system_gravity(fx)?;
        let mut wind = vec::zero();
        if (*fx).damping * (*fx).wind_modifier > 0.0 {
            wind = vec::scale(
                ops.system_total_wind(level, (*fx).base.location)?,
                (*fx).wind_modifier,
            );
        }

        let slice_time = !is_zero((*fx).attraction) || (*fx).elasticity > KINDA_SMALL_NUMBER;
        let mut remaining = tick;
        while remaining > 0.0 {
            let slice = if remaining > MAX_TIMESLICE && slice_time {
                MAX_TIMESLICE
            } else {
                remaining
            };
            let damping = if (*fx).damping > 0.0 {
                ops.app_exp(f64::from(-(*fx).damping * slice))? as f32
            } else {
                1.0
            };
            let mut particle = ops.list_start(list)?;
            while !particle.is_null() {
                let _ = particle::update_with_ops(
                    particle,
                    gravity,
                    damping,
                    wind,
                    level,
                    slice,
                    fx.cast(),
                    ops,
                )?;
                particle = ops.list_next(list)?;
            }
            remaining -= slice;
        }
    }
    Ok(())
}

/// Computes source particle-density sampling for a frame without callback services.
///
/// # Safety
/// `fx` and `frame` must point to live Engine objects and `fx` must own a valid list.
pub unsafe fn lod_particle_density_with_ops<O: ParticleLifecycleOps>(
    ops: &mut O,
    fx: *mut AParticleFX,
    frame: *const FSceneNode,
    screen_frac: f32,
) -> Result<f32, O::Error> {
    unsafe {
        let list = (*fx).particle_list.cast::<c_void>();
        if list.is_null() {
            return Ok(0.0);
        }
        let lod = if (*fx).distribution == DIST_UNIFORM {
            1.0
        } else {
            let bias = ops.actor_lod_bias(fx)?;
            min_c(1.0, bias * 2.0 * ops.sqrt_approx(screen_frac)?)
        };
        let max_priority = math::app_round(lod * 9.0);
        let size = ops.list_size(list)?;
        let sample_lod = 20.0 / size as f32;
        let sample_priority = std::cmp::min(max_priority, math::app_round(sample_lod * 9.0));
        let factor =
            ((max_priority as f64 + 1.0) as f32) / ((sample_priority as f64 + 1.0) as f32);
        let frame = &*frame;
        let size_mul = frame.proj.z / (frame.fx * frame.fy);
        let mut density = 0.0_f32;
        let mut particle = ops.list_start(list)?;
        while !particle.is_null() {
            if (*particle).priority_tag <= sample_priority {
                let distance = vec::sub((*particle).position, frame.coords.origin);
                let distance = ops.sqrt_approx(vec::size_squared(distance))?;
                let size = (*particle).width * (*particle).length * size_mul;
                density += if size >= distance {
                    1.0
                } else {
                    size / distance
                };
            }
            particle = ops.list_next(list)?;
        }
        Ok(density * factor)
    }
}

/// `AParticleFX::UpdateParticles` (TU L223–289).
///
/// # Safety
/// `fx` must point to a live `AParticleFX` with a valid particle list.
pub unsafe fn update_particles(
    fx: *mut AParticleFX,
    tick: f32,
    svc: &impl ParticleFxOps,
) -> Result<(), i32> {
    Ok(unsafe {
        unsafe {
            let max_life = max_lifetime(fx);
            if max_life > 0.0 && tick > max_life {
                svc.list_remove_all((*fx).particle_list)?;
                return Ok(());
            }

            let mut p = svc.list_start((*fx).particle_list)?.cast::<UParticle>();
            while !p.is_null() {
                if (*p).lifetime > 0.0 && (*p).age + tick >= (*p).lifetime {
                    svc.list_remove_current((*fx).particle_list)?;
                }
                p = svc.list_next((*fx).particle_list)?.cast::<UParticle>();
            }

            if svc.list_size((*fx).particle_list)? == 0 {
                return Ok(());
            }

            let level = (*fx).base.x_level;
            let system_gravity = system_gravity(fx, svc)?;
            let mut system_wind = vec::zero();
            if (*fx).damping * (*fx).wind_modifier > 0.0 {
                system_wind = vec::scale(
                    total_wind(svc, level, (*fx).base.location)?,
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
                    svc.app_exp(f64::from(-(*fx).damping * tick_slice))? as f32
                } else {
                    1.0
                };
                let mut p = svc.list_start((*fx).particle_list)?.cast::<UParticle>();
                while !p.is_null() {
                    let _ = svc.update_particle(
                        p, system_gravity, system_damping, system_wind,
                        level, tick_slice, fx.cast(),
                    )?;
                    p = svc.list_next((*fx).particle_list)?.cast::<UParticle>();
                }
                tick -= tick_slice;
            }
        }
    })
}

/// `AParticleFX::Update` (TU L701–804). The C++ shell supplies the
/// `FAppRandTraceNativeObjectScope` (offset 0).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn update(
    fx: *mut AParticleFX,
    step: f32,
    svc: &impl ParticleFxOps,
) -> Result<bool, i32> {
    Ok(unsafe {
        unsafe {
            let list = (*fx).particle_list;
            if list.is_null() {
                return Ok(false);
            }

            svc.stat_clock_update()?;
            svc.sync_owner_transient(fx.cast())?;

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
                let max_step_raw = (max_particles - svc.list_size((*fx).particle_list)? as f32)
                    / min_val((*fx).particles_per_sec);
                let max_step = max_c(max_step_raw, MAX_TIMESLICE);
                step = min_c(step, max_life);
                step = min_c(step, max_step);
            }

            if step <= 0.0 && !(*fx).b_shell_only() {
                svc.stat_unclock_update()?;
                return Ok(true);
            }
            if (*fx).b_shell_only() {
                step = 0.1;
            }

            svc.update_particles_shell(fx.cast(), step)?;
            svc.stat_unclock_update()?;

            if (*fx).particles_max == 0 || (*fx).particles_emitted < (*fx).particles_max {
                if (*fx).b_emit() {
                    svc.emit_particles_shell(fx.cast(), step)?;
                } else {
                    (*fx).last_emit_location = (*fx).base.location;
                }
            } else if svc.list_size((*fx).particle_list)? == 0 {
                if svc.is_editor()? != 0 {
                    (*fx).particles_emitted = 0;
                    (*fx).emit_delay = 0.0;
                } else {
                    svc.level_destroy_actor(fx.cast())?;
                    return Ok(false);
                }
            }

            svc.list_update_box((*fx).particle_list)?;

            (*fx).last_update_location = (*fx).base.location;
            (*fx).last_update_rotation = (*fx).base.rotation;
            true
        }
    })
}

/// `AParticleFX::GetRenderBoundingBox` (TU L121–191). `Exact` is unused in the
/// TU and is not part of the export.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `out` must be writable.
pub unsafe fn get_render_bounding_box(
    fx: *mut AParticleFX,
    out: *mut FCoords,
    svc: &impl ParticleFxOps,
) -> Result<(), i32> {
    Ok(unsafe {
        unsafe {
            let list = (*fx).particle_list;
            let wb = world_box(svc, list)?;
            let dloc = vec::sub((*fx).base.location, (*fx).last_update_location);
            let wb_ext = vec::size_squared(vec::sub(wb.max, wb.min));
            if vec::size_squared(dloc) > max_c(wb_ext, 10000.0) {
                let ok = svc.update_shell(fx.cast(), 0.0)? != 0;
                *out = if ok && !(*fx).particle_list.is_null() {
                    box_get_coords(&world_box(svc, (*fx).particle_list)?)
                } else {
                    zero_coords()
                };
                return Ok(());
            }

            let mut bx = world_box(svc, list)?;
            if (*fx).age > (*fx).elapsed_time {
                let start_size = max_c(max_val((*fx).size_width), max_val((*fx).size_length)) * 0.5;
                let b_const = (*fx).b_steady_state()
                    && vec_eq((*fx).base.location, (*fx).last_update_location);

                let owner = (*fx).base.owner.cast::<AActor>();
                let source_box = if (*fx).distribution == DIST_OWNER_MESH
                    && !owner.is_null()
                    && !(*owner).mesh.is_null()
                {
                    let mut c = zero_coords();
                    svc.mesh_render_box(owner.cast(), (&mut c as *mut FCoords).cast())?;
                    aabb_from_coords(&c)
                } else if !(*fx).pattern.is_null() {
                    let sb =
                        Aabb::from_extent(vec::scale(fvec(0.0, 0.5, 0.5), (*fx).base.draw_scale));
                    aabb_transform(&sb, &to_local(fx, svc)?)
                } else {
                    let ext = vec::scale(
                        fvec(
                            max_val((*fx).source_depth),
                            max_val((*fx).source_width),
                            max_val((*fx).source_height),
                        ),
                        0.5,
                    );
                    aabb_transform(&Aabb::from_extent(ext), &to_local(fx, svc)?)
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

                    let gravity_travel = system_gravity(fx, svc)?;
                    let travel = vec::scale(gravity_travel, square(delta) * 0.5);
                    expand_by_vec(&mut bx, vec::scale(travel, 1.1));
                }
            }

            *out = box_get_coords(&bx);
        }
    })
}

/// `AParticleFX::InitExecution` after `Super::InitExecution()`.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn init_execution(fx: *mut AParticleFX, svc: &impl ParticleFxOps) -> Result<(), i32> {
    Ok(unsafe {
        unsafe {
            let steady = (*fx).particles_max == 0 && (*fx).distribution != DIST_UNIFORM;
            (*fx).set_b_steady_state(steady);
            if steady
                && ((*fx).b_prime() || svc.level_time_seconds(fx.cast())? == 0.0)
                && (*fx).age < max_lifetime(fx)
            {
                (*fx).age = max_lifetime(fx);
            }
            (*fx).elapsed_time = 0.0;
        }
    })
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
    svc: &impl ParticleFxOps,
) -> Result<(), i32> {
    Ok(unsafe {
        unsafe {
            let editor = svc.is_editor()? != 0;
            if tick_type != LEVELTICK_VIEWPORTS_ONLY || editor {
                let list = (*fx).particle_list;
                let has_particles = !list.is_null() && svc.list_size((*fx).particle_list)? > 0;
                if has_particles || max_val((*fx).particles_per_sec) > 0.0 {
                    (*fx).age += delta_seconds;
                    if !editor && (*fx).particles_max > 0 && (*fx).particles_per_sec.base > 0.0 {
                        let sys_life = (*fx).particles_max as f32 / (*fx).particles_per_sec.base
                            + max_lifetime(fx);
                        if (*fx).age > sys_life {
                            svc.level_destroy_actor(fx.cast())?;
                        }
                    }
                }
            }
        }
    })
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
pub unsafe fn lod(
    fx: *mut AParticleFX,
    screen_frac: f32,
    svc: &impl ParticleFxOps,
) -> Result<f32, i32> {
    Ok(unsafe {
        unsafe {
            if (*fx).distribution == DIST_UNIFORM {
                1.0
            } else {
                let bias = svc.actor_lod_bias(fx.cast())?;
                min_c(1.0, bias * 2.0 * svc.sqrt_approx(screen_frac)?)
            }
        }
    })
}

/// `AParticleFX::LodParticles` (TU L1102–1104).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn lod_particles(
    fx: *mut AParticleFX,
    screen_frac: f32,
    svc: &impl ParticleFxOps,
) -> Result<f32, i32> {
    Ok(unsafe {
        unsafe {
            let list = (*fx).particle_list;
            let count = if list.is_null() {
                0
            } else {
                svc.list_size((*fx).particle_list)?
            };
            lod(fx, screen_frac, svc)? * count as f32
        }
    })
}

/// `AParticleFX::LodParticleDensity` (TU L1109–1140).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `frame` must be a valid scene node.
pub unsafe fn lod_particle_density(
    fx: *mut AParticleFX,
    frame: *const FSceneNode,
    screen_frac: f32,
    svc: &impl ParticleFxOps,
) -> Result<f32, i32> {
    Ok(unsafe {
        unsafe {
            let list = (*fx).particle_list;
            if list.is_null() {
                return Ok(0.0);
            }
            let cur_lod = lod(fx, screen_frac, svc)?;
            let max_pt = math::app_round(cur_lod * 9.0);
            let size = svc.list_size((*fx).particle_list)?;
            let sample_lod = 20.0 / size as f32;
            let sample_pt = std::cmp::min(max_pt, math::app_round(sample_lod * 9.0));
            let factor = ((max_pt as f64 + 1.0) as f32) / ((sample_pt as f64 + 1.0) as f32);
            let max_pt = sample_pt;

            let fr = frame;
            let size_mul = (*fr).proj.z / ((*fr).fx * (*fr).fy);
            let mut density = 0.0f32;
            let mut p = svc.list_start((*fx).particle_list)?.cast::<UParticle>();
            while !p.is_null() {
                if (*p).priority_tag <= max_pt {
                    let d = vec::sub((*p).position, (*fr).coords.origin);
                    let dist = svc.sqrt_approx(vec::size_squared(d))?;
                    let sz = (*p).width * (*p).length * size_mul;
                    if sz >= dist {
                        density += 1.0;
                    } else {
                        density += sz / dist;
                    }
                }
                p = svc.list_next((*fx).particle_list)?.cast::<UParticle>();
            }
            density * factor
        }
    })
}

/// `AParticleFX::RecomputeDeltas` (TU L1054–1088).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn recompute_deltas(
    fx: *mut AParticleFX,
    id: i32,
    svc: &impl ParticleFxOps,
) -> Result<bool, i32> {
    Ok(unsafe {
        unsafe {
            let list = (*fx).particle_list;
            if !(*fx).color_palette.is_null() || list.is_null() {
                return Ok(false);
            }
            let p = svc
                .list_get_particle((*fx).particle_list, id)?
                .cast::<UParticle>();
            if p.is_null() {
                return Ok(false);
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
    })
}

/// `execAddParticle` body after `P_FINISH` (TU L1148–1266).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `loc` must be valid.
pub unsafe fn exec_add_particle(
    fx: *mut AParticleFX,
    id: i32,
    loc: *const FVector,
    svc: &impl ParticleFxOps,
) -> Result<(), i32> {
    Ok(unsafe {
        unsafe {
            (*fx).set_b_steady_state(false);
            let mut buf = MaybeUninit::<AParticleFX>::uninit();
            let sys = svc
                .get_sys_params_shell(fx.cast(), buf.as_mut_ptr().cast())?
                .cast::<AParticleFX>();
            let mut np: FParams = std::mem::zeroed();
            svc.get_params_shell(sys.cast(), (&mut np as *mut FParams).cast())?;
            svc.add_particle_shell(fx.cast(), id, loc.cast(), (&np as *const FParams).cast())?;
        }
    })
}

/// `execNumParticles` body (TU L1148–1266): `ParticleList ? Size() : -1`.
///
/// # Safety
/// `fx` must point to a live `AParticleFX`.
pub unsafe fn num_particles(fx: *mut AParticleFX, svc: &impl ParticleFxOps) -> Result<i32, i32> {
    Ok(unsafe {
        unsafe {
            let list = (*fx).particle_list;
            if list.is_null() {
                -1
            } else {
                svc.list_size((*fx).particle_list)?
            }
        }
    })
}

/// `execGetParticleParams` body (TU L1148–1266).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `out` must be writable.
pub unsafe fn get_particle_params(
    fx: *mut AParticleFX,
    id: i32,
    out: *mut FParticleParams,
    svc: &impl ParticleFxOps,
) -> Result<bool, i32> {
    Ok(unsafe {
        unsafe {
            let list = (*fx).particle_list;
            let p = if list.is_null() {
                std::ptr::null_mut()
            } else {
                svc.list_get_particle((*fx).particle_list, id)?
                    .cast::<UParticle>()
            };
            if p.is_null() {
                return Ok(false);
            }
            let o = out;
            (*o).position = (*p).position;
            (*o).velocity = (*p).velocity;
            (*o).lifetime = (*p).lifetime;
            (*o).alpha = (*p).alpha;
            (*o).color = fcolor_of(color_from_plane((*p).color));
            (*o).width = (*p).width;
            (*o).length = (*p).length;
            (*o).drip_timer = (*p).drip_timer;
            (*o).spin_rate = (*p).spin_rate;
            true
        }
    })
}

/// `execSetParticleParams` body (TU L1148–1266).
///
/// # Safety
/// `fx` must point to a live `AParticleFX`; `params` must be valid.
pub unsafe fn set_particle_params(
    fx: *mut AParticleFX,
    id: i32,
    params: *const FParticleParams,
    svc: &impl ParticleFxOps,
) -> Result<bool, i32> {
    Ok(unsafe {
        unsafe {
            (*fx).set_b_steady_state(false);
            let list = (*fx).particle_list;
            let p = if list.is_null() {
                std::ptr::null_mut()
            } else {
                svc.list_get_particle((*fx).particle_list, id)?
                    .cast::<UParticle>()
            };
            if p.is_null() {
                return Ok(false);
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
    })
}
