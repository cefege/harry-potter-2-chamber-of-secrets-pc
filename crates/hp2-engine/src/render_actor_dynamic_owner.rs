//! Core-backed source operations for dynamic actor rendering.

use crate::render_owner::{CoreSceneStackOps, RenderGraphState, URender};
use hp2_abi::engine::generated::{AActor, AParticleFX, APlayerPawn, FBspSurf, FSceneNode, ULevel, UModel, UTexture, UViewport};
use hp2_abi::FVector;
use hp2_abi::render_scene::{FActorLink, FDynamicItem, FDynamicSprite, FScreenBounds, FVolActorLink};
use hp2_abi::render_span::{FMemMark, FSpanBuffer};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::mem_owner::MemStackOwner;
use hp2_render::dynamic_items::{
    self, DynamicBounds, DynamicBspSplit, DynamicLightInfo, DynamicOperation, DynamicOps,
    DynamicSetup, DynamicSurfaceInfo, STANDALONE,
};
use hp2_render::sprite_filter::FRasterPolyHeader;
use hp2_render::span::SpanAlloc;
use std::{cell::RefCell, ffi::c_void, ptr, rc::Rc};

const SHOW_ACTOR_ICONS: i32 = 0x20;
const SHOW_REAL_TIME: i32 = 0x4000;
const SHOW_PLAYER_CTRL: i32 = 0x800;
const DRAW_SPRITE: u8 = 1;
const DRAW_SPRITE_ANIM_ONCE: u8 = 7;
const DRAW_MESH: u8 = 2;
const DRAW_PARTICLES: u8 = 8;
const PF_SPECIAL_LIT: u32 = 0x0000_0040;

#[repr(C, packed(4))]
struct RenderIteratorLayout {
    base: hp2_abi::engine::generated::UObject,
    max_items: i32,
    index: i32,
    observer: *mut APlayerPawn,
    frame: *mut FSceneNode,
}
const _: () = assert!(std::mem::size_of::<RenderIteratorLayout>() == 96);
const _: () = assert!(std::mem::offset_of!(RenderIteratorLayout, max_items) == 72);
const _: () = assert!(std::mem::offset_of!(RenderIteratorLayout, index) == 76);
const _: () = assert!(std::mem::offset_of!(RenderIteratorLayout, observer) == 80);
const _: () = assert!(std::mem::offset_of!(RenderIteratorLayout, frame) == 88);

struct IteratorFields {
    iterator: *mut RenderIteratorLayout,
}

unsafe extern "C" fn iterator_field(
    context: *mut c_void,
    leaf: crate::render_iterator::Hp2RenderIteratorLeaf,
    io: *mut crate::render_iterator::Hp2RenderIteratorIo,
) -> i32 {
    if context.is_null() || io.is_null() {
        return 1;
    }
    let fields = unsafe { &mut *context.cast::<IteratorFields>() };
    if fields.iterator.is_null() {
        return 1;
    }
    let iterator = fields.iterator;
    let io = unsafe { &mut *io };
    use crate::render_iterator::Hp2RenderIteratorLeaf as Leaf;
    unsafe {
        match leaf {
            Leaf::SetFrame => ptr::addr_of_mut!((*iterator).frame).write_unaligned(io.pointer.cast()),
            Leaf::CameraActor => {
                let frame = io.pointer.cast::<FSceneNode>();
                let viewport = ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>();
                io.pointer = if viewport.is_null() {
                    ptr::null_mut()
                } else {
                    ptr::addr_of!((*viewport).actor).read_unaligned().cast()
                };
            }
            Leaf::SetObserver => ptr::addr_of_mut!((*iterator).observer).write_unaligned(io.pointer.cast()),
            Leaf::ReadIndex => io.value = ptr::addr_of!((*iterator).index).read_unaligned(),
            Leaf::WriteIndex => ptr::addr_of_mut!((*iterator).index).write_unaligned(io.value),
            Leaf::ReadMaxItems => io.value = ptr::addr_of!((*iterator).max_items).read_unaligned(),
            Leaf::Outer => io.pointer = ptr::addr_of!((*iterator).base.outer).read_unaligned(),
        }
    }
    0
}

fn render_iterator_op(
    iterator: *mut c_void,
    operation: crate::render_iterator::Hp2RenderIteratorOperation,
    camera: *mut FSceneNode,
) -> RuntimeResult<crate::render_iterator::Hp2RenderIteratorIo> {
    let mut fields = IteratorFields { iterator: iterator.cast() };
    let services = crate::render_iterator::Hp2RenderIteratorServices {
        context: (&mut fields as *mut IteratorFields).cast(),
        invoke: Some(iterator_field),
    };
    unsafe { crate::render_iterator::run(services, operation, camera.cast()) }
        .map_err(|_| RuntimeError::new("Core render iterator field operation failed"))
}

struct CoreDynamicOps<'a> {
    access: CoreAccess<'a>,
    frame: *mut FSceneNode,
    graph: Rc<RenderGraphState>,
    stacks: CoreSceneStackOps<'a>,
    error: RefCell<Option<RuntimeError>>,
    bounds: RefCell<hp2_abi::FCoords>,
    leaf_lights_base: *mut *mut FVolActorLink,
    span_mark: RefCell<Option<FMemMark>>,
}

impl<'a> CoreDynamicOps<'a> {
    fn new(
        access: CoreAccess<'a>,
        render: *mut URender,
        frame: *mut FSceneNode,
        leaf_lights_base: *mut *mut FVolActorLink,
    ) -> RuntimeResult<Self> {
        if render.is_null() || frame.is_null() {
            return Err(RuntimeError::new("Canonical dynamic renderer requires a live renderer and frame"));
        }
        let graph = crate::render_owner::state(access)?;
        if graph.render()?.as_ptr() != render {
            return Err(RuntimeError::new("Canonical dynamic renderer does not own the active Render"));
        }
        Ok(Self {
            access,
            frame,
            graph,
            stacks: CoreSceneStackOps::new(access)?,
            error: RefCell::new(None),
            bounds: RefCell::new(unsafe { std::mem::zeroed() }),
            leaf_lights_base,
            span_mark: RefCell::new(None),
        })
    }
    fn new_bounds(access: CoreAccess<'a>) -> RuntimeResult<Self> {
        let graph = crate::render_owner::state(access)?;
        Ok(Self {
            access,
            graph,
            frame: ptr::null_mut(),
            stacks: CoreSceneStackOps::new(access)?,
            error: RefCell::new(None),
            bounds: RefCell::new(unsafe { std::mem::zeroed() }),
            leaf_lights_base: ptr::null_mut(),
            span_mark: RefCell::new(None),
        })
    }

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

    fn fail<T>(&self, message: &'static str) -> Result<T, ()> {
        self.record(RuntimeError::new(message));
        Err(())
    }

    fn model(&self) -> Result<*mut UModel, ()> {
        if self.frame.is_null() {
            return self.fail("Canonical dynamic renderer has no scene frame");
        }
        let level = unsafe { ptr::addr_of!((*self.frame).level).read_unaligned().cast::<ULevel>() };
        if level.is_null() {
            return self.fail("Canonical dynamic scene frame has no level");
        }
        let model = unsafe { ptr::addr_of!((*level).model).read_unaligned().cast::<UModel>() };
        if model.is_null() {
            return self.fail("Canonical dynamic scene level has no model");
        }
        Ok(model)
    }

    fn node(&self, index: i32) -> Result<*mut hp2_abi::render_scene::FBspNode, ()> {
        let model = self.model()?;
        let nodes = unsafe { ptr::addr_of!((*model).nodes).read_unaligned() };
        if index < 0 || index >= nodes.array_num || nodes.data.is_null() {
            return self.fail("Dynamic renderer BSP node index is invalid");
        }
        Ok(unsafe { nodes.data.cast::<hp2_abi::render_scene::FBspNode>().add(index as usize) })
    }

    fn leaf_slot(&self, leaf: i32) -> Result<*mut *mut FVolActorLink, ()> {
        if leaf < 0 {
            return self.fail("Dynamic renderer leaf index is negative");
        }
        let leaf = leaf as usize;
        if !self.leaf_lights_base.is_null() {
            let limit = self.graph.max_leaf_lights();
            if limit > 0 && leaf >= limit as usize {
                return self.fail("Dynamic renderer leaf index exceeds renderer light storage");
            }
            return Ok(unsafe { self.leaf_lights_base.add(leaf) });
        }
        let slot = self.graph.with_leaf_lights(|lights| {
            if leaf >= lights.len() {
                if lights.try_reserve(leaf + 1 - lights.len()).is_err() {
                    return None;
                }
                lights.resize(leaf + 1, ptr::null_mut());
            }
            Some(unsafe { lights.as_mut_ptr().add(leaf) })
        });
        match slot {
            Some(slot) => Ok(slot),
            None => self.fail("Canonical dynamic leaf-light storage allocation failed"),
        }
    }

    fn surf_slot(&self, surf: i32) -> Result<*mut *mut FActorLink, ()> {
        if surf < 0 {
            return self.fail("Dynamic renderer surface index is negative");
        }
        let surf = surf as usize;
        let slot = self.graph.with_surf_lights(|lights| {
            if surf >= lights.len() {
                if lights.try_reserve(surf + 1 - lights.len()).is_err() {
                    return None;
                }
                lights.resize(surf + 1, ptr::null_mut());
            }
            Some(unsafe { lights.as_mut_ptr().add(surf) })
        });
        match slot {
            Some(slot) => Ok(slot),
            None => self.fail("Canonical dynamic surface-light storage allocation failed"),
        }
    }

    fn allocate_link<T>(&self, value: T, stack: &MemStackOwner) -> Result<*mut T, ()> {
        let size = i32::try_from(std::mem::size_of::<T>())
            .map_err(|_| { self.record(RuntimeError::new("Dynamic renderer link allocation size overflow")); () })?;
        let ptr = unsafe { stack.push_bytes(size, std::mem::align_of::<T>() as i32) }
            .map_err(|error| { self.record(error.with_context("dynamic renderer link allocation")); () })?;
        let link = ptr.cast::<T>();
        unsafe { link.write(value); }
        Ok(link)
    }

    fn render_bounds(&self, actor: *mut AActor) -> Result<hp2_abi::FCoords, ()> {
        let actor_ref = unsafe { &*actor };
        if !actor_ref.mesh.is_null() {
            return self.remember(unsafe {
                crate::render_owner::particle_mesh_bounds(self.access, actor)
            });
        }
        if !actor_ref.brush.is_null() {
            let model = unsafe { &*actor_ref.brush.cast::<UModel>() };
            let bounds = unsafe { ptr::addr_of!(model.base.base.base.bounding_box).read_unaligned() };
            let actor_coords = hp2_core::math::coords_mul_point(
                hp2_core::math::unit_coords(),
                actor_ref.location,
            );
            let actor_coords = hp2_core::math::coords_mul_rotator(
                actor_coords,
                hp2_core::math::FRotator {
                    pitch: actor_ref.rotation.pitch,
                    yaw: actor_ref.rotation.yaw,
                    roll: actor_ref.rotation.roll,
                },
                hp2_core::math::trig_table(),
            );
            let actor_coords = hp2_core::math::coords_mul_point(actor_coords, actor_ref.pre_pivot);
            let bounds = crate::prim::Aabb::from(crate::mesh::FBoxRaw {
                min: bounds.min,
                max: bounds.max,
                is_valid: i32::from(bounds.is_valid != 0),
            });
            return Ok(crate::coords::box_times_coords(&bounds, &actor_coords));
        }
        if unsafe { hp2_core::object_model::is_a(
            actor.cast(),
            self.remember(self.access.native_class("Engine.ParticleFX"))?.cast(),
        ) } {
            return self.remember(unsafe { crate::particle_fx_owner::render_bounds(
                self.access,
                actor.cast::<AParticleFX>(),
            ) });
        }
        Ok(unsafe { std::mem::zeroed() })
    }

    fn discard_span_mark(&self) {
        if let Some(mark) = self.span_mark.borrow_mut().take() {
            if let Some(engine) = self.access.owner_context::<crate::core_owner::EngineState>() {
                unsafe { engine.memory.pop(&mark); }
            } else {
                self.record(RuntimeError::new("Canonical dynamic span has no Engine owner during cleanup"));
            }
        }
    }

    fn project_box(&self, result: &mut DynamicBounds, visible: &mut bool) -> Result<(), ()> {
        let frame = self.frame;
        let mut stats = hp2_render::scene_pipeline::hp2_scene_pipeline_helper_geometry::Hp2ScenePipelineHelperBoundStats::default();
        let viewport = unsafe { ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>() };
        if viewport.is_null() {
            return self.fail("Canonical dynamic scene frame has no viewport");
        }
        let view_actor = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned().cast::<APlayerPawn>() };
        if view_actor.is_null() {
            return self.fail("Canonical dynamic viewport has no view actor");
        }
        let rend_map = unsafe { ptr::addr_of!((*view_actor).rend_map).read_unaligned() };
        let coords = *self.bounds.borrow();
        let mut output = FScreenBounds::default();
        *visible = unsafe {
            hp2_render::scene_pipeline::hp2_scene_pipeline_helper_geometry::hp2_scene_pipeline_helper_bound_visible(
                frame.cast(), &coords, ptr::null(), &mut output, rend_map, &mut stats,
            ) != 0
        };
        if *visible {
            *result = DynamicBounds {
                min_x: output.min_x, max_x: output.max_x,
                min_y: output.min_y, max_y: output.max_y,
                min_z: output.min_z, max_z: output.max_z,
            };
        }
        Ok(())
    }
}

impl DynamicOps for CoreDynamicOps<'_> {
    unsafe fn operation(&mut self, frame: *mut hp2_abi::render_scene::FSceneNode, operation: DynamicOperation<'_>) -> Result<(), ()> {
        let frame = frame.cast::<FSceneNode>();
        use DynamicOperation as Op;
        match operation {
            Op::Setup { actor, result } => {
                if actor.is_null() { return self.fail("Null actor in dynamic sprite setup"); }
                let actor = actor.cast::<AActor>();
                let viewport = unsafe { ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>() };
                if viewport.is_null() { return self.fail("Dynamic sprite frame has no viewport"); }
                let view = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned().cast::<APlayerPawn>() };
                if view.is_null() { return self.fail("Dynamic sprite viewport has no actor"); }
                let flags = unsafe { ptr::addr_of!((*view).show_flags).read_unaligned() };
                let rend_map = unsafe { ptr::addr_of!((*view).rend_map).read_unaligned() };
                let draw_type = unsafe { ptr::addr_of!((*actor).draw_type).read_unaligned() };
                let icons = flags & SHOW_ACTOR_ICONS != 0;
                let mut texture = unsafe { ptr::addr_of!((*actor).texture).read_unaligned().cast::<UTexture>() };
                if icons && texture.is_null() {
                    let class = self.remember(self.access.native_class("Engine.Actor"))?;
                    let defaults = self.remember(unsafe { self.access.default_object(class) })?.cast::<AActor>();
                    if defaults.is_null() { return self.fail("Canonical default Actor is unavailable for icon rendering"); }
                    texture = unsafe { ptr::addr_of!((*defaults).texture).read_unaligned().cast::<UTexture>() };
                }
                let particle = self.remember(self.access.native_class("Engine.ParticleFX"))?;
                let shell_only = !particle.is_null()
                    && unsafe { hp2_core::object_model::is_a(actor.cast(), particle.cast()) }
                    && unsafe { (*actor.cast::<AParticleFX>()).b_shell_only() };
                let viewport_width = unsafe { ptr::addr_of!((*viewport).size_x).read_unaligned() };
                let viewport_height = unsafe { ptr::addr_of!((*viewport).size_y).read_unaligned() };
                *result = DynamicSetup {
                    sprite: matches!(draw_type, DRAW_SPRITE | DRAW_SPRITE_ANIM_ONCE),
                    mesh: draw_type == DRAW_MESH,
                    particles: draw_type == DRAW_PARTICLES,
                    show_actor_icons: icons,
                    is_ortho: matches!(rend_map, 13 | 14 | 15),
                    is_realtime: flags & (SHOW_REAL_TIME | SHOW_PLAYER_CTRL) != 0,
                    texture_present: !texture.is_null(),
                    texture_width: if texture.is_null() { 0 } else { unsafe { ptr::addr_of!((*texture).base.u_size).read_unaligned() } },
                    texture_height: if texture.is_null() { 0 } else { unsafe { ptr::addr_of!((*texture).base.v_size).read_unaligned() } },
                    shell_only,
                    viewport_width,
                    viewport_height,
                    location: unsafe { ptr::addr_of!((*actor).location).read_unaligned() },
                    sprite_proj_forward: unsafe { ptr::addr_of!((*actor).sprite_proj_forward).read_unaligned() },
                    draw_scale: if icons { 1.0 } else { unsafe { ptr::addr_of!((*actor).draw_scale).read_unaligned() } },
                };
                Ok(())
            }
            Op::ProjectSprite { actor, visible, projection } => {
                let actor = actor.cast::<AActor>();
                if actor.is_null() { return self.fail("Null actor in dynamic sprite projection"); }
                let viewport = unsafe { ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>() };
                if viewport.is_null() { return self.fail("Dynamic sprite frame has no viewport"); }
                let view = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned().cast::<APlayerPawn>() };
                if view.is_null() { return self.fail("Dynamic sprite viewport has no actor"); }
                let screen = hp2_render::softln::ScreenFrame {
                    rend_map: unsafe { ptr::addr_of!((*view).rend_map).read_unaligned() },
                    coords: unsafe { ptr::addr_of!((*frame).coords).read_unaligned() },
                    zoom: unsafe { ptr::addr_of!((*frame).zoom).read_unaligned() },
                    fx2: unsafe { ptr::addr_of!((*frame).fx2).read_unaligned() },
                    fy2: unsafe { ptr::addr_of!((*frame).fy2).read_unaligned() },
                    proj_z: unsafe { ptr::addr_of!((*frame).proj).read_unaligned().z },
                };
                let location = unsafe { ptr::addr_of!((*actor).location).read_unaligned() };
                let projected = hp2_render::softln::project(&screen, location);
                *visible = projected.visible;
                *projection = hp2_render::dynamic_items::DynamicProjection {
                    screen_x: projected.screen_x, screen_y: projected.screen_y, perspective: projected.scale,
                };
                Ok(())
            }
            Op::BoxAxes { actor, axes } => {
                let actor = actor.cast::<AActor>();
                if actor.is_null() { return self.fail("Null actor in dynamic bounding-box query"); }
                let coords = self.render_bounds(actor)?;
                *axes = coords.x_axis;
                *self.bounds.borrow_mut() = coords;
                Ok(())
            }
            Op::ProjectBox { visible, bounds } => self.project_box(bounds, visible),
            Op::DynamicSlot { index, back, slot } => {
                let model = self.model()?;
                let nodes = unsafe { ptr::addr_of!((*model).nodes).read_unaligned() };
                if index < 0 || index >= nodes.array_num || nodes.data.is_null() || !(0..=1).contains(&back) {
                    return self.fail("Canonical dynamic item cache index is invalid");
                }
                let cache = self.graph.with_dynamics_cache(|items| {
                    if index >= items.len() as i32 { None } else { Some(unsafe { items.as_mut_ptr().cast::<hp2_abi::render_scene::FDynamicsCache>().add(index as usize) }) }
                });
                let Some(cache) = cache else { return self.fail("Canonical dynamic item cache is not initialized"); };
                *slot = unsafe { ptr::addr_of_mut!((*cache).dynamics[back as usize]) };
                Ok(())
            }
            Op::AllocateRaster { start_y, end_y, raster } => {
                let line_count = end_y.checked_sub(start_y).filter(|count| *count >= 0)
                    .ok_or_else(|| { self.record(RuntimeError::new("Invalid dynamic raster line range")); () })? as usize;
                let bytes = std::mem::size_of::<FRasterPolyHeader>()
                    .checked_add(line_count.checked_mul(std::mem::size_of::<hp2_abi::render_span::FRasterSpan>()).ok_or_else(|| { self.record(RuntimeError::new("Dynamic raster size overflow")); () })?)
                    .ok_or_else(|| { self.record(RuntimeError::new("Dynamic raster size overflow")); () })?;
                let size = i32::try_from(bytes).map_err(|_| { self.record(RuntimeError::new("Dynamic raster exceeds memory stack limit")); () })?;
                let memory = self.remember(self.graph.dynamic_memory())?;
                *raster = unsafe { memory.push_bytes(size, std::mem::align_of::<FRasterPolyHeader>() as i32) }
                    .map_err(|error| { self.record(error.with_context("dynamic raster allocation")); () })?.cast();
                Ok(())
            }
            Op::BspSplit { index, split } => {
                let node = unsafe { &*self.node(index)? };
                *split = DynamicBspSplit {
                    plane: node.plane,
                    front: node.i_front,
                    back: node.i_back,
                    csg: node.num_vertices > 0 && node.node_flags & 0x21 == 0,
                };
                Ok(())
            }
            Op::InitializeSpan { source_span: _, raster, destination, span } => {
                if raster.is_null() { return self.fail("Invalid dynamic span initialization arguments"); }
                let dynamic = if destination.is_some() {
                    Some(self.remember(self.graph.dynamic_memory())?)
                } else {
                    None
                };
                let engine = self.access.owner_context::<crate::core_owner::EngineState>();
                let fallback;
                let memory: &hp2_core::mem_owner::MemStackOwner = match &dynamic {
                    Some(memory) => memory,
                    None => {
                        let engine = engine.ok_or_else(|| { self.record(RuntimeError::new("Canonical dynamic span has no Engine owner")); () })?;
                        if self.span_mark.borrow().is_some() { return self.fail("Dynamic span mark is already active"); }
                        let mark = engine.memory.mark();
                        *self.span_mark.borrow_mut() = Some(mark);
                        fallback = &engine.memory;
                        fallback
                    }
                };
                let allocated = unsafe { memory.push_bytes(std::mem::size_of::<FSpanBuffer>() as i32, std::mem::align_of::<FSpanBuffer>() as i32) }
                    .map_err(|error| { self.record(error.with_context("dynamic span allocation")); () })?.cast::<FSpanBuffer>();
                unsafe { allocated.write(std::mem::zeroed()); }
                if let Some(field) = destination { unsafe { field.write(allocated); } }
                let start = unsafe { ptr::addr_of!((*raster).start_y).read_unaligned() };
                let end = unsafe { ptr::addr_of!((*raster).end_y).read_unaligned() };
                let stack = memory.as_ptr().cast::<c_void>();
                let alloc = DynamicSpanAlloc { stacks: &self.stacks, error: &self.error };
                unsafe { hp2_render::span::alloc_index(&alloc, &mut *allocated, start, end, stack) }
                    .map_err(|_| { self.record(RuntimeError::new("Canonical dynamic span index allocation failed")); () })?;
                *span = allocated;
                Ok(())
            }
            Op::InsertActorLink { actor, head, dynamic } => {
                if head.is_null() { return self.fail("Null dynamic actor-link head"); }
                let memory = if dynamic {
                    self.remember(self.graph.dynamic_memory())?
                } else {
                    let engine = self.access.owner_context::<crate::core_owner::EngineState>()
                        .ok_or_else(|| { self.record(RuntimeError::new("Canonical dynamic link has no Engine owner")); () })?;
                    unsafe { head.write_unaligned(self.allocate_link(FActorLink { actor, next: head.read_unaligned() }, &engine.memory)?); }
                    return Ok(());
                };
                unsafe { head.write_unaligned(self.allocate_link(FActorLink { actor, next: head.read_unaligned() }, &memory)?); }
                Ok(())
            }
            Op::LightBsp { index, actor, result } => {
                let node = unsafe { *self.node(index)? };
                let actor = actor.cast::<AActor>();
                if actor.is_null() { return self.fail("Null actor in dynamic light BSP query"); }
                *result = DynamicLightInfo {
                    plane: node.plane,
                    front: node.i_front,
                    back: node.i_back,
                    front_leaf: node.i_leaf[1],
                    back_leaf: node.i_leaf[0],
                    special_lit: unsafe { (*actor).b_special_lit() },
                    location: unsafe { ptr::addr_of!((*actor).location).read_unaligned() },
                    light_radius: crate::actor::world_light_radius(unsafe { &*actor }),
                    volumetric_radius: crate::actor::world_volumetric_radius(unsafe { &*actor }),
                };
                Ok(())
            }
            Op::LeafLightStorage { leaf, slot, indices, count } => {
                *slot = self.leaf_slot(leaf)?;
                *indices = self.graph.dyn_light_leaves_ptr();
                *count = self.graph.dynamic_light_leaves.as_ptr();
                Ok(())
            }
            Op::SurfaceLightStorage { index, result, slot, indices, count } => {
                let node = unsafe { *self.node(index)? };
                let model = self.model()?;
                let surfs = unsafe { ptr::addr_of!((*model).surfs).read_unaligned() };
                if node.i_surf < 0 || node.i_surf >= surfs.array_num || surfs.data.is_null() {
                    return self.fail("Canonical dynamic surface index is invalid");
                }
                let surf = unsafe { &*surfs.data.cast::<FBspSurf>().add(node.i_surf as usize) };
                *result = DynamicSurfaceInfo {
                    surface: node.i_surf, plane: node.i_plane, has_light_map: surf.i_light_map >= 0,
                    special_lit: surf.poly_flags & PF_SPECIAL_LIT != 0, max_dynamic_lights: self.graph.max_surf_lights(),
                };
                *slot = self.surf_slot(node.i_surf)?;
                *indices = self.graph.dyn_light_surfs_ptr();
                *count = self.graph.dynamic_light_surfs.as_ptr();
                Ok(())
            }
            Op::InsertVolumetricLink { actor, head, volumetric } => {
                if head.is_null() { return self.fail("Null dynamic volumetric-link head"); }
                let actor_record = actor.cast::<AActor>();
                if actor_record.is_null() { return self.fail("Null actor in dynamic volumetric link"); }
                let location = unsafe { ptr::addr_of!((*actor_record).location).read_unaligned() };
                let memory = self.remember(self.graph.dynamic_memory())?;
                let link = self.allocate_link(FVolActorLink { location, actor, next: unsafe { head.read_unaligned() }, volumetric: i32::from(volumetric) }, &memory)?;
                unsafe { head.write_unaligned(link); }
                Ok(())
            }
            Op::Statistics { index, value } => {
                let stats = unsafe { &mut *self.graph.stats_ptr() };
                match index {
                    0 => stats.num_sprites = stats.num_sprites.wrapping_add(value),
                    1 => stats.num_chunks = stats.num_chunks.wrapping_add(value),
                    2 => stats.num_final_chunks = stats.num_final_chunks.wrapping_add(value),
                    3 => stats.num_moving_lights = stats.num_moving_lights.wrapping_add(value),
                    4 => stats.span_pix = stats.span_pix.wrapping_add(value),
                    5 => stats.chunks_drawn = stats.chunks_drawn.wrapping_add(value),
                    _ => {}
                }
                Ok(())
            }
            Op::CopyFromRaster { destination, source, raster, drawn } => {
                if destination.is_null() || source.is_null() || raster.is_null() { return self.fail("Invalid dynamic raster copy arguments"); }
                let start = unsafe { ptr::addr_of!((*raster).start_y).read_unaligned() };
                let end = unsafe { ptr::addr_of!((*raster).end_y).read_unaligned() };
                let alloc = DynamicSpanAlloc { stacks: &self.stacks, error: &self.error };
                *drawn = unsafe { hp2_render::span::copy_from_raster(&alloc, &mut *destination, &*source, start, end, raster.add(1).cast()) }
                    .map_err(|_| { self.record(RuntimeError::new("Canonical dynamic raster span copy failed")); () })? != 0;
                Ok(())
            }
            Op::ReleaseSpan { span } => {
                if span.is_null() { return self.fail("Null dynamic span release buffer"); }
                let alloc = DynamicSpanAlloc { stacks: &self.stacks, error: &self.error };
                unsafe { hp2_render::span::release(&alloc, &mut *span); }
                Ok(())
            }
            Op::MergeSpan { destination, source } => {
                if destination.is_null() || source.is_null() { return self.fail("Invalid dynamic span merge buffers"); }
                let alloc = DynamicSpanAlloc { stacks: &self.stacks, error: &self.error };
                unsafe { hp2_render::span::merge_with(&alloc, &mut *destination, &*source) }
                    .map_err(|_| { self.record(RuntimeError::new("Canonical dynamic span merge failed")); () })?;
                Ok(())
            }
            Op::PopSpanMark => {
                let Some(mark) = self.span_mark.borrow_mut().take() else { return self.fail("Dynamic span-mark cleanup has no active mark"); };
                let engine = self.access.owner_context::<crate::core_owner::EngineState>()
                    .ok_or_else(|| { self.record(RuntimeError::new("Canonical dynamic span has no Engine owner")); () })?;
                unsafe { engine.memory.pop(&mark); }
                Ok(())
            }
        }
    }
}

struct DynamicSpanAlloc<'a, 'b> {
    stacks: &'a CoreSceneStackOps<'b>,
    error: &'a RefCell<Option<RuntimeError>>,
}

impl SpanAlloc for DynamicSpanAlloc<'_, '_> {
    unsafe fn push(&self, stack: *mut c_void, size: usize, alignment: usize) -> Result<*mut u8, i32> {
        unsafe { hp2_render::scene_pipeline::helper_frames::SceneStackOps::push(self.stacks, ptr::null_mut(), stack, size, alignment as i32, 0) }
    }

    unsafe fn mark_save(&self, stack: *mut c_void, mark: *mut FMemMark) {
        unsafe { hp2_render::scene_pipeline::helper_frames::SceneStackOps::mark_save(self.stacks, stack, mark) }
    }

    unsafe fn mark_pop(&self, mark: *mut FMemMark) {
        unsafe { hp2_render::scene_pipeline::helper_frames::SceneStackOps::mark_pop(self.stacks, mark) }
    }

    fn warn(&self, text: &str) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(RuntimeError::new("Dynamic span diagnostic").with_context(text));
        }
    }
}

fn finish<T>(result: Result<T, ()>, ops: &CoreDynamicOps<'_>, message: &'static str) -> RuntimeResult<T> {
    ops.discard_span_mark();
    let stack_error = ops.stacks.take_error();
    let error = ops.error.borrow_mut().take().or(stack_error);
    match result {
        Ok(value) => match error {
            Some(error) => Err(error),
            None => Ok(value),
        },
        Err(()) => Err(error.unwrap_or_else(|| RuntimeError::new(message))),
    }
}

/// Computes callback-free render bounds for an actor using canonical Core data.
pub(super) fn render_bounds_coords(
    access: CoreAccess<'_>,
    actor: *mut AActor,
) -> RuntimeResult<hp2_abi::FCoords> {
    let ops = CoreDynamicOps::new_bounds(access)?;
    ops.render_bounds(actor).map_err(|_| {
        ops.error
            .borrow_mut()
            .take()
            .unwrap_or_else(|| RuntimeError::new("Canonical actor render bounds failed"))
    })
}

/// Validates the active renderer context before calculating actor render bounds.
///
/// # Safety
/// `render`, `frame`, and `actor` must remain live for the operation.
pub(super) unsafe fn render_bounds_with_ops(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
    actor: *mut AActor,
) -> RuntimeResult<hp2_abi::FCoords> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical actor bounds require a live renderer and frame"));
    }
    render_bounds_coords(access, actor)
}

/// Initializes the active dynamic-render graph for source `URender::SetupDynamics`.
///
/// # Safety
/// `render` and `frame` must be live records in the canonical active scene graph.

pub(super) unsafe fn setup_dynamics(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
    exclude: *mut AActor,
) -> RuntimeResult<()> {
    let mut ops = CoreDynamicOps::new(access, render, frame, ptr::null_mut())?;
    let model = ops.model().map_err(|_| {
        ops.error
            .borrow_mut()
            .take()
            .unwrap_or_else(|| RuntimeError::new("Canonical dynamic setup failed"))
    })?;
    let node_count = unsafe { ptr::addr_of!((*model).nodes.array_num).read_unaligned() };
    if node_count < 0 {
        return Err(RuntimeError::new("Negative canonical dynamic BSP node count"));
    }

    let viewport = unsafe { ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>() };
    if viewport.is_null() {
        return Err(RuntimeError::new("Canonical dynamic scene frame has no viewport"));
    }
    let view = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned().cast::<APlayerPawn>() };
    if view.is_null() {
        return Err(RuntimeError::new("Canonical dynamic viewport has no actor"));
    }
    let flags = unsafe { ptr::addr_of!((*view).show_flags).read_unaligned() };
    if !hp2_render::sprite::dynamics_enabled(node_count, flags & 0x8 != 0) {
        return Ok(());
    }
    let frame_device = unsafe {
        ptr::addr_of!((*viewport).ren_dev)
            .read_unaligned()
            .cast::<hp2_abi::engine::generated::URenderDevice>()
    };
    let high_detail = if frame_device.is_null() {
        false
    } else {
        unsafe { (*frame_device).high_detail_actors() }
    };
    let shadow_trace_pass = crate::live_trace_owner::shadow_admission_begin_pass(access);
    let allocated = ops.graph.with_dynamics_cache(|cache| {
        let count = node_count as usize;
        if count > cache.len() && cache.try_reserve(count - cache.len()).is_err() {
            return false;
        }
        cache.resize(
            count,
            hp2_abi::render_scene::FDynamicsCache {
                dynamics: [ptr::null_mut(); 2],
            },
        );
        true
    });
    if !allocated {
        return Err(RuntimeError::new(
            "Canonical dynamic item cache allocation failed",
        ));
    }

    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
        unsafe { dynamic_stat_clock(&mut (*ops.graph.stats_ptr()).filter_time); }
    }
    let level = unsafe { ptr::addr_of!((*frame).level).read_unaligned().cast::<ULevel>() };
    if level.is_null() {
        return Err(RuntimeError::new("Canonical dynamic scene frame has no level"));
    }
    let actors = unsafe { ptr::addr_of!((*level).base.actors).read_unaligned() };
    if actors.array_num < 0 || (actors.array_num != 0 && actors.data.is_null()) {
        return Err(RuntimeError::new("Canonical dynamic actor array is invalid"));
    }
    let editor = access.process_mode().is_editor;
    let player_pawn_class = access.native_class("Engine.PlayerPawn")?;
    let brush_class = access.native_class("Engine.Brush")?;
    let mut index = 0;
    while index < actors.array_num {
        let actor = unsafe { actors.data.cast::<*mut AActor>().add(index as usize).read_unaligned() };
        let outer_admitted = if actor.is_null() {
            false
        } else {
            let high_detail_actor = reflected_actor_bool(access, actor, "bHighDetail")?;
            hp2_render::sprite::dynamics_outer_admitted(
                true,
                high_detail_actor,
                high_detail,
                unsafe { (*frame.cast::<hp2_abi::render_scene::FSceneNode>()).recursion },
                unsafe { (*view).base.b_behind_view() },
                unsafe { (*view).view_target.cast::<AActor>() == actor },
            )
        };
        if outer_admitted {
            let operation_error = RefCell::new(None);
            let mut operation = |op| {
                if op == 8 {
                    return match reflected_actor_bool(access, actor, "bOwnerNoSee") {
                        Ok(value) => Ok(value),
                        Err(error) => {
                            *operation_error.borrow_mut() = Some(error);
                            Err(())
                        }
                    };
                }
                let actor_ref = unsafe { &*actor };
                Ok(match op {
                    0 => actor == exclude,
                    1 => if editor { actor_ref.b_hidden_ed() } else { actor_ref.b_hidden() },
                    2 => {
                        let is_player = unsafe {
                            hp2_core::object_model::is_a(view.cast(), player_pawn_class.cast())
                        };
                        (editor && flags & SHOW_PLAYER_CTRL == 0)
                            || (is_player && crate::actor::player_pawn_is_actor_visible())
                    }
                    3 => actor_ref.visibility_radius == 0.0 || {
                        let delta = hp2_core::math::vector_sub(
                            actor_ref.location,
                            unsafe { (*frame).coords.origin },
                        );
                        hp2_core::ansi_math::sqrt(hp2_core::math::vector_size_sq_2d(delta) as f64)
                            < actor_ref.visibility_radius as f64
                    },
                    4 => actor_ref.visibility_height == 0.0
                        || (actor_ref.location.z - unsafe { (*frame).coords.origin.z }).abs()
                            < actor_ref.visibility_height,
                    5 => actor_ref.b_only_owner_see(),
                    6 => crate::actor::is_owned_by(actor, view.cast()) != 0,
                    7 => unsafe { (*view).base.b_behind_view() },
                    _ => false,
                })
            };
            let inner_result = hp2_render::sprite::dynamics_inner_admitted(&mut operation);

            let inner_admitted = inner_result.map_err(|_| {
                operation_error
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| RuntimeError::new("Canonical dynamic actor admission failed"))
            })?;
            if inner_admitted {
                let moving_brush = unsafe {
                    crate::actor::is_moving_brush_with(actor, |value, class| {
                        debug_assert_eq!(class, crate::actor::class::ABRUSH);
                        Ok(hp2_core::object_model::is_a(value, brush_class.cast()))
                    })
                }
                .map_err(|_| RuntimeError::new("Canonical moving-brush class query failed"))?;
                if moving_brush {
                    if !unsafe { ptr::addr_of!((*level).brush_tracker).read_unaligned() }.is_null() {
                        unsafe { crate::dynbsp_owner::update(access, level, actor)?; }
                    }
                } else {
                    create_dynamic_actor(&mut ops, frame, actor)?;
                    update_dynamic_shadow(&mut ops, frame, actor, shadow_trace_pass)?;
                }
            }

            let actor_ref = unsafe { &*actor };
            let mut max_radius = 0.0;
            let light_admitted = hp2_render::sprite::dynamics_light_admitted(|op| {
                let plane_admits = |index: usize, radius: f32| {
                    let plane = unsafe { (*frame).view_planes[index] };
                    !(hp2_core::math::plane_dot(plane, actor_ref.location) < -radius)
                };
                Ok(match op {
                    0 => actor_ref.light_type != 0,
                    1 => !(actor_ref.b_static() || actor_ref.b_no_delete()) || actor_ref.b_dynamic_light(),
                    2 => actor_ref.light_brightness != 0,
                    3 => actor_ref.light_radius != 0,
                    4 => {
                        max_radius = crate::actor::world_light_radius(actor_ref)
                            .max(crate::actor::world_volumetric_radius(actor_ref));
                        true
                    },
                    5..=8 => plane_admits((op - 5) as usize, max_radius),
                    9 => {
                        let zone = actor_ref.region.zone.cast::<hp2_abi::engine::generated::AZoneInfo>();
                        !zone.is_null() && unsafe { (*zone).b_fog_zone() }
                            && actor_ref.volume_radius != 0 && actor_ref.volume_brightness != 0
                    }
                    10..=13 => plane_admits(
                        (op - 10) as usize,
                        crate::actor::world_volumetric_radius(actor_ref),
                    ),
                    _ => false,
                })
            })
            .map_err(|_| RuntimeError::new("Canonical dynamic light admission failed"))?;
            if let Some(volumetric) = light_admitted {
                let result = unsafe {
                    dynamic_items::light_with_ops(&mut ops, frame.cast(), 0, actor.cast(), volumetric, false)
                };
                finish(result, &ops, "Canonical dynamic light creation failed")?;
                if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
                    unsafe {
                        let stats = &mut *ops.graph.stats_ptr();
                        stats.dyn_light_actors = stats.dyn_light_actors.wrapping_add(1);
                    }
                }
            }
        }
        index += 1;
    }
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
        unsafe { dynamic_stat_unclock(&mut (*ops.graph.stats_ptr()).filter_time); }
    }
    Ok(())
}
fn create_dynamic_actor(
    ops: &mut CoreDynamicOps<'_>,
    frame: *mut FSceneNode,
    actor: *mut AActor,
) -> RuntimeResult<()> {
    let access = ops.access;
    let iterator_class = reflected_actor_class(access, actor, "RenderIteratorClass")?;
    let mut iterator = unsafe { ptr::addr_of!((*actor).render_interface).read_unaligned() };
    if iterator_class.is_null() {
        if !iterator.is_null() {
            unsafe {
                access.conditional_destroy(iterator.cast())?;
                access.delete_object(iterator.cast())?;
                ptr::addr_of_mut!((*actor).render_interface).write_unaligned(ptr::null_mut());
            }
        }
        let result = unsafe {
            dynamic_items::create_with_ops(
                ops,
                frame.cast(),
                0,
                actor.cast(),
                dynamic_items::SPRITE,
                ptr::null_mut(),
            )
        };
        finish(result, ops, "Canonical dynamic sprite creation failed")?;
        return Ok(());
    }

    if !iterator.is_null() && !is_valid_core_object(access, iterator.cast()) {
        iterator = ptr::null_mut();
    }
    if iterator.is_null() {
        let defaults = unsafe { access.default_object(iterator_class)? };
        iterator = unsafe {
            access.construct_object(iterator_class, actor.cast(), 0, 0, defaults)?.cast()
        };
        unsafe { ptr::addr_of_mut!((*actor).render_interface).write_unaligned(iterator) };
    }


    use crate::render_iterator::Hp2RenderIteratorOperation as IteratorOp;
    let actor_number = unsafe {
        ptr::addr_of!((*ops.graph.stats_ptr()).total_eari_actors).read_unaligned()
    };
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 && (0..64).contains(&actor_number) {
        unsafe {
            let stats = &mut *ops.graph.stats_ptr();
            stats.eari_actor_names[actor_number as usize] =
                ptr::addr_of!((*actor.cast::<hp2_abi::engine::generated::UObject>()).name)
                    .read_unaligned();
            stats.eari_actor_tags[actor_number as usize] =
                ptr::addr_of!((*actor).tag).read_unaligned();
        }
    }
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
        unsafe {
            let stats = &mut *ops.graph.stats_ptr();
            dynamic_stat_clock(&mut stats.total_eari_time);
            if (0..64).contains(&actor_number) {
                dynamic_stat_clock(&mut stats.eari_time[actor_number as usize]);
            }
        }
    }
    render_iterator_op(iterator, IteratorOp::Init, frame)?;
    let iteration = (|| {
        let filter_by_volume = reflected_actor_bool(access, actor, "bFilterByVolume")?;
        let system = if filter_by_volume {
            let result = unsafe {
                dynamic_items::create_with_ops(
                    ops,
                    frame.cast(),
                    0,
                    actor.cast(),
                    dynamic_items::PARENT,
                    ptr::null_mut(),
                )
            };
            finish(result, ops, "Canonical dynamic iterator parent creation failed")?
        } else {
            ptr::null_mut()
        };
        render_iterator_op(iterator, IteratorOp::First, frame)?;
        loop {
            if render_iterator_op(iterator, IteratorOp::IsDone, frame)?.value != 0 {
                break;
            }
            let display = render_iterator_op(iterator, IteratorOp::CurrentItem, frame)?.pointer;
            if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
                unsafe {
                    let stats = &mut *ops.graph.stats_ptr();
                    dynamic_stat_clock(&mut stats.eari_actor_draw_time);
                    if !filter_by_volume && (0..64).contains(&actor_number) {
                        dynamic_stat_clock(&mut stats.eari_draw_time[actor_number as usize]);
                    }
                    stats.total_eari_sub_actors = stats.total_eari_sub_actors.wrapping_add(1);
                    if (0..64).contains(&actor_number) {
                        let count = &mut stats.eari_sub_actors[actor_number as usize];
                        *count = count.wrapping_add(1);
                    }
                }
            }
            let result = unsafe {
                dynamic_items::create_with_ops(
                    ops,
                    frame.cast(),
                    0,
                    display,
                    if filter_by_volume {
                        dynamic_items::CHILD
                    } else {
                        dynamic_items::SPRITE
                    },
                    system,
                )
            };
            finish(result, ops, "Canonical render iterator visit failed")?;
            if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
                unsafe {
                    let stats = &mut *ops.graph.stats_ptr();
                    if !filter_by_volume && (0..64).contains(&actor_number) {
                        dynamic_stat_unclock(&mut stats.eari_draw_time[actor_number as usize]);
                    }
                    dynamic_stat_unclock(&mut stats.eari_actor_draw_time);

                }
            }
            render_iterator_op(iterator, IteratorOp::Next, frame)?;
        }
        Ok(())
    })();
    let uninit = render_iterator_op(iterator, IteratorOp::UnInit, frame).map(|_| ());
    iteration?;
    uninit?;
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
        unsafe {
            let stats = &mut *ops.graph.stats_ptr();
            dynamic_stat_unclock(&mut stats.total_eari_time);
            if (0..64).contains(&actor_number) {
                dynamic_stat_unclock(&mut stats.eari_time[actor_number as usize]);
            }
        }
    }
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
        unsafe {
            let stats = &mut *ops.graph.stats_ptr();
            stats.total_eari_actors = stats.total_eari_actors.wrapping_add(1);
        }
    }
    Ok(())
}
fn dynamic_stat_clock(value: &mut i32) {
    *value = value.wrapping_sub(hp2_core::unix_clock::cycles() as i32);
}

fn dynamic_stat_unclock(value: &mut i32) {
    *value = value.wrapping_add(hp2_core::unix_clock::cycles().wrapping_sub(34) as i32);
}

fn is_valid_core_object(
    access: CoreAccess<'_>,
    object: *mut hp2_abi::engine::generated::UObject,
) -> bool {
    if object.is_null() {
        return false;
    }
    let index = unsafe { ptr::addr_of!((*object).index).read_unaligned() };
    index >= 0 && index < access.object_count() && access.object_at(index) == object
}

fn update_dynamic_shadow(
    ops: &mut CoreDynamicOps<'_>,
    frame: *mut FSceneNode,
    actor: *mut AActor,
    shadow_trace_pass: i32,
) -> RuntimeResult<()> {
    let access = ops.access;
    let viewport = unsafe { ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>() };
    let view = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned().cast::<APlayerPawn>() };
    let perspective = !matches!(unsafe { (*view).rend_map }, 13 | 14 | 15);
    let shadow =
        unsafe { ptr::addr_of!((*actor).shadow).read_unaligned().cast::<c_void>() };
    crate::live_trace_owner::first_renderer_candidate(access, actor, shadow);
    let mut projection = hp2_render::sprite::DynamicShadowProjection {
        min: FVector { x: 0.0, y: 0.0, z: 0.0 },
        max: FVector { x: 0.0, y: 0.0, z: 0.0 },
        behind_projection: 0,
    };
    let outcome = hp2_render::sprite::dynamics_shadow_admission(|query| {
        Ok(match query {
            0 => !shadow.is_null(),
            1 => perspective,
            2 => {
                let actor_ref = unsafe { &*actor };
                projection = hp2_render::sprite::dynamics_shadow_projection(
                    actor_ref.location,
                    actor_ref.collision_radius,
                    unsafe { (*frame).coords.origin },
                    unsafe { (*frame).coords.z_axis },
                );
                let bounds = crate::prim::Aabb {
                    min: projection.min,
                    max: projection.max,
                    valid: true,
                };
                *ops.bounds.borrow_mut() =
                    crate::coords::box_times_coords(&bounds, &hp2_core::math::unit_coords());
                projection.behind_projection != 0
            }
            3 => {
                let mut bounds = DynamicBounds {
                    min_x: 0.0,
                    max_x: 0.0,
                    min_y: 0.0,
                    max_y: 0.0,
                    min_z: 0.0,
                    max_z: 0.0,
                };
                let mut visible = false;
                ops.project_box(&mut bounds, &mut visible).map_err(|_| ())?;
                visible
            }
            _ => false,
        })
    })
    .map_err(|_| ops.error.borrow_mut().take().unwrap_or_else(|| RuntimeError::new("Canonical dynamic shadow admission failed")))?;
    hp2_render::sprite::dynamics_shadow_dispatch(outcome as i32, |action| {
        let candidate_status = match action {
            hp2_render::sprite::DynamicShadowAdmission::MissingShadow => c"missing_shadow".as_ptr(),
            hp2_render::sprite::DynamicShadowAdmission::RejectedOrtho => c"rejected_ortho".as_ptr(),
            hp2_render::sprite::DynamicShadowAdmission::RejectedBehindProjection => {
                c"rejected_behind_projection".as_ptr()
            }
            hp2_render::sprite::DynamicShadowAdmission::RejectedBounds => c"rejected_bounds".as_ptr(),
            hp2_render::sprite::DynamicShadowAdmission::Admitted => c"admitted".as_ptr(),
        };
        let scene = frame.cast::<hp2_abi::render_scene::FSceneNode>();
        let parent = unsafe { ptr::addr_of!((*scene).parent).read_unaligned() };
        let recursion_parent_actor = if parent.is_null() {
            ptr::null_mut()
        } else {
            let parent_viewport =
                unsafe { ptr::addr_of!((*parent).viewport).read_unaligned().cast::<UViewport>() };
            if parent_viewport.is_null() {
                ptr::null_mut()
            } else {
                unsafe { ptr::addr_of!((*parent_viewport).actor).read_unaligned().cast::<AActor>() }
            }
        };
        crate::live_trace_owner::shadow_admission_record(
            access,
            shadow_trace_pass,
            actor,
            shadow,
            candidate_status,
            action == hp2_render::sprite::DynamicShadowAdmission::Admitted,
            view.cast(),
            unsafe { ptr::addr_of!((*view).view_target).read_unaligned().cast::<AActor>() },
            unsafe { (*view).base.b_behind_view() },
            recursion_parent_actor,
            perspective,
            true,
        );
        if action != hp2_render::sprite::DynamicShadowAdmission::Admitted {
            return Ok(());
        }
        let event_name = ops.remember(access.intern_str("Update", hp2_core::name_owner::FindName::Find))?;
        let event = ops.remember(unsafe {
            access.find_object_function_by_name(shadow.cast(), event_name, false)
        })?;
        if event.is_null() {
            ops.record(RuntimeError::new("Canonical dynamic shadow has no Update event"));
            return Err(());
        }
        let mut level_actor = ptr::null_mut::<AActor>();
        ops.remember(unsafe {
            crate::actor_owner::process_event(
                access,
                shadow.cast::<AActor>(),
                event,
                ptr::addr_of_mut!(level_actor).cast(),
                ptr::null_mut(),
            )
        })
    })
    .map_err(|_| ops.error.borrow_mut().take().unwrap_or_else(|| RuntimeError::new("Canonical dynamic shadow dispatch failed")))
}

fn reflected_actor_object(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    property_name: &str,
) -> RuntimeResult<*mut c_void> {
    let object_class = unsafe {
        ptr::addr_of!((*actor.cast::<hp2_abi::engine::generated::UObject>()).class)
            .read_unaligned()
            .cast::<hp2_abi::engine::generated::UClass>()
    };
    let name = access.intern_str(property_name, hp2_core::name_owner::FindName::Find)?;
    let property_class = access.native_class("ObjectProperty")?;
    let property = unsafe { access.find_field(object_class.cast(), name, property_class) };
    if property.is_null() {
        return Ok(ptr::null_mut());
    }
    let offset = unsafe {
        ptr::addr_of!((*property.cast::<hp2_abi::engine::generated::UProperty>()).offset)
            .read_unaligned()
    };
    if offset < 0 {
        return Err(RuntimeError::new(
            "Canonical dynamic actor object property has a negative offset",
        ));
    }
    Ok(unsafe {
        actor
            .cast::<u8>()
            .add(offset as usize)
            .cast::<*mut c_void>()
            .read_unaligned()
    })
}


fn reflected_actor_class(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    property_name: &str,
) -> RuntimeResult<*mut hp2_abi::engine::generated::UClass> {
    if actor.is_null() {
        return Ok(ptr::null_mut());
    }
    let object_class = unsafe {
        ptr::addr_of!((*actor.cast::<hp2_abi::engine::generated::UObject>()).class)
            .read_unaligned()
            .cast::<hp2_abi::engine::generated::UClass>()
    };
    // SAFETY: `object_class` is the live class of a live actor.
    let Some(slot) = (unsafe { access.property_slot(object_class, property_name, "ClassProperty")? }) else {
        return Ok(ptr::null_mut());
    };
    let offset = slot.offset;
    if offset < 0 {
        return Err(RuntimeError::new(
            "Canonical dynamic actor class property has a negative offset",
        ));
    }
    Ok(unsafe {
        actor
            .cast::<u8>()
            .add(offset as usize)
            .cast::<*mut hp2_abi::engine::generated::UClass>()
            .read_unaligned()
    })
}


fn reflected_actor_bool(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    property_name: &str,
) -> RuntimeResult<bool> {
    if actor.is_null() {
        return Ok(false);
    }
    let object_class = unsafe {
        ptr::addr_of!((*actor.cast::<hp2_abi::engine::generated::UObject>()).class)
            .read_unaligned()
            .cast::<hp2_abi::engine::generated::UClass>()
    };
    // SAFETY: `object_class` is the live class of a live actor.
    let Some(slot) = (unsafe { access.property_slot(object_class, property_name, "BoolProperty")? }) else {
        return Ok(false);
    };
    if slot.offset < 0 {
        return Err(RuntimeError::new("Canonical dynamic actor bool property has a negative offset"));
    }
    let bits = unsafe { actor.cast::<u8>().add(slot.offset as usize).cast::<u32>().read_unaligned() };
    Ok(bits & slot.bit_mask != 0)
}

/// Filters one dynamic item against a BSP node using typed Core source operations.
///
/// # Safety
/// `render`, `frame`, and `item` must be live in the active canonical scene graph.
pub(super) unsafe fn filter(
    access: CoreAccess<'_>, render: *mut URender, frame: *mut FSceneNode,
    item: *mut FDynamicItem, node_index: i32, outside: i32,
) -> RuntimeResult<()> {
    let mut ops = CoreDynamicOps::new(access, render, frame, ptr::null_mut())?;
    let result = unsafe { dynamic_items::dispatch_filter_with_ops(&mut ops, frame.cast(), node_index, outside, item) };
    finish(result, &ops, "Canonical dynamic item filtering failed")
}

/// Clips a dynamic item against the active span and selected volumetric lights.
///
/// # Safety
/// The renderer, frame, item, span, and both list bases must belong to the active scene.
pub(super) unsafe fn pre_render(
    access: CoreAccess<'_>, render: *mut URender, frame: *mut FSceneNode,
    item: *mut FDynamicItem, span: *mut FSpanBuffer,
    first_volumetric: *mut *mut FVolActorLink, leaf_lights_base: *mut *mut FVolActorLink,
) -> RuntimeResult<()> {
    if first_volumetric.is_null() { return Err(RuntimeError::new("Null first volumetric link slot")); }
    let mut ops = CoreDynamicOps::new(access, render, frame, leaf_lights_base)?;
    let head = unsafe { first_volumetric.read_unaligned() };
    let result = unsafe { dynamic_items::prerender_with_ops(&mut ops, frame.cast(), item, span, head) };
    finish(result, &ops, "Canonical dynamic item prerender failed")
}

/// Creates and draws one canonical standalone dynamic sprite for `actor`.
///
/// # Safety
/// The actor and frame must be live in the active graph.
pub(super) unsafe fn draw_actor(
    access: CoreAccess<'_>, render: *mut URender, frame: *mut FSceneNode, actor: *mut AActor,
) -> RuntimeResult<()> {
    if actor.is_null() { return Err(RuntimeError::new("Null Actor in dynamic draw")); }
    let token = dynamic_items::begin();
    let result = (|| {
        let mut ops = CoreDynamicOps::new(access, render, frame, ptr::null_mut())?;
        let result = unsafe { dynamic_items::create_with_ops(&mut ops, frame.cast(), 0, actor.cast(), STANDALONE, ptr::null_mut()) };
        let sprite = finish(result, &ops, "Canonical standalone dynamic sprite creation failed")?;
        if !sprite.is_null() {
            unsafe { super::sprite::draw_actor_sprite(access, render, frame, sprite)?; }
        }
        Ok(())
    })();
    let ended = dynamic_items::end(token);
    match result {
        Err(error) => Err(error),
        Ok(()) if ended => Ok(()),
        Ok(()) => Err(RuntimeError::new("Canonical standalone dynamic scope did not close")),
    }
}

#[cfg(test)]
mod render_host_contracts {
    use super::*;
    use hp2_abi::engine::generated::{UBoolProperty, UProperty};
    use hp2_core::name_owner::FindName;

    #[test]
    fn high_detail_actor_admission_preserves_the_reflected_high_bit() {
        let owner = crate::render_owner::render_test_support::owner("actor-flags");
        let access = owner.access();
        let actor = crate::render_owner::render_test_support::construct(access, "Engine.Light")
            .cast::<AActor>();
        let class = unsafe { (*actor.cast::<hp2_abi::engine::generated::UObject>()).class };
        let name = access.intern_str("bHighDetail", FindName::Find).unwrap();
        let property = unsafe {
            access.find_field(class.cast(), name, access.native_class("BoolProperty").unwrap())
        }.cast::<UBoolProperty>();
        assert!(!property.is_null());
        let mask = unsafe { ptr::addr_of!((*property).bit_mask).read_unaligned() };
        assert!(mask > u32::from(u8::MAX), "exercise the native flag above bit seven");
        let address = unsafe { access.property_address(actor.cast(), property.cast::<UProperty>()) }
            .unwrap().cast::<u32>();
        let original = unsafe { address.read_unaligned() };
        unsafe { address.write_unaligned(original | mask); }
        let high_detail = reflected_actor_bool(access, actor, "bHighDetail").unwrap();
        assert!(!hp2_render::sprite::dynamics_outer_admitted(true, high_detail, false, 0, true, false));
        assert!(hp2_render::sprite::dynamics_outer_admitted(true, high_detail, true, 0, true, false));
        unsafe { address.write_unaligned(original & !mask); }
        assert!(hp2_render::sprite::dynamics_outer_admitted(
            true, reflected_actor_bool(access, actor, "bHighDetail").unwrap(), false, 0, true, false,
        ));
        owner.shutdown().unwrap();
    }

    #[test]
    fn bsp_solid_planes_admit_front_chunks_independently_of_occlusion_hints() {
        use hp2_render::sprite_filter::{self, Action, FTransform, FRasterSpan, Source};
        let owner = crate::render_owner::render_test_support::owner("dynamic-bsp-csg");
        let access = owner.access();
        let graph = crate::render_owner::state(access).unwrap();
        graph.initialize_dynamic_memory().unwrap();
        graph.initialize_scene_memory().unwrap();
        graph.initialize_vector_memory().unwrap();
        let mut node: hp2_abi::render_scene::FBspNode = unsafe { std::mem::zeroed() };
        node.plane = hp2_abi::FPlane { x: 0.0, y: 0.0, z: 1.0, w: 0.0 };
        node.i_front = -1;
        node.i_back = -1;
        let mut model: UModel = unsafe { std::mem::zeroed() };
        model.nodes.data = ptr::addr_of_mut!(node).cast();
        model.nodes.array_num = 1;
        model.nodes.array_max = 1;
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        level.model = ptr::addr_of_mut!(model).cast();
        let mut frame: FSceneNode = unsafe { std::mem::zeroed() };
        frame.level = ptr::addr_of_mut!(level).cast();
        let mut ops = CoreDynamicOps::new_bounds(access).unwrap();
        ops.frame = ptr::addr_of_mut!(frame);
        let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let verts = corners.map(|(x, y)| {
            let mut vertex: FTransform = unsafe { std::mem::zeroed() };
            vertex.point = FVector { x, y, z: 1.0 };
            vertex
        });
        let lines = [FRasterSpan { x: [0, 1] }];
        let source = Source { start_y: 0, end_y: 1, lines: &lines };
        for (vertices, flags, expected) in [
            (4, 0x00, Action::Final),
            (4, 0x08, Action::Final), // NF_PolyOccluded is not a solidity flag.
            (4, 0x01, Action::None),  // NF_NotCsg.
            (4, 0x20, Action::None),  // NF_IsNew.
            (0, 0x08, Action::None),  // Empty planes cannot split solid space.
        ] {
            node.num_vertices = vertices;
            node.node_flags = flags;
            let mut split: DynamicBspSplit = unsafe { std::mem::zeroed() };
            unsafe {
                ops.operation(
                    ptr::addr_of_mut!(frame).cast(),
                    DynamicOperation::BspSplit { index: 0, split: &mut split },
                ).unwrap();
            }
            let terminal = sprite_filter::filter_chunk(
                &verts, &split.plane, &source, None, split.front, split.back, false, split.csg,
            );
            assert_eq!(terminal.front_action, expected, "vertices={vertices}, flags={flags:#04x}");
        }
        drop(ops);
        graph.exit_vector_memory().unwrap();
        graph.exit_scene_memory().unwrap();
        graph.exit_dynamic_memory().unwrap();
        owner.shutdown().unwrap();
    }
}
