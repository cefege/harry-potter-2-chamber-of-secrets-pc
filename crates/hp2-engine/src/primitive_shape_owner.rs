//! Canonical declarations for Engine's specialized cylinder primitives.

use hp2_abi::engine::generated::{UAlignedOvalCylinder, UOrientedCylinder, UOrientedOvalCylinder};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_owner::ClassOps;
use std::alloc::Layout;

/// Native class specifications for the cylinder specializations.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "OrientedCylinder",
        package: "Engine",
        super_name: Some("Primitive"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UOrientedCylinder>()),
        operations: Some(OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "AlignedOvalCylinder",
        package: "Engine",
        super_name: Some("Primitive"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UAlignedOvalCylinder>()),
        operations: Some(OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "OrientedOvalCylinder",
        package: "Engine",
        super_name: Some("AlignedOvalCylinder"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UOrientedOvalCylinder>()),
        operations: Some(OPERATIONS),
        properties: &[],
        defaults: &[],
    },
];

const OPERATIONS: ClassOps = {
    let mut operations = NativeRecordKind::Object.operations();
    operations.construct = crate::primitive_owner::construct_primitive;
    operations.serialize = crate::primitive_owner::serialize_primitive;
    operations
};

