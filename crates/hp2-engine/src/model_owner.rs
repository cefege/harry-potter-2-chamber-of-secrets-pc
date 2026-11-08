//! Canonical Core class operations for models and legacy geometry databases.
//!
//! Only this owner's Rust records reach CoreAccess. Native model callers use
//! their own archive/allocator services over the same typed payload kernels.

use crate::model;
use crate::model_archive::{self, Hp2ModelArchiveField as Field, Hp2ModelArchiveServices, Hp2ModelDatabaseKind as Database};
use crate::model_archive_io::Hp2ModelArchiveIoServices;
use crate::model_struct_archive::Hp2ModelStructArchiveServices;
use hp2_abi::engine::generated::{self, TArrayRaw, TTransArrayRaw, UObject, UModel};
use hp2_abi::render_scene::{FBspNode, FBspSurf};
use hp2_core::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_core::compact_index::Hp2CompactIndexServices;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_archive::{ArchiveMode, ObjectArchive};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, cell::RefCell, ffi::c_void, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const ARRAYS: Hp2ArrayServices = array_storage::system_allocator();

const fn spec(name: &'static str, superclass: &'static str, layout: Layout, operations: ClassOps, flags: u32) -> NativeClassSpec {
    NativeClassSpec {
        short_name: name, package: "Engine", super_name: Some(superclass), within: "Object",
        flags, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(layout), operations: Some(operations), properties: &[], defaults: &[],
    }
}

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    spec("Model", "BoxPrim", Layout::new::<UModel>(), MODEL,
        if generated::NODECALS_ENABLED != 0 { generated::CLASS_RUNTIME_STATIC } else { 0 }),
    spec("BspNodes", "Object", Layout::new::<generated::UBspNodes>(), NODES, generated::CLASS_RUNTIME_STATIC),
    spec("BspSurfs", "Object", Layout::new::<generated::UBspSurfs>(), SURFS, generated::CLASS_RUNTIME_STATIC),
    spec("Vectors", "Object", Layout::new::<generated::UVectors>(), VECTORS, generated::CLASS_RUNTIME_STATIC),
    spec("Verts", "Object", Layout::new::<generated::UVerts>(), VERTS, generated::CLASS_RUNTIME_STATIC),
    spec("Polys", "Object", Layout::new::<generated::UPolys>(), POLYS, generated::CLASS_RUNTIME_STATIC),
];

const MODEL: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_model;
    operations.finalize_members = finalize_model;
    operations.serialize = serialize_model;
    operations.visit_references = serialize_model;
    operations.post_load = post_load_model;
    operations
};

macro_rules! database_operations {
    ($ops:ident, $construct:ident, $finalize:ident, $serialize:ident, $kind:ident) => {
        const $ops: ClassOps = {
            let mut operations = OBJECT;
            operations.construct = $construct;
            operations.finalize_members = $finalize;
            operations.serialize = $serialize;
            operations.visit_references = $serialize;
            operations
        };
        unsafe fn $construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
            unsafe { construct_database(access, object, Database::$kind) }
        }
        unsafe fn $finalize(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
            unsafe { finalize_database(access, object, Database::$kind) }
        }
        unsafe fn $serialize(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
            unsafe { archive_object(access, object, archive, Some(Database::$kind)) }
        }
    };
}
database_operations!(NODES, construct_nodes, finalize_nodes, serialize_nodes, Nodes);
database_operations!(SURFS, construct_surfs, finalize_surfs, serialize_surfs, Surfs);
database_operations!(VECTORS, construct_vectors, finalize_vectors, serialize_vectors, Vectors);
database_operations!(VERTS, construct_verts, finalize_verts, serialize_verts, Verts);
database_operations!(POLYS, construct_polys, finalize_polys, serialize_polys, Polys);

fn storage_result(status: i32) -> RuntimeResult<()> {
    if status == 0 { Ok(()) } else { Err(RuntimeError::new("Model array storage operation failed")) }
}

// Constructors overwrite precisely the array facade storage initialized by the
// native constructors. Array Owner remains the newly constructed object, not CDO.
unsafe fn initialize_array(raw: *mut TArrayRaw) {
    unsafe { raw.write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
}

unsafe fn initialize_trans_array(raw: *mut TTransArrayRaw, owner: Option<*mut UObject>) {
    unsafe {
        raw.write_unaligned(TTransArrayRaw {
            data: ptr::null_mut(),
            array_num: 0,
            array_max: 0,
            owner: owner.map_or(ptr::null_mut(), |owner| owner.cast()),
        });
    }
}

unsafe fn construct_model(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (crate::primitive_owner::NATIVE_CLASSES[2].operations.unwrap().construct)(access, object)?;
        let model = object.cast::<UModel>();
        initialize_trans_array(ptr::addr_of_mut!((*model).nodes), Some(object));
        initialize_trans_array(ptr::addr_of_mut!((*model).verts), Some(object));
        initialize_trans_array(ptr::addr_of_mut!((*model).vectors), Some(object));
        initialize_trans_array(ptr::addr_of_mut!((*model).points), Some(object));
        initialize_trans_array(ptr::addr_of_mut!((*model).surfs), Some(object));
        initialize_array(ptr::addr_of_mut!((*model).light_map));
        initialize_array(ptr::addr_of_mut!((*model).light_bits));
        initialize_array(ptr::addr_of_mut!((*model).bounds));
        initialize_array(ptr::addr_of_mut!((*model).leaf_hulls));
        initialize_array(ptr::addr_of_mut!((*model).leaves));
        initialize_array(ptr::addr_of_mut!((*model).lights));
        ptr::addr_of_mut!((*model).root_outside).write_unaligned(1);
        ptr::addr_of_mut!((*model).num_shared_sides).write_unaligned(4);
        ptr::addr_of_mut!((*model).num_zones).write_unaligned(0);
        model::reset_model_zones(ptr::addr_of_mut!((*model).zones).cast());
    }
    Ok(())
}

unsafe fn construct_database(access: CoreAccess<'_>, object: *mut UObject, kind: Database) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.construct)(access, object)?;
        let array = model_archive::database_array(object.cast(), kind);
        if matches!(kind, Database::Nodes | Database::Polys) {
            initialize_trans_array(array.slots.data.cast(), Some(object));
        } else {
            initialize_array(array.slots.data.cast());
        }
        if kind == Database::Nodes {
            let nodes = object.cast::<generated::UBspNodes>();
            ptr::addr_of_mut!((*nodes)._num_zones).write_unaligned(0);
            model::reset_model_zones(ptr::addr_of_mut!((*nodes)._zones).cast());
        }
    }
    Ok(())
}

unsafe fn finalize_model(_access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    storage_result(unsafe { model::destroy_storage(object.cast(), ARRAYS) })
}

unsafe fn finalize_database(_access: CoreAccess<'_>, object: *mut UObject, kind: Database) -> RuntimeResult<()> {
    storage_result(unsafe { model_archive::destroy_database_storage(object.cast(), kind, ARRAYS) })
}

unsafe fn serialize_model(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { archive_object(access, object, archive, None) }
}

unsafe fn post_load_model(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        let model = object.cast::<UModel>();
        let nodes = model::model_array(model, Field::Nodes).map_err(|_| RuntimeError::new("Invalid node storage"))?;
        let surfs = model::model_array(model, Field::Surfs).map_err(|_| RuntimeError::new("Invalid surface storage"))?;
        let count = nodes.slots.num.read();
        let node_data = nodes.slots.data.read_unaligned().cast::<FBspNode>();
        let surf_data = surfs.slots.data.read_unaligned().cast::<FBspSurf>();
        let node_slice = if count == 0 { &[] } else { std::slice::from_raw_parts(node_data, count as usize) };
        for (surface, node) in model::node_surfaces(node_slice) {
            if surface < 0 || surface >= surfs.slots.num.read() {
                return Err(RuntimeError::new("Model node surface index is out of bounds"));
            }
            let raw = ptr::addr_of_mut!((*surf_data.add(surface as usize)).nodes).cast::<TArrayRaw>();
            let slots = Hp2ArraySlots { data: ptr::addr_of_mut!((*raw).data), num: ptr::addr_of_mut!((*raw).num), max: ptr::addr_of_mut!((*raw).max) };
            let index = array_storage::add(slots, 1, size_of::<i32>() as i32, ptr::null(), ARRAYS)
                .map_err(|_| RuntimeError::new("Model surface node allocation failed"))?;
            slots.data.read_unaligned().cast::<i32>().add(index as usize).write(node);
        }
        (OBJECT.post_load)(access, object)
    }
}

struct ArchiveCall<'owner, 'borrow, 'data> {
    access: CoreAccess<'owner>,
    archive: &'borrow ObjectArchive<'data>,
    database: Option<Database>,
    error: RefCell<Option<RuntimeError>>,
}
impl ArchiveCall<'_, '_, '_> {
    fn record(&self, result: RuntimeResult<()>) -> i32 {
        match result {
            Ok(()) => 0,
            Err(error) => {
                self.error.borrow_mut().get_or_insert(error);
                1
            }
        }
    }
}
// All callbacks borrow this stack frame synchronously; no context/reference escapes.
unsafe fn call<'a, 'b, 'c, 'd>(archive: *mut c_void) -> &'a ArchiveCall<'b, 'c, 'd> {
    unsafe { &*archive.cast::<ArchiveCall<'_, '_, '_>>() }
}

unsafe fn archive_object(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>, database: Option<Database>) -> RuntimeResult<()> {
    let mut context = ArchiveCall { access, archive, database, error: RefCell::new(None) };
    let io = Hp2ModelArchiveIoServices {
        compact: Hp2CompactIndexServices { archive_is_loading: is_loading, archive_is_saving: is_saving,
            archive_serialize_int: integer, archive_serialize_byte: byte, archive_mismatch: mismatch },
        bytes: bytes, scalar, object: object_reference, name: name_reference, count_bytes,
    };
    let structs = Hp2ModelStructArchiveServices { io: &io,
        transient_poly_flags: generated::POLYFLAG_TRANSIENT,
        transient_node_flags: generated::NODEFLAG_IS_NEW | generated::NODEFLAG_IS_FRONT | generated::NODEFLAG_IS_BACK,
        decals_enabled: generated::NODECALS_ENABLED == 0,
    };
    let services = Hp2ModelArchiveServices { io: &io, allocator: &ARRAYS, structs: &structs,
        super_serialize: superclass, version, is_transaction, preload,
        // This Rust runtime has neither a native tag scope nor an active editor undo recorder.
        // Native/editor callers continue to supply their real resources.
        tag_scope: None, save_array: None, fast_arrays: cfg!(target_endian = "little"),
    };
    let opaque = (&raw mut context).cast();
    let status = unsafe {
        match database {
            Some(kind) => {
                let status = superclass(object.cast(), opaque);
                if status == 0 { model_archive::serialize_database(object.cast(), opaque, kind, &services) } else { status }
            }
            None => model_archive::serialize(object.cast(), opaque, &services),
        }
    };
    match context.error.into_inner() {
        Some(error) => Err(error),
        None => storage_result(status),
    }
}

unsafe extern "C" fn is_loading(ar: *mut c_void) -> i32 { i32::from(unsafe { call(ar).archive.is_loading() }) }
unsafe extern "C" fn is_saving(ar: *mut c_void) -> i32 { i32::from(unsafe { call(ar).archive.is_saving() }) }
unsafe extern "C" fn is_transaction(ar: *mut c_void) -> bool { unsafe { call(ar).archive.is_trans() } }
unsafe extern "C" fn version(ar: *mut c_void) -> i32 { i32::from(unsafe { call(ar).archive.flags().version }) }
unsafe extern "C" fn mismatch(_: i32, _: i32) -> i32 { 1 }
unsafe extern "C" fn integer(ar: *mut c_void, value: *mut i32) -> i32 { unsafe { scalar(ar, value.cast(), 4) } }
unsafe extern "C" fn byte(ar: *mut c_void, value: *mut u8) -> i32 { unsafe { scalar(ar, value.cast(), 1) } }

unsafe extern "C" fn bytes(ar: *mut c_void, value: *mut c_void, count: i32) -> i32 {
    let context = unsafe { call(ar) };
    let result = if context.archive.is_loading() {
        // The destination may still contain uninitialized model elements.
        unsafe { context.archive.read_raw(value.cast(), count as usize) }
    } else if context.archive.is_saving() {
        let data = if count == 0 { &[] } else { unsafe { std::slice::from_raw_parts(value.cast::<u8>(), count as usize) } };
        context.archive.write(data)
    } else { Ok(()) };
    context.record(result)
}
unsafe extern "C" fn scalar(ar: *mut c_void, value: *mut c_void, count: i32) -> i32 {
    if cfg!(target_endian = "little") { return unsafe { bytes(ar, value, count) }; }
    let context = unsafe { call(ar) };
    if !context.archive.is_loading() && !context.archive.is_saving() { return 0; }
    let mut storage = [0u8; 8];
    let Some(data) = storage.get_mut(..count as usize) else { return context.record(Err(RuntimeError::new("Invalid model scalar width"))); };
    let result = if context.archive.is_loading() {
        context.archive.read(data).map(|()| {
            data.reverse();
            unsafe { ptr::copy_nonoverlapping(data.as_ptr(), value.cast(), data.len()); }
        })
    } else {
        unsafe { ptr::copy_nonoverlapping(value.cast(), data.as_mut_ptr(), data.len()); }
        data.reverse();
        context.archive.write(data)
    };
    context.record(result)
}
unsafe extern "C" fn object_reference(ar: *mut c_void, value: *mut *mut c_void) -> i32 {
    let context = unsafe { call(ar) };
    context.record(unsafe { context.archive.serialize_object_slot(value.cast()) })
}
unsafe extern "C" fn name_reference(ar: *mut c_void, value: *mut c_void) -> i32 {
    let context = unsafe { call(ar) };
    context.record(unsafe { context.archive.serialize_name_slot(value.cast()) })
}
unsafe extern "C" fn count_bytes(ar: *mut c_void, used: usize, capacity: usize) -> i32 {
    let context = unsafe { call(ar) };
    context.record(context.archive.count_bytes(used, capacity))
}
unsafe extern "C" fn superclass(object: *mut c_void, ar: *mut c_void) -> i32 {
    let context = unsafe { call(ar) };
    context.record(unsafe {
        if context.database.is_some() { (OBJECT.serialize)(context.access, object.cast(), context.archive) }
        else { crate::primitive_owner::serialize_box_primitive(context.access, object.cast(), context.archive) }
    })
}
unsafe extern "C" fn preload(ar: *mut c_void, object: *mut c_void) -> i32 {
    let context = unsafe { call(ar) };
    // FArchive/FBufferReader Preload is inert; only package readers own loading.
    if context.archive.mode() != ArchiveMode::Loading { return 0; }
    context.record(unsafe { context.access.preload(object.cast()) })
}
