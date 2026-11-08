//! Canonical Core host for the source `UnRender.cpp` surface draw leaves.

use hp2_abi::{
    FVector,
    engine::generated::{
        AActor, ADecal, ALevelInfo, FColor, AZoneInfo, FSceneNode, FSurfaceFacet, FSurfaceInfo,
        FTextureInfo, HBspSurf, ULevel, UModel, UTexture, UViewport, URender, URenderDevice,
    },
    render_particle::FTransTexture,
    render_scene::{FBspDrawList, FBspSurf as PlainBspSurf, FSceneNode as PlainSceneNode, FSavedPoly},
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::scene_pipeline::draw::{DrawOperation, Hp2ScenePipelineDrawPacket};
use std::{cell::{Cell, UnsafeCell}, ffi::c_void, ptr};

const SURFACE_POINTS: usize = hp2_render::state::MAX_FINAL_VERTICES + 8;
const STAT_ENABLED: bool = hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0;

struct HitState {
    device: *mut URenderDevice,
    clock: Option<u32>,
}

/// Traversal-local scratch matching `Hp2ScenePipelineDrawContext`.
pub(super) struct SurfaceState<'a> {
    access: CoreAccess<'a>,
    maps: UnsafeCell<[FTextureInfo; 4]>,
    locked_slots: Cell<u8>,
    surface: UnsafeCell<FSurfaceInfo>,
    facet: UnsafeCell<FSurfaceFacet>,
    current_model: Cell<*mut UModel>,
    current_draw: Cell<*mut FBspDrawList>,
    setup: UnsafeCell<Option<crate::render_owner::lighting::SurfaceSetup<'a>>>,
    decal_vertices: UnsafeCell<[FTransTexture; SURFACE_POINTS]>,
    decal_points: UnsafeCell<[*mut FTransTexture; SURFACE_POINTS]>,
    decal_point_list: UnsafeCell<*mut *mut FTransTexture>,
    decal_count: Cell<i32>,
    hit_stack: UnsafeCell<Vec<HitState>>,
    decal_clock: Cell<Option<u32>>,
    clip_clock: Cell<Option<u32>>,
}

impl<'a> SurfaceState<'a> {
    pub(super) fn new(access: CoreAccess<'a>) -> Self {
        Self {
            access,
            maps: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            locked_slots: Cell::new(0),
            surface: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            facet: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            current_model: Cell::new(ptr::null_mut()),
            current_draw: Cell::new(ptr::null_mut()),
            setup: UnsafeCell::new(None),
            decal_vertices: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            decal_points: UnsafeCell::new([ptr::null_mut(); SURFACE_POINTS]),
            decal_point_list: UnsafeCell::new(ptr::null_mut()),
            decal_count: Cell::new(0),
            hit_stack: UnsafeCell::new(Vec::new()),
            decal_clock: Cell::new(None),
            clip_clock: Cell::new(None),
        }
    }

    /// Dispatches source-owned surface operations without retaining a Rust borrow
    /// across any texture, device, cache, or VM/provider call.
    pub(super) unsafe fn call(
        &self,
        render: *mut URender,
        frame: *mut FSceneNode,
        op: DrawOperation,
        packet: &mut Hp2ScenePipelineDrawPacket,
    ) -> RuntimeResult<()> {
        if render.is_null() || frame.is_null() {
            return Err(RuntimeError::new("Canonical surface draw requires live Render and SceneNode records"));
        }
        match op {
            DrawOperation::Surface => unsafe { self.surface(frame, packet) },
            DrawOperation::GetTexture => unsafe { self.get_texture(frame, packet) },
            DrawOperation::Texture => self.texture_auxiliary(packet),
            DrawOperation::Lock => unsafe { self.lock(frame, packet) },
            DrawOperation::Unlock => unsafe { self.unlock(packet) },
            DrawOperation::Light => unsafe { self.setup_surface_light(frame, packet) },
            DrawOperation::Transform => self.transform(frame),
            DrawOperation::Flat => unsafe { self.flat(frame, packet) },
            DrawOperation::HitPush => unsafe { self.hit_push(frame, packet) },
            DrawOperation::HitPop => unsafe { self.hit_pop(frame) },
            DrawOperation::Complex => unsafe { self.complex(frame, packet) },
            DrawOperation::LightFinish => self.finish_surface_light(),
            DrawOperation::Clip => unsafe { self.clip_decal(frame, packet) },
            DrawOperation::Triangles => unsafe { self.draw_decal(frame, packet) },
            DrawOperation::DecalTime => self.begin_decal_time(),
            DrawOperation::DecalTimeEnd => self.end_decal_time(),
            _ => Err(RuntimeError::new("Non-surface operation reached canonical surface host")),
        }
    }
    /// Finishes traversal-owned surface state and reports the first cleanup error.
    ///
    /// Every outstanding texture lock and hit proxy is still released if an earlier
    /// cleanup action fails; `Drop` remains a best-effort fallback.
    pub(super) fn finish(mut self) -> RuntimeResult<()> {
        self.finish_resources()
    }

    fn finish_resources(&mut self) -> RuntimeResult<()> {
        let mut first_error = None;
        if let Some(start) = self.decal_clock.take() {
            if let Ok(graph) = self.graph() {
                unclock_stat_at(graph.stats_ptr(), std::mem::offset_of!(hp2_abi::engine::generated::FStats, decal_time), start);
            }
        }
        if let Some(start) = self.clip_clock.take() {
            if let Ok(graph) = self.graph() {
                unclock_stat_at(graph.stats_ptr(), std::mem::offset_of!(hp2_abi::engine::generated::FStats, decal_clip_time), start);
            }
        }
        if let Some(setup) = unsafe { (&mut *self.setup.get()).take() } {
            if let Err(error) = crate::render_owner::lighting::finish_surface(self.access, setup) {
                first_error = Some(error);
            }
        }
        let mut locked = self.locked_slots.get();
        for slot in 0..4 {
            let mask = 1 << slot;
            if locked & mask != 0 {
                let info = unsafe { ptr::addr_of_mut!((*self.maps.get())[slot]) };
                let texture = unsafe { ptr::read_unaligned(ptr::addr_of!((*info).texture)) };
                if texture.is_null() {
                    locked &= !mask;
                } else if let Err(error) = unsafe { crate::texture_owner::unlock(self.access, texture.cast(), info) } {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                } else {
                    locked &= !mask;
                }
                self.locked_slots.set(locked);
            }
        }
        let hit_count = unsafe { (&*self.hit_stack.get()).len() };
        for _ in 0..hit_count {
            let Some(mut entry) = (unsafe { (&mut *self.hit_stack.get()).pop() }) else {
                break;
            };
            if let Some(start) = entry.clock.take() {
                if let Ok(graph) = self.graph() {
                    unclock_stat_at(graph.stats_ptr(), std::mem::offset_of!(hp2_abi::engine::generated::FStats, poly_v_time), start);
                }
            }
            if !entry.device.is_null() {
                if let Err(error) = unsafe {
                    crate::render_device_owner::pop_hit(self.access, entry.device, std::mem::size_of::<HBspSurf>() as i32, false)
                } {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                    let stack = unsafe { &mut *self.hit_stack.get() };
                    stack.push(entry);
                    if stack.len() > 1 {
                        stack.rotate_left(1);
                    }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }


    unsafe fn surface(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let draw_ptr = packet.p[0].cast::<FBspDrawList>();
        if draw_ptr.is_null() { return Err(RuntimeError::new("Canonical Surface has no draw list")); }
        let draw = unsafe { ptr::read_unaligned(draw_ptr) };
        let plain = frame.cast::<PlainSceneNode>();
        let level = unsafe { ptr::read_unaligned(ptr::addr_of!((*plain).level).cast::<*mut ULevel>()) };
        if level.is_null() { return Err(RuntimeError::new("Canonical surface SceneNode has no Level")); }
        let model = unsafe { ptr::read_unaligned(ptr::addr_of!((*level).model).cast::<*mut UModel>()) };
        if model.is_null() { return Err(RuntimeError::new("Canonical surface Level has no Model")); }
        let surfs = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).surfs)) };
        let points = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).points)) };
        let vectors = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).vectors)) };
        let surf = unsafe { array_item::<PlainBspSurf>(surfs.data, surfs.array_num, draw.i_surf, "Canonical surface index is invalid")? };
        let base = unsafe { array_item::<FVector>(points.data, points.array_num, surf.p_base, "Canonical surface base point is invalid")? };
        let u = unsafe { array_item::<FVector>(vectors.data, vectors.array_num, surf.v_texture_u, "Canonical surface U vector is invalid")? };
        let v = unsafe { array_item::<FVector>(vectors.data, vectors.array_num, surf.v_texture_v, "Canonical surface V vector is invalid")? };
        let normal = unsafe { array_item::<FVector>(vectors.data, vectors.array_num, surf.v_normal, "Canonical surface normal is invalid")? };
        let info = unsafe { level_info(level)? };
        let time = unsafe { ptr::read_unaligned(ptr::addr_of!((*info).time_seconds)) };
        let zone = draw.zone.cast::<AZoneInfo>();
        if zone.is_null() { return Err(RuntimeError::new("Canonical surface draw list has no Zone")); }
        self.current_model.set(model);
        self.current_draw.set(draw_ptr);
        unsafe {
            ptr::write(self.surface.get(), FSurfaceInfo {
                poly_flags: draw.poly_flags,
                flat_color: FColor { b: 0, g: 0, r: 0, a: 0 },
                level: level.cast(), texture: ptr::null_mut(), light_map: ptr::null_mut(),
                macro_texture: ptr::null_mut(), detail_texture: ptr::null_mut(), fog_map: ptr::null_mut(),
            });
            ptr::write(self.facet.get(), FSurfaceFacet {
                map_coords: hp2_abi::FCoords { origin: base, x_axis: u, y_axis: v, z_axis: normal },
                map_uncoords: std::mem::zeroed(),
                span: ptr::addr_of_mut!((*draw_ptr).span).cast(),
                polys: draw.polys.cast(),
            });
        }
        packet.p[0] = (surfs.data.cast::<PlainBspSurf>().wrapping_add(draw.i_surf as usize)).cast();
        packet.p[1] = self.surface.get().cast();
        packet.p[2] = self.facet.get().cast();
        packet.f[0] = time;
        packet.f[1] = unsafe { ptr::read_unaligned(ptr::addr_of!((*zone).tex_u_pan_speed)) };
        packet.f[2] = unsafe { ptr::read_unaligned(ptr::addr_of!((*zone).tex_v_pan_speed)) };
        Ok(())
    }

    unsafe fn get_texture(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let viewport = unsafe { ptr::read_unaligned(ptr::addr_of!((*frame).viewport).cast::<*mut UViewport>()) };
        if viewport.is_null() { return Err(RuntimeError::new("Canonical texture Get has no Viewport")); }
        let time = unsafe { i64::from_ne_bytes(ptr::read_unaligned(ptr::addr_of!((*viewport).current_time))) };
        packet.p[0] = unsafe { crate::texture_owner::get(self.access, packet.p[0].cast(), time)? }.cast();
        Ok(())
    }

    fn texture_auxiliary(&self, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        if packet.p[0].is_null() { return Err(RuntimeError::new("Canonical Texture operation has a null texture")); }
        let texture = packet.p[0].cast::<UTexture>();
        packet.p[0] = unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).detail_texture)) };
        packet.p[1] = unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).macro_texture)) };
        let owner = self.access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("Canonical Engine owner context is unavailable"))?;
        packet.p[2] = owner.bitmap_client.get().map_or(ptr::null_mut(), |client| client.as_ptr().cast());
        Ok(())
    }

    unsafe fn lock(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let slot = usize::try_from(packet.n[0]).ok().filter(|slot| *slot < 4)
            .ok_or_else(|| RuntimeError::new("Canonical surface texture slot is outside Maps[4]"))?;
        let texture = packet.p[0].cast::<UTexture>();
        if texture.is_null() { return Err(RuntimeError::new("Canonical texture Lock has a null texture")); }
        let viewport = unsafe { ptr::read_unaligned(ptr::addr_of!((*frame).viewport).cast::<*mut UViewport>()) };
        if viewport.is_null() { return Err(RuntimeError::new("Canonical texture Lock has no Viewport")); }
        let time = unsafe { i64::from_ne_bytes(ptr::read_unaligned(ptr::addr_of!((*viewport).current_time))) };
        let device = unsafe { ptr::read_unaligned(ptr::addr_of!((*viewport).ren_dev)).cast::<URenderDevice>() };
        let info = unsafe { ptr::addr_of_mut!((*self.maps.get())[slot]) };
        unsafe { crate::texture_owner::lock(self.access, texture, info, time, -1, device)?; }
        self.locked_slots.set(self.locked_slots.get() | (1 << slot));
        match slot {
            0 => {
                unsafe { ptr::addr_of_mut!((*info).pan).write_unaligned(FVector { x: packet.f[0], y: packet.f[1], z: 0.0 }); }
                unsafe { ptr::addr_of_mut!((*self.surface.get()).texture).write_unaligned(info.cast()); }
            }
            1 => unsafe { ptr::addr_of_mut!((*self.surface.get()).detail_texture).write_unaligned(info.cast()); },
            2 => unsafe { ptr::addr_of_mut!((*self.surface.get()).macro_texture).write_unaligned(info.cast()); },
            _ => {}
        }
        Ok(())
    }

    unsafe fn unlock(&self, packet: &Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let slot = usize::try_from(packet.n[0]).ok().filter(|slot| *slot < 4)
            .ok_or_else(|| RuntimeError::new("Canonical surface texture slot is outside Maps[4]"))?;
        if packet.p[0].is_null() { return Err(RuntimeError::new("Canonical texture Unlock has a null texture")); }
        unsafe { crate::texture_owner::unlock(self.access, packet.p[0].cast(), ptr::addr_of_mut!((*self.maps.get())[slot]))?; }
        self.locked_slots.set(self.locked_slots.get() & !(1 << slot));
        Ok(())
    }

    unsafe fn transform(&self, frame: *mut FSceneNode) -> RuntimeResult<()> {
        let mut coords = unsafe { ptr::read_unaligned(ptr::addr_of!((*self.facet.get()).map_coords)) };
        let frame_coords = unsafe { ptr::read_unaligned(ptr::addr_of!((*frame.cast::<PlainSceneNode>()).coords)) };
        crate::coords::mul_coords(&mut coords, &frame_coords);
        unsafe { ptr::addr_of_mut!((*self.facet.get()).map_coords).write_unaligned(coords); }
        Ok(())
    }

    unsafe fn flat(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        match packet.n[0] {
            0 => {
                let draw = packet.p[0].cast::<FBspDrawList>();
                if draw.is_null() { return Err(RuntimeError::new("Canonical flat texture lookup has no draw list")); }
                let level = unsafe { ptr::read_unaligned(ptr::addr_of!((*frame).level)).cast::<ULevel>() };
                if level.is_null() { return Err(RuntimeError::new("Canonical flat texture lookup has no Level")); }
                let model = unsafe { ptr::read_unaligned(ptr::addr_of!((*level).model)).cast::<UModel>() };
                if model.is_null() { return Err(RuntimeError::new("Canonical flat texture lookup has no Model")); }
                let nodes = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).nodes)) };
                let node = unsafe { array_item::<hp2_abi::render_scene::FBspNode>(nodes.data, nodes.array_num, ptr::read_unaligned(ptr::addr_of!((*draw).i_node)), "Canonical BSP node index is invalid")? };
                let surfaces = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).surfs)) };
                let surf = unsafe { array_item::<PlainBspSurf>(surfaces.data, surfaces.array_num, node.i_surf, "Canonical BSP surface index is invalid")? };
                packet.p[0] = surf.texture;
            }
            1 => {
                let texture = packet.p[0].cast::<UTexture>();
                if texture.is_null() { return Err(RuntimeError::new("Canonical texture flat lookup has a null texture")); }
                packet.n[0] = unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).base.base.index)) };
                let color = unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).base.mip_zero).cast::<FColor>()) };
                let plane = crate::tex::color_plane(color.r, color.g, color.b);
                packet.f[0] = plane[0]; packet.f[1] = plane[1]; packet.f[2] = plane[2];
            }
            2 => {
                let color = crate::tex::fget_hsv(packet.n[1] as u8, 0, 255);
                packet.f[0] = color[0]; packet.f[1] = color[1]; packet.f[2] = color[2];
            }
            3 => unsafe {
                ptr::addr_of_mut!((*self.surface.get()).flat_color).write_unaligned({ let c = crate::tex::color_of_plane([packet.f[0], packet.f[1], packet.f[2], 0.0]); FColor { r: c.r, g: c.g, b: c.b, a: c.a } });
            },
            _ => return Err(RuntimeError::new("Canonical Flat operation has an invalid mode")),
        }
        Ok(())
    }

    unsafe fn hit_push(&self, frame: *mut FSceneNode, _packet: &Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let draw = self.current_draw.get();
        if draw.is_null() { return Err(RuntimeError::new("Canonical BSP hit push has no active draw list")); }
        let viewport = unsafe { ptr::read_unaligned(ptr::addr_of!((*frame).viewport).cast::<*mut UViewport>()) };
        if viewport.is_null() { return Err(RuntimeError::new("Canonical BSP hit push has no Viewport")); }
        let stats = self.graph()?.stats_ptr();
        let stack = self.hit_stack.get();
        let index = unsafe {
            let stack = &mut *stack;
            stack.try_reserve(1).map_err(|_| RuntimeError::new("Canonical hit proxy stack allocation failed"))?;
            stack.push(HitState { device: ptr::null_mut(), clock: None });
            stack.len() - 1
        };
        let hit_testing = unsafe { ptr::read_unaligned(ptr::addr_of!((*viewport).hit_testing)) != 0 };
        let device = if hit_testing {
            let device = unsafe { ptr::read_unaligned(ptr::addr_of!((*viewport).ren_dev)).cast::<URenderDevice>() };
            let mut hit: HBspSurf = unsafe { std::mem::zeroed() };
            unsafe {
                ptr::addr_of_mut!((*ptr::addr_of_mut!(hit)).base.size).write_unaligned(std::mem::size_of::<HBspSurf>() as i32);
                ptr::addr_of_mut!((*ptr::addr_of_mut!(hit)).i_surf).write_unaligned(ptr::read_unaligned(ptr::addr_of!((*draw).i_surf)));
            }
            if let Err(error) = unsafe { crate::render_device_owner::push_hit(self.access, device, (&hit as *const HBspSurf).cast(), std::mem::size_of::<HBspSurf>() as i32) } {
                unsafe { (&mut *stack).remove(index); }
                return Err(error);
            }
            device
        } else {
            ptr::null_mut()
        };
        let clock = clock_stat(stats, std::mem::offset_of!(hp2_abi::engine::generated::FStats, poly_v_time));
        unsafe {
            let state = &mut (&mut *stack)[index];
            state.device = device;
            state.clock = clock;
        }
        Ok(())
    }

    unsafe fn hit_pop(&self, _frame: *mut FSceneNode) -> RuntimeResult<()> {
        let stats = self.graph()?.stats_ptr();
        let entry = unsafe { (&mut *self.hit_stack.get()).pop() };
        let Some(entry) = entry else { return Ok(()); };
        if let Some(start) = entry.clock {
            unclock_stat_at(stats, std::mem::offset_of!(hp2_abi::engine::generated::FStats, poly_v_time), start);
        }
        if !entry.device.is_null() {
            unsafe { crate::render_device_owner::pop_hit(self.access, entry.device, std::mem::size_of::<HBspSurf>() as i32, false)?; }
        }
        Ok(())
    }

    unsafe fn complex(&self, frame: *mut FSceneNode, packet: &Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let viewport = unsafe { ptr::read_unaligned(ptr::addr_of!((*frame).viewport).cast::<*mut UViewport>()) };
        if viewport.is_null() { return Err(RuntimeError::new("Canonical complex surface has no Viewport")); }
        let device = unsafe { ptr::read_unaligned(ptr::addr_of!((*viewport).ren_dev)).cast::<URenderDevice>() };
        unsafe { crate::render_device_owner::draw_complex_surface(self.access, device, frame, self.surface.get(), self.facet.get(), packet.n[0] as u32, packet.n[1] as u8) }
    }

    unsafe fn setup_surface_light(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        if unsafe { (&*self.setup.get()).is_some() } {
            return Err(RuntimeError::new("Canonical surface light setup was not finished before the next setup"));
        }
        let draw = self.current_draw.get();
        if draw.is_null() { return Err(RuntimeError::new("Canonical SetupForSurf has no active draw list")); }
        let setup = unsafe {
            crate::render_owner::lighting::setup_surface(
                self.access, frame, ptr::addr_of_mut!((*self.facet.get()).map_coords), draw, packet.n[0],
            )?
        };
        let light_map = setup.light_map;
        let fog_map = setup.fog_map;
        unsafe { *self.setup.get() = Some(setup); }
        unsafe {
            ptr::addr_of_mut!((*self.surface.get()).light_map).write_unaligned(light_map.cast());
            ptr::addr_of_mut!((*self.surface.get()).fog_map).write_unaligned(fog_map.cast());
        }
        Ok(())
    }

    fn finish_surface_light(&self) -> RuntimeResult<()> {
        let setup = unsafe { (&mut *self.setup.get()).take() }
            .ok_or_else(|| RuntimeError::new("Canonical FinishSurf has no live SetupForSurf guard"))?;
        crate::render_owner::lighting::finish_surface(self.access, setup)
    }

    unsafe fn clip_decal(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let decal = packet.p[1].cast::<hp2_abi::engine::generated::FDecal>();
        let surf = packet.p[0].cast::<PlainBspSurf>();
        let poly = packet.p[2].cast::<FSavedPoly>();
        if decal.is_null() || surf.is_null() || poly.is_null() { return Err(RuntimeError::new("Canonical decal Clip has invalid geometry")); }
        let model = self.current_model.get();
        if model.is_null() { return Err(RuntimeError::new("Canonical decal Clip has no active Model")); }
        let points = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).points)) };
        let vectors = unsafe { ptr::read_unaligned(ptr::addr_of!((*model).vectors)) };
        let surf_base = unsafe { array_item::<FVector>(points.data, points.array_num, ptr::read_unaligned(ptr::addr_of!((*surf).p_base)), "Canonical decal surface base is invalid")? };
        let surf_normal = unsafe { array_item::<FVector>(vectors.data, vectors.array_num, ptr::read_unaligned(ptr::addr_of!((*surf).v_normal)), "Canonical decal surface normal is invalid")? };
        if STAT_ENABLED && self.clip_clock.get().is_none() {
            self.clip_clock.set(clock_stat(self.graph()?.stats_ptr(), std::mem::offset_of!(hp2_abi::engine::generated::FStats, decal_clip_time)));
        }
        let count = unsafe { hp2_render::scene_pipeline::hp2_scene_pipeline_helper_geometry::hp2_scene_pipeline_helper_clip_decal_with(
            &CoreDecalOps, frame.cast::<PlainSceneNode>(), decal.cast(), (*decal).vertices.as_ptr(), &surf_normal, &surf_base, poly,
            (*self.decal_vertices.get()).as_mut_ptr(), (*self.decal_points.get()).as_mut_ptr(),
            self.decal_point_list.get(), self.decal_count.as_ptr(),
        ) };
        unclock_stat(self.graph()?.stats_ptr(), &self.clip_clock, std::mem::offset_of!(hp2_abi::engine::generated::FStats, decal_clip_time));
        if count < 0 { return Err(RuntimeError::new("Canonical decal clipping failed")); }
        packet.p[0] = unsafe { self.decal_point_list.get().read() }.cast();
        packet.n[0] = count;
        Ok(())
    }

    unsafe fn draw_decal(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        let decal = packet.p[0].cast::<ADecal>();
        if decal.is_null() { return Err(RuntimeError::new("Canonical decal Triangles has no ADecal")); }
        let level = unsafe { ptr::read_unaligned(ptr::addr_of!((*decal).base.level)).cast::<ALevelInfo>() };
        if level.is_null() { return Err(RuntimeError::new("Canonical decal Triangles has no Level")); }
        let level_time = unsafe { ptr::read_unaligned(ptr::addr_of!((*level).time_seconds)) } as f64;
        unsafe { ptr::addr_of_mut!((*decal).last_rendered_time).write_unaligned(level_time as f32); }
        let viewport = unsafe { ptr::read_unaligned(ptr::addr_of!((*frame).viewport).cast::<*mut UViewport>()) };
        if viewport.is_null() { return Err(RuntimeError::new("Canonical decal Triangles has no Viewport")); }
        let device = unsafe { ptr::read_unaligned(ptr::addr_of!((*viewport).ren_dev)).cast::<URenderDevice>() };
        let info = unsafe { ptr::addr_of_mut!((*self.maps.get())[3]) };
        let points = packet.p[1].cast::<*mut FTransTexture>();
        let count = packet.n[0];
        if count < 0 || (count > 0 && points.is_null()) { return Err(RuntimeError::new("Canonical decal Triangles has invalid point span")); }
        let draw = packet.p[2].cast::<FBspDrawList>();
        if draw.is_null() { return Err(RuntimeError::new("Canonical decal Triangles has no draw list")); }
        let span = unsafe { ptr::addr_of_mut!((*draw).span).cast::<c_void>() };
        unsafe { crate::render_device_owner::draw_triangles(self.access, device, frame, info, points.cast::<*mut hp2_abi::engine::generated::FTransTexture>().cast(), count, ptr::null_mut(), count, packet.n[1] as u32, span)?; }
        if STAT_ENABLED {
            let stats = self.graph()?.stats_ptr();
            let count = unsafe { ptr::addr_of_mut!((*stats).decal_count) };
            unsafe { count.write_unaligned(count.read_unaligned().wrapping_add(1)); }
        }
        Ok(())
    }

    fn begin_decal_time(&self) -> RuntimeResult<()> {
        if STAT_ENABLED && self.decal_clock.get().is_none() {
            self.decal_clock.set(clock_stat(self.graph()?.stats_ptr(), std::mem::offset_of!(hp2_abi::engine::generated::FStats, decal_time)));
        }
        Ok(())
    }

    fn end_decal_time(&self) -> RuntimeResult<()> {
        unclock_stat(self.graph()?.stats_ptr(), &self.decal_clock, std::mem::offset_of!(hp2_abi::engine::generated::FStats, decal_time));
        Ok(())
    }

    fn graph(&self) -> RuntimeResult<&crate::render_owner::RenderGraphState> {
        self.access.owner_context::<crate::core_owner::EngineState>()
            .map(|owner| owner.render_graph.as_ref())
            .ok_or_else(|| RuntimeError::new("Canonical Engine owner context is unavailable"))
    }
}

impl Drop for SurfaceState<'_> {
    fn drop(&mut self) {
        let _ = self.finish_resources();
    }
}

struct CoreDecalOps;
impl hp2_render::scene_pipeline::hp2_scene_pipeline_helper_geometry::DecalClipOps for CoreDecalOps {
    unsafe fn material(&self, decal: *mut c_void, out: *mut hp2_render::scene_pipeline::hp2_scene_pipeline_helper_geometry::Hp2ScenePipelineHelperDecalMaterial) -> i32 {
        if decal.is_null() || out.is_null() { return 0; }
        let actor = unsafe { ptr::read_unaligned(ptr::addr_of!((*decal.cast::<hp2_abi::engine::generated::FDecal>()).actor)).cast::<AActor>() };
        if actor.is_null() { return 0; }
        unsafe {
            ptr::addr_of_mut!((*out).scale_glow).write_unaligned(ptr::read_unaligned(ptr::addr_of!((*actor).scale_glow)));
            ptr::addr_of_mut!((*out).ambient_glow).write_unaligned(ptr::read_unaligned(ptr::addr_of!((*actor).ambient_glow)));
            ptr::addr_of_mut!((*out).draw_scale).write_unaligned(ptr::read_unaligned(ptr::addr_of!((*actor).draw_scale)));
        }
        1
    }

    unsafe fn check_capacity(&self, count: i32) -> i32 {
        i32::from(count < SURFACE_POINTS as i32)
    }
}

fn clock_stat(stats: *mut hp2_abi::engine::generated::FStats, offset: usize) -> Option<u32> {
    if !STAT_ENABLED || stats.is_null() { return None; }
    let start = hp2_core::unix_clock::cycles();
    unsafe {
        let value = stats.cast::<u8>().add(offset).cast::<i32>();
        value.write_unaligned(value.read_unaligned().wrapping_sub(start as i32));
    }
    Some(start)
}

fn unclock_stat(stats: *mut hp2_abi::engine::generated::FStats, start: &Cell<Option<u32>>, offset: usize) {
    if let Some(start) = start.take() { unclock_stat_at(stats, offset, start); }
}

fn unclock_stat_at(stats: *mut hp2_abi::engine::generated::FStats, offset: usize, start: u32) {
    if !STAT_ENABLED || stats.is_null() { return; }
    let elapsed = hp2_core::unix_clock::cycles().wrapping_sub(start).wrapping_sub(34) as i32;
    unsafe {
        let value = stats.cast::<u8>().add(offset).cast::<i32>();
        value.write_unaligned(value.read_unaligned().wrapping_add(elapsed));
    }
}

unsafe fn level_info(level: *mut ULevel) -> RuntimeResult<*mut ALevelInfo> {
    if level.is_null() { return Err(RuntimeError::new("Canonical renderer has no Level")); }
    let actors = unsafe { ptr::read_unaligned(ptr::addr_of!((*level).base.actors)) };
    if actors.data.is_null() || actors.array_num <= 0 { return Err(RuntimeError::new("Canonical Level has no LevelInfo actor")); }
    let info = unsafe { ptr::read_unaligned(actors.data.cast::<*mut ALevelInfo>()) };
    if info.is_null() { return Err(RuntimeError::new("Canonical Level has a null LevelInfo actor")); }
    Ok(info)
}

unsafe fn array_item<T: Copy>(data: *mut c_void, count: i32, index: i32, label: &'static str) -> RuntimeResult<T> {
    let index = usize::try_from(index).ok().filter(|&index| index < count.max(0) as usize)
        .ok_or_else(|| RuntimeError::new(label))?;
    if data.is_null() { return Err(RuntimeError::new(label)); }
    Ok(unsafe { ptr::read_unaligned(data.cast::<T>().add(index)) })
}
