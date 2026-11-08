//! Canonical Rust owners for Engine's primitive geometry classes.

use hp2_abi::engine::generated::{FBox, UBoxPrim, UObject, UPrimitive};
use hp2_core::core_access::{CoreAccess, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_archive::ObjectArchive;
use hp2_core::object_owner::ClassOps;
use std::alloc::Layout;
use std::ptr;

const OPERATIONS: ClassOps = NativeRecordKind::Object.operations();

/// Canonical Engine primitive class declarations, ordered superclass first.
pub static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Primitive",
        package: "Engine",
        super_name: Some("Object"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UPrimitive>()),
        operations: Some(PRIMITIVE_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "Box",
        package: "Engine",
        super_name: Some("Primitive"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<hp2_abi::engine::generated::UBox>()),
        operations: Some(BOX_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "BoxPrim",
        package: "Engine",
        super_name: Some("Box"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UBoxPrim>()),
        operations: Some(BOX_PRIM_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
];

const PRIMITIVE_OPERATIONS: ClassOps = {
    let mut operations = OPERATIONS;
    operations.construct = construct_primitive;
    operations.serialize = serialize_primitive;
    operations
};

const BOX_OPERATIONS: ClassOps = {
    let mut operations = PRIMITIVE_OPERATIONS;
    operations.serialize = serialize_box;
    operations
};

const BOX_PRIM_OPERATIONS: ClassOps = {
    let mut operations = BOX_OPERATIONS;
    operations.serialize = serialize_box_primitive;
    operations
};

/// Runs the native primitive constructor's initialization in canonical storage.
///
/// # Safety
/// `object` must be the live canonical primitive selected by Core construction.
pub(crate) unsafe fn construct_primitive(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: Core construction selected a live object; this is its ordinary UObject constructor.
    unsafe { (OPERATIONS.construct)(access, object)?; }
    let primitive = object.cast::<UPrimitive>();
    // SAFETY: the selected allocation uses the compiler-probed UPrimitive layout.
    unsafe {
        ptr::write_bytes(ptr::addr_of_mut!((*primitive).bounding_box), 0, 1);
        ptr::write_bytes(ptr::addr_of_mut!((*primitive).bounding_sphere), 0, 1);
    }
    Ok(())
}

/// Serializes the UObject fields followed by the primitive's native geometry fields.
///
/// # Safety
/// `object` must be a live canonical primitive record whose storage uses the probed layout.
pub(crate) unsafe fn serialize_primitive(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    // SAFETY: Core selected the live object and Object operations serialize its valid prefix.
    unsafe { (OPERATIONS.serialize)(access, object, archive)?; }
    let primitive = object.cast::<UPrimitive>();
    // SAFETY: layout is compiler-probed and the pointer is the live selected primitive.
    let bounds = unsafe { ptr::addr_of_mut!((*primitive).bounding_box) };
    // SAFETY: bounds points to the live embedded FBox.
    unsafe { serialize_box_value(archive, bounds)?; }
    // SAFETY: bounding sphere is a 16-byte compiler-probed field with four f32 components.
    let sphere = unsafe { ptr::addr_of_mut!((*primitive).bounding_sphere).cast::<f32>() };
    for component in 0..if archive.flags().version <= 61 { 3 } else { 4 } {
        // SAFETY: the native wire requires three or four in-bounds sphere components.
        unsafe { archive.serialize_f32(&mut *sphere.add(component))?; }
    }
    Ok(())
}


/// Delegates UBox serialization to its actual UPrimitive superclass.
///
/// # Safety
/// `object` must be a live canonical UBox record with the probed layout.
unsafe fn serialize_box(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    // SAFETY: this uses the inherited UPrimitive portion of the same canonical allocation.
    unsafe { serialize_primitive(access, object, archive) }
}

/// Serializes UBoxPrim and all native superclasses through the direct archive.
///
/// # Safety
/// `object` must be the live canonical UBoxPrim selected by Core dispatch.
pub(crate) unsafe fn serialize_box_primitive(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { serialize_box(access, object, archive) }
}

/// Serializes the native FBox fields in their declared archive order.
///
/// # Safety
/// `bounds` must point to an initialized FBox in live canonical storage.
unsafe fn serialize_box_value(archive: &ObjectArchive<'_>, bounds: *mut FBox) -> RuntimeResult<()> {
    // SAFETY: the caller provides the live probed FBox layout.
    let min = unsafe { ptr::addr_of_mut!((*bounds).min).cast::<f32>() };
    // SAFETY: both FVector fields occupy three contiguous f32 values.
    let max = unsafe { ptr::addr_of_mut!((*bounds).max).cast::<f32>() };
    for component in 0..6 {
        let value = unsafe { if component < 3 { min.add(component) } else { max.add(component - 3) } };
        // SAFETY: component selects one of the six in-bounds FVector components.
        unsafe { archive.serialize_f32(&mut *value)?; }
    }
    // SAFETY: IsValid is the compiler-probed one-byte native field.
    let valid = unsafe { ptr::addr_of_mut!((*bounds).is_valid) };
    // SAFETY: valid addresses the live FBox member.
    unsafe { archive.serialize_u8(&mut *valid)?; }
    Ok(())
}

