//! Canonical Engine class operations for mesh and LOD-mesh object graphs.

use crate::animation_owner;
use crate::lod_mesh_archive::{
    self, Hp2LodArchiveAction as LodAction, Hp2LodArchiveLeaf, Hp2LodArchiveState,
};
use hp2_abi::engine::generated::{
    self, FMeshAnimNotify, FMeshAnimSeq, FMeshFace, FMeshMaterial, FMeshTri, FMeshUV, FMeshVert,
    FMeshVertConnect, FMeshWedge, TArrayRaw, ULodMesh, UMesh, UObject, UPrimitive,
};
use hp2_core::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::lazy_array::{self, LazyArraySlots};
use hp2_core::object_archive::{ArchiveMode, ObjectArchive};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, cell::RefCell, ffi::c_void, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const ARRAYS: Hp2ArrayServices = array_storage::system_allocator();

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    spec("Mesh", "BoxPrim", Layout::new::<UMesh>(), MESH),
    spec("LodMesh", "Mesh", Layout::new::<ULodMesh>(), LOD_MESH),
];

const fn spec(
    name: &'static str,
    superclass: &'static str,
    layout: Layout,
    operations: ClassOps,
) -> NativeClassSpec {
    NativeClassSpec {
        short_name: name,
        package: "Engine",
        super_name: Some(superclass),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(layout),
        operations: Some(operations),
        properties: &[],
        defaults: &[],
    }
}

const MESH: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_mesh;
    operations.finalize_members = finalize_mesh;
    operations.serialize = serialize_mesh;
    operations.visit_references = visit_mesh;
    operations
};

const LOD_MESH: ClassOps = {
    let mut operations = MESH;
    operations.construct = construct_lod_mesh;
    operations.finalize_members = finalize_lod_mesh;
    operations.serialize = serialize_lod_mesh;
    operations.visit_references = visit_lod_mesh;
    operations
};

fn error(message: &'static str) -> RuntimeError {
    RuntimeError::new(message)
}

fn storage_error(_: i32, message: &'static str) -> RuntimeError {
    error(message)
}

fn combine(first: RuntimeResult<()>, next: RuntimeResult<()>) -> RuntimeResult<()> {
    match (first, next) {
        (Err(error), _) => Err(error),
        (Ok(()), result) => result,
    }
}

unsafe fn array_slots(raw: *mut TArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*raw).data) },
        num: unsafe { ptr::addr_of_mut!((*raw).num) },
        max: unsafe { ptr::addr_of_mut!((*raw).max) },
    }
}

unsafe fn initialize_array(raw: *mut TArrayRaw) {
    unsafe {
        raw.write_unaligned(TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        });
    }
}

unsafe fn lazy_slots(
    lazy: *mut c_void,
    array_offset: usize,
    loader_offset: usize,
    saved_archive: *mut *mut c_void,
    saved_position: *mut i32,
) -> LazyArraySlots {
    let bytes = lazy.cast::<u8>();
    unsafe {
        LazyArraySlots {
            array: bytes.add(array_offset).cast::<TArrayRaw>(),
            loader: bytes.add(loader_offset).cast(),
            saved_archive,
            saved_position,
        }
    }
}

unsafe fn verts_slots(mesh: *mut UMesh) -> LazyArraySlots {
    let lazy = unsafe { ptr::addr_of_mut!((*mesh).verts) };
    unsafe {
        lazy_slots(
            lazy.cast(),
            generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET,
            generated::TLazyMeshVerts::FLAZYLOADER_OFFSET,
            ptr::addr_of_mut!((*lazy).saved_ar),
            ptr::addr_of_mut!((*lazy).saved_pos),
        )
    }
}

unsafe fn tris_slots(mesh: *mut UMesh) -> LazyArraySlots {
    let lazy = unsafe { ptr::addr_of_mut!((*mesh).tris) };
    unsafe {
        lazy_slots(
            lazy.cast(),
            generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET,
            generated::TLazyMeshTris::FLAZYLOADER_OFFSET,
            ptr::addr_of_mut!((*lazy).saved_ar),
            ptr::addr_of_mut!((*lazy).saved_pos),
        )
    }
}

unsafe fn connects_slots(mesh: *mut UMesh) -> LazyArraySlots {
    let lazy = unsafe { ptr::addr_of_mut!((*mesh).connects) };
    unsafe {
        lazy_slots(
            lazy.cast(),
            generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET,
            generated::TLazyMeshConnects::FLAZYLOADER_OFFSET,
            ptr::addr_of_mut!((*lazy).saved_ar),
            ptr::addr_of_mut!((*lazy).saved_pos),
        )
    }
}

unsafe fn vert_links_slots(mesh: *mut UMesh) -> LazyArraySlots {
    let lazy = unsafe { ptr::addr_of_mut!((*mesh).vert_links) };
    unsafe {
        lazy_slots(
            lazy.cast(),
            generated::TLazyMeshLinks::TARRAY_INT_OFFSET,
            generated::TLazyMeshLinks::FLAZYLOADER_OFFSET,
            ptr::addr_of_mut!((*lazy).saved_ar),
            ptr::addr_of_mut!((*lazy).saved_pos),
        )
    }
}

unsafe fn initialize_lazy(access: CoreAccess<'_>, slots: LazyArraySlots) -> RuntimeResult<()> {
    unsafe { lazy_array::lazy_array_initialize(access, slots) }
}

/// Constructs the actual `UBoxPrim` superclass and initializes UMesh's empty storage.
///
/// # Safety
/// `object` must be a live canonical UMesh or subclass allocation with compiler-probed layout.
pub(crate) unsafe fn construct_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    unsafe {
        (crate::primitive_owner::NATIVE_CLASSES[2]
            .operations
            .expect("BoxPrim has registered ClassOps")
            .construct)(access, object)?;
    }
    let mesh = object.cast::<UMesh>();
    unsafe {
        initialize_lazy(access, verts_slots(mesh))?;
        initialize_lazy(access, tris_slots(mesh))?;
        initialize_array(ptr::addr_of_mut!((*mesh).anim_seqs));
        initialize_lazy(access, connects_slots(mesh))?;
        initialize_array(ptr::addr_of_mut!((*mesh).bounding_boxes));
        initialize_array(ptr::addr_of_mut!((*mesh).bounding_spheres));
        initialize_lazy(access, vert_links_slots(mesh))?;
        initialize_array(ptr::addr_of_mut!((*mesh).textures));
        initialize_array(ptr::addr_of_mut!((*mesh).texture_lod));
        ptr::addr_of_mut!((*mesh).and_flags).write_unaligned(u32::MAX);
        ptr::addr_of_mut!((*mesh).or_flags).write_unaligned(0);
        ptr::addr_of_mut!((*mesh).scale).write_unaligned(hp2_abi::FVector {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        });
        ptr::addr_of_mut!((*mesh).origin).write_unaligned(hp2_abi::FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
        ptr::addr_of_mut!((*mesh).rot_origin).write_unaligned(generated::FRotator {
            pitch: 0,
            yaw: 0,
            roll: 0,
        });
    }
    Ok(())
}

/// Constructs UMesh first and initializes ULodMesh's declared empty arrays.
///
/// The inline native `ULodMesh()` is empty; the nonzero LOD defaults belong to
/// its separate count-taking constructor and are not applied to default objects.
///
/// # Safety
/// `object` must be a live canonical ULodMesh allocation with compiler-probed layout.
pub(crate) unsafe fn construct_lod_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    unsafe {
        construct_mesh(access, object)?;
    }
    let lod = object.cast::<ULodMesh>();
    unsafe {
        initialize_array(ptr::addr_of_mut!((*lod).collapse_point_thus));
        initialize_array(ptr::addr_of_mut!((*lod).face_level));
        initialize_array(ptr::addr_of_mut!((*lod).faces));
        initialize_array(ptr::addr_of_mut!((*lod).collapse_wedge_thus));
        initialize_array(ptr::addr_of_mut!((*lod).wedges));
        initialize_array(ptr::addr_of_mut!((*lod).materials));
        initialize_array(ptr::addr_of_mut!((*lod).special_faces));
        initialize_array(ptr::addr_of_mut!((*lod).remap_anim_verts));
    }
    Ok(())
}


/// Releases UMesh-owned storage during final member destruction.
///
/// Derived finalizers release their fields before this inherited storage.
///
/// # Safety
/// `object` must be a live canonical UMesh or subclass allocation being finalized.
pub(crate) unsafe fn destroy_mesh_storage(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    let mesh = object.cast::<UMesh>();
    let mut cleanup = Ok(());
    unsafe {
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*mesh).texture_lod),
                size_of::<f32>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*mesh).textures),
                size_of::<*mut UObject>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            destroy_lazy(access, vert_links_slots(mesh), size_of::<i32>()),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*mesh).bounding_spheres).cast(),
                4 * size_of::<f32>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*mesh).bounding_boxes).cast(),
                size_of::<generated::FBox>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            destroy_lazy(access, connects_slots(mesh), size_of::<FMeshVertConnect>()),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*mesh).anim_seqs),
                size_of::<FMeshAnimSeq>(),
                Some(animation_owner::destroy_sequence),
            ),
        );
        cleanup = combine(
            cleanup,
            destroy_lazy(access, tris_slots(mesh), size_of::<FMeshTri>()),
        );
        cleanup = combine(
            cleanup,
            destroy_lazy(access, verts_slots(mesh), size_of::<FMeshVert>()),
        );
    }
    cleanup
}

unsafe fn finalize_mesh(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { destroy_mesh_storage(access, object) }
}

unsafe fn destroy_lazy(
    access: CoreAccess<'_>,
    slots: LazyArraySlots,
    width: usize,
) -> RuntimeResult<()> {
    let destroy = unsafe { lazy_array::lazy_array_destroy(access, slots) };
    let release = unsafe { animation_owner::destroy_array(slots.array, width, None) };
    combine(destroy, release)
}

/// Releases ULodMesh-owned storage followed by inherited UMesh storage.
///
/// Does not run UObject destruction; derived finalizers call this after their
/// own member cleanup.
///
/// # Safety
/// `object` must be a live canonical ULodMesh allocation being finalized.
pub(crate) unsafe fn destroy_lod_mesh_storage(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    let lod = object.cast::<ULodMesh>();
    let mut cleanup = Ok(());
    unsafe {
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).remap_anim_verts),
                size_of::<u16>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).special_faces),
                size_of::<FMeshFace>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).materials),
                size_of::<FMeshMaterial>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).wedges),
                size_of::<FMeshWedge>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).collapse_wedge_thus),
                size_of::<u16>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).faces),
                size_of::<FMeshFace>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).face_level),
                size_of::<u16>(),
                None,
            ),
        );
        cleanup = combine(
            cleanup,
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).collapse_point_thus),
                size_of::<u16>(),
                None,
            ),
        );
    }
    let inherited = unsafe { destroy_mesh_storage(access, object) };
    combine(cleanup, inherited)
}

unsafe fn finalize_lod_mesh(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { destroy_lod_mesh_storage(access, object) }
}

/// Serializes the UBoxPrim and UMesh fields in native order.
///
/// # Safety
/// `object` must be a live canonical UMesh or subclass allocation using this owner's storage.
pub(crate) unsafe fn serialize_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        crate::primitive_owner::serialize_box_primitive(access, object, archive)?;
    }
    if is_reference_mode(archive) {
        return unsafe { visit_mesh_payload(object.cast(), archive) };
    }
    let mesh = object.cast::<UMesh>();
    unsafe {
        serialize_lazy(access, object, mesh, archive, LazyField::Verts)?;
        serialize_lazy(access, object, mesh, archive, LazyField::Tris)?;
        validate_first_triangle(access, mesh)?;
        animation_owner::serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).anim_seqs),
            size_of::<FMeshAnimSeq>(),
            Some(animation_owner::serialize_sequence),
            Some(animation_owner::destroy_sequence),
            false,
        )?;
        serialize_lazy(access, object, mesh, archive, LazyField::Connects)?;
        serialize_box(
            archive,
            ptr::addr_of_mut!((*object.cast::<UPrimitive>()).bounding_box),
        )?;
        serialize_sphere(
            archive,
            ptr::addr_of_mut!((*object.cast::<UPrimitive>()).bounding_sphere).cast(),
        )?;
        serialize_lazy(access, object, mesh, archive, LazyField::VertLinks)?;
        animation_owner::serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).textures),
            size_of::<*mut UObject>(),
            Some(serialize_texture),
            None,
            false,
        )?;
        animation_owner::serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).bounding_boxes),
            size_of::<generated::FBox>(),
            Some(serialize_box_item),
            None,
            false,
        )?;
        animation_owner::serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).bounding_spheres),
            4 * size_of::<f32>(),
            Some(serialize_sphere_item),
            None,
            false,
        )?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mesh).frame_verts))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mesh).anim_frames))?;
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*mesh).and_flags))?;
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*mesh).or_flags))?;
        serialize_vector(archive, ptr::addr_of_mut!((*mesh).scale))?;
        serialize_vector(archive, ptr::addr_of_mut!((*mesh).origin))?;
        serialize_rotator(archive, ptr::addr_of_mut!((*mesh).rot_origin))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mesh).cur_poly))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mesh).cur_vertex))?;
        if archive.flags().version == 65 {
            // Native code serializes an unused, uninitialized FLOAT here; zero
            // is the deterministic filler while loading still consumes the field.
            let mut unused = 0.0;
            archive.serialize_f32(&mut unused)?;
        }
        if archive.flags().version >= 66 {
            animation_owner::serialize_array(
                archive,
                ptr::addr_of_mut!((*mesh).texture_lod),
                size_of::<f32>(),
                Some(serialize_f32),
                None,
                generated::SCALAR_FAST_SERIALIZE != 0,
            )?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum LazyField {
    Verts,
    Tris,
    Connects,
    VertLinks,
}

unsafe fn serialize_lazy(
    access: CoreAccess<'_>,
    owner: *mut UObject,
    mesh: *mut UMesh,
    archive: &ObjectArchive<'_>,
    field: LazyField,
) -> RuntimeResult<()> {
    let (slots, width, body) = unsafe {
        match field {
            LazyField::Verts => (
                verts_slots(mesh),
                size_of::<FMeshVert>(),
                serialize_verts_body as lazy_array::LazyArrayBody,
            ),
            LazyField::Tris => (
                tris_slots(mesh),
                size_of::<FMeshTri>(),
                serialize_tris_body as lazy_array::LazyArrayBody,
            ),
            LazyField::Connects => (
                connects_slots(mesh),
                size_of::<FMeshVertConnect>(),
                serialize_connects_body as lazy_array::LazyArrayBody,
            ),
            LazyField::VertLinks => (
                vert_links_slots(mesh),
                size_of::<i32>(),
                serialize_links_body as lazy_array::LazyArrayBody,
            ),
        }
    };
    unsafe { lazy_array::lazy_array_serialize(access, owner, slots, archive, width, body) }
}

unsafe fn serialize_verts_body(
    _: CoreAccess<'_>,
    array: *mut c_void,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        animation_owner::serialize_array(
            archive,
            array.cast(),
            size_of::<FMeshVert>(),
            Some(serialize_mesh_vert),
            None,
            false,
        )
    }
}
unsafe fn serialize_tris_body(
    _: CoreAccess<'_>,
    array: *mut c_void,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        animation_owner::serialize_array(
            archive,
            array.cast(),
            size_of::<FMeshTri>(),
            Some(serialize_mesh_tri),
            None,
            false,
        )
    }
}
unsafe fn serialize_connects_body(
    _: CoreAccess<'_>,
    array: *mut c_void,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        animation_owner::serialize_array(
            archive,
            array.cast(),
            size_of::<FMeshVertConnect>(),
            Some(serialize_vert_connect),
            None,
            false,
        )
    }
}
unsafe fn serialize_links_body(
    _: CoreAccess<'_>,
    array: *mut c_void,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        animation_owner::serialize_array(
            archive,
            array.cast(),
            size_of::<i32>(),
            Some(serialize_i32),
            None,
            generated::SCALAR_FAST_SERIALIZE != 0,
        )
    }
}

fn is_reference_mode(archive: &ObjectArchive<'_>) -> bool {
    matches!(
        archive.mode(),
        ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports
    )
}

unsafe fn visit_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.visit_references)(access, object, archive)?;
    }
    unsafe { visit_mesh_payload(object.cast(), archive) }
}

pub(crate) unsafe fn visit_lod_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { visit_mesh(access, object, archive) }
}

unsafe fn visit_mesh_payload(mesh: *mut UMesh, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe {
        let (textures, texture_count) =
            array_parts::<*mut UObject>(ptr::addr_of!((*mesh).textures).cast())?;
        for index in 0..texture_count {
            archive.serialize_object_slot(textures.add(index))?;
        }
        let (sequences, sequence_count) =
            array_parts::<FMeshAnimSeq>(ptr::addr_of!((*mesh).anim_seqs).cast())?;
        for index in 0..sequence_count {
            let sequence = sequences.add(index);
            archive.serialize_name_slot(ptr::addr_of_mut!((*sequence).name).cast())?;
            archive.serialize_name_slot(ptr::addr_of_mut!((*sequence).group).cast())?;
            let (notifies, notify_count) =
                array_parts::<FMeshAnimNotify>(ptr::addr_of!((*sequence).notifys).cast())?;
            for notify_index in 0..notify_count {
                let notify = notifies.add(notify_index);
                archive.serialize_name_slot(ptr::addr_of_mut!((*notify).function).cast())?;
            }
        }
    }
    Ok(())
}

unsafe fn array_parts<T>(raw: *const TArrayRaw) -> RuntimeResult<(*mut T, usize)> {
    let raw = unsafe { ptr::read_unaligned(raw) };
    if raw.num < 0 || (raw.num > 0 && raw.data.is_null()) {
        return Err(error("Invalid mesh array storage"));
    }
    Ok((raw.data.cast(), raw.num as usize))
}

unsafe fn validate_first_triangle(access: CoreAccess<'_>, mesh: *mut UMesh) -> RuntimeResult<()> {
    unsafe {
        let slots = tris_slots(mesh);
        let raw = ptr::read_unaligned(slots.array);
        if raw.num < 0 {
            return Err(error("Invalid mesh triangle storage"));
        }
        if raw.num != 0 {
            lazy_array::lazy_array_load(access, slots)?;
            let (tris, count) = array_parts::<FMeshTri>(slots.array)?;
            let verts = ptr::read_unaligned(verts_slots(mesh).array);
            if verts.num < 0 {
                return Err(error("Invalid mesh vertex storage"));
            }
            if count != 0 && i32::from((*tris).i_vertex[0]) >= verts.num {
                return Err(error(
                    "Mesh triangle references a vertex outside its vertex array",
                ));
            }
        }
    }
    Ok(())
}

unsafe fn serialize_mesh_vert(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { archive.serialize_u32(&mut *value.cast::<u32>()) }
}

unsafe fn serialize_mesh_tri(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let tri = value.cast::<FMeshTri>();
    unsafe {
        for index in 0..3 {
            archive.serialize_u16(&mut *ptr::addr_of_mut!((*tri).i_vertex[index]))?;
        }
        for index in 0..3 {
            serialize_uv(archive, ptr::addr_of_mut!((*tri).tex[index]))?;
        }
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*tri).poly_flags))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*tri).texture_index))
    }
}

unsafe fn serialize_uv(archive: &ObjectArchive<'_>, uv: *mut FMeshUV) -> RuntimeResult<()> {
    unsafe {
        archive.serialize_u8(&mut *ptr::addr_of_mut!((*uv).u))?;
        archive.serialize_u8(&mut *ptr::addr_of_mut!((*uv).v))
    }
}

unsafe fn serialize_vert_connect(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    let connect = value.cast::<FMeshVertConnect>();
    unsafe {
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*connect).num_vert_triangles))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*connect).triangle_list_offset))
    }
}

unsafe fn serialize_i32(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { archive.serialize_i32(&mut *value.cast::<i32>()) }
}
unsafe fn serialize_f32(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { archive.serialize_f32(&mut *value.cast::<f32>()) }
}
unsafe fn serialize_texture(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { archive.serialize_object_slot(value.cast::<*mut UObject>()) }
}

unsafe fn serialize_box(
    archive: &ObjectArchive<'_>,
    value: *mut generated::FBox,
) -> RuntimeResult<()> {
    unsafe {
        serialize_vector(archive, ptr::addr_of_mut!((*value).min))?;
        serialize_vector(archive, ptr::addr_of_mut!((*value).max))?;
        archive.serialize_u8(&mut *ptr::addr_of_mut!((*value).is_valid))
    }
}
unsafe fn serialize_box_item(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { serialize_box(archive, value.cast()) }
}
unsafe fn serialize_sphere(archive: &ObjectArchive<'_>, value: *mut f32) -> RuntimeResult<()> {
    let components = if archive.flags().version <= 61 { 3 } else { 4 };
    for index in 0..components {
        unsafe {
            archive.serialize_f32(&mut *value.add(index))?;
        }
    }
    Ok(())
}
unsafe fn serialize_sphere_item(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { serialize_sphere(archive, value.cast()) }
}
unsafe fn serialize_vector(
    archive: &ObjectArchive<'_>,
    value: *mut hp2_abi::FVector,
) -> RuntimeResult<()> {
    let components = value.cast::<f32>();
    for index in 0..3 {
        unsafe {
            archive.serialize_f32(&mut *components.add(index))?;
        }
    }
    Ok(())
}
unsafe fn serialize_rotator(
    archive: &ObjectArchive<'_>,
    value: *mut generated::FRotator,
) -> RuntimeResult<()> {
    unsafe {
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*value).pitch))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*value).yaw))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*value).roll))
    }
}

/// Serializes the LOD fields and delegates remap policy to its existing typed Rust kernel.
///
/// # Safety
/// `object` must be a live canonical ULodMesh allocation using this owner's storage.
pub(crate) unsafe fn serialize_lod_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    if is_reference_mode(archive) {
        return unsafe { serialize_mesh(access, object, archive) };
    }
    let mut context = LodArchiveContext {
        access,
        object,
        archive,
        new_vertices: TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        },
        error: RefCell::new(None),
    };
    let status = unsafe {
        lod_mesh_archive::serialize(
            (&raw mut context).cast(),
            lod_archive_leaf as Hp2LodArchiveLeaf,
        )
    };
    let cleanup = unsafe {
        animation_owner::destroy_array(
            ptr::addr_of_mut!(context.new_vertices),
            size_of::<FMeshVert>(),
            None,
        )
    };
    let result = match context.error.into_inner() {
        Some(error) => Err(error),
        None if status == 0 => Ok(()),
        None => Err(error("LOD mesh archive policy failed")),
    };
    combine(result, cleanup)
}

struct LodArchiveContext<'owner, 'borrow, 'data> {
    access: CoreAccess<'owner>,
    object: *mut UObject,
    archive: &'borrow ObjectArchive<'data>,
    new_vertices: TArrayRaw,
    error: RefCell<Option<RuntimeError>>,
}

unsafe extern "C" fn lod_archive_leaf(
    context: *mut c_void,
    action: LodAction,
    arg: i32,
    state: *mut Hp2LodArchiveState,
) -> i32 {
    let context = unsafe { &mut *context.cast::<LodArchiveContext<'_, '_, '_>>() };
    let result = unsafe { lod_archive_action(context, action, arg, state) };
    match result {
        Ok(()) => 0,
        Err(error) => {
            context.error.borrow_mut().get_or_insert(error);
            1
        }
    }
}

unsafe fn lod_archive_action(
    context: &mut LodArchiveContext<'_, '_, '_>,
    action: LodAction,
    arg: i32,
    state: *mut Hp2LodArchiveState,
) -> RuntimeResult<()> {
    use LodAction::*;
    let lod = context.object.cast::<ULodMesh>();
    let mesh = context.object.cast::<UMesh>();
    let state = unsafe { &mut *state };
    match action {
        Saving => state.saving = i32::from(context.archive.is_saving()),
        EmptyTris => {
            let slots = unsafe { tris_slots(mesh) };
            unsafe { array_storage::release(array_slots(slots.array), ARRAYS) }
                .map_err(|status| storage_error(status, "Mesh triangle release failed"))?;
        }
        Super => unsafe {
            serialize_mesh(context.access, context.object, context.archive)?;
        },
        CollapsePointThus => unsafe {
            serialize_word_array(
                context.archive,
                ptr::addr_of_mut!((*lod).collapse_point_thus),
            )?;
        },
        FaceLevel => unsafe {
            serialize_word_array(context.archive, ptr::addr_of_mut!((*lod).face_level))?;
        },
        Faces => unsafe {
            animation_owner::serialize_array(
                context.archive,
                ptr::addr_of_mut!((*lod).faces),
                size_of::<FMeshFace>(),
                Some(serialize_mesh_face),
                None,
                false,
            )?;
        },
        CollapseWedgeThus => unsafe {
            serialize_word_array(
                context.archive,
                ptr::addr_of_mut!((*lod).collapse_wedge_thus),
            )?;
        },
        Wedges => unsafe {
            animation_owner::serialize_array(
                context.archive,
                ptr::addr_of_mut!((*lod).wedges),
                size_of::<FMeshWedge>(),
                Some(serialize_mesh_wedge),
                None,
                false,
            )?;
        },
        Materials => unsafe {
            animation_owner::serialize_array(
                context.archive,
                ptr::addr_of_mut!((*lod).materials),
                size_of::<FMeshMaterial>(),
                Some(serialize_mesh_material),
                None,
                false,
            )?;
        },
        SpecialFaces => unsafe {
            animation_owner::serialize_array(
                context.archive,
                ptr::addr_of_mut!((*lod).special_faces),
                size_of::<FMeshFace>(),
                Some(serialize_mesh_face),
                None,
                false,
            )?;
        },
        ModelVerts => unsafe {
            context
                .archive
                .serialize_i32(&mut *ptr::addr_of_mut!((*lod).model_verts))?;
        },
        SpecialVerts => unsafe {
            context
                .archive
                .serialize_i32(&mut *ptr::addr_of_mut!((*lod).special_verts))?;
        },
        MeshScaleMax => unsafe {
            context
                .archive
                .serialize_f32(&mut *ptr::addr_of_mut!((*lod).mesh_scale_max))?;
        },
        LODHysteresis => unsafe {
            context
                .archive
                .serialize_f32(&mut *ptr::addr_of_mut!((*lod).lod_hysteresis))?;
        },
        LODStrength => unsafe {
            context
                .archive
                .serialize_f32(&mut *ptr::addr_of_mut!((*lod).lod_strength))?;
        },
        LODMinVerts => unsafe {
            context
                .archive
                .serialize_i32(&mut *ptr::addr_of_mut!((*lod).lod_min_verts))?;
        },
        LODMorph => unsafe {
            context
                .archive
                .serialize_f32(&mut *ptr::addr_of_mut!((*lod).lod_morph))?;
        },
        LODZDisplace => unsafe {
            context
                .archive
                .serialize_f32(&mut *ptr::addr_of_mut!((*lod).lodz_displace))?;
        },
        RemapAnimVerts => unsafe {
            serialize_word_array(context.archive, ptr::addr_of_mut!((*lod).remap_anim_verts))?;
        },
        OldFrameVerts => unsafe {
            context
                .archive
                .serialize_i32(&mut *ptr::addr_of_mut!((*lod).old_frame_verts))?;
        },
        LoadingState => {
            state.loading = i32::from(context.archive.is_loading());
            state.frame_verts = unsafe { (*mesh).frame_verts };
            state.anim_frames = unsafe { (*mesh).anim_frames };
            state.old_frame_verts = unsafe { (*lod).old_frame_verts };
            state.remap_count =
                unsafe { ptr::read_unaligned(ptr::addr_of!((*lod).remap_anim_verts.num)) };
            state.lod_min_verts = unsafe { ptr::addr_of_mut!((*lod).lod_min_verts) };
        }
        LoadVertices => {
            unsafe {
                lazy_array::lazy_array_load(context.access, verts_slots(mesh))?;
            }
            let (vertices, count) = unsafe { array_parts::<FMeshVert>(verts_slots(mesh).array)? };
            let (remap, _) =
                unsafe { array_parts::<u16>(ptr::addr_of!((*lod).remap_anim_verts).cast())? };
            state.vertices = vertices.cast();
            state.vertex_count =
                i32::try_from(count).map_err(|_| error("Mesh vertex count exceeds INT"))?;
            state.remap = remap;
        }
        AllocateVertices => {
            let slots = unsafe { array_slots(ptr::addr_of_mut!(context.new_vertices)) };
            let width = i32::try_from(size_of::<FMeshVert>())
                .map_err(|_| error("Mesh vertex width exceeds INT"))?;
            unsafe { array_storage::set_exact(slots, arg, width, ptr::null(), ARRAYS) }
                .map_err(|status| storage_error(status, "LOD remapped-vertex allocation failed"))?;
            let data = unsafe { slots.data.read_unaligned() };
            if arg > 0 && data.is_null() {
                return Err(error("LOD remapped-vertex allocation returned null"));
            }
            state.destination = data.cast();
        }
        EmptyVertices => unsafe {
            animation_owner::destroy_array(verts_slots(mesh).array, size_of::<FMeshVert>(), None)?;
        },
        ExchangeVertices => unsafe {
            ptr::swap(
                verts_slots(mesh).array,
                ptr::addr_of_mut!(context.new_vertices),
            );
        },
        ReleaseVertices => unsafe {
            animation_owner::destroy_array(
                ptr::addr_of_mut!(context.new_vertices),
                size_of::<FMeshVert>(),
                None,
            )?;
        },
        ReleaseRemap => unsafe {
            animation_owner::destroy_array(
                ptr::addr_of_mut!((*lod).remap_anim_verts),
                size_of::<u16>(),
                None,
            )?;
        },
        InvalidDimensions => return Err(error("LOD mesh animation dimensions are invalid")),
        InvalidRemapIndex => return Err(error("LOD mesh vertex remap index is outside its array")),
        InvalidVertexIndex => {
            return Err(error(
                "LOD mesh vertex remap references outside vertex storage",
            ));
        }
    }
    Ok(())
}

unsafe fn serialize_word_array(
    archive: &ObjectArchive<'_>,
    raw: *mut TArrayRaw,
) -> RuntimeResult<()> {
    unsafe {
        animation_owner::serialize_array(
            archive,
            raw,
            size_of::<u16>(),
            Some(serialize_u16),
            None,
            generated::SCALAR_FAST_SERIALIZE != 0,
        )
    }
}
unsafe fn serialize_u16(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { archive.serialize_u16(&mut *value.cast::<u16>()) }
}
unsafe fn serialize_mesh_face(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    let face = value.cast::<FMeshFace>();
    unsafe {
        for index in 0..3 {
            archive.serialize_u16(&mut *ptr::addr_of_mut!((*face).i_wedge[index]))?;
        }
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*face).material_index))
    }
}
unsafe fn serialize_mesh_wedge(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    let wedge = value.cast::<FMeshWedge>();
    unsafe {
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*wedge).i_vertex))?;
        serialize_uv(archive, ptr::addr_of_mut!((*wedge).tex_uv))
    }
}
unsafe fn serialize_mesh_material(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    let material = value.cast::<FMeshMaterial>();
    unsafe {
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*material).poly_flags))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*material).texture_index))
    }
}
