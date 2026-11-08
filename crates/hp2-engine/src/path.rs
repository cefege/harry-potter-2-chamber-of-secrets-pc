//! Port of the active `FPathBuilder` graph builder, path maintenance, and scout
//! placement/reach-query family in `Engine/Src/UnPath.cpp`.
//!
//! Maintenance, scout selection, placement, creation, merging and wall traversal
//! are Rust-owned; collision, spawn, ownership and diagnostic operations are leaves.
//! The broader reach-spec, pruning and visible-unreachable helpers are referenced
//! only by the commented-out alternate builder, not active `PATHS BUILD` behavior.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, ANavigationPoint, APawn, FName, FReachSpec, FRotator, ULevel,
};

use crate::actor;
use crate::reach::{bot_only_path, combine, less_equal, monster_path};
use crate::services::Hp2EngineServices;
use crate::vec;

/// Length of the per-node `Paths` / `upstreamPaths` arrays.
const PATH_SLOTS: usize = 16;

/// `ULevel::ReachSpecs(idx)`.
///
/// # Safety
/// `level` must be live and `idx` a valid reach-spec index.
unsafe fn spec_at(level: *mut ULevel, idx: i32) -> *mut FReachSpec {
    unsafe { ((*level).reach_specs.data as *mut FReachSpec).offset(idx as isize) }
}

/// `FReachSpec::Init` assigns fields, not the native padding bytes.
unsafe fn init_spec(spec: *mut FReachSpec) {
    unsafe {
        (*spec).start = ptr::null_mut();
        (*spec).end = ptr::null_mut();
        (*spec).distance = 0;
        (*spec).collision_radius = 0;
        (*spec).collision_height = 0;
        (*spec).reach_flags = 0;
        (*spec).b_pruned = 0;
    }
}

/// `FPathBuilder::specFor(Start, End)`: index of the first spec in
/// `Start->Paths` whose `End` is `end`, or -1.
///
/// # Safety
/// `level` and `start` must be live. Every non-(-1) index in `start.paths`
/// must be a valid reach-spec index of `level`.
pub unsafe fn spec_for(level: *mut ULevel, start: *mut ANavigationPoint, end: *mut c_void) -> i32 {
    unsafe {
        let mut i = 0;
        while i < PATH_SLOTS && (*start).paths[i] != -1 {
            let path_spec = spec_at(level, (*start).paths[i]);
            if (*path_spec).end == end {
                return (*start).paths[i];
            }
            i += 1;
        }
        -1
    }
}

/// `FPathBuilder::insertReachSpec(SpecArray, Spec)`: insert `spec` into
/// `spec_array`, ordered longest to shortest distance. When the array is
/// full the shortest entry at the front is dropped. Returns the slot the
/// caller must fill, or -1 when the spec is longer than every entry of a
/// full array.
///
/// # Safety
/// `level` must be live. `spec_array` must have 16 writable entries, each -1
/// or a valid reach-spec index of `level`.
pub unsafe fn insert_reach_spec(
    level: *mut ULevel,
    spec_array: *mut i32,
    spec: *const FReachSpec,
) -> i32 {
    unsafe {
        let arr = std::slice::from_raw_parts_mut(spec_array, PATH_SLOTS);
        let mut n = 0usize;
        while n < PATH_SLOTS
            && arr[n] != -1
            && (*spec_at(level, arr[n])).distance > (*spec).distance
        {
            n += 1;
        }
        let mut pos = n as i32;

        if arr[PATH_SLOTS - 1] == -1 {
            // not full
            if arr[n] != -1 {
                let mut current = arr[n];
                while n < PATH_SLOTS - 1 {
                    std::mem::swap(&mut arr[n + 1], &mut current);
                    if current == -1 {
                        n = PATH_SLOTS - 1;
                    }
                    n += 1;
                }
            }
        } else if n == 0 {
            // current is bigger than biggest
            return -1;
        } else {
            // full, so remove from front
            n -= 1;
            pos = n as i32;
            let mut current = arr[n];
            while n > 0 {
                std::mem::swap(&mut arr[n - 1], &mut current);
                n -= 1;
            }
        }
        pos
    }
}

/// `FPathBuilder::Prune(Node)`: remove straight reach specs that are
/// dominated by a two-hop alternate through `node`. Returns the number of
/// specs pruned.
///
/// # Safety
/// `level` and `node` must be live, with every non-(-1) path index valid.
pub unsafe fn prune(level: *mut ULevel, node: *mut ANavigationPoint) -> i32 {
    unsafe {
        let mut pruned = 0;
        let mut i = 0usize;
        while i < PATH_SLOTS && (*node).upstream_paths[i] != -1 {
            let part1 = spec_at(level, (*node).upstream_paths[i]);
            let mut n = 0usize;
            while n < PATH_SLOTS && (*node).paths[n] != -1 {
                let part2 = spec_at(level, (*node).paths[n]);
                let straight_path_index = spec_for(level, (*part1).start.cast(), (*part2).end);
                if straight_path_index != -1 {
                    let straight_path = spec_at(level, straight_path_index);
                    // Only combine's four assigned fields may be read.
                    let mut storage = std::mem::MaybeUninit::<FReachSpec>::uninit();
                    let alternate_path = storage.as_mut_ptr();
                    combine(part1, part2, alternate_path);
                    if ((*straight_path).distance as f32 * 1.2f32
                        >= (*alternate_path).distance as f32)
                        && (less_equal(alternate_path, straight_path) != 0
                            || bot_only_path(straight_path) != 0
                            || monster_path(alternate_path) != 0)
                    {
                        // prune straightpath
                        pruned += 1;
                        let start_node = (*straight_path).start as *mut ANavigationPoint;
                        let end_node = (*straight_path).end as *mut ANavigationPoint;

                        let mut j = 0usize;
                        while j < 15 && (*start_node).paths[j] != straight_path_index {
                            j += 1;
                        }
                        if (*start_node).paths[j] == straight_path_index {
                            while j < 15 && (*start_node).paths[j] != -1 {
                                (*start_node).paths[j] = (*start_node).paths[j + 1];
                                j += 1;
                            }
                            (*start_node).paths[15] = -1;
                        }

                        j = 0;
                        while j < 15 && (*start_node).pruned_paths[j] != -1 {
                            j += 1;
                        }
                        (*start_node).pruned_paths[j] = straight_path_index;
                        (*spec_at(level, straight_path_index)).b_pruned = 1;

                        j = 0;
                        while j < 15 && (*end_node).upstream_paths[j] != straight_path_index {
                            j += 1;
                        }
                        if (*end_node).upstream_paths[j] == straight_path_index {
                            while j < 15 && (*end_node).upstream_paths[j] != -1 {
                                (*end_node).upstream_paths[j] = (*end_node).upstream_paths[j + 1];
                                j += 1;
                            }
                            (*end_node).upstream_paths[15] = -1;
                        }
                        // Specs remain in the reachspec list and in PrunedPaths[],
                        // but are removed from the visible navigation graph.
                    }
                }
                n += 1;
            }
            i += 1;
        }
        pruned
    }
}

/// `ALiftCenter` / `ALiftExit` / `ATeleporter` / `AWarpZoneMarker` class codes
/// for `Hp2PathServices::actor_is_a`.
pub const PCLASS_NAVIGATION_POINT: i32 = 0;
pub const PCLASS_LIFT_CENTER: i32 = 1;
pub const PCLASS_LIFT_EXIT: i32 = 2;
pub const PCLASS_TELEPORTER: i32 = 3;
pub const PCLASS_WARP_ZONE_MARKER: i32 = 4;
pub const PCLASS_PLAYER_START: i32 = 5;
pub const PCLASS_BASE_STATION: i32 = 6;

/// `UnPath.h` placement constants.
const HUMANRADIUS: f32 = 18.0;
const HUMANHEIGHT: f32 = 39.0;
const R_SPECIAL: i32 = 32;

/// Engine calls the path builder makes into other slices: the Scout pawn,
/// level tracing and actor iteration, plus fields the ABI does not expose.
/// Filled in by `HP2PathServices.cpp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PathServices {
    /// `ULevel::FarMoveActor(actor, dest, test)`.
    pub far_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        dest: *const c_void,
        test: i32,
        out: *mut i32,
    ) -> i32,
    /// `ULevel::SingleLineCheck(Hit, source, end, start, TRACE_VisBlocking)`;
    /// returns `Hit.Actor`.
    pub single_line_hit_actor: unsafe extern "C" fn(
        level: *mut c_void,
        source: *mut c_void,
        end: *const c_void,
        start: *const c_void,
        out: *mut *mut c_void,
    ) -> i32,
    /// `ULevel::GetLevelInfo()->NavigationPointList`.
    pub navigation_point_list:
        unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ULevel::Actors.Num()`.
    pub actor_count: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `ULevel::Actors(i)`.
    pub actor_at: unsafe extern "C" fn(level: *mut c_void, i: i32, out: *mut *mut c_void) -> i32,
    /// `actor->IsA(class)` for the `PCLASS_*` codes above.
    pub actor_is_a: unsafe extern "C" fn(actor: *mut c_void, kind: i32, out: *mut i32) -> i32,
    /// `node->VisNoReachPaths[i] = path`.
    pub vis_no_reach_set: unsafe extern "C" fn(node: *mut c_void, i: i32, path: *mut c_void),
    /// Read the FName index (kind 1: LiftCenter, kind 2: LiftExit).
    pub lift_tag: unsafe extern "C" fn(actor: *mut c_void, kind: i32) -> i32,
    /// `((ATeleporter*)node)->URL == *actor->Tag`.
    pub teleporter_match:
        unsafe extern "C" fn(node: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32,
    /// `((AWarpZoneMarker*)node)->markedWarpZone->OtherSideURL ==
    /// *((AWarpZoneMarker*)actor)->markedWarpZone->ThisTag`.
    pub warp_zone_match:
        unsafe extern "C" fn(node: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32,
    /// `node->bOneWayPath`.
    pub node_one_way: unsafe extern "C" fn(node: *mut c_void) -> i32,
    /// `ULevel::ReachSpecs.AddItem(spec)`; returns the new index.
    pub reach_spec_add:
        unsafe extern "C" fn(level: *mut c_void, spec: *const c_void, out: *mut i32) -> i32,
    /// `debugf(TEXT("WARNING: %s and %s may be too close!"), actor, node)`.
    pub warn_too_close: unsafe extern "C" fn(actor: *mut c_void, node: *mut c_void) -> i32,
}

/// Scout placement/query leaves, separate from the existing graph-builder ABI.
/// Fallible callbacks return zero on success; a nonzero status is a captured
/// C++ exception. Vector and hit pointers are opaque at the C boundary.
#[repr(C)]
pub struct Hp2PathScoutServices {
    pub actor_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub actor_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub actor_is_a: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub find_scout_class: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    pub set_actor_zone: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub far_move_actor:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, i32, i32, *mut i32) -> i32,
    pub move_actor:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut c_void) -> i32,
    pub point_reachable: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
    pub single_line_check: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const c_void,
        *const c_void,
        *const c_void,
    ) -> i32,
    /// 0: no valid floor after 50 iterations; 1: scout did not fit.
    pub placement_failed: unsafe extern "C" fn(i32) -> i32,
}

/// Class codes for the separate maintenance table (not the graph/scout codes).
pub const MCLASS_NAVIGATION_POINT: i32 = 0;
pub const MCLASS_PATH_NODE: i32 = 1;
pub const MCLASS_WARP_ZONE_MARKER: i32 = 2;
pub const MCLASS_TRIGGER_MARKER: i32 = 3;
pub const MCLASS_INVENTORY_SPOT: i32 = 4;
pub const MCLASS_BUTTON_MARKER: i32 = 5;

/// Borrowed actor storage and typed ownership leaves for path maintenance.
/// Fallible leaves return exception status, never a query value.
#[repr(C)]
pub struct Hp2PathMaintenanceServices {
    pub actor_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub actor_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub actor_is_a: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub destroy_actor: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub reach_spec_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub log_reach_specs: unsafe extern "C" fn(i32) -> i32,
    pub clear_reach_specs: unsafe extern "C" fn(*mut c_void) -> i32,
    pub clear_navigation_point_list: unsafe extern "C" fn(*mut c_void) -> i32,
    pub status_update: unsafe extern "C" fn(i32, i32) -> i32,
    pub inventory_spot_item: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub clear_item_marker: unsafe extern "C" fn(*mut c_void),
    pub vis_no_reach_set: unsafe extern "C" fn(*mut c_void, i32, *mut c_void),
}

/// The active nearest-neighbour graph uses fallible leaves, including allocation.
/// Public alternate-builder helpers use the checked `Hp2PathServices` table.
#[repr(C)]
pub struct Hp2PathDefinitionServices {
    pub actor_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub actor_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub actor_is_a: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub clear_navigation_point_list: unsafe extern "C" fn(*mut c_void) -> i32,
    pub actor_class_name_is: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub actor_same_class: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub reach_spec_add: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
}

/// Clock and diagnostic leaves for the full active PATHS BUILD operation.
#[repr(C)]
pub struct Hp2PathBuildServices {
    pub seconds: unsafe extern "C" fn() -> f32,
    pub log_elapsed: unsafe extern "C" fn(f32) -> i32,
}

unsafe fn maintenance_is_a(
    svc: &Hp2PathMaintenanceServices,
    actor: *mut c_void,
    kind: i32,
) -> Result<bool, PathScoutThrown> {
    let mut result = 0;
    scout_status(unsafe { (svc.actor_is_a)(actor, kind, &mut result) })?;
    Ok(result != 0)
}

/// Remove every live PathNode slot, counting even a refused destruction.
///
/// # Safety
/// The table and level must be live engine objects for the duration of the call.
pub unsafe fn remove_paths(
    svc: &Hp2PathMaintenanceServices,
    level: *mut c_void,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        let mut removed = 0;
        let mut index = 0;
        while index < (svc.actor_count)(level) {
            let mut actor = ptr::null_mut();
            scout_status((svc.actor_at)(level, index, &mut actor))?;
            if !actor.is_null() && maintenance_is_a(svc, actor, MCLASS_PATH_NODE)? {
                removed += 1;
                scout_status((svc.destroy_actor)(level, actor))?;
            }
            index += 1;
        }
        Ok(removed)
    }
}

unsafe fn set_path_visibility(
    svc: &Hp2PathMaintenanceServices,
    level: *mut c_void,
    draw_type: u8,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        let mut shown = 0;
        let mut index = 0;
        while index < (svc.actor_count)(level) {
            let mut actor = ptr::null_mut();
            scout_status((svc.actor_at)(level, index, &mut actor))?;
            if !actor.is_null() && maintenance_is_a(svc, actor, MCLASS_PATH_NODE)? {
                shown += 1;
                (*actor.cast::<AActor>()).draw_type = draw_type;
            }
            index += 1;
        }
        Ok(shown)
    }
}

/// Show PathNodes with the legacy `DT_Sprite = 1`.
///
/// # Safety
/// The table and level must be live engine objects.
pub unsafe fn show_paths(
    svc: &Hp2PathMaintenanceServices,
    level: *mut c_void,
) -> Result<i32, PathScoutThrown> {
    unsafe { set_path_visibility(svc, level, 1) }
}

/// Hide PathNodes with the legacy `DT_None = 0`.
///
/// # Safety
/// The table and level must be live engine objects.
pub unsafe fn hide_paths(
    svc: &Hp2PathMaintenanceServices,
    level: *mut c_void,
) -> Result<i32, PathScoutThrown> {
    unsafe { set_path_visibility(svc, level, 0) }
}

/// Clear reach specs, destroy generated markers, and reset persistent nodes.
///
/// # Safety
/// Class checks must describe the actual actor layout. Callbacks may mutate the
/// live actor slots; status updates must leave the current slot readable.
pub unsafe fn undefine_paths(
    svc: &Hp2PathMaintenanceServices,
    level: *mut c_void,
) -> Result<(), PathScoutThrown> {
    unsafe {
        scout_status((svc.log_reach_specs)((svc.reach_spec_count)(level)))?;
        scout_status((svc.clear_reach_specs)(level))?;
        scout_status((svc.clear_navigation_point_list)(level))?;
        let mut index = 0;
        while index < (svc.actor_count)(level) {
            scout_status((svc.status_update)(index, (svc.actor_count)(level)))?;
            // StatusUpdate can replace this slot or append actors.
            let mut actor = ptr::null_mut();
            scout_status((svc.actor_at)(level, index, &mut actor))?;
            if !actor.is_null() && maintenance_is_a(svc, actor, MCLASS_NAVIGATION_POINT)? {
                if maintenance_is_a(svc, actor, MCLASS_WARP_ZONE_MARKER)?
                    || maintenance_is_a(svc, actor, MCLASS_TRIGGER_MARKER)?
                    || maintenance_is_a(svc, actor, MCLASS_INVENTORY_SPOT)?
                    || maintenance_is_a(svc, actor, MCLASS_BUTTON_MARKER)?
                {
                    if maintenance_is_a(svc, actor, MCLASS_INVENTORY_SPOT)? {
                        let item = (svc.inventory_spot_item)(actor);
                        if !item.is_null() {
                            (svc.clear_item_marker)(item);
                        }
                    }
                    scout_status((svc.destroy_actor)(level, actor))?;
                } else {
                    let node = actor.cast::<ANavigationPoint>();
                    (*node).next_navigation_point = ptr::null_mut();
                    (*node).next_ordered = ptr::null_mut();
                    (*node).prev_ordered = ptr::null_mut();
                    (*node).start_path = ptr::null_mut();
                    (*node).previous_path = ptr::null_mut();
                    for slot in 0..PATH_SLOTS {
                        (*node).paths[slot] = -1;
                        (*node).upstream_paths[slot] = -1;
                        (*node).pruned_paths[slot] = -1;
                        (svc.vis_no_reach_set)(actor, slot as i32, ptr::null_mut());
                    }
                }
            }
            index += 1;
        }
        Ok(())
    }
}

/// Remove old auto-built PathNodes, rebuild the graph, generate paths with a
/// non-jumping Scout, destroy the Scout, then rebuild the final graph.
///
/// # Safety
/// All tables and level must be live; `scout` is the builder's writable member
/// slot. As in the legacy code, a throwing phase leaves its partial state intact.
pub unsafe fn build_paths(
    svc: &Hp2PathScoutServices,
    maintenance: &Hp2PathMaintenanceServices,
    creation: &Hp2PathCreationServices,
    definition: &Hp2PathDefinitionServices,
    build: &Hp2PathBuildServices,
    level: *mut c_void,
    scout: *mut *mut APawn,
    optimization: i32,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        let mut elapsed = (build.seconds)() * -1.0;
        let mut index = 0;
        while index < (maintenance.actor_count)(level) {
            let mut actor = ptr::null_mut();
            scout_status((maintenance.actor_at)(level, index, &mut actor))?;
            if !actor.is_null()
                && maintenance_is_a(maintenance, actor, MCLASS_PATH_NODE)?
                && (creation.auto_built)(actor) != 0
            {
                scout_status((creation.destroy_actor)(level, actor))?;
            }
            index += 1;
        }
        undefine_paths(maintenance, level)?;
        define_paths(definition, level.cast())?;
        get_scout(svc, level, scout)?;
        actor::set_collision(&actor_services()?, (*scout).cast(), true, true, true)
            .map_err(PathScoutThrown)?;
        (**scout).base.set_b_collide_world(true);
        (**scout).jump_z = -1.0;
        (**scout).ground_speed = 320.0;
        (**scout).max_step_height = 24.0;
        let count = create_paths(svc, creation, level, *scout, optimization)?;
        scout_status((creation.destroy_actor)(level, (*scout).cast()))?;
        undefine_paths(maintenance, level)?;
        define_paths(definition, level.cast())?;
        elapsed += (build.seconds)();
        scout_status((build.log_elapsed)(elapsed))?;
        Ok(count)
    }
}

/// Fallible traversal callback retained for the C++ member entry point.
pub type Hp2PathPass2From =
    unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, f32) -> i32;

#[derive(Debug, PartialEq, Eq)]
pub struct PathScoutThrown(pub i32);

fn scout_status(status: i32) -> Result<(), PathScoutThrown> {
    if status == 0 {
        Ok(())
    } else {
        Err(PathScoutThrown(status))
    }
}

unsafe fn actor_services() -> Result<actor::Hp2ActorServices, PathScoutThrown> {
    unsafe { actor::published_services().map_err(PathScoutThrown) }
}

unsafe fn scout_is_a(
    svc: &Hp2PathScoutServices,
    actor: *mut c_void,
    kind: i32,
) -> Result<bool, PathScoutThrown> {
    let mut result = 0;
    scout_status(unsafe { (svc.actor_is_a)(actor, kind, &mut result) })?;
    Ok(result != 0)
}

unsafe fn scout_far_move(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut APawn,
    dest: FVector,
    no_check: i32,
) -> Result<i32, PathScoutThrown> {
    let mut result = 0;
    scout_status(unsafe {
        (svc.far_move_actor)(
            level,
            scout.cast(),
            ptr::from_ref(&dest).cast(),
            0,
            no_check,
            &mut result,
        )
    })?;
    Ok(result)
}

fn scout_hit() -> hp2_abi::engine::generated::FCheckResult {
    let mut hit: hp2_abi::engine::generated::FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = 1.0;
    hit.item = -1;
    hit
}

/// Select the last Scout in actor order, or resolve the Scout class and spawn.
/// The output slot is updated during iteration, just like the legacy member.
///
/// # Safety
/// All pointers and callback outputs must be live engine objects of their stated types.
pub unsafe fn get_scout(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut *mut APawn,
) -> Result<(), PathScoutThrown> {
    unsafe {
        *scout = ptr::null_mut();
        let mut index = 0;
        while index < (svc.actor_count)(level) {
            let mut actor = ptr::null_mut();
            scout_status((svc.actor_at)(level, index, &mut actor))?;
            if !actor.is_null() && scout_is_a(svc, actor, 7)? {
                *scout = actor.cast();
            }
            index += 1;
        }
        if (*scout).is_null() {
            let mut class = ptr::null_mut();
            scout_status((svc.find_scout_class)(&mut class))?;
            let actor = crate::level_spawn_actor::virtual_spawn_actor(
                level,
                class,
                FName { index: 0 },
                ptr::null_mut(),
                ptr::null_mut(),
                FVector { x: 0.0, y: 0.0, z: 0.0 },
                FRotator { pitch: 0, yaw: 0, roll: 0 },
                ptr::null_mut(),
                0,
                0,
            )
            .map_err(PathScoutThrown)?;
            *scout = (actor as *mut c_void).cast();
        }
        actor::set_collision(&actor_services()?, (*scout).cast(), true, true, true)
            .map_err(PathScoutThrown)?;
        (**scout).base.set_b_collide_world(true);
        scout_status((svc.set_actor_zone)(level, (*scout).cast()))
    }
}

/// Move to `start`, descend, and slide along up to two walls per iteration.
/// Even a floor found on iteration 50 fails, matching the post-move limit.
///
/// # Safety
/// `scout` and `level` must be live and callbacks must obey their contracts.
pub unsafe fn find_scout_start(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut APawn,
    start: FVector,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        if scout_far_move(svc, level, scout, start, 0)? == 0 {
            scout_status((svc.placement_failed)(1))?;
            return Ok(0);
        }
        let down = FVector {
            x: 0.0,
            y: 0.0,
            z: -50.0,
        };
        let mut hit = scout_hit();
        hit.normal.z = 0.0;
        let mut iterations = 0;
        while hit.normal.z < 0.7 {
            scout_status((svc.move_actor)(
                level,
                scout.cast(),
                ptr::from_ref(&down).cast(),
                ptr::from_mut(&mut hit).cast(),
            ))?;
            if hit.time < 1.0 && hit.normal.z < 0.7 {
                let old_normal = hit.normal;
                let mut delta = vec::scale(
                    vec::sub(down, vec::scale(hit.normal, dot(down, hit.normal))),
                    1.0 - hit.time,
                );
                if dot(delta, down) >= 0.0 {
                    scout_status((svc.move_actor)(
                        level,
                        scout.cast(),
                        ptr::from_ref(&delta).cast(),
                        ptr::from_mut(&mut hit).cast(),
                    ))?;
                    if hit.time < 1.0 && hit.normal.z < 0.7 {
                        crate::physic::two_wall_adjust(
                            vec::unsafe_normal(down),
                            &mut delta,
                            hit.normal,
                            old_normal,
                            hit.time,
                        );
                        scout_status((svc.move_actor)(
                            level,
                            scout.cast(),
                            ptr::from_ref(&delta).cast(),
                            ptr::from_mut(&mut hit).cast(),
                        ))?;
                    }
                }
            }
            iterations += 1;
            if iterations >= 50 {
                scout_status((svc.placement_failed)(0))?;
                return Ok(0);
            }
        }
        Ok(1)
    }
}

/// Try the original start, then only if needed the +20 Z fallback. Pass2From
/// always receives the original start. Collision radii are the legacy
/// MAXCOMMONRADIUS/MINCOMMONRADIUS (both 18 in this game), stepping by seven.
///
/// # Safety
/// Pointers must be live; `pass2` must accept `builder` and capture exceptions.
pub unsafe fn test_paths_from(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut APawn,
    builder: *mut c_void,
    start: FVector,
    pass2: Hp2PathPass2From,
) -> Result<(), PathScoutThrown> {
    unsafe {
        test_paths_from_with(svc, level, scout, start, |start, direction, turn| {
            scout_status(pass2(
                builder,
                ptr::from_ref(&start).cast(),
                ptr::from_ref(&direction).cast(),
                turn,
            ))
        })
    }
}

unsafe fn test_paths_from_with(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut APawn,
    start: FVector,
    mut pass2: impl FnMut(FVector, FVector, f32) -> Result<(), PathScoutThrown>,
) -> Result<(), PathScoutThrown> {
    unsafe {
        if (find_scout_start(svc, level, scout, start)? == 0
            || ((*scout).base.location.z - start.z).abs() > (*scout).base.collision_height)
            && find_scout_start(
                svc,
                level,
                scout,
                vec::add(
                    start,
                    FVector {
                        x: 0.0,
                        y: 0.0,
                        z: 20.0,
                    },
                ),
            )? == 0
        {
            return Ok(());
        }
        let mut direction = FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        let mut turn = 1.0;
        let mut radius = 18.0;
        while radius >= 18.0 {
            actor::set_collision_size(&actor_services()?, scout.cast(), radius, 39.0)
                .map_err(PathScoutThrown)?;
            pass2(start, direction, turn)?;
            turn *= -1.0;
            std::mem::swap(&mut direction.x, &mut direction.y);
            radius -= 7.0;
        }
        Ok(())
    }
}

/// Legacy membership filter, including subclass-aware navigation/lift tests.
///
/// # Safety
/// Non-null candidates must be live actors.
pub unsafe fn valid_node(
    svc: &Hp2PathScoutServices,
    node: *mut c_void,
    candidate: *mut AActor,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        Ok((!candidate.is_null()
            && candidate.cast::<c_void>() != node
            && !(*candidate).b_delete_me()
            && scout_is_a(svc, candidate.cast(), PCLASS_NAVIGATION_POINT)?
            && !scout_is_a(svc, candidate.cast(), PCLASS_LIFT_CENTER)?) as i32)
    }
}

/// Reachability restores location but intentionally leaves Physics=Walking.
/// A failed initial FarMoveActor does not suppress the reachability query.
///
/// # Safety
/// Engine pointers and callbacks must be live.
pub unsafe fn test_reach(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut APawn,
    start: FVector,
    end: FVector,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        let old_location = (*scout).base.location;
        scout_far_move(svc, level, scout, start, 0)?;
        (*scout).base.physics = 1; // PHYS_Walking
        let mut result = 0;
        scout_status(unsafe {
            (svc.point_reachable)(scout.cast(), ptr::from_ref(&end).cast(), &mut result)
        })?;
        scout_far_move(svc, level, scout, old_location, 1)?;
        Ok(result)
    }
}

/// Walk and require a supporting floor only when walkMove returns exactly 1.
///
/// # Safety
/// `hit` is a writable local copy of the caller's hit, as in the C++ value parameter.
pub unsafe fn test_walk(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut APawn,
    direction: FVector,
    hit: *mut hp2_abi::engine::generated::FCheckResult,
    threshold: f32,
    adjust: i32,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        let old_location = (*scout).base.location;
        let reach = crate::pawn_reach::published_services().map_err(PathScoutThrown)?;
        let result = crate::pawn::walk_move(
            &reach.pawn,
            scout,
            direction,
            hit,
            ptr::null_mut(),
            threshold,
            adjust,
        )
        .map_err(PathScoutThrown)?;
        if result == 1 {
            let start = (*scout).base.location;
            let end = vec::sub(
                start,
                FVector {
                    x: 0.0,
                    y: 0.0,
                    z: (*scout).base.collision_height + (*scout).max_step_height + 4.0,
                },
            );
            let extent = FVector {
                x: 16.0,
                y: 16.0,
                z: 1.0,
            };
            scout_status((svc.single_line_check)(
                level,
                scout.cast(),
                hit.cast(),
                ptr::from_ref(&end).cast(),
                ptr::from_ref(&start).cast(),
                ptr::from_ref(&extent).cast(),
            ))?;
            if (*hit).time < 1.0 {
                return Ok(result);
            }
            scout_far_move(svc, level, scout, old_location, 1)?;
            return Ok(-1);
        }
        Ok(result)
    }
}

/// Prefer the forward cylinder hit, then preserve the input normal if a
/// landing exists, then use the below-ledge horizontal probe if it hits.
///
/// # Safety
/// Engine pointers and callbacks must be live; `normal` must be writable.
pub unsafe fn find_blocking_normal(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    scout: *mut APawn,
    normal: *mut FVector,
) -> Result<(), PathScoutThrown> {
    unsafe {
        let mut hit = scout_hit();
        let mut extent = vec::zero();
        actor::get_cylinder_extent(
            &actor_services()?,
            scout.cast(),
            ptr::from_mut(&mut extent).cast(),
        )
        .map_err(PathScoutThrown)?;
        let start = (*scout).base.location;
        let end = vec::sub(start, vec::scale(*normal, 16.0));
        scout_status((svc.single_line_check)(
            level,
            scout.cast(),
            ptr::from_mut(&mut hit).cast(),
            ptr::from_ref(&end).cast(),
            ptr::from_ref(&start).cast(),
            ptr::from_ref(&extent).cast(),
        ))?;
        if hit.time < 1.0 {
            *normal = hit.normal;
            return Ok(());
        }
        let destination = vec::sub((*scout).base.location, vec::scale(*normal, 16.0));
        let down = FVector {
            x: 0.0,
            y: 0.0,
            z: -1.0 * (*scout).max_step_height,
        };
        let end = vec::add(destination, down);
        actor::get_cylinder_extent(
            &actor_services()?,
            scout.cast(),
            ptr::from_mut(&mut extent).cast(),
        )
        .map_err(PathScoutThrown)?;
        scout_status((svc.single_line_check)(
            level,
            scout.cast(),
            ptr::from_mut(&mut hit).cast(),
            ptr::from_ref(&end).cast(),
            ptr::from_ref(&destination).cast(),
            ptr::from_ref(&extent).cast(),
        ))?;
        if hit.time < 1.0 {
            return Ok(());
        }
        let start = vec::add(destination, down);
        actor::get_cylinder_extent(
            &actor_services()?,
            scout.cast(),
            ptr::from_mut(&mut extent).cast(),
        )
        .map_err(PathScoutThrown)?;
        let end = vec::add((*scout).base.location, down);
        scout_status((svc.single_line_check)(
            level,
            scout.cast(),
            ptr::from_mut(&mut hit).cast(),
            ptr::from_ref(&end).cast(),
            ptr::from_ref(&start).cast(),
            ptr::from_ref(&extent).cast(),
        ))?;
        if hit.time < 1.0 {
            *normal = hit.normal;
        }
        Ok(())
    }
}

/// `FPathBuilder::definePaths`: link each non-`PlayerStart`, non-`baseStation`
/// navigation point to its two nearest same-class or `baseStation` points.
/// The caller’s active legacy implementation uses this restricted graph
/// builder; the broader Scout/lift/visibility helpers below belong to the
/// commented-out alternate implementation.
///
/// # Safety
/// `level` and every non-null actor returned by the service callbacks must be
/// live engine objects; actor entries passing `actor_is_a` must be navigation
/// points with valid 16-entry `paths` arrays.
pub unsafe fn define_paths(
    svc: &Hp2PathDefinitionServices,
    level: *mut ULevel,
) -> Result<(), PathScoutThrown> {
    unsafe {
        let mut spec_storage = std::mem::MaybeUninit::<FReachSpec>::uninit();
        let spec = spec_storage.as_mut_ptr();
        let lvl = level.cast::<c_void>();
        scout_status((svc.clear_navigation_point_list)(lvl))?;
        let actor_count = (svc.actor_count)(lvl);

        for i in 0..actor_count {
            let actor1 = definition_actor(svc, lvl, i)?;
            if actor1.is_null()
                || !definition_class(svc, actor1.cast(), PCLASS_NAVIGATION_POINT, false)?
                || definition_class(svc, actor1.cast(), PCLASS_PLAYER_START, true)?
                || definition_class(svc, actor1.cast(), PCLASS_BASE_STATION, true)?
            {
                continue;
            }

            let mut node = actor1.cast::<ANavigationPoint>();
            let mut current_actor: *mut AActor = ptr::null_mut();
            let mut current_distance = 300_000_000.0f32;
            let mut second_actor: *mut AActor = ptr::null_mut();
            let mut second_distance = 300_000_000.0f32;

            for ib in 0..actor_count {
                let actor2 = definition_actor(svc, lvl, ib)?;
                if actor2.is_null()
                    || actor2 == actor1
                    || !definition_class(svc, actor2.cast(), PCLASS_NAVIGATION_POINT, false)?
                    || definition_class(svc, actor2.cast(), PCLASS_PLAYER_START, true)?
                {
                    continue;
                }
                let mut same_class = 0;
                scout_status((svc.actor_same_class)(
                    actor2.cast(),
                    actor1.cast(),
                    &mut same_class,
                ))?;
                if same_class == 0
                    && !definition_class(svc, actor2.cast(), PCLASS_BASE_STATION, true)?
                {
                    continue;
                }

                let distance =
                    vec::size_squared(vec::sub((*actor2).location, (*actor1).location)).sqrt();
                if current_actor.is_null() {
                    current_actor = actor2;
                    current_distance = distance;
                } else if distance.abs() < current_distance {
                    second_actor = current_actor;
                    second_distance = current_distance;
                    current_actor = actor2;
                    current_distance = distance.abs();
                } else if distance.abs() < second_distance {
                    second_actor = actor2;
                    second_distance = distance.abs();
                }
            }

            if !current_actor.is_null()
                && (*current_actor.cast::<ANavigationPoint>()).paths[2] < 0
                && (*node).paths[1] < 0
            {
                init_spec(spec);
                (*spec).collision_radius = 60;
                (*spec).collision_height = 60;
                (*spec).reach_flags = R_SPECIAL;
                (*spec).start = node.cast();
                (*spec).end = current_actor.cast();
                (*spec).distance = 500;
                unsafe_add_path_spec(svc, level, node, spec)?;

                (*spec).start = current_actor.cast();
                (*spec).end = node.cast();
                unsafe_add_path_spec(svc, level, node, spec)?;

                if !second_actor.is_null() {
                    (*spec).start = node.cast();
                    (*spec).end = second_actor.cast();
                    unsafe_add_path_spec(svc, level, node, spec)?;

                    node = second_actor.cast();
                    (*spec).start = node.cast();
                    (*spec).end = actor1.cast();
                    unsafe_add_path_spec(svc, level, node, spec)?;
                }
            }
        }
        Ok(())
    }
}

unsafe fn definition_actor(
    svc: &Hp2PathDefinitionServices,
    level: *mut c_void,
    index: i32,
) -> Result<*mut AActor, PathScoutThrown> {
    let mut actor = ptr::null_mut();
    scout_status(unsafe { (svc.actor_at)(level, index, &mut actor) })?;
    Ok(actor.cast())
}

unsafe fn definition_class(
    svc: &Hp2PathDefinitionServices,
    actor: *mut c_void,
    kind: i32,
    by_name: bool,
) -> Result<bool, PathScoutThrown> {
    let mut result = 0;
    let query = if by_name {
        svc.actor_class_name_is
    } else {
        svc.actor_is_a
    };
    scout_status(unsafe { query(actor, kind, &mut result) })?;
    Ok(result != 0)
}

/// Appends a spec and records its index in `node.paths`, matching the active
/// C++ `definePaths` implementation.
///
/// # Safety
/// `level`, `node`, and `spec` must satisfy `insert_reach_spec` and the service
/// callback’s live-object contract.
unsafe fn unsafe_add_path_spec(
    svc: &Hp2PathDefinitionServices,
    level: *mut ULevel,
    node: *mut ANavigationPoint,
    spec: *const FReachSpec,
) -> Result<(), PathScoutThrown> {
    unsafe {
        let pos = insert_reach_spec(level, (*node).paths.as_mut_ptr(), spec);
        if pos != -1 {
            let mut index = -1;
            scout_status((svc.reach_spec_add)(level.cast(), spec.cast(), &mut index))?;
            (*node).paths[pos as usize] = index;
        }
        Ok(())
    }
}

/// `IsA(kind)` through the path services table.
///
/// # Safety
/// `actor` must be non-null and live.
unsafe fn is_a<T>(
    svc: &Hp2PathServices,
    actor: *mut T,
    kind: i32,
) -> Result<bool, PathScoutThrown> {
    let mut result = 0;
    scout_status(unsafe { (svc.actor_is_a)(actor.cast(), kind, &mut result) })?;
    Ok(result != 0)
}

unsafe fn graph_actor(
    svc: &Hp2PathServices,
    level: *mut c_void,
    index: i32,
) -> Result<*mut AActor, PathScoutThrown> {
    let mut actor = ptr::null_mut();
    scout_status(unsafe { (svc.actor_at)(level, index, &mut actor) })?;
    Ok(actor.cast())
}

/// `FVector | FVector` (`UnMath.h`), same operand order as `vec::size_squared`.
fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.y.mul_add(b.y, a.x * b.x))
}

/// `FPathBuilder::addVisNoReach(start)`: record up to 16 visible-but-unreachable
/// navigation points within 2000 units into `start.VisNoReachPaths`.
///
/// # Safety
/// `level`, `scout` and `start` must be live. `start` must be an
/// `ANavigationPoint`.
pub unsafe fn add_vis_no_reach(
    svc: &Hp2PathServices,
    level: *mut ULevel,
    scout: *mut APawn,
    start: *mut AActor,
) -> Result<(), PathScoutThrown> {
    unsafe {
        let lvl = level.cast::<c_void>();
        if is_a(svc, start, PCLASS_LIFT_CENTER)? {
            return Ok(());
        }
        let node = start.cast::<ANavigationPoint>();

        actor::set_collision_size(&actor_services()?, scout.cast(), HUMANRADIUS, HUMANHEIGHT)
            .map_err(PathScoutThrown)?;
        let mut moved = 0;
        scout_status((svc.far_move_actor)(
            lvl,
            scout.cast(),
            ptr::addr_of!((*start).location).cast(),
            1,
            &mut moved,
        ))?;
        (*scout).move_target = start.cast();
        (*scout).set_b_can_do_special(true);
        let mut new_path = std::mem::MaybeUninit::<*mut c_void>::uninit();
        let mut i = 0i32;

        let mut first = ptr::null_mut();
        scout_status((svc.navigation_point_list)(lvl, &mut first))?;
        let mut path = first.cast::<ANavigationPoint>();
        while !path.is_null() {
            let distsq = vec::size_squared(vec::sub((*start).location, (*path).base.location));
            if !is_a(svc, path, PCLASS_LIFT_CENTER)? && path != node && distsq < 4000000.0 && i < 16
            {
                let mut hit = ptr::null_mut();
                scout_status((svc.single_line_hit_actor)(
                    lvl,
                    scout.cast(),
                    ptr::addr_of!((*path).base.location).cast(),
                    ptr::addr_of!((*start).location).cast(),
                    &mut hit,
                ))?;
                if hit.is_null() {
                    let found = crate::route::find_path_toward(
                        crate::route::published_services().map_err(PathScoutThrown)?,
                        scout,
                        path.cast::<AActor>(),
                        0,
                        new_path.as_mut_ptr().cast::<*mut AActor>(),
                        1,
                    )
                    .map_err(PathScoutThrown)?;
                    let path_dist: f32 = if found != 0 {
                        (*new_path.assume_init().cast::<ANavigationPoint>()).visited_weight as f32
                    } else {
                        200000000.0
                    };
                    if path_dist != 10000000.0 && path_dist * path_dist > 4.0 * distsq {
                        (svc.vis_no_reach_set)(node.cast(), i, path.cast());
                        i += 1;
                    }
                }
            }
            path = (*path).next_navigation_point;
        }
        Ok(())
    }
}

/// `FPathBuilder::addReachSpecs(start)`: add reach specs from `start` to every
/// reachable navigation point, plus the special lift, teleporter and
/// warp-zone links, and register each in the destination's `upstreamPaths`.
///
/// # Safety
/// `level`, `scout` and `start` must be live navigation-point actors, and
/// every object reached through the level's actor list must be live.
pub unsafe fn add_reach_specs(
    svc: &Hp2PathServices,
    engine: &Hp2EngineServices,
    level: *mut ULevel,
    scout: *mut APawn,
    start: *mut AActor,
) -> Result<(), PathScoutThrown> {
    unsafe {
        let lvl = level.cast::<c_void>();
        let node = start.cast::<ANavigationPoint>();
        let mut spec_storage = std::mem::MaybeUninit::<FReachSpec>::uninit();
        let spec = spec_storage.as_mut_ptr();

        // Adds `spec` to `from.paths` and `to.upstream_paths` (the C++ pattern).
        let link = |spec: *const FReachSpec,
                    from: *mut ANavigationPoint,
                    to: *mut ANavigationPoint|
         -> Result<(), PathScoutThrown> {
            let pos = insert_reach_spec(level, (*from).paths.as_mut_ptr(), spec);
            if pos != -1 {
                let mut idx = -1;
                scout_status((svc.reach_spec_add)(lvl, spec.cast(), &mut idx))?;
                (*from).paths[pos as usize] = idx;
                let pos2 = insert_reach_spec(level, (*to).upstream_paths.as_mut_ptr(), spec);
                if pos2 != -1 {
                    (*to).upstream_paths[pos2 as usize] = idx;
                }
            }
            Ok(())
        };

        if is_a(svc, start, PCLASS_LIFT_CENTER)? {
            let lift_tag = (svc.lift_tag)(start.cast(), PCLASS_LIFT_CENTER);
            let mut i = 0;
            while i < (svc.actor_count)(lvl) {
                let actor = graph_actor(svc, lvl, i)?;
                i += 1;
                if !actor.is_null()
                    && is_a(svc, actor, PCLASS_LIFT_EXIT)?
                    && (svc.lift_tag)(actor.cast(), PCLASS_LIFT_EXIT) == lift_tag
                {
                    init_spec(spec);
                    (*spec).collision_radius = 60;
                    (*spec).collision_height = 60;
                    (*spec).reach_flags = R_SPECIAL;
                    (*spec).start = node.cast();
                    (*spec).end = actor.cast();
                    (*spec).distance = 500;
                    link(spec, node, actor.cast())?;

                    init_spec(spec);
                    (*spec).collision_radius = 60;
                    (*spec).collision_height = 60;
                    (*spec).reach_flags = R_SPECIAL;
                    (*spec).start = actor.cast();
                    (*spec).end = node.cast();
                    (*spec).distance = 500;
                    link(spec, actor.cast::<ANavigationPoint>(), node)?;
                }
            }
            return Ok(());
        }

        if is_a(svc, start, PCLASS_TELEPORTER)? || is_a(svc, start, PCLASS_WARP_ZONE_MARKER)? {
            let mut i = 0;
            while i < (svc.actor_count)(lvl) {
                let mut found = false;
                let actor = graph_actor(svc, lvl, i)?;
                i += 1;
                if is_a(svc, start, PCLASS_TELEPORTER)? {
                    if !actor.is_null()
                        && is_a(svc, actor, PCLASS_TELEPORTER)?
                        && actor != start.cast()
                    {
                        let mut matched = 0;
                        scout_status((svc.teleporter_match)(
                            start.cast(),
                            actor.cast(),
                            &mut matched,
                        ))?;
                        found = matched != 0;
                    }
                } else if !actor.is_null()
                    && is_a(svc, actor, PCLASS_WARP_ZONE_MARKER)?
                    && actor != start.cast()
                {
                    let mut matched = 0;
                    scout_status((svc.warp_zone_match)(
                        start.cast(),
                        actor.cast(),
                        &mut matched,
                    ))?;
                    found = matched != 0;
                }

                if found {
                    init_spec(spec);
                    (*spec).collision_radius = 150;
                    (*spec).collision_height = 150;
                    (*spec).reach_flags = R_SPECIAL;
                    (*spec).start = node.cast();
                    (*spec).end = actor.cast();
                    (*spec).distance = 100;
                    link(spec, node, actor.cast())?;
                    break;
                }
            }
        }

        let mut i = 0;
        while i < (svc.actor_count)(lvl) {
            let actor = graph_actor(svc, lvl, i)?;
            i += 1;
            if actor.is_null()
                || (*actor).b_delete_me()
                || !is_a(svc, actor, PCLASS_NAVIGATION_POINT)?
                || is_a(svc, actor, PCLASS_LIFT_CENTER)?
                || actor == start
                || vec::size_squared(vec::sub((*start).location, (*actor).location)) >= 1000000.0
            {
                continue;
            }
            // Negated form kept on purpose: NaN takes the C++ `!(d > 0)` branch.
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if (svc.node_one_way)(start.cast()) != 0
                && !(dot(
                    vec::sub((*actor).location, (*start).location),
                    rotator_vector(engine, ptr::addr_of!((*start).rotation)),
                ) > 0.0)
            {
                continue;
            }
            if vec::size_squared(vec::sub((*actor).location, (*start).location)) < 1000.0 {
                scout_status((svc.warn_too_close)(actor.cast(), start.cast()))?;
            }
            init_spec(spec);
            let mut reachable = 0;
            scout_status((engine.reach_define_for)(
                spec.cast(),
                start.cast(),
                actor.cast(),
                scout.cast(),
                &mut reachable,
            ))?;
            if reachable != 0 {
                link(spec, node, actor.cast::<ANavigationPoint>())?;
            }
        }
        Ok(())
    }
}

/// `FRotator::Vector()` through the engine services table.
fn rotator_vector(engine: &Hp2EngineServices, rot: *const FRotator) -> FVector {
    let mut out = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    // SAFETY: `rot` is a live FRotator; `out` is a valid FVector destination.
    unsafe { (engine.rotator_vector)(rot.cast(), ptr::addr_of_mut!(out).cast()) };
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(distance: i32) -> FReachSpec {
        // SAFETY: FReachSpec is plain old data; all-zero is a valid value.
        let mut s: FReachSpec = unsafe { std::mem::zeroed() };
        s.distance = distance;
        s
    }

    #[test]
    fn insert_keeps_longest_first_and_drops_shortest_when_full() {
        // Table index i holds distance i, so larger index = longer spec.
        let mut table: Vec<FReachSpec> = (0..40).map(spec).collect();
        // SAFETY: ULevel is plain old data; only reach_specs is read here.
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        level.reach_specs.data = table.as_mut_ptr().cast();
        let lp: *mut ULevel = &mut level;

        let mut arr = [-1i32; 16];
        unsafe {
            // Empty array: first insert lands at slot 0.
            assert_eq!(insert_reach_spec(lp, arr.as_mut_ptr(), &spec(10)), 0);
            assert_eq!(arr[0], -1); // caller fills the slot; nothing stored yet
            arr[0] = 10;
            // Longer spec (distance 20) goes in front of distance 10 (index 10).
            assert_eq!(insert_reach_spec(lp, arr.as_mut_ptr(), &spec(20)), 0);
            arr[0] = 20;
            assert_eq!(arr[..2], [20, 10]);
        }
    }

    #[test]
    fn insert_full_array_rejects_longer_than_all_and_evicts_front() {
        let mut table: Vec<FReachSpec> = (0..40).map(spec).collect();
        // SAFETY: as above.
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        level.reach_specs.data = table.as_mut_ptr().cast();
        let lp: *mut ULevel = &mut level;

        // Full array of distances 15..=0 (index 0 longest, index 15 shortest).
        let mut arr: [i32; 16] = std::array::from_fn(|i| (15 - i) as i32);
        unsafe {
            // Distance 100 is longer than every entry of a full array: rejected.
            assert_eq!(insert_reach_spec(lp, arr.as_mut_ptr(), &spec(100)), -1);
            assert_eq!(arr[0], 15);
            // Distance 5: shifted in; slot is n-1 = 9 and the front (15) is dropped.
            let pos = insert_reach_spec(lp, arr.as_mut_ptr(), &spec(5));
            assert_eq!(pos, 9);
            assert_eq!(arr[..9], [14, 13, 12, 11, 10, 9, 8, 7, 6]);
        }
    }

    #[test]
    fn spec_for_finds_first_matching_end() {
        let mut table: Vec<FReachSpec> = (0..4).map(spec).collect();
        let end_a = 0x10 as *mut c_void;
        let end_b = 0x20 as *mut c_void;
        table[1].end = end_a;
        table[2].end = end_b;
        // SAFETY: as above.
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        level.reach_specs.data = table.as_mut_ptr().cast();
        // SAFETY: ANavigationPoint is plain old data.
        let mut node: ANavigationPoint = unsafe { std::mem::zeroed() };
        node.paths = [-1; 16];
        node.paths[0] = 1;
        node.paths[1] = 2;
        let np: *mut ANavigationPoint = &mut node;
        let lp: *mut ULevel = &mut level;
        unsafe {
            assert_eq!(spec_for(lp, np, end_b), 2);
            assert_eq!(spec_for(lp, np, end_a), 1);
            assert_eq!(spec_for(lp, np, 0x30 as *mut c_void), -1);
        }
    }

    #[test]
    fn prune_removes_straight_spec_dominated_by_two_hop_alternate() {
        // Specs: 0 = U (A->N, dist 10), 1 = P (N->C, dist 10), 2 = S (A->C, dist 100).
        let mut table: Vec<FReachSpec> = (0..3).map(|_| spec(0)).collect();
        for s in table.iter_mut() {
            s.collision_radius = 18;
            s.collision_height = 39;
        }
        // Nodes: 0 = A, 1 = N, 2 = C. Their addresses must stay fixed.
        let mut nodes: Vec<ANavigationPoint> = (0..3)
            .map(|_| {
                // SAFETY: ANavigationPoint is plain old data.
                let mut n: ANavigationPoint = unsafe { std::mem::zeroed() };
                n.paths = [-1; 16];
                n.upstream_paths = [-1; 16];
                n.pruned_paths = [-1; 16];
                n
            })
            .collect();
        let a = &mut nodes[0] as *mut ANavigationPoint as *mut c_void;
        let nn = &mut nodes[1] as *mut ANavigationPoint as *mut c_void;
        let c = &mut nodes[2] as *mut ANavigationPoint as *mut c_void;
        table[0].start = a;
        table[0].end = nn;
        table[0].distance = 10;
        table[1].start = nn;
        table[1].end = c;
        table[1].distance = 10;
        table[2].start = a;
        table[2].end = c;
        table[2].distance = 100;

        nodes[0].paths[0] = 2;
        nodes[2].upstream_paths[0] = 2;
        nodes[1].upstream_paths[0] = 0;
        nodes[1].paths[0] = 1;

        // SAFETY: as above.
        let mut level: ULevel = unsafe { std::mem::zeroed() };
        level.reach_specs.data = table.as_mut_ptr().cast();
        let lp: *mut ULevel = &mut level;
        let np = &mut nodes[1] as *mut ANavigationPoint;

        unsafe {
            assert_eq!(prune(lp, np), 1);
        }
        assert_eq!(nodes[0].paths[0], -1);
        assert_eq!(nodes[0].pruned_paths[0], 2);
        assert_eq!(nodes[2].upstream_paths[0], -1);
        assert_eq!(table[2].b_pruned, 1);
        assert_eq!(table[0].b_pruned, 0);
    }
    #[test]
    fn lift_reach_specs_are_registered_in_both_navigation_directions() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static LIFT_CENTER: AtomicUsize = AtomicUsize::new(0);
        static LIFT_EXIT: AtomicUsize = AtomicUsize::new(0);

        #[repr(C)]
        struct Fixture {
            level: ULevel,
            actors: [*mut AActor; 2],
            specs: [FReachSpec; 4],
            spec_count: usize,
        }

        unsafe extern "C" fn far_move(
            _: *mut c_void,
            _: *mut c_void,
            _: *const c_void,
            _: i32,
            out: *mut i32,
        ) -> i32 {
            unsafe {
                *out = 0;
            }
            0
        }
        unsafe extern "C" fn line_hit(
            _: *mut c_void,
            _: *mut c_void,
            _: *const c_void,
            _: *const c_void,
            out: *mut *mut c_void,
        ) -> i32 {
            unsafe {
                *out = ptr::null_mut();
            }
            0
        }
        unsafe extern "C" fn nav_list(_: *mut c_void, out: *mut *mut c_void) -> i32 {
            unsafe {
                *out = ptr::null_mut();
            }
            0
        }
        unsafe extern "C" fn actor_count(_: *mut c_void) -> i32 {
            2
        }
        unsafe extern "C" fn actor_at(
            level: *mut c_void,
            index: i32,
            out: *mut *mut c_void,
        ) -> i32 {
            unsafe {
                *out = (*(level.cast::<Fixture>())).actors[index as usize].cast();
            }
            0
        }
        unsafe extern "C" fn actor_is_a(actor: *mut c_void, kind: i32, out: *mut i32) -> i32 {
            let address = actor as usize;
            unsafe {
                *out = match kind {
                    PCLASS_LIFT_CENTER => (address == LIFT_CENTER.load(Ordering::Relaxed)) as i32,
                    PCLASS_LIFT_EXIT => (address == LIFT_EXIT.load(Ordering::Relaxed)) as i32,
                    _ => 0,
                };
            }
            0
        }
        unsafe extern "C" fn vis_set(_: *mut c_void, _: i32, _: *mut c_void) {}
        unsafe extern "C" fn lift_tag(_: *mut c_void, _: i32) -> i32 {
            1
        }
        unsafe extern "C" fn special_match(_: *mut c_void, _: *mut c_void, out: *mut i32) -> i32 {
            unsafe {
                *out = 0;
            }
            0
        }
        unsafe extern "C" fn node_one_way(_: *mut c_void) -> i32 {
            0
        }
        unsafe extern "C" fn add_spec(
            level: *mut c_void,
            spec: *const c_void,
            out: *mut i32,
        ) -> i32 {
            unsafe {
                let fixture = &mut *level.cast::<Fixture>();
                let index = fixture.spec_count;
                let source = spec.cast::<FReachSpec>();
                let dest = &mut fixture.specs[index];
                dest.distance = (*source).distance;
                dest.start = (*source).start;
                dest.end = (*source).end;
                dest.collision_radius = (*source).collision_radius;
                dest.collision_height = (*source).collision_height;
                dest.reach_flags = (*source).reach_flags;
                dest.b_pruned = (*source).b_pruned;
                fixture.spec_count += 1;
                *out = index as i32;
                0
            }
        }
        unsafe extern "C" fn warn(_: *mut c_void, _: *mut c_void) -> i32 {
            0
        }
        unsafe extern "C" fn random_float(out: *mut f32) -> i32 {
            unsafe {
                *out = 0.0;
            }
            0
        }
        unsafe extern "C" fn random_int(out: *mut i32) -> i32 {
            unsafe {
                *out = 0;
            }
            0
        }
        unsafe extern "C" fn engine_far_move(
            _: *mut c_void,
            _: *mut c_void,
            _: *const c_void,
            out: *mut i32,
        ) -> i32 {
            unsafe {
                *out = 0;
            }
            0
        }
        unsafe extern "C" fn point_reachable_mock(
            _: *mut c_void,
            _: *const c_void,
            out: *mut i32,
        ) -> i32 {
            unsafe {
                *out = 0;
            }
            0
        }
        unsafe extern "C" fn model_line(_: *mut c_void, _: *const c_void, _: *const c_void) -> u8 {
            0
        }
        unsafe extern "C" fn rotator_vector(_: *const c_void, _: *mut c_void) {}
        unsafe extern "C" fn wind_count() -> i32 {
            0
        }
        unsafe extern "C" fn wind_at(_: i32) -> *mut c_void {
            ptr::null_mut()
        }
        unsafe extern "C" fn wind_random() -> *mut c_void {
            ptr::null_mut()
        }
        unsafe extern "C" fn reach_define(
            _: *mut c_void,
            _: *mut c_void,
            _: *mut c_void,
            _: *mut c_void,
            out: *mut i32,
        ) -> i32 {
            unsafe {
                *out = 0;
            }
            0
        }
        unsafe extern "C" fn reach_best(
            _: *mut c_void,
            _: *const c_void,
            _: *const c_void,
            _: *mut c_void,
            out: *mut i32,
        ) -> i32 {
            unsafe {
                *out = 0;
            }
            0
        }

        let path_services = Hp2PathServices {
            far_move_actor: far_move,
            single_line_hit_actor: line_hit,
            navigation_point_list: nav_list,
            actor_count,
            actor_at,
            actor_is_a,
            vis_no_reach_set: vis_set,
            lift_tag,
            teleporter_match: special_match,
            warp_zone_match: special_match,
            node_one_way,
            reach_spec_add: add_spec,
            warn_too_close: warn,
        };
        let engine_services = Hp2EngineServices {
            app_frand: random_float,
            app_rand: random_int,
            far_move_actor: engine_far_move,
            point_reachable: point_reachable_mock,
            model_fast_line_check: model_line,
            rotator_vector,
            wind_count,
            wind_at,
            wind_random,
            reach_define_for: reach_define,
            reach_find_best: reach_best,
        };

        // SAFETY: ABI structs are plain old data and are initialized before use.
        let mut center: ANavigationPoint = unsafe { std::mem::zeroed() };
        let mut exit: ANavigationPoint = unsafe { std::mem::zeroed() };
        center.paths = [-1; PATH_SLOTS];
        center.upstream_paths = [-1; PATH_SLOTS];
        exit.paths = [-1; PATH_SLOTS];
        exit.upstream_paths = [-1; PATH_SLOTS];
        let center_actor = ptr::addr_of_mut!(center.base);
        let exit_actor = ptr::addr_of_mut!(exit.base);
        LIFT_CENTER.store(center_actor as usize, Ordering::Relaxed);
        LIFT_EXIT.store(exit_actor as usize, Ordering::Relaxed);

        let mut fixture = Fixture {
            // SAFETY: the Rust algorithm reads only `reach_specs.data`.
            level: unsafe { std::mem::zeroed() },
            actors: [center_actor, exit_actor],
            // SAFETY: FReachSpec is plain old data.
            specs: unsafe { std::mem::zeroed() },
            spec_count: 0,
        };
        fixture.level.reach_specs.data = fixture.specs.as_mut_ptr().cast();
        let mut scout: APawn = unsafe { std::mem::zeroed() };

        // SAFETY: the fixture owns the engine objects and callback storage.
        unsafe {
            add_reach_specs(
                &path_services,
                &engine_services,
                &mut fixture.level,
                &mut scout,
                center_actor,
            )
            .unwrap();
        }

        assert_eq!(fixture.spec_count, 2);
        assert_eq!(fixture.specs[0].start, center_actor.cast());
        assert_eq!(fixture.specs[0].end, exit_actor.cast());
        assert_eq!(fixture.specs[1].start, exit_actor.cast());
        assert_eq!(fixture.specs[1].end, center_actor.cast());
        assert_eq!(
            (fixture.specs[0].distance, fixture.specs[1].distance),
            (500, 500)
        );
        assert_eq!(
            (fixture.specs[0].reach_flags, fixture.specs[1].reach_flags),
            (R_SPECIAL, R_SPECIAL)
        );
        assert_eq!(center.paths[0], 0);
        assert_eq!(exit.upstream_paths[0], 0);
        assert_eq!(exit.paths[0], 1);
        assert_eq!(center.upstream_paths[0], 1);
    }
    #[test]
    fn active_define_paths_connects_same_class_nodes_and_excludes_player_starts() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static FIRST: AtomicUsize = AtomicUsize::new(0);
        static SECOND: AtomicUsize = AtomicUsize::new(0);
        static PLAYER_START: AtomicUsize = AtomicUsize::new(0);

        #[repr(C)]
        struct Fixture {
            level: ULevel,
            actors: [*mut AActor; 3],
            specs: [FReachSpec; 8],
            spec_count: usize,
            navigation_list_cleared: bool,
        }

        unsafe extern "C" fn clear_navigation_points(level: *mut c_void) -> i32 {
            unsafe { (*level.cast::<Fixture>()).navigation_list_cleared = true };
            0
        }
        unsafe extern "C" fn actor_count(_: *mut c_void) -> i32 {
            3
        }
        unsafe extern "C" fn actor_at(
            level: *mut c_void,
            index: i32,
            out: *mut *mut c_void,
        ) -> i32 {
            unsafe { *out = (*level.cast::<Fixture>()).actors[index as usize].cast() };
            0
        }
        unsafe extern "C" fn actor_is_a(actor: *mut c_void, kind: i32, out: *mut i32) -> i32 {
            let known = [
                FIRST.load(Ordering::Relaxed),
                SECOND.load(Ordering::Relaxed),
                PLAYER_START.load(Ordering::Relaxed),
            ];
            unsafe {
                *out = (kind == PCLASS_NAVIGATION_POINT && known.contains(&(actor as usize))) as i32
            };
            0
        }
        unsafe extern "C" fn class_name_is(actor: *mut c_void, kind: i32, out: *mut i32) -> i32 {
            unsafe {
                *out = (kind == PCLASS_PLAYER_START
                    && actor as usize == PLAYER_START.load(Ordering::Relaxed))
                    as i32;
            }
            0
        }
        unsafe extern "C" fn same_class(
            actor1: *mut c_void,
            actor2: *mut c_void,
            out: *mut i32,
        ) -> i32 {
            let first = FIRST.load(Ordering::Relaxed);
            let second = SECOND.load(Ordering::Relaxed);
            unsafe {
                *out = ((actor1 as usize == first && actor2 as usize == second)
                    || (actor1 as usize == second && actor2 as usize == first))
                    as i32;
            }
            0
        }
        unsafe extern "C" fn add_spec(
            level: *mut c_void,
            spec: *const c_void,
            out: *mut i32,
        ) -> i32 {
            unsafe {
                let fixture = &mut *level.cast::<Fixture>();
                let index = fixture.spec_count;
                fixture.specs[index] = *spec.cast::<FReachSpec>();
                fixture.spec_count += 1;
                *out = index as i32;
            }
            0
        }

        let path_services = Hp2PathDefinitionServices {
            actor_count,
            actor_at,
            actor_is_a,
            clear_navigation_point_list: clear_navigation_points,
            actor_class_name_is: class_name_is,
            actor_same_class: same_class,
            reach_spec_add: add_spec,
        };

        let mut first: ANavigationPoint = unsafe { std::mem::zeroed() };
        let mut second: ANavigationPoint = unsafe { std::mem::zeroed() };
        let mut player_start: ANavigationPoint = unsafe { std::mem::zeroed() };
        first.paths = [-1; PATH_SLOTS];
        second.paths = [-1; PATH_SLOTS];
        player_start.paths = [-1; PATH_SLOTS];
        first.base.location = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        second.base.location = FVector {
            x: 100.0,
            y: 0.0,
            z: 0.0,
        };
        player_start.base.location = FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        let first_actor = ptr::addr_of_mut!(first.base);
        let second_actor = ptr::addr_of_mut!(second.base);
        let player_start_actor = ptr::addr_of_mut!(player_start.base);
        FIRST.store(first_actor as usize, Ordering::Relaxed);
        SECOND.store(second_actor as usize, Ordering::Relaxed);
        PLAYER_START.store(player_start_actor as usize, Ordering::Relaxed);

        let mut fixture = Fixture {
            level: unsafe { std::mem::zeroed() },
            actors: [first_actor, second_actor, player_start_actor],
            specs: unsafe { std::mem::zeroed() },
            spec_count: 0,
            navigation_list_cleared: false,
        };
        fixture.level.reach_specs.data = fixture.specs.as_mut_ptr().cast();

        assert_eq!(
            unsafe { define_paths(&path_services, &mut fixture.level) },
            Ok(())
        );

        assert!(fixture.navigation_list_cleared);
        assert_eq!(fixture.spec_count, 4);
        assert_eq!(fixture.specs[0].start, first_actor.cast());
        assert_eq!(fixture.specs[0].end, second_actor.cast());
        assert_eq!(fixture.specs[1].start, second_actor.cast());
        assert_eq!(fixture.specs[1].end, first_actor.cast());
        assert_eq!(fixture.specs[2].start, second_actor.cast());
        assert_eq!(fixture.specs[2].end, first_actor.cast());
        assert_eq!(fixture.specs[3].start, first_actor.cast());
        assert_eq!(fixture.specs[3].end, second_actor.cast());
        assert_eq!(fixture.specs[0].reach_flags, R_SPECIAL);
        assert_eq!(fixture.specs[0].distance, 500);
        assert!(player_start.paths.iter().all(|index| *index == -1));
    }
}

/// Creation-only engine leaves. No selection or traversal is performed by these callbacks.
#[repr(C)]
pub struct Hp2PathCreationServices {
    pub spawn_path: unsafe extern "C" fn(*mut c_void, *const c_void, *mut *mut c_void) -> i32,
    pub destroy_actor: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub auto_built: unsafe extern "C" fn(*mut c_void) -> i32,
    pub set_auto_built: unsafe extern "C" fn(*mut c_void),
    pub one_way: unsafe extern "C" fn(*mut c_void) -> i32,
    pub rotation_vector: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub fast_line: unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, *mut i32) -> i32,
    pub diagnostic: unsafe extern "C" fn(i32, *mut c_void, *mut c_void, f32, f32, f32) -> i32,
}

unsafe fn creation_log(
    svc: &Hp2PathCreationServices,
    code: i32,
    a: *mut ANavigationPoint,
    b: *mut ANavigationPoint,
    x: f32,
    y: f32,
    z: f32,
) -> Result<(), PathScoutThrown> {
    scout_status(unsafe { (svc.diagnostic)(code, a.cast(), b.cast(), x, y, z) })
}

unsafe fn creation_actor(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
    index: i32,
) -> Result<*mut ANavigationPoint, PathScoutThrown> {
    let mut actor = ptr::null_mut();
    scout_status(unsafe { (svc.actor_at)(level, index, &mut actor) })?;
    Ok(actor.cast())
}

unsafe fn live_nav(
    svc: &Hp2PathScoutServices,
    node: *mut ANavigationPoint,
) -> Result<bool, PathScoutThrown> {
    unsafe {
        Ok(!node.is_null()
            && !(*node.cast::<AActor>()).b_delete_me()
            && scout_is_a(svc, node.cast(), PCLASS_NAVIGATION_POINT)?)
    }
}

unsafe fn clear_creation_marks(
    svc: &Hp2PathScoutServices,
    level: *mut c_void,
) -> Result<(), PathScoutThrown> {
    unsafe {
        let mut i = 0;
        while i < (svc.actor_count)(level) {
            let node = creation_actor(svc, level, i)?;
            if live_nav(svc, node)? {
                (*node).visited_weight = 1;
                (*node).set_b_end_point(false);
            }
            i += 1;
        }
        Ok(())
    }
}

unsafe fn creation_line(
    svc: &Hp2PathCreationServices,
    level: *mut c_void,
    a: FVector,
    b: FVector,
) -> Result<bool, PathScoutThrown> {
    let mut result = 0;
    scout_status(unsafe {
        (svc.fast_line)(
            level,
            ptr::from_ref(&a).cast(),
            ptr::from_ref(&b).cast(),
            &mut result,
        )
    })?;
    Ok(result != 0)
}

unsafe fn forward_path(
    svc: &Hp2PathCreationServices,
    node: *mut ANavigationPoint,
    end: FVector,
) -> Result<bool, PathScoutThrown> {
    unsafe {
        if (svc.one_way)(node.cast()) == 0 {
            return Ok(true);
        }
        let mut direction = vec::zero();
        scout_status((svc.rotation_vector)(
            node.cast(),
            ptr::from_mut(&mut direction).cast(),
        ))?;
        Ok(dot(vec::sub(end, (*node).base.location), direction) > 0.0)
    }
}

fn distance_sq(a: FVector, b: FVector) -> f32 {
    let d = vec::sub(a, b);
    dot(d, d)
}

unsafe fn creation_reachable(
    svc: &Hp2PathScoutServices,
    scout: *mut APawn,
    end: FVector,
) -> Result<bool, PathScoutThrown> {
    unsafe {
        (*scout).base.physics = 1;
        let mut result = 0;
        scout_status((svc.point_reachable)(
            scout.cast(),
            ptr::from_ref(&end).cast(),
            &mut result,
        ))?;
        Ok(result != 0)
    }
}

/// Spawn a path at the legacy height and initialize its two reach-spec lists.
///
/// # Safety
/// Pointers and callback results must be live engine objects of the stated types.
pub unsafe fn new_path(
    svc: &Hp2PathCreationServices,
    level: *mut c_void,
    scout: *mut APawn,
    mut spot: FVector,
) -> Result<*mut ANavigationPoint, PathScoutThrown> {
    unsafe {
        if (*scout).base.collision_height < 48.0 {
            spot.z = spot.z + 48.0 - (*scout).base.collision_height;
        }
        let mut actor = ptr::null_mut();
        scout_status((svc.spawn_path)(
            level,
            ptr::from_ref(&spot).cast(),
            &mut actor,
        ))?;
        let node = actor.cast::<ANavigationPoint>();
        if node.is_null() {
            creation_log(svc, 0, node, ptr::null_mut(), 0.0, 0.0, 0.0)?;
            return Ok(node);
        }
        creation_log(
            svc,
            1,
            node,
            ptr::null_mut(),
            (*node).base.location.x,
            (*node).base.location.y,
            0.0,
        )?;
        (svc.set_auto_built)(node.cast());
        (*node).paths.fill(-1);
        (*node).upstream_paths.fill(-1);
        Ok(node)
    }
}

/// Multi-pass start traversal, reachable-node merging, intermediate placement,
/// and pawn collision restoration. Actor bounds are re-read after every mutation.
///
/// # Safety
/// Tables, level, Scout and returned actors must remain valid during each callback.
pub unsafe fn create_paths(
    svc: &Hp2PathScoutServices,
    creation: &Hp2PathCreationServices,
    level: *mut c_void,
    scout: *mut APawn,
    _optimization: i32,
) -> Result<i32, PathScoutThrown> {
    unsafe {
        clear_creation_marks(svc, level)?;
        let mut i = 0;
        while i < (svc.actor_count)(level) {
            let node = creation_actor(svc, level, i)?;
            if !node.is_null()
                && (scout_is_a(svc, node.cast(), 8)?
                    || scout_is_a(svc, node.cast(), PCLASS_PLAYER_START)?)
            {
                creation_log(creation, 2, node, ptr::null_mut(), 0.0, 0.0, 0.0)?;
                if !(*node).b_end_point() {
                    test_paths_from_with(
                        svc,
                        level,
                        scout,
                        (*node).base.location,
                        |start, direction, turn| {
                            pass2_from(svc, creation, level, scout, start, direction, turn)
                        },
                    )?;
                } else {
                    creation_log(creation, 3, node, ptr::null_mut(), 0.0, 0.0, 0.0)?;
                }
            }
            i += 1;
        }
        actor::set_collision_size(&actor_services()?, scout.cast(), 18.0, 39.0)
            .map_err(PathScoutThrown)?;
        i = 0;
        while i < (svc.actor_count)(level) {
            let node = creation_actor(svc, level, i)?;
            if live_nav(svc, node)? && (creation.auto_built)(node.cast()) != 0 {
                let mut j = 0;
                while j < (svc.actor_count)(level) {
                    let node2 = creation_actor(svc, level, j)?;
                    if valid_node(svc, node.cast(), node2.cast())? != 0
                        && distance_sq((*node).base.location, (*node2).base.location) < 16384.0
                        && forward_path(creation, node, (*node2).base.location)?
                        && creation_line(
                            creation,
                            level,
                            (*node).base.location,
                            (*node2).base.location,
                        )?
                    {
                        creation_log(creation, 4, node, node2, 0.0, 0.0, 0.0)?;
                        if test_reach(
                            svc,
                            level,
                            scout,
                            (*node).base.location,
                            (*node2).base.location,
                        )? != 0
                        {
                            let mut found_diff = false;
                            let mut must_not_avg = false;
                            let avg = vec::scale(
                                vec::add((*node).base.location, (*node2).base.location),
                                0.5,
                            );
                            let mut k = 0;
                            while k < (svc.actor_count)(level) {
                                let node3 = creation_actor(svc, level, k)?;
                                if valid_node(svc, node.cast(), node3.cast())? != 0
                                    && node3 != node2
                                    && distance_sq((*node).base.location, (*node3).base.location)
                                        < 640000.0
                                    && distance_sq((*node2).base.location, (*node3).base.location)
                                        < 640000.0
                                {
                                    actor::set_collision_size(
                                        &actor_services()?,
                                        scout.cast(),
                                        18.0,
                                        39.0,
                                    )
                                    .map_err(PathScoutThrown)?;
                                    if creation_line(
                                        creation,
                                        level,
                                        (*node).base.location,
                                        (*node3).base.location,
                                    )? {
                                        if test_reach(
                                            svc,
                                            level,
                                            scout,
                                            (*node).base.location,
                                            (*node3).base.location,
                                        )? != 0
                                        {
                                            found_diff = true;
                                        } else {
                                            actor::set_collision_size(
                                                &actor_services()?,
                                                scout.cast(),
                                                18.0,
                                                39.0,
                                            )
                                            .map_err(PathScoutThrown)?;
                                            if test_reach(
                                                svc,
                                                level,
                                                scout,
                                                (*node).base.location,
                                                (*node3).base.location,
                                            )? != 0
                                            {
                                                found_diff = true;
                                            }
                                        }
                                    }
                                    if found_diff
                                        && (creation.auto_built)(node2.cast()) != 0
                                        && !must_not_avg
                                    {
                                        if creation_line(
                                            creation,
                                            level,
                                            avg,
                                            (*node3).base.location,
                                        )? && test_reach(
                                            svc,
                                            level,
                                            scout,
                                            avg,
                                            (*node3).base.location,
                                        )? != 0
                                        {
                                            found_diff = false;
                                        }
                                        if found_diff {
                                            must_not_avg = true;
                                        }
                                    }
                                    if found_diff
                                        && creation_line(
                                            creation,
                                            level,
                                            (*node2).base.location,
                                            (*node3).base.location,
                                        )?
                                        && test_reach(
                                            svc,
                                            level,
                                            scout,
                                            (*node2).base.location,
                                            (*node3).base.location,
                                        )? != 0
                                    {
                                        found_diff = false;
                                    }
                                    if found_diff {
                                        break;
                                    }
                                }
                                k += 1;
                            }
                            if !found_diff {
                                let break_out = (creation.auto_built)(node2.cast()) == 0;
                                let (keeper, removed) = if break_out {
                                    (node2, node)
                                } else {
                                    (node, node2)
                                };
                                creation_log(creation, 5, removed, ptr::null_mut(), 0.0, 0.0, 0.0)?;
                                scout_status((creation.destroy_actor)(level, removed.cast()))?;
                                if !must_not_avg {
                                    (*keeper).base.location = avg;
                                    creation_log(
                                        creation,
                                        6,
                                        keeper,
                                        ptr::null_mut(),
                                        avg.x,
                                        avg.y,
                                        0.0,
                                    )?;
                                }
                                if break_out {
                                    break;
                                }
                            }
                            actor::set_collision_size(&actor_services()?, scout.cast(), 18.0, 39.0)
                                .map_err(PathScoutThrown)?;
                        }
                    }
                    j += 1;
                }
            }
            i += 1;
        }
        i = 0;
        while i < (svc.actor_count)(level) {
            let node = creation_actor(svc, level, i)?;
            if live_nav(svc, node)? && !scout_is_a(svc, node.cast(), PCLASS_LIFT_CENTER)? {
                let mut j = 0;
                while j < (svc.actor_count)(level) {
                    let node2 = creation_actor(svc, level, j)?;
                    if valid_node(svc, node.cast(), node2.cast())? != 0
                        && distance_sq((*node).base.location, (*node2).base.location) > 360000.0
                        && forward_path(creation, node, (*node2).base.location)?
                        && creation_line(
                            creation,
                            level,
                            (*node).base.location,
                            (*node2).base.location,
                        )?
                    {
                        creation_log(creation, 7, node, node2, 0.0, 0.0, 0.0)?;
                        scout_far_move(svc, level, scout, (*node).base.location, 0)?;
                        if creation_reachable(svc, scout, (*node2).base.location)? {
                            let mut found = false;
                            let total =
                                distance_sq((*node).base.location, (*node2).base.location).sqrt();
                            let total_sq = total * total;
                            let mut k = 0;
                            while k < (svc.actor_count)(level) {
                                let node3 = creation_actor(svc, level, k)?;
                                if valid_node(svc, node.cast(), node3.cast())? != 0
                                    && node3 != node2
                                    && distance_sq((*node).base.location, (*node3).base.location)
                                        < total_sq
                                    && distance_sq((*node2).base.location, (*node3).base.location)
                                        < total_sq
                                    && forward_path(creation, node3, (*node2).base.location)?
                                    && creation_line(
                                        creation,
                                        level,
                                        (*node).base.location,
                                        (*node3).base.location,
                                    )?
                                    && creation_line(
                                        creation,
                                        level,
                                        (*node2).base.location,
                                        (*node3).base.location,
                                    )?
                                {
                                    let d13 =
                                        distance_sq((*node).base.location, (*node3).base.location)
                                            .sqrt();
                                    let d32 =
                                        distance_sq((*node3).base.location, (*node2).base.location)
                                            .sqrt();
                                    creation_log(
                                        creation,
                                        8,
                                        node3,
                                        ptr::null_mut(),
                                        total,
                                        d13,
                                        d32,
                                    )?;
                                    if ((d13 + d32) as f64) < 1.3 * total as f64 {
                                        scout_far_move(
                                            svc,
                                            level,
                                            scout,
                                            (*node).base.location,
                                            0,
                                        )?;
                                        if creation_reachable(svc, scout, (*node3).base.location)? {
                                            scout_far_move(
                                                svc,
                                                level,
                                                scout,
                                                (*node3).base.location,
                                                0,
                                            )?;
                                            if creation_reachable(
                                                svc,
                                                scout,
                                                (*node2).base.location,
                                            )? {
                                                creation_log(
                                                    creation,
                                                    9,
                                                    node3,
                                                    ptr::null_mut(),
                                                    0.0,
                                                    0.0,
                                                    0.0,
                                                )?;
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                                k += 1;
                            }
                            if !found
                                && scout_far_move(
                                    svc,
                                    level,
                                    scout,
                                    vec::scale(
                                        vec::add((*node).base.location, (*node2).base.location),
                                        0.5,
                                    ),
                                    0,
                                )? != 0
                            {
                                new_path(creation, level, scout, (*scout).base.location)?;
                            }
                        }
                    }
                    j += 1;
                }
            }
            i += 1;
        }
        let mut count = 0;
        i = 0;
        while i < (svc.actor_count)(level) {
            let actor = creation_actor(svc, level, i)?;
            if !actor.is_null() {
                if scout_is_a(svc, actor.cast(), 9)? {
                    actor::set_collision(&actor_services()?, actor.cast(), true, true, true)
                        .map_err(PathScoutThrown)?;
                } else if scout_is_a(svc, actor.cast(), PCLASS_NAVIGATION_POINT)?
                    && (creation.auto_built)(actor.cast()) != 0
                {
                    count += 1;
                }
            }
            i += 1;
        }
        Ok(count)
    }
}

/// Follow a wall, selecting/placing anchors and stopping at the original loop,
/// repeated-anchor, right-turn and total-step limits. Uses only local traversal state.
///
/// # Safety
/// Tables and all actor pointers must obey the same lifetime rules as `create_paths`.
pub unsafe fn pass2_from(
    svc: &Hp2PathScoutServices,
    creation: &Hp2PathCreationServices,
    level: *mut c_void,
    scout: *mut APawn,
    _start: FVector,
    mut direction: FVector,
    turn: f32,
) -> Result<(), PathScoutThrown> {
    unsafe {
        creation_log(
            creation,
            10,
            ptr::null_mut(),
            ptr::null_mut(),
            (*scout).base.collision_radius,
            (*scout).base.collision_height,
            0.0,
        )?;
        clear_creation_marks(svc, level)?;
        let mut hit = scout_hit();
        // TestWalk takes FCheckResult by value; its modifications must not leak
        // into the caller's later floor check or the next TestWalk.
        while test_walk(
            svc,
            level,
            scout,
            vec::scale(direction, 16.0),
            &mut { hit },
            4.1,
            0,
        )? == 1
        {}
        let mut normal = vec::scale(direction, -1.0);
        find_blocking_normal(svc, level, scout, &mut normal)?;
        direction = FVector {
            x: normal.y,
            y: -normal.x,
            z: 0.0,
        };
        let mut anchor: *mut ANavigationPoint = ptr::null_mut();
        let mut best = 640000.0;
        let mut i = 0;
        while i < (svc.actor_count)(level) {
            let nav = creation_actor(svc, level, i)?;
            if live_nav(svc, nav)?
                && distance_sq((*nav).base.location, (*scout).base.location) < best
            {
                let real = (*scout).base.location;
                if test_reach(svc, level, scout, (*nav).base.location, real)? != 0 {
                    creation_log(creation, 11, nav, ptr::null_mut(), 0.0, 0.0, 0.0)?;
                    anchor = nav;
                    (*anchor).base.velocity = direction;
                    (*anchor).set_b_end_point(true);
                    best = distance_sq((*anchor).base.location, (*scout).base.location);
                }
                scout_far_move(svc, level, scout, real, 0)?;
            }
            i += 1;
        }
        let mut complete = 0;
        let mut steps = 0;
        let mut total = 0;
        let mut test_loc = (*scout).base.location;
        let mut right_turns = 0;
        while complete < 2 && steps < 1000 {
            if distance_sq(test_loc, (*scout).base.location) > 40000.0 {
                steps = 0;
                test_loc = (*scout).base.location;
            } else {
                steps += 1;
            }
            total += 1;
            if total > 200 {
                creation_log(
                    creation,
                    12,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    0.0,
                    0.0,
                    0.0,
                )?;
                steps = 2000;
            }
            let old_pos = (*scout).base.location;
            let right = FVector {
                x: -turn * direction.y,
                y: turn * direction.x,
                z: 0.0,
            };
            let try_turn = test_walk(
                svc,
                level,
                scout,
                vec::scale(right, 16.0),
                &mut { hit },
                12.0,
                0,
            )?;
            let still_moving = if try_turn == 1 {
                1
            } else {
                test_walk(
                    svc,
                    level,
                    scout,
                    vec::scale(direction, 16.0),
                    &mut { hit },
                    4.1,
                    0,
                )?
            };
            let start = (*scout).base.location;
            let end = vec::sub(
                start,
                FVector {
                    x: 0.0,
                    y: 0.0,
                    z: (*scout).base.collision_height + (*scout).max_step_height + 4.0,
                },
            );
            let extent = FVector {
                x: 16.0,
                y: 16.0,
                z: 1.0,
            };
            scout_status((svc.single_line_check)(
                level,
                scout.cast(),
                ptr::from_mut(&mut hit).cast(),
                ptr::from_ref(&end).cast(),
                ptr::from_ref(&start).cast(),
                ptr::from_ref(&extent).cast(),
            ))?;
            if !anchor.is_null() && hit.time < 1.0 {
                let real = (*scout).base.location;
                if test_reach(svc, level, scout, (*anchor).base.location, real)? == 0 {
                    let old_anchor = anchor;
                    let mut closest: *mut ANavigationPoint = ptr::null_mut();
                    let mut closest_dist = 65536.0;
                    best = 65536.0;
                    i = 0;
                    while i < (svc.actor_count)(level) {
                        let nav = creation_actor(svc, level, i)?;
                        if live_nav(svc, nav)? {
                            let dist = distance_sq((*nav).base.location, (*scout).base.location);
                            if dist < best {
                                if test_reach(svc, level, scout, (*nav).base.location, real)? != 0 {
                                    if dist < closest_dist {
                                        closest_dist = dist;
                                        closest = nav;
                                    }
                                    if test_reach(
                                        svc,
                                        level,
                                        scout,
                                        (*old_anchor).base.location,
                                        (*nav).base.location,
                                    )? != 0
                                        && test_reach(
                                            svc,
                                            level,
                                            scout,
                                            old_pos,
                                            (*nav).base.location,
                                        )? != 0
                                    {
                                        creation_log(
                                            creation,
                                            13,
                                            nav,
                                            ptr::null_mut(),
                                            0.0,
                                            0.0,
                                            0.0,
                                        )?;
                                        anchor = nav;
                                        if (*anchor).b_end_point()
                                            && (dot((*anchor).base.velocity, direction) as f64)
                                                > 0.9
                                        {
                                            complete += 1;
                                        } else {
                                            complete = 0;
                                        }
                                        (*anchor).base.velocity = direction;
                                        (*anchor).set_b_end_point(true);
                                        scout_far_move(svc, level, scout, real, 0)?;
                                        best = distance_sq(
                                            (*anchor).base.location,
                                            (*scout).base.location,
                                        );
                                    }
                                }
                                scout_far_move(svc, level, scout, real, 0)?;
                            }
                        }
                        i += 1;
                    }
                    if anchor == old_anchor {
                        let mut adjusted = false;
                        creation_log(
                            creation,
                            14,
                            ptr::null_mut(),
                            ptr::null_mut(),
                            0.0,
                            0.0,
                            0.0,
                        )?;
                        if closest_dist < 2500.0 {
                            let mut new = true;
                            let mut avg_ok = true;
                            let avg = vec::scale(vec::add(old_pos, (*closest).base.location), 0.5);
                            creation_log(creation, 15, closest, ptr::null_mut(), 0.0, 0.0, 0.0)?;
                            scout_far_move(svc, level, scout, (*closest).base.location, 0)?;
                            i = 0;
                            while i < (svc.actor_count)(level) {
                                let nav = creation_actor(svc, level, i)?;
                                if valid_node(svc, closest.cast(), nav.cast())? != 0
                                    && distance_sq((*closest).base.location, (*nav).base.location)
                                        < 640000.0
                                    && creation_reachable(svc, scout, (*nav).base.location)?
                                {
                                    if new {
                                        new = test_reach(
                                            svc,
                                            level,
                                            scout,
                                            old_pos,
                                            (*nav).base.location,
                                        )? != 0;
                                        scout_far_move(
                                            svc,
                                            level,
                                            scout,
                                            (*closest).base.location,
                                            0,
                                        )?;
                                    }
                                    if avg_ok {
                                        avg_ok = test_reach(
                                            svc,
                                            level,
                                            scout,
                                            avg,
                                            (*nav).base.location,
                                        )? != 0;
                                        scout_far_move(
                                            svc,
                                            level,
                                            scout,
                                            (*closest).base.location,
                                            0,
                                        )?;
                                    }
                                    if !new && !avg_ok {
                                        break;
                                    }
                                }
                                i += 1;
                            }
                            if avg_ok {
                                anchor = closest;
                                (*anchor).base.location = avg;
                                adjusted = true;
                            } else if new {
                                anchor = closest;
                                (*anchor).base.location = old_pos;
                                adjusted = true;
                            } else if closest_dist < 1000.0 {
                                adjusted = true;
                            }
                        }
                        if !adjusted {
                            anchor = new_path(creation, level, scout, old_pos)?;
                            if anchor.is_null() {
                                return Ok(());
                            }
                            if distance_sq((*old_anchor).base.location, (*anchor).base.location)
                                < 1000.0
                            {
                                scout_status((creation.destroy_actor)(level, anchor.cast()))?;
                                anchor = old_anchor;
                            } else if !closest.is_null()
                                && distance_sq((*closest).base.location, (*anchor).base.location)
                                    < 1000.0
                            {
                                scout_status((creation.destroy_actor)(level, anchor.cast()))?;
                                anchor = closest;
                            } else {
                                i = 0;
                                while i < (svc.actor_count)(level) {
                                    let nav = creation_actor(svc, level, i)?;
                                    if !nav.is_null()
                                        && nav != anchor
                                        && live_nav(svc, nav)?
                                        && distance_sq(
                                            (*anchor).base.location,
                                            (*nav).base.location,
                                        ) < 1000.0
                                        && creation_line(
                                            creation,
                                            level,
                                            (*anchor).base.location,
                                            (*nav).base.location,
                                        )?
                                    {
                                        scout_status((creation.destroy_actor)(
                                            level,
                                            anchor.cast(),
                                        ))?;
                                        anchor = nav;
                                        break;
                                    }
                                    i += 1;
                                }
                            }
                            (*anchor).visited_weight += 1;
                            if (*anchor).visited_weight > 10 {
                                steps = 5000;
                            }
                            (*anchor).base.velocity = direction;
                            (*anchor).set_b_end_point(true);
                            creation_log(
                                creation,
                                16,
                                anchor,
                                ptr::null_mut(),
                                (*anchor).base.location.x,
                                (*anchor).base.location.y,
                                0.0,
                            )?;
                        }
                        if anchor != old_anchor {
                            total = 0;
                        }
                    }
                }
                scout_far_move(svc, level, scout, real, 0)?;
            }
            if try_turn == 1 {
                if anchor.is_null() && right_turns == 0 {
                    anchor = new_path(creation, level, scout, old_pos)?;
                }
                right_turns += 1;
                if right_turns > 100 {
                    steps = 3000;
                }
                if right_turns < 3 {
                    direction = right;
                }
            } else if still_moving != 1 {
                right_turns = 4;
                normal = vec::scale(direction, -1.0);
                find_blocking_normal(svc, level, scout, &mut normal)?;
                direction = FVector {
                    x: -turn * normal.y,
                    y: turn * normal.x,
                    z: 0.0,
                };
            } else {
                right_turns = 0;
            }
        }
        creation_log(
            creation,
            17,
            ptr::null_mut(),
            ptr::null_mut(),
            steps as f32,
            0.0,
            0.0,
        )
    }
}
