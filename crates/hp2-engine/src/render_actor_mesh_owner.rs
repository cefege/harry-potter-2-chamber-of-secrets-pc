//! Canonical Core providers for actor mesh and source software-geometry drawing.
#[path = "render_actor_mesh_provider.rs"]
mod render_actor_mesh_provider;
#[path = "render_actor_lod_provider.rs"]
mod render_actor_lod_provider;
pub(crate) use render_actor_lod_provider::set_high_detail_models;

pub(crate) use render_actor_lod_provider::{
    skeletal_adjust_root_movement, skeletal_bone_coords, skeletal_root_movement,
};

use hp2_abi::engine::generated::{
    self as generated, AActor, FSceneNode, URenderDevice, UViewport,
};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::softln_draw::SoftlnDrawOps;
use std::{ffi::c_void, ptr};

/// Returns a slice over a native array whose storage is owned by a live object.
unsafe fn raw_slice<'a, T>(data: *mut c_void, num: i32, label: &'static str) -> RuntimeResult<&'a [T]> {
    if num < 0 || (num != 0 && data.is_null()) {
        return Err(RuntimeError::new(label));
    }
    Ok(if num == 0 { &[] } else { unsafe { std::slice::from_raw_parts(data.cast::<T>(), num as usize) } })
}

/// Builds the `Owner->Owner` animation view used by every mesh bounds/frame read.
pub(super) unsafe fn owner_view(actor: *mut AActor) -> crate::mesh::OwnerView {
    unsafe {
        let actor_ref = &*actor;
        let parent = actor_ref.owner.cast::<AActor>();
        crate::mesh::OwnerView {
            pose: crate::mesh::ActorPose {
                location: actor_ref.location,
                pre_pivot: actor_ref.pre_pivot,
                rotation: actor_ref.rotation,
                draw_scale: actor_ref.draw_scale,
            },
            anim: crate::mesh::AnimState {
                sequence: i32::from_ne_bytes(actor_ref.anim_sequence),
                frame: actor_ref.anim_frame,
            },
            parent_anim: (!parent.is_null()).then(|| crate::mesh::AnimState {
                sequence: i32::from_ne_bytes((*parent).anim_sequence),
                frame: (*parent).anim_frame,
            }),
            anim_by_owner: actor_ref.b_anim_by_owner(),
        }
    }
}

/// Builds the ordinary `UMesh` view without retaining draw state.
///
/// The returned slices borrow `mesh`'s live arrays; the caller must keep the
/// mesh alive for the duration of the view.
pub(super) unsafe fn ordinary_views(
    mesh: *mut generated::UMesh,
    actor: *mut AActor,
) -> RuntimeResult<(crate::mesh::MeshView<'static>, crate::mesh::OwnerView)> {
    if mesh.is_null() || actor.is_null() {
        return Err(RuntimeError::new("Canonical mesh view requires live mesh and actor"));
    }
    let m = unsafe { &*mesh };
    let verts = unsafe { raw_slice::<u32>(m.verts.data, m.verts.array_num, "Invalid UMesh Verts")? };
    let anim_seqs = unsafe { raw_slice::<crate::mesh::AnimSeqRaw>(m.anim_seqs.data, m.anim_seqs.num, "Invalid UMesh AnimSeqs")? };
    let boxes = unsafe { raw_slice::<crate::mesh::FBoxRaw>(m.bounding_boxes.data, m.bounding_boxes.num, "Invalid UMesh BoundingBoxes")? };
    if m.frame_verts < 0 || verts.len() < m.frame_verts as usize {
        return Err(RuntimeError::new("UMesh vertex storage is shorter than FrameVerts"));
    }
    let bounds = m.base.base.base.bounding_box;
    Ok((
        crate::mesh::MeshView {
            frame_verts: m.frame_verts,
            verts,
            anim_seqs,
            bounding_boxes: boxes,
            bounding_box: crate::mesh::FBoxRaw {
                min: bounds.min,
                max: bounds.max,
                is_valid: i32::from(bounds.is_valid != 0),
            },
            scale: m.scale,
            origin: m.origin,
            rot_origin: m.rot_origin,
        },
        unsafe { owner_view(actor) },
    ))
}

/// Builds the `ULodMesh`/`USkeletalMesh` view without retaining draw state.
///
/// The returned slices borrow `mesh`'s live arrays; the caller must keep the
/// mesh alive for the duration of the view.
pub(super) unsafe fn lod_views(
    mesh: *mut generated::ULodMesh,
    actor: *mut AActor,
) -> RuntimeResult<(crate::mesh::MeshView<'static>, crate::mesh::OwnerView)> {
    if mesh.is_null() || actor.is_null() {
        return Err(RuntimeError::new("Canonical LOD mesh view requires live mesh and actor"));
    }
    let m = unsafe { &*mesh };
    let verts = unsafe { raw_slice::<u32>(m.base.verts.data, m.base.verts.array_num, "Invalid ULodMesh Verts")? };
    let anim_seqs = unsafe { raw_slice::<crate::mesh::AnimSeqRaw>(m.base.anim_seqs.data, m.base.anim_seqs.num, "Invalid ULodMesh AnimSeqs")? };
    let boxes = unsafe { raw_slice::<crate::mesh::FBoxRaw>(m.base.bounding_boxes.data, m.base.bounding_boxes.num, "Invalid ULodMesh BoundingBoxes")? };
    if m.base.frame_verts < 0 || verts.len() < m.base.frame_verts as usize {
        return Err(RuntimeError::new("ULodMesh vertex storage is shorter than FrameVerts"));
    }
    let bounds = m.base.base.base.base.bounding_box;
    Ok((
        crate::mesh::MeshView {
            frame_verts: m.base.frame_verts,
            verts,
            anim_seqs,
            bounding_boxes: boxes,
            bounding_box: crate::mesh::FBoxRaw {
                min: bounds.min,
                max: bounds.max,
                is_valid: i32::from(bounds.is_valid != 0),
            },
            scale: m.base.scale,
            origin: m.base.origin,
            rot_origin: m.base.rot_origin,
        },
        unsafe { owner_view(actor) },
    ))
}

/// `UMesh::GetBoneCoords`/`USkeletalMesh::GetBoneCoords` for canonical physics.
///
/// # Safety
/// `mesh` and `actor` are live canonical records; a skeletal mesh's cache and
/// backing arrays remain live for the synchronous query.
pub(crate) unsafe fn bone_coords(
    access: CoreAccess<'_>,
    mesh: *mut c_void,
    actor: *mut AActor,
    bone: i32,
) -> RuntimeResult<FCoords> {
    if mesh.is_null() || actor.is_null() {
        return Err(RuntimeError::new("Canonical GetBoneCoords requires a live mesh and actor"));
    }
    if mesh_is(access, mesh, "Engine.SkeletalMesh")? {
        return unsafe {
            render_actor_lod_provider::skeletal_bone_coords(
                access,
                mesh.cast::<generated::USkeletalMesh>(),
                actor,
                bone,
            )
        };
    }
    let (view, owner) = unsafe { ordinary_views(mesh.cast::<generated::UMesh>(), actor)? };
    let trig = crate::coords::Trig {
        sin: &|angle| hp2_core::math::sin_tab_inline(angle, hp2_core::math::trig_table()),
        cos: &|angle| hp2_core::math::cos_tab_inline(angle, hp2_core::math::trig_table()),
    };
    Ok(crate::mesh::bone_coords(&view, &owner, &trig))
}

fn sync_trig() -> (impl Fn(i32) -> f32, impl Fn(i32) -> f32) {
    let table = hp2_core::math::trig_table();
    (
        move |angle| hp2_core::math::sin_tab_inline(angle, table),
        move |angle| hp2_core::math::cos_tab_inline(angle, table),
    )
}

unsafe fn actor_mesh(actor: *mut AActor) -> RuntimeResult<*mut c_void> {
    if actor.is_null() {
        return Err(RuntimeError::new("Particle mesh operation requires an actor"));
    }
    let mesh = unsafe { ptr::addr_of!((*actor).mesh).read_unaligned() };
    if mesh.is_null() {
        return Err(RuntimeError::new("Particle mesh operation requires an actor mesh"));
    }
    Ok(mesh)
}

fn mesh_is(access: CoreAccess<'_>, mesh: *mut c_void, class: &str) -> RuntimeResult<bool> {
    let class = access.native_class(class)?;
    Ok(unsafe { hp2_core::object_model::is_a(mesh.cast(), class.cast()) })
}

fn mesh_is_lod(access: CoreAccess<'_>, mesh: *mut c_void) -> RuntimeResult<bool> {
    Ok(mesh_is(access, mesh, "Engine.LodMesh")? || mesh_is(access, mesh, "Engine.SkeletalMesh")?)
}

/// `Owner->Mesh->GetRenderBoundingBox(Owner, 0)` for the canonical particle system.
///
/// # Safety
/// `actor` must be a live actor with a live mesh.
pub(crate) unsafe fn particle_mesh_bounds(
    access: CoreAccess<'_>,
    actor: *mut AActor,
) -> RuntimeResult<FCoords> {
    let mesh = unsafe { actor_mesh(actor)? };
    if mesh_is(access, mesh, "Engine.SkeletalMesh")? {
        return unsafe { render_actor_lod_provider::skeletal_bounds(access, mesh.cast(), actor) };
    }
    let (sin, cos) = sync_trig();
    let trig = crate::coords::Trig { sin: &sin, cos: &cos };
    if mesh_is_lod(access, mesh)? {
        let (view, owner) = unsafe { lod_views(mesh.cast::<generated::ULodMesh>(), actor)? };
        Ok(crate::mesh::render_bounding_box(&view, &owner, &trig))
    } else {
        let (view, owner) = unsafe { ordinary_views(mesh.cast::<generated::UMesh>(), actor)? };
        Ok(crate::mesh::render_bounding_box(&view, &owner, &trig))
    }
}

/// `Owner->Mesh->GetNumTris()` for the canonical particle system.
///
/// # Safety
/// `actor` must be a live actor with a live mesh.
pub(crate) unsafe fn particle_mesh_triangles(
    access: CoreAccess<'_>,
    actor: *mut AActor,
) -> RuntimeResult<i32> {
    let mesh = unsafe { actor_mesh(actor)? };
    if mesh_is_lod(access, mesh)? {
        let faces = unsafe { (*mesh.cast::<generated::ULodMesh>()).faces };
        unsafe { raw_slice::<generated::FMeshFace>(faces.data, faces.num, "Invalid ULodMesh Faces")? };
        Ok(faces.num)
    } else {
        let tris = unsafe { (*mesh.cast::<generated::UMesh>()).tris };
        unsafe { raw_slice::<generated::FMeshTri>(tris.data, tris.array_num, "Invalid UMesh Tris")? };
        Ok(tris.array_num)
    }
}

/// `Owner->Mesh->GetTriVerts(T, Verts)` for the canonical particle system.
///
/// # Safety
/// `actor` must be a live actor with a live mesh.
pub(crate) unsafe fn particle_mesh_triangle(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    index: i32,
) -> RuntimeResult<[i32; 3]> {
    let mesh = unsafe { actor_mesh(actor)? };
    if mesh_is_lod(access, mesh)? {
        let lod = unsafe { &*mesh.cast::<generated::ULodMesh>() };
        let faces = unsafe { raw_slice::<generated::FMeshFace>(lod.faces.data, lod.faces.num, "Invalid ULodMesh Faces")? };
        let wedges = unsafe { raw_slice::<generated::FMeshWedge>(lod.wedges.data, lod.wedges.num, "Invalid ULodMesh Wedges")? };
        let face = faces.get(index.max(0) as usize).filter(|_| index >= 0)
            .ok_or_else(|| RuntimeError::new("Particle mesh face index is out of range"))?;
        let mut result = [0i32; 3];
        for (out, wedge) in result.iter_mut().zip(face.i_wedge) {
            let wedge = wedges.get(wedge as usize)
                .ok_or_else(|| RuntimeError::new("Particle mesh wedge index is out of range"))?;
            *out = i32::from(wedge.i_vertex);
        }
        Ok(result)
    } else {
        let tris = unsafe { (*mesh.cast::<generated::UMesh>()).tris };
        let tris = unsafe { raw_slice::<generated::FMeshTri>(tris.data, tris.array_num, "Invalid UMesh Tris")? };
        let tri = tris.get(index.max(0) as usize).filter(|_| index >= 0)
            .ok_or_else(|| RuntimeError::new("Particle mesh triangle index is out of range"))?;
        Ok([i32::from(tri.i_vertex[0]), i32::from(tri.i_vertex[1]), i32::from(tri.i_vertex[2])])
    }
}

/// `Owner->Mesh->GetFrame(UnitCoords, Owner)` for the canonical particle system.
///
/// # Safety
/// `actor` must be a live actor with a live mesh; `body` must not retain `points`.
pub(crate) unsafe fn particle_mesh_frame<T>(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    body: impl FnOnce(&[FVector]) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    let mesh = unsafe { actor_mesh(actor)? };
    let skeletal = mesh_is(access, mesh, "Engine.SkeletalMesh")?;
    let lod = !skeletal && mesh_is(access, mesh, "Engine.LodMesh")?;
    let count = if skeletal {
        unsafe { (*mesh.cast::<generated::USkeletalMesh>()).frame_verts }
    } else {
        unsafe { (*mesh.cast::<generated::UMesh>()).frame_verts }
    };
    if count < 0 {
        return Err(RuntimeError::new("Particle mesh FrameVerts is negative"));
    }
    let mut points = vec![FVector { x: 0.0, y: 0.0, z: 0.0 }; count as usize];
    let coords = hp2_core::math::unit_coords();
    if skeletal {
        unsafe { render_actor_lod_provider::skeletal_frame(access, mesh.cast(), actor, coords, &mut points)? };
    } else {
        let (view, owner) = if lod {
            unsafe { lod_views(mesh.cast::<generated::ULodMesh>(), actor)? }
        } else {
            unsafe { ordinary_views(mesh.cast::<generated::UMesh>(), actor)? }
        };
        let (sin, cos) = sync_trig();
        let trig = crate::coords::Trig { sin: &sin, cos: &cos };
        let engine = access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("Particle mesh frame has no Engine owner"))?;
        let mut cache = engine.cache.try_borrow_mut()
            .map_err(|_| RuntimeError::new("Canonical mesh cache is already borrowed"))?;
        let cache = cache.as_mut()
            .ok_or_else(|| RuntimeError::new("Canonical mesh cache is unavailable"))?;
        let index = unsafe { (*actor).index };
        if lod {
            let mut frame_cache = render_actor_lod_provider::CoreFrameCache::new(access, cache);
            let view = crate::lodmesh::LodView {
                base: view,
                special_verts: unsafe { (*mesh.cast::<generated::ULodMesh>()).special_verts },
                lod_request: count,
            };
            let key = crate::mesh::CacheKey {
                id: hp2_core::cache_id::object(0x30, index as u32, 0),
                mesh: mesh as usize as u64,
            };
            let result = crate::lodmesh::get_frame(&view, &owner, coords, &trig, &mut frame_cache, key, &mut |i, p| {
                if let Some(slot) = points.get_mut(i) { *slot = p; }
            });
            if let Some(error) = frame_cache.take_error() { return Err(error); }
            if result < 0 { return Err(RuntimeError::new("ULodMesh::GetFrame failed")); }
        } else {
            let mut frame_cache = render_actor_mesh_provider::CoreFrameCache::new(access, cache);
            let key = crate::mesh::CacheKey {
                id: hp2_core::cache_id::object(0x1d, index as u32, 0),
                mesh: mesh as usize as u64,
            };
            crate::mesh::get_frame(&view, &owner, coords, &trig, &mut frame_cache, key, &mut |i, p| {
                if let Some(slot) = points.get_mut(i) { *slot = p; }
            });
            if let Some(error) = frame_cache.take_error() { return Err(error); }
        }
    }
    body(&points)
}

const REN_ORTH_XY: i32 = 13;
const REN_ORTH_YZ: i32 = 15;

struct CoreSoftlnOps<'a> {
    access: CoreAccess<'a>,
    device: *mut URenderDevice,
    frame: *mut FSceneNode,
    color: FPlane,
}

impl<'a> CoreSoftlnOps<'a> {
    unsafe fn new(
        access: CoreAccess<'a>,
        render: *mut super::super::URender,
        frame: *mut FSceneNode,
        color: FPlane,
    ) -> RuntimeResult<Self> {
        if render.is_null() || frame.is_null() {
            return Err(RuntimeError::new("Canonical actor geometry requires a live renderer and frame"));
        }
        let graph = super::super::state(access)?;
        if graph.render()?.as_ptr() != render {
            return Err(RuntimeError::new("Canonical actor geometry does not own the active Render"));
        }
        let viewport = unsafe { ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>() };
        if viewport.is_null() {
            return Err(RuntimeError::new("Canonical actor geometry frame has no viewport"));
        }
        let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
        if device.is_null() {
            return Err(RuntimeError::new("Canonical actor geometry viewport has no render device"));
        }
        Ok(Self { access, device, frame, color })
    }

    unsafe fn line(&mut self, flags: u32, start: FVector, end: FVector) -> RuntimeResult<()> {
        unsafe {
            crate::render_device_owner::draw_3d_line(
                self.access, self.device, self.frame, self.color, flags, start, end,
            )
        }
    }
}

impl SoftlnDrawOps for CoreSoftlnOps<'_> {
    type Error = RuntimeError;

    fn rend_map(&mut self) -> RuntimeResult<i32> {
        unsafe {
            let viewport = ptr::addr_of!((*self.frame).viewport).read_unaligned().cast::<UViewport>();
            if viewport.is_null() { return Err(RuntimeError::new("Canonical actor geometry frame has no viewport")); }
            let actor = ptr::addr_of!((*viewport).actor).read_unaligned().cast::<hp2_abi::engine::generated::APlayerPawn>();
            if actor.is_null() { return Err(RuntimeError::new("Canonical actor geometry viewport has no actor")); }
            Ok(ptr::addr_of!((*actor).rend_map).read_unaligned())
        }
    }

    fn zoom(&mut self) -> RuntimeResult<f32> {
        Ok(unsafe { ptr::addr_of!((*self.frame).zoom).read_unaligned() })
    }

    fn fx2(&mut self) -> RuntimeResult<f32> {
        Ok(unsafe { ptr::addr_of!((*self.frame).fx2).read_unaligned() })
    }

    fn fy2(&mut self) -> RuntimeResult<f32> {
        Ok(unsafe { ptr::addr_of!((*self.frame).fy2).read_unaligned() })
    }

    fn proj_z(&mut self) -> RuntimeResult<f32> {
        Ok(unsafe { ptr::addr_of!((*self.frame).proj.z).read_unaligned() })
    }

    fn coords(&mut self) -> RuntimeResult<FCoords> {
        Ok(unsafe { ptr::addr_of!((*self.frame).coords).read_unaligned() })
    }

    fn is_ortho(&mut self) -> RuntimeResult<bool> {
        let mode = self.rend_map()?;
        Ok((REN_ORTH_XY..=REN_ORTH_YZ).contains(&mode))
    }

    fn ortho_zoom(&mut self) -> RuntimeResult<f32> {
        unsafe {
            let viewport = ptr::addr_of!((*self.frame).viewport).read_unaligned().cast::<UViewport>();
            if viewport.is_null() { return Err(RuntimeError::new("Canonical actor geometry frame has no viewport")); }
            let actor = ptr::addr_of!((*viewport).actor).read_unaligned().cast::<hp2_abi::engine::generated::APlayerPawn>();
            if actor.is_null() { return Err(RuntimeError::new("Canonical actor geometry viewport has no actor")); }
            Ok(ptr::addr_of!((*actor).base.ortho_zoom).read_unaligned())
        }
    }

    fn unit_coords(&mut self) -> RuntimeResult<FCoords> {
        Ok(hp2_core::math::unit_coords())
    }

    fn trig_table(&mut self) -> RuntimeResult<&'static [f32; hp2_core::math::NUM_ANGLES]> {
        Ok(hp2_core::math::trig_table())
    }

    fn render_line(&mut self, start: FVector, end: FVector, flags: u32) -> RuntimeResult<()> {
        unsafe { self.line(flags, start, end) }
    }

    fn device_line(&mut self, start: FVector, end: FVector, flags: u32) -> RuntimeResult<()> {
        unsafe { self.line(flags, start, end) }
    }
}

/// Submits the actor sprite's depth-cued line through the canonical device leaf.
///
/// # Safety
/// The active renderer, frame, and actor-sprite point pointers must be live.
pub(super) unsafe fn draw_line(
    access: CoreAccess<'_>,
    render: *mut super::super::URender,
    frame: *mut FSceneNode,
    color: FVector,
    flags: u32,
    start: FVector,
    end: FVector,
) -> RuntimeResult<()> {
    let color = FPlane { x: color.x, y: color.y, z: color.z, w: 0.0 };
    let mut ops = unsafe { CoreSoftlnOps::new(access, render, frame, color)? };
    unsafe { ops.line(flags, start, end) }
}

/// Draws the source circular actor bounds using the shared software geometry policy.
///
/// # Safety
/// The active renderer and frame must be live in the canonical render graph.
pub(super) unsafe fn draw_circle(
    access: CoreAccess<'_>,
    render: *mut super::super::URender,
    frame: *mut FSceneNode,
    color: FPlane,
    center: FVector,
    radius: f32,
    scale: bool,
) -> RuntimeResult<()> {
    let mut ops = unsafe { CoreSoftlnOps::new(access, render, frame, color)? };
    hp2_render::softln_draw::draw_circle(&mut ops, center, radius, 0, scale)
}

/// Draws the source oriented unit selection box with the shared geometry policy.
///
/// # Safety
/// The active renderer and frame must be live in the canonical render graph.
pub(super) unsafe fn draw_box(
    access: CoreAccess<'_>,
    render: *mut super::super::URender,
    frame: *mut FSceneNode,
    color: FPlane,
    flags: u32,
    coords: FCoords,
) -> RuntimeResult<()> {
    let mut ops = unsafe { CoreSoftlnOps::new(access, render, frame, color)? };
    let bounds = hp2_core::math::FBox {
        min: FVector { x: 0.0, y: 0.0, z: 0.0 },
        max: FVector { x: 1.0, y: 1.0, z: 1.0 },
        is_valid: 1,
    };
    hp2_render::softln_draw::draw_box_oriented(&mut ops, bounds, coords, flags)
}
/// Draws source collision-cylinder wire geometry through the shared software policy.
///
/// # Safety
/// The active renderer and frame must be live in the canonical render graph.
pub(super) unsafe fn draw_cylinder(
    access: CoreAccess<'_>,
    render: *mut super::super::URender,
    frame: *mut FSceneNode,
    color: FPlane,
    coords: FCoords,
    radius: f32,
    height: f32,
    width: f32,
) -> RuntimeResult<()> {
    let mut ops = unsafe { CoreSoftlnOps::new(access, render, frame, color)? };
    hp2_render::softln_draw::draw_cylinder(&mut ops, coords, radius, height, width, 2)
}

/// Draws a source-aligned or oriented primitive collision boundary.
///
/// # Safety
/// `actor` and its collision primitive must be live Core records.
pub(super) unsafe fn draw_collision_bounds(
    access: CoreAccess<'_>,
    render: *mut super::super::URender,
    frame: *mut FSceneNode,
    actor: *mut AActor,
) -> RuntimeResult<()> {
    if actor.is_null() {
        return Err(RuntimeError::new("Collision bounds require an actor"));
    }
    let primitive = unsafe { crate::actor_owner::get_primitive(actor)? };
    if primitive.is_null() {
        return Err(RuntimeError::new("Collision bounds actor has no primitive"));
    }
    let color = FPlane { x: 0.0, y: 1.0, z: 1.0, w: 0.0 };
    let class_is = |name: &str| -> RuntimeResult<bool> {
        let class = access.native_class(name)?;
        Ok(unsafe { hp2_core::object_model::is_a(primitive.cast(), class.cast()) })
    };
    if unsafe { class_is("Engine.Box")? } {
        let bounds = unsafe { crate::collision_owner::actor_collision_box_local(access, actor)? };
        let bounds = hp2_core::math::FBox {
            min: bounds.min,
            max: bounds.max,
            is_valid: bounds.is_valid,
        };
        let mut ops = unsafe { CoreSoftlnOps::new(access, render, frame, color)? };
        return hp2_render::softln_draw::draw_box_oriented(
            &mut ops,
            bounds,
            unsafe { crate::actor::to_local(actor) },
            2,
        );
    }
    let oriented = unsafe {
        class_is("Engine.OrientedCylinder")?
            || class_is("Engine.OrientedOvalCylinder")?
    };
    let oval = unsafe {
        class_is("Engine.AlignedOvalCylinder")?
            || class_is("Engine.OrientedOvalCylinder")?
    };
    let place = if oriented {
        unsafe { crate::actor::to_local(actor) }
    } else {
        let location = unsafe { ptr::addr_of!((*actor).location).read_unaligned() };
        let center = if oval {
            location
        } else {
            let width = unsafe { ptr::addr_of!((*actor).collision_width).read_unaligned() };
            hp2_core::math::vector_add(location, FVector { x: 0.0, y: 0.0, z: width })
        };
        crate::coords::coords_from_origin(center)
    };
    let radius = unsafe { ptr::addr_of!((*actor).collision_radius).read_unaligned() };
    let height = unsafe { ptr::addr_of!((*actor).collision_height).read_unaligned() };
    let width = if oval {
        unsafe { ptr::addr_of!((*actor).collision_width).read_unaligned() }
    } else {
        -1.0
    };
    unsafe { draw_cylinder(access, render, frame, color, place, radius, height, width) }
}

/// Dispatches an actor mesh to its exact ordinary, LOD, or skeletal source renderer.
/// # Safety
/// All pointers must be live canonical actor/render records from the active scene.
pub(super) unsafe fn draw_actor_mesh(
    access: CoreAccess<'_>,
    render: *mut super::super::URender,
    frame: *mut FSceneNode,
    sprite: *mut hp2_abi::render_scene::FDynamicSprite,
    actor: *mut AActor,
    coords: *const FCoords,
    flags: u32,
) -> RuntimeResult<()> {
    if actor.is_null() || sprite.is_null() || coords.is_null() {
        return Err(RuntimeError::new("Invalid canonical mesh draw arguments"));
    }
    let mesh = unsafe { ptr::addr_of!((*actor).mesh).read_unaligned() };
    if mesh.is_null() {
        return Err(RuntimeError::new("Canonical mesh draw requires an actor mesh"));
    }
    let skeletal_class = access.native_class("Engine.SkeletalMesh")?;
    if unsafe { hp2_core::object_model::is_a(mesh.cast(), skeletal_class.cast()) } {
        return unsafe {
            render_actor_lod_provider::draw_skeletal_mesh(
                access, render, frame, sprite, actor, coords, flags,
            )
        };
    }
    let lod_class = access.native_class("Engine.LodMesh")?;
    if unsafe { hp2_core::object_model::is_a(mesh.cast(), lod_class.cast()) } {
        unsafe {
            render_actor_lod_provider::draw_lod_mesh(
                access, render, frame, sprite, actor, coords, flags,
            )
        }
    } else {
        unsafe {
            render_actor_mesh_provider::draw_ordinary_mesh(
                access, render, frame, sprite, actor, coords, flags,
            )
        }
    }
}
