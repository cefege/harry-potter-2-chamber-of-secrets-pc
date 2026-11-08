//! Canonical Engine.GameInfo native class metadata.

use hp2_abi::engine::generated::AGameInfo;
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use std::alloc::Layout;

const CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;

/// Native GameInfo derives from Info/AActor and uses the actor class lifecycle.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "GameInfo",
    package: "Engine",
    super_name: Some("Info"),
    within: "Object",
    flags: 0x0000_0004,
    object_flags: CLASS_OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<AGameInfo>()),
    operations: Some(crate::actor_owner::ACTOR_OPERATIONS),
    properties: &[],
    defaults: &[],
}];
