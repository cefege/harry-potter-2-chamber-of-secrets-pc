//! Canonical Rust owner for Engine WarpZoneInfo state and coordinate transforms.

use hp2_abi::engine::generated::AWarpZoneInfo;
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind},
};
use std::alloc::Layout;

const CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_NATIVE_REPLICATION: u32 = 0x0000_0800;


/// Native canonical class descriptions for WarpZoneInfo records.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "WarpZoneInfo",
    package: "Engine",
    super_name: Some("ZoneInfo"),
    within: "Object",
    flags: CLASS_NATIVE_REPLICATION,
    object_flags: CLASS_OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<AWarpZoneInfo>()),
    operations: Some(crate::info_owner::ZONE_INFO_OPERATIONS),
    properties: &[],
    defaults: &[],
}];

/// Transforms the three Warp/UnWarp reference arguments using the canonical zone record.
///
/// # Safety
/// `zone` must be a live canonical WarpZoneInfo and each output pointer must be live and writable.
pub(crate) unsafe fn transform(
    access: CoreAccess<'_>,
    zone: *mut AWarpZoneInfo,
    location: *mut hp2_abi::FVector,
    velocity: *mut hp2_abi::FVector,
    rotation: *mut hp2_abi::engine::generated::FRotator,
    transpose: bool,
) -> RuntimeResult<()> {
    if zone.is_null() || location.is_null() || velocity.is_null() || rotation.is_null() {
        return Err(RuntimeError::new("Invalid canonical WarpZoneInfo transform arguments"));
    }
    let class = access.native_class("Engine.WarpZoneInfo")?;
    if !hp2_core::object_model::is_a(zone.cast(), class.cast()) {
        return Err(RuntimeError::new("Warp transform receiver is not a WarpZoneInfo"));
    }
    let coords = unsafe { std::ptr::addr_of!((*zone).warp_coords).read_unaligned() };
    unsafe {
        crate::actor::exec_warp_with_coords(coords, location, velocity, rotation, transpose);
    }
    Ok(())
}
