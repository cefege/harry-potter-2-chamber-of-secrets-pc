//! Authoritative dynamic renderer nodes, dispatch and frame-scoped reusable backing.
//! SDK callbacks are exception-contained leaves; no native node owns state.
use crate::{sprite, sprite_filter as filter};
use filter::FRasterPolyHeader;
use hp2_abi::render_scene::*;
use hp2_abi::render_span::{FRasterSpan, FSpanBuffer};
use hp2_abi::{FPlane, FVector};
use std::{cell::RefCell, ffi::c_void, ptr};

pub const SPRITE: u32 = 1;
pub const CHUNK: u32 = 2;
pub const FINAL: u32 = 3;
pub const LIGHT: u32 = 4;
pub const PARENT: u32 = 5;
pub const CHILD: u32 = 6;
pub const STANDALONE: u32 = 7;

#[repr(C)]
pub struct DynamicPacket {
    pub p: [*mut c_void; 8],
    pub n: [i32; 16],
    pub f: [f32; 16],
}
impl Default for DynamicPacket {
    fn default() -> Self {
        Self {
            p: [ptr::null_mut(); 8],
            n: [0; 16],
            f: [0.0; 16],
        }
    }
}
#[repr(C)]
pub struct DynamicServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, u32, *mut FSceneNode, *mut DynamicPacket) -> i32,
}
/// Facts gathered by the native actor setup leaf.
#[derive(Clone, Copy, Debug)]
pub struct DynamicSetup {
    pub sprite: bool,
    pub mesh: bool,
    pub particles: bool,
    pub show_actor_icons: bool,
    pub is_ortho: bool,
    pub is_realtime: bool,
    pub texture_present: bool,
    pub texture_width: i32,
    pub texture_height: i32,
    pub shell_only: bool,
    pub viewport_width: i32,
    pub viewport_height: i32,
    pub location: FVector,
    pub sprite_proj_forward: f32,
    pub draw_scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DynamicProjection {
    pub screen_x: f32,
    pub screen_y: f32,
    pub perspective: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DynamicBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DynamicBspSplit {
    pub plane: FPlane,
    pub front: i32,
    pub back: i32,
    pub csg: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct DynamicLightInfo {
    pub plane: FPlane,
    pub front: i32,
    pub back: i32,
    pub front_leaf: i32,
    pub back_leaf: i32,
    pub special_lit: bool,
    pub location: FVector,
    pub light_radius: f32,
    pub volumetric_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DynamicSurfaceInfo {
    pub surface: i32,
    pub plane: i32,
    pub has_light_map: bool,
    pub special_lit: bool,
    pub max_dynamic_lights: i32,
}

/// Typed requests for the source-level leaves used by dynamic actor rendering.
pub enum DynamicOperation<'a> {
    Setup {
        actor: *mut c_void,
        result: &'a mut DynamicSetup,
    },
    ProjectSprite {
        actor: *mut c_void,
        visible: &'a mut bool,
        projection: &'a mut DynamicProjection,
    },
    BoxAxes {
        actor: *mut c_void,
        axes: &'a mut FVector,
    },
    ProjectBox {
        visible: &'a mut bool,
        bounds: &'a mut DynamicBounds,
    },
    DynamicSlot {
        index: i32,
        back: i32,
        slot: &'a mut *mut *mut FDynamicItem,
    },
    AllocateRaster {
        start_y: i32,
        end_y: i32,
        raster: &'a mut *mut FRasterPolyHeader,
    },
    BspSplit {
        index: i32,
        split: &'a mut DynamicBspSplit,
    },
    InitializeSpan {
        source_span: *mut FSpanBuffer,
        raster: *mut FRasterPolyHeader,
        destination: Option<*mut *mut FSpanBuffer>,
        span: &'a mut *mut FSpanBuffer,
    },
    InsertActorLink {
        actor: *mut c_void,
        head: *mut *mut FActorLink,
        dynamic: bool,
    },
    LightBsp {
        index: i32,
        actor: *mut c_void,
        result: &'a mut DynamicLightInfo,
    },
    LeafLightStorage {
        leaf: i32,
        slot: &'a mut *mut *mut FVolActorLink,
        indices: &'a mut *mut i32,
        count: &'a mut *mut i32,
    },
    SurfaceLightStorage {
        index: i32,
        result: &'a mut DynamicSurfaceInfo,
        slot: &'a mut *mut *mut FActorLink,
        indices: &'a mut *mut i32,
        count: &'a mut *mut i32,
    },
    InsertVolumetricLink {
        actor: *mut c_void,
        head: *mut *mut FVolActorLink,
        volumetric: bool,
    },
    Statistics {
        index: i32,
        value: i32,
    },
    CopyFromRaster {
        destination: *mut FSpanBuffer,
        source: *mut FSpanBuffer,
        raster: *mut FRasterPolyHeader,
        drawn: &'a mut bool,
    },
    ReleaseSpan {
        span: *mut FSpanBuffer,
    },
    MergeSpan {
        destination: *mut FSpanBuffer,
        source: *mut FSpanBuffer,
    },
    PopSpanMark,
}

/// Supplies typed native source operations to the Rust-owned dynamic policy.
pub trait DynamicOps {
    /// Executes one source operation; errors stop the current policy path immediately.
    ///
    /// # Safety
    /// Pointers in `operation` and `frame` must be valid for the source operation.
    unsafe fn operation(
        &mut self,
        frame: *mut FSceneNode,
        operation: DynamicOperation<'_>,
    ) -> Result<(), ()>;
}

struct DynamicServicesAdapter<'a>(&'a DynamicServices);

unsafe fn call(
    s: &DynamicServices,
    op: u32,
    frame: *mut FSceneNode,
    q: &mut DynamicPacket,
) -> Result<(), ()> {
    if unsafe { (s.call)(s.context, op, frame, q) } != 0 {
        Ok(())
    } else {
        Err(())
    }
}
impl DynamicOps for DynamicServicesAdapter<'_> {
    unsafe fn operation(
        &mut self,
        frame: *mut FSceneNode,
        operation: DynamicOperation<'_>,
    ) -> Result<(), ()> {
        let mut packet = DynamicPacket::default();
        let service = self.0;
        match operation {
            DynamicOperation::Setup { actor, result } => {
                packet.p[0] = actor;
                unsafe { call(service, 0, frame, &mut packet)? };
                *result = DynamicSetup {
                    sprite: packet.n[0] != 0,
                    mesh: packet.n[1] != 0,
                    particles: packet.n[2] != 0,
                    show_actor_icons: packet.n[3] != 0,
                    is_ortho: packet.n[4] != 0,
                    is_realtime: packet.n[5] != 0,
                    texture_present: !packet.p[1].is_null(),
                    texture_width: packet.n[6],
                    texture_height: packet.n[7],
                    shell_only: packet.n[8] != 0,
                    viewport_width: packet.n[9],
                    viewport_height: packet.n[10],
                    location: vector(&packet.f),
                    sprite_proj_forward: packet.f[3],
                    draw_scale: packet.f[4],
                };
            }
            DynamicOperation::ProjectSprite {
                actor,
                visible,
                projection,
            } => {
                packet.p[0] = actor;
                unsafe { call(service, 1, frame, &mut packet)? };
                *visible = packet.n[0] != 0;
                *projection = DynamicProjection {
                    screen_x: packet.f[0],
                    screen_y: packet.f[1],
                    perspective: packet.f[2],
                };
            }
            DynamicOperation::BoxAxes { actor, axes } => {
                packet.p[0] = actor;
                unsafe { call(service, 18, frame, &mut packet)? };
                *axes = vector(&packet.f);
            }
            DynamicOperation::ProjectBox { visible, bounds } => {
                unsafe { call(service, 2, frame, &mut packet)? };
                *visible = packet.n[0] != 0;
                *bounds = DynamicBounds {
                    min_x: packet.f[0],
                    max_x: packet.f[1],
                    min_y: packet.f[2],
                    max_y: packet.f[3],
                    min_z: packet.f[4],
                    max_z: packet.f[5],
                };
            }
            DynamicOperation::DynamicSlot { index, back, slot } => {
                packet.n[0] = index;
                packet.n[1] = back;
                unsafe { call(service, 3, frame, &mut packet)? };
                *slot = packet.p[0].cast();
            }
            DynamicOperation::AllocateRaster {
                start_y,
                end_y,
                raster,
            } => {
                packet.n[0] = start_y;
                packet.n[1] = end_y;
                unsafe { call(service, 4, frame, &mut packet)? };
                *raster = packet.p[0].cast();
            }
            DynamicOperation::BspSplit { index, split } => {
                packet.n[0] = index;
                unsafe { call(service, 5, frame, &mut packet)? };
                *split = DynamicBspSplit {
                    plane: unsafe { *packet.p[0].cast::<FPlane>() },
                    front: packet.n[1],
                    back: packet.n[2],
                    csg: packet.n[3] != 0,
                };
            }
            DynamicOperation::InitializeSpan {
                source_span,
                raster,
                destination,
                span,
            } => {
                packet.p[1] = source_span.cast();
                packet.p[2] = raster.cast();
                if let Some(destination) = destination {
                    packet.p[3] = destination.cast();
                }
                let op = if destination.is_some() { 6 } else { 7 };
                unsafe { call(service, op, frame, &mut packet)? };
                *span = packet.p[0].cast();
            }
            DynamicOperation::InsertActorLink {
                actor,
                head,
                dynamic,
            } => {
                packet.p[0] = actor;
                packet.p[1] = unsafe { *head }.cast();
                packet.n[0] = dynamic as i32;
                unsafe { call(service, 8, frame, &mut packet)? };
                unsafe { *head = packet.p[0].cast() };
            }
            DynamicOperation::LightBsp {
                index,
                actor,
                result,
            } => {
                packet.n[0] = index;
                packet.p[0] = actor;
                unsafe { call(service, 9, frame, &mut packet)? };
                *result = DynamicLightInfo {
                    plane: unsafe { *packet.p[0].cast::<FPlane>() },
                    front: packet.n[1],
                    back: packet.n[2],
                    front_leaf: packet.n[3],
                    back_leaf: packet.n[4],
                    special_lit: packet.n[5] != 0,
                    location: vector(&packet.f),
                    light_radius: packet.f[3],
                    volumetric_radius: packet.f[4],
                };
            }
            DynamicOperation::LeafLightStorage {
                leaf,
                slot,
                indices,
                count,
            } => {
                packet.n[0] = leaf;
                unsafe { call(service, 10, frame, &mut packet)? };
                *slot = packet.p[0].cast();
                *indices = packet.p[1].cast();
                *count = packet.p[2].cast();
            }
            DynamicOperation::SurfaceLightStorage {
                index,
                result,
                slot,
                indices,
                count,
            } => {
                packet.n[0] = index;
                unsafe { call(service, 11, frame, &mut packet)? };
                *result = DynamicSurfaceInfo {
                    surface: packet.n[0],
                    plane: packet.n[1],
                    has_light_map: packet.n[2] != 0,
                    special_lit: packet.n[3] != 0,
                    max_dynamic_lights: packet.n[4],
                };
                *slot = packet.p[0].cast();
                *indices = packet.p[1].cast();
                *count = packet.p[2].cast();
            }
            DynamicOperation::InsertVolumetricLink {
                actor,
                head,
                volumetric,
            } => {
                packet.p[0] = actor;
                packet.p[1] = unsafe { *head }.cast();
                packet.n[0] = volumetric as i32;
                unsafe { call(service, 12, frame, &mut packet)? };
                unsafe { *head = packet.p[0].cast() };
            }
            DynamicOperation::Statistics { index, value } => {
                packet.n[0] = index;
                packet.n[1] = value;
                unsafe { call(service, 13, frame, &mut packet)? };
            }
            DynamicOperation::CopyFromRaster {
                destination,
                source,
                raster,
                drawn,
            } => {
                packet.p[0] = destination.cast();
                packet.p[1] = source.cast();
                packet.p[2] = raster.cast();
                unsafe { call(service, 14, frame, &mut packet)? };
                *drawn = packet.n[0] != 0;
            }
            DynamicOperation::ReleaseSpan { span } => {
                packet.p[0] = span.cast();
                unsafe { call(service, 15, frame, &mut packet)? };
            }
            DynamicOperation::MergeSpan {
                destination,
                source,
            } => {
                packet.p[0] = source.cast();
                packet.p[3] = destination.cast();
                unsafe { call(service, 16, frame, &mut packet)? };
            }
            DynamicOperation::PopSpanMark => unsafe { call(service, 17, frame, &mut packet)? },
        }
        Ok(())
    }
}
#[repr(C)]
struct Node {
    view: FDynamicSprite,
    children: Vec<*mut FDynamicSprite>,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            view: unsafe { std::mem::zeroed() },
            children: Vec::new(),
        }
    }
}
#[repr(C)]
struct Chunk {
    base: FDynamicItem,
    raster: *mut FRasterPolyHeader,
    sprite: *mut FDynamicSprite,
}
impl Default for Chunk {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
#[repr(C)]
struct Light {
    base: FDynamicItem,
    actor: *mut c_void,
    is_vol: bool,
    hit_leaf: bool,
}
impl Default for Light {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
const SLAB: usize = 256;
struct Storage<T> {
    slabs: Vec<Box<[T]>>,
    cursor: usize,
}
impl<T> Default for Storage<T> {
    fn default() -> Self {
        Self {
            slabs: Vec::new(),
            cursor: 0,
        }
    }
}
impl<T: Default> Storage<T> {
    fn allocate(&mut self) -> *mut T {
        let i = self.cursor;
        if i / SLAB == self.slabs.len() {
            self.slabs.push(
                (0..SLAB)
                    .map(|_| T::default())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        }
        self.cursor += 1;
        &mut self.slabs[i / SLAB][i % SLAB]
    }
}
#[derive(Default)]
struct Pool {
    sprites: Storage<Node>,
    chunks: Storage<Chunk>,
    lights: Storage<Light>,
    marks: Vec<[usize; 3]>,
}
thread_local! { static POOL:RefCell<Pool>=RefCell::new(Pool::default()); }
pub fn begin() -> usize {
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        let cursor = [p.sprites.cursor, p.chunks.cursor, p.lights.cursor];
        p.marks.push(cursor);
        p.marks.len()
    })
}
pub fn end(token: usize) -> bool {
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        if token == 0 || token != p.marks.len() {
            return false;
        }
        let start = p.marks.pop().unwrap();
        for i in start[0]..p.sprites.cursor {
            let n = &mut p.sprites.slabs[i / SLAB][i % SLAB];
            n.children.clear();
            n.view.children = ptr::null_mut();
            n.view.child_count = 0;
        }
        p.sprites.cursor = start[0];
        p.chunks.cursor = start[1];
        p.lights.cursor = start[2];
        true
    })
}
fn allocate(kind: u32, actor: *mut c_void) -> Result<*mut Node, ()> {
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        if p.marks.is_empty() {
            return Err(());
        }
        let node = unsafe { &mut *p.sprites.allocate() };
        node.view = unsafe { std::mem::zeroed() };
        node.view.base.kind = kind;
        node.view.actor = actor;
        node.children.clear();
        Ok(node as *mut Node)
    })
}
unsafe fn stat<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    value: i32,
) -> Result<(), ()> {
    unsafe { ops.operation(frame, DynamicOperation::Statistics { index, value }) }
}
unsafe fn slot<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    back: i32,
) -> Result<*mut *mut FDynamicItem, ()> {
    let mut result = ptr::null_mut();
    unsafe {
        ops.operation(
            frame,
            DynamicOperation::DynamicSlot {
                index,
                back,
                slot: &mut result,
            },
        )?;
    }
    Ok(result)
}
unsafe fn publish<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    back: i32,
    item: *mut FDynamicItem,
    sorted: bool,
) -> Result<(), ()> {
    let mut link = unsafe { slot(ops, frame, index, back)? };
    if sorted {
        unsafe {
            while !(*link).is_null()
                && (**link).min_z + (**link).max_z < (*item).min_z + (*item).max_z
            {
                link = &mut (**link).filter_next;
            }
        }
    }
    unsafe {
        (*item).filter_next = *link;
        *link = item;
    }
    Ok(())
}
unsafe fn raster<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    start: i32,
    end: i32,
) -> Result<*mut FRasterPolyHeader, ()> {
    let mut result = ptr::null_mut();
    unsafe {
        ops.operation(
            frame,
            DynamicOperation::AllocateRaster {
                start_y: start,
                end_y: end,
                raster: &mut result,
            },
        )?;
    }
    Ok(result)
}
fn vector(f: &[f32]) -> FVector {
    FVector {
        x: f[0],
        y: f[1],
        z: f[2],
    }
}
unsafe fn setup<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    node: *mut Node,
) -> Result<bool, ()> {
    let v = unsafe { &mut (*node).view };
    let fr = unsafe { &*frame };
    let mut facts = DynamicSetup {
        sprite: false,
        mesh: false,
        particles: false,
        show_actor_icons: false,
        is_ortho: false,
        is_realtime: false,
        texture_present: false,
        texture_width: 0,
        texture_height: 0,
        shell_only: false,
        viewport_width: 0,
        viewport_height: 0,
        location: FVector { x: 0.0, y: 0.0, z: 0.0 },
        sprite_proj_forward: 0.0,
        draw_scale: 0.0,
    };
    unsafe {
        ops.operation(
            frame,
            DynamicOperation::Setup {
                actor: v.actor,
                result: &mut facts,
            },
        )?;
    }
    let path = sprite::setup_path(
        facts.sprite,
        facts.mesh,
        facts.particles,
        facts.show_actor_icons,
        facts.is_ortho,
        facts.is_realtime,
    );
    match path {
        sprite::SetupPath::Reject => return Ok(false),
        sprite::SetupPath::Sprite => {
            if !facts.texture_present {
                return Ok(false);
            }
            let plane = sprite::projection_plane(
                facts.location,
                fr.coords.origin,
                fr.coords.z_axis,
                facts.sprite_proj_forward,
                facts.is_ortho,
            );
            v.base.min_z = plane.min_z;
            v.base.max_z = plane.min_z;
            if plane.reject {
                return Ok(false);
            }
            let mut visible = false;
            let mut projection = DynamicProjection {
                screen_x: 0.0,
                screen_y: 0.0,
                perspective: 0.0,
            };
            unsafe {
                ops.operation(
                    frame,
                    DynamicOperation::ProjectSprite {
                        actor: v.actor,
                        visible: &mut visible,
                        projection: &mut projection,
                    },
                )?;
            }
            v.screen_x = projection.screen_x;
            v.screen_y = projection.screen_y;
            v.persp = projection.perspective;
            if !visible {
                return Ok(false);
            }
            let x = sprite::extent_x(
                v.screen_x,
                v.persp,
                facts.draw_scale,
                facts.texture_width,
                fr.x,
            );
            v.x1 = x.lo;
            v.x2 = x.hi;
            if !x.visible {
                return Ok(false);
            }
            let y = sprite::extent_y(
                v.screen_y,
                v.persp,
                facts.draw_scale,
                facts.texture_height,
                fr.y,
            );
            v.y1 = y.lo;
            v.y2 = y.hi;
            if !y.visible {
                return Ok(false);
            }
        }
        sprite::SetupPath::Box => {
            // The old shell-only branch never initialized depth. Use the actor's
            // projection depth, not a claimed compatibility value for native UB.
            if facts.shell_only {
                v.x1 = 0;
                v.y1 = 0;
                v.x2 = facts.viewport_width;
                v.y2 = facts.viewport_height;
                let z = sprite::projection_plane(
                    facts.location,
                    fr.coords.origin,
                    fr.coords.z_axis,
                    facts.sprite_proj_forward,
                    facts.is_ortho,
                )
                .min_z;
                v.base.min_z = z;
                v.base.max_z = z;
                return Ok(true);
            }
            let mut axes = FVector { x: 0.0, y: 0.0, z: 0.0 };
            unsafe {
                ops.operation(
                    frame,
                    DynamicOperation::BoxAxes {
                        actor: v.actor,
                        axes: &mut axes,
                    },
                )?;
            }
            if sprite::box_axis_is_empty(axes) {
                return Ok(false);
            }
            let mut visible = false;
            let mut bounds = DynamicBounds {
                min_x: 0.0,
                max_x: 0.0,
                min_y: 0.0,
                max_y: 0.0,
                min_z: 0.0,
                max_z: 0.0,
            };
            unsafe {
                ops.operation(
                    frame,
                    DynamicOperation::ProjectBox {
                        visible: &mut visible,
                        bounds: &mut bounds,
                    },
                )?;
            }
            if !visible {
                return Ok(false);
            }
            let b = sprite::box_extent(
                bounds.min_x,
                bounds.max_x,
                bounds.min_y,
                bounds.max_y,
                bounds.min_z,
                bounds.max_z,
            );
            v.x1 = b.x1;
            v.x2 = b.x2;
            v.y1 = b.y1;
            v.y2 = b.y2;
            v.base.min_z = b.min_z;
            v.base.max_z = b.max_z;
            if b.reject {
                return Ok(false);
            }
            unsafe {
                stat(ops, frame, 4, (v.x2 + 1 - v.x1).wrapping_mul(v.y2 + 1 - v.y1))?;
            }
        }
    }
    Ok(true)
}
unsafe fn chunk<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    sprite: *mut FDynamicSprite,
    raster: *mut FRasterPolyHeader,
    back: Option<i32>,
) -> Result<*mut Chunk, ()> {
    let node = POOL.with(|p| {
        let mut p = p.borrow_mut();
        if p.marks.is_empty() {
            Err(())
        } else {
            Ok(p.chunks.allocate())
        }
    })?;
    unsafe {
        *node = Chunk::default();
        (*node).base.kind = if back.is_some() { FINAL } else { CHUNK };
        (*node).sprite = sprite;
        (*node).raster = raster;
        if back.is_some() {
            (*node).base.min_z = (*sprite).base.min_z;
            (*node).base.max_z = (*sprite).base.max_z;
        }
        publish(
            ops,
            frame,
            index,
            back.unwrap_or(0),
            node.cast(),
            back.is_some(),
        )?;
        stat(ops, frame, if back.is_some() { 2 } else { 1 }, 1)?;
    }
    Ok(node)
}
pub unsafe fn create_with_ops<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    actor: *mut c_void,
    kind: u32,
    parent: *mut FDynamicSprite,
) -> Result<*mut FDynamicSprite, ()> {
    let node = allocate(kind, actor)?;
    let visible = unsafe { setup(ops, frame, node)? };
    if visible && kind != STANDALONE {
        // SysParent deliberately uses exactly the base sprite admission.
        if kind != CHILD {
            unsafe {
                publish(ops, frame, index, 0, node.cast(), false)?;
            }
        }
        let v = unsafe { &mut (*node).view };
        let fr = unsafe { &*frame };
        let z = v.base.min_z;
        let zr = z * fr.r_proj.z;
        for (i, (x, y)) in [(v.x1, v.y1), (v.x2, v.y1), (v.x2, v.y2), (v.x1, v.y2)]
            .into_iter()
            .enumerate()
        {
            v.proxy_verts[i].point = hp2_core::math::vector_transform_point(
                FVector {
                    x: zr * (x as f32 - fr.fx2),
                    y: zr * (y as f32 - fr.fy2),
                    z,
                },
                fr.uncoords,
            );
            v.proxy_verts[i].screen_x = x as f32;
            v.proxy_verts[i].screen_y = y as f32;
        }
        if v.y1 < 0 || v.y2 > fr.y || v.y1 >= v.y2 {
            return Err(());
        }
        // Legacy children filled an unreachable raster; only filtered sprites
        // need raster storage. Children retain the same projection proxy.
        if kind != CHILD {
            let r = unsafe { raster(ops, frame, v.y1, v.y2)? };
            unsafe {
                (*r).start_y = v.y1;
                (*r).end_y = v.y2;
                for line in std::slice::from_raw_parts_mut(
                    r.add(1).cast::<FRasterSpan>(),
                    (v.y2 - v.y1) as usize,
                ) {
                    line.x = [v.x1, v.x2];
                }
                chunk(ops, frame, index, &mut (*node).view, r, None)?;
            }
        }
    }
    if kind != STANDALONE {
        unsafe {
            stat(ops, frame, 0, 1)?;
        }
    }
    if !parent.is_null() {
        let p = parent.cast::<Node>();
        unsafe {
            (*p).children.push(&mut (*node).view);
            (*p).view.children = (*p).children.as_mut_ptr();
            (*p).view.child_count = (*p).children.len() as i32;
        }
    }
    if kind == STANDALONE && !visible {
        Ok(ptr::null_mut())
    } else {
        Ok(node.cast())
    }
}
/// Runs dynamic sprite construction using typed source operations.
pub unsafe fn create(
    services: &DynamicServices,
    frame: *mut FSceneNode,
    index: i32,
    actor: *mut c_void,
    kind: u32,
    parent: *mut FDynamicSprite,
) -> Result<*mut FDynamicSprite, ()> {
    let mut ops = DynamicServicesAdapter(services);
    unsafe { create_with_ops(&mut ops, frame, index, actor, kind, parent) }
}
pub unsafe fn light_with_ops<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    actor: *mut c_void,
    is_vol: bool,
    hit_leaf: bool,
) -> Result<(), ()> {
    let node = POOL.with(|p| {
        let mut p = p.borrow_mut();
        if p.marks.is_empty() {
            Err(())
        } else {
            Ok(p.lights.allocate())
        }
    })?;
    unsafe {
        *node = Light::default();
        (*node).base.kind = LIGHT;
        (*node).actor = actor;
        (*node).is_vol = is_vol;
        (*node).hit_leaf = hit_leaf;
        publish(ops, frame, index, 0, node.cast(), false)?;
        stat(ops, frame, 3, 1)
    }
}
pub unsafe fn light(
    services: &DynamicServices,
    frame: *mut FSceneNode,
    index: i32,
    actor: *mut c_void,
    is_vol: bool,
    hit_leaf: bool,
) -> Result<(), ()> {
    let mut ops = DynamicServicesAdapter(services);
    unsafe { light_with_ops(&mut ops, frame, index, actor, is_vol, hit_leaf) }
}
unsafe fn filter_chunk<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    outside: i32,
    node: *mut Chunk,
) -> Result<(), ()> {
    let mut split = DynamicBspSplit {
        plane: unsafe { std::mem::zeroed() },
        front: 0,
        back: 0,
        csg: false,
    };
    unsafe {
        ops.operation(
            frame,
            DynamicOperation::BspSplit {
                index,
                split: &mut split,
            },
        )?;
    }
    let plane = split.plane;
    let v = unsafe { &(*(*node).sprite).proxy_verts };
    let r = unsafe { (*node).raster };
    let start = unsafe { (*r).start_y };
    let end = unsafe { (*r).end_y };
    let len = (end - start) as usize;
    let source = filter::Source {
        start_y: start,
        end_y: end,
        lines: unsafe { std::slice::from_raw_parts(r.add(1).cast(), len) },
    };
    let mut front = ptr::null_mut::<FRasterPolyHeader>();
    let mut back = ptr::null_mut::<FRasterPolyHeader>();
    let bufs = if filter::split_allocates(&filter::crossings(v, &plane)) {
        front = unsafe { raster(ops, frame, start, end)? };
        back = unsafe { raster(ops, frame, start, end)? };
        Some(unsafe {
            [
                filter::Buf {
                    header: &mut *front,
                    lines: std::slice::from_raw_parts_mut(front.add(1).cast(), len),
                },
                filter::Buf {
                    header: &mut *back,
                    lines: std::slice::from_raw_parts_mut(back.add(1).cast(), len),
                },
            ]
        })
    } else {
        None
    };
    let result = filter::filter_chunk(
        v,
        &plane,
        &source,
        bufs,
        split.front,
        split.back,
        outside != 0,
        split.csg,
    );
    for (action, which, child, side) in [
        (result.front_action, result.front, split.front, 0),
        (result.back_action, result.back, split.back, 1),
    ] {
        let r = match which {
            filter::Raster::Null => ptr::null_mut(),
            filter::Raster::Source => r,
            filter::Raster::Front => front,
            filter::Raster::Back => back,
        };
        unsafe {
            match action {
                filter::Action::None => {}
                filter::Action::Chunk => {
                    chunk(ops, frame, child, (*node).sprite, r, None)?;
                }
                filter::Action::Final => {
                    chunk(ops, frame, index, (*node).sprite, r, Some(side))?;
                }
            }
        }
    }
    Ok(())
}
unsafe fn contains(mut link: *mut FActorLink, actor: *mut c_void) -> bool {
    unsafe {
        while !link.is_null() {
            if (*link).actor == actor {
                return true;
            }
            link = (*link).next;
        }
    }
    false
}
unsafe fn light_side<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    node: *mut Light,
    leaf: i32,
    child: i32,
    vol: bool,
) -> Result<(), ()> {
    let mut hit = unsafe { (*node).hit_leaf };
    if !hit && leaf != -1 {
        let mut slot = ptr::null_mut();
        let mut indices = ptr::null_mut();
        let mut count = ptr::null_mut();
        unsafe {
            ops.operation(
                frame,
                DynamicOperation::LeafLightStorage {
                    leaf,
                    slot: &mut slot,
                    indices: &mut indices,
                    count: &mut count,
                },
            )?;
        }
        unsafe {
            if (*slot).is_null() {
                *indices.add(*count as usize) = leaf;
                *count += 1;
            }
            ops.operation(
                frame,
                DynamicOperation::InsertVolumetricLink {
                    actor: (*node).actor,
                    head: slot,
                    volumetric: vol,
                },
            )?;
        }
        hit = true;
    }
    if child != -1 {
        unsafe {
            light_with_ops(ops, frame, child, (*node).actor, vol, hit)?;
        }
    }
    Ok(())
}
unsafe fn filter_light<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    node: *mut Light,
) -> Result<(), ()> {
    let mut info = DynamicLightInfo {
        plane: unsafe { std::mem::zeroed() },
        front: 0,
        back: 0,
        front_leaf: 0,
        back_leaf: 0,
        special_lit: false,
        location: FVector { x: 0.0, y: 0.0, z: 0.0 },
        light_radius: 0.0,
        volumetric_radius: 0.0,
    };
    unsafe {
        ops.operation(
            frame,
            DynamicOperation::LightBsp {
                index,
                actor: (*node).actor,
                result: &mut info,
            },
        )?;
    }
    let flags = filter::light_flags(
        &info.plane,
        &info.location,
        info.light_radius,
        info.volumetric_radius,
        unsafe { (*node).is_vol },
    );
    if flags & 1 != 0 {
        unsafe {
            light_side(
                ops,
                frame,
                node,
                info.front_leaf,
                info.front,
                flags & 2 != 0,
            )?;
        }
        if flags & 4 != 0 {
            let mut surface = index;
            while surface != -1 {
                let mut data = DynamicSurfaceInfo {
                    surface: 0,
                    plane: -1,
                    has_light_map: false,
                    special_lit: false,
                    max_dynamic_lights: 0,
                };
                let mut slot = ptr::null_mut();
                let mut indices = ptr::null_mut();
                let mut count = ptr::null_mut();
                unsafe {
                    ops.operation(
                        frame,
                        DynamicOperation::SurfaceLightStorage {
                            index: surface,
                            result: &mut data,
                            slot: &mut slot,
                            indices: &mut indices,
                            count: &mut count,
                        },
                    )?;
                }
                surface = data.plane;
                if data.has_light_map
                    && unsafe { *count } < data.max_dynamic_lights
                    && info.special_lit == data.special_lit
                    && !unsafe { contains(*slot, (*node).actor) }
                {
                    unsafe {
                        if (*slot).is_null() {
                            *indices.add(*count as usize) = data.surface;
                            *count += 1;
                        }
                        ops.operation(
                            frame,
                            DynamicOperation::InsertActorLink {
                                actor: (*node).actor,
                                head: slot,
                                dynamic: false,
                            },
                        )?;
                    }
                }
            }
        }
    }
    if flags & 4 != 0 {
        unsafe {
            light_side(
                ops,
                frame,
                node,
                info.back_leaf,
                info.back,
                flags & 8 != 0,
            )?;
        }
    }
    Ok(())
}
pub unsafe fn dispatch_filter_with_ops<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    index: i32,
    outside: i32,
    item: *mut FDynamicItem,
) -> Result<(), ()> {
    unsafe {
        match (*item).kind {
            CHUNK => filter_chunk(ops, frame, index, outside, item.cast()),
            LIGHT => filter_light(ops, frame, index, item.cast()),
            _ => Ok(()),
        }
    }
}

pub unsafe fn dispatch_filter(
    services: &DynamicServices,
    frame: *mut FSceneNode,
    index: i32,
    outside: i32,
    item: *mut FDynamicItem,
) -> Result<(), ()> {
    let mut ops = DynamicServicesAdapter(services);
    unsafe { dispatch_filter_with_ops(&mut ops, frame, index, outside, item) }
}
pub unsafe fn prerender_with_ops<O: DynamicOps + ?Sized>(
    ops: &mut O,
    frame: *mut FSceneNode,
    item: *mut FDynamicItem,
    span: *mut FSpanBuffer,
    mut first_volumetric: *mut FVolActorLink,
) -> Result<(), ()> {
    if unsafe { (*item).kind } != FINAL {
        return Ok(());
    }
    let node = item.cast::<Chunk>();
    let sprite = unsafe { (*node).sprite };
    let first = unsafe { (*sprite).span_buffer.is_null() };
    let mut allocated_span = ptr::null_mut();
    let mut drawn = false;
    let prepare = (|| {
        let destination_field = if first {
            Some(unsafe { &mut (*sprite).span_buffer as *mut *mut FSpanBuffer })
        } else {
            None
        };
        unsafe {
            ops.operation(
                frame,
                DynamicOperation::InitializeSpan {
                    source_span: span,
                    raster: (*node).raster,
                    destination: destination_field,
                    span: &mut allocated_span,
                },
            )?;
        }
        unsafe {
            ops.operation(
                frame,
                DynamicOperation::CopyFromRaster {
                    destination: allocated_span,
                    source: span,
                    raster: (*node).raster,
                    drawn: &mut drawn,
                },
            )?;
        }
        if first && !drawn {
            unsafe {
                ops.operation(
                    frame,
                    DynamicOperation::ReleaseSpan {
                        span: allocated_span,
                    },
                )?;
                (*sprite).span_buffer = ptr::null_mut();
            }
        } else if !first && drawn {
            unsafe {
                ops.operation(
                    frame,
                    DynamicOperation::MergeSpan {
                        destination: (*sprite).span_buffer,
                        source: allocated_span,
                    },
                )?;
                stat(ops, frame, 5, 1)?;
            }
        }
        Ok(())
    })();
    if !first {
        let pop = unsafe { ops.operation(frame, DynamicOperation::PopSpanMark) };
        if prepare.is_err() {
            let _ = pop;
            return prepare;
        }
        pop?;
    }
    prepare?;
    if !drawn {
        return Ok(());
    }
    if first {
        unsafe { stat(ops, frame, 5, 1)? };
    }
    if first {
        let mut link = unsafe { &mut (*frame).sprite as *mut *mut FDynamicSprite };
        unsafe {
            while !(*link).is_null()
                && (**link).base.min_z + (**link).base.max_z
                    > (*sprite).base.min_z + (*sprite).base.max_z
            {
                link = &mut (**link).render_next;
            }
            (*sprite).render_next = *link;
            *link = sprite;
        }
    }
    unsafe {
        while !first_volumetric.is_null() {
            if (*first_volumetric).volumetric != 0
                && !contains((*sprite).volumetrics, (*first_volumetric).actor)
            {
                ops.operation(
                    frame,
                    DynamicOperation::InsertActorLink {
                        actor: (*first_volumetric).actor,
                        head: &mut (*sprite).volumetrics,
                        dynamic: true,
                    },
                )?;
            }
            first_volumetric = (*first_volumetric).next;
        }
    }
    Ok(())
}

pub unsafe fn prerender(
    services: &DynamicServices,
    frame: *mut FSceneNode,
    item: *mut FDynamicItem,
    span: *mut FSpanBuffer,
    first_volumetric: *mut FVolActorLink,
) -> Result<(), ()> {
    let mut ops = DynamicServicesAdapter(services);
    unsafe { prerender_with_ops(&mut ops, frame, item, span, first_volumetric) }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FrameScope(usize);
    impl FrameScope {
        fn new() -> Self {
            Self(begin())
        }
    }
    impl Drop for FrameScope {
        fn drop(&mut self) {
            assert!(end(self.0));
        }
    }

    #[test]
    fn nested_growth_preserves_outer_nodes_and_reuses_child_backing() {
        let first;
        let backing;
        {
            let _outer = FrameScope::new();
            first = allocate(PARENT, ptr::null_mut()).unwrap();
            unsafe {
                for _ in 0..128 {
                    (*first).children.push(first.cast());
                }
                (*first).view.children = (*first).children.as_mut_ptr();
                (*first).view.child_count = 128;
                backing = (*first).view.children;
            }
            {
                let _inner = FrameScope::new();
                for _ in 0..(SLAB * 3 + 1) {
                    allocate(CHILD, ptr::null_mut()).unwrap();
                }
                unsafe {
                    assert_eq!((*first).view.child_count, 128);
                    assert_eq!((*first).view.children, backing);
                    assert_eq!(*backing.add(127), first.cast());
                }
            }
            unsafe {
                assert_eq!((*first).children.len(), 128);
            }
        }
        let _next = FrameScope::new();
        let reused = allocate(PARENT, ptr::null_mut()).unwrap();
        assert_eq!(first, reused);
        unsafe {
            assert!((*reused).children.is_empty());
            assert_eq!((*reused).view.child_count, 0);
            assert!((*reused).view.children.is_null());
            (*reused).children.push(reused.cast());
            assert_eq!((*reused).children.as_mut_ptr(), backing);
        }
    }

    #[test]
    fn error_unwind_reclaims_children_without_reclaiming_outer_frame() {
        let _outer = FrameScope::new();
        let outer = allocate(SPRITE, ptr::null_mut()).unwrap();
        let mut inner = ptr::null_mut();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _scope = FrameScope::new();
            inner = allocate(PARENT, ptr::null_mut()).unwrap();
            unsafe {
                (*inner).children.resize(129, outer.cast());
            }
            panic!("simulated native entry unwind");
        }));
        assert!(failure.is_err());
        let next = allocate(PARENT, ptr::null_mut()).unwrap();
        assert_eq!(inner, next);
        unsafe {
            assert_eq!((*outer).view.base.kind, SPRITE);
            assert!((*next).children.is_empty());
            assert!((*next).children.capacity() >= 129);
        }
    }

    unsafe extern "C" fn links(
        context: *mut c_void,
        op: u32,
        _frame: *mut FSceneNode,
        q: *mut DynamicPacket,
    ) -> i32 {
        match op {
            3 => unsafe {
                (*q).p[0] = context;
                1
            },
            13 => 1,
            _ => 0,
        }
    }
    #[test]
    fn final_depth_order_is_strict_and_inserts_equal_or_nan_before_existing() {
        let _scope = FrameScope::new();
        let mut head: *mut FDynamicItem = ptr::null_mut();
        let services = DynamicServices {
            context: (&mut head as *mut *mut FDynamicItem).cast(),
            call: links,
        };
        let mut ops = DynamicServicesAdapter(&services);
        let mut made = Vec::new();
        for z in [2.0, 1.0, 3.0, 1.0] {
            let sprite = allocate(SPRITE, ptr::null_mut()).unwrap();
            unsafe {
                (*sprite).view.base.min_z = z;
                (*sprite).view.base.max_z = z;
                made.push(
                    chunk(
                        &mut ops,
                        ptr::null_mut(),
                        0,
                        sprite.cast(),
                        ptr::null_mut(),
                        Some(0),
                    )
                    .unwrap(),
                );
            }
        }
        let mut item = head;
        for index in [3, 1, 0, 2] {
            assert_eq!(item, made[index].cast());
            item = unsafe { (*item).filter_next };
        }
        assert!(item.is_null());
        let sprite = allocate(SPRITE, ptr::null_mut()).unwrap();
        unsafe {
            (*sprite).view.base.min_z = f32::NAN;
            let newest = chunk(
                &mut ops,
                ptr::null_mut(),
                0,
                sprite.cast(),
                ptr::null_mut(),
                Some(0),
            )
            .unwrap();
            assert_eq!(head, newest.cast());
            assert_eq!((*head).filter_next, made[3].cast());
        }
    }
}
