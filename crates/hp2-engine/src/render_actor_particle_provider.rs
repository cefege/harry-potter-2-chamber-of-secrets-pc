//! Canonical Core host for `URender::DrawParticleSystem`.
//!
//! The shared `hp2_render::particle_system` policy owns primitive selection,
//! draw ordering, clipping and batching. This host supplies the source Engine
//! leaves directly from canonical Core records; no native service table or
//! UObject virtual call is used.

use super::super::super::{lighting, state, RenderGraphState, URender};
use hp2_abi::engine::generated::{
    AActor, AParticleFX, FSceneNode as GeneratedSceneNode, FTextureInfo, UParticle, UTexture,
    UViewport,
};
use hp2_abi::render_particle::FTransTexture;
use hp2_abi::render_scene::{FDynamicSprite, FSceneNode};
use hp2_abi::FPlane;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::mem_owner::MemStackOwner;
use hp2_render::particle::ParticleFrame;
use hp2_render::particle_shard::RenderMath;
use hp2_render::particle_system::{
    self, ColorState, Draw, ParticleDrawKind, ParticlePrimitive, ParticleRenderData,
    ParticleRenderOps, ParticleRenderView, ParticleStatistic, ParticleSystemOps,
};
use std::{cell::{Cell, RefCell}, ffi::c_void, mem, ptr, rc::Rc};

const STATS_ENABLED: bool = hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0;

struct CoreParticleOps<'a> {
    access: CoreAccess<'a>,
    graph: Rc<RenderGraphState>,
    render: *mut URender,
    frame: *mut GeneratedSceneNode,
    sprite: *mut FDynamicSprite,
    fx: *mut AParticleFX,
    texture: Cell<*mut UTexture>,
    info: RefCell<FTextureInfo>,
    locked: Cell<bool>,
    mark: RefCell<Option<hp2_abi::render_span::FMemMark>>,
    light_setup: RefCell<Option<lighting::ActorSetup<'a>>>,
    error: RefCell<Option<RuntimeError>>,
    clocks: Cell<[u32; 3]>,
}

impl<'a> CoreParticleOps<'a> {
    fn record(&self, error: RuntimeError) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    }

    fn remember<T>(&self, result: RuntimeResult<T>) -> Result<T, ()> {
        result.map_err(|error| {
            self.record(error);
        })
    }

    fn take_error(&self) -> Option<RuntimeError> {
        self.error.borrow_mut().take()
    }

    fn viewport(&self) -> RuntimeResult<*mut UViewport> {
        let viewport = unsafe { ptr::addr_of!((*self.frame).viewport).read_unaligned().cast::<UViewport>() };
        if viewport.is_null() {
            return Err(RuntimeError::new("Canonical particle frame has no viewport"));
        }
        Ok(viewport)
    }

    fn device(&self) -> RuntimeResult<*mut hp2_abi::engine::generated::URenderDevice> {
        let device = unsafe { ptr::addr_of!((*self.viewport()?).ren_dev).read_unaligned() };
        if device.is_null() {
            return Err(RuntimeError::new("Canonical particle viewport has no render device"));
        }
        Ok(device.cast())
    }

    fn current_time(&self) -> RuntimeResult<i64> {
        let viewport = self.viewport()?;
        Ok(i64::from_ne_bytes(unsafe { ptr::addr_of!((*viewport).current_time).read_unaligned() }))
    }

    fn texture(&self) -> RuntimeResult<*mut UTexture> {
        let texture = self.texture.get();
        if texture.is_null() {
            return Err(RuntimeError::new("Canonical particle pass has no texture"));
        }
        Ok(texture)
    }

    fn push<T>(&self, count: usize) -> RuntimeResult<*mut T> {
        let bytes = count
            .checked_mul(mem::size_of::<T>())
            .ok_or_else(|| RuntimeError::new("Particle allocation size overflow"))?;
        let size = i32::try_from(bytes)
            .map_err(|_| RuntimeError::new("Particle allocation exceeds engine limits"))?;
        let alignment = i32::try_from(mem::align_of::<T>()).unwrap_or(8).max(4);
        let memory: &MemStackOwner = self.access.scratch_memory();
        let data = unsafe { memory.push_bytes(size, alignment)? };
        if data.is_null() {
            return Err(RuntimeError::new("Particle allocation returned null storage"));
        }
        Ok(data.cast())
    }

    fn stats_add(&self, field: unsafe fn(*mut hp2_abi::engine::generated::FStats) -> *mut i32, amount: i32) {
        if !STATS_ENABLED {
            return;
        }
        let stats = self.graph.stats_ptr();
        if stats.is_null() {
            return;
        }
        // SAFETY: the owner graph's stats record is exclusive to this render thread.
        unsafe {
            let slot = field(stats);
            slot.write_unaligned(slot.read_unaligned().wrapping_add(amount));
        }
    }

    fn particle_stat_value(&self, statistic: ParticleStatistic, amount: i32) {
        match statistic {
            ParticleStatistic::Updated => self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particles_updated) }, amount),
            ParticleStatistic::Seen => self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particles_seen) }, amount),
            ParticleStatistic::Rendered => self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particles_rendered) }, amount),
            ParticleStatistic::Area => self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particle_area) }, amount),
            ParticleStatistic::AreaRendered => self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particle_area_rendered) }, amount),
        }
    }
}

impl RenderMath for CoreParticleOps<'_> {
    fn sin_tab(&self, angle: i32) -> f32 {
        hp2_core::math::sin_tab_inline(angle, hp2_core::math::trig_table())
    }

    fn cos_tab(&self, angle: i32) -> f32 {
        hp2_core::math::cos_tab_inline(angle, hp2_core::math::trig_table())
    }

    fn gmath_div_sqrt_approx(&self, f: f32) -> f32 {
        hp2_core::math::power_inline(
            f,
            hp2_core::math::div_sqrt_man_table(),
            hp2_core::math::div_sqrt_exp_table(),
        )
    }
}

impl ParticleSystemOps for CoreParticleOps<'_> {
    type Error = ();

    fn particle_density(&mut self, sprite: *mut FDynamicSprite) -> Result<bool, ()> {
        let result = (|| {
            if sprite.is_null() {
                return Err(RuntimeError::new("Particle density has no sprite"));
            }
            let actor = unsafe { ptr::addr_of!((*sprite).actor).read_unaligned().cast::<AActor>() };
            if actor.is_null() {
                return Err(RuntimeError::new("Particle density sprite has no actor"));
            }
            let level = unsafe { ptr::addr_of!((*actor).x_level).read_unaligned().cast::<hp2_abi::engine::generated::ULevel>() };
            if level.is_null() {
                return Err(RuntimeError::new("Particle density actor has no Level"));
            }
            let engine = unsafe { ptr::addr_of!((*level).base.engine).read_unaligned().cast::<hp2_abi::engine::generated::UEngine>() };
            if engine.is_null() {
                return Err(RuntimeError::new("Particle density Level has no Engine"));
            }
            let client = unsafe { ptr::addr_of!((*engine).client).read_unaligned().cast::<hp2_abi::engine::generated::UClient>() };
            if client.is_null() {
                return Err(RuntimeError::new("Particle density Engine has no Client"));
            }
            Ok(unsafe { (*client).particle_density != 0 })
        })();
        self.remember(result)
    }

    fn update_particles(&mut self, _sprite: *mut FDynamicSprite) -> Result<bool, ()> {
        self.remember(unsafe {
            crate::particle_fx_owner::update(self.access, self.fx, 0.0)
        })
    }

    fn begin_particle_timing(&mut self) -> Result<(), ()> {
        self.clocks.set([hp2_core::unix_clock::cycles(), 0, 0]);
        Ok(())
    }

    fn begin_particle_render_timing(&mut self) -> Result<(), ()> {
        let clocks = self.clocks.get();
        let now = hp2_core::unix_clock::cycles();
        let update = now.wrapping_sub(clocks[0]) as i32;
        self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particle_time) }, update);
        self.clocks.set([clocks[0], now, clocks[2]]);
        Ok(())
    }

    fn finish_particle_render_timing(&mut self) -> Result<(), ()> {
        let clocks = self.clocks.get();
        let now = hp2_core::unix_clock::cycles();
        let render = now.wrapping_sub(clocks[1]) as i32;
        self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particle_render_time) }, render);
        Ok(())
    }
}

impl ParticleRenderOps for CoreParticleOps<'_> {
    type Error = ();

    fn particle_primitive(&mut self, _sprite: *mut FDynamicSprite) -> Result<ParticlePrimitive, ()> {
        let primitive = unsafe { (*self.fx).render_primitive };
        Ok(ParticlePrimitive::from(primitive as usize))
    }

    fn shell_only(&mut self, _sprite: *mut FDynamicSprite) -> Result<bool, ()> {
        Ok(unsafe { (*self.fx).b_shell_only() })
    }

    fn invalid_particle_primitive(&mut self) -> Result<(), ()> {
        self.record(RuntimeError::new("Unknown particle renderer primitive"));
        Err(())
    }

    fn render_dimensions(
        &mut self,
        _frame: *mut FSceneNode,
        _sprite: *mut FDynamicSprite,
        _kind: ParticleDrawKind,
    ) -> Result<[f32; 3], ()> {
        let max_size = unsafe { (*self.render).max_particle_size };
        let frame = unsafe { &*self.frame };
        Ok([max_size, frame.fx, frame.fy])
    }

    fn texture_available(&mut self, _sprite: *mut FDynamicSprite) -> Result<bool, ()> {
        let texture = unsafe { (*self.fx).textures[0].cast::<UTexture>() };
        self.texture.set(texture);
        Ok(!texture.is_null())
    }

    fn particle_list_nonempty(&mut self, _sprite: *mut FDynamicSprite) -> Result<bool, ()> {
        let list = unsafe { (*self.fx).particle_list };
        if list.is_null() {
            return Ok(false);
        }
        self.remember(crate::particle_list_owner::size(self.access, list)).map(|size| size != 0)
    }

    fn resolve_texture(&mut self, _frame: *mut FSceneNode, _sprite: *mut FDynamicSprite) -> Result<(), ()> {
        let result = (|| {
            let time = self.current_time()?;
            let texture = unsafe { crate::texture_owner::get(self.access, self.texture()?, time)? };
            self.texture.set(texture);
            if texture.is_null() {
                return Err(RuntimeError::new("Canonical particle texture Get returned null"));
            }
            Ok(())
        })();
        self.remember(result)
    }

    fn lock_texture(&mut self, _frame: *mut FSceneNode) -> Result<(), ()> {
        let result = (|| {
            let device = self.device()?;
            let time = self.current_time()?;
            let texture = self.texture()?;
            let info = self.info.borrow_mut();
            let info = ptr::addr_of!(*info).cast_mut();
            unsafe { crate::texture_owner::lock(self.access, texture, info, time, 0, device)? };
            self.locked.set(true);
            Ok(())
        })();
        self.remember(result)
    }

    fn texture_flags(&mut self) -> Result<[u32; 2], ()> {
        let style = unsafe { (*self.fx).base.style as u32 };
        let texture = match self.texture() {
            Ok(texture) => texture,
            Err(error) => { self.record(error); return Err(()); }
        };
        Ok([style, unsafe { (*texture).poly_flags }])
    }

    fn max_vertices(&mut self, _frame: *mut FSceneNode) -> Result<i32, ()> {
        let device = match self.device() {
            Ok(device) => device,
            Err(error) => {
                self.record(error);
                return Err(());
            }
        };
        self.remember(unsafe { crate::render_device_owner::max_vertices(self.access, device) })
    }

    fn mark_memory(&mut self) -> Result<(), ()> {
        *self.mark.borrow_mut() = Some(unsafe { self.access.scratch_memory().mark() });
        Ok(())
    }

    fn allocate_vertices(&mut self, count: usize) -> Result<ptr::NonNull<FTransTexture>, ()> {
        let vertices = match self.push::<FTransTexture>(count) {
            Ok(vertices) => vertices,
            Err(error) => {
                self.record(error);
                return Err(());
            }
        };
        ptr::NonNull::new(vertices).ok_or_else(|| {
            self.record(RuntimeError::new("Particle vertex allocation is null"));
        })
    }

    fn allocate_vertex_pointers(&mut self, count: usize) -> Result<ptr::NonNull<*mut FTransTexture>, ()> {
        let pointers = match self.push::<*mut FTransTexture>(count) {
            Ok(pointers) => pointers,
            Err(error) => {
                self.record(error);
                return Err(());
            }
        };
        ptr::NonNull::new(pointers).ok_or_else(|| {
            self.record(RuntimeError::new("Particle pointer allocation is null"));
        })
    }

    fn allocate_indices(&mut self, count: usize) -> Result<ptr::NonNull<u16>, ()> {
        let indices = match self.push::<u16>(count) {
            Ok(indices) => indices,
            Err(error) => {
                self.record(error);
                return Err(());
            }
        };
        ptr::NonNull::new(indices).ok_or_else(|| {
            self.record(RuntimeError::new("Particle index allocation is null"));
        })
    }

    fn is_unlit(&mut self) -> Result<bool, ()> {
        Ok(unsafe { (*self.fx).base.b_unlit() })
    }

    fn set_white_light(&mut self) -> Result<(), ()> {
        unsafe { (*self.fx).light_color = FPlane { x: 1.0, y: 1.0, z: 1.0, w: 1.0 } };
        Ok(())
    }

    fn setup_lighting(&mut self, _frame: *mut FSceneNode, sprite: *mut FDynamicSprite) -> Result<(), ()> {
        let result = (|| {
            if sprite.is_null() {
                return Err(RuntimeError::new("Particle lighting has no sprite"));
            }
            let leaf = unsafe { (*self.fx).base.region.i_leaf };
            if leaf < 0 {
                return Err(RuntimeError::new("Particle lighting leaf index is negative"));
            }
            let leaf_lights = self.graph.with_leaf_lights(|lights| {
                lights.get(leaf as usize).copied().unwrap_or(ptr::null_mut())
            });
            if leaf_lights.is_null() {
                return Err(RuntimeError::new("Particle lighting leaf has no canonical light link"));
            }
            let (_, setup) = unsafe {
                lighting::setup_actor(
                    self.access,
                    self.frame,
                    self.fx.cast(),
                    leaf_lights,
                    ptr::null_mut(),
                )?
            };
            *self.light_setup.borrow_mut() = Some(setup);
            Ok(())
        })();
        self.remember(result)
    }

    fn light_particles(&mut self, _frame: *mut FSceneNode, _sprite: *mut FDynamicSprite) -> Result<(), ()> {
        unsafe { (*self.fx).light_color = FPlane { x: 0.0, y: 0.0, z: 0.0, w: 1.0 } };
        self.graph.with_light_manager(|| unsafe {
            let manager = hp2_render::light_manager::state();
            if manager.is_null() {
                return;
            }
            let mut color = [(*self.fx).light_color.x, (*self.fx).light_color.y, (*self.fx).light_color.z, (*self.fx).light_color.w];
            let mut light = ptr::addr_of!((*manager).FirstLight).cast::<hp2_render::light_manager::LightInfo>();
            let last = (*manager).LastLight;
            let first = light;
            while light < last {
                let info = &*light;
                hp2_render::light_actor::particle_accumulate(
                    &mut color,
                    &info.ScaledColor,
                    info.MaxSpecDir,
                    info.LightDir,
                );
                light = light.add(1);
            }
            let _ = first;
            (*self.fx).light_color = FPlane { x: color[0], y: color[1], z: color[2], w: color[3] };
        });
        Ok(())
    }

    fn finish_lighting(&mut self) -> Result<(), ()> {
        let Some(setup) = self.light_setup.borrow_mut().take() else {
            return Ok(());
        };
        self.remember(lighting::finish_actor(self.access, setup))
    }

    fn first_particle(&mut self, _sprite: *mut FDynamicSprite) -> Result<*mut UParticle, ()> {
        let list = unsafe { (*self.fx).particle_list };
        if list.is_null() {
            return Ok(ptr::null_mut());
        }
        self.remember(crate::particle_list_owner::start(self.access, list)).map(|p| p.cast())
    }

    fn next_particle(
        &mut self,
        _sprite: *mut FDynamicSprite,
        _particle: *mut UParticle,
    ) -> Result<*mut UParticle, ()> {
        let list = unsafe { (*self.fx).particle_list };
        if list.is_null() {
            return Ok(ptr::null_mut());
        }
        self.remember(crate::particle_list_owner::next(self.access, list)).map(|p| p.cast())
    }

    fn render_view(&mut self, _frame: *mut FSceneNode, _sprite: *mut FDynamicSprite) -> Result<ParticleRenderView, ()> {
        let texture = match self.texture() {
            Ok(texture) => texture,
            Err(error) => {
                self.record(error);
                return Err(());
            }
        };
        let frame = unsafe { &*self.frame };
        Ok(ParticleRenderView {
            frame: ParticleFrame {
                proj_z: frame.proj.z,
                rproj_z: frame.r_proj.z,
                fx15: frame.fx15,
                fy15: frame.fy15,
                prjxm: frame.prj_xm,
                prjxp: frame.prj_xp,
                prjym: frame.prj_ym,
                prjyp: frame.prj_yp,
            },
            coords: frame.coords,
            texture_u: unsafe { (*texture).base.u_size },
            texture_v: unsafe { (*texture).base.v_size },
        })
    }

    fn particle_data(&mut self, particle: *mut UParticle) -> Result<ParticleRenderData, ()> {
        if particle.is_null() {
            self.record(RuntimeError::new("Particle render data has a null particle"));
            return Err(());
        }
        let p = unsafe { &*particle };
        Ok(ParticleRenderData {
            position: p.position,
            velocity: p.velocity,
            width: p.width,
            length: p.length,
            drip_timer: p.drip_timer,
            spin: p.spin,
            spin_rate: p.spin_rate,
            alpha: p.alpha,
            priority_tag: p.priority_tag,
            color: p.color,
        })
    }

    fn set_particle_alpha(&mut self, particle: *mut UParticle, alpha: f32) -> Result<(), ()> {
        if particle.is_null() {
            self.record(RuntimeError::new("Particle alpha write has a null particle"));
            return Err(());
        }
        unsafe { (*particle).alpha = alpha };
        Ok(())
    }

    fn color_state(&mut self) -> Result<ColorState, ()> {
        Ok(ColorState {
            light: unsafe { (*self.fx).light_color },
            opacity: unsafe { (*self.fx).base.opacity },
            style: unsafe { (*self.fx).base.style as u32 },
        })
    }

    fn draw_triangles(&mut self, draw: &Draw, final_draw: bool) -> Result<(), ()> {
        if draw.valid == 0 {
            self.record(RuntimeError::new("Particle batch exceeds the device vertex limit"));
            return Err(());
        }
        let device = match self.device() {
            Ok(device) => device,
            Err(error) => {
                self.record(error);
                return Err(());
            }
        };
        let info = self.info.borrow_mut();
        let info = ptr::addr_of!(*info).cast_mut();
        let span = if self.sprite.is_null() {
            ptr::null_mut()
        } else {
            unsafe { ptr::addr_of!((*self.sprite).span_buffer).read_unaligned().cast::<c_void>() }
        };
        let clocked = !final_draw && STATS_ENABLED;
        if clocked {
            let clocks = self.clocks.get();
            self.clocks.set([clocks[0], clocks[1], hp2_core::unix_clock::cycles()]);
        }
        let result = unsafe {
            crate::render_device_owner::draw_triangles(
                self.access,
                device,
                self.frame.cast(),
                info,
                draw.vertices.cast::<*mut hp2_abi::engine::generated::FTransTexture>().cast(),
                draw.vertex_count,
                draw.indices,
                draw.index_count,
                draw.flags,
                span,
            )
        };
        if clocked {
            let clocks = self.clocks.get();
            let elapsed = hp2_core::unix_clock::cycles().wrapping_sub(clocks[2]) as i32;
            self.stats_add(|s| unsafe { ptr::addr_of_mut!((*s).particle_raster_time) }, elapsed);
        }
        self.remember(result)
    }

    fn allocate_clip_vertex(&mut self) -> Result<ptr::NonNull<FTransTexture>, ()> {
        let vertex = match self.push::<FTransTexture>(1) {
            Ok(vertex) => vertex,
            Err(error) => {
                self.record(error);
                return Err(());
            }
        };
        ptr::NonNull::new(vertex).ok_or_else(|| {
            self.record(RuntimeError::new("Particle clip vertex allocation is null"));
        })
    }

    fn particle_stat(&mut self, statistic: ParticleStatistic, value: f32) -> Result<(), ()> {
        let amount = match statistic {
            ParticleStatistic::Area | ParticleStatistic::AreaRendered => value as i32,
            _ => 1,
        };
        self.particle_stat_value(statistic, amount);
        Ok(())
    }

    fn pop_memory_mark(&mut self) -> Result<(), ()> {
        let Some(mark) = self.mark.borrow_mut().take() else {
            self.record(RuntimeError::new("Particle memory mark was not saved"));
            return Err(());
        };
        unsafe { self.access.scratch_memory().pop(&mark) };
        Ok(())
    }

    fn unlock_texture(&mut self) -> Result<(), ()> {
        if !self.locked.get() {
            return Ok(());
        }
        self.locked.set(false);
        let result = (|| {
            let texture = self.texture()?;
            let info = self.info.borrow_mut();
            let info = ptr::addr_of!(*info).cast_mut();
            unsafe { crate::texture_owner::unlock(self.access, texture, info) }
        })();
        self.remember(result)
    }
}

/// Runs `URender::DrawParticleSystem` through the shared particle policy.
///
/// # Safety
/// `render`, `frame`, `sprite`, and the sprite's particle actor must be live
/// canonical records for the duration of the synchronous draw.
pub(super) unsafe fn draw_particles(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut GeneratedSceneNode,
    sprite: *mut FDynamicSprite,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() || sprite.is_null() {
        return Err(RuntimeError::new("Invalid canonical particle-system draw arguments"));
    }
    let graph = state(access)?;
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("Canonical particle system does not own the active Render"));
    }
    let actor = unsafe { ptr::addr_of!((*sprite).actor).read_unaligned().cast::<AParticleFX>() };
    if actor.is_null() {
        return Err(RuntimeError::new("Canonical particle sprite has no actor"));
    }
    let class = access.native_class("Engine.ParticleFX")?;
    if !unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) } {
        return Err(RuntimeError::new("Canonical particle sprite actor is not an AParticleFX"));
    }
    // The draw host must stay at a stable address: the policy retains no references,
    // but every leaf records errors into it.
    let mut ops = CoreParticleOps {
        access,
        graph,
        render,
        frame,
        sprite,
        fx: actor,
        texture: Cell::new(ptr::null_mut()),
        info: RefCell::new(unsafe { std::mem::zeroed() }),
        locked: Cell::new(false),
        mark: RefCell::new(None),
        light_setup: RefCell::new(None),
        error: RefCell::new(None),
        clocks: Cell::new([0; 3]),
    };
    let result = particle_system::run_with_ops(
        &mut ops,
        frame.cast::<FSceneNode>(),
        ptr::null_mut(),
        sprite,
    );
    if let Some(error) = ops.take_error() {
        return Err(error);
    }
    result.map_err(|()| RuntimeError::new("Canonical particle-system policy failed without a recorded Core error"))
}

impl Drop for CoreParticleOps<'_> {
    fn drop(&mut self) {
        let _ = self.finish_lighting();
        let _ = self.unlock_texture();
        if let Some(mark) = self.mark.borrow_mut().take() {
            unsafe { self.access.scratch_memory().pop(&mark) };
        }
    }
}
