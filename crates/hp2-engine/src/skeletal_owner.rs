//! Canonical skeletal-mesh storage, wire data and reference ownership.

use crate::animation_owner::{destroy_array, serialize_array};
use crate::mesh_owner;
use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    self, FBox, FMeshBone, FMeshExtWedge, TArrayRaw, UObject, USkeletalMesh, VBoneInfIndex,
    VBoneInfluence,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::math;
use hp2_core::object_archive::{ArchiveMode, ObjectArchive};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, ffi::c_void, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const SKELETAL: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_skeletal_mesh;
    operations.finalize_members = finalize_skeletal_mesh;
    operations.serialize = serialize_skeletal_mesh;
    operations.visit_references = visit_skeletal_mesh;
    operations
};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "SkeletalMesh",
    package: "Engine",
    super_name: Some("LodMesh"),
    within: "Object",
    flags: 0,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<USkeletalMesh>()),
    operations: Some(SKELETAL),
    properties: &[],
    defaults: &[],
}];

/// # Safety
/// Core must select a live canonical SkeletalMesh allocation with the probed layout.
unsafe fn construct_skeletal_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    // SAFETY: the selected object contains the entire canonical LodMesh prefix.
    unsafe {
        mesh_owner::construct_lod_mesh(access, object)?;
    }
    let mesh = object.cast::<USkeletalMesh>();
    // SAFETY: these are this class's actual compiler-probed native members. Native
    // constructors initialize TArray headers, not the scalar fields copied from defaults.
    unsafe {
        for array in [
            ptr::addr_of_mut!((*mesh).ext_wedges),
            ptr::addr_of_mut!((*mesh).points),
            ptr::addr_of_mut!((*mesh).ref_skeleton),
            ptr::addr_of_mut!((*mesh).bone_weight_idx),
            ptr::addr_of_mut!((*mesh).bone_weights),
            ptr::addr_of_mut!((*mesh).local_points),
            ptr::addr_of_mut!((*mesh).debug_pivots),
            ptr::addr_of_mut!((*mesh).debug_parents),
            ptr::addr_of_mut!((*mesh).debug_verts),
        ] {
            array.write_unaligned(TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            });
        }
        ptr::addr_of_mut!((*mesh).weapon_adjust).write_unaligned(math::unit_coords());
    }
    Ok(())
}

/// # Safety
/// The object must be a live canonical SkeletalMesh, including partially loaded storage.
unsafe fn finalize_skeletal_mesh(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let mesh = object.cast::<USkeletalMesh>();
    let mut storage = Ok(());
    unsafe {
        for (array, width) in [
            (ptr::addr_of_mut!((*mesh).debug_verts), size_of::<u8>()),
            (ptr::addr_of_mut!((*mesh).debug_parents), size_of::<i32>()),
            (ptr::addr_of_mut!((*mesh).debug_pivots), size_of::<FVector>()),
            (ptr::addr_of_mut!((*mesh).local_points), size_of::<FVector>()),
            (ptr::addr_of_mut!((*mesh).bone_weights), size_of::<VBoneInfluence>()),
            (ptr::addr_of_mut!((*mesh).bone_weight_idx), size_of::<VBoneInfIndex>()),
            (ptr::addr_of_mut!((*mesh).ref_skeleton), size_of::<FMeshBone>()),
            (ptr::addr_of_mut!((*mesh).points), size_of::<FVector>()),
            (ptr::addr_of_mut!((*mesh).ext_wedges), size_of::<FMeshExtWedge>()),
        ] {
            storage = storage.and(destroy_array(array, width, None));
        }
        storage = storage.and(mesh_owner::destroy_lod_mesh_storage(access, object));
    }
    storage
}

/// # Safety
/// Core must select a live canonical SkeletalMesh and its active archive.
unsafe fn serialize_skeletal_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    // SAFETY: the inherited serializer also performs LodMesh's saving-time Tris clear.
    unsafe {
        mesh_owner::serialize_lod_mesh(access, object, archive)?;
    }
    let mesh = object.cast::<USkeletalMesh>();
    if matches!(
        archive.mode(),
        ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports
    ) {
        // SAFETY: traversal archives need references, not array wire counts or cursors.
        return unsafe { visit_fields(mesh, archive) };
    }
    // SAFETY: member locations and element layouts are compiler-probed. Native wire
    // order is independent of member order, notably DefaultAnimation/WeaponBoneIndex.
    unsafe {
        serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).ext_wedges),
            size_of::<FMeshExtWedge>(),
            Some(serialize_wedge),
            None,
            false,
        )?;
        serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).points),
            size_of::<FVector>(),
            Some(serialize_vector),
            None,
            generated::VECTOR_FAST_SERIALIZE != 0,
        )?;
        serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).ref_skeleton),
            size_of::<FMeshBone>(),
            Some(serialize_bone),
            None,
            false,
        )?;
        serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).bone_weight_idx),
            size_of::<VBoneInfIndex>(),
            Some(serialize_influence_index),
            None,
            false,
        )?;
        serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).bone_weights),
            size_of::<VBoneInfluence>(),
            Some(serialize_influence),
            None,
            false,
        )?;
        serialize_array(
            archive,
            ptr::addr_of_mut!((*mesh).local_points),
            size_of::<FVector>(),
            Some(serialize_vector),
            None,
            generated::VECTOR_FAST_SERIALIZE != 0,
        )?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mesh).skeletal_depth))?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*mesh).default_animation).cast())?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*mesh).weapon_bone_index))?;
        let coords = ptr::addr_of_mut!((*mesh).weapon_adjust);
        serialize_vector(archive, ptr::addr_of_mut!((*coords).origin).cast())?;
        serialize_vector(archive, ptr::addr_of_mut!((*coords).x_axis).cast())?;
        serialize_vector(archive, ptr::addr_of_mut!((*coords).y_axis).cast())?;
        serialize_vector(archive, ptr::addr_of_mut!((*coords).z_axis).cast())?;
        if archive.is_loading() {
            average_bounds(mesh)?;
        }
    }
    Ok(())
}

/// # Safety
/// `value` must point to one initialized FMeshExtWedge in canonical array storage.
unsafe fn serialize_wedge(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let wedge = value.cast::<FMeshExtWedge>();
    // SAFETY: all scalar fields are aligned at compiler-probed offsets.
    unsafe {
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*wedge).i_vertex))?;
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*wedge).flags))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*wedge).tex_uv.u))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*wedge).tex_uv.v))
    }
}

/// # Safety
/// `value` must point to one initialized FMeshBone in canonical array storage.
unsafe fn serialize_bone(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let bone = value.cast::<FMeshBone>();
    // SAFETY: canonical array allocation and the generated element stride preserve alignment.
    unsafe {
        archive.serialize_name_slot(ptr::addr_of_mut!((*bone).name).cast())?;
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*bone).flags))?;
        for component in 0..4 {
            archive.serialize_f32(
                &mut *ptr::addr_of_mut!((*bone).bone_pos.place.quat)
                    .cast::<f32>()
                    .add(component),
            )?;
        }
        serialize_vector(
            archive,
            ptr::addr_of_mut!((*bone).bone_pos.place.pos).cast(),
        )?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*bone).bone_pos.length))?;
        // UnSkeletalMesh.h's VJointPos operator deliberately serializes XSize twice,
        // omits YSize, and then writes ZSize. Depth is likewise not a wire field.
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*bone).bone_pos.x_size))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*bone).bone_pos.x_size))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*bone).bone_pos.z_size))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*bone).num_children))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*bone).parent_index))
    }
}

/// # Safety
/// `value` must point to one initialized VBoneInfIndex in canonical array storage.
unsafe fn serialize_influence_index(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    let index = value.cast::<VBoneInfIndex>();
    // SAFETY: four WORD members occupy the compiler-probed record.
    unsafe {
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*index).weight_index))?;
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*index).number))?;
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*index).detail_a))?;
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*index).detail_b))
    }
}

/// # Safety
/// `value` must point to one initialized VBoneInfluence in canonical array storage.
unsafe fn serialize_influence(
    archive: &ObjectArchive<'_>,
    value: *mut c_void,
) -> RuntimeResult<()> {
    let influence = value.cast::<VBoneInfluence>();
    // SAFETY: the canonical element contains both aligned WORD fields.
    unsafe {
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*influence).point_index))?;
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*influence).bone_weight))
    }
}

/// # Safety
/// `value` must point to an initialized FVector in canonical storage.
unsafe fn serialize_vector(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let vector = value.cast::<FVector>();
    // SAFETY: FVector contains three aligned scalar components.
    unsafe {
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*vector).x))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*vector).y))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*vector).z))
    }
}

/// # Safety
/// The canonical mesh must own valid BoundingBoxes array storage.
unsafe fn average_bounds(mesh: *mut USkeletalMesh) -> RuntimeResult<()> {
    // SAFETY: a live canonical mesh contains this initialized array header.
    let (boxes, count) = unsafe { array_parts::<FBox>(ptr::addr_of!((*mesh).bounding_boxes))? };
    if count == 0 {
        return Ok(());
    }
    // SAFETY: native FBox(0) zeroes coordinates and validity, even for this average.
    unsafe {
        ptr::addr_of_mut!((*mesh).bounding_box_sum).write_bytes(0, 1);
    }
    let mut min = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let mut max = min;
    for index in 0..count {
        // SAFETY: the checked header and compiler-probed stride bound the element.
        let bounds = unsafe { boxes.add(index).read_unaligned() };
        min = math::vector_add(min, bounds.min);
        max = math::vector_add(max, bounds.max);
    }
    // SAFETY: only these native members change; IsValid remains the native zero value.
    unsafe {
        ptr::addr_of_mut!((*mesh).bounding_box_sum.min)
            .write_unaligned(math::vector_div(min, count as f32));
        ptr::addr_of_mut!((*mesh).bounding_box_sum.max)
            .write_unaligned(math::vector_div(max, count as f32));
    }
    Ok(())
}

/// # Safety
/// `raw` must be an initialized array header owned by the selected canonical object.
unsafe fn array_parts<T>(raw: *const TArrayRaw) -> RuntimeResult<(*mut T, usize)> {
    // SAFETY: the caller supplies the selected member header, possibly packed.
    let raw = unsafe { raw.read_unaligned() };
    let count =
        usize::try_from(raw.num).map_err(|_| RuntimeError::new("Negative skeletal array count"))?;
    if count > 0 && raw.data.is_null() {
        return Err(RuntimeError::new("Nonempty skeletal array has no storage"));
    }
    Ok((raw.data.cast(), count))
}

/// # Safety
/// Core must select a live canonical SkeletalMesh for reference traversal.
unsafe fn visit_skeletal_mesh(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    // SAFETY: inherited traversal and own fields use the same live allocation.
    unsafe {
        mesh_owner::visit_lod_mesh(access, object, archive)?;
        visit_fields(object.cast(), archive)
    }
}

/// # Safety
/// The canonical mesh's reference skeleton and animation slot must be initialized.
unsafe fn visit_fields(mesh: *mut USkeletalMesh, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    // SAFETY: the actual member header bounds each native FMeshBone name slot.
    unsafe {
        let (bones, count) = array_parts::<FMeshBone>(ptr::addr_of!((*mesh).ref_skeleton))?;
        for index in 0..count {
            archive.serialize_name_slot(ptr::addr_of_mut!((*bones.add(index)).name).cast())?;
        }
        archive.serialize_object_slot(ptr::addr_of_mut!((*mesh).default_animation).cast())
    }
}
