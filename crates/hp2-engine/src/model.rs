//! Model geometry and lifecycle policy. Native arrays use shared Rust storage
//! operations while the live C++ object retains its ABI identity and vtables.

use hp2_abi::engine::generated::{
    FBox, FDecal, FLightMapIndex, FPoly, TArrayRaw, TTransArrayRaw, UModel, UPolys,
};
use hp2_abi::render_scene::{FBspNode, FBspSurf, FLeaf, FVert, FZoneProperties};
use hp2_core::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_abi::{FCoords, FPlane, FVector};
use std::{ffi::c_void, mem::size_of, ptr};

use crate::fpoly;
use crate::math::app_sqrt;
use crate::vec;

/// Storage element types used by model arrays and archive replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelArrayKind {
    Byte,
    Int32,
    Object,
    FVector,
    FVert,
    FBspNode,
    FBspSurf,
    FLeaf,
    FLightMapIndex,
    FBox,
    FPoly,
    FDecal,
}

/// Direct view of an array buffer embedded in a live native model object.
#[derive(Clone, Copy)]
pub(crate) struct ModelArray {
    pub(crate) slots: Hp2ArraySlots,
    pub(crate) width: i32,
    pub(crate) kind: ModelArrayKind,
}

impl ModelArrayKind {
    pub(crate) const fn width(self) -> i32 {
        match self {
            Self::Byte => 1,
            Self::Int32 => 4,
            Self::Object => size_of::<*mut c_void>() as i32,
            Self::FVector => size_of::<FVector>() as i32,
            Self::FVert => size_of::<FVert>() as i32,
            Self::FBspNode => size_of::<FBspNode>() as i32,
            Self::FBspSurf => size_of::<FBspSurf>() as i32,
            Self::FLeaf => size_of::<FLeaf>() as i32,
            Self::FLightMapIndex => size_of::<FLightMapIndex>() as i32,
            Self::FBox => size_of::<FBox>() as i32,
            Self::FPoly => size_of::<FPoly>() as i32,
            Self::FDecal => size_of::<FDecal>() as i32,
        }
    }
}

fn model_slots(raw: *mut TArrayRaw) -> Hp2ArraySlots {
    // SAFETY: callers derive `raw` from a live model/element field; slots preserve
    // the packed ABI's independently addressed pointer and count locations.
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*raw).data) },
        num: unsafe { ptr::addr_of_mut!((*raw).num) },
        max: unsafe { ptr::addr_of_mut!((*raw).max) },
    }
}

fn model_trans_slots(raw: *mut TTransArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*raw).data) },
        num: unsafe { ptr::addr_of_mut!((*raw).array_num) },
        max: unsafe { ptr::addr_of_mut!((*raw).array_max) },
    }
}

/// Returns compiler-probed array storage for a model archive/lifecycle field.
///
/// # Safety
/// `model` must point to a live native `UModel`; `Polys` requires live `UPolys`.
pub(crate) unsafe fn model_array(
    model: *mut UModel,
    field: crate::model_archive::Hp2ModelArchiveField,
) -> Result<ModelArray, i32> {
    use crate::model_archive::Hp2ModelArchiveField::*;
    // SAFETY: the caller guarantees the model and any non-null Polys object are live.
    unsafe {
        let (slots, kind) = match field {
            Vectors => (model_trans_slots(ptr::addr_of_mut!((*model).vectors)), ModelArrayKind::FVector),
            Points => (model_trans_slots(ptr::addr_of_mut!((*model).points)), ModelArrayKind::FVector),
            Nodes => (model_trans_slots(ptr::addr_of_mut!((*model).nodes)), ModelArrayKind::FBspNode),
            Surfs => (model_trans_slots(ptr::addr_of_mut!((*model).surfs)), ModelArrayKind::FBspSurf),
            Verts => (model_trans_slots(ptr::addr_of_mut!((*model).verts)), ModelArrayKind::FVert),
            Polys => {
                let polys = ptr::addr_of!((*model).polys).read_unaligned().cast::<UPolys>();
                if polys.is_null() { return Err(1); }
                (model_trans_slots(ptr::addr_of_mut!((*polys).element)), ModelArrayKind::FPoly)
            }
            LightMap => (model_slots(ptr::addr_of_mut!((*model).light_map)), ModelArrayKind::FLightMapIndex),
            LightBits => (model_slots(ptr::addr_of_mut!((*model).light_bits)), ModelArrayKind::Byte),
            Bounds => (model_slots(ptr::addr_of_mut!((*model).bounds)), ModelArrayKind::FBox),
            LeafHulls => (model_slots(ptr::addr_of_mut!((*model).leaf_hulls)), ModelArrayKind::Int32),
            Leaves => (model_slots(ptr::addr_of_mut!((*model).leaves)), ModelArrayKind::FLeaf),
            Lights => (model_slots(ptr::addr_of_mut!((*model).lights)), ModelArrayKind::Object),
            _ => return Err(1),
        };
        Ok(ModelArray { slots, width: kind.width(), kind })
    }
}
/// Destroys a typed element's nested arrays, without releasing its owning buffer.
///
/// # Safety
/// `element` points to a live element matching `kind`; `arrays` owns its buffers.
pub(crate) unsafe fn destroy_model_element(
    kind: ModelArrayKind,
    element: *mut c_void,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    // SAFETY: the caller guarantees this is a live element of `kind`, and every
    // nested buffer belongs to the supplied matching array allocator.
    unsafe {
        match kind {
            ModelArrayKind::FBspSurf => {
                let surf = element.cast::<FBspSurf>();
                array_storage::release(model_slots(ptr::addr_of_mut!((*surf).nodes).cast()), arrays)?;
                let decals = model_slots(ptr::addr_of_mut!((*surf).decals).cast());
                let count = decals.num.read();
                let data = decals.data.read_unaligned().cast::<FDecal>();
                for i in 0..count {
                    destroy_model_element(
                        ModelArrayKind::FDecal,
                        data.add(i as usize).cast(),
                        arrays,
                    )?;
                }
                array_storage::release(decals, arrays)?;
            }
            ModelArrayKind::FDecal => {
                array_storage::release(
                    model_slots(ptr::addr_of_mut!((*element.cast::<FDecal>()).nodes)),
                    arrays,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Empties a model array and destroys typed nested storage before releasing it.
///
/// # Safety
/// The descriptor points to an exclusively accessible live native array.
pub(crate) unsafe fn empty_model_array(
    array: ModelArray,
    slack: i32,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    // SAFETY: the caller guarantees exclusive access to a valid array allocation
    // and its live typed elements for the duration of nested destruction/release.
    unsafe {
        destroy_array_elements(array, arrays)?;
        array.slots.num.write(0);
        reallocate_model_array(array, 0, slack, arrays)
    }
}

/// Commits a replacement header only after the allocator has succeeded.
///
/// # Safety
/// The descriptor exclusively owns its allocation; retained elements fit `num`.
pub(crate) unsafe fn reallocate_model_array(
    array: ModelArray, num: i32, capacity: i32, arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    if capacity < 0 { return Err(1); }
    let mut bytes = 0;
    let status = unsafe { (arrays.checked_int_size)(
        arrays.context, capacity as usize * array.width as usize, &mut bytes,
    ) };
    if status != 0 { return Err(status); }
    unsafe {
        let mut header = TArrayRaw { data: array.slots.data.read_unaligned(), num, max: capacity };
        let staged = Hp2ArraySlots {
            data: ptr::addr_of_mut!(header.data),
            num: ptr::addr_of_mut!(header.num),
            max: ptr::addr_of_mut!(header.max),
        };
        array_storage::realloc(staged, array.width, ptr::null(), arrays)?;
        array.slots.data.write_unaligned(header.data);
        array.slots.num.write(header.num);
        array.slots.max.write(header.max);
    }
    Ok(())
}

unsafe fn destroy_array_elements(array: ModelArray, arrays: Hp2ArrayServices) -> Result<(), i32> {
    unsafe {
        if matches!(array.kind, ModelArrayKind::FBspSurf | ModelArrayKind::FDecal) {
            for i in 0..array.slots.num.read() {
                destroy_model_element(
                    array.kind,
                    array.slots.data.read_unaligned().cast::<u8>()
                        .add(i as usize * array.width as usize).cast(),
                    arrays,
                )?;
            }
        }
        Ok(())
    }
}

/// Releases one final buffer without an intermediate shrink/reallocation.
///
/// # Safety
/// The descriptor is exclusive, owns its live elements and uses this allocator.
pub(crate) unsafe fn release_model_array(array: ModelArray, arrays: Hp2ArrayServices) -> Result<(), i32> {
    unsafe {
        destroy_array_elements(array, arrays)?;
        array_storage::release(array.slots, arrays)
    }
}

/// Replaces implicit native member destruction in reverse declaration order.
///
/// # Safety
/// `model` is a live native shell undergoing final destruction, using `arrays`.
/// Its manually managed native array members must not be destroyed again.
pub unsafe fn destroy_storage(model: *mut c_void, arrays: Hp2ArrayServices) -> i32 {
    use crate::model_archive::Hp2ModelArchiveField::*;
    unsafe {
        let result = (|| -> Result<(), i32> {
            for field in [Lights, Leaves, LeafHulls, Bounds, LightBits, LightMap,
                Surfs, Points, Vectors, Verts, Nodes] {
                release_model_array(model_array(model.cast(), field)?, arrays)?;
            }
            Ok(())
        })();
        result.err().unwrap_or(0)
    }
}
/// Constructs facade members and makes reference slots safe to trace after an I/O failure.
///
/// # Safety
/// `element` points to storage for one element of `kind`; no existing value is live.
pub(crate) unsafe fn initialize_model_element(kind: ModelArrayKind, element: *mut c_void) {
    // SAFETY: the caller guarantees uninitialized, correctly sized element storage.
    unsafe {
        match kind {
            ModelArrayKind::Object => element.cast::<*mut c_void>().write(ptr::null_mut()),
            // FVector and FBspNode have trivial native constructors which do not initialize.
            ModelArrayKind::FBspSurf => {
                let surf = element.cast::<FBspSurf>();
                ptr::addr_of_mut!((*surf).texture).write(ptr::null_mut());
                ptr::addr_of_mut!((*surf).actor).write(ptr::null_mut());
                ptr::addr_of_mut!((*surf).decals).write_unaligned(hp2_abi::render_scene::TArrayRaw {
                    data: ptr::null_mut(), array_num: 0, array_max: 0,
                });
                ptr::addr_of_mut!((*surf).nodes).write_unaligned(hp2_abi::render_scene::TArrayRaw {
                    data: ptr::null_mut(), array_num: 0, array_max: 0,
                });
            }
            ModelArrayKind::FDecal => {
                ptr::addr_of_mut!((*element.cast::<FDecal>()).nodes)
                    .write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 });
            }
            ModelArrayKind::FPoly => {
                // FPoly's native default constructor only assigns ItemName = NAME_None.
                ptr::addr_of_mut!((*element.cast::<FPoly>()).item_name).write([0; 4]);
            }
            _ => {}
        }
    }
}

/// `PF_Selected` (`Engine/Inc/UnObj.h`): editor poly selection flag.
pub const PF_SELECTED: u32 = 0x0200_0000;

/// `FBox` value as the Rust side computes it. The C++ shell copies it into its
/// `FBox` member field by field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bound {
    pub min: FVector,
    pub max: FVector,
    pub is_valid: bool,
}

/// `FBox(0)`: zeroed corners, invalid.
pub fn empty_bound() -> Bound {
    Bound {
        min: vec::zero(),
        max: vec::zero(),
        is_valid: false,
    }
}

/// `::Min(A, B)` from `UnTemplate.h`: `(A<=B) ? A : B`.
fn min(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `::Max(A, B)` from `UnTemplate.h`: `(A>=B) ? A : B`.
fn max(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `FBox::operator+=(const FVector&)`: the first point seeds both corners.
fn bound_add(b: &mut Bound, p: FVector) {
    if b.is_valid {
        b.min = FVector {
            x: min(b.min.x, p.x),
            y: min(b.min.y, p.y),
            z: min(b.min.z, p.z),
        };
        b.max = FVector {
            x: max(b.max.x, p.x),
            y: max(b.max.y, p.y),
            z: max(b.max.z, p.z),
        };
    } else {
        b.min = p;
        b.max = p;
        b.is_valid = true;
    }
}

/// `FBox(const FVector* Points, INT Count)` over the points in iteration order.
pub fn bound_of_points(points: impl IntoIterator<Item = FVector>) -> Bound {
    let mut b = empty_bound();
    for p in points {
        bound_add(&mut b, p);
    }
    b
}

/// `FSphere(const FVector* Pts, INT Count)`. Returns `FPlane(0,0,0,0)` when no
/// points are given. Otherwise the center is the box center and the radius is the
/// farthest point distance, widened by 1.001 in DOUBLE before narrowing.
pub fn sphere_of_points<I>(points: I) -> FPlane
where
    I: Iterator<Item = FVector> + Clone,
{
    let bx = bound_of_points(points.clone());
    if !bx.is_valid {
        return FPlane {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        };
    }
    let center = vec::div(vec::add(bx.min, bx.max), 2.0);
    let mut w = 0.0f32;
    for p in points {
        // `FDistSquared(Pts[i], *this)`: V2 - V1 with V2 the sphere center. The
        // original object is not FMA-contracted here (fixture-verified).
        let d = vec::sub(center, p);
        let dist = d.x * d.x + d.y * d.y + d.z * d.z;
        if dist > w {
            w = dist;
        }
    }
    // `W = appSqrt(W) * 1.001f`: `appSqrt` is DOUBLE, so the multiply is DOUBLE.
    w = (app_sqrt(f64::from(w)) * f64::from(1.001f32)) as f32;
    FPlane {
        x: center.x,
        y: center.y,
        z: center.z,
        w,
    }
}

/// `UModel::BuildBound` for the polygon list. Returns the box and, when there are
/// polygons, the sphere. With no polygons the C++ keeps `BoundingSphere` untouched.
pub fn build_bound(polys: &[FPoly]) -> (Bound, Option<FPlane>) {
    if polys.is_empty() {
        return (empty_bound(), None);
    }
    // Vertices in polygon order, then vertex order, as the C++ `Points` TArray.
    let points = polys.iter().flat_map(|p| {
        let n = usize::try_from(p.num_vertices).unwrap_or(0);
        p.vertex[..n].iter().copied()
    });
    let bx = bound_of_points(points.clone());
    (bx, Some(sphere_of_points(points)))
}

/// `UModel::PotentiallyVisible`: "the amazing superfast patent-pending 1 cpu cycle
/// potential visibility algorithm". Always visible.
pub fn potentially_visible(_leaf1: i32, _leaf2: i32) -> i32 {
    1
}

/// Services the surface decisions call back into. They are C++ `TTransArray` and
/// `UModel` operations (transaction hooks and `TArray::AddItem`), not math.
pub trait ModelServices {
    /// `UModel::Surfs.ModifyItem(index)` on the owning model.
    fn surf_modify_item(&mut self, index: i32);
    /// `Surf.Actor->Brush->Polys->Element.ModifyItem(iBrushPoly)`.
    fn brush_poly_modify_item(&mut self, actor: *mut c_void, index: i32);
    /// `Surfs(surf).Nodes.AddItem(node)` on the owning model.
    fn surf_add_node(&mut self, surf: i32, node: i32);
}

/// `UModel::ModifySurf(Index, UpdateMaster)`. Indices outside `surfs` are a C++
/// out-of-bounds read (`checkSlow` is compiled out), so the port skips the
/// master update for them, after the transaction hook has run.
pub fn modify_surf(
    svc: &mut impl ModelServices,
    surfs: &[FBspSurf],
    index: i32,
    update_master: i32,
) {
    svc.surf_modify_item(index);
    let Some(surf) = usize::try_from(index).ok().and_then(|i| surfs.get(i)) else {
        return;
    };
    if update_master != 0 && !surf.actor.is_null() {
        svc.brush_poly_modify_item(surf.actor, surf.i_brush_poly);
    }
}

/// `UModel::ModifyAllSurfs(UpdateMaster)`.
pub fn modify_all_surfs(svc: &mut impl ModelServices, surfs: &[FBspSurf], update_master: i32) {
    for i in 0..surfs.len() {
        modify_surf(svc, surfs, i as i32, update_master);
    }
}

/// `UModel::ModifySelectedSurfs(UpdateMaster)`: only surfaces with `PF_Selected`.
pub fn modify_selected_surfs(svc: &mut impl ModelServices, surfs: &[FBspSurf], update_master: i32) {
    for (i, surf) in surfs.iter().enumerate() {
        if surf.poly_flags & PF_SELECTED != 0 {
            modify_surf(svc, surfs, i as i32, update_master);
        }
    }
}

/// `UModel::PostLoad`'s loop: each node registers itself with its surface.
pub fn post_load_nodes(svc: &mut impl ModelServices, nodes: &[FBspNode]) {
    for (surface, node) in node_surfaces(nodes) {
        svc.surf_add_node(surface, node);
    }
}

pub(crate) fn node_surfaces(nodes: &[FBspNode]) -> impl Iterator<Item = (i32, i32)> + '_ {
    nodes.iter().enumerate().map(|(index, node)| (node.i_surf, index as i32))
}

/// `EmptyModel`'s zone reset: every zone is its own connectivity bit, sees all
/// zones, and has no zone actor. `LastRenderTime` is left as it was.
pub fn reset_zones(zones: &mut [FZoneProperties]) {
    debug_assert!(zones.len() <= 64, "connectivity holds 64 zone bits");
    for (i, zone) in zones.iter_mut().enumerate() {
        zone.zone_actor = std::ptr::null_mut();
        zone.connectivity = 1u64 << i;
        zone.visibility = !0u64;
    }
}

/// Resets compiler-probed packed zone storage without assuming 8-byte object alignment.
///
/// # Safety
/// `zones` addresses 64 initialized native zone records in the live UModel.
pub(crate) unsafe fn reset_model_zones(zones: *mut FZoneProperties) {
    // SAFETY: the model layout provides 64 initialized records; unaligned writes
    // preserve the native packed ABI and leave LastRenderTime untouched.
    unsafe {
        for i in 0..64 {
            let zone = zones.cast::<u8>().add(i * size_of::<FZoneProperties>()).cast::<FZoneProperties>();
            ptr::addr_of_mut!((*zone).zone_actor).write_unaligned(ptr::null_mut());
            ptr::addr_of_mut!((*zone).connectivity).write_unaligned(1u64 << i);
            ptr::addr_of_mut!((*zone).visibility).write_unaligned(!0u64);
        }
    }
}

/// Lifecycle entrypoints shared with the native model shell.
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum Hp2ModelLifecycleOperation {
    Modify,
    Empty,
    Shrink,
    Initialize,
}

/// External transaction and UObject graph operations used by model lifecycle.
#[repr(C)]
pub struct Hp2ModelLifecycleServices {
    pub allocator: *const Hp2ArrayServices,
    pub undo_empty: unsafe extern "C" fn(*mut c_void, crate::model_archive::Hp2ModelArchiveField) -> i32,
    pub modify_polys: unsafe extern "C" fn(*mut c_void) -> i32,
    pub create_polys: unsafe extern "C" fn(*mut c_void) -> i32,
    pub transactional: unsafe extern "C" fn(*mut c_void) -> i32,
    pub bind_owner: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}

fn lifecycle_status(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

unsafe fn direct_empty(
    model: *mut UModel,
    field: crate::model_archive::Hp2ModelArchiveField,
    slack: i32,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    // SAFETY: this caller holds the live model exclusively through the operation.
    let array = unsafe { model_array(model, field)? };
    // SAFETY: the descriptor came from this live model and its elements are owned.
    unsafe { empty_model_array(array, slack, arrays) }
}

unsafe fn empty_model(
    model: *mut UModel,
    empty_surfs: i32,
    empty_polys: i32,
    s: &Hp2ModelLifecycleServices,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    use crate::model_archive::Hp2ModelArchiveField::*;
    // SAFETY: caller guarantees exclusive access to live model storage; array and
    // transaction leaves are synchronous and do not retain the model pointer.
    unsafe {
        // Preserve each transaction checkpoint immediately before its mutation.

        for field in [Nodes, Bounds, LeafHulls, Leaves, Lights, LightMap, LightBits, Verts] {
            if matches!(field, Nodes | Verts) {
                lifecycle_status((s.undo_empty)(model.cast(), field))?;
            }
            direct_empty(model, field, 0, arrays)?;
        }
        if empty_surfs != 0 {
            for field in [Vectors, Points] {
                lifecycle_status((s.undo_empty)(model.cast(), field))?;
                direct_empty(model, field, 0, arrays)?;
            }
            let surfaces = model_array(model, Surfs)?;
            for i in 0..surfaces.slots.num.read() {
                let surface = surfaces.slots.data.read_unaligned().cast::<FBspSurf>().add(i as usize);
                let raw = ptr::addr_of_mut!((*surface).decals).cast::<TArrayRaw>();
                empty_model_array(ModelArray {
                    slots: model_slots(raw),
                    width: ModelArrayKind::FDecal.width(), kind: ModelArrayKind::FDecal,
                }, 0, arrays)?;
            }
            lifecycle_status((s.undo_empty)(model.cast(), Surfs))?;
            direct_empty(model, Surfs, 0, arrays)?;
        }
        if empty_polys != 0 {
            // ModelCreatePolys constructs the replacement UObject with native outer/flags;
            // its new Element array is already empty, so Rust does not touch that buffer.
            lifecycle_status((s.create_polys)(model.cast()))?;
        }

        (*model).num_shared_sides = 4;
        (*model).num_zones = 0;
        reset_model_zones(ptr::addr_of_mut!((*model).zones).cast::<FZoneProperties>());
        Ok(())
    }
}

/// Runs model lifecycle policy over direct native array storage.
///
/// # Safety
/// Model and optional owner are live native objects of the matching types.
/// Service callbacks capture exceptions synchronously and array allocator callbacks
/// remain paired with every buffer currently owned by the model.
pub unsafe fn lifecycle(
    model: *mut c_void,
    owner: *mut c_void,
    operation: Hp2ModelLifecycleOperation,
    empty_surfs: i32,
    empty_polys: i32,
    s: &Hp2ModelLifecycleServices,
) -> i32 {
    use crate::model_archive::Hp2ModelArchiveField::*;
    let result = (|| -> Result<(), i32> {
        if model.is_null() || s.allocator.is_null() {
            return Err(1);
        }
        // SAFETY: the caller and native adapter guarantee live compiler-probed objects
        // and a stable allocator resource for this synchronous lifecycle operation.
        let (model, arrays) = unsafe { (model.cast::<UModel>(), s.allocator.read()) };
        match operation {
            Hp2ModelLifecycleOperation::Modify => {
                // SAFETY: model is live for the call; the callback captures exceptions.
                unsafe {
                    let polys = ptr::addr_of!((*model).polys).read_unaligned();
                    if !polys.is_null() {
                        lifecycle_status((s.modify_polys)(polys))?;
                    }
                }
            }
            Hp2ModelLifecycleOperation::Empty => {
                // SAFETY: the model and paired allocator are live and exclusive.
                unsafe { empty_model(model, empty_surfs, empty_polys, s, arrays)?; }
            }
            Hp2ModelLifecycleOperation::Shrink => {
                // SAFETY: model arrays are live, exclusive, and owned by this allocator.
                unsafe {
                    for field in [Vectors, Points, Verts, Nodes, Surfs] {
                        let array = model_array(model, field)?;
                        array_storage::shrink(
                            array.slots, array.width, ptr::null(), arrays,
                        )?;
                    }
                    let polys = ptr::addr_of!((*model).polys).read_unaligned();
                    if !polys.is_null() {
                        let array = model_array(model, Polys)?;
                        array_storage::shrink(
                            array.slots, array.width, ptr::null(), arrays,
                        )?;
                    }
                    for field in [Bounds, LeafHulls] {
                        let array = model_array(model, field)?;
                        array_storage::shrink(
                            array.slots, array.width, ptr::null(), arrays,
                        )?;
                    }
                }
            }
            Hp2ModelLifecycleOperation::Initialize => {
                // SAFETY: model and allocator remain live; callbacks are checked leaves.
                unsafe {
                    lifecycle_status((s.transactional)(model.cast()))?;
                    empty_model(model, 1, 1, s, arrays)?;
                    if !owner.is_null() {
                        lifecycle_status((s.bind_owner)(model.cast(), owner))?;
                    }
                }
            }
        }
        Ok(())
    })();
    result.err().unwrap_or(0)
}

/// `UModel::Transform`'s per-polygon loop: `Polys->Element(i).Transform(Coords,
/// PrePivot, Location, Orientation)` for every polygon in order.
pub fn transform_polys(
    polys: &mut [FPoly],
    point_xform: &FCoords,
    vector_xform: &FCoords,
    pre: FVector,
    post: FVector,
    orientation: f32,
) {
    for poly in polys {
        fpoly::transform(poly, point_xform, vector_xform, pre, post, orientation);
    }
}

/// Releases the typed nested storage held by a transaction snapshot.
///
/// # Safety
/// `element` matches a transactional geometry field and its buffers use `arrays`.
pub unsafe fn destroy_transaction_item(
    element: *mut c_void,
    field: crate::model_archive::Hp2ModelArchiveField,
    arrays: Hp2ArrayServices,
) -> i32 {
    // SAFETY: the undo owner supplies the matching typed item and allocator.
    unsafe {
        destroy_model_element(crate::model_archive::item_kind(field), element, arrays)
            .err().unwrap_or(0)
    }
}
