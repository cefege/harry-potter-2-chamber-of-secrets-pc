//! DrawLodMesh orchestration using the Rust-owned shared mesh clip/batch state.
use crate::mesh;
use core::ffi::c_void;
use hp2_abi::engine::generated::{AActor, AZoneInfo, FTextureInfo, UTexture};
use hp2_abi::render_particle::FTransTexture;
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::math;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LodWedge {
    pub vertex: u16,
    pub u: u8,
    pub v: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LodFace {
    pub wedges: [u16; 3],
    pub material: u16,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LodMaterial {
    pub flags: u32,
    pub texture: i32,
}
/// Native-owned scalar state for SDK bitfields and chained object queries.
/// Refreshed after virtual calls; never borrowed across a service invocation.
#[repr(C)]
pub struct LodLiveState {
    pub selected: i32,
    pub two_sided_bias: i32,
    pub curvy: i32,
    pub anim_by_owner: i32,
    pub has_owner: i32,
    pub ortho: i32,
    pub collision_height: f32,
    pub world_lod: i32,
    pub recommended_lod: i32,
    pub texture_count: i32,
}
/// Borrow the SDK array header fields, not a snapshot of its current backing.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LodArrayView {
    pub data: *const *const c_void,
    pub len: *const i32,
}
impl LodArrayView {
    pub(crate) unsafe fn len(&self) -> usize {
        unsafe { *self.len as usize }
    }
    pub(crate) unsafe fn at<T>(&self, index: usize) -> *const T {
        unsafe { (*self.data).cast::<T>().add(index) }
    }
}
/// Native FFI envelope retained for the legacy draw entry point.
#[repr(C)]
pub struct LodPipelineInput {
    pub context: *mut c_void,
    pub render_services: *const crate::services::Hp2RenderServices,
    pub light_count_stat: *mut i32,
    pub service: unsafe extern "C" fn(*mut c_void, u32, i32, *mut c_void) -> i32,
    pub live: *const LodLiveState,
    pub shared: crate::mesh_shared::MeshSharedInput,
    pub shared_stage: *mut i32,
    pub span: *const *mut c_void,
    pub faces: LodArrayView,
    pub special_faces: LodArrayView,
    pub wedges: LodArrayView,
    pub materials: LodArrayView,
    pub levels: LodArrayView,
    pub collapse_points: LodArrayView,
    pub collapse_wedges: LodArrayView,
    pub model_verts: i32,
    pub special_verts: *const i32,
    pub collapse_count: i32,
    pub lod_min: i32,
    pub lod_strength: f32,
    pub lod_bias: f32,
    pub global_lod: f32,
    pub global_adjust: f32,
    pub rect: [i32; 4],
    pub fx: f32,
    pub mode: *const i32,
    pub fix: f32,
    pub morph: *const f32,
    pub morph_enabled: i32,
    pub wire: i32,
    pub rend_wire: i32,
    pub skeletal: i32,
    pub bones: *const i32,
    pub blend: *const i32,
    pub software: i32,
    pub not_weapon: i32,
    pub editor: *const i32,
    pub anim_stats: *const i32,
    pub scale_glow: *const f32,
    pub ambient_glow: *const u8,
    pub fatness: *const u8,
    pub opacity: *const f32,
    pub mirror: *const f32,
    pub clip: [*const f32; 4],
    pub projection: *const f32,
    pub screen: [*const f32; 4],
    pub coords: *const FCoords,
    pub debug_coords: *const FCoords,
    pub location: *const FVector,
    pub flags: u32,
    pub u_scale: *mut f32,
    pub v_scale: *mut f32,
    pub pivots: *const FVector,
    pub parents: *const i32,
    pub pivot_count: i32,
    pub influences: LodArrayView,
    pub weapon_bone: i32,
    pub weapon_coords: FCoords,
    pub randoms: *const f32,
    /// `Owner`. Read-only mirror access (`AActor.texture`/`.region.zone`/
    /// `.level`) for environment-map lookup; never a live-owner virtual call.
    pub owner: *const c_void,
}
/// Callback-free values consumed by the canonical LOD draw policy.
///
/// Native operation services are supplied through [`LodDrawOps`], while the
/// shared mesh host state contains only allocation and geometry data.
pub struct LodDrawState {
    pub light_count_stat: *mut i32,
    pub live: *const LodLiveState,
    pub shared: crate::mesh_shared::MeshSharedState,
    pub shared_stage: *mut i32,
    pub span: *const *mut c_void,
    pub faces: LodArrayView,
    pub special_faces: LodArrayView,
    pub wedges: LodArrayView,
    pub materials: LodArrayView,
    pub levels: LodArrayView,
    pub collapse_points: LodArrayView,
    pub collapse_wedges: LodArrayView,
    pub model_verts: i32,
    pub special_verts: *const i32,
    pub collapse_count: i32,
    pub lod_min: i32,
    pub lod_strength: f32,
    pub lod_bias: f32,
    pub global_lod: f32,
    pub global_adjust: f32,
    pub rect: [i32; 4],
    pub fx: f32,
    pub mode: *const i32,
    pub fix: f32,
    pub morph: *const f32,
    pub morph_enabled: i32,
    pub wire: i32,
    pub rend_wire: i32,
    pub skeletal: i32,
    pub bones: *const i32,
    pub blend: *const i32,
    pub software: i32,
    pub not_weapon: i32,
    pub editor: *const i32,
    pub anim_stats: *const i32,
    pub scale_glow: *const f32,
    pub ambient_glow: *const u8,
    pub fatness: *const u8,
    pub opacity: *const f32,
    pub mirror: *const f32,
    pub clip: [*const f32; 4],
    pub projection: *const f32,
    pub screen: [*const f32; 4],
    pub coords: *const FCoords,
    pub debug_coords: *const FCoords,
    pub location: *const FVector,
    pub flags: u32,
    pub u_scale: *mut f32,
    pub v_scale: *mut f32,
    pub pivots: *const FVector,
    pub parents: *const i32,
    pub pivot_count: i32,
    pub influences: LodArrayView,
    pub weapon_bone: i32,
    pub weapon_coords: FCoords,
    pub randoms: *const f32,
    pub owner: *const AActor,
}

impl LodDrawState {
    fn from_legacy(input: &LodPipelineInput) -> Self {
        let shared = input.shared;
        Self {
            light_count_stat: input.light_count_stat,
            live: input.live,
            shared: crate::mesh_shared::MeshSharedState {
                geometry: shared.geometry(),
                memory: shared.memory,
                memory_error: shared.memory_error,
                alignment: shared.alignment,
                slow_checks: shared.slow_checks != 0,
            },
            shared_stage: input.shared_stage,
            span: input.span,
            faces: input.faces,
            special_faces: input.special_faces,
            wedges: input.wedges,
            materials: input.materials,
            levels: input.levels,
            collapse_points: input.collapse_points,
            collapse_wedges: input.collapse_wedges,
            model_verts: input.model_verts,
            special_verts: input.special_verts,
            collapse_count: input.collapse_count,
            lod_min: input.lod_min,
            lod_strength: input.lod_strength,
            lod_bias: input.lod_bias,
            global_lod: input.global_lod,
            global_adjust: input.global_adjust,
            rect: input.rect,
            fx: input.fx,
            mode: input.mode,
            fix: input.fix,
            morph: input.morph,
            morph_enabled: input.morph_enabled,
            wire: input.wire,
            rend_wire: input.rend_wire,
            skeletal: input.skeletal,
            bones: input.bones,
            blend: input.blend,
            software: input.software,
            not_weapon: input.not_weapon,
            editor: input.editor,
            anim_stats: input.anim_stats,
            scale_glow: input.scale_glow,
            ambient_glow: input.ambient_glow,
            fatness: input.fatness,
            opacity: input.opacity,
            mirror: input.mirror,
            clip: input.clip,
            projection: input.projection,
            screen: input.screen,
            coords: input.coords,
            debug_coords: input.debug_coords,
            location: input.location,
            flags: input.flags,
            u_scale: input.u_scale,
            v_scale: input.v_scale,
            pivots: input.pivots,
            parents: input.parents,
            pivot_count: input.pivot_count,
            influences: input.influences,
            weapon_bone: input.weapon_bone,
            weapon_coords: input.weapon_coords,
            randoms: input.randoms,
            owner: input.owner.cast(),
        }
    }
}
#[repr(C)]
pub struct LodAllocation {
    pub count: i32,
    pub data: *mut FTransTexture,
}
#[repr(C)]
pub struct LodDebug {
    pub pivots: *const FVector,
    pub parents: *const i32,
    pub pivot_count: i32,
    pub influences: LodArrayView,
    pub weapon_bone: i32,
    pub weapon_coords: FCoords,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LodLine {
    pub color: FPlane,
    pub flags: u32,
    pub a: FVector,
    pub b: FVector,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LodTexture {
    pub object: *mut UTexture,
    pub info: *mut FTextureInfo,
    pub u_scale: f32,
    pub v_scale: f32,
    pub lod: i32,
    pub u_size: i32,
    pub v_size: i32,
}
#[repr(C)]
pub struct LodLightVertex {
    pub sample: *mut FTransTexture,
    pub flags: u32,
}
#[repr(C)]
pub struct LodAnim {
    pub frames: i32,
    pub frame: f32,
    pub alpha: f32,
    pub tween: i32,
    pub has_sequence: i32,
    pub found_sequence: i32,
    pub stat_line: i32,
    pub canvas_height: i32,
}
/// Coordinate system used to obtain the initial LOD vertex frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LodCoordinateMode {
    /// Transform vertices in the actor's current coordinates.
    Actor,
    /// Transform vertices in the engine's unit coordinates.
    Unit,
}

/// Animation owner selected by the source LOD animation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LodAnimationOwner {
    /// Use the owning actor's animation sequence.
    Owner,
    /// Use the drawn actor's animation sequence.
    Actor,
}

/// Kind of animation sequence queried by the LOD renderer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LodAnimationKind {
    /// Query the skeletal animation interface.
    Skeletal,
    /// Query the ordinary mesh animation interface.
    Mesh,
}
/// Supplies the SDK-facing leaves used by the callback-free LOD draw policy.
///
/// Implementations perform one native operation per method; this trait keeps
/// operation selection and draw ordering in [`draw_with_ops`].
pub trait LodDrawOps: crate::mesh_shared::MeshSharedOps<Error = ()> {
    /// Records the initial LOD vertex subset.
    fn record_vertex_subset(&mut self, count: i32) -> Result<(), ()>;
    /// Enables skeletal debug display.
    fn enable_skeletal_debug(&mut self) -> Result<(), ()>;
    /// Allocates a vertex array with the requested capacity.
    fn allocate_vertices(&mut self, count: i32) -> Result<*mut FTransTexture, ()>;
    /// Transforms mesh vertices using the requested native coordinate mode.
    fn transform_vertices(
        &mut self,
        allocation: &mut LodAllocation,
        coordinate_mode: LodCoordinateMode,
    ) -> Result<(), ()>;
    /// Reads skeletal debug data into `debug`.
    fn skeletal_debug(&mut self, debug: &mut LodDebug) -> Result<(), ()>;
    /// Draws a world-space debug line.
    fn draw_line(&mut self, line: &LodLine) -> Result<(), ()>;
    /// Draws a screen-space debug line.
    fn draw_screen_line(&mut self, line: &LodLine) -> Result<(), ()>;
    /// Completes the wireframe rendering phase.
    fn finish_wireframe(&mut self) -> Result<(), ()>;
    /// Checks and starts material rendering for a non-empty face batch.
    fn begin_material_batch(&mut self) -> Result<(), ()>;
    /// Retrieves the mesh texture at `index`.
    fn mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()>;
    /// Resolves a mesh texture and its current LOD.
    fn resolve_mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()>;
    /// Locks a resolved mesh texture.
    fn lock_mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()>;
    /// Completes native material preparation.
    fn finish_material_preparation(&mut self) -> Result<(), ()>;
    /// Locks the environment texture.
    fn lock_environment(&mut self, texture: &mut *mut UTexture) -> Result<(), ()>;
    /// Adds actor lighting setup flags to `flags`.
    fn setup_actor_lighting(&mut self, flags: &mut u32) -> Result<(), ()>;
    /// Reports that a non-empty face batch is ready for lighting.
    fn begin_vertex_lighting(&mut self) -> Result<(), ()>;
    /// Uses the native light manager to light and fog one vertex.
    fn light_and_fog(&mut self, vertex: &mut LodLightVertex, flags: u32) -> Result<(), ()>;
    /// Lights a vertex with the Rust-owned light manager.
    fn light_and_fog_rust(
        &mut self,
        vertex: &mut crate::light::FogVertex,
        flags: u32,
    ) -> Result<bool, ()>;
    /// Reports the visible face count to native rendering statistics.
    fn record_visible_faces(&mut self, count: i32) -> Result<(), ()>;
    /// Finishes actor lighting.
    fn finish_actor_lighting(&mut self) -> Result<(), ()>;
    /// Starts native texture rendering.
    fn begin_texture_rendering(&mut self) -> Result<(), ()>;
    /// Unlocks one indexed mesh texture.
    fn unlock_mesh_texture(&mut self, index: i32) -> Result<(), ()>;
    /// Releases temporary vertex storage after the wireframe pass.
    fn release_wire_vertices(&mut self) -> Result<(), ()>;
    /// Reports whether the Rust-owned light manager is active.
    fn uses_rust_light_manager(&mut self) -> Result<bool, ()>;
    /// Unlocks the environment texture.
    fn unlock_environment(&mut self, texture: &mut *mut UTexture) -> Result<(), ()>;
    /// Completes mesh rendering statistics.
    fn finish_mesh_draw(&mut self) -> Result<(), ()>;
    /// Resolves whether an animation sequence exists.
    fn animation_sequence(&mut self, owner: LodAnimationOwner, anim: &mut LodAnim)
        -> Result<(), ()>;
    /// Retrieves animation sequence and display state.
    fn animation_details(&mut self, kind: LodAnimationKind, anim: &mut LodAnim)
        -> Result<(), ()>;
    /// Draws animation statistics for the current frame.
    fn draw_animation_stats(&mut self, vertex_count: i32, anim: &mut LodAnim)
        -> Result<(), ()>;
}

struct LegacyLodDrawOps {
    context: *mut c_void,
    service: unsafe extern "C" fn(*mut c_void, u32, i32, *mut c_void) -> i32,
    shared: crate::mesh_shared::MeshSharedInput,
    render_services: *const crate::services::Hp2RenderServices,
}

impl LegacyLodDrawOps {
    unsafe fn call<T>(&mut self, op: u32, n: i32, data: &mut T) -> Result<(), ()> {
        if unsafe { (self.service)(self.context, op, n, (data as *mut T).cast()) } != 0 {
            Ok(())
        } else {
            Err(())
        }
    }

    unsafe fn event(&mut self, op: u32, n: i32) -> Result<(), ()> {
        unsafe { self.call(op, n, &mut 0i32) }
    }
}
impl crate::mesh_shared::MeshSharedOps for LegacyLodDrawOps {
    type Error = ();

    unsafe fn allocate_bytes(
        &mut self,
        memory: *mut c_void,
        bytes: usize,
        alignment: i32,
        slow_checks: bool,
        memory_error: *mut i32,
    ) -> Result<*mut c_void, ()> {
        unsafe {
            match hp2_core::mem::push_bytes(
                &mut *memory.cast::<hp2_abi::engine::generated::FMemStack>(),
                bytes as i32,
                alignment,
                slow_checks,
                c"UnMem.h",
                &*self.shared.memory_services,
            ) {
                Ok(ptr) => Ok(ptr.cast()),
                Err(frame) => {
                    *memory_error = frame;
                    Err(())
                }
            }
        }
    }

    fn batch_capacity(&mut self) -> Result<usize, ()> {
        let mut capacity = 0i32;
        unsafe {
            if (self.shared.service)(self.shared.context, 0, (&mut capacity as *mut i32).cast())
                != 0
            {
                Ok(capacity as usize)
            } else {
                Err(())
            }
        }
    }

    fn draw_triangles(
        &mut self,
        draw: &mut crate::mesh_shared::MeshBatchDraw,
    ) -> Result<(), ()> {
        unsafe {
            if (self.shared.service)(
                self.shared.context,
                1,
                (draw as *mut crate::mesh_shared::MeshBatchDraw).cast(),
            ) != 0
            {
                Ok(())
            } else {
                Err(())
            }
        }
    }

    fn subsurface_begin(&mut self) -> Result<(), ()> {
        unsafe { self.shared_event(2) }
    }
    fn surface_ready(&mut self) -> Result<(), ()> {
        unsafe { self.shared_event(4) }
    }
    fn surface_timer_begin(&mut self) -> Result<(), ()> {
        unsafe { self.shared_event(3) }
    }
    fn surface_timer_end(&mut self) -> Result<(), ()> {
        unsafe { self.shared_event(5) }
    }
    fn check_allocation_size(&mut self, bytes: &mut usize) -> Result<(), ()> {
        unsafe {
            if (self.shared.service)(
                self.shared.context,
                6,
                (bytes as *mut usize).cast(),
            ) != 0
            {
                Ok(())
            } else {
                Err(())
            }
        }
    }
}

impl LegacyLodDrawOps {
    unsafe fn shared_event(&mut self, op: u32) -> Result<(), ()> {
        let mut value = 0u8;
        if unsafe {
            (self.shared.service)(
                self.shared.context,
                op,
                (&mut value as *mut u8).cast(),
            )
        } != 0
        {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl LodDrawOps for LegacyLodDrawOps {
    fn record_vertex_subset(&mut self, count: i32) -> Result<(), ()> {
        unsafe { self.event(1, count) }
    }
    fn enable_skeletal_debug(&mut self) -> Result<(), ()> {
        unsafe { self.event(2, 0) }
    }
    fn allocate_vertices(&mut self, count: i32) -> Result<*mut FTransTexture, ()> {
        let mut allocation = LodAllocation { count, data: core::ptr::null_mut() };
        unsafe { self.call(3, 0, &mut allocation)?; }
        Ok(allocation.data)
    }
    fn transform_vertices(
        &mut self,
        allocation: &mut LodAllocation,
        coordinate_mode: LodCoordinateMode,
    ) -> Result<(), ()> {
        let mode = i32::from(coordinate_mode == LodCoordinateMode::Unit);
        unsafe { self.call(4, mode, allocation) }
    }
    fn skeletal_debug(&mut self, debug: &mut LodDebug) -> Result<(), ()> {
        unsafe { self.call(33, 0, debug) }
    }
    fn draw_line(&mut self, line: &LodLine) -> Result<(), ()> {
        let mut line = *line;
        unsafe { self.call(6, 0, &mut line) }
    }
    fn draw_screen_line(&mut self, line: &LodLine) -> Result<(), ()> {
        let mut line = *line;
        unsafe { self.call(7, 0, &mut line) }
    }
    fn finish_wireframe(&mut self) -> Result<(), ()> { unsafe { self.event(8, 0) } }
    fn release_wire_vertices(&mut self) -> Result<(), ()> { unsafe { self.event(9, 0) } }
    fn begin_material_batch(&mut self) -> Result<(), ()> { unsafe { self.event(11, 0) } }
    fn mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        unsafe { self.call(12, index, texture) }
    }
    fn resolve_mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        unsafe { self.call(29, index, texture) }
    }
    fn lock_mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        unsafe { self.call(30, index, texture) }
    }
    fn finish_material_preparation(&mut self) -> Result<(), ()> { unsafe { self.event(34, 0) } }
    fn lock_environment(&mut self, texture: &mut *mut UTexture) -> Result<(), ()> {
        unsafe { self.call(13, 0, texture) }
    }
    fn setup_actor_lighting(&mut self, flags: &mut u32) -> Result<(), ()> { unsafe { self.call(14, 0, flags) } }
    fn begin_vertex_lighting(&mut self) -> Result<(), ()> { unsafe { self.event(15, 0) } }
    fn light_and_fog(&mut self, vertex: &mut LodLightVertex, _flags: u32) -> Result<(), ()> { unsafe { self.call(16, 0, vertex) } }
    fn light_and_fog_rust(
        &mut self,
        vertex: &mut crate::light::FogVertex,
        flags: u32,
    ) -> Result<bool, ()> {
        Ok(unsafe {
            crate::light_manager::light_and_fog(&*self.render_services, vertex, flags)
        })
    }
    fn record_visible_faces(&mut self, count: i32) -> Result<(), ()> { unsafe { self.event(17, count) } }
    fn finish_actor_lighting(&mut self) -> Result<(), ()> { unsafe { self.event(22, 0) } }
    fn begin_texture_rendering(&mut self) -> Result<(), ()> { unsafe { self.event(23, 0) } }
    fn unlock_mesh_texture(&mut self, index: i32) -> Result<(), ()> { unsafe { self.event(24, index) } }
    fn unlock_environment(&mut self, texture: &mut *mut UTexture) -> Result<(), ()> { unsafe { self.call(25, 0, texture) } }
    fn finish_mesh_draw(&mut self) -> Result<(), ()> { unsafe { self.event(26, 0) } }
    fn animation_sequence(&mut self, owner: LodAnimationOwner, anim: &mut LodAnim) -> Result<(), ()> {
        unsafe { self.call(27, i32::from(owner == LodAnimationOwner::Owner), anim) }
    }
    fn animation_details(&mut self, kind: LodAnimationKind, anim: &mut LodAnim) -> Result<(), ()> {
        unsafe { self.call(31, i32::from(kind == LodAnimationKind::Skeletal), anim) }
    }
    fn draw_animation_stats(&mut self, vertex_count: i32, anim: &mut LodAnim) -> Result<(), ()> { unsafe { self.call(28, vertex_count, anim) } }
    fn uses_rust_light_manager(&mut self) -> Result<bool, ()> {
        let mut active = 0;
        unsafe { self.call(35, 0, &mut active)?; }
        Ok(active != 0)
    }
}


const FLAT: u32 = 0x4000;
const TWO: u32 = 0x100;
const SELECTED: u32 = 0x2000000;
const TRANSLUCENT: u32 = 4;
const HIGHLIGHTED: u32 = 0x10000000;
const ENVIRONMENT: u32 = 0x10;
fn v(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}
fn plane(p: FVector, w: f32) -> FPlane {
    FPlane {
        x: p.x,
        y: p.y,
        z: p.z,
        w,
    }
}
fn xyz(p: FPlane) -> FVector {
    v(p.x, p.y, p.z)
}
fn sub(a: FVector, b: FVector) -> FVector {
    v(a.x - b.x, a.y - b.y, a.z - b.z)
}
fn add(a: FVector, b: FVector) -> FVector {
    v(a.x + b.x, a.y + b.y, a.z + b.z)
}
fn scale(a: FVector, b: f32) -> FVector {
    v(a.x * b, a.y * b, a.z * b)
}
fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}
unsafe fn outcode(p: FVector, i: &LodDrawState) -> u8 {
    unsafe {
        (u8::from((*i.clip[0]).mul_add(p.z, p.x) < 0.) << 2)
            | (u8::from((*i.clip[1]).mul_add(p.z, -p.x) < 0.) << 3)
            | (u8::from((*i.clip[2]).mul_add(p.z, p.y) < 0.) << 4)
            | (u8::from((*i.clip[3]).mul_add(p.z, -p.y) < 0.) << 5)
    }
}
fn draw_line<O: LodDrawOps>(
    ops: &mut O,
    color: FPlane,
    flags: u32,
    a: FVector,
    b: FVector,
    screen: bool,
) -> Result<(), ()> {
    let line = LodLine { color, flags, a, b };
    if screen {
        ops.draw_screen_line(&line)
    } else {
        ops.draw_line(&line)
    }
}

// The engine's Sort is deliberately not stable. Preserve equal-key ordering,
// including its selection-sort cutoff, rather than using Rust's sort algorithm.
pub(crate) fn sort_faces<T>(a: &mut [(*const T, i32)]) {
    if a.len() < 2 {
        return;
    }
    let mut stack = [(0usize, 0usize); 32];
    stack[0] = (0, a.len() - 1);
    let mut top = 1;
    while top != 0 {
        top -= 1;
        let (mut lo, mut hi) = stack[top];
        loop {
            let count = hi - lo + 1;
            if count <= 8 {
                for end in (lo + 1..=hi).rev() {
                    let mut m = lo;
                    for j in lo + 1..=end {
                        if a[m].1.wrapping_sub(a[j].1) > 0 {
                            m = j
                        }
                    }
                    a.swap(m, end);
                }
                break;
            }
            a.swap(lo + count / 2, lo);
            let mut l = lo;
            let mut r = hi + 1;
            loop {
                loop {
                    l += 1;
                    if l > hi || a[lo].1.wrapping_sub(a[l].1) > 0 {
                        break;
                    }
                }
                loop {
                    r -= 1;
                    if r <= lo || a[lo].1.wrapping_sub(a[r].1) < 0 {
                        break;
                    }
                }
                if l > r {
                    break;
                }
                a.swap(l, r);
            }
            a.swap(lo, r);
            if r as isize - 1 - lo as isize >= hi as isize - l as isize {
                if lo + 1 < r {
                    stack[top] = (lo, r - 1);
                    top += 1
                }
                if hi > l { lo = l } else { break }
            } else {
                if hi > l {
                    stack[top] = (l, hi);
                    top += 1
                }
                if lo + 1 < r { hi = r - 1 } else { break }
            }
        }
    }
}

/// Executes the source-ordered LOD draw policy through typed native operations.
///
/// # Safety
/// Every pointer in `input` must refer to valid SDK storage for the full draw.
pub unsafe fn draw_with_ops<O: LodDrawOps>(
    i: &mut LodDrawState,
    ops: &mut O,
) -> Result<(), ()> {
    unsafe {
        let lod = mesh::lod_subset(&mesh::LodInputs {
            lod_strength: i.lod_strength,
            collapse_count: i.collapse_count,
            model_verts: i.model_verts,
            lod_min_verts: i.lod_min,
            lod_bias: i.lod_bias,
            global_shape_lod: i.global_lod,
            global_shape_lod_adjust: i.global_adjust,
            x1: i.rect[0],
            y1: i.rect[1],
            x2: i.rect[2],
            y2: i.rect[3],
            fx: i.fx,
            shape_lod_mode: *i.mode,
            shape_lod_fix: i.fix,
        });
        let mut count = lod.vertex_subset;
        ops.record_vertex_subset(count)?;
        i.wire = i32::from(i.wire != 0 || (*i.live).ortho != 0 || i.rend_wire != 0);
        if i.skeletal != 0 && i.wire != 0 {
            ops.enable_skeletal_debug()?
        }
        let all = ops.allocate_vertices(count + *i.special_verts)?;
        let samples = all.add(*i.special_verts as usize);
        let mut frame = LodAllocation { count, data: all };
        ops.transform_vertices(
            &mut frame,
            if i.wire != 0 {
                LodCoordinateMode::Unit
            } else {
                LodCoordinateMode::Actor
            },
        )?;
        count = frame.count;
        if i.skeletal != 0 {
            let mut debug = LodDebug {
                pivots: core::ptr::null(),
                parents: core::ptr::null(),
                pivot_count: 0,
                influences: LodArrayView {
                    data: core::ptr::null(),
                    len: core::ptr::null(),
                },
                weapon_bone: -1,
                weapon_coords: i.weapon_coords,
            };
            ops.skeletal_debug(&mut debug)?;
            i.pivots = debug.pivots;
            i.parents = debug.parents;
            i.pivot_count = debug.pivot_count;
            i.influences = debug.influences;
            i.weapon_bone = debug.weapon_bone;
            i.weapon_coords = debug.weapon_coords;
        }
        let do_morph = i.morph_enabled != 0 && *i.mode & 1 != 0 && *i.morph > 0.;
        let non_morph = if do_morph {
            mesh::non_morph_subset(lod.target_subset, *i.morph)
        } else {
            0
        };
        let do_morph = do_morph && non_morph < count;
        if do_morph {
            let inv = mesh::morph_inv(*i.morph, lod.target_subset);
            for n in (non_morph + 1..count).rev() {
                let mut alpha = mesh::morph_alpha(n, non_morph, inv);
                let target = *i.collapse_points.at::<u16>(n as usize) as usize;
                if target > 0 {
                    (*samples.add(n as usize)).transform.point = mesh::morph_point(
                        (*samples.add(n as usize)).transform.point,
                        (*samples.add(target)).transform.point,
                        alpha,
                    )
                } else {
                    alpha = 0.
                }
                (*samples.add(n as usize)).u = alpha;
            }
        }
        let mut weapon_out = 0x3c;
        let mut mesh_out = 0x3c;
        for n in 0..*i.special_verts as usize {
            let p = &mut *all.add(n);
            p.normal = plane(v(0., 0., 0.), 0.);
            p.transform.flags = outcode(p.transform.point, i);
            weapon_out &= p.transform.flags;
        }
        for n in 0..count as usize {
            let p = &mut *samples.add(n);
            p.light.x = 0.;
            p.normal = plane(v(0., 0., 0.), 0.);
            p.transform.flags = outcode(p.transform.point, i);
            mesh_out &= p.transform.flags;
        }
        crate::mesh_shared::HasSpecialCoords = 0;
        if weapon_out == 0 {
            let face = &*i.special_faces.at::<LodFace>(0);
            let a = &*all.add(face.wedges[0] as usize);
            let b = &*all.add(face.wedges[1] as usize);
            let c = &*all.add(face.wedges[2] as usize);
            if a.transform.flags & b.transform.flags & c.transform.flags == 0 {
                let x = math::vector_safe_normal(sub(b.transform.point, a.transform.point));
                let y = math::vector_safe_normal(mesh::cross(
                    x,
                    sub(a.transform.point, c.transform.point),
                ));
                let axes = FCoords {
                    origin: v(0., 0., 0.),
                    x_axis: x,
                    y_axis: y,
                    z_axis: mesh::cross(y, x),
                };
                let unit = FCoords {
                    origin: scale(add(a.transform.point, c.transform.point), -0.5),
                    x_axis: v(1., 0., 0.),
                    y_axis: v(0., 1., 0.),
                    z_axis: v(0., 0., 1.),
                };
                crate::mesh_shared::SpecialCoords = math::coords_mul(unit, axes);
                crate::mesh_shared::HasSpecialCoords = 1;
            }
        }
        if i.wire != 0 && i.skeletal != 0 && *i.bones != 0 && i.pivot_count > 1 {
            for b in 0..i.pivot_count as usize {
                let color = plane(
                    v(
                        (*i.randoms.add((b * 3) & 255)).mul_add(2., 0.2),
                        (*i.randoms.add((b * 3 + 1) & 255)).mul_add(2., 0.2),
                        (*i.randoms.add((b * 3 + 2) & 255)).mul_add(2., 0.2),
                    ),
                    0.,
                );
                let parent = *i.parents.add(b) as usize;
                if parent != b {
                    draw_line(ops, color, 0, *i.pivots.add(b), *i.pivots.add(parent), false)?
                }
            }
        }
        if i.wire != 0 {
            let color = if (*i.live).selected != 0 {
                plane(v(0.2, 0.8, 0.1), 0.)
            } else {
                plane(v(0.6, 0.4, 0.1), 0.)
            };
            let mut f = 0;
            while f < i.faces.len() {
                if (*i.levels.at::<u16>(f) as i32) <= count {
                    let face = i.faces.at::<LodFace>(f);
                    let mut ids = [0; 3];
                    for w in 0..3 {
                        let mut k = (*face).wedges[w] as usize;
                        while (*i.wedges.at::<LodWedge>(k)).vertex as i32 >= count {
                            k = *i.collapse_wedges.at::<u16>(k) as usize
                        }
                        ids[w] = (*i.wedges.at::<LodWedge>(k)).vertex as usize
                    }
                    let mut p = (*samples.add(ids[2])).transform.point;
                    for id in ids {
                        let q = (*samples.add(id)).transform.point;
                        draw_line(ops, color, 2, p, q, false)?;
                        p = q
                    }
                }
                f += 1;
            }
            let mut f = 0;
            while f < i.special_faces.len() {
                let face = i.special_faces.at::<LodFace>(f);
                let ids = (*face).wedges;
                let mut p = (*all.add(ids[2] as usize)).transform.point;
                for id in ids {
                    let q = (*all.add(id as usize)).transform.point;
                    draw_line(ops, color, 2, p, q, false)?;
                    p = q
                }
                f += 1;
            }
            if i.skeletal != 0 && *i.blend != 0 && i.influences.len() != 0 {
                let influences = i.influences.at::<u8>(0);
                for n in 0..count as usize {
                    let color = plane(
                        match *influences.add(n) {
                            0 => v(0., 0., 0.),
                            1 => v(0., 0.8, 0.),
                            2 => v(0.8, 0.2, 0.),
                            3 => v(0.8, 0., 1.),
                            4 => v(0.15, 0.75, 1.),
                            _ => v(0.9, 0.9, 0.9),
                        },
                        1.,
                    );
                    if (*i.live).ortho == 0 {
                        let mut p = math::vector_transform_point(
                            (*samples.add(n)).transform.point,
                            *i.debug_coords,
                        );
                        let sx = *i.screen[2];
                        let sy = *i.screen[3];
                        p.z = 1. / p.z;
                        p.x = mesh::app_round((p.x * *i.projection).mul_add(p.z, sx)) as f32;
                        p.y = mesh::app_round((p.y * *i.projection).mul_add(p.z, sy)) as f32;
                        draw_line(
                            ops,
                            color,
                            0,
                            add(p, v(1., 1., 0.)),
                            add(p, v(-1., -1., 0.)),
                            true,
                        )?;
                        draw_line(
                            ops,
                            color,
                            0,
                            add(p, v(1., -1., 0.)),
                            add(p, v(-1., 1., 0.)),
                            true,
                        )?
                    }
                }
            }
            ops.finish_wireframe()?;
            ops.release_wire_vertices()?;
            return Ok(());
        }
        if i.skeletal != 0 && i.weapon_bone > -1 {
            crate::mesh_shared::SpecialCoords = i.weapon_coords;
            crate::mesh_shared::HasSpecialCoords = 1;
        }
        let unlit = mesh::unlit(*i.scale_glow, *i.ambient_glow);
        let mut color = v(unlit, unlit, unlit);
        let mut flags = i.flags | FLAT;
        if *i.editor != 0 && flags & SELECTED != 0 {
            color = add(scale(color, 0.4), v(0., 0.6, 0.))
        }
        (*crate::mesh_shared::storage()).unlit_color = color;
        if *i.opacity < 1. {
            flags |= HIGHLIGHTED | TRANSLUCENT
        }
        let depth = i.software != 0 || flags & (HIGHLIGHTED | TRANSLUCENT) != 0;
        let mut wedges = vec![LodWedge::default(); i.wedges.len()];
        let mut used = i.wedges.len() as i32;
        let mut faces: Vec<(*const LodFace, i32)> = Vec::new();
        let mut mat = -1;
        let mut mat_flags = 0;
        let mut sort_bias = 0.;
        if mesh_out == 0 {
            if lod.do_lod {
                used = 0
            }
            for f in 0..i.faces.len() {
                if *i.levels.at::<u16>(f) as i32 > count {
                    continue;
                }
                let face = &*i.faces.at::<LodFace>(f);
                if mat != face.material as i32 {
                    mat = face.material as i32;
                    mat_flags = flags | (*i.materials.at::<LodMaterial>(mat as usize)).flags;
                    sort_bias = if (*i.live).two_sided_bias != 0 && mat_flags & TWO != 0 {
                        (*i.live).collision_height * -0.3
                    } else {
                        0.
                    };
                }
                let mut ids = [0; 3];
                for w in 0..3 {
                    let start = face.wedges[w] as usize;
                    let mut wedge = *i.wedges.at::<LodWedge>(start);
                    if lod.do_lod {
                        if wedges[start].vertex == 0 && wedges[start].u == 0 && wedges[start].v == 0
                        {
                            let mut k = start;
                            while wedge.vertex as i32 >= count {
                                k = *i.collapse_wedges.at::<u16>(k) as usize;
                                wedge.vertex = (*i.wedges.at::<LodWedge>(k)).vertex
                            }
                            wedge.u = (*i.wedges.at::<LodWedge>(k)).u;
                            wedge.v = (*i.wedges.at::<LodWedge>(k)).v;
                            if do_morph && wedge.vertex as i32 > non_morph {
                                let alpha = (*samples.add(wedge.vertex as usize)).u;
                                if alpha.to_bits() != 0 {
                                    let next = *i.collapse_wedges.at::<u16>(k) as usize;
                                    if k != next {
                                        let other = *i.wedges.at::<LodWedge>(next);
                                        if wedge.u != other.u || wedge.v != other.v {
                                            wedge.u = mesh::app_round(
                                                (other.u as f32 - wedge.u as f32)
                                                    .mul_add(alpha, wedge.u as f32),
                                            )
                                                as u8;
                                            wedge.v = mesh::app_round(
                                                (other.v as f32 - wedge.v as f32)
                                                    .mul_add(alpha, wedge.v as f32),
                                            )
                                                as u8;
                                        }
                                    }
                                }
                            }
                            wedges[start] = wedge;
                            used = used.max(start as i32 + 1);
                        } else {
                            wedge = wedges[start]
                        }
                    } else {
                        wedges[start] = wedge
                    }
                    ids[w] = wedge.vertex as usize;
                }
                let a = (*samples.add(ids[0])).transform.point;
                let b = (*samples.add(ids[1])).transform.point;
                let c = (*samples.add(ids[2])).transform.point;
                let normal = mesh::cross(sub(a, b), sub(c, a));
                if (*i.live).curvy == 0 {
                    for id in ids {
                        let p = &mut *samples.add(id);
                        let n = add(xyz(p.normal), normal);
                        p.normal.x = n.x;
                        p.normal.y = n.y;
                        p.normal.z = n.z;
                    }
                }
                if (*samples.add(ids[0])).transform.flags
                    & (*samples.add(ids[1])).transform.flags
                    & (*samples.add(ids[2])).transform.flags
                    == 0
                    && (mat_flags & TWO != 0 || *i.mirror * dot(a, normal) < 0.)
                {
                    for id in ids {
                        (*samples.add(id)).light.x = -1.
                    }
                    let key = if !depth {
                        0
                    } else if i.not_weapon != 0 {
                        mesh::depth_key_height(a.z, b.z, c.z, sort_bias)
                    } else {
                        mesh::depth_key_distance(a, b, c, v(0., -8., 0.))
                    };
                    faces.push((face, key));
                }
            }
        }
        if !faces.is_empty() {
            // These are original pre-texture locals, not live per-vertex reads.
            let fatten = *i.fatness != 128;
            let fat = mesh::fatness_offset(*i.fatness);
            if depth {
                sort_faces(&mut faces)
            }
            ops.begin_material_batch()?;
            let mut environment = core::ptr::null_mut();
            let texture_lod = ((*i.live).world_lod - 1).max((*i.live).recommended_lod);
            let mut n = 0;
            while n < (*i.live).texture_count {
                let mut t = LodTexture {
                    object: core::ptr::null_mut(),
                    info: core::ptr::null_mut(),
                    u_scale: 0.,
                    v_scale: 0.,
                    lod: 0,
                    u_size: 0,
                    v_size: 0,
                };
                ops.mesh_texture(n, &mut t)?;
                if !t.object.is_null() {
                    ops.resolve_mesh_texture(n, &mut t)?;
                    t.lod = texture_lod.min(t.lod);
                    ops.lock_mesh_texture(n, &mut t)?;
                    environment = t.object;
                }
                n += 1;
            }
            ops.finish_material_preparation()?;
            // `Owner->Texture`, `Owner->Region.Zone->EnvironmentMap`,
            // `Owner->Level->EnvironmentMap`: AActor/AZoneInfo mirror reads,
            // not a leaf (ALevelInfo embeds AZoneInfo as `base`, same offset).
            let owner = i.owner;
            let zone = (*owner).region.zone.cast::<AZoneInfo>();
            let level = (*owner).level.cast::<AZoneInfo>();
            let environments = [
                (*owner).texture.cast::<UTexture>(),
                if zone.is_null() {
                    core::ptr::null_mut()
                } else {
                    (*zone).environment_map.cast::<UTexture>()
                },
                (*level).environment_map.cast::<UTexture>(),
            ];
            for choice in environments {
                if !choice.is_null() {
                    environment = choice;
                    break;
                }
            }
            if environment.is_null() {
                return Ok(());
            }
            ops.lock_environment(&mut environment)?;
            ops.setup_actor_lighting(&mut flags)?;
            let rust_manager = ops.uses_rust_light_manager()?;
            ops.begin_vertex_lighting()?;
            let loc = math::vector_transform_point(*i.location, *i.coords);
            for n in 0..count as usize {
                let p = &mut *samples.add(n);
                if p.light.x != -1. {
                    continue;
                }
                if (*i.live).curvy != 0 {
                    p.normal = plane(sub(p.transform.point, loc), 0.);
                }
                let normal = xyz(p.normal);
                let inv = mesh::div_sqrt_approx(mesh::size_squared(normal) + 0.001);
                p.normal = plane(scale(normal, inv), p.normal.w * inv);
                if fatten {
                    p.transform.point = add(p.transform.point, scale(xyz(p.normal), fat));
                    p.transform.flags = outcode(p.transform.point, i);
                }
                if rust_manager {
                    let mut vertex = crate::light::FogVertex {
                        point: p.transform.point,
                        normal: xyz(p.normal),
                        light: [0.; 4],
                        fog: [0.; 4],
                    };
                    if ops.light_and_fog_rust(&mut vertex, flags)? && !i.light_count_stat.is_null() {
                        *i.light_count_stat = (*i.light_count_stat)
                            .wrapping_add(crate::light_manager::light_count() as i32);
                    }
                    p.light = FPlane {
                        x: vertex.light[0],
                        y: vertex.light[1],
                        z: vertex.light[2],
                        w: vertex.light[3],
                    };
                    p.fog = FPlane {
                        x: vertex.fog[0],
                        y: vertex.fog[1],
                        z: vertex.fog[2],
                        w: vertex.fog[3],
                    };
                } else {
                    // A foreign override can throw or replace GLightManager. Do not
                    // prepare another vertex until this virtual call has returned.
                    ops.light_and_fog(&mut LodLightVertex { sample: p, flags }, flags)?;
                }
                if i.software != 0 {
                    p.light.x *= *i.opacity;
                    p.light.y *= *i.opacity;
                    p.light.z *= *i.opacity;
                    p.light.w *= *i.opacity;
                }
                p.light.w = *i.opacity;
                if p.transform.flags == 0 {
                    p.transform.rz = *i.projection / p.transform.point.z;
                    p.transform.screen_x =
                        p.transform.point.x.mul_add(p.transform.rz, *i.screen[0]);
                    p.transform.screen_y =
                        p.transform.point.y.mul_add(p.transform.rz, *i.screen[1]);
                    p.transform.int_y = p.transform.screen_y.floor() as i32;
                }
            }
            ops.record_visible_faces(faces.len() as i32)?;
            let verts = ops.allocate_vertices(used)?;
            for w in 0..used as usize {
                core::ptr::copy_nonoverlapping(
                    samples.add(wedges[w].vertex as usize),
                    verts.add(w),
                    1,
                )
            }
            let mut shared_state = i.shared;
            shared_state.geometry.span = *i.span;
            crate::mesh_shared::timed_init_with_ops(ops, verts, used)?;
            mat = -1;
            let mut tex = LodTexture {
                object: core::ptr::null_mut(),
                info: core::ptr::null_mut(),
                u_scale: 0.,
                v_scale: 0.,
                lod: 0,
                u_size: 0,
                v_size: 0,
            };
            for (f, _) in &faces {
                // SDK callbacks may mutate the native face. Do not retain a
                // Rust shared borrow across any native operation.
                let face = *f;
                if mat != (*face).material as i32 {
                    mat = (*face).material as i32;
                    let material = &*i.materials.at::<LodMaterial>(mat as usize);
                    mat_flags = flags | material.flags;
                    let texture = if mat_flags & ENVIRONMENT != 0 {
                        -1
                    } else {
                        material.texture
                    };
                    tex = crate::mesh_shared::texture(texture);
                    if texture >= 0 && tex.object.is_null() {
                        tex = crate::mesh_shared::texture(-1);
                    }
                    *i.u_scale = tex.u_scale * tex.u_size as f32 / 256.;
                    *i.v_scale = tex.v_scale * tex.v_size as f32 / 256.;
                    sort_bias = if (*i.live).two_sided_bias != 0 && mat_flags & TWO != 0 {
                        -0.001
                    } else {
                        0.
                    };
                }
                let mut pts = [core::ptr::null_mut(); 6];
                for w in 0..3 {
                    let id = (*face).wedges[w] as usize;
                    let p = &mut *verts.add(id);
                    p.u = wedges[id].u as f32 * *i.u_scale;
                    p.v = wedges[id].v as f32 * *i.v_scale;
                    p.transform.rz /= p.transform.rz.mul_add(sort_bias, 1.);
                    pts[w] = p;
                }
                if *i.mirror == -1. {
                    pts.swap(0, 2)
                }
                shared_state.geometry.span = *i.span;
                *i.shared_stage = 1;
                crate::mesh_shared::subsurface_with_ops(
                    &shared_state,
                    ops,
                    tex.info.cast(),
                    pts.as_mut_ptr(),
                    mat_flags,
                )?;
                *i.shared_stage = 0;
            }
            crate::mesh_shared::timed_flush_with_ops(ops)?;
            ops.finish_actor_lighting()?;
            ops.begin_texture_rendering()?;
            let mut n = 0;
            while n < (*i.live).texture_count {
                ops.unlock_mesh_texture(n)?;
                n += 1;
            }
            ops.unlock_environment(&mut environment)?;
        }
        ops.finish_mesh_draw()?;
        ops.release_wire_vertices()?;
        if *i.anim_stats != 0 {
            let mut anim = LodAnim {
                frames: 0,
                frame: 0.,
                alpha: 0.,
                tween: 0,
                has_sequence: 0,
                found_sequence: 0,
                stat_line: 0,
                canvas_height: 0,
            };
            ops.animation_sequence(
                if (*i.live).anim_by_owner != 0 && (*i.live).has_owner != 0 {
                    LodAnimationOwner::Owner
                } else {
                    LodAnimationOwner::Actor
                },
                &mut anim,
            )?;
            if anim.has_sequence != 0 {
                ops.animation_details(
                    if i.skeletal != 0 {
                        LodAnimationKind::Skeletal
                    } else {
                        LodAnimationKind::Mesh
                    },
                    &mut anim,
                )?;
                if anim.found_sequence != 0 && anim.stat_line + 8 < anim.canvas_height {
                    if anim.frame >= 0. {
                        let f = if anim.frame >= 0. { anim.frame } else { 0. } * anim.frames as f32;
                        let index = f.floor() as i32;
                        anim.alpha = f - index as f32;
                        let a = index % anim.frames;
                        let b = (index + 1) % anim.frames;
                        anim.frame = if a <= b || anim.alpha == 0. {
                            a as f32 + anim.alpha
                        } else {
                            anim.alpha - 1.
                        };
                    } else {
                        anim.tween = 1;
                        anim.alpha = anim.frame;
                    }
                    ops.draw_animation_stats(count, &mut anim)?;
                }
            }
        }
        Ok(())
    }
}
/// Runs the legacy FFI-backed LOD policy entry point.
///
/// # Safety
/// All pointers in `input` must remain valid for the duration of the draw.
pub unsafe fn draw(input: &mut LodPipelineInput) -> Result<(), ()> {
    let mut ops = LegacyLodDrawOps {
        context: input.context,
        service: input.service,
        shared: input.shared,
        render_services: input.render_services,
    };
    let mut state = LodDrawState::from_legacy(input);
    let result = unsafe { draw_with_ops(&mut state, &mut ops) };
    input.wire = state.wire;
    input.pivots = state.pivots;
    input.parents = state.parents;
    input.pivot_count = state.pivot_count;
    input.influences = state.influences;
    input.weapon_bone = state.weapon_bone;
    input.weapon_coords = state.weapon_coords;
    result
}
