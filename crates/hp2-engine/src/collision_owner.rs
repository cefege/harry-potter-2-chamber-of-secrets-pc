//! Canonical collision hash operations for objects owned by Core.

use crate::{actcol::{ActColOps, CollisionHash}, actor, actor_owner, coords, mesh, model, model_archive::Hp2ModelArchiveField, prim::{self, Aabb}, skeletal, trace};
use hp2_abi::render_scene::{FBspNode, FBspSurf, FVert, FZoneProperties};
use hp2_abi::engine::generated::{AActor, ABrush, FBox, FCheckResult, FRotator, UBoxPrim, ULevel, UMesh, UModel, UObject};
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::mem_owner::MemStackOwner;
use hp2_core::math;
use hp2_core::object_model;
use std::{alloc::Layout, cell::{RefCell, UnsafeCell}, ffi::{c_char, c_void}, ptr};

/// Owns hash links; collision results use the caller's scratch-memory lifetime.
///
/// Its raw handle is private to Rust canonical callers and is never an ABI-compatible
/// `FCollisionHashBase*`; shipping native objects must not consume it.
pub(crate) struct CollisionOwner {
    hash: Box<UnsafeCell<CollisionHash>>,
    classes: PrimitiveClasses,
}


struct PrimitiveClasses {
    brush: *const c_void,
    model: *const c_void,
    mesh: *const c_void,
    skeletal: *const c_void,
    box_prim: *const c_void,
    box_class: *const c_void,
    oriented_oval: *const c_void,
    aligned_oval: *const c_void,
    oriented: *const c_void,
    cylinder: *const c_void,
}

impl PrimitiveClasses {
    fn new(access: CoreAccess<'_>) -> RuntimeResult<Self> {
        let class = |name| access.native_class(name).map(|value| value.cast::<c_void>() as *const c_void);
        Ok(Self {
            brush: class("Engine.Brush")?,
            model: class("Engine.Model")?,
            mesh: class("Engine.Mesh")?,
            skeletal: class("Engine.SkeletalMesh")?,
            box_prim: class("Engine.BoxPrim")?,
            box_class: class("Engine.Box")?,
            oriented_oval: class("Engine.OrientedOvalCylinder")?,
            aligned_oval: class("Engine.AlignedOvalCylinder")?,
            oriented: class("Engine.OrientedCylinder")?,
            cylinder: class("Engine.Primitive")?,
        })
    }
}
/// Computes a canonical Actor's actual collision-cylinder extent.
///
/// # Safety
/// `actor` must be live and its mesh or brush geometry must belong to `access`.
pub unsafe fn actor_cylinder_extent(
    access: CoreAccess<'_>,
    actor: *mut AActor,
) -> RuntimeResult<FVector> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in cylinder extent"));
    }
    let classes = PrimitiveClasses::new(access)?;
    let ops = CanonicalOps::for_access(access, &classes);
    unsafe { ops.cylinder_extent(actor, (*actor).collision_width) }
}

/// Computes the canonical collision bounds for a live Actor.
///
/// # Safety
/// `actor` and its primitive geometry must be live objects owned by `access`.
pub unsafe fn actor_collision_box(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<FBox> {
    unsafe { actor_collision_box_visual(access, actor, false) }
}

/// Computes local collision bounds for drawing a primitive in actor coordinates.
///
/// # Safety
/// `actor` and its primitive geometry must be live objects owned by `access`.
pub unsafe fn actor_collision_box_local(
    access: CoreAccess<'_>,
    actor: *mut AActor,
) -> RuntimeResult<FBox> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in local collision bounds"));
    }
    let classes = PrimitiveClasses::new(access)?;
    let ops = CanonicalOps::for_access(access, &classes);
    let primitive = unsafe { actor_owner::get_primitive(actor)? };
    if primitive.is_null() {
        return Err(RuntimeError::new("Canonical Actor has no collision primitive"));
    }
    unsafe { ops.collision_box_for(actor, primitive, false) }
}

/// Computes world collision bounds, selecting the visual mesh/brush when requested.
///
/// # Safety
/// `actor` and its selected primitive geometry must be live objects owned by `access`.
pub unsafe fn actor_collision_box_visual(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    visual: bool,
) -> RuntimeResult<FBox> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in collision bounds"));
    }
    let classes = PrimitiveClasses::new(access)?;
    let ops = CanonicalOps::for_access(access, &classes);
    let primitive = unsafe {
        if visual && !(*actor).mesh.is_null() {
            (*actor).mesh.cast::<UObject>()
        } else if visual && !(*actor).brush.is_null() {
            (*actor).brush.cast::<UObject>()
        } else {
            actor_owner::get_primitive(actor)?
        }
    };
    if primitive.is_null() {
        return Err(RuntimeError::new("Canonical Actor has no collision primitive"));
    }
    unsafe { ops.collision_box_for(actor, primitive, true) }
}

/// Computes the canonical render extent (local primitive bounds, except skeletal mesh bounds).
///
/// # Safety
/// `actor` and selected primitive geometry are live objects owned by `access`.
pub(crate) unsafe fn actor_render_extent(
    access: CoreAccess<'_>,
    actor: *mut AActor,
) -> RuntimeResult<FVector> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical Actor in render extent"));
    }
    let classes = PrimitiveClasses::new(access)?;
    let skeletal_class = classes.skeletal;
    let ops = CanonicalOps::for_access(access, &classes);
    let mesh = unsafe { (*actor).mesh.cast::<UObject>() };
    if !mesh.is_null() && unsafe { object_model::is_a(mesh.cast(), skeletal_class) } {
        let bounds = unsafe { (*mesh.cast::<hp2_abi::engine::generated::USkeletalMesh>()).bounding_box_sum };
        return Ok(FVector {
            x: bounds.max.x - bounds.min.x,
            y: bounds.max.y - bounds.min.y,
            z: bounds.max.z - bounds.min.z,
        });
    }
    let primitive = unsafe {
        if !mesh.is_null() {
            mesh
        } else if !(*actor).brush.is_null() {
            (*actor).brush.cast::<UObject>()
        } else {
            actor_owner::get_primitive(actor)?
        }
    };
    if primitive.is_null() || unsafe { (*primitive).class.is_null() } {
        return Err(RuntimeError::new("Render extent primitive has no canonical class"));
    }
    let bounds = unsafe { ops.collision_box_for(actor, primitive, false)? };
    Ok(FVector {
        x: bounds.max.x - bounds.min.x,
        y: bounds.max.y - bounds.min.y,
        z: bounds.max.z - bounds.min.z,
    })
}


/// Traces the canonical world's BSP model without actor-hash candidates.
///
/// # Safety
/// `model` and all arrays reached by its geometry view remain live.
pub(crate) unsafe fn world_line_check(
    _access: CoreAccess<'_>,
    model: *mut UModel,
    hit: *mut FCheckResult,
    end: FVector,
    start: FVector,
    extent: FVector,
    flags: u32,
) -> RuntimeResult<bool> {
    if model.is_null() || hit.is_null() {
        return Err(RuntimeError::new("Invalid canonical world line-check arguments"));
    }
    let view = unsafe { CanonicalOps::model_view(model)? };
    Ok(crate::trace::model_line_check(
        unsafe { &mut *hit },
        &view,
        None,
        end,
        start,
        extent,
        flags,
    ))
}

/// Runs the canonical `UModel::FastLineCheck` used by `AActor.FastTrace`.
///
/// # Safety
/// `model` and all arrays reached by its geometry view must remain live.
pub(crate) unsafe fn model_fast_line_check(
    model: *mut UModel,
    end: FVector,
    start: FVector,
) -> RuntimeResult<bool> {
    let view = unsafe { CanonicalOps::model_view(model)? };
    Ok(crate::trace::fast_line_check(&view, end, start))
}

/// Checks the canonical world's BSP model at a point without actor-hash candidates.
///
/// # Safety
/// `model` and all arrays reached by its geometry view remain live.
pub unsafe fn world_point_check(
    _access: CoreAccess<'_>,
    model: *mut UModel,
    hit: *mut FCheckResult,
    location: FVector,
    extent: FVector,
    extra_flags: u32,
) -> RuntimeResult<bool> {
    if model.is_null() || hit.is_null() {
        return Err(RuntimeError::new("Invalid canonical world point-check arguments"));
    }
    let view = unsafe { CanonicalOps::model_view(model)? };
    Ok(crate::trace::model_point_check(
        unsafe { &mut *hit },
        &view,
        None,
        location,
        extent,
        extra_flags,
    ))
}

struct CanonicalActorCollisionOps<'a>(CoreAccess<'a>);

impl actor::ActorCollisionOps for CanonicalActorCollisionOps<'_> {
    type Error = RuntimeError;

    unsafe fn has_hash(&mut self, level: *mut c_void) -> RuntimeResult<bool> {
        if level.is_null() {
            return Err(RuntimeError::new("Canonical Actor has no Level during collision update"));
        }
        Ok(!unsafe { (*level.cast::<ULevel>()).hash }.is_null())
    }

    unsafe fn hash_remove(&mut self, level: *mut c_void, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_remove(self.0, level.cast(), actor, false) }
    }

    unsafe fn hash_add(&mut self, level: *mut c_void, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_add(self.0, level.cast(), actor) }
    }
}

/// Applies canonical `AActor::SetCollision` and maintains the Level hash.
///
/// # Safety
/// `actor` and its Level are live records owned by `access`.
pub unsafe fn actor_set_collision(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    collide_actors: bool,
    block_actors: bool,
    block_players: bool,
) -> RuntimeResult<()> {
    let mut ops = CanonicalActorCollisionOps(access);
    unsafe {
        actor::set_collision_with_ops(
            &mut ops,
            actor,
            collide_actors,
            block_actors,
            block_players,
        )
    }
}

/// Applies canonical `AActor::SetCollisionSize` and maintains the Level hash.
///
/// # Safety
/// `actor` and its Level are live records owned by `access`.
pub unsafe fn actor_set_collision_size(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    radius: f32,
    height: f32,
) -> RuntimeResult<()> {
    let mut ops = CanonicalActorCollisionOps(access);
    unsafe { actor::set_collision_size_with_ops(&mut ops, actor, radius, height) }
}


/// Builds the canonical borrowed BSP view used by physics and actor-tick queries.
///
/// # Safety
/// `model` and every backing array remain live for `'model`.
pub(crate) unsafe fn model_view<'model>(
    model: *mut UModel,
) -> RuntimeResult<trace::ModelView<'model>> {
    unsafe { CanonicalOps::model_view(model) }
}
impl CollisionOwner {
    /// Creates an empty canonical hash and initializes the shared engine grid policy.
    pub(crate) fn new(access: CoreAccess<'_>) -> RuntimeResult<Self> {
        let classes = PrimitiveClasses::new(access)?;
        let ops = CanonicalOps::for_access(access, &classes);
        let hash = CollisionHash::new(&ops)
            .map_err(|code| ops.take_error("initializing collision hash", code))?;
        drop(ops);
        Ok(Self { hash: Box::new(UnsafeCell::new(hash)), classes })
    }

    /// Resolves a point's BSP leaf into its canonical model zone actor.
    ///
    /// # Safety
    /// `model` and the canonical objects in its zones table remain live.
    pub(crate) unsafe fn model_point_region(
        model: *mut UModel,
        fallback_zone: *mut c_void,
        location: FVector,
    ) -> RuntimeResult<hp2_abi::engine::generated::FPointRegion> {
        unsafe { CanonicalOps::model_point_region(model, fallback_zone, location) }
    }


    /// Clears per-tick counters without altering shared tables or actor links.
    pub(crate) fn tick(&self) {
        unsafe { (&mut *self.hash.get()).tick(); }
    }

    /// Inserts an actor using its real canonical Actor fields and selected primitive.
    ///
    /// # Safety
    /// `actor` must be a live canonical Actor owned by the same CoreOwner.
    pub(crate) unsafe fn add_actor(&self, access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<()> {
        let ops = CanonicalOps::for_access(access, &self.classes);
        // SAFETY: the caller guarantees canonical actor liveness; the hash retains only
        // its raw identity, and each callback ends before the next raw-link read.
        unsafe { CollisionHash::add_actor(self.hash.get(), &ops, actor) }
            .map_err(|code| ops.take_error("adding actor to collision hash", code))
    }

    /// Removes an actor from its old cells, optionally scanning all buckets.
    ///
    /// # Safety
    /// `actor` must be a live canonical Actor owned by the same CoreOwner.
    pub(crate) unsafe fn remove_actor(
        &self, access: CoreAccess<'_>, actor: *mut AActor, thorough: bool,
    ) -> RuntimeResult<()> {
        let ops = CanonicalOps::for_access(access, &self.classes);
        // SAFETY: see add_actor; `CollisionHash` deliberately uses its reentrant raw handle.
        unsafe { CollisionHash::remove_actor(self.hash.get(), &ops, actor, thorough) }
            .map_err(|code| ops.take_error("removing actor from collision hash", code))
    }

    /// Checks that thorough removal left no links to the actor.
    ///
    /// # Safety
    /// `actor` is live and the collision hash remains stable for this call.
    pub(crate) unsafe fn check_actor_not_referenced(&self, actor: *mut AActor) -> RuntimeResult<()> {
        if unsafe { CollisionHash::actor_is_referenced(self.hash.get(), actor) } {
            return Err(RuntimeError::new("Actor remains referenced by collision hash"));
        }
        Ok(())
    }

    /// Checks actors overlapping a point-shaped extent and allocates results in Engine memory.
    ///
    /// # Safety
    /// Hash links and all actors reached from them must remain live canonical objects.
    pub(crate) unsafe fn point_check(
        &self, access: CoreAccess<'_>, location: FVector, extent: FVector,
    ) -> RuntimeResult<*mut FCheckResult> {
        let ops = CanonicalOps::for_access(access, &self.classes);
        // SAFETY: arguments are copied values and the hash owner keeps link storage stable.
        unsafe { CollisionHash::actor_point_check(self.hash.get(), &ops, location, extent) }
            .map_err(|code| ops.take_error("checking collision hash point", code))
    }

    /// Returns actors whose locations lie strictly inside the requested radius.
    ///
    /// # Safety
    /// Hash links and all actors reached from them must remain live canonical objects.
    pub(crate) unsafe fn radius_check(
        &self, access: CoreAccess<'_>, location: FVector, radius: f32,
    ) -> RuntimeResult<*mut FCheckResult> {
        let ops = CanonicalOps::for_access(access, &self.classes);
        // SAFETY: arguments are copied values and the hash owner keeps link storage stable.
        unsafe { CollisionHash::actor_radius_check(self.hash.get(), &ops, location, radius) }
            .map_err(|code| ops.take_error("checking collision hash radius", code))
    }

    /// Checks a segment against registered actor primitives.
    ///
    /// # Safety
    /// Hash links and all actors reached from them must remain live canonical objects.
    pub(crate) unsafe fn line_check(
        &self, access: CoreAccess<'_>, memory: &MemStackOwner, end: FVector, start: FVector,
        extent: FVector, flags: u32,
    ) -> RuntimeResult<*mut FCheckResult> {
        let ops = CanonicalOps::new(access, memory, &self.classes);
        // SAFETY: arguments are copied values and results are allocated in this owner's memory.
        unsafe { CollisionHash::actor_line_check(self.hash.get(), &ops, ptr::null_mut(), end, start, extent, flags) }
            .map_err(|code| ops.take_error("checking collision hash line", code))
    }

    /// Tests an actor at an exchanged pose against currently hashed actors.
    ///
    /// # Safety
    /// `actor` and all hashed actors must be live canonical objects in the same owner.
    pub(crate) unsafe fn encroachment_check(
        &self, access: CoreAccess<'_>, actor: *mut AActor,
        location: FVector, rotation: FRotator,
    ) -> RuntimeResult<*mut FCheckResult> {
        let ops = CanonicalOps::for_access(access, &self.classes);
        // SAFETY: the caller guarantees canonical actor lifetime and the shared policy
        // preserves the exact native exchanged-pose and reentrant link traversal order.
        unsafe { CollisionHash::actor_encroachment_check(self.hash.get(), &ops, actor, location, rotation) }
            .map_err(|code| ops.take_error("checking actor encroachment", code))
    }

}


struct CanonicalOps<'a> {
    access: CoreAccess<'a>,
    memory: &'a MemStackOwner,
    classes: &'a PrimitiveClasses,
    error: RefCell<Option<RuntimeError>>,
}

impl<'a> CanonicalOps<'a> {
    fn new(access: CoreAccess<'a>, memory: &'a MemStackOwner, classes: &'a PrimitiveClasses) -> Self {
        Self { access, memory, classes, error: RefCell::new(None) }
    }

    fn for_access(access: CoreAccess<'a>, classes: &'a PrimitiveClasses) -> Self {
        Self::new(access, access.scratch_memory(), classes)
    }

    fn record<T>(&self, result: RuntimeResult<T>) -> Result<T, i32> {
        result.map_err(|error| {
            let mut first = self.error.borrow_mut();
            if first.is_none() { *first = Some(error); }
            1
        })
    }

    fn take_error(&self, operation: &str, code: i32) -> RuntimeError {
        self.error.borrow_mut().take().unwrap_or_else(|| {
            RuntimeError::new("Canonical collision operation failed")
                .with_context(&format!("{operation} (status {code})"))
        })
    }

    unsafe fn actor(&self, object: *mut c_void) -> RuntimeResult<*mut AActor> {
        if object.is_null() { return Err(RuntimeError::new("Null actor in collision query")); }
        Ok(object.cast())
    }

    unsafe fn primitive(&self, actor: *mut AActor) -> RuntimeResult<*mut UObject> {
        unsafe { actor_owner::get_primitive(actor) }
    }

    unsafe fn owner(&self, actor: *mut AActor) -> RuntimeResult<prim::Owner> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor in primitive collision")); }
        let a = unsafe { &*actor };
        let (to_world, to_local) = if unsafe { object_model::is_a(actor.cast(), self.classes.brush) } {
            unsafe {
                (crate::brush_owner::to_world(actor.cast::<ABrush>()),
                 crate::brush_owner::to_local(actor.cast::<ABrush>()))
            }
        } else {
            let table = math::trig_table();
            let sin = |angle: i32| math::sin_tab_inline(angle, table);
            let cos = |angle: i32| math::cos_tab_inline(angle, table);
            let trig = coords::Trig { sin: &sin, cos: &cos };
            let mut to_world = coords::unit_coords();
            coords::mul_vec(&mut to_world, a.location);
            coords::mul_rot(&mut to_world, a.rotation, &trig);
            let mut to_local = coords::unit_coords();
            coords::div_rot(&mut to_local, a.rotation, &trig);
            coords::div_vec(&mut to_local, a.location);
            (to_world, to_local)
        };
        let radius = a.collision_radius;
        let width = a.collision_width;
        let height = a.collision_height;
        let cyl_extent = unsafe { self.cylinder_extent(actor, width)? };
        let cyl_extent_zero_width = if a.collide_type == actor::CT_BOX && width != 0.0 {
            unsafe { self.cylinder_extent(actor, 0.0)? }
        } else {
            cyl_extent
        };
        Ok(prim::Owner {
            actor: actor.cast(), location: a.location,
            collision_radius: radius, collision_width: width, collision_height: height,
            to_local, to_world,
            cyl_extent,
            cyl_extent_zero_width,
        })
    }

    unsafe fn mesh_view(&self, primitive: *mut UObject) -> RuntimeResult<mesh::CollisionView> {
        if primitive.is_null() { return Err(RuntimeError::new("Null mesh in collision query")); }
        let m = unsafe { &*primitive.cast::<UMesh>() };
        let b = m.base.base.base.bounding_box;
        Ok(mesh::CollisionView {
            bounding_box: mesh::FBoxRaw { min: b.min, max: b.max, is_valid: b.is_valid as i32 },
            scale: m.scale, origin: m.origin, rot_origin: m.rot_origin,
        })
    }

    fn mesh_bounds(
        &self, mesh: &mesh::CollisionView, actor: *mut AActor, world: bool, draw_scale: f32,
    ) -> Aabb {
        let a = unsafe { &*actor };
        let pose = mesh::ActorPose {
            location: a.location, pre_pivot: a.pre_pivot,
            rotation: a.rotation, draw_scale,
        };
        let table = math::trig_table();
        let sin = |angle: i32| math::sin_tab_inline(angle, table);
        let cos = |angle: i32| math::cos_tab_inline(angle, table);
        let trig = coords::Trig { sin: &sin, cos: &cos };
        mesh::collision_bounding_box(mesh, &pose, world, &trig)
    }
    fn mesh_extent(&self, mesh: &mesh::CollisionView, actor: *mut AActor) -> FVector {
        let a = unsafe { &*actor };
        let pose = mesh::ActorPose {
            location: a.location, pre_pivot: a.pre_pivot,
            rotation: a.rotation, draw_scale: a.draw_scale,
        };
        let table = math::trig_table();
        let sin = |angle: i32| math::sin_tab_inline(angle, table);
        let cos = |angle: i32| math::cos_tab_inline(angle, table);
        let trig = coords::Trig { sin: &sin, cos: &cos };
        mesh::collision_extent(mesh, &pose, &trig)
    }

    unsafe fn cylinder_extent(&self, actor: *mut AActor, width: f32) -> RuntimeResult<FVector> {
        let mut extent = std::mem::MaybeUninit::<FVector>::uninit();
        unsafe {
            actor::get_cylinder_extent_with::<RuntimeError>(
                actor, width, extent.as_mut_ptr(),
                |mesh, owner, out| {
                    let view = self.mesh_view(mesh.cast())?;
                    out.cast::<FVector>().write(self.mesh_extent(&view, owner.cast()));
                    Ok(())
                },
                |brush, _owner, out| {
                    let b = (*brush.cast::<UBoxPrim>()).base.base.bounding_box;
                    let bounds = Aabb { min: b.min, max: b.max, valid: b.is_valid != 0 };
                    out.cast::<FVector>().write(prim::boxprim_extent(&bounds));
                    Ok(())
                },
            )?;
            Ok(extent.assume_init())
        }
    }

    /// Builds a borrowed BSP view directly from a canonical UModel's array storage.
    ///
    /// # Safety
    /// The model and all backing arrays remain live for `'model`.
    pub(crate) unsafe fn model_view<'model>(
        model: *mut UModel,
    ) -> RuntimeResult<trace::ModelView<'model>> {
        unsafe fn slice<'b, T: 'b>(data: *mut c_void, count: i32, label: &str) -> RuntimeResult<&'b [T]> {
            if count < 0 || (count > 0 && data.is_null()) {
                return Err(RuntimeError::new("Invalid canonical model array").with_context(label));
            }
            if count == 0 { return Ok(&[]); }
            // SAFETY: the model's validated TArray storage remains live for the synchronous trace.
            Ok(unsafe { std::slice::from_raw_parts(data.cast::<T>(), count as usize) })
        }
        use Hp2ModelArchiveField::*;
        let get = |field| unsafe { model::model_array(model, field) }
            .map_err(|_| RuntimeError::new("Invalid canonical model geometry array"));
        let nodes = get(Nodes)?;
        let points = get(Points)?;
        let verts = get(Verts)?;
        let surfs = get(Surfs)?;
        let leaf_hulls = get(LeafHulls)?;
        let n = unsafe { nodes.slots.num.read() };
        let p = unsafe { points.slots.num.read() };
        let v = unsafe { verts.slots.num.read() };
        let s = unsafe { surfs.slots.num.read() };
        let l = unsafe { leaf_hulls.slots.num.read() };
        let bsp = prim::BspArrays {
            nodes: unsafe { slice::<FBspNode>(nodes.slots.data.read_unaligned(), n, "Nodes")? },
            points: unsafe { slice::<FVector>(points.slots.data.read_unaligned(), p, "Points")? },
            verts: unsafe { slice::<FVert>(verts.slots.data.read_unaligned(), v, "Verts")? },
            surfs: unsafe { slice::<FBspSurf>(surfs.slots.data.read_unaligned(), s, "Surfs")? },
        };
        let leaf_hulls = unsafe { slice::<i32>(leaf_hulls.slots.data.read_unaligned(), l, "LeafHulls")? };
        let bounds = unsafe { (*model).base.base.base.bounding_box };
        Ok(trace::ModelView {
            bsp, leaf_hulls,
            bounding_box: Aabb { min: bounds.min, max: bounds.max, valid: bounds.is_valid != 0 },
            root_outside: unsafe { (*model).root_outside != 0 },
            num_zones: unsafe { (*model).num_zones },
            primitive: model.cast(),
        })
    }
    /// Resolves a point's BSP leaf into its canonical model zone actor.
    ///
    /// # Safety
    /// `model` and the canonical objects in its zones table remain live.
    pub(crate) unsafe fn model_point_region(
        model: *mut UModel,
        fallback_zone: *mut c_void,
        location: FVector,
    ) -> RuntimeResult<hp2_abi::engine::generated::FPointRegion> {
        let view = unsafe { Self::model_view(model)? };
        let region = trace::point_region(&view, location);
        let zone = if region.zone_number == 0 {
            fallback_zone
        } else {
            let index = usize::from(region.zone_number);
            if index >= 64 {
                return Err(RuntimeError::new("Model point-region zone index is out of range"));
            }
            let pointer = unsafe {
                ptr::addr_of!((*model).zones)
                    .cast::<FZoneProperties>()
                    .add(index)
                    .read_unaligned()
                    .zone_actor
            };
            if pointer.is_null() { fallback_zone } else { pointer }
        };
        let mut result: hp2_abi::engine::generated::FPointRegion =
            unsafe { std::mem::zeroed() };
        result.zone = zone;
        result.i_leaf = region.i_leaf;
        result.zone_number = region.zone_number;
        Ok(result)
    }

    unsafe fn collision_box(&self, actor: *mut AActor) -> RuntimeResult<FBox> {
        let primitive = unsafe { self.primitive(actor)? };
        unsafe { self.collision_box_for(actor, primitive, true) }
    }

    unsafe fn collision_box_for(
        &self,
        actor: *mut AActor,
        primitive: *mut UObject,
        world: bool,
    ) -> RuntimeResult<FBox> {
        let owner = unsafe { self.owner(actor)? };
        let class = unsafe { (*primitive).class.cast::<UObject>() };
        if class.is_null() { return Err(RuntimeError::new("Primitive has no canonical class")); }
        let model_class = self.classes.model;
        let skeletal_class = self.classes.skeletal;
        let mesh_class = self.classes.mesh;
        let box_prim_class = self.classes.box_prim;
        let box_class = self.classes.box_class;
        let oriented_oval = self.classes.oriented_oval;
        let aligned_oval = self.classes.aligned_oval;
        let oriented = self.classes.oriented;
        let cylinder = self.classes.cylinder;
        let is_a = |c: *const c_void| unsafe { object_model::is_a(primitive.cast(), c) };
        let bounds = if is_a(model_class.cast()) {
            let b = unsafe { (*primitive.cast::<UBoxPrim>()).base.base.bounding_box };
            let bounds = Aabb { min: b.min, max: b.max, valid: b.is_valid != 0 };
            prim::boxprim_bounds(&bounds, &owner, world)
        } else if is_a(skeletal_class.cast()) {
            let table = math::trig_table();
            let sin = |angle: i32| math::sin_tab_inline(angle, table);
            let cos = |angle: i32| math::cos_tab_inline(angle, table);
            let trig = coords::Trig { sin: &sin, cos: &cos };
            unsafe { skeletal::get_collision_bounding_box_with_trig(primitive.cast(), actor, world, &trig) }
        } else if is_a(mesh_class.cast()) {
            let mesh = unsafe { self.mesh_view(primitive)? };
            self.mesh_bounds(&mesh, actor, world, unsafe { (*actor).draw_scale })
        } else if is_a(box_prim_class.cast()) {
            let bx = unsafe { (*primitive.cast::<UBoxPrim>()).base.base.bounding_box };
            let local = Aabb { min: bx.min, max: bx.max, valid: bx.is_valid != 0 };
            prim::boxprim_bounds(&local, &owner, world)
        } else if is_a(box_class.cast()) {
            prim::ubox_bounds(&owner, world)
        } else if is_a(oriented_oval.cast()) {
            prim::oriented_oval_bounds(&owner, world)
        } else if is_a(aligned_oval.cast()) {
            prim::aligned_oval_bounds(&owner, world)
        } else if is_a(oriented.cast()) {
            prim::oriented_cyl_bounds(&owner, world)
        } else if is_a(cylinder.cast()) {
            prim::cyl_bounds(&owner, world)
        } else {
            return Err(RuntimeError::new("Primitive has no canonical collision-bounds policy"));
        };
        let mut raw: FBox = unsafe { std::mem::zeroed() };
        raw.min = bounds.min;
        raw.max = bounds.max;
        raw.is_valid = u8::from(bounds.valid);
        Ok(raw)
    }

    unsafe fn point_primitive(
        &self, primitive: *mut UObject, actor: *mut AActor, result: *mut FCheckResult,
        location: FVector, extent: FVector,
    ) -> RuntimeResult<bool> {
        if primitive.is_null() || actor.is_null() || result.is_null() {
            return Err(RuntimeError::new("Invalid canonical primitive point check"));
        }
        let class = unsafe { (*primitive).class.cast::<UObject>() };
        if class.is_null() { return Err(RuntimeError::new("Primitive has no canonical class")); }
        let model_class = self.classes.model;
        let box_class = self.classes.box_class;
        let oriented_oval = self.classes.oriented_oval;
        let aligned_oval = self.classes.aligned_oval;
        let oriented = self.classes.oriented;
        let cylinder = self.classes.cylinder;
        let owner = unsafe { self.owner(actor)? };
        let hit = unsafe { &mut *result };
        let is_a = |c: *const c_void| unsafe { object_model::is_a(primitive.cast(), c) };
        if is_a(model_class.cast()) {
            let view = unsafe { Self::model_view(primitive.cast::<UModel>())? };
            Ok(trace::model_point_check(hit, &view, Some(owner), location, extent, 0))
        } else if is_a(box_class.cast()) {
            Ok(prim::ubox_point_check(hit, &owner, location, extent))
        } else if is_a(oriented_oval.cast()) {
            Ok(prim::oriented_oval_point_check(hit, &owner, location, extent))
        } else if is_a(aligned_oval.cast()) {
            Ok(prim::aligned_oval_point_check(hit, Some(&owner), location, extent))
        } else if is_a(oriented.cast()) {
            Ok(prim::oriented_cyl_point_check(hit, &owner, location, extent))
        } else if is_a(cylinder.cast()) {
            Ok(prim::cyl_point_check(hit, Some(&owner), location, extent))
        } else {
            Err(RuntimeError::new("Primitive has no canonical point-check policy"))
        }
    }

    unsafe fn line_primitive(
        &self, primitive: *mut UObject, actor: *mut AActor, result: *mut FCheckResult,
        end: FVector, start: FVector, extent: FVector, flags: u32,
    ) -> RuntimeResult<bool> {
        if primitive.is_null() || actor.is_null() || result.is_null() {
            return Err(RuntimeError::new("Invalid canonical primitive line check"));
        }
        let class = unsafe { (*primitive).class.cast::<UObject>() };
        if class.is_null() { return Err(RuntimeError::new("Primitive has no canonical class")); }
        let model_class = self.classes.model;
        let box_class = self.classes.box_class;
        let oriented_oval = self.classes.oriented_oval;
        let aligned_oval = self.classes.aligned_oval;
        let oriented = self.classes.oriented;
        let cylinder = self.classes.cylinder;
        let owner = unsafe { self.owner(actor)? };
        let hit = unsafe { &mut *result };
        let is_a = |c: *const c_void| unsafe { object_model::is_a(primitive.cast(), c) };
        if is_a(model_class.cast()) {
            let view = unsafe { Self::model_view(primitive.cast::<UModel>())? };
            Ok(trace::model_line_check(hit, &view, Some(owner), end, start, extent, flags))
        } else if is_a(box_class.cast()) {
            Ok(prim::ubox_line_check(hit, &owner, end, start, extent))
        } else if is_a(oriented_oval.cast()) {
            Ok(prim::oriented_oval_line_check(hit, &owner, end, start, extent))
        } else if is_a(aligned_oval.cast()) {
            Ok(prim::aligned_oval_line_check(hit, Some(&owner), end, start, extent))
        } else if is_a(oriented.cast()) {
            Ok(prim::oriented_cyl_line_check(hit, &owner, end, start, extent))
        } else if is_a(cylinder.cast()) {
            Ok(prim::cyl_line_check(hit, Some(&owner), end, start, extent))
        } else {
            Err(RuntimeError::new("Primitive has no canonical line-check policy"))
        }
    }
}

impl ActColOps for CanonicalOps<'_> {
    unsafe fn app_rand(&self, out: *mut i32) -> i32 {
        if out.is_null() { return self.record::<()>(Err(RuntimeError::new("Null collision RNG output"))).err().unwrap_or(0); }
        unsafe { out.write(self.access.rand()); }
        0
    }

    unsafe fn actor_collision_bounding_box(&self, actor: *mut c_void, out: *mut c_void) -> i32 {
        if out.is_null() { return self.record::<()>(Err(RuntimeError::new("Null collision bounds output"))).err().unwrap_or(0); }
        let result = unsafe { self.actor(actor).and_then(|a| self.collision_box(a)) };
        match self.record(result) {
            Ok(bounds) => { unsafe { out.cast::<FBox>().write(bounds); } 0 },
            Err(code) => code,
        }
    }

    unsafe fn actor_is_moving_brush(&self, actor: *const c_void, out: *mut i32) -> i32 {
        if actor.is_null() || out.is_null() {
            return self.record::<()>(Err(RuntimeError::new("Invalid moving-brush query"))).err().unwrap_or(0);
        }
        let result = unsafe {
            actor::is_moving_brush_with(actor.cast(), |value, class| {
                debug_assert_eq!(class, actor::class::ABRUSH);
                Ok(object_model::is_a(value, self.classes.brush))
            })
        };
        match result {
            Ok(moving) => { unsafe { out.write(i32::from(moving)); } 0 },
            Err(code) => code,
        }
    }

    unsafe fn actor_primitive(&self, actor: *mut c_void, out: *mut *mut c_void) -> i32 {
        if out.is_null() { return self.record::<()>(Err(RuntimeError::new("Null primitive output"))).err().unwrap_or(0); }
        match self.record(unsafe { self.actor(actor).and_then(|a| self.primitive(a)) }) {
            Ok(primitive) => { unsafe { out.write(primitive.cast()); } 0 },
            Err(code) => code,
        }
    }

    unsafe fn primitive_point_check(&self, primitive: *mut c_void, actor: *mut c_void,
        result: *mut c_void, location: *const c_void, size: *const c_void, no_hit: *mut i32) -> i32 {
        if location.is_null() || size.is_null() || no_hit.is_null() {
            return self.record::<()>(Err(RuntimeError::new("Invalid primitive point arguments"))).err().unwrap_or(0);
        }
        let answer = unsafe { self.point_primitive(primitive.cast(), actor.cast(), result.cast(),
            location.cast::<FVector>().read_unaligned(), size.cast::<FVector>().read_unaligned()) };
        match self.record(answer) {
            Ok(no_hit_value) => { unsafe { no_hit.write(i32::from(no_hit_value)); } 0 },
            Err(code) => code,
        }
    }

    unsafe fn primitive_line_check(&self, primitive: *mut c_void, actor: *mut c_void,
        result: *mut c_void, end: *const c_void, start: *const c_void, size: *const c_void,
        flags: u32, no_hit: *mut i32) -> i32 {
        if end.is_null() || start.is_null() || size.is_null() || no_hit.is_null() {
            return self.record::<()>(Err(RuntimeError::new("Invalid primitive line arguments"))).err().unwrap_or(0);
        }
        let answer = unsafe { self.line_primitive(primitive.cast(), actor.cast(), result.cast(),
            end.cast::<FVector>().read_unaligned(), start.cast::<FVector>().read_unaligned(),
            size.cast::<FVector>().read_unaligned(), flags) };
        match self.record(answer) {
            Ok(no_hit_value) => { unsafe { no_hit.write(i32::from(no_hit_value)); } 0 },
            Err(code) => code,
        }
    }

    unsafe fn alloc_check_result(&self, _mem: *mut c_void, out: *mut *mut c_void) -> i32 {
        if out.is_null() { return self.record::<()>(Err(RuntimeError::new("Null collision result output"))).err().unwrap_or(0); }
        let layout = Layout::new::<FCheckResult>();
        let result = unsafe { self.memory.push_bytes(layout.size() as i32, layout.align() as i32) };
        match self.record(result) {
            Ok(result) => {
                // Native FCheckResult() is empty; hash policy initializes the returned fields.
                unsafe { out.write(result.cast()); }
                0
            },
            Err(code) => code,
        }
    }

    unsafe fn debugf_moved_without_hashing(&self, actor: *const c_void) -> i32 {
        let result = (|| {
            let mut message = unsafe { self.access.object_full_name(actor.cast_mut().cast())? };
            message.extend(" moved without proper hashing".chars().map(u32::from));
            let category = self.access.find_name(&[76, 111, 103])?;
            self.access.emit_log(category, &message)
        })();
        self.record(result)
            .map_or_else(|code| code, |_| 0)
    }

    unsafe fn check_failed(&self, expression: *const c_char, file: *const c_char, line: i32) -> i32 {
        let expression = if expression.is_null() {
            String::from("<null>")
        } else {
            unsafe { std::ffi::CStr::from_ptr(expression) }.to_string_lossy().into_owned()
        };
        let file = if file.is_null() {
            String::from("<null>")
        } else {
            unsafe { std::ffi::CStr::from_ptr(file) }.to_string_lossy().into_owned()
        };
        self.record::<()>(Err(RuntimeError::new("Canonical collision assertion failed")
            .with_context(&format!("{expression} ({file}:{line})"))))
            .err().unwrap_or(0)
    }
    unsafe fn actor_cylinder_extent(&self, actor: *mut c_void, out: *mut FVector) -> i32 {
        if actor.is_null() || out.is_null() {
            return self.record::<()>(Err(RuntimeError::new("Invalid actor extent arguments"))).err().unwrap_or(0);
        }
        let actor = actor.cast::<AActor>();
        let result = unsafe { self.cylinder_extent(actor, (*actor).collision_width) };
        match self.record(result) {
            Ok(extent) => { unsafe { out.write(extent); } 0 },
            Err(code) => code,
        }
    }
}

