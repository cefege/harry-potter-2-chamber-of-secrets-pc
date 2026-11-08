//! `WorldCollisionGateReporter` and its three JSON scenario parsers, ported
//! from `HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp:1097-2242`
//! (`WorldCollisionGateRole`/`WorldCollisionGateScenario` through the end of
//! `WorldCollisionGateReporter`). One reporter, three wire protocols
//! selected at construction (`EProtocol`): full collision-gate, touch-only,
//! bump-only -- `SDLLaunch.cpp`'s thin adapter keeps the same class name,
//! `EProtocol` enum and public method signatures so the untouched
//! `FMainLoopState`/`MainLoop` call sites (`SDLLaunch.cpp:5389+`) keep
//! compiling unchanged; everything else here is new.
//!
//! Real engine calls (`ULevel::FindSpot`/`FarMoveActor`/`MoveActor`/
//! `SpawnActor`, `AActor::GetCylinderExtent`/`SetCollision`/
//! `SetCollisionSize`/`SetBase`/`EndTouch`, `UPrimitive::
//! GetCollisionBoundingBox`, `UObject::StaticLoadClass`/`GetPathName`) are
//! called through [`Services`], a table of `extern "C"` leaves published
//! once by `SDLLaunch.cpp`'s C++ adapter and captured by value at
//! construction (reporters are long-lived but single-instance; no process
//! ever constructs two live world-collision-gate reporters at once, so a
//! copy per instance is simpler than global `OnceLock` state and avoids any
//! "is it initialized yet" question). Plain field reads (`Location`,
//! `Rotation`, `Base`, `Region.Zone`, `Touching`, `bJustTeleported`,
//! `bDeleteMe`, `bStatic`, `bMovable`, `bCollideWorld`, `CollisionRadius`,
//! a class's `ClassFlags`, an object's `Class`) go straight through
//! `hp2_abi::engine::generated`'s existing layout mirrors instead -- no
//! leaf needed, and no risk of the leaf table and the mirror disagreeing.

use crate::reporters::json;
use crate::reporters::trace_services::{self, ReportWriteError, write_report};
use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FBox, FCheckResult, FRotator, UClass, UObject};
use std::{cell::RefCell, ffi::c_void, rc::Rc};

const CLASS_ABSTRACT: u32 = 0x0001;
const TOUCHING_SLOTS: usize = 4;

// ---------------------------------------------------------------------
// Scenario model and parsers (SDLLaunch.cpp:1097-1530)
// ---------------------------------------------------------------------

/// `WorldCollisionGateRole` (`SDLLaunch.cpp:1143-1152`).
#[derive(Clone, Default)]
pub(crate) struct Role {
    pub class: String,
    pub transient: bool,
    pub radius: f32,
    pub height: f32,
    pub collide_actors: bool,
    pub collide_world: bool,
    pub block_actors: bool,
    pub block_players: bool,
}

const ZERO_VECTOR: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

/// `WorldCollisionGateScenario` (`SDLLaunch.cpp:1154-1172`).
#[derive(Clone)]
pub(crate) struct Scenario {
    pub fixture: String,
    pub map_relative: String,
    pub map_sha256: String,
    pub fixed_delta_seconds: f32,
    pub requested_ticks: i32,
    pub probe: Role,
    pub blocker: Role,
    pub touch_target: Role,
    pub zone_trigger: Role,
    pub spawn_anchor: Role,
    pub safe_origin_offset: FVector,
    pub touch_placement_offset: FVector,
    pub bump_placement_offset: FVector,
    pub touch_delta: FVector,
    pub block_clearance: f32,
    pub block_delta: FVector,
    pub carry_delta: FVector,
}

impl Default for Scenario {
    fn default() -> Self {
        Scenario {
            fixture: String::new(),
            map_relative: String::new(),
            map_sha256: String::new(),
            fixed_delta_seconds: 0.0,
            requested_ticks: 0,
            probe: Role::default(),
            blocker: Role::default(),
            touch_target: Role::default(),
            zone_trigger: Role::default(),
            spawn_anchor: Role::default(),
            safe_origin_offset: ZERO_VECTOR,
            touch_placement_offset: ZERO_VECTOR,
            bump_placement_offset: ZERO_VECTOR,
            touch_delta: ZERO_VECTOR,
            block_clearance: 0.0,
            block_delta: ZERO_VECTOR,
            carry_delta: ZERO_VECTOR,
        }
    }
}

/// `ParseWorldCollisionGateSelector`.
pub(crate) fn parse_selector(selectors: &[u8], name: &str) -> Option<Role> {
    let selector = json::object_member(selectors, name)?;
    let class = json::string_member(&selector, "class")?;
    let cardinality = json::int_member(&selector, "cardinality")?;
    if cardinality != 1 {
        return None;
    }
    Some(Role {
        class,
        ..Role::default()
    })
}

/// `ParseWorldCollisionGateTransient`.
fn parse_transient(transients: &[u8], name: &str) -> Option<Role> {
    let transient = json::object_member(transients, name)?;
    let class = json::string_member(&transient, "class")?;
    let collision = json::object_member(&transient, "collision")?;
    Some(Role {
        class,
        transient: true,
        radius: json::float_member(&collision, "radius")?,
        height: json::float_member(&collision, "height")?,
        collide_actors: json::bool_member(&collision, "collide_actors")?,
        collide_world: json::bool_member(&collision, "collide_world")?,
        block_actors: json::bool_member(&collision, "block_actors")?,
        block_players: json::bool_member(&collision, "block_players")?,
    })
}

/// `ValidateWorldCollisionGateOperations`.
fn validate_full_operations(document: &[u8], out: &mut Scenario) -> bool {
    const IDS: [&str; 6] = [
        "resolve_safe_origin",
        "touch_sweep",
        "prepare_block",
        "block_sweep",
        "far_move",
        "set_base_and_carry",
    ];
    const OPS: [&str; 6] = [
        "FindSpot",
        "MoveActor",
        "FarMoveActor",
        "MoveActor",
        "FarMoveActor",
        "MoveActor",
    ];
    (|| -> Option<()> {
        let values = json::object_array_member(document, "operations")?;
        if values.len() != IDS.len() {
            return None;
        }
        for (index, value) in values.iter().enumerate() {
            if !json::string_equals(value, "id", IDS[index]) || !json::string_equals(value, "op", OPS[index]) {
                return None;
            }
        }
        if !json::string_equals(&values[0], "actor", "probe") {
            return None;
        }
        let origin = json::object_member(&values[0], "origin")?;
        if !json::string_equals(&origin, "relative_to", "spawn_anchor") {
            return None;
        }
        out.safe_origin_offset = json::vector_member(&origin, "offset")?;

        if !json::string_equals(&values[1], "actor", "probe") {
            return None;
        }
        let touch_placement = json::object_member(&values[1], "placement")?;
        if !json::string_equals(&touch_placement, "actor", "touch_target")
            || !json::string_equals(&touch_placement, "relative_to", "probe")
        {
            return None;
        }
        out.touch_placement_offset = json::vector_member(&touch_placement, "offset")?;
        out.touch_delta = json::vector_member(&values[1], "delta")?;

        if !json::string_equals(&values[2], "actor", "probe") {
            return None;
        }
        let block_placement = json::object_member(&values[2], "placement")?;
        if !json::string_equals(&block_placement, "relative_to", "blocker")
            || !json::string_equals(&block_placement, "approach", "negative_x")
        {
            return None;
        }
        out.block_clearance = json::float_member(&block_placement, "clearance")?;
        if !json::bool_member(&block_placement, "find_spot")? {
            return None;
        }

        if !json::string_equals(&values[3], "actor", "probe") {
            return None;
        }
        out.block_delta = json::vector_member(&values[3], "delta")?;

        if !json::string_equals(&values[4], "actor", "probe")
            || !json::string_equals(&values[4], "destination", "safe_origin")
            || !json::bool_member(&values[4], "find_spot")?
        {
            return None;
        }

        if !json::string_equals(&values[5], "actor", "blocker") {
            return None;
        }
        let prelude = json::object_member(&values[5], "prelude")?;
        if !json::string_equals(&prelude, "op", "SetBase")
            || !json::string_equals(&prelude, "actor", "probe")
            || !json::string_equals(&prelude, "base", "blocker")
        {
            return None;
        }
        out.carry_delta = json::vector_member(&values[5], "delta")?;
        Some(())
    })()
    .is_some()
}

/// `ParseWorldCollisionGateScenario`.
pub(crate) fn parse_collision_scenario(filename: Option<&str>) -> Option<Scenario> {
    let document = json::read_document(filename)?;
    let version = json::int_member(&document, "version")?;
    if version != 1 {
        return None;
    }
    let mut out = Scenario {
        fixture: json::string_member(&document, "fixture")?,
        ..Scenario::default()
    };
    let map = json::object_member(&document, "map")?;
    out.map_relative = json::string_member(&map, "relative")?;
    out.map_sha256 = json::string_member(&map, "sha256")?;
    out.fixed_delta_seconds = json::float_member(&document, "fixed_dt")?;
    out.requested_ticks = json::int_member(&document, "requested_ticks")?;
    let selectors = json::object_member(&document, "selectors")?;
    let transients = json::object_member(&document, "transients")?;
    out.blocker = parse_selector(&selectors, "blocker")?;
    out.zone_trigger = parse_selector(&selectors, "zone_trigger")?;
    out.spawn_anchor = parse_selector(&selectors, "spawn_anchor")?;
    out.probe = parse_transient(&transients, "probe")?;
    out.touch_target = parse_transient(&transients, "touch_target")?;
    if !validate_full_operations(&document, &mut out) {
        return None;
    }
    if out.fixture != "TriggerTest2WorldCollision" || out.fixed_delta_seconds <= 0.0 || out.requested_ticks < 0 {
        return None;
    }
    Some(out)
}

/// `ValidateWorldTouchGateOperations`.
fn validate_touch_operations(document: &[u8], out: &mut Scenario) -> bool {
    const SAFE_KEYS: [&str; 4] = ["id", "op", "actor", "origin"];
    const ORIGIN_KEYS: [&str; 2] = ["relative_to", "offset"];
    const TOUCH_KEYS: [&str; 5] = ["id", "op", "actor", "placement", "delta"];
    const PLACEMENT_KEYS: [&str; 3] = ["actor", "relative_to", "offset"];
    (|| -> Option<()> {
        let values = json::object_array_member(document, "operations")?;
        if values.len() != 2 {
            return None;
        }
        if !json::has_exact_keys(&values[0], &SAFE_KEYS) || !json::has_exact_keys(&values[1], &TOUCH_KEYS) {
            return None;
        }
        if !json::string_equals(&values[0], "id", "resolve_safe_origin")
            || !json::string_equals(&values[0], "op", "FindSpot")
            || !json::string_equals(&values[0], "actor", "probe")
        {
            return None;
        }
        let origin = json::object_member(&values[0], "origin")?;
        if !json::has_exact_keys(&origin, &ORIGIN_KEYS) || !json::string_equals(&origin, "relative_to", "spawn_anchor") {
            return None;
        }
        out.safe_origin_offset = json::vector_member(&origin, "offset")?;

        if !json::string_equals(&values[1], "id", "touch_sweep")
            || !json::string_equals(&values[1], "op", "MoveActor")
            || !json::string_equals(&values[1], "actor", "probe")
        {
            return None;
        }
        let placement = json::object_member(&values[1], "placement")?;
        if !json::has_exact_keys(&placement, &PLACEMENT_KEYS)
            || !json::string_equals(&placement, "actor", "touch_target")
            || !json::string_equals(&placement, "relative_to", "probe")
        {
            return None;
        }
        out.touch_placement_offset = json::vector_member(&placement, "offset")?;
        out.touch_delta = json::vector_member(&values[1], "delta")?;
        Some(())
    })()
    .is_some()
}

/// `ParseWorldTouchGateScenario`.
pub(crate) fn parse_touch_scenario(filename: Option<&str>) -> Option<Scenario> {
    const ROOT_KEYS: [&str; 9] = [
        "version",
        "fixture",
        "map",
        "fixed_dt",
        "requested_ticks",
        "selectors",
        "transients",
        "operations",
        "random_calls",
    ];
    const MAP_KEYS: [&str; 2] = ["relative", "sha256"];
    const SELECTOR_KEYS: [&str; 1] = ["spawn_anchor"];
    const SELECTOR_VALUE_KEYS: [&str; 2] = ["class", "cardinality"];
    const TRANSIENT_KEYS: [&str; 2] = ["probe", "touch_target"];
    const TRANSIENT_VALUE_KEYS: [&str; 2] = ["class", "collision"];
    const COLLISION_KEYS: [&str; 6] = [
        "radius",
        "height",
        "collide_actors",
        "collide_world",
        "block_actors",
        "block_players",
    ];
    let document = json::read_document(filename)?;
    if !json::has_exact_keys(&document, &ROOT_KEYS) {
        return None;
    }
    let version = json::int_member(&document, "version")?;
    if version != 1 || !json::string_equals(&document, "fixture", "TriggerTest2WorldTouch") {
        return None;
    }
    let mut out = Scenario::default();
    let map = json::object_member(&document, "map")?;
    if !json::has_exact_keys(&map, &MAP_KEYS) {
        return None;
    }
    out.map_relative = json::string_member(&map, "relative")?;
    out.map_sha256 = json::string_member(&map, "sha256")?;
    if out.map_relative != "Maps/Studies/TriggerTest2.unr"
        || out.map_sha256 != "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc"
    {
        return None;
    }
    out.fixed_delta_seconds = json::float_member(&document, "fixed_dt")?;
    if out.fixed_delta_seconds != 0.016_666_667 {
        return None;
    }
    out.requested_ticks = json::int_member(&document, "requested_ticks")?;
    if out.requested_ticks != 1 {
        return None;
    }
    let selectors = json::object_member(&document, "selectors")?;
    if !json::has_exact_keys(&selectors, &SELECTOR_KEYS) {
        return None;
    }
    let spawn_anchor_raw = json::object_member(&selectors, "spawn_anchor")?;
    if !json::has_exact_keys(&spawn_anchor_raw, &SELECTOR_VALUE_KEYS) {
        return None;
    }
    out.spawn_anchor = parse_selector(&selectors, "spawn_anchor")?;
    if out.spawn_anchor.class != "Engine.PlayerStart" {
        return None;
    }
    let transients = json::object_member(&document, "transients")?;
    if !json::has_exact_keys(&transients, &TRANSIENT_KEYS) {
        return None;
    }
    let probe_raw = json::object_member(&transients, "probe")?;
    if !json::has_exact_keys(&probe_raw, &TRANSIENT_VALUE_KEYS) {
        return None;
    }
    let probe_collision = json::object_member(&probe_raw, "collision")?;
    if !json::has_exact_keys(&probe_collision, &COLLISION_KEYS) {
        return None;
    }
    let touch_target_raw = json::object_member(&transients, "touch_target")?;
    if !json::has_exact_keys(&touch_target_raw, &TRANSIENT_VALUE_KEYS) {
        return None;
    }
    let touch_collision = json::object_member(&touch_target_raw, "collision")?;
    if !json::has_exact_keys(&touch_collision, &COLLISION_KEYS) {
        return None;
    }
    out.probe = parse_transient(&transients, "probe")?;
    out.touch_target = parse_transient(&transients, "touch_target")?;
    let random_calls = json::object_array_member(&document, "random_calls")?;
    if !random_calls.is_empty() {
        return None;
    }
    if !validate_touch_operations(&document, &mut out) {
        return None;
    }
    out.fixture = "TriggerTest2WorldTouch".to_string();
    Some(out)
}

/// `ValidateWorldBumpGateOperations`.
fn validate_bump_operations(document: &[u8], out: &mut Scenario) -> bool {
    const SAFE_KEYS: [&str; 4] = ["id", "op", "actor", "origin"];
    const ORIGIN_KEYS: [&str; 2] = ["relative_to", "offset"];
    const BLOCK_KEYS: [&str; 5] = ["id", "op", "actor", "placement", "delta"];
    const PLACEMENT_KEYS: [&str; 3] = ["actor", "relative_to", "offset"];
    (|| -> Option<()> {
        let values = json::object_array_member(document, "operations")?;
        if values.len() != 2
            || !json::has_exact_keys(&values[0], &SAFE_KEYS)
            || !json::has_exact_keys(&values[1], &BLOCK_KEYS)
        {
            return None;
        }
        if !json::string_equals(&values[0], "id", "resolve_safe_origin")
            || !json::string_equals(&values[0], "op", "FindSpot")
            || !json::string_equals(&values[0], "actor", "probe")
        {
            return None;
        }
        let origin = json::object_member(&values[0], "origin")?;
        if !json::has_exact_keys(&origin, &ORIGIN_KEYS) || !json::string_equals(&origin, "relative_to", "spawn_anchor") {
            return None;
        }
        out.safe_origin_offset = json::vector_member(&origin, "offset")?;

        if !json::string_equals(&values[1], "id", "block_sweep")
            || !json::string_equals(&values[1], "op", "MoveActor")
            || !json::string_equals(&values[1], "actor", "probe")
        {
            return None;
        }
        let placement = json::object_member(&values[1], "placement")?;
        if !json::has_exact_keys(&placement, &PLACEMENT_KEYS)
            || !json::string_equals(&placement, "actor", "blocker")
            || !json::string_equals(&placement, "relative_to", "safe_origin")
        {
            return None;
        }
        out.bump_placement_offset = json::vector_member(&placement, "offset")?;
        out.block_delta = json::vector_member(&values[1], "delta")?;
        if out.block_delta.x != 256.0 || out.block_delta.y != 0.0 || out.block_delta.z != 0.0 {
            return None;
        }
        Some(())
    })()
    .is_some()
}

/// `IsWorldBumpGateTransient`.
fn is_bump_gate_transient(role: &Role) -> bool {
    role.class == "Engine.Actor"
        && role.radius == 16.0
        && role.height == 24.0
        && role.collide_actors
        && role.collide_world
        && role.block_actors
        && role.block_players
}

/// `ParseWorldBumpGateScenario`.
pub(crate) fn parse_bump_scenario(filename: Option<&str>) -> Option<Scenario> {
    const ROOT_KEYS: [&str; 9] = [
        "version",
        "fixture",
        "map",
        "fixed_dt",
        "requested_ticks",
        "selectors",
        "transients",
        "operations",
        "random_calls",
    ];
    const MAP_KEYS: [&str; 2] = ["relative", "sha256"];
    const SELECTOR_KEYS: [&str; 1] = ["spawn_anchor"];
    const SELECTOR_VALUE_KEYS: [&str; 2] = ["class", "cardinality"];
    const TRANSIENT_KEYS: [&str; 2] = ["probe", "blocker"];
    const TRANSIENT_VALUE_KEYS: [&str; 2] = ["class", "collision"];
    const COLLISION_KEYS: [&str; 6] = [
        "radius",
        "height",
        "collide_actors",
        "collide_world",
        "block_actors",
        "block_players",
    ];
    let document = json::read_document(filename)?;
    if !json::has_exact_keys(&document, &ROOT_KEYS) {
        return None;
    }
    let version = json::int_member(&document, "version")?;
    if version != 1 || !json::string_equals(&document, "fixture", "TriggerTest2WorldBump") {
        return None;
    }
    let mut out = Scenario::default();
    let map = json::object_member(&document, "map")?;
    if !json::has_exact_keys(&map, &MAP_KEYS) {
        return None;
    }
    out.map_relative = json::string_member(&map, "relative")?;
    out.map_sha256 = json::string_member(&map, "sha256")?;
    if out.map_relative != "Maps/Studies/TriggerTest2.unr"
        || out.map_sha256 != "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc"
    {
        return None;
    }
    out.fixed_delta_seconds = json::float_member(&document, "fixed_dt")?;
    if out.fixed_delta_seconds != 0.016_666_667 {
        return None;
    }
    out.requested_ticks = json::int_member(&document, "requested_ticks")?;
    if out.requested_ticks != 1 {
        return None;
    }
    let selectors = json::object_member(&document, "selectors")?;
    if !json::has_exact_keys(&selectors, &SELECTOR_KEYS) {
        return None;
    }
    let spawn_anchor_raw = json::object_member(&selectors, "spawn_anchor")?;
    if !json::has_exact_keys(&spawn_anchor_raw, &SELECTOR_VALUE_KEYS) {
        return None;
    }
    out.spawn_anchor = parse_selector(&selectors, "spawn_anchor")?;
    if out.spawn_anchor.class != "Engine.PlayerStart" {
        return None;
    }
    let transients = json::object_member(&document, "transients")?;
    if !json::has_exact_keys(&transients, &TRANSIENT_KEYS) {
        return None;
    }
    let probe_raw = json::object_member(&transients, "probe")?;
    if !json::has_exact_keys(&probe_raw, &TRANSIENT_VALUE_KEYS) {
        return None;
    }
    let probe_collision = json::object_member(&probe_raw, "collision")?;
    if !json::has_exact_keys(&probe_collision, &COLLISION_KEYS) {
        return None;
    }
    let blocker_raw = json::object_member(&transients, "blocker")?;
    if !json::has_exact_keys(&blocker_raw, &TRANSIENT_VALUE_KEYS) {
        return None;
    }
    let blocker_collision = json::object_member(&blocker_raw, "collision")?;
    if !json::has_exact_keys(&blocker_collision, &COLLISION_KEYS) {
        return None;
    }
    out.probe = parse_transient(&transients, "probe")?;
    out.blocker = parse_transient(&transients, "blocker")?;
    if !is_bump_gate_transient(&out.probe) || !is_bump_gate_transient(&out.blocker) {
        return None;
    }
    let random_calls = json::object_array_member(&document, "random_calls")?;
    if !random_calls.is_empty() {
        return None;
    }
    if !validate_bump_operations(&document, &mut out) {
        return None;
    }
    out.fixture = "TriggerTest2WorldBump".to_string();
    Some(out)
}

// ---------------------------------------------------------------------
// Engine services leaf table (SDLLaunch.cpp real engine calls this
// reporter and `static_bsp_probe` both need)
// ---------------------------------------------------------------------

/// Leaves the `SDLLaunch.cpp` adapter publishes once; captured by value at
/// construction. Every pointer is an opaque native handle (`UEngine*`,
/// `ULevel*`, `AActor*`, `UClass*`); `FVector`/`FRotator`/`FCheckResult`/
/// `FBox` are passed by value using `hp2_abi`'s real-layout mirrors, same
/// bytes as the C++ types the leaf bodies actually operate on.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2WorldCollisionGateServices {
    /// `UObject::StaticLoadClass(AActor::StaticClass(), NULL, Name, NULL, Flags, NULL)`;
    /// `name`/`name_len` is UTF-8, not NUL-terminated.
    pub static_load_actor_class:
        unsafe extern "C" fn(name: *const u8, name_len: usize, flags: u32) -> *mut c_void,
    /// `appToAnsi(*Actor->Tag)`; writes up to `cap` bytes (no NUL) into
    /// `buf`, returns the byte length actually available (may exceed
    /// `cap`, same truncation contract as `snprintf`'s return value).
    pub actor_tag: unsafe extern "C" fn(actor: *mut c_void, buf: *mut u8, cap: usize) -> i32,
    /// `Actor->GetCylinderExtent()`.
    pub actor_cylinder_extent: unsafe extern "C" fn(actor: *mut c_void) -> FVector,
    /// `Actor->SetCollision(CollideActors, BlockActors, BlockPlayers)`.
    pub actor_set_collision:
        unsafe extern "C" fn(actor: *mut c_void, collide_actors: i32, block_actors: i32, block_players: i32),
    /// `Actor->SetCollisionSize(Radius, Height)`.
    pub actor_set_collision_size: unsafe extern "C" fn(actor: *mut c_void, radius: f32, height: f32),
    /// `Actor->SetBase(Base)`.
    pub actor_set_base: unsafe extern "C" fn(actor: *mut c_void, base: *mut c_void),
    /// `Actor->EndTouch(Other, NoNotifySelf)`.
    pub actor_end_touch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void, no_notify_self: i32),
    /// `Actor->GetPrimitive()->GetCollisionBoundingBox(Actor, World)`.
    pub actor_collision_bounding_box: unsafe extern "C" fn(actor: *mut c_void, world: i32) -> FBox,
    /// `Level->FindSpot(Extent, *Location, CheckActors, AssumeFit)`;
    /// `*location` is both the requested and the resolved location.
    pub find_spot: unsafe extern "C" fn(
        level: *mut c_void,
        extent: FVector,
        location: *mut FVector,
        check_actors: i32,
        assume_fit: i32,
    ) -> i32,
    /// `Level->FarMoveActor(Actor, Dest, Test, NoCheck)`.
    pub far_move_actor:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, dest: FVector, test: i32, no_check: i32) -> i32,
    /// `Level->MoveActor(Actor, Delta, Rotation, Hit, Test, IgnorePawns, IgnoreBases, NoFail)`.
    #[allow(clippy::type_complexity)]
    pub move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        delta: FVector,
        rotation: FRotator,
        hit: *mut FCheckResult,
        test: i32,
        ignore_pawns: i32,
        ignore_bases: i32,
        no_fail: i32,
    ) -> i32,
    /// `Level->SpawnActor(Class, NAME_None, NULL, NULL, Location, Rotation, NULL, NoCollisionFail)`.
    pub spawn_actor: unsafe extern "C" fn(
        level: *mut c_void,
        class_: *mut c_void,
        location: FVector,
        rotation: FRotator,
        no_collision_fail: i32,
    ) -> *mut c_void,
}

const NAME_BUFFER_CAP: usize = 1024;

/// Raw `appToAnsi` bytes, not a lossy UTF-8 `String`: see
/// `reporters::json::push_json_string`'s doc comment.
fn fetch_name(
    object: *mut c_void,
    leaf: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> i32,
) -> Vec<u8> {
    if object.is_null() {
        return Vec::new();
    }
    let mut buffer = [0u8; NAME_BUFFER_CAP];
    let len = unsafe { leaf(object, buffer.as_mut_ptr(), buffer.len()) };
    if len <= 0 {
        return Vec::new();
    }
    let len = (len as usize).min(buffer.len());
    buffer[..len].to_vec()
}

/// `Object ? Text(Object->GetPathName()) : ""`, replacing the old
/// dedicated `object_path_name` leaf with the already-published generic
/// `object_path_ansi` one.
fn object_path_text(object: *mut c_void) -> Vec<u8> {
    if object.is_null() {
        return Vec::new();
    }
    match unsafe { trace_services::published_services() } {
        Ok(svc) => unsafe { trace_services::object_path(&svc, object) },
        Err(_) => Vec::new(),
    }
}

/// `Cast<UGameEngine>(Engine) ? GameEngine->GLevel : NULL`, replacing the
/// old dedicated `engine_level` leaf with the already-published generic
/// `game_engine_level` one.
fn game_engine_level(engine: *mut c_void) -> *mut c_void {
    match unsafe { trace_services::published_services() } {
        Ok(svc) => unsafe { (svc.game_engine_level)(engine) },
        Err(_) => std::ptr::null_mut(),
    }
}

/// `Level->Actors.Num()`, replacing the old dedicated `level_actor_count`
/// leaf with the already-published generic one.
fn level_actor_count(level: *mut c_void) -> i32 {
    match unsafe { trace_services::published_services() } {
        Ok(svc) => unsafe { (svc.level_actor_count)(level) },
        Err(_) => 0,
    }
}

/// `Level->Actors(Index)`, replacing the old dedicated `level_actor_at`
/// leaf with the already-published generic one.
fn level_actor_at(level: *mut c_void, index: i32) -> *mut c_void {
    match unsafe { trace_services::published_services() } {
        Ok(svc) => unsafe { (svc.level_actor_at)(level, index) },
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------
// Reporter state (SDLLaunch.cpp:1540-2242)
// ---------------------------------------------------------------------

/// `WorldCollisionGateReporter::EProtocol`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Full,
    Touch,
    Bump,
}

struct Step {
    id: String,
    request: String,
    moved: bool,
    has_hit_time: bool,
    hit_time: f32,
    blocked: bool,
    hit_actor: *mut c_void,
    events: Vec<String>,
    actors: Vec<u8>,
}

struct SelectorDetail {
    id: String,
    class: String,
    path: Vec<u8>,
    match_count: i32,
}

#[derive(Clone, Copy)]
pub enum MovementEvent {
    Bump,
    Touch,
    UnTouch,
}

struct EventCapture {
    recording: bool,
    pending_events: Vec<String>,
    probe: *mut c_void,
    blocker: *mut c_void,
    touch_target: *mut c_void,
    zone_trigger: *mut c_void,
    spawn_anchor: *mut c_void,
}
impl Default for EventCapture {
    fn default() -> Self {
        Self {
            recording: false,
            pending_events: Vec::new(),
            probe: std::ptr::null_mut(),
            blocker: std::ptr::null_mut(),
            touch_target: std::ptr::null_mut(),
            zone_trigger: std::ptr::null_mut(),
            spawn_anchor: std::ptr::null_mut(),
        }
    }
}


/// Independent synchronous event sink; it can be entered while `State::run`
/// is inside a canonical movement operation.
#[derive(Clone, Default)]
pub struct MovementObserver(Rc<RefCell<EventCapture>>);

impl MovementObserver {
    pub fn observe(
        &self,
        event: MovementEvent,
        recipient: *mut c_void,
        other: *mut c_void,
    ) -> Result<(), ()> {
        let mut capture = self.0.try_borrow_mut().map_err(|_| ())?;
        Self::record_into(&mut capture, event, recipient, other);
        Ok(())
    }

    fn record(&self, event: MovementEvent, recipient: *mut c_void, other: *mut c_void) {
        let mut capture = self.0.borrow_mut();
        Self::record_into(&mut capture, event, recipient, other);
    }

    fn set_roles(
        &self,
        probe: *mut c_void,
        blocker: *mut c_void,
        touch_target: *mut c_void,
        zone_trigger: *mut c_void,
        spawn_anchor: *mut c_void,
    ) {
        let mut capture = self.0.borrow_mut();
        capture.probe = probe;
        capture.blocker = blocker;
        capture.touch_target = touch_target;
        capture.zone_trigger = zone_trigger;
        capture.spawn_anchor = spawn_anchor;
    }

    fn set_recording(&self, recording: bool, clear: bool) {
        let mut capture = self.0.borrow_mut();
        if clear {
            capture.pending_events.clear();
        }
        capture.recording = recording;
    }

    fn finish_step(&self) -> Vec<String> {
        let mut capture = self.0.borrow_mut();
        capture.recording = false;
        capture.pending_events.clone()
    }


    fn record_into(capture: &mut EventCapture, event: MovementEvent, recipient: *mut c_void, other: *mut c_void) {
        if !capture.recording {
            return;
        }
        let recipient_role = Self::role_name_in(capture, recipient);
        let other_role = Self::role_name_in(capture, other);
        if let (Some(recipient_role), Some(other_role)) = (recipient_role, other_role) {
            let name = match event {
                MovementEvent::Bump => "Bump",
                MovementEvent::Touch => "Touch",
                MovementEvent::UnTouch => "UnTouch",
            };
            capture.pending_events.push(format!("{name}:{recipient_role}:{other_role}"));
        }
    }

    fn role_name_in(capture: &EventCapture, actor: *mut c_void) -> Option<&'static str> {
        if actor.is_null() {
            None
        } else if actor == capture.probe {
            Some("probe")
        } else if actor == capture.blocker {
            Some("blocker")
        } else if actor == capture.touch_target {
            Some("touch_target")
        } else if actor == capture.zone_trigger {
            Some("zone_trigger")
        } else if actor == capture.spawn_anchor {
            Some("spawn_anchor")
        } else {
            None
        }
    }
}

/// `WorldCollisionGateReporter`.
pub struct State {
    services: Hp2WorldCollisionGateServices,
    touch_only: bool,
    bump_only: bool,
    report_path: String,
    scenario: Scenario,
    scenario_ready: bool,
    scenario_error: String,
    selector_details: Vec<SelectorDetail>,
    movement_observer: MovementObserver,
    steps: Vec<Step>,
    probe: *mut c_void,
    blocker: *mut c_void,
    touch_target: *mut c_void,
    zone_trigger: *mut c_void,
    spawn_anchor: *mut c_void,
}

fn env_non_empty(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

/// `WorldCollisionGateReporter::IsEnabled`.
pub fn is_enabled() -> bool {
    env_non_empty("HP2_WORLD_COLLISION_GATE_TRACE").is_some()
}

/// `WorldCollisionGateReporter::IsTouchEnabled`.
pub fn is_touch_enabled() -> bool {
    env_non_empty("HP2_WORLD_TOUCH_GATE_TRACE").is_some()
}

/// `WorldCollisionGateReporter::IsBumpEnabled`.
pub fn is_bump_enabled() -> bool {
    env_non_empty("HP2_WORLD_BUMP_GATE_TRACE").is_some()
}

/// `WorldCollisionGateReporter::WorldCollisionGateReporter`.
pub fn create(protocol: Protocol, services: Hp2WorldCollisionGateServices) -> Box<State> {
    let (trace_env, scenario_env) = match protocol {
        Protocol::Bump => ("HP2_WORLD_BUMP_GATE_TRACE", "HP2_WORLD_BUMP_GATE_SCENARIO"),
        Protocol::Touch => ("HP2_WORLD_TOUCH_GATE_TRACE", "HP2_WORLD_TOUCH_GATE_SCENARIO"),
        Protocol::Full => (
            "HP2_WORLD_COLLISION_GATE_TRACE",
            "HP2_WORLD_COLLISION_GATE_SCENARIO",
        ),
    };
    let scenario_path = std::env::var(scenario_env).ok();
    let scenario = match protocol {
        Protocol::Bump => parse_bump_scenario(scenario_path.as_deref()),
        Protocol::Touch => parse_touch_scenario(scenario_path.as_deref()),
        Protocol::Full => parse_collision_scenario(scenario_path.as_deref()),
    };
    let scenario_ready = scenario.is_some();
    Box::new(State {
        services,
        touch_only: protocol == Protocol::Touch,
        bump_only: protocol == Protocol::Bump,
        report_path: env_non_empty(trace_env).unwrap_or_default(),
        scenario: scenario.unwrap_or_default(),
        scenario_ready,
        scenario_error: String::new(),
        selector_details: Vec::new(),
        movement_observer: MovementObserver::default(),
        steps: Vec::new(),
        probe: std::ptr::null_mut(),
        blocker: std::ptr::null_mut(),
        touch_target: std::ptr::null_mut(),
        zone_trigger: std::ptr::null_mut(),
        spawn_anchor: std::ptr::null_mut(),
    })
}

impl State {
    fn gate_marker(&self) -> &'static str {
        if self.bump_only {
            "HP2_WORLD_BUMP_GATE"
        } else {
            "HP2_WORLD_COLLISION_GATE"
        }
    }

    fn class_path(&self, class: *mut c_void) -> Vec<u8> {
        object_path_text(class)
    }

    fn actor_path(&self, actor: *mut c_void) -> Vec<u8> {
        object_path_text(actor)
    }

    fn actor_tag(&self, actor: *mut c_void) -> Vec<u8> {
        fetch_name(actor, self.services.actor_tag)
    }

    /// `WorldCollisionGateReporter::LoadActorClass`.
    fn load_actor_class(&self, name: &str) -> *mut c_void {
        unsafe { (self.services.static_load_actor_class)(name.as_ptr(), name.len(), LOAD_NO_FAIL) }
    }

    /// `WorldCollisionGateReporter::ConcreteActorClass`.
    fn concrete_actor_class(&self, role: &Role) -> *mut c_void {
        let requested = self.load_actor_class(&role.class);
        if requested.is_null() {
            return std::ptr::null_mut();
        }
        if !class_is_abstract(requested) {
            return requested;
        }
        let carrier = self.load_actor_class("Engine.Effects");
        if !carrier.is_null() && !class_is_abstract(carrier) {
            carrier
        } else {
            std::ptr::null_mut()
        }
    }

    /// `WorldCollisionGateReporter::FindUniqueClassActor`.
    fn find_unique_class_actor(&mut self, level: *mut c_void, id: &str, class_name: &str) -> *mut c_void {
        let mut match_count = 0;
        let mut found = std::ptr::null_mut();
        let count = level_actor_count(level);
        for slot in 0..count {
            let candidate = level_actor_at(level, slot);
            if candidate.is_null() {
                continue;
            }
            if actor_delete_me(candidate) {
                continue;
            }
            if self.class_path(object_class(candidate)) == class_name.as_bytes() {
                match_count += 1;
                found = candidate;
            }
        }
        let path = if match_count == 1 {
            self.actor_path(found)
        } else {
            Vec::new()
        };
        self.selector_details.push(SelectorDetail {
            id: id.to_string(),
            class: class_name.to_string(),
            path,
            match_count,
        });
        if match_count == 1 { found } else { std::ptr::null_mut() }
    }

    /// `WorldCollisionGateReporter::ResolveMapRoles`.
    fn resolve_map_roles(&mut self, level: *mut c_void) -> bool {
        self.selector_details.clear();
        let spawn_anchor_class = self.scenario.spawn_anchor.class.clone();
        self.spawn_anchor = self.find_unique_class_actor(level, "spawn_anchor", &spawn_anchor_class);
        if self.touch_only || self.bump_only {
            self.sync_event_roles();
            return !self.spawn_anchor.is_null();
        }
        let blocker_class = self.scenario.blocker.class.clone();
        let zone_trigger_class = self.scenario.zone_trigger.class.clone();
        self.blocker = self.find_unique_class_actor(level, "blocker", &blocker_class);
        self.zone_trigger = self.find_unique_class_actor(level, "zone_trigger", &zone_trigger_class);
        self.sync_event_roles();
        !self.blocker.is_null() && !self.zone_trigger.is_null() && !self.spawn_anchor.is_null()
    }

    /// `WorldCollisionGateReporter::ConfigureTransient`.
    fn configure_transient(&self, actor: *mut c_void, role: &Role) {
        unsafe {
            let a = &mut *(actor as *mut AActor);
            a.set_b_static(false);
            a.set_b_movable(true);
            a.set_b_collide_world(role.collide_world);
        }
        unsafe {
            (self.services.actor_set_collision)(
                actor,
                role.collide_actors as i32,
                role.block_actors as i32,
                role.block_players as i32,
            );
            (self.services.actor_set_collision_size)(actor, role.radius, role.height);
        }
    }

    /// `WorldCollisionGateReporter::SpawnTransientRoles`.
    fn spawn_transient_roles(&mut self, level: *mut c_void) -> bool {
        let secondary_role = if self.bump_only {
            self.scenario.blocker.clone()
        } else {
            self.scenario.touch_target.clone()
        };
        let probe_class = self.concrete_actor_class(&self.scenario.probe.clone());
        let secondary_class = self.concrete_actor_class(&secondary_role);
        if !self.scenario.probe.transient || !secondary_role.transient || probe_class.is_null() || secondary_class.is_null()
        {
            return false;
        }
        let anchor_location = actor_location(self.spawn_anchor);
        let probe = unsafe {
            (self.services.spawn_actor)(level, probe_class, anchor_location, zero_rotator(), 1)
        };
        let secondary = unsafe {
            (self.services.spawn_actor)(level, secondary_class, anchor_location, zero_rotator(), 1)
        };
        self.probe = probe;
        if self.bump_only {
            self.blocker = secondary;
        } else {
            self.touch_target = secondary;
        }
        self.sync_event_roles();
        if probe.is_null() || secondary.is_null() {
            return false;
        }
        self.configure_transient(probe, &self.scenario.probe.clone());
        self.configure_transient(secondary, &secondary_role);
        unsafe {
            (self.services.actor_set_collision)(probe, 0, 0, 0);
            (self.services.actor_set_collision)(secondary, 0, 0, 0);
        }
        true
    }

    /// `WorldCollisionGateReporter::RoleName`.
    fn role_name(&self, actor: *mut c_void) -> Option<&'static str> {
        if actor.is_null() {
            return None;
        }
        if actor == self.probe {
            Some("probe")
        } else if actor == self.blocker {
            Some("blocker")
        } else if actor == self.touch_target {
            Some("touch_target")
        } else if actor == self.zone_trigger {
            Some("zone_trigger")
        } else if actor == self.spawn_anchor {
            Some("spawn_anchor")
        } else {
            None
        }
    }

    /// `WorldCollisionGateReporter::RecordEvent`.
    fn record_event(&mut self, recipient: *mut c_void, event: MovementEvent, other: *mut c_void) {
        self.movement_observer.record(event, recipient, other);
    }

    fn sync_event_roles(&self) {
        self.movement_observer.set_roles(
            self.probe,
            self.blocker,
            self.touch_target,
            self.zone_trigger,
            self.spawn_anchor,
        );
    }

    pub fn movement_observer(&self) -> MovementObserver {
        self.movement_observer.clone()
    }

    fn clear_events(&mut self) {
        self.movement_observer.set_recording(false, true);
    }

    fn begin_events(&mut self) {
        self.movement_observer.set_recording(true, true);
    }

    /// `WorldCollisionGateReporter::ZoneJson`.
    fn zone_json(&self, actor: *mut c_void, out: &mut Vec<u8>) {
        let zone = actor_zone(actor);
        if zone.is_null() {
            out.extend_from_slice(b"null");
            return;
        }
        let class = self.class_path(object_class(zone));
        let tag = self.actor_tag(zone);
        out.extend_from_slice(b"{\"class\":");
        json::push_json_string(&class, out);
        out.extend_from_slice(b",\"tag\":");
        json::push_json_string(&tag, out);
        out.push(b'}');
    }

    /// `WorldCollisionGateReporter::TouchingRolesJson`.
    fn touching_roles_json(&self, actor: *mut c_void, out: &mut Vec<u8>) {
        let mut roles: Vec<&'static str> = Vec::new();
        if !actor.is_null() {
            for other in actor_touching(actor) {
                if other == actor {
                    continue;
                }
                if let Some(role) = self.role_name(other) {
                    if role == "probe" || role == "touch_target" || (!self.touch_only && role == "blocker") {
                        roles.push(role);
                    }
                }
            }
        }
        roles.sort_unstable();
        roles.dedup();
        out.push(b'[');
        for (index, role) in roles.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            json::push_json_string(role.as_bytes(), out);
        }
        out.push(b']');
    }

    /// `WorldCollisionGateReporter::ActorJson`.
    fn actor_json(&self, actor: *mut c_void, out: &mut Vec<u8>) {
        if actor.is_null() {
            out.extend_from_slice(b"null");
            return;
        }
        let base_role = self.role_name(actor_base(actor));
        out.extend_from_slice(b"{\"location\":");
        out.extend_from_slice(vector_json(actor_location(actor)).as_bytes());
        out.extend_from_slice(b",\"rotation\":");
        out.extend_from_slice(rotation_json(actor_rotation(actor)).as_bytes());
        out.extend_from_slice(b",\"base_role\":");
        match base_role {
            Some(role) => json::push_json_string(role.as_bytes(), out),
            None => out.extend_from_slice(b"null"),
        }
        out.extend_from_slice(b",\"zone\":");
        self.zone_json(actor, out);
        out.extend_from_slice(b",\"b_just_teleported\":");
        push_bool(actor_just_teleported(actor), out);
        out.extend_from_slice(b",\"delete_marked\":");
        push_bool(actor_delete_me(actor), out);
        out.extend_from_slice(b",\"touching_roles\":");
        self.touching_roles_json(actor, out);
        out.push(b'}');
    }

    /// `WorldCollisionGateReporter::EndStep`.
    #[allow(clippy::too_many_arguments)]
    fn end_step(
        &mut self,
        id: &str,
        request: String,
        moved: bool,
        has_hit_time: bool,
        hit_time: f32,
        blocked: bool,
        hit_actor: *mut c_void,
    ) {
        let events = self.movement_observer.finish_step();
        let mut actors = Vec::new();
        actors.extend_from_slice(b"{\"probe\":");
        self.actor_json(self.probe, &mut actors);
        if self.bump_only {
            actors.extend_from_slice(b",\"blocker\":");
            self.actor_json(self.blocker, &mut actors);
        } else {
            if !self.touch_only {
                actors.extend_from_slice(b",\"blocker\":");
                self.actor_json(self.blocker, &mut actors);
            }
            actors.extend_from_slice(b",\"touch_target\":");
            self.actor_json(self.touch_target, &mut actors);
        }
        actors.push(b'}');
        self.steps.push(Step {
            id: id.to_string(),
            request,
            moved,
            has_hit_time,
            hit_time,
            blocked,
            hit_actor,
            events,
            actors,
        });
    }

    /// `WorldCollisionGateReporter::HasTouchingRelation`.
    fn has_touching_relation(actor: *mut c_void, other: *mut c_void) -> bool {
        if actor.is_null() || other.is_null() {
            return false;
        }
        actor_touching(actor).iter().any(|&touching| touching == other)
    }

    /// `WorldCollisionGateReporter::HasExactBumpEvents`.
    fn has_exact_bump_events(events: &[String]) -> bool {
        events.len() == 2 && events[0] == "Bump:blocker:probe" && events[1] == "Bump:probe:blocker"
    }

    /// `WorldCollisionGateReporter::Run`.
    pub fn run(&mut self, engine: *mut c_void) {
        if !self.scenario_ready {
            self.scenario_error = "invalid_input".to_string();
            eprintln!("<{}> scenario_error=invalid_input", self.gate_marker());
            return;
        }
        let level = game_engine_level(engine);
        if level.is_null() {
            self.scenario_error = "no_level".to_string();
            eprintln!("<{}> scenario_error=no_level", self.gate_marker());
            return;
        }
        if !self.resolve_map_roles(level) {
            self.scenario_error = "role_resolution".to_string();
            eprintln!("<{}> scenario_error=role_resolution", self.gate_marker());
            return;
        }
        if !self.spawn_transient_roles(level) {
            self.scenario_error = "transient_spawn".to_string();
            eprintln!("<{}> scenario_error=transient_spawn", self.gate_marker());
            return;
        }
        if self.bump_only {
            self.run_bump(level);
            return;
        }

        let safe_origin_requested = add(actor_location(self.spawn_anchor), self.scenario.safe_origin_offset);
        let mut safe_origin = safe_origin_requested;
        self.begin_events();
        let probe_extent = unsafe { (self.services.actor_cylinder_extent)(self.probe) };
        let found_safe_origin =
            unsafe { (self.services.find_spot)(level, probe_extent, &mut safe_origin, 1, 0) != 0 };
        let safe_origin_moved = found_safe_origin
            && unsafe { (self.services.far_move_actor)(level, self.probe, safe_origin, 0, 0) != 0 };
        self.end_step(
            "resolve_safe_origin",
            format!("{{\"operation\":\"FindSpot\",\"destination\":{}}}", vector_json(safe_origin_requested)),
            safe_origin_moved,
            false,
            1.0,
            !safe_origin_moved,
            std::ptr::null_mut(),
        );
        if !safe_origin_moved {
            self.scenario_error = "safe_origin".to_string();
            return;
        }
        self.configure_transient(self.probe, &self.scenario.probe.clone());
        self.configure_transient(self.touch_target, &self.scenario.touch_target.clone());

        // Normal FarMove placement causes no touch notifications. It creates
        // the nonblocking overlap candidate used by the following generic
        // MoveActor.
        let touch_dest = add(safe_origin, self.scenario.touch_placement_offset);
        if unsafe { (self.services.far_move_actor)(level, self.touch_target, touch_dest, 0, 0) } == 0 {
            self.scenario_error = "touch_target_placement".to_string();
            return;
        }
        set_actor_just_teleported(self.probe, false);
        self.begin_events();
        let mut touch_hit = check_result(1.0);
        let touch_moved = unsafe {
            (self.services.move_actor)(
                level,
                self.probe,
                self.scenario.touch_delta,
                actor_rotation(self.probe),
                &mut touch_hit,
                0,
                0,
                0,
                0,
            ) != 0
        };
        self.end_step(
            "touch_sweep",
            format!("{{\"operation\":\"MoveActor\",\"delta\":{}}}", vector_json(self.scenario.touch_delta)),
            touch_moved,
            true,
            touch_hit.time,
            touch_hit.time < 1.0,
            touch_hit.actor,
        );

        if self.touch_only {
            return;
        }

        // Keep later movement independent from the intentional persistent
        // touch.
        unsafe {
            (self.services.actor_end_touch)(self.probe, self.touch_target, 0);
            (self.services.actor_set_collision)(self.touch_target, 0, 0, 0);
        }

        let blocker_box = unsafe { (self.services.actor_collision_bounding_box)(self.blocker, 1) };
        let probe_radius = actor_collision_radius(self.probe);
        let mut prepare_destination = FVector {
            x: blocker_box.min.x - probe_radius - self.scenario.block_clearance,
            y: (blocker_box.min.y + blocker_box.max.y) * 0.5,
            z: (blocker_box.min.z + blocker_box.max.z) * 0.5,
        };
        let requested_prepare_destination = prepare_destination;
        // FindSpot performs the required world and actor admission test.
        // Reusing its resolved location with bNoCheck preserves that one
        // checked result instead of making FarMoveActor perturb/reject it a
        // second time.
        let found_prepare_destination =
            unsafe { (self.services.find_spot)(level, probe_extent, &mut prepare_destination, 1, 0) != 0 };
        self.begin_events();
        let far_move_prepare_destination = found_prepare_destination
            && unsafe { (self.services.far_move_actor)(level, self.probe, prepare_destination, 0, 1) != 0 };
        let prepare_moved = far_move_prepare_destination;
        self.end_step(
            "prepare_block",
            format!(
                "{{\"operation\":\"FarMoveActor\",\"destination\":{}}}",
                vector_json(prepare_destination)
            ),
            prepare_moved,
            false,
            0.0,
            !prepare_moved,
            std::ptr::null_mut(),
        );
        if !prepare_moved {
            self.scenario_error = "prepare_block".to_string();
            eprintln!(
                "<HP2_WORLD_COLLISION_GATE> scenario_error=prepare_block find_spot={} far_move={} requested=({},{},{}) resolved=({},{},{})",
                found_prepare_destination as i32,
                far_move_prepare_destination as i32,
                json::format_fixed(requested_prepare_destination.x as f64, 4),
                json::format_fixed(requested_prepare_destination.y as f64, 4),
                json::format_fixed(requested_prepare_destination.z as f64, 4),
                json::format_fixed(prepare_destination.x as f64, 4),
                json::format_fixed(prepare_destination.y as f64, 4),
                json::format_fixed(prepare_destination.z as f64, 4),
            );
            return;
        }
        set_actor_just_teleported(self.probe, false);
        self.begin_events();
        let mut block_hit = check_result(1.0);
        let block_moved = unsafe {
            (self.services.move_actor)(
                level,
                self.probe,
                self.scenario.block_delta,
                actor_rotation(self.probe),
                &mut block_hit,
                0,
                0,
                0,
                0,
            ) != 0
        };
        self.end_step(
            "block_sweep",
            format!("{{\"operation\":\"MoveActor\",\"delta\":{}}}", vector_json(self.scenario.block_delta)),
            block_moved,
            true,
            block_hit.time,
            block_hit.time < 1.0,
            block_hit.actor,
        );
        if block_hit.actor != self.blocker || block_hit.time >= 1.0 {
            self.scenario_error = "block_sweep".to_string();
            return;
        }

        self.begin_events();
        let far_moved = unsafe { (self.services.far_move_actor)(level, self.probe, safe_origin, 0, 0) != 0 };
        self.end_step(
            "far_move",
            format!("{{\"operation\":\"FarMoveActor\",\"destination\":{}}}", vector_json(safe_origin)),
            far_moved,
            false,
            0.0,
            !far_moved,
            std::ptr::null_mut(),
        );
        if !far_moved {
            self.scenario_error = "far_move".to_string();
            return;
        }

        unsafe {
            (self.services.actor_set_base)(self.probe, self.blocker);
        }
        self.begin_events();
        let mut carry_hit = check_result(1.0);
        let carry_moved = unsafe {
            (self.services.move_actor)(
                level,
                self.blocker,
                self.scenario.carry_delta,
                actor_rotation(self.blocker),
                &mut carry_hit,
                0,
                0,
                0,
                0,
            ) != 0
        };
        self.end_step(
            "set_base_and_carry",
            format!("{{\"operation\":\"MoveActor\",\"delta\":{}}}", vector_json(self.scenario.carry_delta)),
            carry_moved,
            true,
            carry_hit.time,
            carry_hit.time < 1.0,
            carry_hit.actor,
        );
    }

    /// `WorldCollisionGateReporter::RunBump`.
    fn run_bump(&mut self, level: *mut c_void) {
        let safe_origin_requested = add(actor_location(self.spawn_anchor), self.scenario.safe_origin_offset);
        let mut safe_origin = safe_origin_requested;
        self.begin_events();
        let probe_extent = unsafe { (self.services.actor_cylinder_extent)(self.probe) };
        let found_safe_origin =
            unsafe { (self.services.find_spot)(level, probe_extent, &mut safe_origin, 1, 0) != 0 };
        let safe_origin_moved = found_safe_origin
            && unsafe { (self.services.far_move_actor)(level, self.probe, safe_origin, 0, 0) != 0 };
        self.end_step(
            "resolve_safe_origin",
            format!("{{\"operation\":\"FindSpot\",\"destination\":{}}}", vector_json(safe_origin_requested)),
            safe_origin_moved,
            false,
            1.0,
            !safe_origin_moved,
            std::ptr::null_mut(),
        );
        if !safe_origin_moved {
            self.scenario_error = "safe_origin".to_string();
            return;
        }

        self.configure_transient(self.probe, &self.scenario.probe.clone());
        self.configure_transient(self.blocker, &self.scenario.blocker.clone());
        let blocker_dest = add(safe_origin, self.scenario.bump_placement_offset);
        if unsafe { (self.services.far_move_actor)(level, self.blocker, blocker_dest, 0, 0) } == 0 {
            self.scenario_error = "blocker_placement".to_string();
            return;
        }

        // FarMove is setup only. The measured sweep begins with neither a
        // teleport flag nor setup callbacks carried into its event ledger.
        set_actor_just_teleported(self.probe, false);
        self.clear_events();
        self.begin_events();
        let mut block_hit = check_result(1.0);
        let block_moved = unsafe {
            (self.services.move_actor)(
                level,
                self.probe,
                self.scenario.block_delta,
                actor_rotation(self.probe),
                &mut block_hit,
                0,
                0,
                0,
                0,
            ) != 0
        };
        self.end_step(
            "block_sweep",
            format!("{{\"operation\":\"MoveActor\",\"delta\":{}}}", vector_json(self.scenario.block_delta)),
            block_moved,
            true,
            block_hit.time,
            block_hit.time < 1.0,
            block_hit.actor,
        );
        let block_step_events = self.steps.last().expect("end_step just pushed a step").events.clone();
        if block_hit.actor != self.blocker
            || block_hit.time <= 0.0
            || block_hit.time >= 1.0
            || !Self::has_exact_bump_events(&block_step_events)
            || Self::has_touching_relation(self.probe, self.blocker)
            || Self::has_touching_relation(self.blocker, self.probe)
        {
            self.scenario_error = "block_sweep".to_string();
        }
    }

    /// `WorldCollisionGateReporter::OnActorBump`.
    pub fn on_actor_bump(&mut self, recipient: *mut c_void, other: *mut c_void) {
        self.record_event(recipient, MovementEvent::Bump, other);
    }

    /// `WorldCollisionGateReporter::OnActorTouch`.
    pub fn on_actor_touch(&mut self, recipient: *mut c_void, other: *mut c_void) {
        self.record_event(recipient, MovementEvent::Touch, other);
    }

    /// `WorldCollisionGateReporter::OnActorUnTouch`.
    pub fn on_actor_untouch(&mut self, recipient: *mut c_void, other: *mut c_void) {
        self.record_event(recipient, MovementEvent::UnTouch, other);
    }

    fn events_json(&self, step: &Step, out: &mut Vec<u8>) {
        out.push(b'[');
        for (index, event) in step.events.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            json::push_json_string(event.as_bytes(), out);
        }
        out.push(b']');
    }

    fn step_json(&self, step: &Step, out: &mut Vec<u8>) {
        let hit_role = self.role_name(step.hit_actor);
        out.extend_from_slice(b"{\"id\":");
        json::push_json_string(step.id.as_bytes(), out);
        out.extend_from_slice(b",\"request\":");
        out.extend_from_slice(step.request.as_bytes());
        out.extend_from_slice(b",\"result\":{\"moved\":");
        push_bool(step.moved, out);
        out.extend_from_slice(b",\"hit_time\":");
        let hit_time = json::format_fixed(if step.has_hit_time { step.hit_time } else { 1.0 } as f64, 4);
        out.extend_from_slice(hit_time.as_bytes());
        out.extend_from_slice(b",\"blocked\":");
        push_bool(step.blocked, out);
        out.extend_from_slice(b",\"hit_role\":");
        match hit_role {
            Some(role) => json::push_json_string(role.as_bytes(), out),
            None => out.extend_from_slice(b"null"),
        }
        out.extend_from_slice(b"},\"actors\":");
        out.extend_from_slice(&step.actors);
        out.extend_from_slice(b",\"events\":");
        self.events_json(step, out);
        out.push(b'}');
    }

    fn role_json(&self, role: &Role, out: &mut Vec<u8>) {
        let mut label = Vec::new();
        if role.transient {
            label.extend_from_slice(b"transient:");
        }
        label.extend_from_slice(role.class.as_bytes());
        json::push_json_string(&label, out);
    }

    fn scenario_error_json(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(b"{\"code\":");
        json::push_json_string(self.scenario_error.as_bytes(), out);
        out.extend_from_slice(b",\"selectors\":[");
        for (index, detail) in self.selector_details.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            out.extend_from_slice(b"{\"id\":");
            json::push_json_string(detail.id.as_bytes(), out);
            out.extend_from_slice(b",\"class\":");
            json::push_json_string(detail.class.as_bytes(), out);
            out.extend_from_slice(b",\"path\":");
            if detail.path.is_empty() {
                out.extend_from_slice(b"null");
            } else {
                json::push_json_string(&detail.path, out);
            }
            out.extend_from_slice(b",\"match_count\":");
            out.extend_from_slice(detail.match_count.to_string().as_bytes());
            out.push(b'}');
        }
        out.extend_from_slice(b"]}");
    }

    /// `WorldCollisionGateReporter::Serialize`.
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"{\"version\":1,\"fixture\":");
        json::push_json_string(self.scenario.fixture.as_bytes(), &mut out);
        out.extend_from_slice(b",\"map\":{\"relative\":");
        json::push_json_string(self.scenario.map_relative.as_bytes(), &mut out);
        out.extend_from_slice(b",\"sha256\":");
        json::push_json_string(self.scenario.map_sha256.as_bytes(), &mut out);
        out.extend_from_slice(b"},\"fixed_dt\":");
        out.extend_from_slice(json::format_g(self.scenario.fixed_delta_seconds as f64, 9).as_bytes());
        out.extend_from_slice(b",\"requested_ticks\":");
        out.extend_from_slice(self.scenario.requested_ticks.to_string().as_bytes());
        out.extend_from_slice(b",\"roles\":{\"probe\":");
        self.role_json(&self.scenario.probe, &mut out);
        if self.bump_only {
            out.extend_from_slice(b",\"blocker\":");
            self.role_json(&self.scenario.blocker, &mut out);
        } else {
            if !self.touch_only {
                out.extend_from_slice(b",\"blocker\":");
                self.role_json(&self.scenario.blocker, &mut out);
                out.extend_from_slice(b",\"zone_trigger\":");
                self.role_json(&self.scenario.zone_trigger, &mut out);
            }
            out.extend_from_slice(b",\"touch_target\":");
            self.role_json(&self.scenario.touch_target, &mut out);
        }
        out.extend_from_slice(b",\"spawn_anchor\":");
        self.role_json(&self.scenario.spawn_anchor, &mut out);
        out.extend_from_slice(b"},\"steps\":[");
        for (index, step) in self.steps.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            self.step_json(step, &mut out);
        }
        out.extend_from_slice(b"],\"random_calls\":[]");
        if !self.scenario_error.is_empty() {
            out.extend_from_slice(b",\"scenario_error\":");
            self.scenario_error_json(&mut out);
        }
        out.extend_from_slice(b"}\n");
        out
    }

    /// `WorldCollisionGateReporter::Flush`.
    pub fn flush(&self) {
        let marker = self.gate_marker();
        let document = self.serialize();
        match write_report(self.report_path.as_bytes(), &document) {
            Ok(()) => {
                println!("<{marker}> report={} steps={}", self.report_path, self.steps.len());
            }
            Err(ReportWriteError::Unwritable) => {
                eprintln!("<{marker}> report_error=unwritable path={}", self.report_path);
            }
            Err(ReportWriteError::WriteFailed) => {
                eprintln!("<{marker}> report_error=write_failed path={}", self.report_path);
            }
        }
    }
}

const LOAD_NO_FAIL: u32 = 0x0000_0008;

fn class_is_abstract(class: *mut c_void) -> bool {
    if class.is_null() {
        return false;
    }
    unsafe { (*(class as *mut UClass)).class_flags & CLASS_ABSTRACT != 0 }
}

fn object_class(object: *mut c_void) -> *mut c_void {
    if object.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (*(object as *mut UObject)).class }
}

fn actor_delete_me(actor: *mut c_void) -> bool {
    unsafe { (*(actor as *mut AActor)).b_delete_me() }
}

fn actor_just_teleported(actor: *mut c_void) -> bool {
    unsafe { (*(actor as *mut AActor)).b_just_teleported() }
}

fn set_actor_just_teleported(actor: *mut c_void, value: bool) {
    unsafe { (*(actor as *mut AActor)).set_b_just_teleported(value) }
}

fn actor_location(actor: *mut c_void) -> FVector {
    unsafe { (*(actor as *mut AActor)).location }
}

fn actor_rotation(actor: *mut c_void) -> FRotator {
    unsafe { (*(actor as *mut AActor)).rotation }
}

fn actor_base(actor: *mut c_void) -> *mut c_void {
    unsafe { (*(actor as *mut AActor)).base }
}

fn actor_zone(actor: *mut c_void) -> *mut c_void {
    if actor.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (*(actor as *mut AActor)).region.zone }
}

fn actor_collision_radius(actor: *mut c_void) -> f32 {
    unsafe { (*(actor as *mut AActor)).collision_radius }
}

fn actor_touching(actor: *mut c_void) -> [*mut c_void; TOUCHING_SLOTS] {
    unsafe { (*(actor as *mut AActor)).touching }
}

fn add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn push_bool(value: bool, out: &mut Vec<u8>) {
    out.extend_from_slice(if value { b"true" } else { b"false" });
}

fn vector_json(value: FVector) -> String {
    format!(
        "[{},{},{}]",
        json::format_fixed(value.x as f64, 4),
        json::format_fixed(value.y as f64, 4),
        json::format_fixed(value.z as f64, 4)
    )
}

fn rotation_json(value: FRotator) -> String {
    format!("[{},{},{}]", value.pitch, value.yaw, value.roll)
}

fn zero_rotator() -> FRotator {
    FRotator {
        pitch: 0,
        yaw: 0,
        roll: 0,
    }
}

/// A fresh out-param for a `MoveActor` call: C++ default-constructs
/// `FCheckResult Hit(1.0)`, i.e. every field but `Time` zeroed and `Item`
/// set to `INDEX_NONE` (`UnPrim.h`'s `FCheckResult(FLOAT,FCheckResult*)`).
fn check_result(time: f32) -> FCheckResult {
    FCheckResult {
        next: std::ptr::null_mut(),
        actor: std::ptr::null_mut(),
        location: ZERO_VECTOR,
        normal: ZERO_VECTOR,
        primitive: std::ptr::null_mut(),
        time,
        item: -1,
    }
}
