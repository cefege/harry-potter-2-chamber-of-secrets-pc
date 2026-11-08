//! Canonical ParticleFX lifetime and source-shared particle simulation.

use crate::{particle, particle_fx::{self, ParticleFxOps}, particle_list_owner as lists};
use hp2_abi::{FCoords, FPlane, FVector, engine::generated::{AActor, ALevelInfo, AParticleFX, AZoneInfo, FBox, FColor, TArrayRaw, UGesture, UObject, UPalette, UParticle, UTexture, ULevel}};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, core_class_specs::{NativeClassSpec, NativeRecordKind}, object_owner::ClassOps};
use std::{alloc::Layout, cell::RefCell, ffi::c_void, mem::MaybeUninit, ptr};

const BASE: ClassOps = crate::actor_owner::ACTOR_OPERATIONS;
const OPERATIONS: ClassOps = {
    let mut ops = BASE;
    ops.construct = construct;
    ops.destroy = destroy;
    ops.initialize_execution = initialize_execution;
    ops.post_edit_change = post_edit_change;
    ops
};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "ParticleFX", package: "Engine", super_name: Some("Actor"), within: "Object",
    flags: 0, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<AParticleFX>()), operations: Some(OPERATIONS), properties: &[], defaults: &[],
}];

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (BASE.construct)(access, object)?; }
    let fx = object.cast::<AParticleFX>();
    unsafe {
        (*fx).particle_list = ptr::null_mut();
        if (*fx).base.draw_type != 8 { return Err(RuntimeError::new("ParticleFX requires DT_Particles")); }
        if !(*fx).base.b_delete_me() {
            let list = access.construct_object(access.native_class("Engine.ParticleListPriv")?, (*object).outer.cast(), 0, 0, ptr::null_mut())?;
            lists::bind_owner(access, list.cast(), fx.cast())?;
            (*fx).particle_list = list.cast();
        }
        (*fx).age = 0.0;
        (*fx).elapsed_time = 0.0;
        (*fx).set_b_update(true);
        (*fx).lod = 1.0;
    }
    Ok(())
}

unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let fx = object.cast::<AParticleFX>();
    let list = unsafe { (*fx).particle_list };
    if !list.is_null() { unsafe { access.delete_object(list.cast())?; } }
    unsafe { (*fx).particle_list = ptr::null_mut(); (BASE.destroy)(access, object) }
}

unsafe fn initialize_execution(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (BASE.initialize_execution)(access, object)?; }
    let ops = CoreFxOps::new(access);
    let result = unsafe { particle_fx::init_execution(object.cast(), &ops) };
    ops.finish(result)
}

// AParticleFX::PostEditChange is deliberately empty in the source; it does not
// call Actor's editor callback.
unsafe fn post_edit_change(_: CoreAccess<'_>, _: *mut UObject) -> RuntimeResult<()> { Ok(()) }

struct CoreFxOps<'a> {
    access: CoreAccess<'a>,
    error: RefCell<Option<RuntimeError>>,
}
impl<'a> CoreFxOps<'a> {
    fn new(access: CoreAccess<'a>) -> Self { Self { access, error: RefCell::new(None) } }
    fn record<T>(&self, result: RuntimeResult<T>) -> Result<T, i32> {
        result.map_err(|error| { let mut first = self.error.borrow_mut(); if first.is_none() { *first = Some(error); } 1 })
    }
    fn finish<T>(&self, result: Result<T, i32>) -> RuntimeResult<T> {
        result.map_err(|status| self.error.borrow_mut().take().unwrap_or_else(|| RuntimeError::new("ParticleFX operation failed").with_context(&status.to_string())))
    }
    fn stats(&self) -> Result<*mut hp2_abi::engine::generated::FStats, i32> {
        self.record(crate::render_owner::state(self.access)).map(|graph| graph.stats_ptr())
    }
    unsafe fn fast_line(&self, level: *mut ULevel, end: FVector, start: FVector) -> RuntimeResult<bool> {
        if level.is_null() { return Err(RuntimeError::new("Particle has no canonical Level")); }
        unsafe { crate::collision_owner::model_fast_line_check((*level).model.cast(), end, start) }
    }
}

impl ParticleFxOps for CoreFxOps<'_> {
    unsafe fn app_frand(&self) -> Result<f32, i32> { Ok(self.access.rand_range_float(0.0, 1.0)) }
    fn sin_tab(&self, index: i32) -> f32 { hp2_core::math::sin_tab_inline(index, hp2_core::math::trig_table()) }
    fn cos_tab(&self, index: i32) -> f32 { hp2_core::math::cos_tab_inline(index, hp2_core::math::trig_table()) }
    unsafe fn total_wind(&self, _level: *mut c_void, location: FVector) -> Result<FVector, i32> {
        self.record(unsafe { crate::wind_owner::get_total_wind(self.access, &location, |level, end, start| self.fast_line(level.as_ptr(), *end, *start)) })
    }
    unsafe fn update_particle(&self, particle: *mut UParticle, gravity: FVector, damping: f32, wind: FVector, level: *mut c_void, tick: f32, system: *mut c_void) -> Result<bool, i32> {
        let mut physics = CoreParticlePhysics(self);
        unsafe { particle::update_with_ops(particle, gravity, damping, wind, level, tick, system, &mut physics) }
    }
    unsafe fn with_particle_scope<T, F: FnMut(*mut UParticle) -> Result<T, i32>>(&self, position: *const FVector, mut body: F) -> Result<T, i32> {
        let mut storage = MaybeUninit::<UParticle>::uninit();
        let particle = storage.as_mut_ptr();
        let position = if position.is_null() { crate::vec::zero() } else { unsafe { position.read_unaligned() } };
        // Match the native constructor and frozen defined scalar defaults only;
        // untouched bytes stay opaque when copied into the stable particle arena.
        unsafe {
            ptr::addr_of_mut!((*particle).last_position).write(position);
            ptr::addr_of_mut!((*particle).position).write(position);
            ptr::addr_of_mut!((*particle).velocity).write(crate::vec::zero());
            ptr::addr_of_mut!((*particle).age).write(0.0);
            ptr::addr_of_mut!((*particle).id).write(-1);
            ptr::addr_of_mut!((*particle).spin).write(0.0);
            ptr::addr_of_mut!((*particle).chaos_delay).write(0.0);
            ptr::addr_of_mut!((*particle).alpha_delta).write(0.0);
            ptr::addr_of_mut!((*particle).color_delta).write(FPlane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 });
        }
        body(particle)
    }
    unsafe fn with_mesh_scope<T, F: FnMut(*mut c_void) -> Result<T, i32>>(&self, fx: *mut AParticleFX, mut body: F) -> Result<T, i32> {
        let owner = unsafe { (*fx).base.owner.cast::<AActor>() };
        if unsafe { (*fx).distribution } != 2 || owner.is_null() || unsafe { (*owner).mesh.is_null() } {
            let mut array = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            return body((&mut array as *mut TArrayRaw).cast());
        }
        let result = unsafe { crate::render_owner::particle_mesh_frame(self.access, owner, |vertices| {
            let count = i32::try_from(vertices.len()).map_err(|_| RuntimeError::new("Particle mesh vertex count exceeds INT"))?;
            let mut array = TArrayRaw { data: vertices.as_ptr().cast_mut().cast(), num: count, max: count };
            self.finish(body((&mut array as *mut TArrayRaw).cast()))
        }) };
        self.record(result)
    }
    unsafe fn update_particles_shell(&self, fx: *mut c_void, tick: f32) -> Result<(), i32> { unsafe { particle_fx::update_particles(fx.cast(), tick, self) } }
    unsafe fn get_sys_params_shell(&self, fx: *mut c_void, buffer: *mut c_void) -> Result<*mut c_void, i32> { unsafe { particle_fx::get_sys_params(fx.cast(), buffer.cast(), self).map(|value| value.cast()) } }
    unsafe fn get_params_shell(&self, sys: *const c_void, params: *mut c_void) -> Result<(), i32> { unsafe { particle_fx::get_params(sys.cast(), params.cast(), self) } }
    unsafe fn add_particle_shell(&self, fx: *mut c_void, id: i32, pos: *const c_void, params: *const c_void) -> Result<i32, i32> { unsafe { particle_fx::add_particle(fx.cast(), id, pos.cast(), params.cast(), self).map(i32::from) } }
    unsafe fn mesh_vertex(&self, vertices: *mut c_void, index: i32, out: *mut FVector) -> Result<(), i32> {
        let result = (|| { let array = unsafe { vertices.cast::<TArrayRaw>().read_unaligned() }; let value = unsafe { array_element::<FVector>(array, index)? }; unsafe { out.write_unaligned(value); } Ok(()) })();
        self.record(result)
    }
    unsafe fn app_frand_range(&self, from: f32, to: f32) -> Result<f32, i32> { Ok(self.access.rand_range_float(from, to)) }
    unsafe fn app_rand_range_int(&self, from: i32, to: i32) -> Result<i32, i32> { let range = to.wrapping_sub(from); Ok(if range > 0 { (self.access.rand() % range).wrapping_add(from) } else { from }) }
    unsafe fn app_exp(&self, value: f64) -> Result<f64, i32> { Ok(hp2_core::ansi_math::exp(value)) }
    unsafe fn sqrt_approx(&self, value: f32) -> Result<f32, i32> { Ok(hp2_core::math::sqrt_approx_inline(value, hp2_core::math::sqrt_man_table())) }
    unsafe fn sin_float(&self, value: f32) -> Result<f32, i32> { Ok(hp2_core::math::sin_float_inline(value, hp2_core::math::trig_table())) }
    unsafe fn cos_float(&self, value: f32) -> Result<f32, i32> { Ok(hp2_core::math::cos_float_inline(value, hp2_core::math::trig_table())) }
    unsafe fn actor_to_local(&self, actor: *mut c_void, out: *mut c_void) -> Result<(), i32> {
        let actor = actor.cast::<AActor>();
        let sin = |i| self.sin_tab(i); let cos = |i| self.cos_tab(i);
        let trig = crate::coords::Trig { sin: &sin, cos: &cos };
        let mut coords = crate::coords::unit_coords();
        unsafe { crate::coords::div_rot(&mut coords, (*actor).rotation, &trig); crate::coords::div_vec(&mut coords, (*actor).location); out.cast::<FCoords>().write_unaligned(coords); }
        Ok(())
    }
    unsafe fn actor_zone_gravity(&self, actor: *mut c_void, out: *mut c_void) -> Result<(), i32> {
        let actor = actor.cast::<AActor>();
        let zone = unsafe { if (*actor).region.zone.is_null() { (*actor).level } else { (*actor).region.zone } }.cast::<AZoneInfo>();
        if zone.is_null() { return self.record(Err(RuntimeError::new("ParticleFX has no gravity zone"))); }
        unsafe { out.cast::<FVector>().write_unaligned((*zone).zone_gravity); } Ok(())
    }
    unsafe fn actor_lod_bias(&self, actor: *mut c_void) -> Result<f32, i32> { Ok(unsafe { (*actor.cast::<AActor>()).lod_bias }) }
    unsafe fn level_time_seconds(&self, actor: *mut c_void) -> Result<f64, i32> {
        let info = unsafe { (*actor.cast::<AActor>()).level.cast::<ALevelInfo>() };
        if info.is_null() { return self.record(Err(RuntimeError::new("ParticleFX has no LevelInfo"))); }
        Ok(f64::from(unsafe { (*info).time_seconds }))
    }
    unsafe fn level_destroy_actor(&self, actor: *mut c_void) -> Result<(), i32> { let actor = actor.cast::<AActor>(); self.record(unsafe { crate::level_lifecycle_owner::destroy_actor(self.access, (*actor).x_level.cast(), actor, false) }).map(|_| ()) }
    unsafe fn is_editor(&self) -> Result<i32, i32> { Ok(i32::from(self.access.process_mode().is_editor)) }
    unsafe fn sync_owner_transient(&self, actor: *mut c_void) -> Result<(), i32> {
        let actor = actor.cast::<AActor>();
        unsafe { if !(*actor).owner.is_null() && (*actor).owner == (*actor).base && (*(*actor).owner.cast::<UObject>()).object_flags & 0x4000 != 0 { (*actor.cast::<UObject>()).object_flags |= 0x4000; } } Ok(())
    }
    unsafe fn parent_defaults(&self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        let class = unsafe { (*actor.cast::<UObject>()).class.cast::<hp2_abi::engine::generated::UClass>() };
        let parent = unsafe { (*class).super_field.cast() };
        self.record(unsafe { self.access.default_object(parent) }).map(|object| object.cast())
    }
    unsafe fn mesh_render_box(&self, actor: *mut c_void, out: *mut c_void) -> Result<(), i32> { let bounds = self.record(unsafe { crate::render_owner::particle_mesh_bounds(self.access, actor.cast()) })?; unsafe { out.cast::<FCoords>().write_unaligned(bounds); } Ok(()) }
    unsafe fn mesh_num_tris(&self, actor: *mut c_void) -> Result<i32, i32> { self.record(unsafe { crate::render_owner::particle_mesh_triangles(self.access, actor.cast()) }) }
    unsafe fn mesh_tri_verts(&self, actor: *mut c_void, index: i32, out: *mut i32) -> Result<(), i32> { let indices = self.record(unsafe { crate::render_owner::particle_mesh_triangle(self.access, actor.cast(), index) })?; for (index, value) in indices.into_iter().enumerate() { unsafe { out.add(index).write_unaligned(value); } } Ok(()) }
    unsafe fn pattern_num_points(&self, pattern: *mut c_void) -> Result<i32, i32> { Ok(unsafe { (*pattern.cast::<UGesture>()).points.num }) }
    unsafe fn pattern_point(&self, pattern: *mut c_void, index: i32, out: *mut c_void) -> Result<(), i32> { let value = self.record(unsafe { array_element::<FVector>((*pattern.cast::<UGesture>()).points, index) })?; unsafe { out.cast::<FVector>().write_unaligned(value); } Ok(()) }
    unsafe fn palette_color0(&self, texture: *mut c_void, out: *mut c_void) -> Result<i32, i32> {
        let palette = unsafe { (*texture.cast::<UTexture>()).base.palette.cast::<UPalette>() };
        if palette.is_null() { return Ok(0); }
        let color = self.record(unsafe { array_element::<FColor>((*palette).colors, 0) })?;
        unsafe { out.cast::<FPlane>().write_unaligned(color_plane(color)); } Ok(1)
    }
    unsafe fn list_start(&self, list: *mut c_void) -> Result<*mut c_void, i32> { self.record(lists::start(self.access, list)).map(|p| p.cast()) }
    unsafe fn list_next(&self, list: *mut c_void) -> Result<*mut c_void, i32> { self.record(lists::next(self.access, list)).map(|p| p.cast()) }
    unsafe fn list_add(&self, list: *mut c_void, particle: *const c_void) -> Result<(), i32> { self.record(lists::add(self.access, list, particle.cast())) }
    unsafe fn list_remove_current(&self, list: *mut c_void) -> Result<(), i32> { self.record(lists::remove_current(self.access, list)) }
    unsafe fn list_remove_all(&self, list: *mut c_void) -> Result<(), i32> { self.record(lists::remove_all(self.access, list)) }
    unsafe fn list_remove_from_front(&self, list: *mut c_void) -> Result<(), i32> { self.record(lists::remove_from_front(self.access, list)) }
    unsafe fn list_size(&self, list: *mut c_void) -> Result<i32, i32> { let count = self.record(lists::size(self.access, list))?; self.record(i32::try_from(count).map_err(|_| RuntimeError::new("Particle count exceeds INT"))) }
    unsafe fn list_get_particle(&self, list: *mut c_void, id: i32) -> Result<*mut c_void, i32> { self.record(lists::get_particle(self.access, list, id)).map(|p| p.cast()) }
    unsafe fn list_update_box(&self, list: *mut c_void) -> Result<(), i32> { self.record(lists::update_box(self.access, list)).map(|_| ()) }
    unsafe fn list_world_box(&self, list: *mut c_void, out: *mut c_void) -> Result<(), i32> { let bounds = self.record(lists::world_box(self.access, list))?; unsafe { out.cast::<FBox>().write_unaligned(bounds); } Ok(()) }
    unsafe fn emit_particles_shell(&self, fx: *mut c_void, tick: f32) -> Result<i32, i32> { let _clock = StatClock::new(self, true)?; let _trace = TraceScope::new(self, fx.cast(), 1)?; unsafe { particle_fx::emit_particles(fx.cast(), tick, self) } }
    unsafe fn update_shell(&self, fx: *mut c_void, tick: f32) -> Result<i32, i32> { let _trace = TraceScope::new(self, fx.cast(), 0)?; unsafe { particle_fx::update(fx.cast(), tick, self).map(i32::from) } }
    unsafe fn stat_clock_update(&self) -> Result<(), i32> { let stats = self.stats()?; unsafe { (*stats).particle_update_time = (*stats).particle_update_time.wrapping_sub(hp2_core::unix_clock::cycles() as i32); } Ok(()) }
    unsafe fn stat_unclock_update(&self) -> Result<(), i32> { let stats = self.stats()?; unsafe { (*stats).particle_update_time = (*stats).particle_update_time.wrapping_add(hp2_core::unix_clock::cycles() as i32); } Ok(()) }
    unsafe fn stat_particles_emitted(&self, count: i32) -> Result<(), i32> { let stats = self.stats()?; unsafe { (*stats).particles_emitted = (*stats).particles_emitted.wrapping_add(count); } Ok(()) }
}

unsafe fn array_element<T: Copy>(array: TArrayRaw, index: i32) -> RuntimeResult<T> {
    if index < 0 || index >= array.num || array.max < array.num || array.data.is_null() { return Err(RuntimeError::new("Particle array index out of bounds")); }
    Ok(unsafe { array.data.cast::<T>().add(index as usize).read_unaligned() })
}
fn color_plane(color: FColor) -> FPlane { FPlane { x: color.r as f32 / 255.0, y: color.g as f32 / 255.0, z: color.b as f32 / 255.0, w: 0.0 } }

struct CoreParticlePhysics<'host, 'owner>(&'host CoreFxOps<'owner>);
impl particle::ParticleUpdateOps for CoreParticlePhysics<'_, '_> {
    type Error = i32;
    fn app_frand(&mut self) -> Result<f32, i32> { unsafe { self.0.app_frand() } }
    fn sin_tab(&self, index: i32) -> f32 { self.0.sin_tab(index) }
    fn cos_tab(&self, index: i32) -> f32 { self.0.cos_tab(index) }
    fn total_wind(&mut self, level: *mut c_void, location: FVector, out: &mut FVector) -> Result<(), i32> { *out = unsafe { self.0.total_wind(level, location)? }; Ok(()) }
    fn fast_line_check(&mut self, level: *mut c_void, end: FVector, start: FVector) -> Result<i32, i32> { self.0.record(unsafe { self.0.fast_line(level.cast(), end, start) }).map(i32::from) }
    fn system_trace_level_hit(&mut self, system: *mut c_void, end: FVector, start: FVector, normal: &mut FVector, location: &mut FVector) -> Result<i32, i32> {
        let mut hit = crate::levact::check_result(1.0);
        let level = unsafe { (*system.cast::<AActor>()).x_level.cast::<ULevel>() };
        self.0.record(unsafe { crate::level_trace_owner::single_line_check(self.0.access, level, &mut hit, ptr::null_mut(), end, start, 0x06, crate::vec::zero(), 0) })?;
        let info = self.0.record(self.0.access.native_class("Engine.LevelInfo"))?;
        if !hit.actor.is_null() && unsafe { hp2_core::object_model::is_a(hit.actor, info.cast()) } { *normal = hit.normal; *location = hit.location; Ok(1) } else { Ok(0) }
    }
    fn palette_plane(&mut self, texture: *mut c_void, age: f32, lifetime: f32, delay: f32, out: &mut FPlane) -> Result<(), i32> {
        let palette = unsafe { (*texture.cast::<UTexture>()).base.palette.cast::<UPalette>() };
        if !palette.is_null() { let index = if lifetime > 0.0 && age > delay { ((age / lifetime) * 255.0) as i32 } else { 0 }; *out = color_plane(self.0.record(unsafe { array_element::<FColor>((*palette).colors, index) })?); } Ok(())
    }
}

struct StatClock { stats: *mut hp2_abi::engine::generated::FStats, emit: bool }
impl StatClock {
    fn new(ops: &CoreFxOps<'_>, emit: bool) -> Result<Self, i32> { let stats = ops.stats()?; unsafe { let counter = if emit { ptr::addr_of_mut!((*stats).particle_emit_time) } else { ptr::addr_of_mut!((*stats).particle_update_time) }; counter.write(counter.read().wrapping_sub(hp2_core::unix_clock::cycles() as i32)); } Ok(Self { stats, emit }) }
}
impl Drop for StatClock {
    fn drop(&mut self) { unsafe { let counter = if self.emit { ptr::addr_of_mut!((*self.stats).particle_emit_time) } else { ptr::addr_of_mut!((*self.stats).particle_update_time) }; counter.write(counter.read().wrapping_add(hp2_core::unix_clock::cycles() as i32)); } }
}
struct TraceScope(*mut hp2_core::ansi_trace::Context);
impl TraceScope {
    fn new(ops: &CoreFxOps<'_>, actor: *mut AParticleFX, offset: i32) -> Result<Self, i32> {
        if !hp2_core::ansi_trace::with_process_state(|state| state.enabled && !state.flushed) { return Ok(Self(ptr::null_mut())); }
        let path = |object: *mut UObject| -> RuntimeResult<Vec<u8>> {
            if object.is_null() { return Ok(Vec::new()); }
            let text = unsafe { ops.access.object_full_name(object)? };
            let path = text.iter().position(|&unit| unit == 32).map_or(&text[..], |index| &text[index+1..]);
            Ok(path.iter().copied().map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect::<String>().into_bytes())
        };
        let actor_path = ops.record(path(actor.cast()))?;
        let class_path = ops.record(path(unsafe { (*actor.cast::<UObject>()).class.cast() }))?;
        Ok(Self(hp2_core::ansi_trace::with_process_state(|state| hp2_core::ansi_trace::direct_native_scope_begin(state, actor.cast(), ptr::null(), c"Render/ParticleFX", -1, offset, actor_path, class_path, Vec::new()))))
    }
}
impl Drop for TraceScope { fn drop(&mut self) { unsafe { hp2_core::ansi_trace::context_scope_end(self.0); } } }

/// Advances the source ParticleFX simulation in the canonical object graph.
/// # Safety
/// `fx` is a live canonical ParticleFX; every linked world/resource remains live.
pub unsafe fn update(access: CoreAccess<'_>, fx: *mut AParticleFX, step: f32) -> RuntimeResult<bool> {
    let ops = CoreFxOps::new(access); let result = unsafe { ops.update_shell(fx.cast(), step) }; ops.finish(result).map(|value| value != 0)
}
/// # Safety
/// `fx` and its canonical particle list remain live through this synchronous query.
pub unsafe fn render_bounds(access: CoreAccess<'_>, fx: *mut AParticleFX) -> RuntimeResult<FCoords> {
    let ops = CoreFxOps::new(access);
    let mut out = MaybeUninit::<FCoords>::uninit();
    let result = unsafe { particle_fx::get_render_bounding_box(fx, out.as_mut_ptr(), &ops) };
    ops.finish(result)?;
    Ok(unsafe { out.assume_init() })
}
/// # Safety
/// `fx` remains live through the source tick and any actor destruction.
pub unsafe fn tick(access: CoreAccess<'_>, fx: *mut AParticleFX, delta: f32, tick_type: i32) -> RuntimeResult<()> {
    let ops = CoreFxOps::new(access); let result = unsafe { particle_fx::tick(fx, delta, tick_type, &ops) }; ops.finish(result)
}
/// # Safety
/// `fx` is a live canonical ParticleFX record.
pub unsafe fn lod_particles(access: CoreAccess<'_>, fx: *mut AParticleFX, screen_fraction: f32) -> RuntimeResult<f32> {
    let ops = CoreFxOps::new(access); let result = unsafe { particle_fx::lod_particles(fx, screen_fraction, &ops) }; ops.finish(result)
}
/// # Safety
/// `fx`, its list, and the active scene frame remain live.
pub unsafe fn lod_particle_density(access: CoreAccess<'_>, fx: *mut AParticleFX, frame: *const hp2_abi::render_scene::FSceneNode, screen_fraction: f32) -> RuntimeResult<f32> {
    let ops = CoreFxOps::new(access); let result = unsafe { particle_fx::lod_particle_density(fx, frame, screen_fraction, &ops) }; ops.finish(result)
}

/// Returns the source particle count, or -1 for a missing particle list.
/// # Safety
/// `fx` is a live canonical ParticleFX.
pub unsafe fn num_particles(access: CoreAccess<'_>, fx: *mut AParticleFX) -> RuntimeResult<i32> {
    let ops = CoreFxOps::new(access);
    let result = unsafe { particle_fx::num_particles(fx, &ops) };
    ops.finish(result)
}

/// Adds the script-requested particle, retaining the native void-result behavior.
/// # Safety
/// `fx` and all reached world/resource records are live canonical objects.
pub unsafe fn add_particle(access: CoreAccess<'_>, fx: *mut AParticleFX, id: i32, position: &FVector) -> RuntimeResult<()> {
    let ops = CoreFxOps::new(access);
    let result = unsafe { particle_fx::exec_add_particle(fx, id, position, &ops) };
    ops.finish(result)
}

/// Exports a particle's state, preserving the output when its ID is absent.
/// # Safety
/// `fx` is live and `params` points to writable ParticleParams storage.
pub unsafe fn get_particle_params(access: CoreAccess<'_>, fx: *mut AParticleFX, id: i32, params: *mut particle_fx::FParticleParams) -> RuntimeResult<bool> {
    let ops = CoreFxOps::new(access);
    let result = unsafe { particle_fx::get_particle_params(fx, id, params, &ops) };
    ops.finish(result)
}

/// Changes an existing particle without replacing its identity.
/// # Safety
/// `fx` is live and `params` points to readable ParticleParams storage.
pub unsafe fn set_particle_params(access: CoreAccess<'_>, fx: *mut AParticleFX, id: i32, params: *const particle_fx::FParticleParams) -> RuntimeResult<bool> {
    let ops = CoreFxOps::new(access);
    let result = unsafe { particle_fx::set_particle_params(fx, id, params, &ops) };
    ops.finish(result)
}

/// Recomputes the source color/alpha/size deltas for a script-selected particle.
/// # Safety
/// `fx` and its list are live canonical records.
pub unsafe fn recompute_deltas(access: CoreAccess<'_>, fx: *mut AParticleFX, id: i32) -> RuntimeResult<bool> {
    let ops = CoreFxOps::new(access);
    let result = unsafe { particle_fx::recompute_deltas(fx, id, &ops) };
    ops.finish(result)
}
