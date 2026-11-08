//! Canonical Rust ClassOps for Engine's ANavigationPoint record.

use hp2_abi::engine::generated::{ANavigationPoint, APlayerStart};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind};
use std::alloc::Layout;

const RF_PUBLIC: u32 = 4;
const CPF_EDIT: u32 = 1;
const CPF_CONST: u32 = 2;

const fn boolean(name: &'static str, bit: usize, property_flags: u32) -> StartupProperty {
    StartupProperty {
        name, kind: StartupPropertyKind::Bool { bit_mask: 1 << (bit % 32) },
        category: "", property_flags, object_flags: RF_PUBLIC, inner_name: None,
        offset: bit / 32 * 4, array_dim: 1,
    }
}

const PROPERTIES: &[StartupProperty] = &[
    StartupProperty { name: "ownerTeam", kind: StartupPropertyKind::Name, category: "", property_flags: CPF_EDIT, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, owner_team), array_dim: 1 },
    boolean("taken", ANavigationPoint::TAKEN_BIT, 0),
    StartupProperty { name: "upstreamPaths", kind: StartupPropertyKind::Int, category: "", property_flags: 0, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, upstream_paths), array_dim: 16 },
    StartupProperty { name: "Paths", kind: StartupPropertyKind::Int, category: "", property_flags: 0, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, paths), array_dim: 16 },
    StartupProperty { name: "PrunedPaths", kind: StartupPropertyKind::Int, category: "", property_flags: 0, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, pruned_paths), array_dim: 16 },
    StartupProperty { name: "VisNoReachPaths", kind: StartupPropertyKind::Object { class: "Engine.NavigationPoint" }, category: "", property_flags: 0, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, vis_no_reach_paths), array_dim: 16 },
    StartupProperty { name: "visitedWeight", kind: StartupPropertyKind::Int, category: "", property_flags: 0, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, visited_weight), array_dim: 1 },
    StartupProperty { name: "RouteCache", kind: StartupPropertyKind::Object { class: "Engine.Actor" }, category: "", property_flags: 0, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, route_cache), array_dim: 1 },
    StartupProperty { name: "bestPathWeight", kind: StartupPropertyKind::Int, category: "", property_flags: CPF_CONST, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, best_path_weight), array_dim: 1 },
    StartupProperty { name: "nextNavigationPoint", kind: StartupPropertyKind::Object { class: "Engine.NavigationPoint" }, category: "", property_flags: CPF_CONST, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, next_navigation_point), array_dim: 1 },
    StartupProperty { name: "nextOrdered", kind: StartupPropertyKind::Object { class: "Engine.NavigationPoint" }, category: "", property_flags: CPF_CONST, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, next_ordered), array_dim: 1 },
    StartupProperty { name: "prevOrdered", kind: StartupPropertyKind::Object { class: "Engine.NavigationPoint" }, category: "", property_flags: CPF_CONST, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, prev_ordered), array_dim: 1 },
    StartupProperty { name: "startPath", kind: StartupPropertyKind::Object { class: "Engine.NavigationPoint" }, category: "", property_flags: CPF_CONST, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, start_path), array_dim: 1 },
    StartupProperty { name: "previousPath", kind: StartupPropertyKind::Object { class: "Engine.NavigationPoint" }, category: "", property_flags: CPF_CONST, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, previous_path), array_dim: 1 },
    StartupProperty { name: "cost", kind: StartupPropertyKind::Int, category: "", property_flags: 0, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, cost), array_dim: 1 },
    StartupProperty { name: "ExtraCost", kind: StartupPropertyKind::Int, category: "", property_flags: CPF_EDIT, object_flags: RF_PUBLIC, inner_name: None, offset: std::mem::offset_of!(ANavigationPoint, extra_cost), array_dim: 1 },
    boolean("bPlayerOnly", ANavigationPoint::B_PLAYER_ONLY_BIT, CPF_EDIT),
    boolean("bEndPoint", ANavigationPoint::B_END_POINT_BIT, 0),
    boolean("bSpecialCost", ANavigationPoint::B_SPECIAL_COST_BIT, 0),
    boolean("bEndPointOnly", ANavigationPoint::B_END_POINT_ONLY_BIT, 0),
    boolean("bOneWayPath", ANavigationPoint::B_ONE_WAY_PATH_BIT, CPF_EDIT),
    boolean("bNeverUseStrafing", ANavigationPoint::B_NEVER_USE_STRAFING_BIT, CPF_EDIT),
    boolean("bAutoBuilt", ANavigationPoint::B_AUTO_BUILT_BIT, 0),
    boolean("bTwoWay", ANavigationPoint::B_TWO_WAY_BIT, 0),
];

/// Canonical navigation classes; PlayerStart inherits Actor's native lifecycle.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "NavigationPoint", package: "Engine", super_name: Some("Actor"), within: "Object",
        flags: 0, object_flags: RF_PUBLIC | 0x0008_0000 | 0x0000_4000 | 0x0400_0000, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<ANavigationPoint>()),
        operations: Some(crate::actor_owner::ACTOR_OPERATIONS), properties: PROPERTIES, defaults: &[],
    },
    NativeClassSpec {
        short_name: "PlayerStart", package: "Engine", super_name: Some("NavigationPoint"), within: "Object",
        flags: 0, object_flags: RF_PUBLIC | 0x0008_0000 | 0x0000_4000 | 0x0400_0000, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<APlayerStart>()),
        operations: Some(crate::actor_owner::ACTOR_OPERATIONS), properties: &[], defaults: &[],
    },
];
