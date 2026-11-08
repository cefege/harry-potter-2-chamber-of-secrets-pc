//! Scene traversal and frame orchestration over the pinned native scene layouts.
//! Native calls are fallible leaves. No reference into an engine array is held
//! across one of those calls; headers and linked-list successors are reread.
#![allow(unsafe_op_in_unsafe_fn)]
use hp2_abi::engine::generated::FMemStack;
use hp2_abi::render_particle::FTransform;
use hp2_abi::render_scene::*;
use hp2_abi::render_span::FSpanBuffer;
use hp2_abi::{FPlane, FVector};
use hp2_core::mem::{self, Hp2MemServices};
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{self, null_mut};

/// Existing core memory owner plus the native exception-history leaf.
#[repr(C)]
pub struct Hp2SceneStackServices {
    pub memory: *const Hp2MemServices,
    pub slow_checks: i32,
    /// Failure-only history: 0 PushBytes, 1 New<T>, 2 New1, 3 New2.
    pub allocation_failure: unsafe extern "C" fn(*mut c_void, i32, u32),
}

unsafe fn allocate_stack(
    services: &Hp2SceneStackServices,
    context: *mut c_void,
    stack: *mut c_void,
    bytes: usize,
    alignment: i32,
    kind: u32,
) -> Result<*mut u8, i32> {
    let size = match mem::checked_int_size(&*services.memory, bytes, 3) {
        Ok(size) => size,
        Err(status) => {
            (services.allocation_failure)(context, status, kind);
            return Err(status);
        }
    };
    push_stack(services, context, stack, size, alignment, kind)
}

unsafe fn push_stack(
    services: &Hp2SceneStackServices,
    context: *mut c_void,
    stack: *mut c_void,
    size: i32,
    alignment: i32,
    kind: u32,
) -> Result<*mut u8, i32> {
    let result = mem::push_bytes(
        &mut *stack.cast::<FMemStack>(),
        size,
        alignment,
        services.slow_checks != 0,
        c"UnMem.h",
        &*services.memory,
    );
    if let Err(status) = result {
        (services.allocation_failure)(context, status, kind);
    }
    result
}

#[repr(C)]
pub struct Hp2ScenePipelineArgs {
    pub integers: [i32; 8],
    pub values: [f32; 8],
    pub pointers: [*mut c_void; 8],
}
impl Default for Hp2ScenePipelineArgs {
    fn default() -> Self {
        Self {
            integers: [0; 8],
            values: [0.0; 8],
            pointers: [null_mut(); 8],
        }
    }
}
#[repr(C)]
pub struct Hp2ScenePipelineData {
    pub mem_stack: *mut c_void,
    pub dyn_mem_stack: *mut c_void,
    pub frame: *mut c_void,
    pub nodes: *mut c_void,
    pub surfs: *mut c_void,
    pub verts: *mut c_void,
    pub points: *mut c_void,
    pub leaves: *mut c_void,
    pub zones: *mut c_void,
    pub dynamics: *mut c_void,
    pub first_volumetric: *mut c_void,
    pub surf_lights: *mut c_void,
    pub leaf_lights: *mut c_void,
    pub dyn_light_surfs: *mut c_void,
    pub dyn_light_leaves: *mut c_void,
    pub num_dyn_light_surfs: *mut i32,
    pub num_dyn_light_leaves: *mut i32,
    pub max_surf_lights: *mut i32,
    pub max_leaf_lights: *mut i32,
    pub stamp: *mut u32,
    pub nodes_draw: *mut u32,
    pub polys_draw: *mut u32,
    pub actor: *mut c_void,
    pub view_target: *mut c_void,
    pub capabilities: u32,
    pub root_outside: i32,
    pub toggle: i32,
    pub frame_stamp: i32,
    pub behind_view: i32,
    pub extra_poly_flags: u32,
    pub time_seconds: f32,
    pub lumos_radius: f32,
    pub player_location: [f32; 3],
}
#[repr(C)]
pub struct Hp2ScenePipelineState {
    pub context: *mut c_void,
    pub invoke: unsafe extern "C" fn(*mut c_void, u32, *mut Hp2ScenePipelineArgs) -> i32,
    pub memory: Hp2SceneStackServices,
    pub data: Hp2ScenePipelineData,
}
const INV: u32 = 1;
const TRANSLUCENT: u32 = 4;
const NO_OCCLUDE: u32 = 0x47;
const BACKDROP: u32 = 0x80;
const TWO_SIDED: u32 = 0x100;
const NO_MERGE: u32 = 0x10000;
const PORTAL: u32 = 0x04000000;
const MIRROR: u32 = 0x08000000;
const OCCLUDE: u32 = 0x80000000;
const LUMOS: u32 = 0x00800040;
const LUMOS_ON: u32 = 0x00040000;
const EDITOR: u32 = 1;
const PLAYER_CTRL: u32 = 2;
const VOLUMETRIC: u32 = 4;
const FOG_MAPS: u32 = 8;
const FOG_ZONE: u32 = 16;
const SPAN: u32 = 32;
const SHINY: u32 = 64;
const BRUSH: u32 = 128;
const ZONES_VIEW: u32 = 256;
const PLAYER: u32 = 512;
const LUMOS_ACTIVE: u32 = 1024;
const LEGEND: u32 = 2048;
const LUMOS_ENABLED: u32 = 4096;

// The hardware draw for an unactivated disappearing surface is opaque and
// depth-writing. Do not let its marker's PF_Modulated bit disable BSP span
// occlusion or force the surface into the translucent bin.
fn occlusion_surface_flags(surface_flags: u32, effective_flags: u32, capabilities: u32) -> u32 {
    const APPEAR: u32 = 0x00200000;
    const MODULATED: u32 = 0x40;
    const OTHER_UNSAFE: u32 = (NO_OCCLUDE & !MODULATED) | 0x10000000 | BACKDROP | PORTAL | MIRROR;
    if capabilities & (PLAYER | LUMOS_ENABLED | EDITOR | SPAN) == PLAYER | LUMOS_ENABLED
        && surface_flags & LUMOS == LUMOS
        && (surface_flags | effective_flags) & (LUMOS_ON | APPEAR) == 0
        && (surface_flags | effective_flags) & OTHER_UNSAFE == 0
    {
        effective_flags & !MODULATED
    } else {
        effective_flags
    }
}

/// Source SDK operations used by both Native and canonical Core scene traversal.
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum OcclusionOperation {
    Initialize = 0,
    InitializeDrawList = 1,
    ViewZone = 3,
    WarpInfo = 4,
    CreateChild = 5,
    InitializeSpan = 6,
    Clip = 7,
    Raster = 8,
    AllocateSpan = 9,
    CopySpan = 10,
    ReleaseSpan = 11,
    MergeSpan = 12,
    IsDynamic = 13,
    BoundVisible = 14,
    Filter = 15,
    PreRender = 16,
    LeafLighting = 17,
    SurfaceVisible = 18,
    ZoneActor = 19,
    TextureInfo = 20,
    SkyInfo = 21,
    Generate = 22,
    Link = 24,
    Radius = 25,
    ObjectIndex = 26,
    Statistics = 27,
    AllocateLights = 29,
    SetupDynamics = 30,
    OccludeBsp = 31,
    ActorLeaf = 32,
    OccludeFrame = 33,
    AllocateDynamics = 34,
    SizeApprox = 35,
    Snapshot = 36,
}

/// Fallible provider leaves; traversal never calls Native services directly.
pub trait OcclusionOps {
    /// # Safety
    /// The data and operation-specific packet pointers remain live for this call.
    unsafe fn call(
        &self,
        operation: OcclusionOperation,
        data: *mut Hp2ScenePipelineData,
        packet: &mut Hp2ScenePipelineArgs,
    ) -> Result<(), ()>;

    /// # Safety
    /// `stack` belongs to this provider and its enclosing frame mark is active.
    unsafe fn push(
        &self,
        stack: *mut c_void,
        bytes: usize,
        alignment: i32,
        kind: u32,
    ) -> Result<*mut u8, ()>;

    /// Preserves the source checked-size expression before allocation.
    unsafe fn checked_size(&self, bytes: usize, kind: u32) -> Result<usize, ()>;
}

struct OcclusionContext<'a> {
    data: *mut Hp2ScenePipelineData,
    ops: &'a dyn OcclusionOps,
}

struct NativeOcclusionOps {
    state: *mut Hp2ScenePipelineState,
}

impl OcclusionOps for NativeOcclusionOps {
    unsafe fn call(
        &self,
        operation: OcclusionOperation,
        _data: *mut Hp2ScenePipelineData,
        packet: &mut Hp2ScenePipelineArgs,
    ) -> Result<(), ()> {
        let state = &*self.state;
        if (state.invoke)(state.context, operation as u32, packet) == 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    unsafe fn push(
        &self,
        stack: *mut c_void,
        bytes: usize,
        alignment: i32,
        kind: u32,
    ) -> Result<*mut u8, ()> {
        let state = &*self.state;
        allocate_stack(&state.memory, state.context, stack, bytes, alignment, kind)
            .map_err(|_| ())
    }

    unsafe fn checked_size(&self, bytes: usize, kind: u32) -> Result<usize, ()> {
        let state = &*self.state;
        match mem::checked_int_size(&*state.memory.memory, bytes, 3) {
            Ok(size) => Ok(size as usize),
            Err(status) => {
                (state.memory.allocation_failure)(state.context, status, kind);
                Err(())
            }
        }
    }
}

unsafe fn call(
    s: *mut OcclusionContext<'_>,
    op: OcclusionOperation,
    a: &mut Hp2ScenePipelineArgs,
) -> Result<(), ()> {
    (*s).ops.call(op, (*s).data, a)
}
unsafe fn array<T>(header: *mut c_void) -> *mut T {
    (*(header as *mut TArrayRaw)).data.cast()
}
unsafe fn count(header: *mut c_void) -> i32 {
    (*(header as *mut TArrayRaw)).array_num
}
unsafe fn node(s: *mut OcclusionContext<'_>, i: i32) -> *mut FBspNode {
    array::<FBspNode>((*(*s).data).nodes).add(i as usize)
}
unsafe fn surf(s: *mut OcclusionContext<'_>, i: i32) -> *mut FBspSurf {
    array::<FBspSurf>((*(*s).data).surfs).add(i as usize)
}
fn dot(p: FPlane, v: FVector) -> f32 {
    hp2_core::math::vector_dot(
        FVector {
            x: p.x,
            y: p.y,
            z: p.z,
        },
        v,
    )
}
fn plane_dot(p: FPlane, v: FVector) -> f32 {
    hp2_core::math::plane_dot(p, v)
}
fn vector_dot(a: FVector, b: FVector) -> f32 {
    hp2_core::math::vector_dot(a, b)
}
fn cross(a: FVector, b: FVector) -> FVector {
    hp2_core::math::vector_cross(a, b)
}
fn outside(n: FBspNode, side: usize, was: i32) -> i32 {
    let csg = n.num_vertices > 0 && n.node_flags & 0x21 == 0;
    if side != 0 {
        (was != 0 || csg) as i32
    } else {
        (was != 0 && !csg) as i32
    }
}
fn child(n: FBspNode, side: usize) -> i32 {
    if side == 0 { n.i_back } else { n.i_front }
}
unsafe fn stat(s: *mut OcclusionContext<'_>, event: i32, value: i32) -> Result<(), ()> {
    let mut a = Hp2ScenePipelineArgs::default();
    a.integers[0] = event;
    a.integers[1] = value;
    call(s, OcclusionOperation::Statistics, &mut a)
}
unsafe fn is_dynamic(s: *mut OcclusionContext<'_>, index: i32) -> Result<bool, ()> {
    if (*(*s).data).capabilities & BRUSH == 0 {
        return Ok(false);
    }
    let mut a = Hp2ScenePipelineArgs::default();
    a.integers[0] = index;
    call(s, OcclusionOperation::IsDynamic, &mut a)?;
    Ok(a.integers[0] != 0)
}
unsafe fn alloc_draw(
    s: *mut OcclusionContext<'_>,
    temporary: bool,
) -> Result<*mut FBspDrawList, ()> {
    let p = (*s).ops.push(
        if temporary {
            (*(*s).data).mem_stack
        } else {
            (*(*s).data).dyn_mem_stack
        },
        size_of::<FBspDrawList>(),
        crate::span::DEFAULT_ALIGNMENT as i32,
        if temporary { 2 } else { 1 },
    )?
    .cast::<FBspDrawList>();
    if temporary {
        let mut a = Hp2ScenePipelineArgs::default();
        a.pointers[0] = p.cast();
        call(s, OcclusionOperation::InitializeDrawList, &mut a)?;
    }
    Ok(p)
}
unsafe fn zone_actor(s: *mut OcclusionContext<'_>, zone: i32) -> Result<*mut c_void, ()> {
    let mut a = Hp2ScenePipelineArgs::default();
    a.integers[0] = zone;
    call(s, OcclusionOperation::ZoneActor, &mut a)?;
    Ok(a.pointers[0])
}
unsafe fn release(s: *mut OcclusionContext<'_>, span: *mut FSpanBuffer) -> Result<(), ()> {
    let mut a = Hp2ScenePipelineArgs::default();
    a.pointers[0] = span.cast();
    call(s, OcclusionOperation::ReleaseSpan, &mut a)
}
unsafe fn merge_span(
    s: *mut OcclusionContext<'_>,
    dst: *mut FSpanBuffer,
    src: *mut FSpanBuffer,
    copy: bool,
) -> Result<(), ()> {
    let mut a = Hp2ScenePipelineArgs::default();
    a.pointers[0] = dst.cast();
    a.pointers[1] = src.cast();
    a.integers[0] = copy as i32;
    call(s, OcclusionOperation::MergeSpan, &mut a)
}
unsafe fn leaf_dynamics(
    s: *mut OcclusionContext<'_>,
    index: i32,
    side: usize,
    was: i32,
    zones: *mut FSpanBuffer,
    mask: u8,
    volumetric: bool,
) -> Result<(), ()> {
    let mut n = *node(s, index);
    if volumetric && n.i_leaf[side] != -1 {
        let mut a = Hp2ScenePipelineArgs::default();
        a.integers[0] = n.i_leaf[side];
        call(s, OcclusionOperation::LeafLighting, &mut a)?;
        n = *node(s, index);
    }
    let span = zones.add((n.i_zone[side] & mask) as usize);
    if (*span).valid_lines != 0 && (outside(n, side, was) != 0 || (*(*s).data).toggle != 0) {
        let mut item =
            (*array::<FDynamicsCache>((*(*s).data).dynamics).add(index as usize)).dynamics[1 - side];
        while !item.is_null() {
            let mut a = Hp2ScenePipelineArgs::default();
            a.pointers[0] = item.cast();
            a.pointers[1] = span.cast();
            a.integers[0] = index;
            call(s, OcclusionOperation::PreRender, &mut a)?;
            item = (*item).filter_next;
        }
    }
    Ok(())
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NodeStack {
    node: i32,
    far: i32,
    far_outside: i32,
    outside: i32,
    pass: i32,
    next: *mut NodeStack,
}
unsafe fn stack(s: *mut OcclusionContext<'_>, next: *mut NodeStack) -> Result<*mut NodeStack, ()> {
    let p = (*s).ops.push(
        (*(*s).data).mem_stack,
        size_of::<NodeStack>(),
        crate::span::DEFAULT_ALIGNMENT as i32,
        1,
    )?
    .cast::<NodeStack>();
    (*p).next = next;
    Ok(p)
}
unsafe fn add_child(
    s: *mut OcclusionContext<'_>,
    kind: i32,
    span: *mut FSpanBuffer,
    object: *mut c_void,
    index: i32,
    zone: i32,
    front: usize,
    bounds: &mut FScreenBounds,
) -> Result<(), ()> {
    let mut a = Hp2ScenePipelineArgs::default();
    a.integers[0] = kind;
    a.integers[1] = index;
    a.integers[2] = zone;
    a.integers[3] = front as i32;
    a.pointers[0] = span.cast();
    a.pointers[1] = object;
    a.pointers[2] = if (*(*s).data).toggle != 0 {
        null_mut()
    } else {
        bounds as *mut _ as *mut c_void
    };
    a.values[0] = if front != 0 { -1.0 } else { 1.0 };
    call(s, OcclusionOperation::CreateChild, &mut a)
}
unsafe fn volume_visible(
    s: *mut OcclusionContext<'_>,
    link: *mut FVolActorLink,
    edges: &[FVector],
) -> Result<bool, ()> {
    for edge in edges {
        let location = (*link).location;
        let mut a = Hp2ScenePipelineArgs::default();
        a.pointers[0] = (*link).actor;
        call(s, OcclusionOperation::Radius, &mut a)?;
        if vector_dot(location, *edge) > a.values[0] {
            return Ok(false);
        }
    }
    Ok(true)
}
unsafe fn link_volume(
    s: *mut OcclusionContext<'_>,
    draw: *mut FBspDrawList,
    actor: *mut c_void,
) -> Result<(), ()> {
    let mut a = Hp2ScenePipelineArgs::default();
    a.pointers[0] = actor;
    a.pointers[1] = (*draw).volumetrics.cast();
    a.pointers[2] = (*s).ops.push(
        (*(*s).data).dyn_mem_stack,
        size_of::<FActorLink>(),
        crate::span::DEFAULT_ALIGNMENT as i32,
        2,
    )?
    .cast();
    call(s, OcclusionOperation::Link, &mut a)?;
    (*draw).volumetrics = a.pointers[0].cast();
    Ok(())
}
unsafe fn save_poly(
    s: *mut OcclusionContext<'_>,
    draw: *mut FBspDrawList,
    index: i32,
    pts: *mut *mut FTransform,
    num: i32,
    next: *mut FSavedPoly,
) -> Result<(), ()> {
    let bytes = size_of::<FSavedPoly>()
        .wrapping_add((num as usize).wrapping_mul(size_of::<*mut FTransform>()));
    // The outer checked-size expression precedes New<BYTE>'s own guard.
    let bytes = (*s).ops.checked_size(bytes, 0)?;
    let saved = (*s).ops.push(
        (*(*s).data).dyn_mem_stack,
        bytes,
        crate::span::DEFAULT_ALIGNMENT as i32,
        1,
    )?
    .cast::<FSavedPoly>();
    (*saved).next = next;
    (*saved).i_node = index;
    (*saved).num_pts = num;
    ptr::copy_nonoverlapping(
        pts,
        ptr::addr_of_mut!((*saved).pts).cast::<*mut FTransform>(),
        num as usize,
    );
    (*draw).polys = saved;
    Ok(())
}

/// The engine owns every pointer reachable from `s` for this synchronous call.
/// A zero result means a native leaf failed; the native guard rethrows its first exception.
pub unsafe fn occlude_bsp(s: *mut Hp2ScenePipelineState) -> i32 {
    let ops = NativeOcclusionOps { state: s };
    occlude_bsp_with(ptr::addr_of_mut!((*s).data), &ops).is_ok() as i32
}

/// Runs the source BSP traversal with the caller's typed provider.
///
/// # Safety
/// Every pointer in `data` stays live for the synchronous traversal.
pub unsafe fn occlude_bsp_with(
    data: *mut Hp2ScenePipelineData,
    ops: &dyn OcclusionOps,
) -> Result<(), ()> {
    let mut context = OcclusionContext { data, ops };
    occlude_bsp_inner(&mut context)
}

unsafe fn occlude_bsp_inner(s: *mut OcclusionContext<'_>) -> Result<(), ()> {
    if count((*(*s).data).nodes) == 0 {
        return Ok(());
    }
    stat(s, 0, 0)?;
    let mut a = Hp2ScenePipelineArgs::default();
    a.pointers[0] = (*s).data.cast();
    call(s, OcclusionOperation::Snapshot, &mut a)?;
    // Iterator initialization (not traversal) selects the first editor sky actor.
    let mut editor_sky = null_mut();
    if (*(*s).data).capabilities & EDITOR != 0 {
        call(s, OcclusionOperation::SkyInfo, &mut a)?;
        editor_sky = a.pointers[0];
    }
    call(s, OcclusionOperation::AllocateDynamics, &mut a)?;
    *(*(*s).data).stamp = (*(*(*s).data).stamp).wrapping_add(1);
    let frame = (*(*s).data).frame.cast::<FSceneNode>();
    let mut temporary = alloc_draw(s, true)?;
    let list_count = count((*(*s).data).surfs);
    let all = (*s).ops.push(
        (*(*s).data).mem_stack,
        (list_count as usize).wrapping_mul(size_of::<*mut FBspDrawList>()),
        crate::span::DEFAULT_ALIGNMENT as i32,
        3,
    )?
    .cast::<*mut FBspDrawList>();
    ptr::write_bytes(
        all.cast::<u8>(),
        0,
        ((list_count as usize).wrapping_mul(size_of::<*mut FBspDrawList>()) as i32) as usize,
    );
    let origin = (*frame).coords.origin;
    let mut view_zone = (*frame).zone_number;
    if (*(*s).data).capabilities & PLAYER != 0 {
        a.integers[0] = view_zone;
        call(s, OcclusionOperation::ViewZone, &mut a)?;
        view_zone = a.integers[0];
    }
    view_zone = view_zone as u8 as i32;
    let mask = if view_zone != 0 { 255 } else { 0 };
    let mut active = [0u8; 64];
    active[0] = view_zone as u8;
    let mut active_count = 1usize;
    let mut active_mask = 1u64 << (view_zone as u32);
    let volumetric = (*(*s).data).capabilities & (VOLUMETRIC | FOG_MAPS | FOG_ZONE)
        == (VOLUMETRIC | FOG_MAPS | FOG_ZONE);
    let poly_mask = if (*(*s).data).capabilities & PLAYER_CTRL != 0 {
        !0
    } else {
        !INV
    };
    *(*(*s).data).first_volumetric.cast::<*mut FVolActorLink>() = null_mut();
    call(s, OcclusionOperation::Initialize, &mut a)?;
    (*((*(*s).data).zones.cast::<FZoneProperties>().add(view_zone as usize))).last_render_time =
        (*(*s).data).time_seconds;
    a.pointers[0] = (*((*(*s).data).zones.cast::<FZoneProperties>().add(view_zone as usize))).zone_actor;
    call(s, OcclusionOperation::WarpInfo, &mut a)?;
    if a.integers[0] != 0 && !a.pointers[1].is_null() && !a.pointers[2].is_null() {
        let mut bounds = FScreenBounds::default();
        add_child(s, 0, (*frame).span, a.pointers[0], -1, 0, 0, &mut bounds)?;
        stat(s, 1, 0)?;
        return Ok(());
    }
    let mut zones: [FSpanBuffer; 64] = std::mem::zeroed();
    for zone in &mut zones {
        a.pointers[0] = zone as *mut _ as *mut c_void;
        call(s, OcclusionOperation::InitializeSpan, &mut a)?;
    }
    zones[view_zone as usize] = ptr::read((*frame).span);
    let mut top = stack(s, null_mut())?;
    let mut index = 0;
    let mut was = (*(*s).data).root_outside;
    let mut pass = 0;
    if (*(*s).data).capabilities & (LUMOS_ENABLED | LUMOS_ACTIVE | EDITOR) == LUMOS_ENABLED | LUMOS_ACTIVE {
        for i in 0..count((*(*s).data).nodes) {
            let n = *node(s, i);
            if n.zone_mask & (1u64 << (view_zone as u32)) == 0 {
                continue;
            }
            let flags = (*surf(s, n.i_surf)).poly_flags | (*(*s).data).extra_poly_flags;
            if flags & LUMOS != 0 && flags & LUMOS_ON == 0 {
                for j in 0..n.num_vertices as usize {
                    let vertex =
                        (*array::<FVert>((*(*s).data).verts).add(n.i_vert_pool as usize + j)).p_vertex;
                    let point = *array::<FVector>((*(*s).data).points).add(vertex as usize);
                    a.values[0] = point.x - (*(*s).data).player_location[0];
                    a.values[1] = point.y - (*(*s).data).player_location[1];
                    a.values[2] = point.z - (*(*s).data).player_location[2];
                    call(s, OcclusionOperation::SizeApprox, &mut a)?;
                    if a.values[0] < (*(*s).data).lumos_radius {
                        (*surf(s, n.i_surf)).poly_flags |= LUMOS_ON;
                        break;
                    }
                }
            }
        }
    }
    'traverse: loop {
        let mut n = *node(s, index);
        let mut pop = false;
        if pass == 0 {
            if view_zone != 0 && n.zone_mask & active_mask == 0 && !is_dynamic(s, n.i_surf)? {
                stat(s, 2, 0)?;
                pop = true;
            }
            if !pop {
                let flags = (*surf(s, n.i_surf)).poly_flags | (*(*s).data).extra_poly_flags;
                let reject_bounds = (*(*s).data).capabilities & LEGEND == 0 || flags & 0x00200000 == 0;
                if reject_bounds
                    && n.i_render_bound != -1
                    && !is_dynamic(s, n.i_surf)?
                    && (n.node_flags & 0x10 != 0 || (index ^ (*(*s).data).frame_stamp) & 15 == 0)
                {
                    (*node(s, index)).node_flags &= !0x10;
                    let mut bounds = FScreenBounds::default();
                    a.integers[0] = n.i_render_bound;
                    a.pointers[0] = if view_zone != 0 {
                        null_mut()
                    } else {
                        zones.as_mut_ptr().cast()
                    };
                    a.pointers[1] = &mut bounds as *mut _ as *mut c_void;
                    call(s, OcclusionOperation::BoundVisible, &mut a)?;
                    if a.integers[0] == 0 {
                        (*node(s, index)).node_flags |= 0x10;
                        pop = true;
                    } else if view_zone != 0 {
                        let mut visible = false;
                        for &z in &active[..active_count] {
                            if (*node(s, index)).zone_mask & (1u64 << z) != 0
                                && crate::span::box_is_visible(
                                    &zones[z as usize],
                                    bounds.min_x as i32,
                                    bounds.min_y as i32,
                                    bounds.max_x as i32,
                                    bounds.max_y as i32,
                                ) != 0
                            {
                                visible = true;
                                break;
                            }
                        }
                        if !visible {
                            stat(s, 3, 0)?;
                            (*node(s, index)).node_flags |= 0x10;
                            pop = true;
                        }
                    }
                }
            }
            if !pop {
                stat(s, 4, 0)?;
                let mut item =
                    (*array::<FDynamicsCache>((*(*s).data).dynamics).add(index as usize)).dynamics[0];
                while !item.is_null() {
                    a.pointers[0] = item.cast();
                    a.integers[0] = index;
                    a.integers[1] = was;
                    call(s, OcclusionOperation::Filter, &mut a)?;
                    item = (*item).filter_next;
                }
                stat(s, 5, 0)?;
                n = *node(s, index);
                let front = (plane_dot(n.plane, origin) > 0.0) as usize;
                (*top).far = child(n, 1 - front);
                (*top).far_outside = outside(n, 1 - front, was);
                if child(n, front) != -1 {
                    (*top).node = index;
                    (*top).outside = was;
                    (*top).pass = 1;
                    top = stack(s, top)?;
                    index = child(n, front);
                    was = outside(n, front, was);
                    pass = 0;
                    continue;
                }
                pass = 1;
            }
        }
        if !pop && pass == 1 {
            n = *node(s, index);
            if view_zone != 0 && n.zone_mask & active_mask == 0 && !is_dynamic(s, n.i_surf)? {
                stat(s, 2, 0)?;
                pop = true;
            }
            if !pop {
                let original = index;
                let front = (plane_dot(n.plane, origin) > 0.0) as usize;
                leaf_dynamics(s, index, front, was, zones.as_mut_ptr(), mask, volumetric)?;
                let sign = if front != 0 { 1.0 } else { -1.0 };
                let plane = (*node(s, index)).plane;
                if (*frame)
                    .view_sides
                    .iter()
                    .all(|v| sign * dot(plane, *v) > 0.0)
                {
                    pop = true;
                }
                if !pop {
                    loop {
                        n = *node(s, index);
                        let distance = plane_dot(n.plane, origin);
                        let front = (distance > 0.0) as usize;
                        let mut flags = (*surf(s, n.i_surf)).poly_flags | (*(*s).data).extra_poly_flags;
                        'polygon: {
                            if front == 0 && distance < -1.0 && flags & (TWO_SIDED | PORTAL) == 0 {
                                break 'polygon;
                            }
                            if flags & PORTAL != 0 && view_zone == 0 {
                                break 'polygon;
                            }
                            if (*(*s).data).capabilities & PLAYER != 0 {
                                a.integers[0] = index;
                                a.integers[1] = front as i32;
                                call(s, OcclusionOperation::SurfaceVisible, &mut a)?;
                                if a.integers[0] == 0 {
                                    break 'polygon;
                                }
                            }
                            n = *node(s, index);
                            let mut zone = n.i_zone[front] & mask;
                            if (*(*s).data).capabilities & LEGEND != 0 && flags & 0x10 != 0 {
                                zone = view_zone as u8;
                            }
                            let span = &mut zones[zone as usize] as *mut FSpanBuffer;
                            if (*span).valid_lines <= 0 {
                                break 'polygon;
                            }
                            a.integers[0] = index;
                            call(s, OcclusionOperation::Clip, &mut a)?;
                            let num = a.integers[0];
                            let pts = a.pointers[0].cast::<*mut FTransform>();
                            if num == 0 {
                                break 'polygon;
                            }
                            if (front == 0 && flags & (TWO_SIDED | PORTAL) != 0)
                                ^ ((*frame).mirror == -1.0)
                            {
                                for i in 0..num / 2 {
                                    ptr::swap(pts.add(i as usize), pts.add((num - i - 1) as usize));
                                }
                            }
                            a.pointers[0] = pts.cast();
                            a.pointers[1] = if (*node(s, index)).node_flags & 8 != 0 {
                                span.cast()
                            } else {
                                null_mut()
                            };
                            a.integers[0] = num;
                            call(s, OcclusionOperation::Raster, &mut a)?;
                            if a.integers[0] == 0 {
                                break 'polygon;
                            }
                            let mut bounds = FScreenBounds {
                                min_y: a.integers[1] as f32,
                                max_y: a.integers[2] as f32,
                                min_x: a.integers[3] as f32,
                                max_x: a.integers[4] as f32,
                                ..Default::default()
                            };
                            let texture = (*surf(s, n.i_surf)).texture;
                            if !texture.is_null() {
                                a.pointers[0] = texture;
                                call(s, OcclusionOperation::TextureInfo, &mut a)?;
                                flags |= a.integers[0] as u32;
                            }
                            flags &= poly_mask;
                            let occlusion_flags = occlusion_surface_flags(
                                (*surf(s, n.i_surf)).poly_flags,
                                flags,
                                (*(*s).data).capabilities,
                            );
                            let mut merge = null_mut::<FBspDrawList>();
                            if occlusion_flags & (NO_OCCLUDE | NO_MERGE | PORTAL) == 0 {
                                let actor = zone_actor(s, n.i_zone[front] as i32)?;
                                merge = *all.add(n.i_surf as usize);
                                while !merge.is_null() && (*merge).zone != actor {
                                    merge = (*merge).surf_next;
                                }
                            }
                            let temp_span = ptr::addr_of_mut!((*temporary).span);
                            a.pointers[0] = temp_span.cast();
                            a.integers[0] =
                                (!merge.is_null() || (*(*s).data).capabilities & SPAN == 0) as i32;
                            call(s, OcclusionOperation::AllocateSpan, &mut a)?;
                            a.pointers[0] = temp_span.cast();
                            a.pointers[1] = span.cast();
                            a.integers[0] = (occlusion_flags & NO_OCCLUDE == 0
                                || flags & (PORTAL | INV) == PORTAL | INV
                                || flags & MIRROR != 0)
                                as i32;
                            call(s, OcclusionOperation::CopySpan, &mut a)?;
                            if a.integers[0] == 0 {
                                (*node(s, index)).node_flags |= 8;
                                release(s, temp_span)?;
                                break 'polygon;
                            }
                            let mut bin = 1 + (occlusion_flags & NO_OCCLUDE != 0) as usize;
                            let mut draw = false;
                            let mut sky = null_mut();
                            if flags & BACKDROP != 0 {
                                if (*(*s).data).capabilities & EDITOR != 0 {
                                    sky = editor_sky;
                                } else {
                                    a.integers[0] = zone as i32;
                                    a.integers[1] = 1;
                                    call(s, OcclusionOperation::SkyInfo, &mut a)?;
                                    sky = a.pointers[0];
                                }
                            }
                            if flags & BACKDROP != 0
                                && !sky.is_null()
                                && (*frame).recursion < 3
                                && (*(*s).data).capabilities & PLAYER_CTRL != 0
                            {
                                add_child(
                                    s,
                                    1,
                                    temp_span,
                                    sky,
                                    index,
                                    zone as i32,
                                    front,
                                    &mut bounds,
                                )?;
                            } else if flags & MIRROR != 0
                                && (*frame).recursion < 3
                                && (*(*s).data).capabilities & PLAYER_CTRL != 0
                            {
                                if flags & TRANSLUCENT != 0 && (*(*s).data).capabilities & SHINY == 0 {
                                    flags = (flags & !TRANSLUCENT) | OCCLUDE;
                                    bin = 1;
                                    draw = true;
                                } else {
                                    add_child(
                                        s,
                                        2,
                                        temp_span,
                                        null_mut(),
                                        index,
                                        zone as i32,
                                        front,
                                        &mut bounds,
                                    )?;
                                    bin = 0;
                                    if flags & NO_OCCLUDE == 0 {
                                        if (*(*s).data).capabilities & SPAN != 0 {
                                            break 'polygon;
                                        }
                                        flags |= INV;
                                    }
                                    flags |= OCCLUDE;
                                    draw = true;
                                }
                            } else if flags & PORTAL != 0
                                && ((*(*s).data).capabilities & ZONES_VIEW == 0 || flags & NO_OCCLUDE != 0)
                            {
                                let render = flags & INV == 0;
                                let opposite = (*node(s, index)).i_zone[1 - front] & mask;
                                if opposite != 0 && flags & NO_OCCLUDE != 0 {
                                    a.pointers[0] = (*(*(*s).data).zones
                                        .cast::<FZoneProperties>()
                                        .add(opposite as usize))
                                    .zone_actor;
                                    call(s, OcclusionOperation::WarpInfo, &mut a)?;
                                    let warp = a.pointers[0];
                                    let mut normal = a.integers[0] == 0 || (*frame).recursion >= 3;
                                    if !normal
                                        && (a.pointers[1].is_null() || a.pointers[2].is_null())
                                    {
                                        a.pointers[0] = warp;
                                        call(s, OcclusionOperation::Generate, &mut a)?;
                                        a.pointers[0] = warp;
                                        call(s, OcclusionOperation::WarpInfo, &mut a)?;
                                        normal = a.pointers[1].is_null() || a.pointers[2].is_null();
                                    }
                                    if normal {
                                        let old = active_mask;
                                        active_mask |= 1u64 << opposite;
                                        if active_mask != old {
                                            active[active_count] = opposite;
                                            active_count += 1;
                                        }
                                        merge_span(
                                            s,
                                            &mut zones[opposite as usize],
                                            temp_span,
                                            render,
                                        )?;
                                        (*(*(*s).data).zones
                                            .cast::<FZoneProperties>()
                                            .add(opposite as usize))
                                        .last_render_time = (*(*s).data).time_seconds;
                                    } else {
                                        add_child(
                                            s,
                                            3,
                                            temp_span,
                                            warp,
                                            index,
                                            zone as i32,
                                            front,
                                            &mut bounds,
                                        )?;
                                        bin = 0;
                                        if flags & INV != 0 && (*(*s).data).capabilities & SPAN != 0 {
                                            break 'polygon;
                                        }
                                        flags |= OCCLUDE;
                                        draw = true;
                                    }
                                }
                                if !draw && render {
                                    merge = null_mut();
                                    draw = true;
                                }
                            } else if flags & INV == 0 {
                                draw = true;
                            }
                            if draw {
                                let mut edges = [FVector {
                                    x: 0.0,
                                    y: 0.0,
                                    z: 0.0,
                                }; 32];
                                let mut edge_count = 0;
                                if !(*(*(*s).data).first_volumetric.cast::<*mut FVolActorLink>()).is_null()
                                {
                                    for i in 0..num {
                                        let previous = *pts.add(if i == 0 {
                                            num as usize - 1
                                        } else {
                                            i as usize - 1
                                        });
                                        if (*previous).int_y != -1 {
                                            let v = cross(
                                                (**pts.add(i as usize)).point,
                                                (*previous).point,
                                            );
                                            edges[edge_count] =
                                                hp2_core::math::vector_unsafe_normal(v);
                                            edge_count += 1;
                                        }
                                    }
                                }
                                if merge.is_null() {
                                    (*temporary).i_node = index;
                                    (*temporary).i_zone = (*node(s, index)).i_zone[front] as i32;
                                    (*temporary).zone = zone_actor(s, (*temporary).i_zone)?;
                                    (*temporary).i_surf = (*node(s, index)).i_surf;
                                    (*temporary).poly_flags = flags;
                                    (*temporary).next = (*frame).draw[bin];
                                    (*temporary).surf_next = *all.add((*temporary).i_surf as usize);
                                    *all.add((*temporary).i_surf as usize) = temporary;
                                    (*temporary).volumetrics = null_mut();
                                    let mut link =
                                        *(*(*s).data).first_volumetric.cast::<*mut FVolActorLink>();
                                    while !link.is_null() {
                                        if volume_visible(s, link, &edges[..edge_count])? {
                                            link_volume(s, temporary, (*link).actor)?;
                                        }
                                        link = (*link).next;
                                    }
                                    if (*(*s).data).capabilities & SPAN == 0 {
                                        save_poly(s, temporary, index, pts, num, null_mut())?;
                                        release(s, temp_span)?;
                                    }
                                    a.pointers[0] = (*temporary).zone;
                                    call(s, OcclusionOperation::ObjectIndex, &mut a)?;
                                    (*temporary).key = a.integers[0].wrapping_shl(26);
                                    let texture = (*surf(s, (*node(s, index)).i_surf)).texture;
                                    if !texture.is_null() {
                                        a.pointers[0] = texture;
                                        call(s, OcclusionOperation::TextureInfo, &mut a)?;
                                        (*temporary).key =
                                            (*temporary).key.wrapping_add(a.integers[1]);
                                        if !a.pointers[1].is_null() {
                                            a.pointers[0] = a.pointers[1];
                                            call(s, OcclusionOperation::ObjectIndex, &mut a)?;
                                            (*temporary).key = (*temporary)
                                                .key
                                                .wrapping_add(a.integers[0].wrapping_shl(12));
                                        }
                                    }
                                    (*frame).draw[bin] = temporary;
                                    temporary = alloc_draw(s, false)?;
                                    *(*(*s).data).polys_draw = (*(*(*s).data).polys_draw).wrapping_add(1);
                                } else {
                                    if (*(*s).data).capabilities & SPAN == 0 {
                                        save_poly(s, merge, index, pts, num, (*merge).polys)?;
                                    } else {
                                        merge_span(
                                            s,
                                            ptr::addr_of_mut!((*merge).span),
                                            temp_span,
                                            false,
                                        )?;
                                    }
                                    release(s, temp_span)?;
                                    let mut link =
                                        *(*(*s).data).first_volumetric.cast::<*mut FVolActorLink>();
                                    while !link.is_null() {
                                        let mut other = (*merge).volumetrics;
                                        while !other.is_null() && (*other).actor != (*link).actor {
                                            other = (*other).next;
                                        }
                                        if other.is_null()
                                            && volume_visible(s, link, &edges[..edge_count])?
                                        {
                                            link_volume(s, merge, (*link).actor)?;
                                        }
                                        link = (*link).next;
                                    }
                                }
                                (*node(s, index)).node_flags &= !8;
                                *(*(*s).data).nodes_draw = (*(*(*s).data).nodes_draw).wrapping_add(1);
                                if (*span).valid_lines <= 0 {
                                    let mut out = 0;
                                    for i in 0..active_count {
                                        active[out] = active[i];
                                        if active[i] != zone {
                                            out += 1;
                                        }
                                    }
                                    active_count = out;
                                    active_mask &= !(1u64 << zone);
                                    if active_count == 0 {
                                        break 'traverse;
                                    }
                                }
                            }
                        }
                        index = (*node(s, index)).i_plane;
                        if index == -1 {
                            break;
                        }
                    }
                    index = original;
                    n = *node(s, index);
                    let front = (plane_dot(n.plane, origin) > 0.0) as usize;
                    leaf_dynamics(
                        s,
                        index,
                        1 - front,
                        was,
                        zones.as_mut_ptr(),
                        mask,
                        volumetric,
                    )?;
                    if (*top).far != -1 {
                        index = (*top).far;
                        was = (*top).far_outside;
                        pass = 0;
                        continue;
                    }
                }
            }
        }
        top = (*top).next;
        if top.is_null() {
            break;
        }
        index = (*top).node;
        was = (*top).outside;
        pass = (*top).pass;
    }
    stat(s, 6, view_zone)?;
    for zone in &zones {
        if zone.end_y != 0 {
            stat(s, 7, 0)?;
        }
    }
    stat(s, 1, 0)
}

pub unsafe fn occlude_frame(s: *mut Hp2ScenePipelineState) -> i32 {
    let ops = NativeOcclusionOps { state: s };
    occlude_frame_with(ptr::addr_of_mut!((*s).data), &ops).is_ok() as i32
}

/// Runs source frame orchestration with the caller's typed provider.
///
/// # Safety
/// Every pointer in `data` stays live for this frame and recursive child work.
pub unsafe fn occlude_frame_with(
    data: *mut Hp2ScenePipelineData,
    ops: &dyn OcclusionOps,
) -> Result<(), ()> {
    let mut context = OcclusionContext { data, ops };
    occlude_frame_inner(&mut context)
}

unsafe fn occlude_frame_inner(s: *mut OcclusionContext<'_>) -> Result<(), ()> {
    let frame = (*(*s).data).frame.cast::<FSceneNode>();
    let mut a = Hp2ScenePipelineArgs::default();
    a.pointers[0] = (*s).data.cast();
    call(s, OcclusionOperation::Snapshot, &mut a)?;
    call(s, OcclusionOperation::AllocateDynamics, &mut a)?;
    if (*(*(*s).data).surf_lights.cast::<*mut *mut FActorLink>()).is_null()
        || count((*(*s).data).surfs) > *(*(*s).data).max_surf_lights
    {
        *(*(*s).data).max_surf_lights = count((*(*s).data).surfs);
        a.integers[0] = 0;
        call(s, OcclusionOperation::AllocateLights, &mut a)?;
    }
    if count((*(*s).data).leaves) != 0
        && ((*(*(*s).data).leaf_lights.cast::<*mut *mut FVolActorLink>()).is_null()
            || count((*(*s).data).leaves) > *(*(*s).data).max_leaf_lights)
    {
        *(*(*s).data).max_leaf_lights = count((*(*s).data).leaves);
        a.integers[0] = 1;
        call(s, OcclusionOperation::AllocateLights, &mut a)?;
    }
    *(*(*s).data).num_dyn_light_surfs = 0;
    *(*(*s).data).num_dyn_light_leaves = 0;
    a.pointers[0] = if (*(*s).data).behind_view != 0 || !(*frame).parent.is_null() {
        null_mut()
    } else if !(*(*s).data).view_target.is_null() {
        (*(*s).data).view_target
    } else {
        (*(*s).data).actor
    };
    call(s, OcclusionOperation::SetupDynamics, &mut a)?;
    call(s, OcclusionOperation::OccludeBsp, &mut a)?;
    for pass in 0..3 {
        let mut draw = (*frame).draw[pass];
        while !draw.is_null() {
            (*draw).surf_lights =
                *(*(*(*s).data).surf_lights.cast::<*mut *mut FActorLink>()).add((*draw).i_surf as usize);
            draw = (*draw).next;
        }
    }
    if count((*(*s).data).leaves) != 0 {
        let mut sprite = (*frame).sprite;
        while !sprite.is_null() {
            a.pointers[0] = (*sprite).actor;
            call(s, OcclusionOperation::ActorLeaf, &mut a)?;
            if a.integers[0] != -1 {
                (*sprite).leaf_lights = *(*(*(*s).data).leaf_lights.cast::<*mut *mut FVolActorLink>())
                    .add(a.integers[0] as usize);
            }
            sprite = (*sprite).render_next;
        }
    }
    // Cleanup precedes recursion: a child's global dynamic-light cache must not
    // overwrite the parent associations, which have just been copied above.
    ptr::write_bytes(
        array::<FDynamicsCache>((*(*s).data).dynamics),
        0,
        count((*(*s).data).dynamics) as usize,
    );
    for i in 0..*(*(*s).data).num_dyn_light_surfs {
        let index = *(*(*s).data).dyn_light_surfs.cast::<i32>().add(i as usize);
        *(*(*(*s).data).surf_lights.cast::<*mut *mut FActorLink>()).add(index as usize) = null_mut();
    }
    for i in 0..*(*(*s).data).num_dyn_light_leaves {
        let index = *(*(*s).data).dyn_light_leaves.cast::<i32>().add(i as usize);
        *(*(*(*s).data).leaf_lights.cast::<*mut *mut FVolActorLink>()).add(index as usize) = null_mut();
    }
    let mut child = (*frame).child;
    while !child.is_null() {
        a.pointers[0] = child.cast();
        call(s, OcclusionOperation::OccludeFrame, &mut a)?;
        child = (*child).sibling;
    }
    Ok(())
}

pub mod helpers_core {
    use super::{Hp2SceneStackServices, allocate_stack};
    use crate::particle::{
        FVF_OUT_XMAX, FVF_OUT_XMIN, FVF_OUT_YMAX, FVF_OUT_YMIN, ParticleFrame, compute_outcode,
        lerp_vector, madd,
    };
    use hp2_abi::render_particle::{FTransTexture, FTransform};
    use hp2_abi::render_scene::{
        FDynamicsCache, FSceneNode, FStampedPoint, FVert, FVolActorLink, TArrayRaw,
    };
    use hp2_abi::render_span::{FRasterSpan, FSpanBuffer};
    use hp2_abi::{FCoords, FVector};
    use hp2_core::math::vector_dot as dot;
    use std::{ffi::c_void, mem::size_of, ptr};

    #[repr(C)]
    pub struct Hp2ScenePipelineHelperServices {
        pub context: *mut c_void,
        pub memory: Hp2SceneStackServices,
        pub stacks: [*mut c_void; 2],
        /// Begins the native VectorMem transform lifetime; never allocates.
        pub construct: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    }
    unsafe fn allocate_transform(
        s: &Hp2ScenePipelineHelperServices,
        pool: usize,
        texture: i32,
    ) -> Result<*mut FTransform, i32> {
        let bytes = if texture == 0 {
            size_of::<FTransform>()
        } else {
            size_of::<FTransTexture>()
        };
        unsafe {
            let p = allocate_stack(
                &s.memory,
                s.context,
                s.stacks[pool],
                bytes,
                crate::span::DEFAULT_ALIGNMENT as i32,
                if pool == 0 { 1 } else { 2 },
            )?;
            if pool == 1 && (s.construct)(s.context, p.cast()) == 0 {
                return Err(1);
            }
            Ok(p.cast())
        }
    }
    #[repr(C)]
    pub struct Hp2ScenePipelineHelperClipBsp {
        pub frame: *const FSceneNode,
        pub vertices: *const FVert,
        pub count: i32,
        pub points: *const FVector,
        pub cache: *mut FStampedPoint,
        pub stamp: u32,
        pub scratch: *mut *mut FTransform,
        pub capacity: i32,
        pub num_transform: *mut i32,
        pub num_points: *mut i32,
        pub num_clip: *mut i32,
    }
    #[inline]
    unsafe fn increment(p: *mut i32) {
        if !p.is_null() {
            unsafe {
                *p = (*p).wrapping_add(1);
            }
        }
    }
    #[inline]
    pub fn particle_frame(f: &FSceneNode) -> ParticleFrame {
        ParticleFrame {
            proj_z: f.proj.z,
            rproj_z: f.r_proj.z,
            fx15: f.fx15,
            fy15: f.fy15,
            prjxm: f.prj_xm,
            prjxp: f.prj_xp,
            prjym: f.prj_ym,
            prjyp: f.prj_yp,
        }
    }
    #[inline]
    pub fn transform(p: FVector, c: &FCoords) -> FVector {
        hp2_core::math::vector_transform_point(p, *c)
    }
    pub fn pipe(result: &mut FTransform, frame: &FSceneNode, input: FVector) {
        result.point = transform(input, &frame.coords);
        let f = particle_frame(frame);
        result.flags = compute_outcode(&f, &result.point);
        if result.flags == 0 {
            crate::particle_shard::project(result, &f);
        }
    }

    // Pointer graphs are accessed only for individual loads/stores. No reference into
    // engine-owned storage survives an allocation callback.
    pub unsafe fn clip(
        s: &Hp2ScenePipelineHelperServices,
        frame: *const FSceneNode,
        dest: *mut *mut FTransform,
        src: *const *mut FTransform,
        count: i32,
        dots: *const f32,
        texture: i32,
        result: *mut i32,
    ) -> i32 {
        unsafe {
            let mut n = 0usize;
            for i in 0..count as usize {
                let j = if i == 0 { count as usize - 1 } else { i - 1 };
                let a = *dots.add(j);
                let b = *dots.add(i);
                if a >= 0.0 {
                    *dest.add(n) = *src.add(j);
                    n += 1;
                }
                if (a >= 0.0) != (b >= 0.0) {
                    let t = match allocate_transform(s, 0, texture) {
                        Ok(p) => p,
                        Err(_) => return 0,
                    };
                    *dest.add(n) = t;
                    n += 1;
                    let p = (*(*src.add(j))).point;
                    let q = (*(*src.add(i))).point;
                    ptr::addr_of_mut!((*t).point).write(lerp_vector(a / (a - b), &p, &q));
                    crate::particle_shard::project_raw(t, unsafe { (*t).point }, &particle_frame(&*frame));
                }
            }
            *result = n as i32;
            1
        }
    }

    pub unsafe fn clip_bsp(
        s: &Hp2ScenePipelineHelperServices,
        a: &Hp2ScenePipelineHelperClipBsp,
        result: *mut *mut *mut FTransform,
        count: *mut i32,
    ) -> i32 {
        unsafe {
            increment(a.num_transform);
            let mut all = 0u8;
            let mut reject = FVF_OUT_XMIN | FVF_OUT_XMAX | FVF_OUT_YMIN | FVF_OUT_YMAX;
            let mut n = a.count;
            for i in 0..n as usize {
                let index = (*a.vertices.add(i)).p_vertex as usize;
                if (*a.cache.add(index)).stamp != a.stamp {
                    (*a.cache.add(index)).stamp = a.stamp;
                    let p = match allocate_transform(s, 1, 0) {
                        Ok(p) => p,
                        Err(_) => return 0,
                    };
                    (*a.cache.add(index)).point = p;
                    crate::particle_shard::pipe_raw(p, &*a.frame, *a.points.add(index));
                    increment(a.num_points);
                }
                let p = (*a.cache.add(index)).point;
                *a.scratch.add(i) = p;
                reject &= (*p).flags;
                all |= (*p).flags;
            }
            if reject != 0 {
                *count = 0;
                return 1;
            }
            increment(a.num_clip);
            let mut src = a.scratch;
            // The native scratch has six capacity-sized rows, matching the original
            // distinct static arrays and preserving result lifetime through rasterization.
            let mut dots = [0.0f32; 24];
            for plane in 0..5usize {
                let bit = match plane {
                    0 => FVF_OUT_XMIN,
                    1 => FVF_OUT_XMAX,
                    2 => FVF_OUT_YMIN,
                    3 => FVF_OUT_YMAX,
                    _ => 0,
                };
                if plane < 4 && all & bit == 0 {
                    continue;
                }
                if plane == 4 && (*a.frame).near_clip.w == 0.0 {
                    continue;
                }
                let mut negative = false;
                for i in 0..n as usize {
                    let p = (*(*src.add(i))).point;
                    let f = &*a.frame;
                    dots[i] = match plane {
                        0 => madd(f.prj_xm, p.z, p.x),
                        1 => madd(f.prj_xp, p.z, -p.x),
                        2 => madd(f.prj_ym, p.z, p.y),
                        3 => madd(f.prj_yp, p.z, -p.y),
                        _ => {
                            dot(
                                p,
                                FVector {
                                    x: f.near_clip.x,
                                    y: f.near_clip.y,
                                    z: f.near_clip.z,
                                },
                            ) - f.near_clip.w
                        }
                    };
                    negative |= dots[i] < 0.0;
                }
                if plane == 4 && !negative {
                    continue;
                }
                let dest = a.scratch.add((plane + 1) * a.capacity as usize);
                if clip(s, a.frame, dest, src, n, dots.as_ptr(), 0, &mut n) == 0 {
                    return 0;
                }
                if n == 0 {
                    *count = 0;
                    return 1;
                }
                src = dest;
            }
            *result = src;
            *count = n;
            1
        }
    }

    #[repr(C)]
    pub struct Hp2ScenePipelineHelperRaster {
        pub start_y: i32,
        pub end_y: i32,
        pub start_x: i32,
        pub end_x: i32,
    }
    pub unsafe fn setup_raster(
        pts: *const *mut FTransform,
        n: i32,
        span: *const FSpanBuffer,
        end_y: i32,
        raster: *mut FRasterSpan,
        bounds: &mut Hp2ScenePipelineHelperRaster,
        rejects: *mut i32,
        polys: *mut i32,
    ) -> i32 {
        unsafe {
            let first = *pts;
            bounds.start_y = (*first).int_y;
            bounds.end_y = bounds.start_y;
            bounds.start_x = (*first).screen_x as i32;
            bounds.end_x = bounds.start_x;
            for i in 1..n as usize {
                let p = *pts.add(i);
                let y = (*p).int_y;
                let x = (*p).screen_x as i32;
                if y < bounds.start_y {
                    bounds.start_y = y;
                } else if y > bounds.end_y {
                    bounds.end_y = y;
                }
                if x < bounds.start_x {
                    bounds.start_x = x;
                } else if x > bounds.end_x {
                    bounds.end_x = x;
                }
            }
            if bounds.start_y < 0 || bounds.end_y > end_y {
                bounds.start_y = bounds.start_y.clamp(0, end_y);
                bounds.end_y = bounds.end_y.clamp(0, end_y);
                for i in 0..n as usize {
                    let p = *pts.add(i);
                    (*p).int_y = (*p).int_y.clamp(0, end_y);
                    (*p).screen_y = (*p).int_y as f32;
                }
            }
            if !span.is_null()
                && crate::span::box_is_visible(
                    &*span,
                    bounds.start_x,
                    bounds.start_y,
                    bounds.end_x,
                    bounds.end_y,
                ) == 0
            {
                increment(rejects);
                return 0;
            }
            increment(polys);
            for i in 0..n as usize {
                let p0 = *pts.add(if i == 0 { n as usize - 1 } else { i - 1 });
                let p1 = *pts.add(i);
                if (*p0).int_y == (*p1).int_y {
                    continue;
                }
                let index = usize::from((*p1).int_y > (*p0).int_y);
                let (top, bot) = if index == 1 { (p0, p1) } else { (p1, p0) };
                let adj = (*top).int_y as f32 - (*top).screen_y;
                let fdx = 65536.0 * ((*bot).screen_x - (*top).screen_x)
                    / ((*bot).screen_y - (*top).screen_y);
                let mut x = madd(65536.0, (*top).screen_x, adj * fdx) as i32 as u32;
                let dx = fdx as i32 as u32;
                for y in (*top).int_y..(*bot).int_y {
                    x = x.wrapping_add(dx);
                    (*raster.add(y as usize)).x[index] = (x as i32) >> 16;
                }
            }
            1
        }
    }

    #[repr(C)]
    pub struct Hp2ScenePipelineHelperCache {
        pub context: *mut c_void,
        pub point_count: i32,
        pub node_count: i32,
        pub point_capacity: i32,
        pub dynamics_capacity: i32,
        pub stamp: u32,
        pub resize_points: unsafe extern "C" fn(*mut c_void, i32, *mut *mut FStampedPoint) -> i32,
        pub resize_dynamics:
            unsafe extern "C" fn(*mut c_void, i32, *mut *mut FDynamicsCache) -> i32,
    }
    pub unsafe fn alloc_dynamics(a: &Hp2ScenePipelineHelperCache) -> i32 {
        unsafe {
            if a.point_capacity < a.point_count {
                let mut p = ptr::null_mut();
                if (a.resize_points)(a.context, a.point_count, &mut p) == 0 {
                    return 0;
                }
                for i in 0..a.point_count as usize {
                    (*p.add(i)).stamp = a.stamp;
                }
            }
            if a.dynamics_capacity < a.node_count {
                let mut p = ptr::null_mut();
                if (a.resize_dynamics)(a.context, a.node_count, &mut p) == 0 {
                    return 0;
                }
                ptr::write_bytes(p, 0, a.node_count as usize);
            }
            1
        }
    }

    #[repr(C)]
    pub struct Hp2ScenePipelineHelperLightingActor {
        pub location: FVector,
        pub volume_radius: u8,
        pub tag: *mut i32,
    }
    #[repr(C)]
    pub struct Hp2ScenePipelineHelperLighting {
        pub context: *mut c_void,
        pub memory: Hp2SceneStackServices,
        pub stack: *mut c_void,
        pub frame: *mut FSceneNode,
        /// Model Lights array header; its backing allocation may move during callbacks.
        pub lights: *const c_void,
        pub first_static: i32,
        pub dynamic: *mut FVolActorLink,
        pub first: *mut *mut FVolActorLink,
        pub stamp: u32,
        pub actor: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut Hp2ScenePipelineHelperLightingActor,
        ) -> i32,
        /// Native placement construction only; storage is allocated by the core owner.
        pub construct_static: unsafe extern "C" fn(
            *mut c_void,
            *mut FSceneNode,
            *mut c_void,
            *mut FVolActorLink,
            *mut FVolActorLink,
        ) -> i32,
        pub construct_dynamic: unsafe extern "C" fn(
            *mut c_void,
            *mut FVolActorLink,
            *mut FVolActorLink,
            *mut FVolActorLink,
        ) -> i32,
    }
    pub trait LeafLightingOps {
        type Error;
        unsafe fn actor(
            &mut self,
            actor: *mut c_void,
            out: &mut Hp2ScenePipelineHelperLightingActor,
        ) -> Result<(), Self::Error>;
        unsafe fn allocate_link(&mut self) -> Result<*mut FVolActorLink, Self::Error>;
        unsafe fn construct_static(
            &mut self,
            frame: *mut FSceneNode,
            actor: *mut c_void,
            next: *mut FVolActorLink,
            storage: *mut FVolActorLink,
        ) -> Result<(), Self::Error>;
        unsafe fn construct_dynamic(
            &mut self,
            link: *mut FVolActorLink,
            next: *mut FVolActorLink,
            storage: *mut FVolActorLink,
        ) -> Result<(), Self::Error>;
    }

    /// Shared source walk for static and dynamic leaf volumetrics.
    ///
    /// # Safety
    /// All model, frame, link, and stamp pointers must remain live for this walk.
    pub unsafe fn leaf_lighting_with_ops<O: LeafLightingOps>(
        ops: &mut O,
        frame: *mut FSceneNode,
        lights: *const c_void,
        first_static: i32,
        mut dynamic: *mut FVolActorLink,
        first: *mut *mut FVolActorLink,
        stamp: u32,
    ) -> Result<(), O::Error> {
        unsafe {
            if first_static != -1 {
                let mut i = first_static as usize;
                loop {
                    let light_array = &*lights.cast::<TArrayRaw>();
                    let actor = *light_array.data.cast::<*mut c_void>().add(i);
                    if actor.is_null() {
                        break;
                    }
                    let mut info = Hp2ScenePipelineHelperLightingActor {
                        location: FVector { x: 0.0, y: 0.0, z: 0.0 },
                        volume_radius: 0,
                        tag: ptr::null_mut(),
                    };
                    ops.actor(actor, &mut info)?;
                    if info.tag.is_null() {
                        break;
                    }
                    if *info.tag != stamp as i32 {
                        *info.tag = stamp as i32;
                        let radius = 25.0 * ((info.volume_radius as i32 + 1) as f32);
                        let mut visible = true;
                        if hp2_core::math::vector_distance_sq((*frame).coords.origin, info.location) > radius * radius {
                            for plane in (*frame).view_planes {
                                if hp2_core::math::plane_dot(plane, info.location) < -radius {
                                    visible = false;
                                    break;
                                }
                            }
                        }
                        if visible {
                            let storage = ops.allocate_link()?;
                            let next = *first;
                            ops.construct_static(frame, actor, next, storage)?;
                            *first = storage;
                        }
                    }
                    i += 1;
                }
            }
            while !dynamic.is_null() {
                if (*dynamic).volumetric != 0 {
                    let mut info = Hp2ScenePipelineHelperLightingActor {
                        location: FVector { x: 0.0, y: 0.0, z: 0.0 },
                        volume_radius: 0,
                        tag: ptr::null_mut(),
                    };
                    ops.actor((*dynamic).actor, &mut info)?;
                    if info.tag.is_null() {
                        break;
                    }
                    if *info.tag != stamp as i32 {
                        *info.tag = stamp as i32;
                        let storage = ops.allocate_link()?;
                        let next = *first;
                        ops.construct_dynamic(dynamic, next, storage)?;
                        *first = storage;
                    }
                }
                dynamic = (*dynamic).next;
            }
        }
        Ok(())
    }

    struct NativeLightingOps<'a>(&'a Hp2ScenePipelineHelperLighting);

    impl LeafLightingOps for NativeLightingOps<'_> {
        type Error = ();
        unsafe fn actor(
            &mut self,
            actor: *mut c_void,
            out: &mut Hp2ScenePipelineHelperLightingActor,
        ) -> Result<(), Self::Error> {
            if unsafe { (self.0.actor)(self.0.context, actor, out) } != 0 { Ok(()) } else { Err(()) }
        }
        unsafe fn allocate_link(&mut self) -> Result<*mut FVolActorLink, Self::Error> {
            let storage = unsafe {
                allocate_stack(
                    &self.0.memory,
                    self.0.context,
                    self.0.stack,
                    size_of::<FVolActorLink>(),
                    crate::span::DEFAULT_ALIGNMENT as i32,
                    2,
                )
            }.map_err(|_| ())?;
            Ok(storage.cast())
        }
        unsafe fn construct_static(
            &mut self,
            frame: *mut FSceneNode,
            actor: *mut c_void,
            next: *mut FVolActorLink,
            storage: *mut FVolActorLink,
        ) -> Result<(), Self::Error> {
            if unsafe { (self.0.construct_static)(self.0.context, frame, actor, next, storage) } != 0 { Ok(()) } else { Err(()) }
        }
        unsafe fn construct_dynamic(
            &mut self,
            link: *mut FVolActorLink,
            next: *mut FVolActorLink,
            storage: *mut FVolActorLink,
        ) -> Result<(), Self::Error> {
            if unsafe { (self.0.construct_dynamic)(self.0.context, link, next, storage) } != 0 { Ok(()) } else { Err(()) }
        }
    }

    pub unsafe fn leaf_lighting(a: &Hp2ScenePipelineHelperLighting) -> i32 {
        let mut ops = NativeLightingOps(a);
        match unsafe {
            leaf_lighting_with_ops(
                &mut ops,
                a.frame,
                a.lights,
                a.first_static,
                a.dynamic,
                a.first,
                a.stamp,
            )
        } {
            Ok(()) => 1,
            Err(()) => 0,
        }
    }

    /// Core-owned allocation leaf for the renderer's existing BSP clip algorithm.
    /// The allocator chooses the active VectorMem stack and preserves engine errors.
    pub trait CoreClipBspOps {
        unsafe fn push(&self, bytes: usize, align: i32, kind: u32) -> Result<*mut u8, ()>;
    }

    unsafe fn core_transform<O: CoreClipBspOps>(
        ops: &O,
        frame: *const FSceneNode,
    ) -> Result<*mut FTransform, ()> {
        unsafe {
            Ok(ops.push(size_of::<FTransform>(), crate::span::DEFAULT_ALIGNMENT as i32, 2)?.cast())
        }
    }

    unsafe fn core_project(output: *mut FTransform, frame: *const FSceneNode, point: FVector) {
        unsafe {
            ptr::addr_of_mut!((*output).point).write(point);
            crate::particle_shard::project_raw(output, point, &particle_frame(&*frame));
        }
    }

    unsafe fn core_pipe(output: *mut FTransform, frame: *const FSceneNode, input: FVector) {
        unsafe { crate::particle_shard::pipe_raw(output, &*frame, input); }
    }

    unsafe fn core_clip<O: CoreClipBspOps>(
        ops: &O,
        frame: *const FSceneNode,
        dst: *mut *mut FTransform,
        src: *const *mut FTransform,
        count: i32,
        dots: *const f32,
        out_count: *mut i32,
    ) -> Result<(), ()> {
        let mut n = 0usize;
        for i in 0..count as usize {
            let j = if i == 0 { count as usize - 1 } else { i - 1 };
            let a = unsafe { *dots.add(j) };
            let b = unsafe { *dots.add(i) };
            if a >= 0.0 {
                unsafe { *dst.add(n) = *src.add(j); }
                n += 1;
            }
            if (a >= 0.0) != (b >= 0.0) {
                let transform = unsafe { core_transform(ops, frame)? };
                unsafe {
                    *dst.add(n) = transform;
                    let p = (*(*src.add(j))).point;
                    let q = (*(*src.add(i))).point;
                    let point = lerp_vector(a / (a - b), &p, &q);
                    core_project(transform, frame, point);
                }
                n += 1;
            }
        }
        unsafe { *out_count = n as i32; }
        Ok(())
    }

    /// Source `URender::ClipBspSurf`, using pinned Rust model cache and Core VectorMem.
    ///
    /// # Safety
    /// All arrays and the six-row scratch buffer remain live through the next raster call.
    pub unsafe fn clip_bsp_with<O: CoreClipBspOps>(
        ops: &O,
        input: &Hp2ScenePipelineHelperClipBsp,
        result: *mut *mut *mut FTransform,
        count: *mut i32,
    ) -> Result<(), ()> {
        unsafe {
            increment(input.num_transform);
            let mut all = 0u8;
            let mut reject = FVF_OUT_XMIN | FVF_OUT_XMAX | FVF_OUT_YMIN | FVF_OUT_YMAX;
            let mut n = input.count;
            for i in 0..n as usize {
                let index = (*input.vertices.add(i)).p_vertex as usize;
                if (*input.cache.add(index)).stamp != input.stamp {
                    (*input.cache.add(index)).stamp = input.stamp;
                    let p = core_transform(ops, input.frame)?;
                    core_pipe(p, input.frame, *input.points.add(index));
                    (*input.cache.add(index)).point = p;
                    increment(input.num_points);
                }
                let p = (*input.cache.add(index)).point;
                *input.scratch.add(i) = p;
                reject &= (*p).flags;
                all |= (*p).flags;
            }
            if reject != 0 {
                *count = 0;
                return Ok(());
            }
            increment(input.num_clip);
            let mut src = input.scratch;
            let mut dots = [0.0f32; 24];
            for plane in 0..5usize {
                let bit = match plane { 0 => FVF_OUT_XMIN, 1 => FVF_OUT_XMAX, 2 => FVF_OUT_YMIN, 3 => FVF_OUT_YMAX, _ => 0 };
                if plane < 4 && all & bit == 0 { continue; }
                if plane == 4 && (*input.frame).near_clip.w == 0.0 { continue; }
                let mut negative = false;
                for i in 0..n as usize {
                    let point = (*(*src.add(i))).point;
                    let f = &*input.frame;
                    dots[i] = match plane {
                        0 => madd(f.prj_xm, point.z, point.x),
                        1 => madd(f.prj_xp, point.z, -point.x),
                        2 => madd(f.prj_ym, point.z, point.y),
                        3 => madd(f.prj_yp, point.z, -point.y),
                        _ => dot(point, FVector { x: f.near_clip.x, y: f.near_clip.y, z: f.near_clip.z }) - f.near_clip.w,
                    };
                    negative |= dots[i] < 0.0;
                }
                if plane == 4 && !negative { continue; }
                let dest = input.scratch.add((plane + 1) * input.capacity as usize);
                core_clip(ops, input.frame, dest, src, n, dots.as_ptr(), &mut n)?;
                if n == 0 {
                    *count = 0;
                    return Ok(());
                }
                src = dest;
            }
            *result = src;
            *count = n;
            Ok(())
        }
    }
}

pub mod hp2_scene_pipeline_helper_geometry {
    use crate::particle::{ParticleFrame, compute_outcode, madd};
    use crate::softln::{REN_ORTH_XY, REN_ORTH_XZ, REN_ORTH_YZ, ScreenFrame, project};
    use crate::state::MAX_FINAL_VERTICES;
    const MAX_DECAL_CLIP_VERTICES: usize = MAX_FINAL_VERTICES + 8;
    use hp2_abi::render_particle::FTransTexture;
    use hp2_abi::render_scene::{FSavedPoly, FSceneNode, FScreenBounds};
    use hp2_abi::render_span::FSpanBuffer;
    use hp2_abi::{FCoords, FPlane, FVector};
    use hp2_core::math;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct Hp2ScenePipelineHelperBoundStats {
        pub inside: i32,
        pub behind: i32,
        pub outside: i32,
        pub occluded: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct Hp2ScenePipelineHelperDecalMaterial {
        pub scale_glow: f32,
        pub ambient_glow: u8,
        pub draw_scale: f32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Hp2ScenePipelineHelperDecalLeaves {
        pub context: *mut c_void,
        pub material: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut Hp2ScenePipelineHelperDecalMaterial,
        ) -> i32,
        pub check_capacity: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    }
    /// Typed decal leaves required by clipping; Core adapters retain their `CoreAccess` here.
    pub trait DecalClipOps {
        /// Loads the source actor's scale/glow material fields.
        unsafe fn material(
            &self,
            decal: *mut c_void,
            out: *mut Hp2ScenePipelineHelperDecalMaterial,
        ) -> i32;
        /// Applies the source final-vertex capacity check.
        unsafe fn check_capacity(&self, count: i32) -> i32;
    }

    struct NativeDecalClipOps(*const Hp2ScenePipelineHelperDecalLeaves);

    impl DecalClipOps for NativeDecalClipOps {
        unsafe fn material(
            &self,
            decal: *mut c_void,
            out: *mut Hp2ScenePipelineHelperDecalMaterial,
        ) -> i32 {
            let leaves = unsafe { *self.0 };
            unsafe { (leaves.material)(leaves.context, decal, out) }
        }

        unsafe fn check_capacity(&self, count: i32) -> i32 {
            let leaves = unsafe { *self.0 };
            unsafe { (leaves.check_capacity)(leaves.context, count) }
        }
    }

    fn particle_frame(f: &FSceneNode) -> ParticleFrame {
        ParticleFrame {
            proj_z: f.proj.z,
            rproj_z: f.r_proj.z,
            fx15: f.fx15,
            fy15: f.fy15,
            prjxm: f.prj_xm,
            prjxp: f.prj_xp,
            prjym: f.prj_ym,
            prjyp: f.prj_yp,
        }
    }

    fn clamp(value: f32, low: f32, high: f32) -> f32 {
        if value < low {
            low
        } else if value < high {
            value
        } else {
            high
        }
    }

    // Native pointers have the same validity and aliasing requirements as the
    // original method. No SDK callbacks occur while these frame/span borrows live.
    pub unsafe fn hp2_scene_pipeline_helper_bound_visible(
        frame: *const FSceneNode,
        box_coords: *const FCoords,
        span: *const FSpanBuffer,
        result: *mut FScreenBounds,
        rend_map: i32,
        stats: *mut Hp2ScenePipelineHelperBoundStats,
    ) -> i32 {
        unsafe {
            *stats = Hp2ScenePipelineHelperBoundStats::default();
            let f = &*frame;
            let b = *box_coords;
            if matches!(rend_map, REN_ORTH_XY | REN_ORTH_XZ | REN_ORTH_YZ) {
                let world = math::box_from_coords(&b);
                let screen = ScreenFrame {
                    rend_map,
                    coords: f.coords,
                    zoom: f.zoom,
                    fx2: f.fx2,
                    fy2: f.fy2,
                    proj_z: f.proj.z,
                };
                let low = project(&screen, world.min);
                (*result).min_x = low.screen_x;
                (*result).min_y = low.screen_y;
                let high = project(&screen, world.max);
                (*result).max_x = high.screen_x;
                (*result).max_y = high.screen_y;
                if (*result).min_x > (*result).max_x {
                    std::ptr::swap(
                        std::ptr::addr_of_mut!((*result).min_x),
                        std::ptr::addr_of_mut!((*result).max_x),
                    );
                }
                if (*result).min_y > (*result).max_y {
                    std::ptr::swap(
                        std::ptr::addr_of_mut!((*result).min_y),
                        std::ptr::addr_of_mut!((*result).max_y),
                    );
                }
                // Orthographic projection deliberately leaves MinZ/MaxZ alone.
                return ((*result).max_x > 0.0
                    && (*result).max_y > 0.0
                    && (*result).min_x < f.fx
                    && (*result).min_y < f.fy) as i32;
            }

            let mut coords = math::coords_mul(f.coords, b);
            coords.origin = math::vector_shift(f.coords.origin, math::coords_inverse(&b));
            let origin = coords.origin;
            if origin.x >= 0.0
                && origin.x <= 1.0
                && origin.y >= 0.0
                && origin.y <= 1.0
                && origin.z >= 0.0
                && origin.z <= 1.0
            {
                *result = FScreenBounds {
                    min_x: 0.0,
                    min_y: 0.0,
                    max_x: f.fx,
                    max_y: f.fy,
                    min_z: 0.0,
                    max_z: 1.0e20,
                };
                (*stats).inside = 1;
                return 1;
            }
            let z0 = math::vector_mul(math::vector_neg(origin), coords.z_axis);
            let z1 = math::vector_add(z0, coords.z_axis);
            if z0.x < 0.0 && z0.y < 0.0 && z0.z < 0.0 && z1.x < 0.0 && z1.y < 0.0 && z1.z < 0.0 {
                (*stats).behind = 1;
                return 0;
            }
            let x0 = math::vector_mul(math::vector_neg(origin), coords.x_axis);
            let y0 = math::vector_mul(math::vector_neg(origin), coords.y_axis);
            let xs = [x0, math::vector_add(x0, coords.x_axis)];
            let ys = [y0, math::vector_add(y0, coords.y_axis)];
            let zs = [z0, z1];
            let zero = FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            let mut points = [zero; 8];
            let mut min_z = 0.0;
            let mut max_z = 0.0;
            let mut out_code = 15u8;
            let mut all_codes = 0u8;
            let projection = particle_frame(f);
            for (n, point) in points.iter_mut().enumerate() {
                let i = n & 1;
                let j = (n >> 1) & 1;
                let k = (n >> 2) & 1;
                point.z = (zs[i].x + zs[j].y) + zs[k].z;
                if n == 0 || point.z < min_z {
                    min_z = point.z;
                }
                if n == 0 || point.z > max_z {
                    max_z = point.z;
                }
                point.x = (xs[i].x + xs[j].y) + xs[k].z;
                point.y = (ys[i].x + ys[j].y) + ys[k].z;
                let code = compute_outcode(&projection, point) >> 2;
                out_code &= code;
                all_codes |= code;
            }
            if out_code != 0 {
                (*stats).outside = 1;
                return 0;
            }
            // appFloor is the target's truncating float-to-INT conversion.
            let rz = f.proj.z / points[0].z;
            let mut min_x = madd(points[0].x, rz, f.fx2) as i32;
            let mut max_x = min_x;
            let mut min_y = madd(points[0].y, rz, f.fy2) as i32;
            let mut max_y = min_y;
            if all_codes & 1 != 0 {
                min_x = 0;
            }
            if all_codes & 2 != 0 {
                max_x = f.x;
            }
            if all_codes & 4 != 0 {
                min_y = 0;
            }
            if all_codes & 8 != 0 {
                max_y = f.y;
            }
            for point in &points[1..] {
                let rz = f.proj.z / point.z;
                let x = madd(point.x, rz, f.fx2) as i32;
                let y = madd(point.y, rz, f.fy2) as i32;
                if x < min_x {
                    min_x = x;
                } else if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                } else if y > max_y {
                    max_y = y;
                }
            }
            *result = FScreenBounds {
                min_x: min_x.max(0) as f32,
                min_y: min_y.max(0) as f32,
                max_x: max_x.min(f.x) as f32,
                max_y: max_y.min(f.y) as f32,
                min_z: if min_z >= 0.0 { min_z } else { 0.0 },
                max_z,
            };
            if span.is_null()
                || crate::span::box_is_visible(&*span, min_x, min_y, max_x, max_y) != 0
            {
                1
            } else {
                (*stats).occluded = 1;
                0
            }
        }
    }

    // Storage and pointer-array lifetime remain the native method's static
    // lifetime. Geometry scratch is local, with no allocation or callback borrow.
    /// Clips a decal using typed Rust-owned source leaves.
    ///
    /// # Safety
    /// All pointers must satisfy the native helper's live-storage and aliasing requirements.
    pub unsafe fn hp2_scene_pipeline_helper_clip_decal_with(
        ops: &impl DecalClipOps,
        frame: *const FSceneNode,
        decal: *mut c_void,
        vertices: *const FVector,
        surf_normal: *const FVector,
        surf_base: *const FVector,
        poly: *const FSavedPoly,
        storage: *mut FTransTexture,
        pointers: *mut *mut FTransTexture,
        decal_points: *mut *mut *mut FTransTexture,
        num_pts: *mut i32,
    ) -> i32 {
        unsafe {
            let normal = *surf_normal;
            let base = *surf_base;
            *num_pts = 4;
            let adj = math::vector_normalize_inline(normal).1;
            let zero = FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            let mut points = [zero; MAX_DECAL_CLIP_VERTICES];
            let mut poly_points = [zero; MAX_DECAL_CLIP_VERTICES];
            let mut dots = [0.0; MAX_DECAL_CLIP_VERTICES];
            for i in 0..4 {
                points[i] = math::vector_add(math::vector_add(*vertices.add(i), base), adj);
            }
            let poly_count = (*poly).num_pts as usize;
            if poly_count > MAX_DECAL_CLIP_VERTICES {
                return -1;
            }
            let poly_ptrs =
                std::ptr::addr_of!((*poly).pts).cast::<*mut hp2_abi::render_particle::FTransform>();
            for i in 0..poly_count {
                poly_points[i] =
                    math::vector_transform_point((**poly_ptrs.add(i)).point, (*frame).uncoords);
            }
            let mut count = 4usize;
            let mut prev = poly_count.wrapping_sub(1);
            for i in 0..poly_count {
                let normal = math::vector_cross(
                    math::vector_sub(poly_points[prev], poly_points[i]),
                    math::vector_sub((*frame).coords.origin, poly_points[i]),
                );
                let plane = FPlane {
                    x: normal.x,
                    y: normal.y,
                    z: normal.z,
                    w: math::vector_dot(poly_points[i], normal),
                };
                for j in 0..count {
                    dots[j] = math::plane_dot(plane, points[j]);
                }
                let mut j = 0;
                while j < count {
                    let n = if j + 1 == count { 0 } else { j + 1 };
                    if (dots[j] > 0.0 && dots[n] < 0.0) || (dots[j] < 0.0 && dots[n] > 0.0) {
                        if count >= MAX_DECAL_CLIP_VERTICES {
                            return -1;
                        }
                        let point = math::line_plane(points[j], points[n], plane);
                        if j < count - 1 {
                            dots.copy_within(j + 1..count, j + 2);
                            points.copy_within(j + 1..count, j + 2);
                        }
                        points[j + 1] = point;
                        dots[j + 1] = 0.0;
                        count += 1;
                        *num_pts = count as i32;
                        j += 1;
                        if count >= MAX_FINAL_VERTICES
                            && unsafe { ops.check_capacity(count as i32) } == 0
                        {
                            return -1;
                        }
                    }
                    j += 1;
                }
                let mut j = 0;
                while j < count {
                    if dots[j] < 0.0 {
                        dots.copy_within(j + 1..count, j);
                        points.copy_within(j + 1..count, j);
                        count -= 1;
                        *num_pts = count as i32;
                    } else {
                        j += 1;
                    }
                }
                if count == 0 {
                    return 0;
                }
                prev = i;
            }
            let mut material = Hp2ScenePipelineHelperDecalMaterial::default();
            if unsafe { ops.material(decal, &mut material) } == 0 {
                return -1;
            }
            let unlit = clamp(
                madd(
                    material.scale_glow,
                    0.5,
                    material.ambient_glow as f32 / 256.0,
                ),
                0.0,
                1.0,
            );
            *decal_points = pointers;
            let projection = particle_frame(&*frame);
            for i in 0..count {
                let out = storage.add(i);
                *pointers.add(i) = out;
                let point = math::vector_transform_point(points[i], (*frame).coords);
                let flags = compute_outcode(&projection, &point);
                (*out).transform.point = point;
                (*out).transform.flags = flags;
                crate::particle_shard::project(&mut (*out).transform, &projection);
                if flags != 0 {
                    (*out).transform.screen_x =
                        clamp((*out).transform.screen_x, 0.0, ((*frame).x - 1) as f32);
                    (*out).transform.screen_y =
                        clamp((*out).transform.screen_y, 0.0, ((*frame).y - 1) as f32);
                }
                let offset = math::vector_sub(math::vector_sub(points[i], base), *vertices);
                let u = math::vector_sub(*vertices.add(1), *vertices);
                let v = math::vector_sub(*vertices.add(3), *vertices);
                (*out).u = math::vector_dot(offset, math::vector_div(u, math::vector_size(u)))
                    / material.draw_scale;
                (*out).v = math::vector_dot(offset, math::vector_div(v, math::vector_size(v)))
                    / material.draw_scale;
                (*out).light = FPlane {
                    x: unlit,
                    y: unlit,
                    z: unlit,
                    w: 0.0,
                };
                (*out).fog = FPlane {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                };
                // Normal and padding deliberately retain the native static contents.
            }
            count as i32
        }
    }
    /// Preserves the callback-table entrypoint for existing native hosts.
    ///
    /// # Safety
    /// All pointers and callback targets must remain valid for the duration of the call.
    pub unsafe fn hp2_scene_pipeline_helper_clip_decal(
        leaves: *const Hp2ScenePipelineHelperDecalLeaves,
        frame: *const FSceneNode,
        decal: *mut c_void,
        vertices: *const FVector,
        surf_normal: *const FVector,
        surf_base: *const FVector,
        poly: *const FSavedPoly,
        storage: *mut FTransTexture,
        pointers: *mut *mut FTransTexture,
        decal_points: *mut *mut *mut FTransTexture,
        num_pts: *mut i32,
    ) -> i32 {
        let ops = NativeDecalClipOps(leaves);
        unsafe {
            hp2_scene_pipeline_helper_clip_decal_with(
                &ops,
                frame,
                decal,
                vertices,
                surf_normal,
                surf_base,
                poly,
                storage,
                pointers,
                decal_points,
                num_pts,
            )
        }
    }
}

pub mod helper_frames {
    use super::{Hp2SceneStackServices, allocate_stack};
    use crate::span::{self, SpanAlloc};
    use hp2_abi::render_scene::{FSceneNode, FScreenBounds, TArrayRaw};
    use hp2_abi::render_span::{FMemMark, FSpanBuffer};
    use hp2_abi::{FCoords, FPlane, FVector};
    use hp2_core::math::coords_transpose;
    use hp2_core::mem;
    use std::{
        ffi::c_void,
        mem::{MaybeUninit, size_of},
        panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
        ptr,
    };

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Hp2ScenePipelineHelperActor {
        pub actor: *mut c_void,
        pub target: *mut c_void,
        pub canvas: *mut c_void,
        pub admin: i32,
        pub no_cheating: i32,
        pub behind: i32,
        pub show_flags: u32,
    }
    #[repr(C)]
    pub struct Hp2ScenePipelineHelperFrames {
        pub context: *mut c_void,
        pub memory: Hp2SceneStackServices,
        pub scene_count: *mut i32,
        pub master_marks: [*mut FMemMark; 3],
        pub stacks: [*mut c_void; 4],
        pub frame_stamp: *mut i32,
        pub ugly_flags: *mut u32,
        pub mover_rejects: *mut i32,
        pub editor: *const i32,
        pub show_actors: u32,
        pub dyn_light: i32,
        pub viewport: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut i32,
            *mut i32,
            *mut *mut c_void,
        ) -> i32,
        pub location: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut FVector) -> i32,
        pub rotation: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32) -> i32,
        pub fog_get: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
        pub fog_set: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
        /// Native object lifetime only; storage already belongs to hp2-core.
        pub frame_construct: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
        pub span_construct: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
        pub coords: unsafe extern "C" fn(
            *mut c_void,
            *mut FSceneNode,
            *const FVector,
            *const c_void,
        ) -> i32,
        pub size: unsafe extern "C" fn(*mut c_void, *mut FSceneNode) -> i32,
        pub zone: unsafe extern "C" fn(*mut c_void, *mut c_void, *const FVector, *mut i32) -> i32,
        pub occlude_bsp: unsafe extern "C" fn(*mut c_void, *mut FSceneNode) -> i32,
        pub occlude_frame: unsafe extern "C" fn(*mut c_void, *mut FSceneNode) -> i32,
        pub draw_frame: unsafe extern "C" fn(*mut c_void, *mut FSceneNode) -> i32,
        pub add_surface: unsafe extern "C" fn(*mut c_void, *mut TArrayRaw, i32) -> i32,
        pub actor:
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Hp2ScenePipelineHelperActor) -> i32,
        pub render_mode: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32) -> i32,
        pub audio_get: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
        pub audio_render: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut FSceneNode) -> i32,
        pub mover_enabled: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
        pub mover_report: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
        pub overlays: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
        pub count_failure: unsafe extern "C" fn(*mut c_void) -> i32,
    }
    /// Core-owned inputs for the shared world orchestration policy.
    pub struct WorldFrames {
        pub stacks: [*mut c_void; 4],
        pub frame_stamp: *mut i32,
        pub mover_rejects: *mut i32,
        pub editor: bool,
        pub show_actors: u32,
        pub dyn_light: i32,
    }
    /// Typed world-render operations supplied by Native or canonical Core.
    ///
    /// # Safety
    /// Implementations must access only live records and preserve the caller's frame scope.
    pub trait WorldOps {
        type Error;

        unsafe fn actor(
            &mut self,
            viewport: *mut c_void,
        ) -> Result<Hp2ScenePipelineHelperActor, Self::Error>;
        unsafe fn render_mode(
            &mut self,
            viewport: *mut c_void,
            mode: i32,
            lighting: i32,
        ) -> Result<(), Self::Error>;
        unsafe fn audio(&mut self) -> Result<*mut c_void, Self::Error>;
        unsafe fn render_audio(
            &mut self,
            audio: *mut c_void,
            frame: *mut FSceneNode,
        ) -> Result<(), Self::Error>;
        unsafe fn occlude_frame(&mut self, frame: *mut FSceneNode) -> Result<(), Self::Error>;
        unsafe fn draw_frame(&mut self, frame: *mut FSceneNode) -> Result<(), Self::Error>;
        unsafe fn mover_debug_enabled(&mut self) -> Result<bool, Self::Error>;
        unsafe fn mover_report(&mut self, stamp: i32, rejects: i32) -> Result<(), Self::Error>;
        unsafe fn overlays_begin(&mut self) -> Result<(), Self::Error>;
        unsafe fn overlays(
            &mut self,
            actor: *mut c_void,
            canvas: *mut c_void,
        ) -> Result<(), Self::Error>;
        unsafe fn overlays_end(&mut self) -> Result<(), Self::Error>;
    }
    struct WorldMarks<'a, A: SceneStackOps> {
        allocator: &'a A,
        stacks: [*mut c_void; 4],
        marks: [MaybeUninit<FMemMark>; 4],
        saved: [bool; 4],
    }
    impl<'a, A: SceneStackOps> WorldMarks<'a, A> {
        unsafe fn save(allocator: &'a A, stacks: [*mut c_void; 4]) -> WorldMarks<'a, A> {
            let mut guard = WorldMarks {
                allocator,
                stacks,
                marks: [MaybeUninit::uninit(); 4],
                saved: [false; 4],
            };
            for index in [2usize, 0, 1, 3] {
                unsafe {
                    guard
                        .allocator
                        .mark_save(guard.stacks[index], guard.marks[index].as_mut_ptr());
                }
                guard.saved[index] = true;
            }
            guard
        }
    }
    impl<A: SceneStackOps> Drop for WorldMarks<'_, A> {
        fn drop(&mut self) {
            for index in [3usize, 1, 0, 2] {
                if self.saved[index] {
                    unsafe { self.allocator.mark_pop(self.marks[index].assume_init_ref()) };
                }
            }
        }
    }
    /// Executes the source-ordered `URender::DrawWorld` scope over typed Native/Core ops.
    ///
    /// # Safety
    /// `frame`, service pointers, and the four stack pointers must remain valid until return.
    pub unsafe fn world_with<A: SceneStackOps, O: WorldOps>(
        s: &WorldFrames,
        allocator: &A,
        ops: &mut O,
        frame: *mut FSceneNode,
    ) -> Result<(), O::Error> {
        let _marks = unsafe { WorldMarks::save(allocator, s.stacks) };
        unsafe {
            *s.frame_stamp = (*s.frame_stamp).wrapping_add(1);
            let viewport = (*frame).viewport;
            let first = ops.actor(viewport)?;
            if first.admin == 0 && first.no_cheating != 0 && !s.editor {
                ops.render_mode(viewport, s.dyn_light, 0)?;
            }
            let audio = ops.audio()?;
            if !audio.is_null() && !s.editor {
                ops.render_audio(audio, frame)?;
            }
            ops.occlude_frame(frame)?;
            ops.draw_frame(frame)?;
            let mover_debug = ops.mover_debug_enabled()?;
            let rejects = *s.mover_rejects;
            if mover_debug && rejects > 0 {
                ops.mover_report(*s.frame_stamp, rejects)?;
                *s.mover_rejects = 0;
            }
            let last = ops.actor(viewport)?;
            let overlay = if last.behind != 0 {
                ptr::null_mut()
            } else if !last.target.is_null() {
                last.target
            } else {
                last.actor
            };
            if !s.editor && !overlay.is_null() && last.show_flags & s.show_actors != 0 {
                ops.overlays_begin()?;
                let overlays = ops.overlays(overlay, last.canvas);
                let cleanup = ops.overlays_end();
                overlays?;
                cleanup?;
            }
        }
        Ok(())
    }

    struct NativeWorldOps<'a>(&'a Hp2ScenePipelineHelperFrames, bool);
    impl WorldOps for NativeWorldOps<'_> {
        type Error = ();

        unsafe fn actor(
            &mut self,
            viewport: *mut c_void,
        ) -> Result<Hp2ScenePipelineHelperActor, Self::Error> {
            let mut actor = MaybeUninit::<Hp2ScenePipelineHelperActor>::uninit();
            unsafe {
                ok((self.0.actor)(self.0.context, viewport, actor.as_mut_ptr()))?;
                Ok(actor.assume_init())
            }
        }
        unsafe fn render_mode(
            &mut self,
            viewport: *mut c_void,
            mode: i32,
            lighting: i32,
        ) -> Result<(), Self::Error> {
            unsafe { ok((self.0.render_mode)(self.0.context, viewport, mode, lighting)) }
        }
        unsafe fn audio(&mut self) -> Result<*mut c_void, Self::Error> {
            let mut audio = ptr::null_mut();
            unsafe {
                ok((self.0.audio_get)(self.0.context, &mut audio))?;
            }
            Ok(audio)
        }
        unsafe fn render_audio(
            &mut self,
            audio: *mut c_void,
            frame: *mut FSceneNode,
        ) -> Result<(), Self::Error> {
            unsafe { ok((self.0.audio_render)(self.0.context, audio, frame)) }
        }
        unsafe fn occlude_frame(&mut self, frame: *mut FSceneNode) -> Result<(), Self::Error> {
            unsafe { ok((self.0.occlude_frame)(self.0.context, frame)) }
        }
        unsafe fn draw_frame(&mut self, frame: *mut FSceneNode) -> Result<(), Self::Error> {
            unsafe { ok((self.0.draw_frame)(self.0.context, frame)) }
        }
        unsafe fn mover_debug_enabled(&mut self) -> Result<bool, Self::Error> {
            let mut enabled = 0;
            unsafe {
                ok((self.0.mover_enabled)(self.0.context, &mut enabled))?;
            }
            Ok(enabled != 0)
        }
        unsafe fn mover_report(&mut self, stamp: i32, rejects: i32) -> Result<(), Self::Error> {
            unsafe { ok((self.0.mover_report)(self.0.context, stamp, rejects)) }
        }
        unsafe fn overlays(
            &mut self,
            actor: *mut c_void,
            canvas: *mut c_void,
        ) -> Result<(), Self::Error> {
            unsafe { ok((self.0.overlays)(self.0.context, actor, canvas)) }
        }
        unsafe fn overlays_begin(&mut self) -> Result<(), Self::Error> {
            unsafe {
                self.1 = *self.0.ugly_flags & 1 != 0;
                *self.0.ugly_flags |= 1;
            }
            Ok(())
        }
        unsafe fn overlays_end(&mut self) -> Result<(), Self::Error> {
            unsafe {
                if self.1 {
                    *self.0.ugly_flags |= 1;
                } else {
                    *self.0.ugly_flags &= !1;
                }
            }
            Ok(())
        }
    }
    /// Typed Core-side input needed for master-frame creation and finish.
    /// This is not an FFI record; the Native `Hp2ScenePipelineHelperFrames` ABI stays fixed.
    pub struct MasterFrames {
        pub context: *mut c_void,
        pub scene_count: *mut i32,
        pub master_marks: [*mut FMemMark; 3],
        pub stacks: [*mut c_void; 4],
        pub viewport: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut i32,
            *mut i32,
            *mut *mut c_void,
        ) -> i32,
        pub frame_construct: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
        pub span_construct: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
        pub coords: unsafe extern "C" fn(
            *mut c_void,
            *mut FSceneNode,
            *const FVector,
            *const c_void,
        ) -> i32,
        pub zone: unsafe extern "C" fn(*mut c_void, *mut c_void, *const FVector, *mut i32) -> i32,
        pub count_failure: unsafe extern "C" fn(*mut c_void) -> i32,
    }
    fn ok(status: i32) -> Result<(), ()> {
        if status != 0 { Ok(()) } else { Err(()) }
    }
    /// Scene-frame scratch allocation and mark operations for Native or canonical Core stacks.
    ///
    /// # Safety
    /// Stack pointers and marks belong to the active scene owner's live arenas; allocation
    /// remains valid until the matching LIFO mark pop.
    pub trait SceneStackOps {
        unsafe fn push(
            &self,
            context: *mut c_void,
            stack: *mut c_void,
            size: usize,
            alignment: i32,
            kind: u32,
        ) -> Result<*mut u8, i32>;
        unsafe fn mark_save(&self, stack: *mut c_void, mark: *mut FMemMark);
        unsafe fn mark_pop(&self, mark: *const FMemMark);
    }
    struct NativeStackOps<'a>(&'a Hp2ScenePipelineHelperFrames);
    impl SceneStackOps for NativeStackOps<'_> {
        unsafe fn push(
            &self,
            context: *mut c_void,
            stack: *mut c_void,
            size: usize,
            alignment: i32,
            kind: u32,
        ) -> Result<*mut u8, i32> {
            unsafe { allocate_stack(&self.0.memory, context, stack, size, alignment, kind) }
        }
        unsafe fn mark_save(&self, stack: *mut c_void, mark: *mut FMemMark) {
            unsafe { mem::mark_init(mark, stack.cast()) }
        }
        unsafe fn mark_pop(&self, mark: *const FMemMark) {
            unsafe { mem::mark_pop(&*mark) }
        }
    }
    struct Alloc<'a, A> {
        context: *mut c_void,
        allocator: &'a A,
    }
    // Publish the partially initialized header even when allocation fails.
    impl<A: SceneStackOps> SpanAlloc for Alloc<'_, A> {
        unsafe fn push(&self, stack: *mut c_void, size: usize, align: usize) -> Result<*mut u8, i32> {
            unsafe { self.allocator.push(self.context, stack, size, align as i32, 0) }
        }
        unsafe fn mark_save(&self, stack: *mut c_void, mark: *mut FMemMark) {
            unsafe { self.allocator.mark_save(stack, mark) }
        }
        unsafe fn mark_pop(&self, mark: *mut FMemMark) {
            unsafe { self.allocator.mark_pop(mark) }
        }
        fn warn(&self, _: &str) {
            unreachable!("frame span operations do not warn")
        }
    }
    unsafe fn span_edit<A: SceneStackOps>(
        context: *mut c_void,
        allocator: &A,
        dst: *mut FSpanBuffer,
        initial: FSpanBuffer,
        f: impl FnOnce(&Alloc<'_, A>, &mut FSpanBuffer) -> Result<(), i32>,
    ) -> Result<(), ()> {
        let mut local = initial;
        let result = catch_unwind(AssertUnwindSafe(|| {
            f(&Alloc { context, allocator }, &mut local)
        }));
        unsafe {
            dst.write(local);
        }
        match result {
            Ok(result) => result.map_err(|_| ()),
            Err(e) => resume_unwind(e),
        }
    }
    unsafe fn scene_alloc<T, A: SceneStackOps>(
        context: *mut c_void,
        allocator: &A,
        stack: *mut c_void,
        construct: impl FnOnce(*mut c_void) -> Result<(), ()>,
    ) -> Result<*mut T, ()> {
        unsafe {
            let p = allocator.push(
                context,
                stack,
                size_of::<T>(),
                span::DEFAULT_ALIGNMENT as i32,
                2,
            ).map_err(|_| ())?;
            construct(p.cast())?;
            Ok(p.cast())
        }
    }
    fn empty_span() -> FSpanBuffer {
        FSpanBuffer {
            start_y: 0,
            end_y: 0,
            valid_lines: 0,
            index: ptr::null_mut(),
            mem: ptr::null_mut(),
            mark: FMemMark {
                mem: ptr::null_mut(),
                top: ptr::null_mut(),
                saved_chunk: ptr::null_mut(),
            },
        }
    }
    unsafe fn merge<A: SceneStackOps>(
        context: *mut c_void,
        allocator: &A,
        dst: *mut FSpanBuffer,
        src: *const FSpanBuffer,
    ) -> Result<(), ()> {
        let initial = unsafe { dst.read() };
        let source = unsafe { src.read() };
        unsafe { span_edit(context, allocator, dst, initial, |a, b| span::merge_with(a, b, &source)) }
    }
    pub unsafe fn master(
        s: &Hp2ScenePipelineHelperFrames,
        viewport: *mut c_void,
        location: FVector,
        rotation: *const c_void,
        out: *mut *mut FSceneNode,
    ) -> Result<(), ()> {
        let frames = MasterFrames {
            context: s.context,
            scene_count: s.scene_count,
            master_marks: s.master_marks,
            stacks: s.stacks,
            viewport: s.viewport,
            frame_construct: s.frame_construct,
            span_construct: s.span_construct,
            coords: s.coords,
            zone: s.zone,
            count_failure: s.count_failure,
        };
        unsafe { master_with(&frames, &NativeStackOps(s), viewport, location, rotation, out) }
    }
    struct MasterFrameGuard<'a, A: SceneStackOps> {
        services: &'a MasterFrames,
        allocator: &'a A,
        active: bool,
    }
    impl<A: SceneStackOps> Drop for MasterFrameGuard<'_, A> {
        fn drop(&mut self) {
            if self.active {
                // SAFETY: this guard owns the matching increment and stack marks.
                let _ = unsafe { finish_with(self.services, self.allocator) };
            }
        }
    }
    pub unsafe fn master_with<A: SceneStackOps>(
        s: &MasterFrames,
        allocator: &A,
        viewport: *mut c_void,
        location: FVector,
        rotation: *const c_void,
        out: *mut *mut FSceneNode,
    ) -> Result<(), ()> {
        unsafe {
            *s.scene_count = (*s.scene_count).wrapping_add(1);
            if *s.scene_count == 1 {
                for i in 0..3 {
                    allocator.mark_save(s.stacks[i], s.master_marks[i]);
                }
            }
            let mut cleanup = MasterFrameGuard { services: s, allocator, active: true };
            let result = (|| {
                let frame = scene_alloc::<FSceneNode, A>(s.context, allocator, s.stacks[2], |storage| {
                    ok((s.frame_construct)(s.context, storage))
                })?;
                (*frame).viewport = viewport;
                let (mut x, mut y, mut level) = (0, 0, ptr::null_mut());
                ok((s.viewport)(s.context, viewport, &mut x, &mut y, &mut level))?;
                (*frame).x = x;
                (*frame).y = y;
                (*frame).xb = 0;
                (*frame).yb = 0;
                (*frame).level = level;
                (*frame).parent = ptr::null_mut();
                (*frame).sibling = ptr::null_mut();
                (*frame).child = ptr::null_mut();
                (*frame).i_surf = -1;
                (*frame).recursion = 0;
                (*frame).mirror = 1.0;
                (*frame).near_clip = FPlane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
                (*frame).draw = [ptr::null_mut(); 3];
                (*frame).sprite = ptr::null_mut();
                let spans = scene_alloc::<FSpanBuffer, A>(s.context, allocator, s.stacks[2], |storage| {
                    ok((s.span_construct)(s.context, storage))
                })?;
                (*frame).span = spans;
                span_edit(s.context, allocator, spans, empty_span(), |a, b| {
                    span::alloc_index_for_screen(a, b, x, y, s.stacks[2])
                })?;
                ok((s.coords)(s.context, frame, &location, rotation))?;
                let origin = (*frame).coords.origin;
                let mut zone = 0;
                ok((s.zone)(s.context, viewport, &origin, &mut zone))?;
                (*frame).zone_number = zone;
                out.write(frame);
                Ok(())
            })();
            if result.is_ok() {
                cleanup.active = false;
            }
            result
        }
    }
    pub unsafe fn finish(s: &Hp2ScenePipelineHelperFrames) -> Result<(), ()> {
        let frames = MasterFrames {
            context: s.context,
            scene_count: s.scene_count,
            master_marks: s.master_marks,
            stacks: s.stacks,
            viewport: s.viewport,
            frame_construct: s.frame_construct,
            span_construct: s.span_construct,
            coords: s.coords,
            zone: s.zone,
            count_failure: s.count_failure,
        };
        unsafe { finish_with(&frames, &NativeStackOps(s)) }
    }
    pub unsafe fn finish_with<A: SceneStackOps>(
        s: &MasterFrames,
        allocator: &A,
    ) -> Result<(), ()> {
        unsafe {
            *s.scene_count = (*s.scene_count).wrapping_sub(1);
            if *s.scene_count == 0 {
                for i in 0..3 {
                    allocator.mark_pop(s.master_marks[i]);
                }
            }
            if *s.scene_count < 0 {
                ok((s.count_failure)(s.context))?;
            }
            Ok(())
        }
    }
    // Native Max(A,B) is A >= B ? A : B, including unordered floating values.
    fn native_max(a: f32, b: f32) -> f32 {
        if a >= b { a } else { b }
    }
    unsafe fn bounds(frame: *mut FSceneNode, b: FScreenBounds, merging: bool) {
        unsafe {
            let xm = (b.min_x - (*frame).fx2) * -(*frame).r_proj.z;
            let xp = (b.max_x - (*frame).fx2) * (*frame).r_proj.z;
            let ym = (b.min_y - (*frame).fy2) * -(*frame).r_proj.z;
            let yp = (b.max_y - (*frame).fy2) * (*frame).r_proj.z;
            (*frame).prj_xm = if merging {
                native_max((*frame).prj_xm, xm)
            } else {
                xm
            };
            (*frame).prj_xp = if merging {
                native_max((*frame).prj_xp, xp)
            } else {
                xp
            };
            (*frame).prj_ym = if merging {
                native_max((*frame).prj_ym, ym)
            } else {
                ym
            };
            (*frame).prj_yp = if merging {
                native_max((*frame).prj_yp, yp)
            } else {
                yp
            };
        }
    }
    /// Placement lifetimes and scene sizing for the shared child-frame algorithm.
    pub trait ChildFrameOps {
        /// `storage` is aligned frame storage owned by the active scene stack.
        unsafe fn construct_frame(&self, storage: *mut c_void) -> Result<(), ()>;
        /// `storage` is aligned span-buffer storage owned by the active scene stack.
        unsafe fn construct_span(&self, storage: *mut c_void) -> Result<(), ()>;
        /// `frame` is a live child in the current scene scope.
        unsafe fn size(&self, frame: *mut FSceneNode) -> Result<(), ()>;
    }
    struct NativeChildFrameOps<'a>(&'a Hp2ScenePipelineHelperFrames);
    impl ChildFrameOps for NativeChildFrameOps<'_> {
        unsafe fn construct_frame(&self, storage: *mut c_void) -> Result<(), ()> {
            unsafe { ok((self.0.frame_construct)(self.0.context, storage)) }
        }
        unsafe fn construct_span(&self, storage: *mut c_void) -> Result<(), ()> {
            unsafe { ok((self.0.span_construct)(self.0.context, storage)) }
        }
        unsafe fn size(&self, frame: *mut FSceneNode) -> Result<(), ()> {
            unsafe { ok((self.0.size)(self.0.context, frame)) }
        }
    }
    pub unsafe fn child(
        s: &Hp2ScenePipelineHelperFrames,
        parent: *mut FSceneNode,
        spans: *const FSpanBuffer,
        level: *mut c_void,
        surf: i32,
        zone: i32,
        mirror: f32,
        near: FPlane,
        coords: FCoords,
        screen: *const FScreenBounds,
        out: *mut *mut FSceneNode,
    ) -> Result<(), ()> {
        unsafe {
            child_with(
                &NativeStackOps(s), &NativeChildFrameOps(s), s.context, s.stacks[2],
                parent, spans, level, surf, zone, mirror, near, coords, screen, out,
            )
        }
    }
    pub unsafe fn child_with<A: SceneStackOps, O: ChildFrameOps>(
        allocator: &A,
        ops: &O,
        context: *mut c_void,
        stack: *mut c_void,
        parent: *mut FSceneNode,
        spans: *const FSpanBuffer,
        level: *mut c_void,
        surf: i32,
        zone: i32,
        mirror: f32,
        near: FPlane,
        coords: FCoords,
        screen: *const FScreenBounds,
        out: *mut *mut FSceneNode,
    ) -> Result<(), ()> {
        unsafe {
            let mut frame = (*parent).child;
            while !frame.is_null() {
                if (*frame).level == level
                    && (*frame).i_surf == surf
                    && (*frame).parent == parent
                    && (*frame).near_clip == near
                    && (*frame).zone_number == zone
                {
                    merge(context, allocator, (*frame).span, spans)?;
                    if !screen.is_null() {
                        bounds(frame, screen.read(), true);
                    }
                    out.write(frame);
                    return Ok(());
                }
                frame = (*frame).sibling;
            }
            frame = scene_alloc::<FSceneNode, A>(context, allocator, stack, |storage| {
                ops.construct_frame(storage)
            })?;
            ptr::copy_nonoverlapping(parent, frame, 1);
            let new_spans = scene_alloc::<FSpanBuffer, A>(context, allocator, stack, |storage| {
                ops.construct_span(storage)
            })?;
            (*frame).span = new_spans;
            (*frame).viewport = (*parent).viewport;
            (*frame).level = level;
            (*frame).i_surf = surf;
            (*frame).zone_number = zone;
            (*frame).recursion = (*parent).recursion.wrapping_add(1);
            (*frame).mirror = mirror;
            (*frame).near_clip = near;
            (*frame).coords = coords;
            (*frame).uncoords = coords_transpose(coords);
            (*frame).draw = [ptr::null_mut(); 3];
            (*frame).sprite = ptr::null_mut();
            (*frame).parent = parent;
            (*frame).child = ptr::null_mut();
            (*frame).sibling = (*parent).child;
            (*parent).child = frame;
            ops.size(frame)?;
            if !screen.is_null() {
                bounds(frame, screen.read(), false);
            }
            span_edit(context, allocator, new_spans, empty_span(), |a, b| {
                span::alloc_index(a, b, 0, 0, stack)
            })?;
            merge(context, allocator, new_spans, spans)?;
            out.write(frame);
            Ok(())
        }
    }
    pub unsafe fn visible(
        s: &Hp2ScenePipelineHelperFrames,
        viewport: *mut c_void,
        surfaces: *mut TArrayRaw,
    ) -> Result<(), ()> {
        unsafe {
            for (pitch, yaw) in [
                (0x4000, 0),
                (0xc000, 0),
                (0, 0),
                (0, 0x8000),
                (0, 0xc000),
                (0, 0x4000),
            ] {
                let mut vector_mark = MaybeUninit::<FMemMark>::uninit();
                mem::mark_init(vector_mark.as_mut_ptr(), s.stacks[3].cast());
                ok((s.rotation)(s.context, viewport, pitch, yaw))?;
                let mut location = MaybeUninit::<FVector>::uninit();
                ok((s.location)(s.context, viewport, location.as_mut_ptr()))?;
                let rotation = [pitch, yaw, 0i32];
                let mut frame = ptr::null_mut();
                master(
                    s,
                    viewport,
                    location.assume_init(),
                    rotation.as_ptr().cast(),
                    &mut frame,
                )?;
                let mut fog = 0;
                ok((s.fog_get)(s.context, viewport, &mut fog))?;
                ok((s.fog_set)(s.context, viewport, 0))?;
                ok((s.occlude_bsp)(s.context, frame))?;
                for bin in 0..3 {
                    let mut draw = (*frame).draw[bin];
                    while !draw.is_null() {
                        let surf = (*draw).i_surf;
                        let mut found = false;
                        for i in 0..(*surfaces).array_num {
                            if *(*surfaces).data.cast::<i32>().add(i as usize) == surf {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            ok((s.add_surface)(s.context, surfaces, surf))?;
                        }
                        draw = (*draw).next;
                    }
                }
                ok((s.fog_set)(s.context, viewport, fog))?;
                finish(s)?;
                mem::mark_pop(vector_mark.assume_init_ref());
            }
            Ok(())
        }
    }
    pub unsafe fn world(
        s: &Hp2ScenePipelineHelperFrames,
        frame: *mut FSceneNode,
    ) -> Result<(), ()> {
        let frames = WorldFrames {
            stacks: s.stacks,
            frame_stamp: s.frame_stamp,
            mover_rejects: s.mover_rejects,
            editor: unsafe { *s.editor != 0 },
            show_actors: s.show_actors,
            dyn_light: s.dyn_light,
        };
        unsafe { world_with(&frames, &NativeStackOps(s), &mut NativeWorldOps(s, false), frame) }
    }
}

pub mod draw {
    use super::{Hp2SceneStackServices, push_stack};
    use crate::particle::madd;
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use core::{ffi::c_void, ptr};
    use hp2_abi::FVector;
    use hp2_abi::render_scene::*;
    use hp2_core::math::{plane_dot, vector_add, vector_scale, vector_sub, vector_transform_point};

    #[inline]
    fn madd64(a: f64, b: f64, c: f64) -> f64 {
        #[cfg(target_arch = "aarch64")]
        {
            a.mul_add(b, c)
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            a * b + c
        }
    }

    #[repr(C)]
    pub struct Hp2ScenePipelineDrawPacket {
        pub p: [*mut c_void; 12],
        pub n: [i32; 12],
        pub f: [f32; 16],
        pub d: [f64; 2],
    }
    impl Default for Hp2ScenePipelineDrawPacket {
        fn default() -> Self {
            unsafe { core::mem::zeroed() }
        }
    }
    #[repr(C)]
    pub struct Hp2ScenePipelineDrawServices {
        pub context: *mut c_void,
        pub memory: Hp2SceneStackServices,
        pub stack: *mut c_void,
        pub call: unsafe extern "C" fn(
            *mut c_void,
            u32,
            *mut FSceneNode,
            *mut Hp2ScenePipelineDrawPacket,
        ) -> i32,
    }
    /// Typed allocator and leaf host for the shared complete scene draw policy.
    /// Native FFI remains an adapter; canonical Core can implement this without
    /// calling Engine/Core object methods.
    pub trait DrawOps {
        unsafe fn push(&self, bytes: usize) -> core::result::Result<*mut u8, ()>;
        unsafe fn call(
            &self,
            op: DrawOperation,
            frame: *mut FSceneNode,
            packet: &mut Hp2ScenePipelineDrawPacket,
        ) -> core::result::Result<(), ()>;
    }
    struct NativeDrawOps<'a>(&'a Hp2ScenePipelineDrawServices);
    impl DrawOps for NativeDrawOps<'_> {
        unsafe fn push(&self, bytes: usize) -> Result<*mut u8> {
            unsafe {
                push_stack(
                    &self.0.memory,
                    self.0.context,
                    self.0.stack,
                    bytes as i32,
                    core::mem::align_of::<*mut c_void>() as i32,
                    0,
                )
                .map_err(|_| ())
            }
        }
        unsafe fn call(
            &self,
            op: DrawOperation,
            frame: *mut FSceneNode,
            packet: &mut Hp2ScenePipelineDrawPacket,
        ) -> Result {
            if unsafe { (self.0.call)(self.0.context, op as u32, frame, packet) } == 0 {
                Err(())
            } else {
                Ok(())
            }
        }
    }
    unsafe fn allocate(s: &dyn DrawOps, bytes: i32) -> Result<*mut u8> {
        unsafe { s.push(bytes as usize) }
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Corona {
        actor: *mut c_void,
        index: i32,
        bright: f32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct SpriteKey {
        sprite: *mut FDynamicSprite,
        /// `UObject::Index` of the actor's class. UE1 keyed this sort on the `UClass*`
        /// address, which made the draw order (and the render-path `appFrand` draws of
        /// particle systems) depend on heap layout; the object index groups by class the
        /// same way and is identical across runs.
        class: i32,
    }
    #[repr(C)]
    struct Hp2ScenePipelineDrawDecal {
        vertices: [FVector; 4],
        actor: *mut c_void,
        nodes: TArrayRaw,
    }
    const _: () = assert!(core::mem::size_of::<Hp2ScenePipelineDrawDecal>() == 72);
    unsafe fn decal_at(surf: *mut FBspSurf, i: usize) -> *mut Hp2ScenePipelineDrawDecal {
        unsafe {
            (*surf)
                .decals
                .data
                .cast::<Hp2ScenePipelineDrawDecal>()
                .add(i)
        }
    }
    type Result<T = ()> = core::result::Result<T, ()>;
    #[repr(u32)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum DrawOperation {
        Frame = 0,
        Clear = 2,
        Actor = 3,
        DrawSprite = 4,
        Surface = 6,
        GetTexture = 7,
        Texture = 8,
        Lock = 9,
        Unlock = 10,
        Light = 11,
        Transform = 12,
        Flat = 13,
        HitPush = 14,
        HitPop = 15,
        Complex = 16,
        LightFinish = 17,
        Clip = 19,
        Triangles = 20,
        DecalTime = 21,
        CacheGet = 22,
        CacheCreate = 23,
        CacheUnlock = 24,
        Now = 25,
        Trace = 26,
        ActorIndex = 27,
        CoronaSources = 28,
        LevelActor = 29,
        Icon = 30,
        Sky = 31,
        Finish = 32,
        Distance = 33,
        Lumos = 34,
        Debug = 35,
        Log = 36,
        Flare = 37,
        Valid = 38,
        GetDefaultTexture = 39,
        WorldTextureLod = 40,
        DecalTimeEnd = 41,
    }
    const FRAME: DrawOperation = DrawOperation::Frame;
    const CLEAR: DrawOperation = DrawOperation::Clear;
    const ACTOR: DrawOperation = DrawOperation::Actor;
    const DRAW_SPRITE: DrawOperation = DrawOperation::DrawSprite;
    const SURFACE: DrawOperation = DrawOperation::Surface;
    const GET_TEXTURE: DrawOperation = DrawOperation::GetTexture;
    const TEXTURE: DrawOperation = DrawOperation::Texture;
    const LOCK: DrawOperation = DrawOperation::Lock;
    const UNLOCK: DrawOperation = DrawOperation::Unlock;
    const LIGHT: DrawOperation = DrawOperation::Light;
    const TRANSFORM: DrawOperation = DrawOperation::Transform;
    const FLAT: DrawOperation = DrawOperation::Flat;
    const HIT_PUSH: DrawOperation = DrawOperation::HitPush;
    const HIT_POP: DrawOperation = DrawOperation::HitPop;
    const COMPLEX: DrawOperation = DrawOperation::Complex;
    const LIGHT_FINISH: DrawOperation = DrawOperation::LightFinish;
    const CLIP: DrawOperation = DrawOperation::Clip;
    const TRIANGLES: DrawOperation = DrawOperation::Triangles;
    const DECAL_TIME: DrawOperation = DrawOperation::DecalTime;
    const DECAL_TIME_END: DrawOperation = DrawOperation::DecalTimeEnd;
    const CACHE_GET: DrawOperation = DrawOperation::CacheGet;
    const CACHE_CREATE: DrawOperation = DrawOperation::CacheCreate;
    const CACHE_UNLOCK: DrawOperation = DrawOperation::CacheUnlock;
    const NOW: DrawOperation = DrawOperation::Now;
    const TRACE: DrawOperation = DrawOperation::Trace;
    const ACTOR_INDEX: DrawOperation = DrawOperation::ActorIndex;
    const CORONA_SOURCES: DrawOperation = DrawOperation::CoronaSources;
    const LEVEL_ACTOR: DrawOperation = DrawOperation::LevelActor;
    const ICON: DrawOperation = DrawOperation::Icon;
    const SKY: DrawOperation = DrawOperation::Sky;
    const FINISH: DrawOperation = DrawOperation::Finish;
    const DISTANCE: DrawOperation = DrawOperation::Distance;
    const LUMOS: DrawOperation = DrawOperation::Lumos;
    const DEBUG: DrawOperation = DrawOperation::Debug;
    const LOG: DrawOperation = DrawOperation::Log;
    const FLARE: DrawOperation = DrawOperation::Flare;
    const VALID: DrawOperation = DrawOperation::Valid;
    unsafe fn call(
        s: &dyn DrawOps,
        frame: *mut FSceneNode,
        op: DrawOperation,
        p: &[*mut c_void],
        n: &[i32],
        f: &[f32],
    ) -> Result<Hp2ScenePipelineDrawPacket> {
        let mut q = Hp2ScenePipelineDrawPacket::default();
        q.p[..p.len()].copy_from_slice(p);
        q.n[..n.len()].copy_from_slice(n);
        q.f[..f.len()].copy_from_slice(f);
        unsafe { s.call(op, frame, &mut q) }?;
        Ok(q)
    }
    // An unactivated appearing surface must not draw. A zero-contribution
    // blend is not guaranteed when its texture contains non-opaque alpha.
    fn pending_lumos_appear(flags: u32) -> bool {
        flags & super::LUMOS == super::LUMOS
            && flags & 0x00200000 != 0
            && flags & super::LUMOS_ON == 0
    }
    unsafe fn lumos_player(s: &dyn DrawOps, frame: *mut FSceneNode) -> Result<Option<Hp2ScenePipelineDrawPacket>> {
        let player = unsafe { call(s, frame, LUMOS, &[], &[0, 0, 0, 0, 0, 1], &[]) }?;
        if player.n[3] == 0 {
            return Ok(None);
        }
        let state = unsafe { call(s, frame, LUMOS, &[], &[0, 0, 0, player.n[3], 0, 2], &[]) }?;
        Ok(Some(state))
    }
    #[cfg(test)]
    mod lumos_contract_tests {
        use super::*;
        use std::cell::{Cell, RefCell};

        #[repr(C)]
        struct TestPoly {
            header: hp2_abi::render_scene::FSavedPoly,
            points: [*mut hp2_abi::render_particle::FTransform; 1],
        }

        struct SurfaceHost {
            surf: *mut FBspSurf,
            info: *mut FSurfaceInfo,
            facet: *mut FSurfaceFacet,
            flags: u32,
            lumos_on: bool,
            complex: RefCell<Vec<(u32, i32)>>,
        }
        impl DrawOps for SurfaceHost {
            unsafe fn push(&self, _: usize) -> Result<*mut u8> { Err(()) }
            unsafe fn call(&self, op: DrawOperation, _: *mut FSceneNode, p: &mut Hp2ScenePipelineDrawPacket) -> Result {
                match op {
                    DrawOperation::Surface => {
                        p.p[0] = self.surf.cast();
                        p.p[1] = self.info.cast();
                        p.p[2] = self.facet.cast();
                    }
                    DrawOperation::Lumos => match p.n[5] {
                        0 => { p.n[0] = self.flags as i32; p.n[1] = 1; }
                        1 => p.n[3] = 1,
                        2 => { p.n[4] = i32::from(self.lumos_on); p.f[0] = 512.0; }
                        _ => panic!("unexpected Lumos query"),
                    },
                    DrawOperation::Distance => p.f[0] = 204.8,
                    DrawOperation::Complex => self.complex.borrow_mut().push((p.n[0] as u32, p.n[1])),
                    DrawOperation::GetDefaultTexture | DrawOperation::Texture | DrawOperation::Lock
                    | DrawOperation::Unlock | DrawOperation::Frame | DrawOperation::Transform
                    | DrawOperation::HitPush | DrawOperation::HitPop | DrawOperation::Debug => {}
                    _ => panic!("unexpected surface operation: {op:?}"),
                }
                Ok(())
            }
        }

        fn submitted_surface(flags: u32, lumos_on: bool) -> Vec<(u32, i32)> {
            // The saved polygon has one live tail pointer, as in the real ABI.
            let mut point: hp2_abi::render_particle::FTransform = unsafe { std::mem::zeroed() };
            let mut poly = TestPoly {
                header: unsafe { std::mem::zeroed() },
                points: [&mut point],
            };
            poly.header.num_pts = 1;
            let mut facet: FSurfaceFacet = unsafe { std::mem::zeroed() };
            facet.polys = &mut poly.header;
            let mut surf: FBspSurf = unsafe { std::mem::zeroed() };
            surf.poly_flags = flags;
            surf.i_light_map = -1;
            let mut info: FSurfaceInfo = unsafe { std::mem::zeroed() };
            info.poly_flags = flags;
            let mut draw: FBspDrawList = unsafe { std::mem::zeroed() };
            let host = SurfaceHost {
                surf: &mut surf, info: &mut info, facet: &mut facet,
                flags, lumos_on, complex: RefCell::new(Vec::new()),
            };
            unsafe { surface(&host, std::ptr::null_mut(), &mut draw, 2) }.unwrap();
            host.complex.into_inner()
        }

        #[test]
        fn hardware_surface_submission_preserves_persistent_and_fading_lumos() {
            let marker = super::super::LUMOS | 0x8000 | 8;
            let appear = 0x00200000;
            let activated = super::super::LUMOS_ON;
            let occlude = 0x80000000;
            for lumos_on in [false, true] {
                assert!(submitted_surface(marker | appear, lumos_on).is_empty());
                assert_eq!(submitted_surface(marker | appear | activated | occlude, lumos_on),
                    vec![(marker | appear | activated, 127)]);
                assert_eq!(submitted_surface(marker, lumos_on), vec![(marker | occlude, 0)]);
                // The existing f32 distance calculation truncates this value to 127.
                assert_eq!(submitted_surface(marker | activated | occlude, lumos_on),
                    vec![(marker | activated, 127)]);
            }
        }

        struct OcclusionHost {
            allocations: RefCell<Vec<Box<[u128]>>>,
            points: [*mut hp2_abi::render_particle::FTransform; 3],
            updates: RefCell<Vec<i32>>,
            span_available: Cell<bool>,
        }
        impl crate::scene_pipeline::OcclusionOps for OcclusionHost {
            unsafe fn push(&self, _: *mut c_void, bytes: usize, _: i32, _: u32) -> std::result::Result<*mut u8, ()> {
                let mut allocation = vec![0u128; (bytes + 15) / 16].into_boxed_slice();
                let pointer = allocation.as_mut_ptr().cast();
                self.allocations.borrow_mut().push(allocation);
                Ok(pointer)
            }
            unsafe fn checked_size(&self, bytes: usize, _: u32) -> std::result::Result<usize, ()> { Ok(bytes) }
            unsafe fn call(&self, operation: crate::scene_pipeline::OcclusionOperation, _: *mut crate::scene_pipeline::Hp2ScenePipelineData, p: &mut crate::scene_pipeline::Hp2ScenePipelineArgs) -> std::result::Result<(), ()> {
                use crate::scene_pipeline::OcclusionOperation as O;
                match operation {
                    O::InitializeDrawList => unsafe { p.pointers[0].cast::<FBspDrawList>().write(std::mem::zeroed()) },
                    O::WarpInfo => { p.integers[0] = 0; p.pointers[1] = std::ptr::null_mut(); p.pointers[2] = std::ptr::null_mut(); }
                    O::SkyInfo => p.pointers[0] = std::ptr::null_mut(),
                    O::ViewZone => p.integers[0] = 0,
                    O::SurfaceVisible => p.integers[0] = 1,
                    O::Clip => { p.integers[0] = 3; p.pointers[0] = self.points.as_ptr().cast_mut().cast(); }
                    O::Raster => { p.integers[0] = 1; p.integers[1] = 0; p.integers[2] = 1; p.integers[3] = 0; p.integers[4] = 1; }
                    O::CopySpan => {
                        let update = p.integers[0];
                        self.updates.borrow_mut().push(update);
                        let visible = self.span_available.get();
                        if update != 0 && visible { self.span_available.set(false); }
                        p.integers[0] = visible as i32;
                    }
                    O::ZoneActor => p.pointers[0] = std::ptr::null_mut(),
                    O::ObjectIndex => p.integers[0] = 0,
                    O::Statistics | O::Snapshot | O::AllocateDynamics | O::Initialize | O::InitializeSpan
                    | O::AllocateSpan | O::ReleaseSpan => {}
                    _ => panic!("unexpected occlusion operation: {operation:?}"),
                }
                Ok(())
            }
        }

        // CopySpan models one fully overlapping span; traversal and binning are real.
        fn occluded_surfaces(flags: &[u32], capabilities: u32) -> (Vec<i32>, Vec<(usize, i32, u32)>) {
            use crate::scene_pipeline::{self as scene, Hp2ScenePipelineData};
            let mut point: hp2_abi::render_particle::FTransform = unsafe { std::mem::zeroed() };
            let host = OcclusionHost { allocations: RefCell::new(Vec::new()), points: [&mut point; 3], updates: RefCell::new(Vec::new()), span_available: Cell::new(true) };
            let mut node_data: Vec<FBspNode> = (0..flags.len()).map(|i| {
                let mut node: FBspNode = unsafe { std::mem::zeroed() };
                node.plane.x = 1.0; node.i_back = -1; node.i_front = -1;
                node.i_plane = if i + 1 < flags.len() { (i + 1) as i32 } else { -1 };
                node.i_surf = i as i32; node.i_render_bound = -1; node.i_collision_bound = -1; node.num_vertices = 3;
                node
            }).collect();
            let mut surf_data: Vec<FBspSurf> = flags.iter().map(|&flags| {
                let mut surf: FBspSurf = unsafe { std::mem::zeroed() };
                surf.poly_flags = flags;
                surf
            }).collect();
            let mut nodes: TArrayRaw = unsafe { std::mem::zeroed() };
            nodes.data = node_data.as_mut_ptr().cast(); nodes.array_num = flags.len() as i32; nodes.array_max = nodes.array_num;
            let mut surfs: TArrayRaw = unsafe { std::mem::zeroed() };
            surfs.data = surf_data.as_mut_ptr().cast(); surfs.array_num = flags.len() as i32; surfs.array_max = surfs.array_num;
            let mut dyn_cache: Vec<FDynamicsCache> = (0..flags.len()).map(|_| unsafe { std::mem::zeroed() }).collect();
            let mut dynamics: TArrayRaw = unsafe { std::mem::zeroed() };
            dynamics.data = dyn_cache.as_mut_ptr().cast(); dynamics.array_num = flags.len() as i32; dynamics.array_max = dynamics.array_num;
            let mut zones: [FZoneProperties; 64] = unsafe { std::mem::zeroed() };
            let mut span: hp2_abi::render_span::FSpanBuffer = unsafe { std::mem::zeroed() };
            span.valid_lines = 1;
            let mut frame: FSceneNode = unsafe { std::mem::zeroed() };
            frame.coords.origin.x = 2.0; frame.span = &mut span;
            let mut volumetric: *mut FVolActorLink = std::ptr::null_mut();
            let mut stamp = 0u32; let mut nodes_draw = 0u32; let mut polys_draw = 0u32;
            let mut data: Hp2ScenePipelineData = unsafe { std::mem::zeroed() };
            data.frame = (&mut frame as *mut FSceneNode).cast();
            data.nodes = (&mut nodes as *mut TArrayRaw).cast(); data.surfs = (&mut surfs as *mut TArrayRaw).cast();
            data.dynamics = (&mut dynamics as *mut TArrayRaw).cast(); data.zones = zones.as_mut_ptr().cast();
            data.first_volumetric = (&mut volumetric as *mut *mut FVolActorLink).cast();
            data.stamp = &mut stamp; data.nodes_draw = &mut nodes_draw; data.polys_draw = &mut polys_draw;
            data.capabilities = capabilities;
            unsafe { scene::occlude_bsp_with(&mut data, &host) }.unwrap();
            let mut drawn = Vec::new();
            for bin in 0..3 {
                let mut entry = frame.draw[bin];
                while !entry.is_null() {
                    unsafe { drawn.push((bin, (*entry).i_surf, (*entry).poly_flags)); entry = (*entry).next; }
                }
            }
            (host.updates.into_inner(), drawn)
        }

        fn occluded_surface(flags: u32, capabilities: u32) -> (Vec<i32>, Option<(usize, u32)>) {
            let (updates, drawn) = occluded_surfaces(&[flags], capabilities);
            (updates, drawn.first().map(|&(bin, _, flags)| (bin, flags)))
        }

        #[test]
        fn texture_or_extra_flags_can_prevent_occlusion_promotion() {
            use crate::scene_pipeline::{occlusion_surface_flags, LUMOS, LUMOS_ENABLED, LUMOS_ON, PLAYER};
            let caps = PLAYER | LUMOS_ENABLED;
            let marker = LUMOS | 0x8000 | 8;
            for added in [LUMOS_ON, 0x00200000, 1, 2, 4, 0x80, 0x10000000] {
                assert_eq!(occlusion_surface_flags(marker, marker | added, caps), marker | added);
            }
        }

        #[test]
        fn overlapping_bsp_surfaces_respect_span_update_and_traversal_order() {
            use crate::scene_pipeline::{LUMOS, LUMOS_ENABLED, LUMOS_ON, PLAYER};
            let caps = PLAYER | LUMOS_ENABLED;
            let inactive = LUMOS | 0x8000 | 8;
            let active = inactive | LUMOS_ON;
            let backing = 0x8000 | 8;
            let (updates, draws) = occluded_surfaces(&[inactive, backing], caps);
            assert_eq!(updates, vec![1, 1]);
            assert_eq!(draws, vec![(1, 0, inactive)]);
            let (updates, draws) = occluded_surfaces(&[active, backing], caps);
            assert_eq!(updates, vec![0, 1]);
            assert_eq!(draws, vec![(1, 1, backing), (2, 0, active)]);
            for lumos in [inactive, active] {
                let (updates, draws) = occluded_surfaces(&[backing, lumos], caps);
                assert_eq!(updates[0], 1);
                assert_eq!(draws, vec![(1, 0, backing)]);
            }
        }

        #[test]
        fn bsp_occlusion_promotes_only_inactive_opaque_disappearing_surfaces() {
            use crate::scene_pipeline::{EDITOR, LUMOS_ENABLED, LUMOS_ON, PLAYER, SPAN};
            let marker = super::super::LUMOS | 0x8000 | 8;
            let caps = PLAYER | LUMOS_ENABLED;
            let (updates, draw) = occluded_surface(marker, caps);
            assert_eq!(updates, vec![1]);
            assert_eq!(draw, Some((1, marker)), "draw flags must retain the Lumos marker");
            for flags in [marker | 0x00200000, marker | LUMOS_ON, marker | 1, marker | 2,
                          marker | 4, marker | 0x80, marker | 0x10000000, marker & !0x00800000] {
                let (updates, draw) = occluded_surface(flags, caps);
                // Without PLAYER_CTRL the existing poly mask clears INV before
                // storing draw flags; it must still prevent Lumos promotion.
                assert_eq!(updates, vec![0], "flags {flags:#x}");
                assert_eq!(draw, Some((2, flags & !1)));
            }
            for capabilities in [LUMOS_ENABLED, PLAYER, caps | EDITOR, caps | SPAN] {
                let (updates, draw) = occluded_surface(marker, capabilities);
                assert_eq!(updates, vec![0], "capabilities {capabilities:#x}");
                assert_eq!(draw, Some((2, marker)));
            }
        }

        struct Host { is_player: bool, state_calls: Cell<u32> }
        impl DrawOps for Host {
            unsafe fn push(&self, _: usize) -> Result<*mut u8> { Err(()) }
            unsafe fn call(&self, op: DrawOperation, _: *mut FSceneNode, p: &mut Hp2ScenePipelineDrawPacket) -> Result {
                assert!(op == LUMOS);
                match p.n[5] {
                    1 => p.n[3] = i32::from(self.is_player),
                    2 => {
                        self.state_calls.set(self.state_calls.get() + 1);
                        if p.n[3] != 0 { p.n[4] = 1; p.f[0] = 512.0; }
                    }
                    _ => panic!("unexpected lumos query"),
                }
                Ok(())
            }
        }

        #[test]
        fn unactivated_appearing_surface_is_the_only_hardware_skip() {
            let marker = super::super::LUMOS;
            let appear = 0x00200000;
            let activated = super::super::LUMOS_ON;
            assert!(pending_lumos_appear(marker | appear));
            assert!(pending_lumos_appear(marker | appear | 0x8000 | 8));
            assert!(!pending_lumos_appear(marker | appear | activated));
            assert!(!pending_lumos_appear(marker));
            assert!(!pending_lumos_appear(marker | activated));
            assert!(!pending_lumos_appear(appear | 0x40));
            assert!(!pending_lumos_appear(appear | 0x00800000));
            assert!(!pending_lumos_appear(appear));
        }

        #[test]
        fn lumos_state_query_preserves_player_identity() {
            let host = Host { is_player: true, state_calls: Cell::new(0) };
            let state = unsafe { lumos_player(&host, std::ptr::null_mut()) }.unwrap().unwrap();
            assert_eq!(state.n[4], 1);
            assert_eq!(state.f[0], 512.0);
            assert_eq!(host.state_calls.get(), 1);
            let nonplayer = Host { is_player: false, state_calls: Cell::new(0) };
            assert!(unsafe { lumos_player(&nonplayer, std::ptr::null_mut()) }.unwrap().is_none());
            assert_eq!(nonplayer.state_calls.get(), 0);
        }
    }
    fn vec(f: &[f32]) -> FVector {
        FVector {
            x: f[0],
            y: f[1],
            z: f[2],
        }
    }
    // Match UnTemplate::Sort, including exchanges of equal keys. Rust's sort
    // intentionally has different equal-key ordering and must not replace this.
    unsafe fn sort<T: Copy>(a: *mut T, len: usize, mut cmp: impl FnMut(T, T) -> i32) {
        if len < 2 {
            return;
        }
        let mut stack = [(0usize, 0usize); 32];
        let mut count = 1;
        stack[0] = (0, len - 1);
        while count != 0 {
            count -= 1;
            let (mut lo, mut hi) = stack[count];
            loop {
                let n = hi - lo + 1;
                if n <= 8 {
                    while hi > lo {
                        let mut max = lo;
                        for i in lo + 1..=hi {
                            if cmp(unsafe { *a.add(i) }, unsafe { *a.add(max) }) > 0 {
                                max = i
                            }
                        }
                        unsafe { ptr::swap(a.add(max), a.add(hi)) };
                        hi -= 1;
                    }
                    break;
                }
                unsafe { ptr::swap(a.add(lo + n / 2), a.add(lo)) };
                let (mut l, mut r) = (lo, hi + 1);
                loop {
                    l += 1;
                    while l <= hi && cmp(unsafe { *a.add(l) }, unsafe { *a.add(lo) }) <= 0 {
                        l += 1
                    }
                    r -= 1;
                    while r > lo && cmp(unsafe { *a.add(r) }, unsafe { *a.add(lo) }) >= 0 {
                        r -= 1
                    }
                    if l > r {
                        break;
                    }
                    unsafe { ptr::swap(a.add(l), a.add(r)) };
                }
                unsafe { ptr::swap(a.add(lo), a.add(r)) };
                if (r as isize - 1 - lo as isize) >= (hi as isize - l as isize) {
                    if lo + 1 < r {
                        stack[count] = (lo, r - 1);
                        count += 1
                    }
                    if hi > l {
                        lo = l;
                        continue;
                    }
                } else {
                    if hi > l {
                        stack[count] = (l, hi);
                        count += 1
                    }
                    if lo + 1 < r {
                        hi = r - 1;
                        continue;
                    }
                }
                break;
            }
        }
    }
    unsafe fn actor(
        s: &dyn DrawOps,
        f: *mut FSceneNode,
        a: *mut c_void,
    ) -> Result<Hp2ScenePipelineDrawPacket> {
        unsafe { call(s, f, ACTOR, &[a], &[], &[]) }
    }
    unsafe fn texture(
        s: &dyn DrawOps,
        f: *mut FSceneNode,
        t: *mut c_void,
    ) -> Result<*mut c_void> {
        Ok(unsafe {
            call(
                s,
                f,
                if t.is_null() { DrawOperation::GetDefaultTexture } else { GET_TEXTURE },
                &[t],
                &[],
                &[],
            )
        }?
        .p[0])
    }
    unsafe fn add_corona(
        s: &dyn DrawOps,
        f: *mut FSceneNode,
        c: *mut Corona,
        free: &mut usize,
        a: *mut c_void,
        delta: f32,
    ) -> Result {
        let v = unsafe { actor(s, f, a) }?;
        if v.n[3] == 0 || v.p[2].is_null() || v.n[4] != 0 {
            return Ok(());
        }
        if unsafe { call(s, f, TRACE, &[a], &[], &[]) }?.n[0] == 0 {
            return Ok(());
        }
        for i in 0..32 {
            if unsafe { (*c.add(i)).actor } == a {
                let bright = madd(2.0, delta, unsafe { (*c.add(i)).bright });
                unsafe { (*c.add(i)).bright = if 1.0 < bright { 1.0 } else { bright } };
                return Ok(());
            }
        }
        while *free < 32 && !unsafe { (*c.add(*free)).actor }.is_null() {
            *free += 1
        }
        if *free < 32 {
            // Preserve publication before the SDK actor-index lookup.
            unsafe { (*c.add(*free)).actor = a };
            let index = unsafe { call(s, f, ACTOR_INDEX, &[a], &[], &[]) }?.n[0];
            unsafe {
                (*c.add(*free)).index = index;
                (*c.add(*free)).bright = if 1.0 < 2.0 * delta { 1.0 } else { 2.0 * delta }
            };
        }
        Ok(())
    }
    static TIME_READY: AtomicBool = AtomicBool::new(false);
    static LAST_TIME: AtomicU64 = AtomicU64::new(0);
    unsafe fn coronas(s: &dyn DrawOps, f: *mut FSceneNode) -> Result {
        let info = unsafe { call(s, f, FRAME, &[], &[], &[]) }?;
        if info.n[4] == -1
            || info.p[4].is_null()
            || info.n[5] == 0
            || unsafe { (*f).recursion } != 0
        {
            return Ok(());
        }
        let mut cache = unsafe { call(s, f, CACHE_GET, &[], &[], &[]) }?;
        if cache.p[0].is_null() {
            cache = unsafe { call(s, f, CACHE_CREATE, &[], &[], &[]) }?;
            for i in 0..32 {
                unsafe { (*cache.p[0].cast::<Corona>().add(i)).actor = ptr::null_mut() }
            }
        }
        let c = cache.p[0].cast::<Corona>();
        if !TIME_READY.load(Ordering::Relaxed) {
            LAST_TIME.store(
                unsafe { call(s, f, NOW, &[], &[], &[]) }?.d[0].to_bits(),
                Ordering::Relaxed,
            );
            TIME_READY.store(true, Ordering::Relaxed);
        }
        let now = unsafe { call(s, f, NOW, &[], &[], &[]) }?.d[0].to_bits() as i64;
        let delta = 3.0
            * (now.wrapping_sub(LAST_TIME.load(Ordering::Relaxed) as i64) as f32
                / 4294967296.0_f32);
        LAST_TIME.store(
            unsafe { call(s, f, NOW, &[], &[], &[]) }?.d[0].to_bits(),
            Ordering::Relaxed,
        );
        for i in 0..32 {
            unsafe {
                if !(*c.add(i)).actor.is_null() {
                    (*c.add(i)).bright -= delta;
                    if (*c.add(i)).bright < 0.0 {
                        (*c.add(i)).actor = ptr::null_mut()
                    }
                }
            }
        }
        let mut free = 0;
        let sources = unsafe { call(s, f, CORONA_SOURCES, &[], &[], &[]) }?;
        if sources.n[0] != -1 {
            let mut i = sources.n[0];
            let lights = sources.p[0].cast::<TArrayRaw>();
            loop {
                // The TArray header belongs to the model; reacquire its data
                // after each add_corona callback, never retain an element borrow.
                let light = unsafe { *(*lights).data.cast::<*mut c_void>().add(i as usize) };
                if light.is_null() {
                    break;
                }
                unsafe { add_corona(s, f, c, &mut free, light, delta) }?;
                i += 1;
            }
        }
        let mut link = sources.p[1].cast::<FVolActorLink>();
        while !link.is_null() {
            let a = unsafe { (*link).actor };
            unsafe { add_corona(s, f, c, &mut free, a, delta) }?;
            link = unsafe { (*link).next }
        }
        for i in 0..32 {
            // Empty cache slots initialize only actor, not index or brightness.
            if unsafe { (*c.add(i)).actor }.is_null() {
                continue;
            }
            let entry = unsafe { *c.add(i) };
            let count = unsafe { call(s, f, LEVEL_ACTOR, &[], &[-1], &[]) }?.n[0];
            if entry.index > count {
                continue;
            }
            if unsafe { call(s, f, LEVEL_ACTOR, &[], &[entry.index, 1], &[]) }?.p[0] != entry.actor
            {
                continue;
            }
            unsafe { call(s, f, VALID, &[entry.actor], &[], &[]) }?;
            let light = unsafe { actor(s, f, entry.actor) }?;
            let loc = vector_transform_point(vec(&light.f), unsafe { (*f).coords });
            if !(loc.z > 1.0) {
                continue;
            }
            let h = light.n[5] as f32;
            let hue = if h < 86.0 {
                [(85.0 - h) / 85.0, h / 85.0, 0.0]
            } else if h < 171.0 {
                [0.0, (170.0 - h) / 85.0, (h - 85.0) / 85.0]
            } else {
                [(h - 170.0) / 85.0, 0.0, (255.0 - h) / 84.0]
            };
            let alpha = light.n[6] as f32 / 255.0;
            let rz = unsafe { (*f).proj.z } / loc.z;
            let x = madd(loc.x, rz, unsafe { (*f).fx2 });
            let y = madd(loc.y, rz, unsafe { (*f).fy2 });
            let scale = 512.0 * light.f[4] * unsafe { (*f).x } as f32 / 640.0;
            let legend = info.n[8] != 0;
            // Native FVector operators call the existing Rust ABI separately;
            // preserve those rounded vector operations rather than fusing them.
            let hue = vec(&hue);
            let color = vector_add(
                hue,
                vector_scale(
                    vector_sub(
                        FVector {
                            x: 1.0,
                            y: 1.0,
                            z: 1.0,
                        },
                        hue,
                    ),
                    alpha,
                ),
            );
            if legend && light.n[7] != 0 {
                let mut j = 0;
                loop {
                    let count = unsafe { call(s, f, FLARE, &[entry.actor], &[], &[]) }?.n[0];
                    if j >= count {
                        break;
                    }
                    let q = unsafe { call(s, f, FLARE, &[entry.actor], &[j, 1], &[]) }?;
                    if q.p[0].is_null() {
                        break;
                    }
                    let xx = madd((q.f[2] as i32 / 2) as f32 - x, -q.f[0], x);
                    let yy = madd((q.f[3] as i32 / 2) as f32 - y, -q.f[0], y);
                    let ax = scale / 2.0;
                    let ay = (scale - ((unsafe { (*f).x } - unsafe { (*f).y }) / 2) as f32) / 2.0;
                    if j == 0 && (xx < ax || yy < ay || xx > q.f[4] - ax || yy > q.f[5] - ay) {
                        break;
                    }
                    let sc = (scale as f64 * 0.2 * q.f[1] as f64) as f32;
                    let brightness = unsafe { (*c.add(i)).bright } * rz;
                    let tint = vector_scale(color, brightness);
                    unsafe {
                        call(
                            s,
                            f,
                            ICON,
                            &[q.p[0]],
                            &[],
                            &[
                                madd(-0.5, sc, xx),
                                madd(-0.5, sc, yy),
                                sc,
                                tint.x,
                                tint.y,
                                tint.z,
                            ],
                        )
                    }?;
                    j += 1;
                }
            }
            let skin = if legend {
                unsafe { actor(s, f, entry.actor) }?.p[2]
            } else {
                light.p[2]
            };
            let brightness = if legend {
                (unsafe { (*c.add(i)).bright }) * rz
            } else {
                unsafe { (*c.add(i)).bright }
            };
            let tint = vector_scale(color, brightness);
            unsafe {
                call(
                    s,
                    f,
                    ICON,
                    &[skin],
                    &[],
                    &[
                        madd(-0.5, scale, x),
                        madd(-0.5, scale, y),
                        scale,
                        tint.x,
                        tint.y,
                        tint.z,
                    ],
                )
            }?;
        }
        unsafe { call(s, f, CACHE_UNLOCK, &[cache.p[1]], &[], &[]) }?;
        Ok(())
    }
    unsafe fn surface(
        s: &dyn DrawOps,
        f: *mut FSceneNode,
        draw: *mut FBspDrawList,
        pass: i32,
    ) -> Result {
        let q = unsafe { call(s, f, SURFACE, &[draw.cast()], &[], &[]) }?;
        let surf = q.p[0].cast::<FBspSurf>();
        let si = q.p[1].cast::<FSurfaceInfo>();
        let facet = q.p[2].cast::<FSurfaceFacet>();
        let flags = unsafe { (*surf).poly_flags };
        let t = unsafe { texture(s, f, (*surf).texture) }?;
        let mut u = unsafe { (*surf).pan_u } as f32;
        let mut v = unsafe { (*surf).pan_v } as f32;
        if flags & 0x200 != 0 {
            u += ((q.f[0] * 35.0 * q.f[1] * 256.0) as i32 & 0x3ffff) as f32 / 256.0
        }
        if flags & 0x400 != 0 {
            v += ((q.f[0] * 35.0 * q.f[2] * 256.0) as i32 & 0x3ffff) as f32 / 256.0
        }
        if flags & 0x2000 != 0 {
            let time = q.f[0];
            let sin = hp2_core::ansi_math::sin;
            let cos = hp2_core::ansi_math::cos;
            u = (u as f64 + madd64(8.0, sin(time as f64), 4.0 * cos((2.3 * time) as f64))) as f32;
            v = (v as f64 + madd64(8.0, cos(time as f64), 4.0 * sin((2.3 * time) as f64))) as f32;
        }
        unsafe { call(s, f, LOCK, &[t], &[0], &[-u, -v]) }?;
        let tv = unsafe { call(s, f, TEXTURE, &[t], &[], &[]) }?;
        if !tv.p[0].is_null()
            && unsafe { (*si).poly_flags } & 0x04000000 == 0
            && !tv.p[2].is_null()
            && unsafe { call(s, f, DrawOperation::WorldTextureLod, &[tv.p[2]], &[], &[]) }?.n[0] == 0
        {
            unsafe { call(s, f, LOCK, &[tv.p[0]], &[1], &[]) }?;
        }
        let tv = unsafe { call(s, f, TEXTURE, &[t], &[], &[]) }?;
        if !tv.p[1].is_null() {
            unsafe { call(s, f, LOCK, &[tv.p[1]], &[2], &[]) }?;
        }
        let fi = unsafe { call(s, f, FRAME, &[], &[], &[]) }?;
        if unsafe { (*surf).i_light_map } != -1 && fi.n[1] == 5 && fi.n[2] != 0 && fi.n[3] == 0 {
            unsafe { call(s, f, LIGHT, &[draw.cast()], &[(pass == 0) as i32], &[]) }?;
        }
        unsafe { call(s, f, TRANSFORM, &[], &[], &[]) }?;
        if fi.n[1] == 2 || fi.n[1] == 3 || fi.n[1] == 4 {
            let fq = unsafe { call(s, f, FLAT, &[draw.cast()], &[0], &[]) }?;
            let ft = unsafe { texture(s, f, fq.p[0]) }?;
            let fq = unsafe { call(s, f, FLAT, &[ft], &[1], &[]) }?;
            let mut color = [0.0; 3];
            if fi.n[1] == 3 {
                let i = fq.n[0];
                color = [
                    (i.wrapping_mul(67) & 255) as f32 / 256.0,
                    (i.wrapping_mul(1371) & 255) as f32 / 256.0,
                    (i.wrapping_mul(1991) & 255) as f32 / 256.0,
                ]
            } else if fi.n[1] == 4 {
                let h = unsafe { (*draw).i_node } / 32;
                let hsv = unsafe { call(s, f, FLAT, &[], &[2, h], &[]) }?;
                color.copy_from_slice(&hsv.f[..3]);
            } else {
                let zone = unsafe { (*draw).i_zone };
                if zone == 0 {
                    color.copy_from_slice(&fq.f[..3])
                } else {
                    color = [
                        ((zone * 67) & 255) as f32 / 256.0,
                        ((zone * 1371) & 255) as f32 / 256.0,
                        ((zone * 1991) & 255) as f32 / 256.0,
                    ]
                };
                for c in &mut color {
                    *c *= 0.5 + (unsafe { (*draw).i_node } & 7) as f32 / 16.0
                }
            }
            unsafe { call(s, f, FLAT, &[], &[3], &color) }?;
            unsafe { (*si).poly_flags |= 0x40000000 };
        }
        unsafe { call(s, f, HIT_PUSH, &[draw.cast()], &[], &[]) }?;
        let lumos = unsafe { call(s, f, LUMOS, &[draw.cast()], &[], &[]) }?;
        let lf = lumos.n[0] as u32;
        if lumos.n[1] != 0 && lf & 0x00800040 != 0 && lumos.n[2] == 0 {
            if let Some(lumos) = unsafe { lumos_player(s, f) }? {
                let mut lf = lf;
                let mut alpha = if lf & 0x00200000 != 0 { 255 } else { 0 };
                let mut render = true;
                if fi.n[0] != 0 {
                    if lf & 0x40000 != 0 {
                        lf |= 4
                    } else if lumos.n[4] == 0 && lf & 0x00200000 != 0 {
                        render = false
                    }
                    lf &= !(0x00800040 | 0x00200000 | 0x40000);
                } else if pending_lumos_appear(lf) {
                    render = false;
                } else if lf & 0x40000 != 0 {
                    let poly = unsafe { (*facet).polys };
                    if poly.is_null() {
                        render = false
                    } else {
                        let mut distance = 99999.0;
                        for i in 0..unsafe { (*poly).num_pts } as usize {
                            let point = unsafe { *(*poly).pts.as_ptr().add(i) };
                            let d =
                                unsafe { call(s, f, DISTANCE, &[point.cast()], &[], &[]) }?.f[0];
                            if d < distance {
                                distance = d
                            }
                        }
                        let radius = lumos.f[0] * 0.8;
                        let ratio = distance / radius;
                        let min = hp2_core::math::positive_min(1.0, ratio);
                        lf &= !0x80000000;
                        if lf & 0x00200000 != 0 {
                            alpha = (255.0 * min) as i32;
                            if alpha < 100 {
                                alpha = 100
                            }
                        } else {
                            alpha = madd(-255.0, min, 255.0) as i32;
                            if alpha > 200 {
                                alpha = 200
                            } else if alpha == 0 {
                                lf |= 0x80000000
                            }
                        }
                    }
                } else if lf & 0x00200000 == 0 {
                    lf |= 0x80000000
                }
                if render {
                    if unsafe { call(s, f, DEBUG, &[], &[], &[]) }?.n[0] != 0 && alpha < 255 {
                        let counter = unsafe { call(s, f, DEBUG, &[], &[1], &[]) }?.n[1];
                        if counter < 64 || counter.wrapping_add(1) % 512 == 0 {
                            unsafe { call(s, f, LOG, &[draw.cast()], &[lf as i32, alpha], &[]) }?;
                        }
                    }
                    unsafe { call(s, f, COMPLEX, &[], &[lf as i32, alpha], &[]) }?;
                }
            }
        } else {
            unsafe { call(s, f, COMPLEX, &[], &[(*si).poly_flags as i32, 255], &[]) }?;
        }
        unsafe { call(s, f, HIT_POP, &[], &[], &[]) }?;
        if fi.n[9] != 0 {
            unsafe { call(s, f, DECAL_TIME, &[], &[], &[]) }?;
            if fi.n[0] == 0 && fi.n[6] != 0 && flags & 0x08000000 == 0 {
                let mut i = 0usize;
                let mut locked = false;
                let mut dt = ptr::null_mut();
                while i < unsafe { (*surf).decals.array_num } as usize {
                    if !locked {
                        let a = unsafe { (*decal_at(surf, i)).actor };
                        let a = unsafe { actor(s, f, a) }?;
                        dt = unsafe { texture(s, f, a.p[1]) }?;
                        unsafe { call(s, f, LOCK, &[dt], &[3], &[]) }?;
                    }
                    let mut poly = unsafe { (*facet).polys };
                    while !poly.is_null() {
                        // No array reference or iterator survives an SDK callback.
                        let d = unsafe { decal_at(surf, i) };
                        let nodes = unsafe { (*d).nodes };
                        let mut found = nodes.array_num == 0;
                        for j in 0..nodes.array_num as usize {
                            if unsafe { *nodes.data.cast::<i32>().add(j) }
                                == unsafe { (*poly).i_node }
                            {
                                found = true;
                                break;
                            }
                        }
                        if found {
                            let clipped = unsafe {
                                call(s, f, CLIP, &[surf.cast(), d.cast(), poly.cast()], &[], &[])
                            }?;
                            if clipped.n[0] != 0 {
                                let a = unsafe { (*decal_at(surf, i)).actor };
                                let style = unsafe { actor(s, f, a) }?.n[0];
                                let pf = if style == 4 {
                                    0x40000040
                                } else if style == 3 {
                                    4
                                } else {
                                    0
                                };
                                unsafe {
                                    call(
                                        s,
                                        f,
                                        TRIANGLES,
                                        &[a, clipped.p[0], draw.cast()],
                                        &[clipped.n[0], pf],
                                        &[],
                                    )
                                }?;
                            }
                        }
                        poly = unsafe { (*poly).next };
                    }
                    let same = if i + 1 < unsafe { (*surf).decals.array_num } as usize {
                        let a = unsafe { (*decal_at(surf, i)).actor };
                        let texture = unsafe { actor(s, f, a) }?.p[1];
                        let b = unsafe { (*decal_at(surf, i + 1)).actor };
                        unsafe { actor(s, f, b) }?.p[1] == texture
                    } else {
                        false
                    };
                    if same {
                        locked = true
                    } else if !dt.is_null() {
                        unsafe { call(s, f, UNLOCK, &[dt], &[3], &[]) }?;
                        locked = false
                    }
                    i += 1;
                }
            }
            unsafe { call(s, f, DECAL_TIME_END, &[], &[], &[]) }?;
        }
        unsafe { call(s, f, UNLOCK, &[t], &[0], &[]) }?;
        if !unsafe { (*si).detail_texture }.is_null() {
            let tv = unsafe { call(s, f, TEXTURE, &[t], &[], &[]) }?;
            unsafe { call(s, f, UNLOCK, &[tv.p[0]], &[1], &[]) }?;
        }
        if !unsafe { (*si).macro_texture }.is_null() {
            let tv = unsafe { call(s, f, TEXTURE, &[t], &[], &[]) }?;
            unsafe { call(s, f, UNLOCK, &[tv.p[1]], &[2], &[]) }?;
        }
        if !unsafe { (*si).light_map }.is_null() || !unsafe { (*si).fog_map }.is_null() {
            unsafe { call(s, f, LIGHT_FINISH, &[], &[], &[]) }?;
        }
        Ok(())
    }
    unsafe fn frame(s: &dyn DrawOps, f: *mut FSceneNode) -> Result {
        unsafe { call(s, f, FRAME, &[], &[], &[]) }?;
        let mut child = unsafe { (*f).child };
        while !child.is_null() {
            unsafe { frame(s, child) }?;
            child = unsafe { (*child).sibling }
        }
        if !unsafe { (*f).draw[0] }.is_null() {
            unsafe { call(s, f, CLEAR, &[], &[], &[]) }?;
        }
        let mut lists = [ptr::null_mut::<*mut FBspDrawList>(); 3];
        let mut counts = [0usize; 3];
        for pass in 0..3 {
            let mut d = unsafe { (*f).draw[pass] };
            while !d.is_null() {
                counts[pass] += 1;
                d = unsafe { (*d).next }
            }
            lists[pass] = unsafe {
                allocate(
                    s,
                    (counts[pass] * core::mem::size_of::<*mut FBspDrawList>()) as i32,
                )
            }?
            .cast();
            d = unsafe { (*f).draw[pass] };
            let mut i = 0;
            while !d.is_null() {
                unsafe { *lists[pass].add(i) = d };
                i += 1;
                d = unsafe { (*d).next }
            }
        }
        for i in 0..counts[0] / 2 {
            unsafe { ptr::swap(lists[0].add(i), lists[0].add(counts[0] - 1 - i)) }
        }
        unsafe { sort(lists[1], counts[1], |a, b| (*a).key.wrapping_sub((*b).key)) };
        let mut n = 0;
        let mut sprite = unsafe { (*f).sprite };
        while !sprite.is_null() {
            n += 1;
            sprite = unsafe { (*sprite).render_next }
        }
        let sprites = unsafe { allocate(s, (n * core::mem::size_of::<SpriteKey>()) as i32) }?
            .cast::<SpriteKey>();
        let info = unsafe { call(s, f, FRAME, &[], &[], &[]) }?;
        let span = info.n[0] != 0;
        sprite = unsafe { (*f).sprite };
        let mut i = 0;
        while !sprite.is_null() {
            let class = if span {
                0
            } else {
                let class = unsafe { actor(s, f, (*sprite).actor) }?.p[0]
                    .cast::<hp2_abi::engine::generated::UObject>();
                unsafe { (*class).index }
            };
            unsafe { *sprites.add(i) = SpriteKey { sprite, class } };
            i += 1;
            sprite = unsafe { (*sprite).render_next }
        }
        if !span {
            unsafe {
                sort(sprites, n, |a, b| {
                    if a.class < b.class {
                        -1
                    } else if a.class > b.class {
                        1
                    } else {
                        0
                    }
                })
            }
        }
        for pass in 0..3 {
            for index in 0..counts[pass] {
                let draw = unsafe { *lists[pass].add(index) };
                if pass == 2 && span {
                    let info = unsafe { call(s, f, FRAME, &[], &[], &[]) }?;
                    let plane = unsafe {
                        (*info.p[0].cast::<FBspNode>().add((*draw).i_node as usize)).plane
                    };
                    let side = plane_dot(plane, unsafe { (*f).coords.origin });
                    let mut i = 0;
                    while i < n {
                        let sp = unsafe { (*sprites.add(i)).sprite };
                        let a = unsafe { actor(s, f, (*sp).actor) }?;
                        if plane_dot(plane, vec(&a.f)) * side < 0.0
                            && unsafe {
                                crate::span::box_is_visible(
                                    &(*draw).span,
                                    (*sp).x1,
                                    (*sp).y1,
                                    (*sp).x2,
                                    (*sp).y2,
                                )
                            } != 0
                        {
                            unsafe { call(s, f, DRAW_SPRITE, &[sp.cast()], &[], &[]) }?;
                            n -= 1;
                            unsafe { ptr::copy(sprites.add(i + 1), sprites.add(i), n - i) };
                        } else {
                            i += 1
                        }
                    }
                }
                unsafe { surface(s, f, draw, pass as i32) }?;
            }
            if pass > if span { 1 } else { 0 } {
                for i in 0..n {
                    let sp = unsafe { (*sprites.add(i)).sprite };
                    let a = unsafe { actor(s, f, (*sp).actor) }?;
                    if !span {
                        let translucent = a.n[0] == 3 || a.f[3] < 1.0;
                        if pass != if translucent { 2 } else { 1 } {
                            continue;
                        }
                    }
                    if a.n[1] == 0 {
                        unsafe { call(s, f, DRAW_SPRITE, &[sp.cast()], &[], &[]) }?;
                    } else {
                        let mut j = 0;
                        loop {
                            if j >= unsafe { (*sp).child_count } {
                                break;
                            }
                            let child = unsafe { *(*sp).children.add(j as usize) };
                            unsafe { call(s, f, DRAW_SPRITE, &[child.cast()], &[], &[]) }?;
                            j += 1
                        }
                    }
                }
            }
        }
        unsafe { coronas(s, f) }?;
        if unsafe { call(s, f, SKY, &[], &[], &[]) }?.n[0] != 0 {
            unsafe { call(s, f, CLEAR, &[], &[], &[]) }?;
        }
        unsafe { call(s, f, FINISH, &[], &[], &[]) }?;
        Ok(())
    }
    struct CleanupDrawOps<'a> {
        inner: &'a dyn DrawOps,
        locks: [core::cell::Cell<(*mut c_void, i32, *mut FSceneNode)>; 4],
        lock_count: core::cell::Cell<usize>,
    }
    impl DrawOps for CleanupDrawOps<'_> {
        unsafe fn push(&self, bytes: usize) -> core::result::Result<*mut u8, ()> {
            unsafe { self.inner.push(bytes) }
        }
        unsafe fn call(
            &self,
            op: DrawOperation,
            frame: *mut FSceneNode,
            packet: &mut Hp2ScenePipelineDrawPacket,
        ) -> core::result::Result<(), ()> {
            unsafe { self.inner.call(op, frame, packet) }?;
            let count = self.lock_count.get();
            if op == DrawOperation::Lock {
                if count == self.locks.len() {
                    let mut unlock = Hp2ScenePipelineDrawPacket::default();
                    unlock.p[0] = packet.p[0];
                    unlock.n[0] = packet.n[0];
                    // SAFETY: this successful lock was not retained because the
                    // fixed cleanup list is full, so immediately release it.
                    let _ = unsafe { self.inner.call(DrawOperation::Unlock, frame, &mut unlock) };
                    return Err(());
                }
                self.locks[count].set((packet.p[0], packet.n[0], frame));
                self.lock_count.set(count + 1);
            } else if op == DrawOperation::Unlock {
                let lock = (packet.p[0], packet.n[0]);
                if let Some(index) = self.locks[..count]
                    .iter()
                    .position(|entry| {
                        let (texture, slot, _) = entry.get();
                        (texture, slot) == lock
                    })
                {
                    self.lock_count.set(count - 1);
                    self.locks[index].set(self.locks[count - 1].get());
                }
            }
            Ok(())
        }
    }
    impl Drop for CleanupDrawOps<'_> {
        fn drop(&mut self) {
            while self.lock_count.get() != 0 {
                let count = self.lock_count.get() - 1;
                self.lock_count.set(count);
                let (texture, slot, frame) = self.locks[count].get();
                let mut packet = Hp2ScenePipelineDrawPacket::default();
                packet.p[0] = texture;
                packet.n[0] = slot;
                // SAFETY: every entry was acquired by a successful Lock call during
                // this synchronous traversal and remains owned by the host context.
                let _ = unsafe {
                    self.inner
                        .call(DrawOperation::Unlock, frame, &mut packet)
                };
            }
        }
    }
    /// Runs the complete shared scene graph through a typed Native/Core host.
    ///
    /// # Safety
    /// `f` and every object referenced by it must remain live for this traversal.
    pub unsafe fn draw_frame_with(
        s: &dyn DrawOps,
        f: *mut FSceneNode,
    ) -> core::result::Result<(), ()> {
        let cleanup = CleanupDrawOps {
            inner: s,
            locks: core::array::from_fn(|_| {
                core::cell::Cell::new((core::ptr::null_mut(), 0, core::ptr::null_mut()))
            }),
            lock_count: core::cell::Cell::new(0),
        };
        unsafe { frame(&cleanup, f) }
    }
    /// Native ABI adapter for the shared complete scene draw policy.
    ///
    /// # Safety
    /// Services and all objects referenced by `f` must remain live for the call.
    pub unsafe fn hp2_scene_pipeline_draw_frame(
        s: *const Hp2ScenePipelineDrawServices,
        f: *mut FSceneNode,
    ) -> i32 {
        let host = NativeDrawOps(unsafe { &*s });
        unsafe { draw_frame_with(&host, f) }.is_ok() as i32
    }
}
