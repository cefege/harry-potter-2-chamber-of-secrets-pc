//! Canonical Engine class operations for native palette color arrays.

use crate::animation_owner;
use hp2_abi::engine::generated::{self, FColor, TArrayRaw, UObject, UPalette};
use hp2_core::core_access::{CoreAccess, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_archive::ObjectArchive;
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Palette",
    package: "Engine",
    super_name: Some("Object"),
    within: "Object",
    flags: generated::PALETTE_CLASS_FLAGS,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<UPalette>()),
    operations: Some(PALETTE),
    properties: &[],
    defaults: &[],
}];

const PALETTE: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_palette;
    operations.finalize_members = finalize_palette;
    operations.serialize = serialize_palette;
    operations
};

unsafe fn construct_palette(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.construct)(access, object)?;
    }
    let colors = unsafe { ptr::addr_of_mut!((*object.cast::<UPalette>()).colors) };
    // SAFETY: this is the live palette's compiler-probed member header.
    unsafe {
        colors.write_unaligned(TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        });
    }
    Ok(())
}

unsafe fn finalize_palette(_access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        animation_owner::destroy_array(
            ptr::addr_of_mut!((*object.cast::<UPalette>()).colors).cast(),
            size_of::<FColor>(),
            None,
        )
    }
}

unsafe fn serialize_palette(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.serialize)(access, object, archive)?;
    }
    let palette = object.cast::<UPalette>();
    unsafe {
        animation_owner::serialize_array(
            archive,
            ptr::addr_of_mut!((*palette).colors),
            size_of::<FColor>(),
            Some(serialize_color),
            None,
            false,
        )?;
        if archive.flags().version < 66 {
            crate::texture_runtime::force_palette_opaque(
                (*palette).colors.data.cast(),
                (*palette).colors.num,
            );
        }
    }
    Ok(())
}

unsafe fn serialize_color(
    archive: &ObjectArchive<'_>,
    element: *mut std::ffi::c_void,
) -> RuntimeResult<()> {
    let color = unsafe { &mut *element.cast::<FColor>() };
    archive.serialize_u8(&mut color.r)?;
    archive.serialize_u8(&mut color.g)?;
    archive.serialize_u8(&mut color.b)?;
    archive.serialize_u8(&mut color.a)
}
