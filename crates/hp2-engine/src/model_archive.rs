//! Model and legacy geometry archives over Rust-owned typed buffer operations.
use std::{ffi::c_void, mem::size_of, ptr};
use hp2_abi::{FVector, engine::generated::{self, TArrayRaw, TTransArrayRaw, UModel}, render_scene::{FVert, FLeaf, FZoneProperties}};
use hp2_core::{array_archive as array, array_storage::{self, Hp2ArrayServices, Hp2ArraySlots}};
use crate::{model::{self, ModelArray, ModelArrayKind as Kind}, model_archive_io::{Hp2ModelArchiveIoServices, NativeModelArchive}, model_struct_archive::{self, Hp2ModelStructArchiveServices, Hp2ModelStructKind}};

type Result<T = ()> = std::result::Result<T, i32>;
fn checked(status: i32) -> Result { if status == 0 { Ok(()) } else { Err(status) } }

/// Fields in their native archive order.
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum Hp2ModelArchiveField {
    Vectors, Points, Nodes, Surfs, Verts, Polys, NumSharedSides, NumZones,
    Zone, LightMap, LightBits, Bounds, LeafHulls, Leaves, Lights, RootOutside, Linked,
}

/// Geometry database layout used by old packages and polygon objects.
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2ModelDatabaseKind { Nodes, Surfs, Vectors, Verts, Polys }

pub type Hp2ModelArchiveContinuation = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

/// Only virtual archive, object graph, transaction and allocator resource leaves.
#[repr(C)]
pub struct Hp2ModelArchiveServices {
    pub io: *const Hp2ModelArchiveIoServices,
    pub allocator: *const Hp2ArrayServices,
    pub structs: *const Hp2ModelStructArchiveServices,
    pub super_serialize: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub version: unsafe extern "C" fn(*mut c_void) -> i32,
    pub is_transaction: unsafe extern "C" fn(*mut c_void) -> bool,
    pub preload: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// None when the owner has no native allocation-tag resource.
    pub tag_scope: Option<unsafe extern "C" fn(Hp2ModelArchiveField, *mut c_void, Hp2ModelArchiveContinuation) -> i32>,
    /// None when no undo recorder is active for this owner.
    pub save_array: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, Hp2ModelArchiveField, i32, i32, i32) -> i32>,
    pub fast_arrays: bool,
}

unsafe fn slots(raw: *mut TArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*raw).data) },
        num: unsafe { ptr::addr_of_mut!((*raw).num) },
        max: unsafe { ptr::addr_of_mut!((*raw).max) },
    }
}
unsafe fn descriptor(raw: *mut TArrayRaw, kind: Kind) -> ModelArray {
    ModelArray { slots: unsafe { slots(raw) }, width: kind.width(), kind }
}

unsafe fn trans_slots(raw: *mut TTransArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*raw).data) },
        num: unsafe { ptr::addr_of_mut!((*raw).array_num) },
        max: unsafe { ptr::addr_of_mut!((*raw).array_max) },
    }
}

unsafe fn trans_descriptor(raw: *mut TTransArrayRaw, kind: Kind) -> ModelArray {
    ModelArray { slots: unsafe { trans_slots(raw) }, width: kind.width(), kind }
}

unsafe fn vector(io: &NativeModelArchive<'_>, p: *mut FVector) -> Result {
    unsafe {
        checked(io.scalar(ptr::addr_of_mut!((*p).x)))?;
        checked(io.scalar(ptr::addr_of_mut!((*p).y)))?;
        checked(io.scalar(ptr::addr_of_mut!((*p).z)))
    }
}
unsafe fn zone(io: &NativeModelArchive<'_>, p: *mut FZoneProperties) -> Result {
    unsafe {
        checked(io.object(ptr::addr_of_mut!((*p).zone_actor)))?;
        checked(io.scalar(ptr::addr_of_mut!((*p).connectivity)))?;
        checked(io.scalar(ptr::addr_of_mut!((*p).visibility)))
    }
}

unsafe fn element(io: &NativeModelArchive<'_>, archive: *mut c_void, data: *mut c_void, kind: Kind, s: &Hp2ModelArchiveServices) -> Result {
    unsafe {
        match kind {
            Kind::Byte => checked(io.scalar(data.cast::<u8>())),
            Kind::Int32 => checked(io.scalar(data.cast::<i32>())),
            Kind::Object => checked(io.object(data.cast())),
            Kind::FVector => vector(io, data.cast()),
            Kind::FVert => {
                let p = data.cast::<FVert>();
                checked(io.index(ptr::addr_of_mut!((*p).p_vertex)))?;
                checked(io.index(ptr::addr_of_mut!((*p).i_side)))
            }
            Kind::FBspNode | Kind::FBspSurf | Kind::FPoly => checked(model_struct_archive::serialize(
                data, archive, match kind { Kind::FBspNode => Hp2ModelStructKind::Node, Kind::FBspSurf => Hp2ModelStructKind::Surf, _ => Hp2ModelStructKind::Poly }, &*s.structs,
            )),
            Kind::FLeaf => {
                let p = data.cast::<FLeaf>();
                checked(io.index(ptr::addr_of_mut!((*p).i_zone)))?;
                checked(io.index(ptr::addr_of_mut!((*p).i_permeating)))?;
                checked(io.index(ptr::addr_of_mut!((*p).i_volumetric)))?;
                checked(io.scalar(ptr::addr_of_mut!((*p).visible_zones)))
            }
            Kind::FLightMapIndex => {
                let p = data.cast::<generated::FLightMapIndex>();
                checked(io.scalar(ptr::addr_of_mut!((*p).data_offset)))?;
                vector(io, ptr::addr_of_mut!((*p).pan))?;
                checked(io.index(ptr::addr_of_mut!((*p).u_clamp)))?;
                checked(io.index(ptr::addr_of_mut!((*p).v_clamp)))?;
                checked(io.scalar(ptr::addr_of_mut!((*p).u_scale)))?;
                checked(io.scalar(ptr::addr_of_mut!((*p).v_scale)))?;
                checked(io.scalar(ptr::addr_of_mut!((*p).i_light_actors)))
            }
            Kind::FBox => {
                let p = data.cast::<generated::FBox>();
                vector(io, ptr::addr_of_mut!((*p).min))?;
                vector(io, ptr::addr_of_mut!((*p).max))?;
                checked(io.scalar(ptr::addr_of_mut!((*p).is_valid)))
            }
            Kind::FDecal => {
                if !io.is_loading() && !io.is_saving() {
                    checked(io.object(ptr::addr_of_mut!((*data.cast::<generated::FDecal>()).actor)))?;
                }
                Ok(())
            }
        }
    }
}

struct ArrayContext<'a> {
    array: ModelArray,
    io: NativeModelArchive<'a>,
    archive: *mut c_void,
    services: &'a Hp2ModelArchiveServices,
    loading_count: std::cell::Cell<i32>,
}

// The same Core array state machine used by TArray drives direct Rust storage.
unsafe extern "C" fn array_operation(context: *mut c_void, _: *mut c_void, op: i32, arg: i32, value: *mut c_void) -> i32 {
    unsafe {
        let c = &*context.cast::<ArrayContext<'_>>();
        let a = c.array;
        let arrays = *c.services.allocator;
        let result = (|| -> Result {
            match op {
                array::LOADING => value.cast::<i32>().write(i32::from(c.io.is_loading())),
                array::SAVING => value.cast::<i32>().write(i32::from(c.io.is_saving())),
                array::NUM => value.cast::<i32>().write(a.slots.num.read()),
                array::FAST => value.cast::<i32>().write(i32::from(c.services.fast_arrays && matches!(a.kind, Kind::Byte | Kind::Int32 | Kind::FVector))),
                array::NEEDS_DESTRUCTOR => value.cast::<i32>().write(i32::from(matches!(a.kind, Kind::FBspSurf | Kind::FDecal))),
                array::COUNT_BYTES => checked(c.io.count_bytes(a.slots.num.read() as usize * a.width as usize, a.slots.max.read() as usize * a.width as usize))?,
                array::COUNT_INDEX if c.io.is_loading() => {
                    let mut count = 0;
                    checked(c.io.index(&mut count))?;
                    c.loading_count.set(count);
                }
                array::COUNT_INDEX => checked(c.io.index(a.slots.num))?,
                array::COMPACT => checked(c.io.index(value.cast()))?,
                array::SET_EXACT => {
                    let count = c.loading_count.get();
                    model::reallocate_model_array(a, count, count, arrays)?;
                }
                array::RAW_SERIALIZE => {
                    let mut bytes = 0;
                    checked((arrays.checked_int_size)(arrays.context, a.slots.num.read() as usize * a.width as usize, &mut bytes))?;
                    checked(c.io.raw(a.slots.data.read_unaligned(), bytes))?;
                }
                array::DESTRUCT => model::destroy_model_element(a.kind, a.slots.data.read_unaligned().cast::<u8>().add(arg as usize * a.width as usize).cast(), arrays)?,
                array::EMPTY_STORAGE => {
                    // The state machine already destroyed the old live elements.
                    a.slots.num.write(0);
                    model::reallocate_model_array(a, 0, arg, arrays)?;
                }
                array::CONSTRUCT_SERIALIZE => {
                    let index = array_storage::add(a.slots, 1, a.width, ptr::null(), arrays)?;
                    let p = a.slots.data.read_unaligned().cast::<u8>().add(index as usize * a.width as usize).cast();
                    model::initialize_model_element(a.kind, p);
                    element(&c.io, c.archive, p, a.kind, c.services)?;
                }
                array::ITEM_SERIALIZE => element(&c.io, c.archive, a.slots.data.read_unaligned().cast::<u8>().add(arg as usize * a.width as usize).cast(), a.kind, c.services)?,
                _ => return Err(1),
            }
            Ok(())
        })();
        result.err().unwrap_or(0)
    }
}

unsafe fn serialize_array(a: ModelArray, archive: *mut c_void, s: &Hp2ModelArchiveServices) -> Result {
    unsafe {
        let mut context = ArrayContext { array: a, io: NativeModelArchive::new(archive, &*s.io), archive, services: s, loading_count: std::cell::Cell::new(0) };
        array::run((&raw mut context).cast(), archive, array_operation, array::action::ARRAY, 0, ptr::null_mut())
    }
}

fn tagged(s: &Hp2ModelArchiveServices, field: Hp2ModelArchiveField, enabled: bool, mut body: impl FnMut() -> Result) -> Result {
    let Some(scope) = s.tag_scope.filter(|_| enabled) else { return body(); };
    // Native scope retains only the allocator's original memory tag lifetime.
    unsafe { hp2_core::native_text::scoped(
        |context, resume| scope(field, context, resume),
        |_| body().map_err(|_| ()),
    ) }.map_err(|()| 1)
}

unsafe fn field(model: *mut UModel, archive: *mut c_void, f: Hp2ModelArchiveField, s: &Hp2ModelArchiveServices) -> Result {
    use Hp2ModelArchiveField::*;
    unsafe {
        let io = NativeModelArchive::new(archive, &*s.io);
        match f {
            Polys => checked(io.object(ptr::addr_of_mut!((*model).polys))),
            NumSharedSides => checked(io.scalar(ptr::addr_of_mut!((*model).num_shared_sides))),
            NumZones => checked(io.scalar(ptr::addr_of_mut!((*model).num_zones))),
            RootOutside => checked(io.scalar(ptr::addr_of_mut!((*model).root_outside))),
            Linked => checked(io.scalar(ptr::addr_of_mut!((*model).linked))),
            Vectors | Points | Nodes | Surfs | Verts if (s.is_transaction)(archive) => Ok(()),
            _ => serialize_array(model::model_array(model, f)?, archive, s),
        }
    }
}

unsafe fn preload_polys(model: *mut UModel, archive: *mut c_void, s: &Hp2ModelArchiveServices) -> Result {
    unsafe {
        let polys = ptr::addr_of!((*model).polys).read_unaligned();
        if !polys.is_null() && !(s.is_transaction)(archive) { checked((s.preload)(archive, polys))?; }
        Ok(())
    }
}

pub(crate) unsafe fn database_array(object: *mut c_void, kind: Hp2ModelDatabaseKind) -> ModelArray {
    use Hp2ModelDatabaseKind::*;
    unsafe {
        match kind {
            Nodes => trans_descriptor(ptr::addr_of_mut!((*object.cast::<generated::UBspNodes>()).element), Kind::FBspNode),
            Surfs => descriptor(ptr::addr_of_mut!((*object.cast::<generated::UBspSurfs>()).element), Kind::FBspSurf),
            Vectors => descriptor(ptr::addr_of_mut!((*object.cast::<generated::UVectors>()).element), Kind::FVector),
            Verts => descriptor(ptr::addr_of_mut!((*object.cast::<generated::UVerts>()).element), Kind::FVert),
            Polys => trans_descriptor(ptr::addr_of_mut!((*object.cast::<generated::UPolys>()).element), Kind::FPoly),
        }
    }
}

/// Serializes one complete live native model using direct Rust field/storage policy.
///
/// # Safety
/// Pointers name live native ABI records; services capture exceptions and do not unwind.
/// All model buffers use the supplied allocator. No native graph enters CoreAccess.
pub unsafe fn serialize(model: *mut c_void, archive: *mut c_void, s: &Hp2ModelArchiveServices) -> i32 {
    use Hp2ModelArchiveField::*;
    unsafe {
        let model = model.cast::<UModel>();
        let io = NativeModelArchive::new(archive, &*s.io);
        let result = (|| -> Result {
            checked((s.super_serialize)(model.cast(), archive))?;
            let modern = (s.version)(archive) > 61;
            if modern {
                for f in [Vectors, Points, Nodes, Surfs, Verts] { tagged(s, f, true, || field(model, archive, f, s))?; }
                field(model, archive, NumSharedSides, s)?;
                field(model, archive, NumZones, s)?;
                if ptr::addr_of!((*model).num_zones).read_unaligned() > 64 { return Err(1); }
                tagged(s, Zone, true, || {
                    for i in 0..ptr::addr_of!((*model).num_zones).read_unaligned() {
                        zone(&io, ptr::addr_of_mut!((*model).zones).cast::<u8>().add(i as usize * size_of::<FZoneProperties>()).cast())?;
                    }
                    Ok(())
                })?;
                tagged(s, Polys, true, || field(model, archive, Polys, s))?;
            } else {
                let mut objects = [ptr::null_mut(); 5];
                for object in &mut objects { checked(io.object(object))?; }
                field(model, archive, Polys, s)?;
                for (i, f, kind) in [
                    (0, Vectors, Hp2ModelDatabaseKind::Vectors), (1, Points, Hp2ModelDatabaseKind::Vectors),
                    (3, Surfs, Hp2ModelDatabaseKind::Surfs), (4, Verts, Hp2ModelDatabaseKind::Verts),
                    (2, Nodes, Hp2ModelDatabaseKind::Nodes),
                ] {
                    let object = objects[i];
                    if object.is_null() { continue; }
                    checked((s.preload)(archive, object))?;
                    let source = database_array(object, kind);
                    let destination = model::model_array(model, f)?;
                    let old = destination.slots.data.read_unaligned();
                    let old_num = destination.slots.num.read();
                    let old_max = destination.slots.max.read();
                    destination.slots.data.write_unaligned(source.slots.data.read_unaligned());
                    destination.slots.num.write(source.slots.num.read());
                    destination.slots.max.write(source.slots.max.read());
                    source.slots.data.write_unaligned(old);
                    source.slots.num.write(old_num);
                    source.slots.max.write(old_max);
                    if kind == Hp2ModelDatabaseKind::Verts {
                        ptr::addr_of_mut!((*model).num_shared_sides).write_unaligned(ptr::addr_of!((*object.cast::<generated::UVerts>()).num_shared_sides).read_unaligned());
                    } else if kind == Hp2ModelDatabaseKind::Nodes {
                        let source = object.cast::<generated::UBspNodes>();
                        let count = ptr::addr_of!((*source)._num_zones).read_unaligned();
                        if count > 64 { return Err(1); }
                        ptr::addr_of_mut!((*model).num_zones).write_unaligned(count);
                        for i in 0..count {
                            ptr::copy_nonoverlapping(ptr::addr_of!((*source)._zones).cast::<u8>().add(i as usize * size_of::<FZoneProperties>()), ptr::addr_of_mut!((*model).zones).cast::<u8>().add(i as usize * size_of::<FZoneProperties>()), size_of::<FZoneProperties>());
                        }
                    }
                }
            }
            preload_polys(model, archive, s)?;
            for f in [LightMap, LightBits, Bounds, LeafHulls, Leaves, Lights] { tagged(s, f, modern, || field(model, archive, f, s))?; }
            if !modern {
                let mut discard = ptr::null_mut();
                checked(io.object(&mut discard))?;
                checked(io.object(&mut discard))?;
            }
            field(model, archive, RootOutside, s)?;
            field(model, archive, Linked, s)
        })();
        result.err().unwrap_or(0)
    }
}

/// Archives legacy database payload after its native UObject superclass serializer.
///
/// # Safety
/// Object matches kind, services are live, and all buffers use the supplied allocator.
pub unsafe fn serialize_database(object: *mut c_void, archive: *mut c_void, kind: Hp2ModelDatabaseKind, s: &Hp2ModelArchiveServices) -> i32 {
    unsafe {
        let io = NativeModelArchive::new(archive, &*s.io);
        let a = database_array(object, kind);
        let arrays = *s.allocator;
        let result = (|| -> Result {
            let polys = kind == Hp2ModelDatabaseKind::Polys;
            if polys && (s.is_transaction)(archive) { return Ok(()); }
            if io.is_loading() || polys {
                if polys || kind == Hp2ModelDatabaseKind::Verts {
                    checked(io.count_bytes(a.slots.num.read() as usize * a.width as usize, a.slots.max.read() as usize * a.width as usize))?;
                }
                let mut count = if kind == Hp2ModelDatabaseKind::Surfs { 0 } else { a.slots.num.read() };
                let mut capacity = count;
                checked(io.scalar(&mut count))?;
                checked(io.scalar(&mut capacity))?;
                if io.is_loading() {
                    let transaction = polys || kind == Hp2ModelDatabaseKind::Nodes;
                    let f = if polys { Hp2ModelArchiveField::Polys } else { Hp2ModelArchiveField::Nodes };
                    let raw_array = a.slots.data.cast::<c_void>();
                    let owner = if transaction { raw_array.cast::<TTransArrayRaw>().read_unaligned().owner } else { ptr::null_mut() };
                    if transaction {
                        if let Some(save) = s.save_array { checked(save(owner, raw_array, f, 0, a.slots.num.read(), -1))?; }
                    }
                    model::empty_model_array(a, count, arrays)?;
                    let index = if transaction && count == 0 { a.slots.num.read() } else { array_storage::add(a.slots, count, a.width, ptr::null(), arrays)? };
                    if transaction && count != 0 {
                        if let Some(save) = s.save_array { checked(save(owner, raw_array, f, index, count, 1))?; }
                    }
                    if polys || matches!(kind, Hp2ModelDatabaseKind::Nodes | Hp2ModelDatabaseKind::Surfs) {
                        let mut bytes = 0;
                        checked((arrays.checked_int_size)(arrays.context, count as usize * a.width as usize, &mut bytes))?;
                        if bytes != 0 { ptr::write_bytes(a.slots.data.read_unaligned().cast::<u8>().add(index as usize * a.width as usize), 0, bytes as usize); }
                    }
                }
                for i in 0..a.slots.num.read() {
                    element(&io, archive, a.slots.data.read_unaligned().cast::<u8>().add(i as usize * a.width as usize).cast(), a.kind, s)?;
                }
                if kind == Hp2ModelDatabaseKind::Nodes {
                    let node = object.cast::<generated::UBspNodes>();
                    checked(io.index(ptr::addr_of_mut!((*node)._num_zones)))?;
                    if ptr::addr_of!((*node)._num_zones).read_unaligned() > 64 { return Err(1); }
                    for i in 0..ptr::addr_of!((*node)._num_zones).read_unaligned() {
                        zone(&io, ptr::addr_of_mut!((*node)._zones).cast::<u8>().add(i as usize * size_of::<FZoneProperties>()).cast())?;
                    }
                }
            }
            if kind == Hp2ModelDatabaseKind::Verts { checked(io.index(ptr::addr_of_mut!((*object.cast::<generated::UVerts>()).num_shared_sides)))?; }
            Ok(())
        })();
        result.err().unwrap_or(0)
    }
}

/// Serializes a transaction-owned geometry item without native typed dispatch.
///
/// # Safety
/// Item matches the field's element type; archive/services are live and synchronous.
pub unsafe fn serialize_item(item: *mut c_void, archive: *mut c_void, field: Hp2ModelArchiveField, s: &Hp2ModelArchiveServices) -> i32 {
    unsafe { element(&NativeModelArchive::new(archive, &*s.io), archive, item, item_kind(field), s).err().unwrap_or(0) }
}
/// Element category for native undo records.
pub(crate) fn item_kind(field: Hp2ModelArchiveField) -> Kind {
    use Hp2ModelArchiveField::*;
    match field { Vectors | Points => Kind::FVector, Nodes => Kind::FBspNode, Surfs => Kind::FBspSurf, Verts => Kind::FVert, Polys => Kind::FPoly, _ => unreachable!("not a transactional geometry array") }
}

/// Replaces final native member destruction for a legacy geometry database.
///
/// # Safety
/// The native shell matches `kind`; its manually managed array is exclusively
/// owned by this allocator and must not be destroyed again.
pub unsafe fn destroy_database_storage(object: *mut c_void, kind: Hp2ModelDatabaseKind, arrays: hp2_core::array_storage::Hp2ArrayServices) -> i32 {
    unsafe { model::release_model_array(database_array(object, kind), arrays).err().unwrap_or(0) }
}
