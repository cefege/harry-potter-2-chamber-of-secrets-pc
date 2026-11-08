//! `StaticBspProbeReporter` and its scenario parser, ported from
//! `HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp:2243-2951`
//! (`StaticBspProbeConfig`/`StaticMovementGateScenario` through the end of
//! `StaticBspProbeReporter`). `SDLLaunch.cpp`'s thin adapter keeps the class
//! name and `IsEnabled`/ctor/`Run`/`Flush` signatures so the untouched
//! `FMainLoopState`/`MainLoop` call sites (`SDLLaunch.cpp:5389+`) keep
//! compiling.
//!
//! Dropped as dead code (never read by `Serialize` nor by any live control
//! flow in `Run`, confirmed by a full read of the C++ class body and by
//! `grep`ing the whole file for call sites of `RunStaticLineCases`, which
//! has none): `StaticBspSweepCase`, `FaceCenter`, `PrepareSweepCase`,
//! `PreparePendingSweepCase`, `TraceStaticLine`, `IsLateralStaticHit`,
//! `IsFloorStaticHit`, `IsStaticHit`, `IsEdgeCornerStaticHit`,
//! `DiscoverSideHit`, `DiscoverEdgeCornerHit`, `RunStaticLineCases`,
//! `SweepCaseJson`, and the `NoHitCase`/`FloorHitCase`/`SideHitCase`/
//! `EdgeCornerCase`/`SweepCasesFeasible` fields that only that dead group
//! writes to.
//!
//! `Level->Model->PointCheck(...)` (`SDLLaunch.cpp:2528-2530`) IS kept and
//! called in its original position even though its result
//! (`PointCheckClear`) is never read by any live code afterward: the
//! contract requires the same engine calls in the same order, not just the
//! same observable `Run`/`Serialize` output, so dropping a live engine call
//! is not a "dead code" deletion the way the sweep-case group above is.
//!
//! `Hit.Actor->IsA(ALevelInfo::StaticClass())` (`SDLLaunch.cpp:2572`) is
//! ported as a dedicated `actor_is_a_level_info` leaf mirroring
//! `Hp2LevelServices.cpp`'s `CoreActorIsALevelInfo`, not a by-name
//! `StaticLoadClass("Engine.LevelInfo")`: the C++ never calls
//! `StaticLoadClass` for this check, and that call has different
//! observable side effects (package load, linear name-table lookup) than a
//! direct `ALevelInfo::StaticClass()` read.

use crate::reporters::json;
use crate::reporters::trace_services::{self, ReportWriteError, write_report};
use crate::reporters::world_collision_gate::Role;
use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FCheckResult, FRotator, UObject};
use std::ffi::c_void;

const LOAD_CLASS_FLAGS: u32 = 0;
const NAME_BUFFER_CAP: usize = 1024;

const ZERO_VECTOR: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

const ZERO_ROTATOR: FRotator = FRotator {
    pitch: 0,
    yaw: 0,
    roll: 0,
};

/// `StaticMovementGateScenario` (`SDLLaunch.cpp:2247-2260`).
#[derive(Clone)]
pub(crate) struct Scenario {
    pub fixture: String,
    pub map_relative: String,
    pub map_sha256: String,
    pub fixed_delta_seconds: f32,
    pub requested_ticks: i32,
    pub probe: Role,
    pub spawn_anchor: Role,
    pub origin_offset: FVector,
    pub sweep_delta: FVector,
    pub sweep_rotation: FRotator,
    pub far_move_rotation: FRotator,
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
            spawn_anchor: Role::default(),
            origin_offset: ZERO_VECTOR,
            sweep_delta: ZERO_VECTOR,
            sweep_rotation: ZERO_ROTATOR,
            far_move_rotation: ZERO_ROTATOR,
        }
    }
}

/// `ParseStaticMovementGateSelector`.
fn parse_role_selector(selectors: &[u8], name: &str) -> Option<Role> {
    const KEYS: [&str; 2] = ["class", "cardinality"];
    let selector = json::object_member(selectors, name)?;
    if !json::has_exact_keys(&selector, &KEYS) {
        return None;
    }
    let role = crate::reporters::world_collision_gate::parse_selector(selectors, name)?;
    if role.class != "Engine.PlayerStart" {
        return None;
    }
    Some(role)
}

/// `ParseStaticMovementGateTransient`.
fn parse_transient(transients: &[u8]) -> Option<Role> {
    const KEYS: [&str; 7] = [
        "class",
        "collision_radius",
        "collision_height",
        "collide_world",
        "collide_actors",
        "block_actors",
        "block_players",
    ];
    let transient = json::object_member(transients, "probe")?;
    if !json::has_exact_keys(&transient, &KEYS) {
        return None;
    }
    Some(Role {
        class: json::string_member(&transient, "class")?,
        transient: true,
        radius: json::float_member(&transient, "collision_radius")?,
        height: json::float_member(&transient, "collision_height")?,
        collide_world: json::bool_member(&transient, "collide_world")?,
        collide_actors: json::bool_member(&transient, "collide_actors")?,
        block_actors: json::bool_member(&transient, "block_actors")?,
        block_players: json::bool_member(&transient, "block_players")?,
    })
}

/// `ParseStaticMovementGateRotation`.
fn parse_rotation(object: &[u8], key: &str) -> Option<FRotator> {
    let value = json::vector_member(object, key)?;
    if value.x != (value.x as i32) as f32 || value.y != (value.y as i32) as f32 || value.z != (value.z as i32) as f32 {
        return None;
    }
    Some(FRotator {
        pitch: value.x as i32,
        yaw: value.y as i32,
        roll: value.z as i32,
    })
}

/// `ValidateStaticMovementGateOperations`.
fn validate_operations(document: &[u8], out: &mut Scenario) -> bool {
    const RESOLVE_KEYS: [&str; 6] = ["id", "operation", "actor", "relative_to", "offset", "check_actors"];
    const SWEEP_KEYS: [&str; 5] = ["id", "operation", "actor", "delta", "rotation"];
    const FAR_MOVE_KEYS: [&str; 5] = ["id", "operation", "actor", "destination", "rotation"];
    (|| -> Option<()> {
        let values = json::object_array_member(document, "operations")?;
        if values.len() != 3
            || !json::has_exact_keys(&values[0], &RESOLVE_KEYS)
            || !json::has_exact_keys(&values[1], &SWEEP_KEYS)
            || !json::has_exact_keys(&values[2], &FAR_MOVE_KEYS)
        {
            return None;
        }
        if !json::string_equals(&values[0], "id", "resolve_origin")
            || !json::string_equals(&values[0], "operation", "FindSpot")
            || !json::string_equals(&values[0], "actor", "probe")
            || !json::string_equals(&values[0], "relative_to", "spawn_anchor")
        {
            return None;
        }
        out.origin_offset = json::vector_member(&values[0], "offset")?;
        if json::bool_member(&values[0], "check_actors")? {
            return None;
        }
        if !json::string_equals(&values[1], "id", "static_sweep")
            || !json::string_equals(&values[1], "operation", "MoveActor")
            || !json::string_equals(&values[1], "actor", "probe")
        {
            return None;
        }
        out.sweep_delta = json::vector_member(&values[1], "delta")?;
        out.sweep_rotation = parse_rotation(&values[1], "rotation")?;
        if !json::string_equals(&values[2], "id", "static_far_move")
            || !json::string_equals(&values[2], "operation", "FarMoveActor")
            || !json::string_equals(&values[2], "actor", "probe")
        {
            return None;
        }
        let destination = json::string_member(&values[2], "destination")?;
        if destination != "resolved_origin" {
            return None;
        }
        out.far_move_rotation = parse_rotation(&values[2], "rotation")?;
        Some(())
    })()
    .is_some()
        && out.origin_offset.x == 0.0
        && out.origin_offset.y == 0.0
        && out.origin_offset.z == 26.0
        && out.sweep_delta.x == 0.0
        && out.sweep_delta.y == 0.0
        && out.sweep_delta.z == -64.0
        && out.sweep_rotation.pitch == 10
        && out.sweep_rotation.yaw == 20
        && out.sweep_rotation.roll == 30
        && out.far_move_rotation.pitch == 40
        && out.far_move_rotation.yaw == 50
        && out.far_move_rotation.roll == 60
}

/// `ParseStaticMovementGateScenario`.
pub(crate) fn parse_scenario(filename: Option<&str>) -> Option<Scenario> {
    const ROOT_KEYS: [&str; 9] = [
        "version",
        "fixture",
        "map",
        "fixed_dt",
        "requested_ticks",
        "roles",
        "selectors",
        "transients",
        "operations",
    ];
    const MAP_KEYS: [&str; 2] = ["relative", "sha256"];
    const ROLE_KEYS: [&str; 2] = ["probe", "spawn_anchor"];
    const SINGLE_SPAWN_ANCHOR: [&str; 1] = ["spawn_anchor"];
    const SINGLE_PROBE: [&str; 1] = ["probe"];
    const EXPECTED_MAP: &str = "Maps/Studies/TriggerTest2.unr";
    const EXPECTED_MAP_SHA256: &str = "fffa53fee540d756b9ff6bbc872c64d3bc81eca47f366f3d4071196fae2546fc";

    let document = json::read_document(filename)?;
    if !json::has_exact_keys(&document, &ROOT_KEYS) {
        return None;
    }
    let version = json::int_member(&document, "version")?;
    if version != 1 {
        return None;
    }
    let mut out = Scenario {
        fixture: json::string_member(&document, "fixture")?,
        ..Scenario::default()
    };
    if out.fixture != "TriggerTest2StaticMovement" {
        return None;
    }
    let map = json::object_member(&document, "map")?;
    if !json::has_exact_keys(&map, &MAP_KEYS) {
        return None;
    }
    out.map_relative = json::string_member(&map, "relative")?;
    if out.map_relative != EXPECTED_MAP {
        return None;
    }
    out.map_sha256 = json::string_member(&map, "sha256")?;
    if out.map_sha256 != EXPECTED_MAP_SHA256 {
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
    let roles = json::object_member(&document, "roles")?;
    if !json::has_exact_keys(&roles, &ROLE_KEYS) {
        return None;
    }
    let probe_role = json::string_member(&roles, "probe")?;
    if probe_role != "transient:Engine.Effects" {
        return None;
    }
    let spawn_anchor_role = json::string_member(&roles, "spawn_anchor")?;
    if spawn_anchor_role != "Engine.PlayerStart" {
        return None;
    }
    let selectors = json::object_member(&document, "selectors")?;
    if !json::has_exact_keys(&selectors, &SINGLE_SPAWN_ANCHOR) {
        return None;
    }
    let transients = json::object_member(&document, "transients")?;
    if !json::has_exact_keys(&transients, &SINGLE_PROBE) {
        return None;
    }
    out.spawn_anchor = parse_role_selector(&selectors, "spawn_anchor")?;
    out.probe = parse_transient(&transients)?;
    if !validate_operations(&document, &mut out) {
        return None;
    }
    if out.probe.class != "Engine.Effects"
        || out.probe.radius != 16.0
        || out.probe.height != 24.0
        || out.probe.collide_actors
        || !out.probe.collide_world
        || out.probe.block_actors
        || out.probe.block_players
    {
        return None;
    }
    Some(out)
}

/// `StaticBspProbeConfig`.
pub(crate) fn parse_config(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

/// Engine leaves `StaticBspProbeReporter` needs. Published once by
/// `SDLLaunch.cpp`'s adapter and captured by value at construction (same
/// rationale as `world_collision_gate::Hp2WorldCollisionGateServices`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2StaticBspProbeServices {
    /// `Level->Model`; null if the level has no static BSP.
    pub level_model: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `Level->DestroyActor(Actor)`; return value unused by `Run`.
    pub level_destroy_actor: unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void),
    /// `UObject::StaticLoadClass(AActor::StaticClass(), NULL, Name, NULL, Flags, NULL)`;
    /// `name`/`name_len` is UTF-8, not NUL-terminated.
    pub static_load_actor_class:
        unsafe extern "C" fn(name: *const u8, name_len: usize, flags: u32) -> *mut c_void,
    /// `Actor->IsA(ALevelInfo::StaticClass())`; `Actor` may be null. Resolves
    /// the class natively (no `StaticLoadClass` package/name lookup), same
    /// technique as `Hp2LevelServices.cpp`'s `CoreActorIsALevelInfo`.
    pub actor_is_a_level_info: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Level->Model->PointCheck(Hit, SourceActor, Location, Extent, ExtraNodeFlags)`;
    /// return value and `*hit` are unused by `Run` (kept only to preserve the
    /// engine's call order/side effects).
    pub model_point_check: unsafe extern "C" fn(
        model: *mut c_void,
        hit: *mut FCheckResult,
        source_actor: *mut c_void,
        location: FVector,
        extent: FVector,
        extra_node_flags: u32,
    ) -> i32,
    /// `appToAnsi(*Actor->Tag)`; writes up to `cap` bytes (no NUL) into
    /// `buf`, returns the byte length actually available (may exceed
    /// `cap`, same truncation contract as `snprintf`'s return value).
    pub actor_tag: unsafe extern "C" fn(actor: *mut c_void, buf: *mut u8, cap: usize) -> i32,
    /// `Actor->SetCollision(CollideActors, BlockActors, BlockPlayers)`.
    pub actor_set_collision:
        unsafe extern "C" fn(actor: *mut c_void, collide_actors: i32, block_actors: i32, block_players: i32),
    /// `Actor->SetCollisionSize(Radius, Height)`.
    pub actor_set_collision_size: unsafe extern "C" fn(actor: *mut c_void, radius: f32, height: f32),
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

struct Step {
    id: &'static str,
    request: String,
    result: String,
    actor: Vec<u8>,
}

/// `StaticBspProbeReporter`.
pub struct State {
    services: Hp2StaticBspProbeServices,
    report_path: String,
    configured: bool,
    scenario: Scenario,
    scenario_ready: bool,
    steps: Vec<Step>,
    error: String,
    feasible: bool,
}

fn env_non_empty(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

/// `StaticBspProbeReporter::IsEnabled`.
pub fn is_enabled() -> bool {
    parse_config(env_non_empty("HP2_STATIC_BSP_PROBE").as_deref()).is_some()
}

/// `StaticBspProbeReporter::StaticBspProbeReporter`.
pub fn create(services: Hp2StaticBspProbeServices) -> Box<State> {
    let report_path = parse_config(env_non_empty("HP2_STATIC_BSP_PROBE").as_deref());
    let scenario = parse_scenario(env_non_empty("HP2_STATIC_MOVEMENT_GATE_SCENARIO").as_deref());
    Box::new(State {
        services,
        configured: report_path.is_some(),
        report_path: report_path.unwrap_or_default(),
        scenario_ready: scenario.is_some(),
        scenario: scenario.unwrap_or_default(),
        steps: Vec::new(),
        error: String::new(),
        feasible: false,
    })
}

fn object_class(object: *mut c_void) -> *mut c_void {
    if object.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (*(object as *mut UObject)).class }
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

/// `Actor->IsA(Class)`, replacing the old dedicated `actor_is_a` leaf by
/// composing the already-published generic `object_class`/`class_super`
/// leaves (`UObject::IsA` walks `GetClass()`'s super chain exactly as
/// `UClass::IsChildOf` does; see `UnType.h`/`UnClass.h`).
fn actor_is_a(actor: *mut c_void, class: *mut c_void) -> bool {
    match unsafe { trace_services::published_services() } {
        Ok(svc) => unsafe { trace_services::class_is_child_of(&svc, object_class(actor), class) },
        Err(_) => false,
    }
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

fn actor_zone(actor: *mut c_void) -> *mut c_void {
    if actor.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { (*(actor as *mut AActor)).region.zone }
}

fn add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn size_squared(value: FVector) -> f32 {
    value.x * value.x + value.y * value.y + value.z * value.z
}

fn sub(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

/// A fresh out-param for a `MoveActor`-shaped call: C++ default-constructs
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

impl State {
    /// `StaticBspProbeReporter::Run`.
    pub fn run(&mut self, engine: *mut c_void) {
        if !self.configured {
            self.error = "invalid_output_path".to_string();
            return;
        }
        if !self.scenario_ready {
            self.error = "invalid_input".to_string();
            return;
        }
        let level = game_engine_level(engine);
        let model = if level.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { (self.services.level_model)(level) }
        };
        if level.is_null() || model.is_null() {
            self.error = if level.is_null() { "no_level" } else { "no_static_bsp" }.to_string();
            return;
        }
        let player_start_class = self.load_actor_class("Engine.PlayerStart");
        if player_start_class.is_null() {
            self.error = "player_start_class".to_string();
            return;
        }
        let mut player_start_matches = 0;
        let mut player_start = std::ptr::null_mut();
        let count = level_actor_count(level);
        for slot in 0..count {
            let candidate = level_actor_at(level, slot);
            if candidate.is_null() || actor_delete_me(candidate) {
                continue;
            }
            if actor_is_a(candidate, player_start_class) {
                player_start_matches += 1;
                player_start = candidate;
            }
        }
        if player_start_matches != 1 {
            self.error = "player_start_not_unique".to_string();
            return;
        }
        let requested_origin = add(actor_location(player_start), self.scenario.origin_offset);
        let probe_extent = FVector {
            x: self.scenario.probe.radius,
            y: self.scenario.probe.radius,
            z: self.scenario.probe.height,
        };
        let mut point_check_hit = check_result(0.0);
        unsafe {
            (self.services.model_point_check)(
                model,
                &mut point_check_hit,
                std::ptr::null_mut(),
                requested_origin,
                probe_extent,
                0,
            )
        };
        let mut resolved_origin = requested_origin;
        let find_spot_succeeded = unsafe {
            (self.services.find_spot)(level, probe_extent, &mut resolved_origin, 0, 0) != 0
        };
        let effects_class = self.load_actor_class("Engine.Effects");
        if effects_class.is_null() {
            self.error = "effects_class".to_string();
            return;
        }
        let carrier = unsafe {
            (self.services.spawn_actor)(level, effects_class, resolved_origin, zero_rotator(), 1)
        };
        if carrier.is_null() {
            self.error = "effects_spawn".to_string();
            return;
        }
        unsafe {
            let a = &mut *(carrier as *mut AActor);
            a.set_b_static(false);
            a.set_b_movable(true);
            a.set_b_collide_world(self.scenario.probe.collide_world);
        }
        unsafe {
            (self.services.actor_set_collision)(carrier, 0, 0, 0);
            (self.services.actor_set_collision_size)(carrier, self.scenario.probe.radius, self.scenario.probe.height);
        }
        self.add_step(
            "resolve_origin",
            format!(
                "{{\"operation\":\"FindSpot\",\"requested_origin\":{},\"extent\":{},\"check_actors\":false}}",
                vector_json(requested_origin),
                vector_json(probe_extent),
            ),
            format!(
                "{{\"found\":{},\"resolved_origin\":{}}}",
                find_spot_succeeded,
                vector_json(resolved_origin),
            ),
            carrier,
        );
        if !find_spot_succeeded || size_squared(sub(resolved_origin, requested_origin)) > 0.0001 {
            self.error = "resolve_origin".to_string();
            unsafe { (self.services.level_destroy_actor)(level, carrier) };
            return;
        }
        set_actor_just_teleported(carrier, false);
        let mut hit = check_result(1.0);
        let sweep_moved = unsafe {
            (self.services.move_actor)(
                level,
                carrier,
                self.scenario.sweep_delta,
                self.scenario.sweep_rotation,
                &mut hit,
                0,
                0,
                0,
                0,
            ) != 0
        };
        let sweep_blocked = hit.time < 1.0;
        let sweep_hit_level_info =
            !hit.actor.is_null() && unsafe { (self.services.actor_is_a_level_info)(hit.actor) != 0 };
        let sweep_hit_static_bsp = sweep_hit_level_info && hit.primitive == model;
        self.add_step(
            "static_sweep",
            format!(
                "{{\"operation\":\"MoveActor\",\"delta\":{},\"rotation\":{}}}",
                vector_json(self.scenario.sweep_delta),
                rotation_json(self.scenario.sweep_rotation),
            ),
            format!(
                "{{\"moved\":{},\"hit_time\":{},\"blocked\":{},\"hit_actor\":null,\"hit_primitive\":{},\"location\":{},\"normal\":{},\"item\":{}}}",
                sweep_moved,
                number_json(hit.time),
                sweep_blocked,
                if sweep_hit_static_bsp { "\"static_bsp\"" } else { "null" },
                vector_json(hit.location),
                vector_json(hit.normal),
                hit.item,
            ),
            carrier,
        );
        self.feasible = sweep_moved
            && sweep_blocked
            && sweep_hit_static_bsp
            && (hit.time - 0.742_187_5).abs() < 0.0001
            && size_squared(sub(hit.location, FVector { x: -3168.0, y: -7760.0, z: -231.5 })) < 0.0001
            && size_squared(sub(actor_location(carrier), FVector { x: -3168.0, y: -7760.0, z: -229.5 })) < 0.0001
            && hit.normal.x == 0.0
            && hit.normal.y == 0.0
            && hit.normal.z == 1.0
            && hit.item == 26;
        if !self.feasible {
            self.error = "static_sweep".to_string();
            unsafe { (self.services.level_destroy_actor)(level, carrier) };
            return;
        }
        unsafe { (*(carrier as *mut AActor)).rotation = self.scenario.far_move_rotation };
        let far_moved =
            unsafe { (self.services.far_move_actor)(level, carrier, resolved_origin, 0, 0) != 0 };
        self.add_step(
            "static_far_move",
            format!(
                "{{\"operation\":\"FarMoveActor\",\"destination\":{},\"rotation\":{}}}",
                vector_json(resolved_origin),
                rotation_json(self.scenario.far_move_rotation),
            ),
            format!("{{\"moved\":{far_moved}}}"),
            carrier,
        );
        if !far_moved {
            self.error = "static_far_move".to_string();
        }
        unsafe { (self.services.level_destroy_actor)(level, carrier) };
    }

    fn load_actor_class(&self, name: &str) -> *mut c_void {
        unsafe { (self.services.static_load_actor_class)(name.as_ptr(), name.len(), LOAD_CLASS_FLAGS) }
    }

    fn zone_json(&self, actor: *mut c_void, out: &mut Vec<u8>) {
        let zone = actor_zone(actor);
        if zone.is_null() {
            out.extend_from_slice(b"null");
            return;
        }
        let class = object_path_text(object_class(zone));
        let tag = fetch_name(zone, self.services.actor_tag);
        out.extend_from_slice(b"{\"class\":");
        json::push_json_string(&class, out);
        out.extend_from_slice(b",\"tag\":");
        json::push_json_string(&tag, out);
        out.push(b'}');
    }

    fn actor_json(&self, actor: *mut c_void) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"{\"location\":");
        out.extend_from_slice(vector_json(actor_location(actor)).as_bytes());
        out.extend_from_slice(b",\"rotation\":");
        out.extend_from_slice(rotation_json(actor_rotation(actor)).as_bytes());
        out.extend_from_slice(b",\"zone\":");
        self.zone_json(actor, &mut out);
        out.extend_from_slice(b",\"b_just_teleported\":");
        out.extend_from_slice(if actor_just_teleported(actor) { b"true" } else { b"false" });
        out.push(b'}');
        out
    }

    /// `StaticBspProbeReporter::AddStep`.
    fn add_step(&mut self, id: &'static str, request: String, result: String, actor: *mut c_void) {
        let actor = self.actor_json(actor);
        self.steps.push(Step {
            id,
            request,
            result,
            actor,
        });
    }

    /// `StaticBspProbeReporter::Serialize`.
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
        out.extend_from_slice(
            b",\"roles\":{\"probe\":\"transient:Engine.Effects\",\"spawn_anchor\":\"Engine.PlayerStart\"},\"steps\":[",
        );
        for (index, step) in self.steps.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            out.extend_from_slice(b"{\"id\":");
            json::push_json_string(step.id.as_bytes(), &mut out);
            out.extend_from_slice(b",\"request\":");
            out.extend_from_slice(step.request.as_bytes());
            out.extend_from_slice(b",\"result\":");
            out.extend_from_slice(step.result.as_bytes());
            out.extend_from_slice(b",\"actor\":");
            out.extend_from_slice(&step.actor);
            out.push(b'}');
        }
        out.extend_from_slice(b"],\"random_calls\":[]}\n");
        out
    }

    /// `StaticBspProbeReporter::Flush`.
    pub fn flush(&self) {
        if !self.configured {
            eprintln!("<HP2_STATIC_BSP_PROBE> report_error=invalid_output_path");
            return;
        }
        let document = self.serialize();
        match write_report(self.report_path.as_bytes(), &document) {
            Ok(()) => {
                if self.error.is_empty() {
                    println!(
                        "<HP2_STATIC_BSP_PROBE> report={} feasible={}",
                        self.report_path,
                        self.feasible as i32
                    );
                } else {
                    eprintln!(
                        "<HP2_STATIC_BSP_PROBE> report={} scenario_error={}",
                        self.report_path, self.error
                    );
                }
            }
            Err(ReportWriteError::Unwritable) => {
                eprintln!("<HP2_STATIC_BSP_PROBE> report_error=unwritable path={}", self.report_path);
            }
            Err(ReportWriteError::WriteFailed) => {
                eprintln!("<HP2_STATIC_BSP_PROBE> report_error=write_failed path={}", self.report_path);
            }
        }
    }
}

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

fn actor_location(actor: *mut c_void) -> FVector {
    unsafe { (*(actor as *mut AActor)).location }
}

fn actor_rotation(actor: *mut c_void) -> FRotator {
    unsafe { (*(actor as *mut AActor)).rotation }
}

fn zero_rotator() -> FRotator {
    ZERO_ROTATOR
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

fn number_json(value: f32) -> String {
    json::format_fixed(value as f64, 4)
}
