//! Canonical Rust class lifecycle for Engine's Info actor family.

use crate::actor_owner;
use hp2_abi::{FColor, FVector, engine::generated::{self, AGameReplicationInfo, AInfo, AInterpolationManager, AInterpolationPoint, AKeypoint, ALevelInfo, APlayerReplicationInfo, AReplicationInfo, AZoneInfo}};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind, StartupScalarKind, StartupStructField};
use hp2_core::object_owner::ClassOps;
use std::alloc::Layout;

const CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_CONFIG: u32 = 0x0000_0004;
const CLASS_NATIVE_REPLICATION: u32 = 0x0000_0800;

const VECTOR_FIELDS: &[StartupStructField] = &[
    StartupStructField { name: "X", kind: StartupScalarKind::Float, offset: std::mem::offset_of!(FVector, x) },
    StartupStructField { name: "Y", kind: StartupScalarKind::Float, offset: std::mem::offset_of!(FVector, y) },
    StartupStructField { name: "Z", kind: StartupScalarKind::Float, offset: std::mem::offset_of!(FVector, z) },
];
const COLOR_FIELDS: &[StartupStructField] = &[
    StartupStructField { name: "R", kind: StartupScalarKind::Byte, offset: std::mem::offset_of!(FColor, r) },
    StartupStructField { name: "G", kind: StartupScalarKind::Byte, offset: std::mem::offset_of!(FColor, g) },
    StartupStructField { name: "B", kind: StartupScalarKind::Byte, offset: std::mem::offset_of!(FColor, b) },
    StartupStructField { name: "A", kind: StartupScalarKind::Byte, offset: std::mem::offset_of!(FColor, a) },
];

const fn interpolation_property(name: &'static str, kind: StartupPropertyKind, offset: usize) -> StartupProperty {
    StartupProperty {
        name, kind, category: "", property_flags: 0, object_flags: 4,
        inner_name: None, offset, array_dim: 1,
    }
}

// AInterpolationManager::StaticConstructor pins these script accesses to its native record.
const INTERPOLATION_PROPERTIES: &[StartupProperty] = &[
    interpolation_property("Dest", StartupPropertyKind::Object { class: "Engine.InterpolationPoint" }, std::mem::offset_of!(AInterpolationManager, dest)),
    interpolation_property("Last", StartupPropertyKind::Object { class: "Engine.InterpolationPoint" }, std::mem::offset_of!(AInterpolationManager, last)),
    interpolation_property("PhysAlpha", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, phys_alpha)),
    interpolation_property("PhysRate", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, phys_rate)),
    interpolation_property("RemainingPause", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, remaining_pause)),
    interpolation_property("StartPause", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, start_pause)),
    interpolation_property("PauseNum", StartupPropertyKind::Int, std::mem::offset_of!(AInterpolationManager, pause_num)),
    interpolation_property("OldGameSpeed", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, old_game_speed)),
    interpolation_property("OldFOVModifier", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, old_fov_modifier)),
    interpolation_property("OldFlashScale", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, old_flash_scale)),
    interpolation_property("OldFlashFog", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, std::mem::offset_of!(AInterpolationManager, old_flash_fog)),
    interpolation_property("OldFogColor", StartupPropertyKind::Struct { name: "Color", fields: COLOR_FIELDS }, std::mem::offset_of!(AInterpolationManager, old_fog_color)),
    interpolation_property("OldFogStart", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, old_fog_start)),
    interpolation_property("OldFogEnd", StartupPropertyKind::Float, std::mem::offset_of!(AInterpolationManager, old_fog_end)),
    interpolation_property("TurnRateX", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, std::mem::offset_of!(AInterpolationManager, turn_rate_x)),
    interpolation_property("TurnRateZ", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, std::mem::offset_of!(AInterpolationManager, turn_rate_z)),
    interpolation_property("OldDesiredX", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, std::mem::offset_of!(AInterpolationManager, old_desired_x)),
    interpolation_property("OldDesiredZ", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, std::mem::offset_of!(AInterpolationManager, old_desired_z)),
    interpolation_property("bInstantMove", StartupPropertyKind::Bool { bit_mask: 1 }, AInterpolationManager::B_INSTANT_MOVE_BIT / 32 * 4),
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Info",
        package: "Engine",
        super_name: Some("Actor"),
        within: "Object",
        flags: 0,
        object_flags: CLASS_OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AInfo>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "ZoneInfo",
        package: "Engine",
        super_name: Some("Info"),
        within: "Object",
        flags: CLASS_NATIVE_REPLICATION,
        object_flags: CLASS_OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AZoneInfo>()),
        operations: Some(ZONE_INFO_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "LevelInfo",
        package: "Engine",
        super_name: Some("ZoneInfo"),
        within: "Object",
        flags: CLASS_CONFIG | CLASS_NATIVE_REPLICATION,
        object_flags: CLASS_OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<ALevelInfo>()),
        operations: Some(ZONE_INFO_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    // These classes add only optimized replication dispatch, already owned by
    // actor_rep's shared Core backend; their ordinary lifecycle is AActor's.
    NativeClassSpec {
        short_name: "ReplicationInfo", package: "Engine", super_name: Some("Info"),
        within: "Object", flags: 0, object_flags: CLASS_OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AReplicationInfo>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "PlayerReplicationInfo", package: "Engine", super_name: Some("ReplicationInfo"),
        within: "Object", flags: CLASS_NATIVE_REPLICATION, object_flags: CLASS_OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<APlayerReplicationInfo>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "GameReplicationInfo", package: "Engine", super_name: Some("ReplicationInfo"),
        within: "Object", flags: CLASS_CONFIG | CLASS_NATIVE_REPLICATION, object_flags: CLASS_OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AGameReplicationInfo>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "Keypoint", package: "Engine", super_name: Some("Actor"),
        within: "Object", flags: 0, object_flags: CLASS_OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AKeypoint>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "InterpolationPoint", package: "Engine", super_name: Some("Keypoint"),
        within: "Object", flags: 0, object_flags: CLASS_OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AInterpolationPoint>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "InterpolationManager", package: "Engine", super_name: Some("Info"),
        within: "Object", flags: 0, object_flags: CLASS_OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AInterpolationManager>()),
        operations: Some(actor_owner::ACTOR_OPERATIONS),
        properties: INTERPOLATION_PROPERTIES, defaults: &[],
    },
];

pub(crate) const ZONE_INFO_OPERATIONS: ClassOps = {
    let mut operations = actor_owner::ACTOR_OPERATIONS;
    operations.post_edit_change = post_edit_change_zone_info;
    operations
};

/// Runs the superclass edit hook and flushes the actual owner-local Engine cache.
///
/// # Safety
/// `object` must be a live canonical `AZoneInfo` selected by Core class dispatch.
unsafe fn post_edit_change_zone_info(access: CoreAccess<'_>, object: *mut generated::UObject) -> RuntimeResult<()> {
    // SAFETY: native AZoneInfo::PostEditChange starts with AActor::PostEditChange.
    unsafe { (actor_owner::ACTOR_OPERATIONS.post_edit_change)(access, object)?; }
    if access.process_mode().is_editor {
        let state = access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("ZoneInfo has no canonical Engine owner"))?;
        let mut cache = state.cache.try_borrow_mut()
            .map_err(|_| RuntimeError::new("Engine cache flush reentered an active cache operation"))?;
        cache.as_mut().ok_or_else(|| RuntimeError::new("Engine cache has exited"))?
            .flush(access, 0, u32::MAX, 0)?;
    }
    Ok(())
}

