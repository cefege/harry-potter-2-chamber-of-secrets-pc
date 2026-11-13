//! Canonical Core host for source `URender::OccludeFrame`/`OccludeBsp`.

use super::{CoreSceneStackOps, RenderGraphState};
use hp2_abi::engine::generated::{AActor, APlayerPawn, AZoneInfo, FSceneNode as GeneratedSceneNode, ULevel, UModel, UTexture, UViewport, URender, URenderDevice};
use hp2_abi::render_scene::{FActorLink, FSceneNode, FVolActorLink, TArrayRaw};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::scene_pipeline::{self, Hp2ScenePipelineArgs, Hp2ScenePipelineData, OcclusionOperation, OcclusionOps};
use std::{cell::{Cell, RefCell}, ffi::c_void, pin::Pin, ptr};

/// Executes canonical Core occlusion for the active renderer and scene frame.
///
/// # Safety
/// `render` and `frame` are live canonical records and remain valid through recursion.
pub(super) unsafe fn occlude_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut GeneratedSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical OccludeFrame requires a live renderer and frame"));
    }
    let graph = super::state(access)?;
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("Canonical OccludeFrame renderer is not published"));
    }
    let level = unsafe { (*frame).level.cast::<ULevel>() };
    if level.is_null() {
        return Err(RuntimeError::new("Canonical OccludeFrame has no level"));
    }
    let model = unsafe { (*level).model.cast::<UModel>() };
    if model.is_null() {
        return Err(RuntimeError::new("Canonical OccludeFrame level has no model"));
    }
    let stacks = CoreSceneStackOps::new(access)?;
    let mut state = CoreOcclusionState::new(access, &graph, &stacks, render, frame, model)?;
    let state_ptr = unsafe { Pin::as_mut(&mut state).get_unchecked_mut() as *mut CoreOcclusionState<'_> };
    let mut ops = CoreOcclusionOps { access, graph: &graph, stacks: &stacks, render, frame, model, state: state_ptr, error: RefCell::new(None), clocks: RefCell::new([0; 2]) };
    // SAFETY: state remains pinned and all graph/frame borrows span the synchronous policy.
    let data = unsafe { ptr::addr_of_mut!((*state_ptr).data) };
    let policy = unsafe { scene_pipeline::occlude_frame_with(data, &ops) };
    finish(policy, ops.error.borrow_mut().take(), stacks.take_error())
}

/// Executes canonical Core BSP occlusion for a child or standalone scene frame.
///
/// # Safety
/// `render` and `frame` are live canonical records and remain valid through the call.
pub(super) unsafe fn occlude_bsp(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut GeneratedSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical OccludeBsp requires a live renderer and frame"));
    }
    let graph = super::state(access)?;
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("Canonical OccludeBsp renderer is not published"));
    }
    let level = unsafe { (*frame).level.cast::<ULevel>() };
    if level.is_null() { return Err(RuntimeError::new("Canonical OccludeBsp has no level")); }
    let model = unsafe { (*level).model.cast::<UModel>() };
    if model.is_null() { return Err(RuntimeError::new("Canonical OccludeBsp level has no model")); }
    let stacks = CoreSceneStackOps::new(access)?;
    let mut state = CoreOcclusionState::new(access, &graph, &stacks, render, frame, model)?;
    let state_ptr = unsafe { Pin::as_mut(&mut state).get_unchecked_mut() as *mut CoreOcclusionState<'_> };
    let mut ops = CoreOcclusionOps { access, graph: &graph, stacks: &stacks, render, frame, model, state: state_ptr, error: RefCell::new(None), clocks: RefCell::new([0; 2]) };
    // SAFETY: state remains pinned and all graph/frame borrows span the synchronous policy.
    let data = unsafe { ptr::addr_of_mut!((*state_ptr).data) };
    let policy = unsafe { scene_pipeline::occlude_bsp_with(data, &ops) };
    finish(policy, ops.error.borrow_mut().take(), stacks.take_error())
}

fn finish(
    policy: Result<(), ()>,
    operation_error: Option<RuntimeError>,
    stack_error: Option<RuntimeError>,
) -> RuntimeResult<()> {
    if let Some(error) = operation_error.or(stack_error) { return Err(error); }
    policy.map_err(|_| RuntimeError::new("Canonical Core occlusion policy failed"))
}

struct CoreOcclusionState<'a> {
    data: Hp2ScenePipelineData,
    _graph: &'a RenderGraphState,
    _stacks: &'a CoreSceneStackOps<'a>,
    dynamics: TArrayRaw,
    surf_lights: *mut *mut FActorLink,
    leaf_lights: *mut *mut FVolActorLink,
    clip_scratch: [*mut hp2_abi::render_particle::FTransform; 144],
    raster: [std::mem::MaybeUninit<hp2_abi::render_span::FRasterSpan>; 2880],
    raster_bounds: Cell<[i32; 4]>,
    view_zone: Cell<i32>,
}

impl<'a> CoreOcclusionState<'a> {
    fn new(
        access: CoreAccess<'_>, graph: &'a RenderGraphState, stacks: &'a CoreSceneStackOps<'a>,
        render: *mut URender, frame: *mut GeneratedSceneNode, model: *mut UModel,
    ) -> RuntimeResult<Pin<Box<Self>>> {
        let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
        let actor = unsafe { (*viewport).actor.cast::<AActor>() };
        if actor.is_null() { return Err(RuntimeError::new("Canonical occlusion viewport has no actor")); }
        let player_class = access.native_class("Engine.PlayerPawn")?;
        let is_player = unsafe { hp2_core::object_model::is_a(actor.cast(), player_class.cast()) };
        if !is_player { return Err(RuntimeError::new("Canonical occlusion viewport actor is not a PlayerPawn")); }
        let player = actor.cast::<APlayerPawn>();
        let level = unsafe { (*frame).level.cast::<ULevel>() };
        let info = unsafe { crate::package_map_level_owner::level_info(level)? };
        let device = unsafe { (*viewport).ren_dev.cast::<URenderDevice>() };
        if device.is_null() { return Err(RuntimeError::new("Canonical occlusion viewport has no render device")); }
        // UnRender.cpp gates volumetric fog on Viewport->Actor->Region.Zone->bFogZone.
        let zone = unsafe { (*actor).region.zone.cast::<AZoneInfo>() };
        let stacks_ptrs = stacks.stack_ptrs();
        let render_state = graph.render_state_ptr();
        let raster_count = unsafe { (*frame.cast::<FSceneNode>()).y };
        if raster_count < 0 || raster_count as usize > 2880 {
            return Err(RuntimeError::new("Canonical scene height exceeds native raster capacity"));
        }
        let mut result = Box::pin(Self {
            // SAFETY: callback-free data is published field-by-field before policy entry.
            data: unsafe { std::mem::zeroed() },
            _graph: graph, _stacks: stacks,
            dynamics: TArrayRaw { data: ptr::null_mut(), array_num: 0, array_max: 0 },
            surf_lights: ptr::null_mut(), leaf_lights: ptr::null_mut(),
            clip_scratch: [ptr::null_mut(); 144],
            raster: unsafe { std::mem::MaybeUninit::<[std::mem::MaybeUninit<hp2_abi::render_span::FRasterSpan>; 2880]>::uninit().assume_init() },
            raster_bounds: Cell::new([0; 4]),
            view_zone: Cell::new(unsafe { (*frame.cast::<FSceneNode>()).zone_number }),
        });
        let this = unsafe { Pin::as_mut(&mut result).get_unchecked_mut() };
        graph.with_dynamics_cache(|items| {
            this.dynamics.data = items.as_mut_ptr().cast();
            this.dynamics.array_num = items.len() as i32;
            this.dynamics.array_max = items.capacity() as i32;
        });
        graph.with_surf_lights(|items| this.surf_lights = items.as_mut_ptr());
        graph.with_leaf_lights(|items| this.leaf_lights = items.as_mut_ptr());
        let volumetric = unsafe { (*device).volumetric_lighting() };
        let fog_maps = unsafe { (*device).supports_fog_maps != 0 };
        let fog_zone = if volumetric && fog_maps {
            if zone.is_null() { return Err(RuntimeError::new("Canonical occlusion actor has no fog zone")); }
            unsafe { (*zone).b_fog_zone() }
        } else { false };
        let capabilities = (u32::from(access.process_mode().is_editor))
            | (u32::from(unsafe { (*player).show_flags as u32 & hp2_abi::engine::generated::SHOW_PLAYER_CTRL != 0 }) << 1)
            | (u32::from(volumetric) << 2)
            | (u32::from(fog_maps) << 3)
            | (u32::from(fog_zone) << 4)
            | (u32::from(unsafe { (*device).span_based != 0 }) << 5)
            | (u32::from(unsafe { (*device).shiny_surfaces() }) << 6)
            | (u32::from(unsafe { !(*level).brush_tracker.is_null() }) << 7)
            | (u32::from(unsafe { (*player).rend_map == hp2_abi::engine::generated::ENGINE_REN_ZONES }) << 8)
            | (u32::from(is_player) << 9)
            | (u32::from(unsafe { (*player).b_lumos_on() }) << 10)
            | (hp2_abi::engine::generated::ENGINE_LUMOS_ENABLED << 12);
        let location = unsafe { (*actor).location };
        this.data = unsafe { Hp2ScenePipelineData {
            mem_stack: stacks_ptrs[0], dyn_mem_stack: stacks_ptrs[1], frame: frame.cast(),
            nodes: ptr::addr_of_mut!((*model).nodes).cast(), surfs: ptr::addr_of_mut!((*model).surfs).cast(),
            verts: ptr::addr_of_mut!((*model).verts).cast(), points: ptr::addr_of_mut!((*model).points).cast(),
            leaves: ptr::addr_of_mut!((*model).leaves).cast(), zones: ptr::addr_of_mut!((*model).zones).cast(),
            dynamics: ptr::addr_of_mut!(this.dynamics).cast(), first_volumetric: ptr::addr_of_mut!((*render).first_volumetric).cast(),
            surf_lights: ptr::addr_of_mut!(this.surf_lights).cast(), leaf_lights: ptr::addr_of_mut!(this.leaf_lights).cast(),
            dyn_light_surfs: graph.dyn_light_surfs_ptr().cast(), dyn_light_leaves: graph.dyn_light_leaves_ptr().cast(),
            num_dyn_light_surfs: graph.dynamic_light_surfs.as_ptr(), num_dyn_light_leaves: graph.dynamic_light_leaves.as_ptr(),
            max_surf_lights: graph.max_surf_lights.as_ptr(), max_leaf_lights: graph.max_leaf_lights.as_ptr(),
            stamp: ptr::addr_of_mut!((*render_state).stamp), nodes_draw: ptr::addr_of_mut!((*render).nodes_draw),
            polys_draw: ptr::addr_of_mut!((*render).polys_draw), actor: actor.cast(), view_target: unsafe { (*player).view_target },
            capabilities, root_outside: unsafe { (*model).root_outside }, toggle: unsafe { (*render).toggle as i32 },
            frame_stamp: unsafe { (*render_state).gframe_stamp }, behind_view: unsafe { (*player).base.b_behind_view() as i32 },
            extra_poly_flags: unsafe { (*viewport).extra_poly_flags }, time_seconds: unsafe { (*info).time_seconds },
            lumos_radius: unsafe { (*player).f_lumos_radius }, player_location: [location.x, location.y, location.z],
        } };
        Ok(result)
    }
}

struct CoreOcclusionOps<'a> {
    access: CoreAccess<'a>, graph: &'a RenderGraphState, stacks: &'a CoreSceneStackOps<'a>,
    render: *mut URender, frame: *mut GeneratedSceneNode, model: *mut UModel,
    state: *mut CoreOcclusionState<'a>,
    error: RefCell<Option<RuntimeError>>,
    clocks: RefCell<[u32; 2]>,
}

impl CoreOcclusionOps<'_> {
    fn record(&self, error: RuntimeError) { let mut slot = self.error.borrow_mut(); if slot.is_none() { *slot = Some(error); } }

    fn remember<T>(&self, result: RuntimeResult<T>) -> Result<T, ()> {
        result.map_err(|error| { self.record(error); () })
    }

    unsafe fn zone(&self, index: i32) -> *mut hp2_abi::render_scene::FZoneProperties {
        unsafe { (*self.model).zones.as_mut_ptr().cast::<hp2_abi::render_scene::FZoneProperties>().add(index as usize) }
    }

    unsafe fn surface(&self, index: i32) -> *mut hp2_abi::render_scene::FBspSurf {
        unsafe { (*self.model).surfs.data.cast::<hp2_abi::render_scene::FBspSurf>().add(index as usize) }
    }

    unsafe fn node(&self, index: i32) -> *mut hp2_abi::render_scene::FBspNode {
        unsafe { (*self.model).nodes.data.cast::<hp2_abi::render_scene::FBspNode>().add(index as usize) }
    }

    unsafe fn perform(&self, operation: OcclusionOperation, data: *mut Hp2ScenePipelineData, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        match operation {
            OcclusionOperation::Initialize => self.initialize(data),
            // `FBspDrawList` has no user constructor; source placement-new only
            // begins its lifetime. Its fields are initialized at their use sites.
            OcclusionOperation::InitializeDrawList => Ok(()),
            OcclusionOperation::ViewZone => {
                packet.integers[0] = crate::actor::player_pawn_view_zone(packet.integers[0]);
                unsafe { (*self.state).view_zone.set(packet.integers[0]); }
                Ok(())
            }
            OcclusionOperation::WarpInfo => self.warp_info(packet),
            OcclusionOperation::CreateChild => self.create_child(packet),
            OcclusionOperation::InitializeSpan => {
                let span = packet.pointers[0].cast::<hp2_abi::render_span::FSpanBuffer>();
                unsafe { hp2_render::span::alloc_index(&CoreSpanAlloc(self), &mut *span, 0, 0, (*data).dyn_mem_stack) }
                    .map_err(|_| { self.record(RuntimeError::new("Core span index allocation failed")); () })
            }
            OcclusionOperation::Clip => self.clip(packet),
            OcclusionOperation::Raster => self.raster(packet),
            OcclusionOperation::AllocateSpan => self.alloc_span(packet),
            OcclusionOperation::CopySpan => self.copy_span(packet),
            OcclusionOperation::ReleaseSpan => self.release_span(packet),
            OcclusionOperation::MergeSpan => self.merge_span(packet),
            OcclusionOperation::IsDynamic => {
                let index = packet.integers[0];
                self.remember(unsafe { crate::dynbsp_owner::surf_is_dynamic((*self.frame).level.cast(), index) })
                    .map(|value| packet.integers[0] = i32::from(value))
            }
            OcclusionOperation::BoundVisible => self.bound_visible(packet),
            OcclusionOperation::Filter => self.dynamic_filter(packet),
            OcclusionOperation::PreRender => self.dynamic_pre_render(packet),
            OcclusionOperation::LeafLighting => self.leaf_lighting(packet),
            OcclusionOperation::SurfaceVisible => self.surface_visible(packet),
            OcclusionOperation::ZoneActor => {
                packet.pointers[0] = self.remember(unsafe {
                    crate::level_owner::zone_actor((*self.frame).level.cast(), packet.integers[0])
                })?.cast();
                Ok(())
            }
            OcclusionOperation::TextureInfo => self.texture_info(packet),
            OcclusionOperation::SkyInfo => self.sky_info(packet),
            OcclusionOperation::Generate => self.generate(packet),
            OcclusionOperation::Link => self.link(packet),
            OcclusionOperation::Radius => self.radius(packet),
            OcclusionOperation::ObjectIndex => {
                let object = packet.pointers[0].cast::<hp2_abi::engine::generated::UObject>();
                packet.integers[0] = if object.is_null() { -1 } else { unsafe { (*object).index } };
                Ok(())
            }
            OcclusionOperation::Statistics => self.statistics(packet),
            OcclusionOperation::SetupDynamics => self.setup_dynamics(packet),
            OcclusionOperation::OccludeBsp => self.remember(unsafe { occlude_bsp(self.access, self.render, self.frame) }),
            OcclusionOperation::ActorLeaf => self.actor_leaf(packet),
            OcclusionOperation::OccludeFrame => self.remember(unsafe { occlude_frame(self.access, self.render, packet.pointers[0].cast()) }),
            OcclusionOperation::AllocateLights => self.allocate_lights(packet),
            OcclusionOperation::AllocateDynamics => self.allocate_dynamics(),
            OcclusionOperation::SizeApprox => self.size_approx(packet),
            OcclusionOperation::Snapshot => self.snapshot(packet),
        }
    }
}

impl OcclusionOps for CoreOcclusionOps<'_> {
    unsafe fn call(&self, operation: OcclusionOperation, data: *mut Hp2ScenePipelineData, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        unsafe { self.perform(operation, data, packet) }
    }

    unsafe fn push(&self, stack: *mut c_void, bytes: usize, alignment: i32, kind: u32) -> Result<*mut u8, ()> {
        let bytes = unsafe { self.checked_size(bytes, kind)? };
        unsafe { hp2_render::scene_pipeline::helper_frames::SceneStackOps::push(self.stacks, ptr::null_mut(), stack, bytes, alignment, kind) }
            .map_err(|_| ())
    }

    unsafe fn checked_size(&self, bytes: usize, kind: u32) -> Result<usize, ()> {
        match i32::try_from(bytes) {
            Ok(size) if size >= 0 => Ok(size as usize),
            _ => {
                self.record(RuntimeError::new("Occlusion allocation exceeds Core memory size")
                    .with_context(&format!("allocation kind {kind}")));
                Err(())
            }
        }
    }
}

struct CoreSpanAlloc<'a, 'b>(&'a CoreOcclusionOps<'b>);
impl hp2_render::span::SpanAlloc for CoreSpanAlloc<'_, '_> {
    unsafe fn push(&self, mem: *mut c_void, size: usize, align: usize) -> Result<*mut u8, i32> {
        unsafe { self.0.push(mem, size, align as i32, 1) }.map_err(|_| -1)
    }
    unsafe fn mark_save(&self, mem: *mut c_void, mark: *mut hp2_abi::render_span::FMemMark) {
        unsafe { hp2_render::scene_pipeline::helper_frames::SceneStackOps::mark_save(self.0.stacks, mem, mark) }
    }
    unsafe fn mark_pop(&self, mark: *mut hp2_abi::render_span::FMemMark) {
        unsafe { hp2_render::scene_pipeline::helper_frames::SceneStackOps::mark_pop(self.0.stacks, mark) }
    }
    fn warn(&self, text: &str) {
        let mut units = [0u32; 256];
        let mut len = 0;
        for character in text.chars().take(units.len()) {
            units[len] = u32::from(character);
            len += 1;
        }
        if let Err(error) = self.0.access.emit_log(self.0.access.intern_str("Warning", hp2_core::name_owner::FindName::Add).unwrap_or(0), &units[..len]) { self.0.record(error); }
    }
}

impl CoreOcclusionOps<'_> {
    fn warp_info(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let class = self.remember(self.access.native_class("Engine.WarpZoneInfo"))?;
        let actor = packet.pointers[0].cast::<AActor>();
        let valid = !actor.is_null() && unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) };
        packet.integers[0] = i32::from(valid);
        packet.pointers[1] = ptr::null_mut();
        packet.pointers[2] = ptr::null_mut();
        if valid {
            let warp = actor.cast::<hp2_abi::engine::generated::AWarpZoneInfo>();
            packet.pointers[1] = unsafe { (*warp).other_side_actor };
            packet.pointers[2] = unsafe { (*warp).other_side_level };
        }
        Ok(())
    }

    fn create_child(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        let frame = unsafe { self.frame.cast::<FSceneNode>().as_ref() }
            .ok_or_else(|| { self.record(RuntimeError::new("Canonical child frame has no parent")); })?;
        let bounds = packet.pointers[2].cast::<hp2_abi::render_scene::FScreenBounds>();
        let span = packet.pointers[0].cast::<hp2_abi::render_span::FSpanBuffer>();
        let (level, surf, zone, mirror, near, coords) = match packet.integers[0] {
            0 => {
                let warp = packet.pointers[1].cast::<hp2_abi::engine::generated::AWarpZoneInfo>();
                if warp.is_null() { return Err(()); }
                let other = unsafe { (*warp).other_side_actor.cast::<hp2_abi::engine::generated::AWarpZoneInfo>() };
                if other.is_null() { return Err(()); }
                let coords = hp2_core::math::coords_mul(
                    hp2_core::math::coords_mul(frame.coords, unsafe { (*warp).warp_coords }),
                    hp2_core::math::coords_transpose(unsafe { (*other).warp_coords }),
                );
                (frame.level.cast(), -1, unsafe { (*other).i_warp_zone }, frame.mirror, frame.near_clip, coords)
            }
            1 => {
                let sky = packet.pointers[1].cast::<AZoneInfo>();
                if sky.is_null() { return Err(()); }
                let mut coords = hp2_core::math::coords_mul_point(frame.coords, frame.coords.origin);
                coords = hp2_core::math::coords_div_rotator(&coords, { let r = unsafe { (*sky).base.base.rotation }; hp2_core::math::FRotator { pitch: r.pitch, yaw: r.yaw, roll: r.roll } }, hp2_core::math::trig_table());
                coords = hp2_core::math::coords_div_point(coords, unsafe { (*sky).base.base.location });
                (frame.level.cast(), 0, unsafe { (*sky).base.base.region.zone_number as i32 }, frame.mirror, frame.near_clip, coords)
            }
            2 => {
                let plane = unsafe { *self.node(packet.integers[1]) }.plane;
                (frame.level.cast(), 0, packet.integers[2], -frame.mirror,
                    hp2_core::math::plane_flip(hp2_core::math::plane_transform(plane, frame.coords)),
                    hp2_core::math::coords_mirror_plane(frame.coords, plane))
            }
            3 => {
                let warp = packet.pointers[1].cast::<hp2_abi::engine::generated::AWarpZoneInfo>();
                if warp.is_null() { return Err(()); }
                let other = unsafe { (*warp).other_side_actor.cast::<hp2_abi::engine::generated::AWarpZoneInfo>() };
                if other.is_null() { return Err(()); }
                let node = unsafe { *self.node(packet.integers[1]) };
                let plane = hp2_core::math::plane_scale(node.plane, packet.values[0]);
                let coords = hp2_core::math::coords_mul(
                    hp2_core::math::coords_mul(frame.coords, unsafe { (*warp).warp_coords }),
                    hp2_core::math::coords_transpose(unsafe { (*other).warp_coords }),
                );
                (unsafe { (*warp).other_side_level.cast() }, node.i_surf, unsafe { (*other).i_warp_zone },
                    frame.mirror, hp2_core::math::plane_transform(plane, frame.coords), coords)
            }
            _ => return Err(()),
        };
        let result = unsafe {
            super::create_child_frame(
                self.access, self.render, self.frame.cast(), span, level, surf, zone, mirror, near, coords,
                // Native warp children retain the full frustum; spans clip their portal.
                if matches!(packet.integers[0], 0 | 3) { ptr::null() } else { bounds },
            )
        };
        self.remember(result).map(|_| ())
    }
    fn leaf_lighting(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        self.remember(unsafe {
            super::lighting::leaf_volumetric_lighting(
                self.access,
                self.frame.cast(),
                self.model,
                packet.integers[0],
            )
        })
    }

    fn surface_visible(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let node = unsafe { *self.node(packet.integers[0]) };
        let _ = (node.i_zone[packet.integers[1] as usize], unsafe { *self.surface(node.i_surf) });
        packet.integers[0] = i32::from(crate::actor::player_pawn_is_surf_visible());
        Ok(())
    }

    fn texture_info(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let texture = packet.pointers[0].cast::<UTexture>();
        if texture.is_null() {
            packet.integers[0] = 0;
            packet.integers[1] = -1;
            packet.pointers[1] = ptr::null_mut();
        } else {
            packet.integers[0] = unsafe { (*texture).poly_flags as i32 };
            packet.integers[1] = unsafe { (*texture).base.base.index };
            packet.pointers[1] = unsafe { (*texture).base.palette };
        }
        Ok(())
    }

    fn sky_info(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        if packet.integers[1] != 0 {
            let zone_actor = self.remember(unsafe {
                crate::level_owner::zone_actor((*self.frame).level.cast(), packet.integers[0])
            })?;
            packet.pointers[0] = unsafe { (*zone_actor).sky_zone };
            return Ok(());
        }
        let sky_class = self.remember(self.access.native_class("Engine.SkyZoneInfo"))?;
        packet.pointers[0] = ptr::null_mut();
        for index in 0..self.access.object_count() {
            let object = self.access.object_at(index);
            if !object.is_null() && unsafe { hp2_core::object_model::is_a(object.cast(), sky_class.cast()) } {
                packet.pointers[0] = object.cast();
                break;
            }
        }
        Ok(())
    }

    fn generate(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        self.remember(unsafe { self.access.execute_named_event(packet.pointers[0].cast(), "Generate") }).map(|_| ())
    }

    fn link(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let link = packet.pointers[2].cast::<FActorLink>();
        if link.is_null() { return Err(()); }
        unsafe { link.write(FActorLink { actor: packet.pointers[0], next: packet.pointers[1].cast() }); }
        packet.pointers[0] = link.cast();
        Ok(())
    }

    fn radius(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        packet.values[0] = crate::actor::world_volumetric_radius(unsafe { &*packet.pointers[0].cast::<AActor>() });
        Ok(())
    }

    fn size_approx(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        packet.values[0] = hp2_render::light::GMath::canonical().size_approx(hp2_abi::FVector {
            x: packet.values[0], y: packet.values[1], z: packet.values[2],
        });
        Ok(())
    }

    fn actor_leaf(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let actor = packet.pointers[0].cast::<AActor>();
        packet.integers[0] = if actor.is_null() { -1 } else { unsafe { (*actor).region.i_leaf } };
        Ok(())
    }

    // Canonical callback-free fields were captured before policy entry; unlike
    // the Native adapter, there is no state record to copy into this snapshot.
    fn snapshot(&self, _packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        Ok(())
    }
}

impl CoreOcclusionOps<'_> {
    fn statistics(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        let stats = unsafe { &mut *self.graph.stats_ptr() };
        let now = hp2_core::unix_clock::cycles();
        let mut clocks = self.clocks.borrow_mut();
        match packet.integers[0] {
            0 => clocks[0] = now,
            1 => stats.occlusion_time = stats.occlusion_time.wrapping_add(now.wrapping_sub(clocks[0]) as i32),
            2 => stats.mask_reject_zones = stats.mask_reject_zones.wrapping_add(1),
            3 => stats.box_span_occluded = stats.box_span_occluded.wrapping_add(1),
            4 => clocks[1] = now,
            5 => stats.dyn_filter_time = stats.dyn_filter_time.wrapping_add(now.wrapping_sub(clocks[1]) as i32),
            6 => {
                stats.num_zones = stats.num_zones.wrapping_add(unsafe { (*self.model).num_zones });
                stats.cur_zone = packet.integers[1];
            }
            7 => stats.visible_zones = stats.visible_zones.wrapping_add(1),
            _ => {}
        }
        Ok(())
    }

    fn allocate_dynamics(&self) -> Result<(), ()> {
        let node_count = unsafe { (*self.model).nodes.array_num };
        let point_count = unsafe { (*self.model).points.array_num };
        if node_count < 0 || point_count < 0 { self.record(RuntimeError::new("Negative canonical BSP cache count")); return Err(()); }
        let render_state = unsafe { &mut *self.graph.render_state_ptr() };
        let stamp = render_state.stamp;
        let point_count = point_count as usize;
        if point_count > render_state.point_cache.len() {
            let cache = &mut render_state.point_cache;
            if cache.try_reserve(point_count - cache.len()).is_err() {
                self.record(RuntimeError::new("Canonical BSP point cache allocation failed"));
                return Err(());
            }
            cache.resize(point_count, hp2_abi::render_scene::FStampedPoint { point: ptr::null_mut(), stamp });
            for item in cache.iter_mut() { item.stamp = stamp; }
        }
        let dynamics_allocated = self.graph.with_dynamics_cache(|items| {
            let count = node_count as usize;
            if count <= items.len() { return true; }
            if items.try_reserve(count - items.len()).is_err() { return false; }
            items.resize(count, hp2_abi::render_scene::FDynamicsCache { dynamics: [ptr::null_mut(); 2] });
            true
        });
        if !dynamics_allocated { self.record(RuntimeError::new("Canonical dynamics cache allocation failed")); return Err(()); }
        self.graph.with_dynamics_cache(|items| unsafe {
            (*self.state).dynamics.data = items.as_mut_ptr().cast();
            (*self.state).dynamics.array_num = items.len() as i32;
            (*self.state).dynamics.array_max = items.capacity() as i32;
        });
        Ok(())
    }

    fn allocate_lights(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        if packet.integers[0] != 0 {
            let count = self.graph.max_leaf_lights.get();
            if count < 0 { self.record(RuntimeError::new("Negative leaf-light capacity")); return Err(()); }
            let allocated = self.graph.with_leaf_lights(|items| {
                let count = count as usize;
                if count > items.len() && items.try_reserve(count - items.len()).is_err() { return false; }
                items.resize(count, ptr::null_mut());
                items.fill(ptr::null_mut());
                unsafe { (*self.state).leaf_lights = items.as_mut_ptr(); }
                true
            });
            if !allocated { self.record(RuntimeError::new("Canonical leaf-light array allocation failed")); return Err(()); }
        } else {
            let count = self.graph.max_surf_lights.get();
            if count < 0 { self.record(RuntimeError::new("Negative surface-light capacity")); return Err(()); }
            let allocated = self.graph.with_surf_lights(|items| {
                let count = count as usize;
                if count > items.len() && items.try_reserve(count - items.len()).is_err() { return false; }
                items.resize(count, ptr::null_mut());
                items.fill(ptr::null_mut());
                unsafe { (*self.state).surf_lights = items.as_mut_ptr(); }
                true
            });
            if !allocated { self.record(RuntimeError::new("Canonical surface-light array allocation failed")); return Err(()); }
        }
        Ok(())
    }

    fn setup_dynamics(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        self.remember(unsafe {
            super::actor::setup_dynamics(
                self.access, self.render, self.frame.cast(), packet.pointers[0].cast(),
            )
        })
    }

    // Core traversal consumes per-call data directly instead of publishing
    // Native file-scope GFrame/GNodes/GSurfs globals.
    fn initialize(&self, _data: *mut Hp2ScenePipelineData) -> Result<(), ()> {
        Ok(())
    }
    fn dynamic_filter(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        self.remember(unsafe {
            super::actor::filter(
                self.access,
                self.render,
                self.frame.cast(),
                packet.pointers[0].cast(),
                packet.integers[0],
                packet.integers[1],
            )
        })
    }

    fn dynamic_pre_render(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        let state = self.state;
        self.remember(unsafe {
            super::actor::pre_render(
                self.access,
                self.render,
                self.frame.cast(),
                packet.pointers[0].cast(),
                packet.pointers[1].cast(),
                unsafe { ptr::addr_of_mut!((*self.render).first_volumetric) }.cast::<*mut FVolActorLink>(),
                (*state).leaf_lights.cast(),
            )
        })
    }

    fn clip(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let node = unsafe { *self.node(packet.integers[0]) };
        let frame = self.frame.cast::<FSceneNode>();
        let render_state = unsafe { &mut *self.graph.render_state_ptr() };
        let vertices = unsafe { (*self.model).verts.data.cast::<hp2_abi::render_scene::FVert>().add(node.i_vert_pool as usize) };
        let points = unsafe { (*self.model).points.data.cast::<hp2_abi::FVector>() };
        let input = hp2_render::scene_pipeline::helpers_core::Hp2ScenePipelineHelperClipBsp {
            frame, vertices, count: node.num_vertices as i32, points,
            cache: render_state.point_cache.as_mut_ptr(), stamp: render_state.stamp,
            scratch: unsafe { (*self.state).clip_scratch.as_mut_ptr() },
            capacity: hp2_render::state::MAX_FINAL_VERTICES as i32,
            num_transform: unsafe { ptr::addr_of_mut!((*self.graph.stats_ptr()).num_transform) },
            num_points: unsafe { ptr::addr_of_mut!((*self.graph.stats_ptr()).num_points) },
            num_clip: unsafe { ptr::addr_of_mut!((*self.graph.stats_ptr()).num_clip) },
        };
        let stats = unsafe { self.graph.stats_ptr() };
        let began = hp2_core::unix_clock::cycles();
        unsafe { (*stats).nodes_done = (*stats).nodes_done.wrapping_add(1); }
        let mut result = ptr::null_mut();
        let mut count = 0;
        if unsafe { hp2_render::scene_pipeline::helpers_core::clip_bsp_with(self, &input, &mut result, &mut count) }.is_err() {
            if self.error.borrow().is_none() { self.record(RuntimeError::new("Core BSP clipping allocation failed")); }
            return Err(());
        }
        unsafe { (*stats).clip_time = (*stats).clip_time.wrapping_add(hp2_core::unix_clock::cycles().wrapping_sub(began) as i32); }
        packet.pointers[0] = result.cast();
        packet.integers[0] = count;
        Ok(())
    }
}

impl hp2_render::scene_pipeline::helpers_core::CoreClipBspOps for CoreOcclusionOps<'_> {
    unsafe fn push(&self, bytes: usize, align: i32, kind: u32) -> Result<*mut u8, ()> {
        let vector_stack = self.stacks.stack_ptrs()[3];
        unsafe { <Self as OcclusionOps>::push(self, vector_stack, bytes, align, kind) }
    }
}

impl CoreOcclusionOps<'_> {
    fn raster(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let mut bounds = hp2_render::scene_pipeline::helpers_core::Hp2ScenePipelineHelperRaster {
            start_y: 0, end_y: 0, start_x: 0, end_x: 0,
        };
        let stats = unsafe { self.graph.stats_ptr() };
        let frame = unsafe { &*self.frame.cast::<FSceneNode>() };
        let began = hp2_core::unix_clock::cycles();
        let success = unsafe {
            hp2_render::scene_pipeline::helpers_core::setup_raster(
                packet.pointers[0].cast(),
                packet.integers[0],
                packet.pointers[1].cast(),
                frame.y,
                (*self.state).raster.as_mut_ptr().cast(),
                &mut bounds,
                ptr::addr_of_mut!((*stats).num_raster_box_reject),
                ptr::addr_of_mut!((*stats).num_raster_polys),
            )
        };
        unsafe { (*stats).raster_time = (*stats).raster_time.wrapping_add(hp2_core::unix_clock::cycles().wrapping_sub(began) as i32); }
        unsafe { (*self.state).raster_bounds.set([bounds.start_y, bounds.end_y, bounds.start_x, bounds.end_x]); }
        packet.integers[0] = success;
        packet.integers[1] = bounds.start_y;
        packet.integers[2] = bounds.end_y;
        packet.integers[3] = bounds.start_x;
        packet.integers[4] = bounds.end_x;
        Ok(())
    }

    fn alloc_span(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        let [start_y, end_y, _, _] = unsafe { (*self.state).raster_bounds.get() };
        let span = packet.pointers[0].cast::<hp2_abi::render_span::FSpanBuffer>();
        let stack = if packet.integers[0] != 0 { unsafe { (*self.state).data.mem_stack } } else { unsafe { (*self.state).data.dyn_mem_stack } };
        unsafe { hp2_render::span::alloc_index(&CoreSpanAlloc(self), &mut *span, start_y, end_y, stack) }
            .map_err(|_| { self.record(RuntimeError::new("Core raster span allocation failed")); () })
    }

    fn copy_span(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let [start_y, end_y, _, _] = unsafe { (*self.state).raster_bounds.get() };
        let dst = packet.pointers[0].cast::<hp2_abi::render_span::FSpanBuffer>();
        let screen = packet.pointers[1].cast::<hp2_abi::render_span::FSpanBuffer>();
        let raster = unsafe { (*self.state).raster.as_ptr().cast::<hp2_abi::render_span::FRasterSpan>().add(start_y as usize) };
        let began = hp2_core::unix_clock::cycles();
        let result = unsafe {
            if packet.integers[0] != 0 {
                hp2_render::span::copy_from_raster_update(&CoreSpanAlloc(self), &mut *dst, &mut *screen, start_y, end_y, raster)
            } else {
                hp2_render::span::copy_from_raster(&CoreSpanAlloc(self), &mut *dst, &*screen, start_y, end_y, raster)
            }
        };
        match result {
            Ok(copied) => {
                let stats = unsafe { self.graph.stats_ptr() };
                unsafe { (*stats).span_time = (*stats).span_time.wrapping_add(hp2_core::unix_clock::cycles().wrapping_sub(began) as i32); }
                packet.integers[0] = copied;
                Ok(())
            }
            Err(_) => { self.record(RuntimeError::new("Core span raster copy rejected its source range")); Err(()) }
        }
    }

    fn release_span(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        unsafe { hp2_render::span::release(&CoreSpanAlloc(self), &mut *packet.pointers[0].cast::<hp2_abi::render_span::FSpanBuffer>()); }
        Ok(())
    }

    fn merge_span(&self, packet: &Hp2ScenePipelineArgs) -> Result<(), ()> {
        let dst = packet.pointers[0].cast::<hp2_abi::render_span::FSpanBuffer>();
        let src = packet.pointers[1].cast::<hp2_abi::render_span::FSpanBuffer>();
        let began = hp2_core::unix_clock::cycles();
        let result = if packet.integers[0] == 0 {
            unsafe { hp2_render::span::merge_with(&CoreSpanAlloc(self), &mut *dst, &*src) }
                .map_err(|_| { self.record(RuntimeError::new("Core span merge allocation failed")); () })
        } else {
            self.merge_span_copy(dst, src)
        };
        if result.is_ok() {
            let stats = unsafe { self.graph.stats_ptr() };
            unsafe { (*stats).span_time = (*stats).span_time.wrapping_add(hp2_core::unix_clock::cycles().wrapping_sub(began) as i32); }
        }
        result
    }

    fn merge_span_copy(&self, dst: *mut hp2_abi::render_span::FSpanBuffer, src: *mut hp2_abi::render_span::FSpanBuffer) -> Result<(), ()> {
        let stack = unsafe { (*self.state).data.dyn_mem_stack };
        let mut copy: hp2_abi::render_span::FSpanBuffer = unsafe { std::mem::zeroed() };
        let alloc = CoreSpanAlloc(self);
        if unsafe { core_clone_span(&alloc, &mut copy, &*src, stack) }.is_err() {
            if !copy.index.is_null() {
                unsafe { hp2_render::span::release(&alloc, &mut copy); }
            }
            self.record(RuntimeError::new("Core dynamic span copy allocation failed"));
            return Err(());
        }
        let result = unsafe { hp2_render::span::merge_with(&alloc, &mut *dst, &copy) }
            .map_err(|_| { self.record(RuntimeError::new("Core dynamic span merge allocation failed")); () });
        unsafe { hp2_render::span::release(&alloc, &mut copy); }
        result
    }
}

unsafe fn core_clone_span<A: hp2_render::span::SpanAlloc>(
    alloc: &A,
    dst: *mut hp2_abi::render_span::FSpanBuffer,
    src: &hp2_abi::render_span::FSpanBuffer,
    stack: *mut c_void,
) -> Result<(), i32> {
    use hp2_abi::render_span::FSpan;
    unsafe {
        (*dst).start_y = src.start_y;
        (*dst).end_y = src.end_y;
        (*dst).valid_lines = src.valid_lines;
        let count = src.end_y.wrapping_sub(src.start_y) as usize;
        let index = alloc.push(stack, count.wrapping_mul(std::mem::size_of::<*mut FSpan>()), hp2_render::span::DEFAULT_ALIGNMENT)?;
        (*dst).index = index.cast();
        ptr::write_bytes((*dst).index, 0, count);
        (*dst).mem = stack;
        alloc.mark_save(stack, ptr::addr_of_mut!((*dst).mark));
        for i in 0..count {
            let mut previous = (*dst).index.add(i);
            let mut other = *src.index.add(i);
            while !other.is_null() {
                let node = alloc.push(stack, std::mem::size_of::<FSpan>(), 4)?.cast::<FSpan>();
                node.write(FSpan { start: (*other).start, end: (*other).end, next: ptr::null_mut() });
                previous.write(node);
                previous = ptr::addr_of_mut!((*node).next);
                other = (*other).next;
            }
            previous.write(ptr::null_mut());
        }
        Ok(())
    }
}

impl CoreOcclusionOps<'_> {
    fn bound_visible(&self, packet: &mut Hp2ScenePipelineArgs) -> Result<(), ()> {
        let frame = self.frame.cast::<FSceneNode>();
        let box_record = unsafe { (*self.model).bounds.data.cast::<hp2_abi::engine::generated::FBox>().add(packet.integers[0] as usize) };
        let bounds_native = unsafe { *box_record };
        let bounds_core = hp2_core::math::FBox {
            min: bounds_native.min, max: bounds_native.max, is_valid: bounds_native.is_valid,
        };
        let coords = hp2_core::math::box_get_coords(&bounds_core);
        let span = packet.pointers[0].cast::<hp2_abi::render_span::FSpanBuffer>();
        let bounds = packet.pointers[1].cast::<hp2_abi::render_scene::FScreenBounds>();
        let render_map = unsafe {
            let viewport = (*frame).viewport.cast::<UViewport>();
            (*(*viewport).actor.cast::<APlayerPawn>()).rend_map
        };
        let mut measures = hp2_render::scene_pipeline::hp2_scene_pipeline_helper_geometry::Hp2ScenePipelineHelperBoundStats::default();
        let stats = unsafe { self.graph.stats_ptr() };
        let began = hp2_core::unix_clock::cycles();
        let visible = unsafe {
            hp2_render::scene_pipeline::hp2_scene_pipeline_helper_geometry::hp2_scene_pipeline_helper_bound_visible(
                frame, &coords as *const hp2_abi::FCoords, span, bounds, render_map, &mut measures,
            )
        };
        unsafe {
            (*stats).box_time = (*stats).box_time.wrapping_add(hp2_core::unix_clock::cycles().wrapping_sub(began) as i32);
            (*stats).box_checks = (*stats).box_checks.wrapping_add(1);
            (*stats).box_in = (*stats).box_in.wrapping_add(measures.inside);
            (*stats).box_backs = (*stats).box_backs.wrapping_add(measures.behind);
            (*stats).box_out_of_pyramid = (*stats).box_out_of_pyramid.wrapping_add(measures.outside);
            (*stats).box_span_occluded = (*stats).box_span_occluded.wrapping_add(measures.occluded);
        }
        packet.integers[0] = visible;
        Ok(())
    }
}
