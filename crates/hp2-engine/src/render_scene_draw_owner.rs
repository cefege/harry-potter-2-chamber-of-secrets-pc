//! Canonical Core host for the complete shared scene-pipeline draw traversal.

use hp2_abi::{
    engine::generated::{
        AActor, ALevelInfo, APlayerPawn, FSceneNode, FTransform, UClient, ULevel, UModel,
        UObject, UViewport, URender, URenderDevice,
    },
    render_scene::{self, FSceneNode as PlainSceneNode},
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::scene_pipeline::draw::{
    DrawOperation, DrawOps, Hp2ScenePipelineDrawPacket,
};
use std::{cell::RefCell, ffi::c_void, mem, ptr};

/// Runs the shared source draw policy against the canonical Engine graph.
///
/// # Safety
/// `render` and `frame` and every record reached from them must remain live for the call.
pub(crate) unsafe fn draw_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Invalid canonical scene draw arguments"));
    }
    let host = CoreDrawOps::new(access, render)?;
    // SAFETY: the caller guarantees the complete scene graph remains live synchronously.
    let result = unsafe {
        hp2_render::scene_pipeline::draw::draw_frame_with(
            &host,
            frame.cast::<PlainSceneNode>(),
        )
    };
    let host_error = host.take_error();
    let traversal_failed = result.is_err();
    let surface_error = host.surface.finish().err();
    let corona_error = host.corona.finish().err();
    if let Some(error) = host_error.or(surface_error).or(corona_error) {
        return Err(error);
    }
    if traversal_failed {
        return Err(RuntimeError::new("Canonical scene draw policy failed"));
    }
    Ok(())
}

struct CoreDrawOps<'a> {
    access: CoreAccess<'a>,
    render: *mut URender,
    stacks: super::CoreSceneStackOps<'a>,
    surface: super::surface::SurfaceState<'a>,
    corona: super::corona::CoronaState<'a>,
    error: RefCell<Option<RuntimeError>>,
}

impl<'a> CoreDrawOps<'a> {
    fn new(access: CoreAccess<'a>, render: *mut URender) -> RuntimeResult<Self> {
        Ok(Self {
            access,
            render,
            stacks: super::CoreSceneStackOps::new(access)?,
            surface: super::surface::SurfaceState::new(access),
            corona: super::corona::CoronaState::new(access),
            error: RefCell::new(None),
        })
    }

    fn remember(&self, error: RuntimeError) {
        let mut first = self.error.borrow_mut();
        if first.is_none() {
            *first = Some(error);
        }
    }

    fn take_error(&self) -> Option<RuntimeError> {
        self.error.borrow_mut().take()
    }

    fn fail<T>(&self, error: RuntimeError) -> Result<T, ()> {
        self.remember(error);
        Err(())
    }

    unsafe fn dispatch(
        &self,
        op: DrawOperation,
        frame: *mut render_scene::FSceneNode,
        packet: &mut Hp2ScenePipelineDrawPacket,
    ) -> RuntimeResult<()> {
        if frame.is_null() {
            return Err(RuntimeError::new("Canonical scene draw operation has no frame"));
        }
        let view = unsafe { (*frame).viewport.cast::<UViewport>() };
        let actor = if view.is_null() {
            ptr::null_mut()
        } else {
            unsafe { (*view).actor.cast::<AActor>() }
        };
        let level = unsafe { (*frame).level.cast::<ULevel>() };
        let model = if level.is_null() {
            ptr::null_mut()
        } else {
            unsafe { (*level).model.cast::<UModel>() }
        };
        let device = if view.is_null() {
            ptr::null_mut()
        } else {
            unsafe { (*view).ren_dev.cast::<URenderDevice>() }
        };
        match op {
            DrawOperation::Frame => {
                if model.is_null() || actor.is_null() || device.is_null() {
                    return Err(RuntimeError::new("Canonical draw frame is missing Model, Actor, or RenderDevice"));
                }
                let nodes = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).nodes)) };
                if nodes.array_num <= 0 {
                    return Err(RuntimeError::new("Canonical draw Model has no BSP nodes"));
                }
                let client = unsafe { (*view).outer.cast::<UClient>() };
                if client.is_null() {
                    return Err(RuntimeError::new("Canonical draw Viewport has no Client"));
                }
                let actor_ref = unsafe { &*actor };
                packet.p[0] = nodes.data;
                packet.p[4] = actor_ref.region.zone;
                packet.n[0] = unsafe { (*device).span_based as i32 };
                packet.n[1] = unsafe { (*actor.cast::<APlayerPawn>()).rend_map };
                packet.n[2] = unsafe { (*model).light_map.num };
                packet.n[3] = i32::from((*client).no_lighting());
                packet.n[4] = actor_ref.region.i_leaf;
                packet.n[5] = i32::from((*device).coronas());
                packet.n[6] = i32::from((*client).decals());
                if hp2_abi::engine::generated::NODECALS_ENABLED == 0 {
                    packet.n[9] = 1;
                }
            }
            DrawOperation::Clear => unsafe {
                crate::render_device_owner::clear_z(self.access, device)?;
            },
            DrawOperation::Actor => unsafe { self.actor_query(packet)?; },
            DrawOperation::DrawSprite => unsafe {
                super::actor::draw_actor_sprite(
                    self.access,
                    self.render,
                    frame.cast(),
                    packet.p[0].cast(),
                )?;
            },
            DrawOperation::Icon => unsafe {
                super::actor::draw_icon(self.access, self.render, frame.cast(), packet)?;
            },
            DrawOperation::Surface
            | DrawOperation::GetTexture
            | DrawOperation::Texture
            | DrawOperation::Lock
            | DrawOperation::Unlock
            | DrawOperation::Light
            | DrawOperation::Transform
            | DrawOperation::Flat
            | DrawOperation::HitPush
            | DrawOperation::HitPop
            | DrawOperation::Complex
            | DrawOperation::LightFinish
            | DrawOperation::Clip
            | DrawOperation::Triangles
            | DrawOperation::DecalTime
            | DrawOperation::DecalTimeEnd => unsafe {
                self.surface.call(self.render, frame.cast(), op, packet)?;
            },
            DrawOperation::CacheGet
            | DrawOperation::CacheCreate
            | DrawOperation::CacheUnlock
            | DrawOperation::Now
            | DrawOperation::Trace
            | DrawOperation::ActorIndex
            | DrawOperation::CoronaSources
            | DrawOperation::LevelActor
            | DrawOperation::Lumos
            | DrawOperation::Debug
            | DrawOperation::Log
            | DrawOperation::Flare => unsafe {
                self.corona.call(self.render, frame.cast(), op, packet)?;
            },
            DrawOperation::Sky => unsafe {
                let index = usize::try_from((*frame).zone_number)
                    .ok()
                    .filter(|&index| index < (*model).num_zones.max(0) as usize)
                    .ok_or_else(|| RuntimeError::new("Canonical draw ZoneNumber is invalid"))?;
                let zones = ptr::addr_of!((*model).zones).cast::<render_scene::FZoneProperties>();
                let zone = (*zones.add(index)).zone_actor.cast::<AActor>();
                let class = self.access.native_class("Engine.SkyZoneInfo")?;
                packet.n[0] = i32::from(!zone.is_null() && super::class_is_child_of((*zone.cast::<UObject>()).class.cast(), class));
            },
            DrawOperation::Finish => unsafe { self.finish_frame(frame, packet)?; },
            DrawOperation::Distance => unsafe { self.distance(frame, packet)?; },
            DrawOperation::Valid => unsafe {
                let target = packet.p[0].cast::<UObject>();
                if target.is_null() {
                    return Err(RuntimeError::new("Canonical draw validity check has a null Actor"));
                }
                let index = (*target).index;
                let valid = index >= 0
                    && index < self.access.object_count()
                    && self.access.object_at(index) == target;
                if !valid {
                    return Err(RuntimeError::new("Canonical draw Actor failed UObject::IsValid"));
                }
            },
            DrawOperation::GetDefaultTexture => unsafe {
                if actor.is_null() {
                    return Err(RuntimeError::new("Canonical default texture query has no Viewport Actor"));
                }
                let actor_level = (*actor).level.cast::<ALevelInfo>();
                if actor_level.is_null() {
                    return Err(RuntimeError::new("Canonical default texture query has no LevelInfo"));
                }
                packet.p[0] = (*actor_level).default_texture;
            },
            DrawOperation::WorldTextureLod => unsafe {
                let client = packet.p[0].cast::<UClient>();
                if client.is_null() {
                    return Err(RuntimeError::new("Canonical world texture LOD query has no Client"));
                }
                // LODSET_World is 1 in Engine/Inc/UnTex.h.
                packet.n[0] = (*client).texture_lod_set[1];
            },
        }
        Ok(())
    }

    unsafe fn actor_query(&self, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let actor = packet.p[0].cast::<AActor>();
        if actor.is_null() {
            return Err(RuntimeError::new("Canonical Actor draw query has a null Actor"));
        }
        let actor_ref = unsafe { &*actor };
        packet.p[0] = unsafe { (*actor.cast::<UObject>()).class.cast() };
        packet.p[1] = actor_ref.texture;
        packet.p[2] = actor_ref.skin;
        packet.n[0] = i32::from(actor_ref.style);
        packet.n[1] = i32::from(actor_ref.b_filter_by_volume());
        packet.n[3] = i32::from(actor_ref.b_corona());
        packet.n[4] = i32::from(actor_ref.b_delete_me());
        packet.n[5] = i32::from(actor_ref.light_hue);
        packet.n[6] = i32::from(actor_ref.light_saturation);
        packet.f[0] = actor_ref.location.x;
        packet.f[1] = actor_ref.location.y;
        packet.f[2] = actor_ref.location.z;
        packet.f[3] = actor_ref.opacity;
        packet.f[4] = actor_ref.draw_scale;
        Ok(())
    }

    unsafe fn finish_frame(
        &self,
        frame: *mut render_scene::FSceneNode,
        _packet: &mut Hp2ScenePipelineDrawPacket,
    ) -> RuntimeResult<()> {
        if hp2_abi::engine::generated::ENGINE_BUILD_STATS == 0 {
            return Ok(());
        }
        let engine = self
            .access
            .owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("Canonical draw has no Engine owner"))?;
        let graph = &engine.render_graph;
        let level = unsafe { (*frame).level.cast::<ULevel>() };
        if level.is_null() {
            return Err(RuntimeError::new("Canonical draw finish has no Level"));
        }
        let model = unsafe { (*level).model.cast::<UModel>() };
        if model.is_null() {
            return Err(RuntimeError::new("Canonical draw finish has no Model"));
        }
        let stats = graph.stats_ptr();
        if stats.is_null() {
            return Err(RuntimeError::new("Canonical draw statistics are unavailable"));
        }
        let dynamic = graph.dynamic_memory()?;
        let global_bytes = engine.memory.byte_count()?;
        let dynamic_bytes = dynamic.byte_count()?;
        unsafe {
            let gmem = ptr::addr_of_mut!((*stats).g_mem);
            gmem.write_unaligned(gmem.read_unaligned().wrapping_add(global_bytes));
            let gdyn = ptr::addr_of_mut!((*stats).g_dyn_mem);
            gdyn.write_unaligned(gdyn.read_unaligned().wrapping_add(dynamic_bytes));
            let nodes = ptr::addr_of_mut!((*stats).nodes_total);
            nodes.write_unaligned(nodes.read_unaligned().wrapping_add((*model).nodes.array_num));
        }
        Ok(())
    }

    unsafe fn distance(
        &self,
        frame: *mut render_scene::FSceneNode,
        packet: &mut Hp2ScenePipelineDrawPacket,
    ) -> RuntimeResult<()> {
        let transform = packet.p[0].cast::<FTransform>();
        if transform.is_null() {
            return Err(RuntimeError::new("Canonical draw distance query has no Transform"));
        }
        let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
        let actor = if viewport.is_null() { ptr::null_mut() } else { unsafe { (*viewport).actor.cast::<AActor>() } };
        if actor.is_null() {
            return Err(RuntimeError::new("Canonical draw distance query has no Viewport Actor"));
        }
        let point = unsafe { ptr::read_unaligned(ptr::addr_of!((*transform).base.point)) };
        let projected = hp2_core::math::vector_transform_point(point, unsafe { (*frame).uncoords });
        let delta = hp2_core::math::vector_sub(projected, unsafe { (*actor).location });
        packet.f[0] = hp2_core::math::sqrt_approx_inline(
            hp2_core::math::vector_size_sq(delta),
            hp2_core::math::sqrt_man_table(),
        );
        Ok(())
    }
}

impl DrawOps for CoreDrawOps<'_> {
    unsafe fn push(&self, bytes: usize) -> Result<*mut u8, ()> {
        let stack = self.stacks.stack_ptrs()[0];
        let size = match i32::try_from(bytes) {
            Ok(size) => size,
            Err(_) => return self.fail(RuntimeError::new("Canonical scene draw allocation exceeds the stack size limit")),
        };
        let result = unsafe {
            <super::CoreSceneStackOps<'_> as hp2_render::scene_pipeline::helper_frames::SceneStackOps>::push(
                &self.stacks,
                ptr::null_mut(),
                stack,
                size as usize,
                mem::align_of::<*mut c_void>() as i32,
                0,
            )
        };
        match result {
            Ok(pointer) => Ok(pointer),
            Err(_) => self.fail(self.stacks.take_error().unwrap_or_else(|| RuntimeError::new("Canonical scene draw allocation failed"))),
        }
    }

    unsafe fn call(
        &self,
        op: DrawOperation,
        frame: *mut render_scene::FSceneNode,
        packet: &mut Hp2ScenePipelineDrawPacket,
    ) -> Result<(), ()> {
        match unsafe { self.dispatch(op, frame, packet) } {
            Ok(()) => Ok(()),
            Err(error) => self.fail(error),
        }
    }
}


