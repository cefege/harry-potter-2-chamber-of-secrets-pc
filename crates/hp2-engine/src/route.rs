//! Port of `Engine/Src/UnRoute.cpp`: APawn route finding over the
//! navigation-point graph (`findPath`, `findPathToward`, `findPathTo`,
//! breadth searches, route cache).
//!
//! Engine calls that the C++ makes into other slices (level collision,
//! `APawn`/`AInventory` virtuals, class tests, logging) go through
//! [`Hp2RouteServices`], filled in by `HP2RouteServices.cpp`.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, AInventory, AInventorySpot, ANavigationPoint, APawn, FReachSpec, ULevel,
};

use crate::math::{app_round, app_sqrt};
use crate::reach::{R_FLY, R_SWIM};
use crate::vec;

/// `MAXSORTED` (`UnPath.h`).
pub const MAXSORTED: usize = 32;
/// Length of the `APawn::findPath` scratch arrays.
const SEARCH_LIST_LEN: usize = 100;

/// `EReachSpecFlags` (`UnPath.h`) not already exported by `reach.rs`.
const R_WALK: i32 = 1;
const R_JUMP: i32 = 8;
const R_DOOR: i32 = 16;
const R_SPECIAL: i32 = 32;
const R_PLAYERONLY: i32 = 64;

/// `EPhysics` (`EngineClasses.h`).
const PHYS_FALLING: u8 = 2;
const PHYS_FLYING: u8 = 4;

/// Class codes understood by `Hp2RouteServices::actor_is_a`.
pub const CLASS_NAVIGATION_POINT: i32 = 0;
pub const CLASS_PAWN: i32 = 1;
pub const CLASS_INVENTORY_SPOT: i32 = 2;
pub const CLASS_MOVER: i32 = 3;

/// `Hp2RouteServices::log_actor` kinds.
const LOG_START_NAME: i32 = 0;
const LOG_END_NAME: i32 = 1;
const LOG_BREADTH_OVERFLOW: i32 = 2;
const LOG_1000_SEARCHED: i32 = 3;

/// Per-slice engine services for the route code. Filled by
/// `HP2RouteServices.cpp` (`GHp2RouteServices`). Pointers are the engine's
/// `ULevel`, `AActor`, `APawn`, `UModel`, `AMover`, `ANavigationPoint`,
/// `AInventory` objects.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2RouteServices {
    /// `appFrand()`.
    pub app_frand: unsafe extern "C" fn() -> f32,
    /// `ULevel::FarMoveActor(actor, dest, test, noCheck)`.
    pub far_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        dest: *const c_void,
        test: i32,
        no_check: i32,
    ) -> i32,
    /// `APawn::pointReachable(point, knowVisible)`.
    pub point_reachable:
        unsafe extern "C" fn(pawn: *mut c_void, point: *const c_void, know_visible: i32) -> i32,
    /// `APawn::actorReachable(other, knowVisible)`.
    pub actor_reachable:
        unsafe extern "C" fn(pawn: *mut c_void, other: *mut c_void, know_visible: i32) -> i32,
    /// `((APawn*)goal)->jumpLanding(goal->Velocity, *dest)`.
    pub goal_jump_landing: unsafe extern "C" fn(goal: *mut c_void, dest: *mut c_void),
    /// `UModel::FastLineCheck(end, start)`.
    pub model_fast_line_check:
        unsafe extern "C" fn(model: *mut c_void, end: *const c_void, start: *const c_void) -> u8,
    /// `ULevel::SingleLineCheck(Hit, source, end, start, TRACE_VisBlocking)`;
    /// returns `Hit.Actor`.
    pub single_line_hit_actor: unsafe extern "C" fn(
        level: *mut c_void,
        source: *mut c_void,
        end: *const c_void,
        start: *const c_void,
    ) -> *mut c_void,
    /// `ULevel::GetLevelInfo()->NavigationPointList`.
    pub navigation_point_list: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `actor->IsA(class)` for the `CLASS_*` codes above.
    pub actor_is_a: unsafe extern "C" fn(actor: *mut c_void, kind: i32) -> i32,
    /// `((AMover*)mover)->bPlayerOnly`.
    pub mover_player_only: unsafe extern "C" fn(mover: *mut c_void) -> i32,
    /// `((ANavigationPoint*)node)->eventSpecialCost(seeker)`.
    pub event_special_cost: unsafe extern "C" fn(node: *mut c_void, seeker: *mut c_void) -> i32,
    /// `((AInventory*)item)->IsProbing(NAME_Touch)`.
    pub item_is_probing_touch: unsafe extern "C" fn(item: *mut c_void) -> i32,
    /// `((AInventory*)item)->eventBotDesireability(bot)`.
    pub item_bot_desirability: unsafe extern "C" fn(item: *mut c_void, bot: *mut c_void) -> f32,
    /// `actor->GetFName().GetIndex()`.
    pub actor_name_index: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `actor->GetClass()` (identity only).
    pub actor_class: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// Engine `debugf` with the actor's name (kinds `LOG_*`).
    pub log_actor: unsafe extern "C" fn(kind: i32, actor: *mut c_void),
}

/// `FSortedPathList` (`UnPath.h`). Same layout as the C++ class so the C++
/// methods can forward `this`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2SortedPathList {
    pub path: [*mut c_void; 32],
    pub dist: [i32; 32],
    pub num_points: i32,
    pub check_pos: i32,
}

impl Hp2SortedPathList {
    /// Zeroed list (the C++ leaves these uninitialized; only the `numPoints`
    /// prefix is ever read).
    pub fn empty() -> Self {
        Self {
            path: [ptr::null_mut(); 32],
            dist: [0; 32],
            num_points: 0,
            check_pos: 0,
        }
    }
}

/// `FVector::operator|` (dot product) with the `vec::size_squared` FMA
/// convention.
#[inline]
fn dot(a: FVector, b: FVector) -> f32 {
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `Abs(x)` on FLOAT.
#[inline]
fn fabs(x: f32) -> f32 {
    if x >= 0.0 { x } else { -x }
}

/// `appRound(appSqrt(d))` for an INT `d` (`appSqrt` takes DOUBLE).
#[inline]
fn round_sqrt(d: i32) -> i32 {
    app_round(app_sqrt(f64::from(d)) as f32)
}

/// `FReachSpec::supports(iRadius, iHeight, moveFlags)` (`UnReach.h`).
///
/// # Safety
/// `spec` must point to a live `FReachSpec`.
unsafe fn supports(spec: *const FReachSpec, i_radius: i32, i_height: i32, move_flags: i32) -> bool {
    unsafe {
        (*spec).collision_radius >= i_radius
            && (*spec).collision_height >= i_height
            && ((*spec).reach_flags & move_flags) == (*spec).reach_flags
    }
}

/// `APawn::calcMoveFlags()` (`APawn.h`).
///
/// # Safety
/// `pawn` must point to a live `APawn`.
pub unsafe fn calc_move_flags(pawn: *const APawn) -> i32 {
    unsafe {
        (*pawn).b_can_walk() as i32 * R_WALK
            + (*pawn).b_can_fly() as i32 * R_FLY
            + (*pawn).b_can_swim() as i32 * R_SWIM
            + (*pawn).b_can_jump() as i32 * R_JUMP
            + (*pawn).b_can_open_doors() as i32 * R_DOOR
            + (*pawn).b_can_do_special() as i32 * R_SPECIAL
            + (*pawn).b_is_player() as i32 * R_PLAYERONLY
    }
}

/// `ULevel::ReachSpecs(i)`.
///
/// # Safety
/// `level` must be live and `idx` a valid reach-spec index.
unsafe fn spec_at(level: *const ULevel, idx: i32) -> *mut FReachSpec {
    unsafe { ((*level).reach_specs.data as *mut FReachSpec).offset(idx as isize) }
}

/// `APawn::XLevel` as a `ULevel`.
///
/// # Safety
/// `pawn` must point to a live `APawn`.
unsafe fn level_of(pawn: *const APawn) -> *mut ULevel {
    unsafe { (*pawn).base.x_level.cast() }
}

/// `actor->IsA(class)`.
///
/// # Safety
/// `s` must be valid; `actor` must be non-null.
unsafe fn is_a(s: &Hp2RouteServices, actor: *mut c_void, class: i32) -> bool {
    unsafe { (s.actor_is_a)(actor, class) != 0 }
}

/// `ULevel::SingleLineCheck(Hit, source, end, start)` returning `Hit.Actor`.
///
/// # Safety
/// `s` must be valid; `level`/`source` live; `end`/`start` valid vectors.
unsafe fn single_line_hit(
    s: &Hp2RouteServices,
    level: *mut ULevel,
    source: *mut APawn,
    end: *const FVector,
    start: *const FVector,
) -> *mut c_void {
    unsafe { (s.single_line_hit_actor)(level.cast(), source.cast(), end.cast(), start.cast()) }
}

/// `ULevel::FarMoveActor(pawn, dest, test, noCheck)`.
///
/// # Safety
/// `s` must be valid; `level`/`pawn` live; `dest` a valid vector.
unsafe fn far_move(
    s: &Hp2RouteServices,
    level: *mut ULevel,
    pawn: *mut APawn,
    dest: *const FVector,
    test: i32,
    no_check: i32,
) -> i32 {
    unsafe { (s.far_move_actor)(level.cast(), pawn.cast(), dest.cast(), test, no_check) }
}

/// `ULevel::Model->FastLineCheck(end, start)`.
///
/// # Safety
/// `level` must be live with a valid `Model`.
unsafe fn fast_line(
    s: &Hp2RouteServices,
    level: *mut ULevel,
    end: *const FVector,
    start: *const FVector,
) -> bool {
    unsafe { (s.model_fast_line_check)((*level).model, end.cast(), start.cast()) != 0 }
}

/// `APawn::pointReachable(point, knowVisible)`.
///
/// # Safety
/// `s` valid; `pawn` live; `point` a valid vector.
unsafe fn point_reachable(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    point: *const FVector,
    know: i32,
) -> bool {
    unsafe { (s.point_reachable)(pawn.cast(), point.cast(), know) != 0 }
}

/// `APawn::actorReachable(other, knowVisible)`.
///
/// # Safety
/// `s` valid; `pawn`/`other` live.
unsafe fn actor_reachable(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    other: *mut AActor,
    know: i32,
) -> bool {
    unsafe { (s.actor_reachable)(pawn.cast(), other.cast(), know) != 0 }
}

/// `ULevel::GetLevelInfo()->NavigationPointList`.
///
/// # Safety
/// `s` valid; `level` live.
unsafe fn nav_list(s: &Hp2RouteServices, level: *mut ULevel) -> *mut ANavigationPoint {
    unsafe { (s.navigation_point_list)(level.cast()).cast() }
}

/// `ANavigationPoint::clearPath()` (`UnRoute.cpp`).
///
/// # Safety
/// `s` valid; `pawn` and `node` live.
unsafe fn clear_path(s: &Hp2RouteServices, pawn: *mut APawn, node: *mut ANavigationPoint) {
    unsafe {
        (*node).visited_weight = 10_000_000;
        (*node).next_ordered = ptr::null_mut();
        (*node).prev_ordered = ptr::null_mut();
        (*node).set_b_end_point(false);
        if (*node).b_special_cost() {
            (*node).cost = (s.event_special_cost)(node.cast(), pawn.cast());
        } else {
            (*node).cost = (*node).extra_cost;
        }
    }
}

/// `APawn::clearPaths()`.
///
/// # Safety
/// `s` valid; `pawn` live with a valid `XLevel`.
pub unsafe fn clear_paths(s: &Hp2RouteServices, pawn: *mut APawn) {
    unsafe {
        let level = level_of(pawn);
        let mut nav = nav_list(s, level);
        while !nav.is_null() {
            clear_path(s, pawn, nav);
            nav = (*nav).next_navigation_point.cast();
        }
    }
}

impl Hp2SortedPathList {
    /// `FSortedPathList::addPath(node, dist)`.
    ///
    /// # Safety
    /// `self.path` entries are only read below `num_points`.
    pub unsafe fn add_path(&mut self, node: *mut AActor, dist: i32) {
        let np = self.num_points;
        let mut n: i32 = 0;
        if np > 8 {
            if dist > self.dist[(np / 2) as usize] {
                n = np / 2;
                if np > 16 && dist > self.dist[(n + np / 4) as usize] {
                    n += np / 4;
                }
            } else if np > 16 && dist > self.dist[(np / 4) as usize] {
                n = np / 4;
            }
        }
        while n < self.num_points && dist > self.dist[n as usize] {
            n += 1;
        }
        if (n as usize) < MAXSORTED {
            let mut next_path = self.path[n as usize];
            let mut next_dist = self.dist[n as usize] as f32;
            self.path[n as usize] = node.cast();
            self.dist[n as usize] = dist;
            if (self.num_points as usize) < MAXSORTED {
                self.num_points += 1;
            }
            n += 1;
            while n < self.num_points {
                let after_path = self.path[n as usize];
                let after_dist = self.dist[n as usize] as f32;
                self.path[n as usize] = next_path;
                self.dist[n as usize] = app_round(next_dist);
                next_path = after_path;
                next_dist = after_dist;
                n += 1;
            }
        }
    }

    /// `FSortedPathList::removePath(p)`.
    pub fn remove_path(&mut self, p: i32) {
        let mut n = p;
        while n < self.num_points - 1 {
            self.path[n as usize] = self.path[(n + 1) as usize];
            self.dist[n as usize] = self.dist[(n + 1) as usize];
            n += 1;
        }
        self.num_points -= 1;
    }

    /// `FSortedPathList::FindVisiblePaths`.
    ///
    /// # Safety
    /// All pointers must be live engine objects.
    #[allow(clippy::too_many_arguments)] // mirrors FSortedPathList::FindVisiblePaths's C++ signature
    pub unsafe fn find_visible_paths(
        &mut self,
        s: &Hp2RouteServices,
        pawn: *mut APawn,
        dest: FVector,
        dest_points: &mut Hp2SortedPathList,
        clear: i32,
        startanchor: &mut i32,
        endanchor: &mut i32,
    ) {
        unsafe {
            let level = level_of(pawn);
            let a_loc = (*pawn).base.location;
            let a_h = (*pawn).base.collision_height;
            let mt = (*pawn).move_target.cast::<AActor>();
            if !mt.is_null()
                && is_a(s, mt.cast(), CLASS_NAVIGATION_POINT)
                && fabs((*mt).location.z - a_loc.z) < a_h
            {
                let mut targ_dir = vec::sub((*mt).location, a_loc);
                targ_dir.z = 0.0;
                let mut max_dist = (*pawn).base.collision_radius * (*pawn).base.collision_radius;
                if (*pawn).b_is_player() && is_a(s, mt.cast(), CLASS_INVENTORY_SPOT) {
                    max_dist *= 2.0;
                }
                if dot(targ_dir, targ_dir) < max_dist {
                    *startanchor = 1;
                    self.path[0] = mt.cast();
                    self.dist[0] = 0;
                    self.num_points = 1;
                }
            }

            let mut nav = nav_list(s, level);
            while !nav.is_null() {
                if clear != 0 {
                    clear_path(s, pawn, nav);
                }
                if *startanchor == 0 {
                    let dist = app_round(vec::size_squared(vec::sub(a_loc, (*nav).base.location)));
                    if dist < 640_000 {
                        self.add_path(nav.cast(), dist);
                    }
                }
                if *endanchor == 0 {
                    let dist = app_round(vec::size_squared(vec::sub(dest, (*nav).base.location)));
                    if dist < 640_000 {
                        dest_points.add_path(nav.cast(), dist);
                    }
                }
                nav = (*nav).next_navigation_point.cast();
            }
        }
    }

    /// `FSortedPathList::findEndPoint(Searcher, startanchor)`.
    ///
    /// # Safety
    /// `pawn` live; list entries below `num_points` are live actors.
    pub unsafe fn find_end_point(
        &mut self,
        s: &Hp2RouteServices,
        pawn: *mut APawn,
        startanchor: &mut i32,
    ) -> i32 {
        unsafe {
            let level = level_of(pawn);
            let mut view = (*pawn).base.location;
            view.z += (*pawn).base_eye_height;
            while self.num_points > 0 {
                let p0: *mut ANavigationPoint = self.path[0].cast();
                let p0_loc = (*p0).base.location;
                if fast_line(s, level, &p0_loc, &view) && point_reachable(s, pawn, &p0_loc, 1) {
                    self.dist[0] = round_sqrt(self.dist[0]);
                    let a_loc = (*pawn).base.location;
                    if self.dist[0] < 48.max(app_round((*pawn).base.collision_radius))
                        && fabs(p0_loc.z - a_loc.z) < (*pawn).base.collision_height
                    {
                        *startanchor = 1;
                    } else {
                        (*p0).set_b_end_point(true);
                        (*p0).best_path_weight = self.dist[0];
                    }
                    return 1;
                } else {
                    self.remove_path(0);
                }
            }
            0
        }
    }

    /// `FSortedPathList::checkAnchorPath(Searcher, Dest)`.
    ///
    /// # Safety
    /// `pawn` live; `path[0]` a live actor.
    pub unsafe fn check_anchor_path(
        &mut self,
        s: &Hp2RouteServices,
        pawn: *mut APawn,
        dest: FVector,
    ) -> i32 {
        unsafe {
            let level = level_of(pawn);
            let real = (*pawn).base.location;
            let p0: *mut AActor = self.path[0].cast();
            let p0_loc = (*p0).location;
            if vec::size_squared(vec::sub(dest, p0_loc)) < 640_000.0
                && fast_line(s, level, &p0_loc, &dest)
                && far_move(s, level, pawn, &p0_loc, 1, 0) != 0
            {
                if point_reachable(s, pawn, &dest, 0) {
                    return 1;
                }
                far_move(s, level, pawn, &real, 1, 1);
            }
            self.num_points = 1;
            0
        }
    }

    /// `FSortedPathList::expandAnchor(Searcher)`.
    ///
    /// # Safety
    /// `pawn` live; `path[0]` a live navigation point.
    pub unsafe fn expand_anchor(&mut self, s: &Hp2RouteServices, pawn: *mut APawn) {
        unsafe {
            let level = level_of(pawn);
            let anchor: *mut ANavigationPoint = self.path[0].cast();
            (*anchor).cost = 1_000_000;
            let move_flags = calc_move_flags(pawn);
            let i_radius = app_round((*pawn).base.collision_radius);
            let i_height = app_round((*pawn).base.collision_height);
            let mut j: usize = 0;
            while j < 16 {
                if (*anchor).paths[j] == -1 {
                    j = 16;
                } else {
                    let spec = spec_at(level, (*anchor).paths[j]);
                    if supports(spec, i_radius, i_height, move_flags) {
                        let end: *mut ANavigationPoint = (*spec).end.cast();
                        let start: *mut ANavigationPoint = (*spec).start.cast();
                        let hit = single_line_hit(
                            s,
                            level,
                            pawn,
                            &(*end).base.location,
                            &(*start).base.location,
                        );
                        if hit.is_null()
                            || !is_a(s, hit, CLASS_MOVER)
                            || ((*pawn).b_can_open_doors()
                                && ((*pawn).b_is_player() || (s.mover_player_only)(hit) == 0))
                        {
                            (*end).set_b_end_point(true);
                            (*end).best_path_weight = (*spec).distance;
                        }
                    }
                    j += 1;
                }
            }
        }
    }

    /// `FSortedPathList::findAltEndPoint(Searcher, bestPath)`.
    ///
    /// # Safety
    /// `pawn` live; `best_path` a live actor; list entries below `num_points` live.
    pub unsafe fn find_alt_end_point(
        &mut self,
        s: &Hp2RouteServices,
        pawn: *mut APawn,
        best_path: &mut *mut AActor,
    ) {
        unsafe {
            let level = level_of(pawn);
            let best_dist =
                (*(self.path[0] as *mut ANavigationPoint)).visited_weight + self.dist[0];
            let mut alt = Hp2SortedPathList::empty();
            let a_loc = (*pawn).base.location;
            for j in 1..self.num_points as usize {
                let pj: *mut ANavigationPoint = self.path[j].cast();
                let mut new_dist = round_sqrt(self.dist[j]);
                new_dist = new_dist.wrapping_add((*pj).visited_weight);
                let pj_loc = (*pj).base.location;
                let best_loc = (*(*best_path)).location;
                if new_dist < best_dist
                    && fabs(pj_loc.z - a_loc.z) < 120.0
                    && (dot(vec::sub(pj_loc, a_loc), vec::sub(best_loc, a_loc)) < 0.0
                        || new_dist
                            < app_round(0.85_f32 * best_dist as f32)
                                .max(best_dist.wrapping_sub(150)))
                {
                    alt.add_path(pj.cast(), new_dist);
                }
            }
            let mut view = a_loc;
            view.z += (*pawn).base_eye_height;
            for j in 0..alt.num_points as usize {
                let aj: *mut AActor = alt.path[j].cast();
                let aj_loc = (*aj).location;
                if fast_line(s, level, &aj_loc, &view) && point_reachable(s, pawn, &aj_loc, 1) {
                    *best_path = aj;
                    return;
                }
            }
        }
    }
}

/// `APawn::canMoveTo(anchor, dest)`.
///
/// # Safety
/// `s` valid; `pawn`, `anchor`, `dest` live.
pub unsafe fn can_move_to(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    anchor: *mut AActor,
    dest: *mut AActor,
) -> i32 {
    unsafe {
        let level = level_of(pawn);
        let start: *mut ANavigationPoint = anchor.cast();
        let i_radius = app_round((*pawn).base.collision_radius);
        let i_height = app_round((*pawn).base.collision_height);
        let move_flags = calc_move_flags(pawn);
        for pass in 0..2 {
            let list: *const [i32; 16] = if pass == 0 {
                &(*start).paths
            } else {
                &(*start).pruned_paths
            };
            let mut j: usize = 0;
            while j < 16 {
                if (*list)[j] == -1 {
                    j = 16;
                } else {
                    let spec = spec_at(level, (*list)[j]);
                    if (*spec).end == dest.cast() && supports(spec, i_radius, i_height, move_flags)
                    {
                        let hit = single_line_hit(
                            s,
                            level,
                            pawn,
                            &(*dest).location,
                            &(*start).base.location,
                        );
                        if hit.is_null()
                            || !is_a(s, hit, CLASS_MOVER)
                            || ((*pawn).b_can_open_doors()
                                && ((*pawn).b_is_player() || (s.mover_player_only)(hit) == 0))
                        {
                            return 1;
                        }
                    }
                    j += 1;
                }
            }
        }
        0
    }
}

/// `APawn::findPathToward(goal, bSinglePath, bestPath, bClearPaths)`.
///
/// # Safety
/// `s` valid; `pawn` live; `goal` live or null; `best_path` writable.
pub unsafe fn find_path_toward(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    goal: *mut AActor,
    single: i32,
    best_path: &mut *mut AActor,
    clear: i32,
) -> i32 {
    unsafe {
        *best_path = ptr::null_mut();
        if goal.is_null() {
            return 0;
        }
        let level = level_of(pawn);
        if nav_list(s, level).is_null() || (*level).reach_specs.num == 0 {
            return 0;
        }
        if (*pawn).base.physics != PHYS_FLYING
            && is_a(s, goal.cast(), CLASS_PAWN)
            && (*goal).physics == PHYS_FALLING
        {
            let mut dest = (*goal).location;
            (s.goal_jump_landing)(goal.cast(), (&mut dest as *mut FVector).cast());
            return find_path_to(s, pawn, dest, single, best_path, clear);
        }

        let real = (*pawn).base.location;
        let mut end_points = Hp2SortedPathList::empty();
        let mut dest_points = Hp2SortedPathList::empty();
        let mut startanchor: i32 = 0;
        let mut endanchor: i32 = 0;
        if is_a(s, goal.cast(), CLASS_NAVIGATION_POINT) {
            endanchor = 1;
            dest_points.path[0] = goal.cast();
            dest_points.dist[0] = 0;
            dest_points.num_points = 1;
        }
        end_points.find_visible_paths(
            s,
            pawn,
            (*goal).location,
            &mut dest_points,
            clear,
            &mut startanchor,
            &mut endanchor,
        );
        if end_points.num_points == 0 || dest_points.num_points == 0 {
            return 0;
        }
        if startanchor == 0 && end_points.find_end_point(s, pawn, &mut startanchor) == 0 {
            far_move(s, level, pawn, &real, 1, 1);
            return 0;
        }
        if startanchor != 0 {
            if (is_a(s, goal.cast(), CLASS_NAVIGATION_POINT)
                && can_move_to(s, pawn, end_points.path[0].cast(), goal) != 0)
                || end_points.check_anchor_path(s, pawn, (*goal).location) != 0
            {
                *best_path = if is_a(s, goal.cast(), CLASS_NAVIGATION_POINT) {
                    goal
                } else {
                    end_points.path[0].cast()
                };
                far_move(s, level, pawn, &real, 1, 1);
                return 1;
            } else {
                end_points.expand_anchor(s, pawn);
            }
        }
        if endanchor == 0 {
            let goal_loc = (*goal).location;
            let mut j: i32 = 0;
            while j < dest_points.num_points {
                let dj: *mut AActor = dest_points.path[j as usize].cast();
                let dj_loc = (*dj).location;
                if fast_line(s, level, &goal_loc, &dj_loc)
                    && far_move(s, level, pawn, &dj_loc, 1, 0) != 0
                    && actor_reachable(s, pawn, goal, 1)
                {
                    endanchor = 1;
                    dest_points.path[0] = dest_points.path[j as usize];
                    dest_points.dist[0] = round_sqrt(dest_points.dist[j as usize]);
                    j = dest_points.num_points;
                }
                j += 1;
            }
        }
        if endanchor == 0 && (*pawn).b_hunting() {
            endanchor = 1;
        }
        if endanchor != 0 {
            let mut new_path: *mut AActor = ptr::null_mut();
            let move_flags = calc_move_flags(pawn);
            let d0: *mut ANavigationPoint = dest_points.path[0].cast();
            (*d0).visited_weight = dest_points.dist[0];
            if breadth_path_from(
                s,
                pawn,
                dest_points.path[0].cast(),
                &mut new_path,
                single,
                move_flags,
            ) != 0
            {
                *best_path = new_path;
                far_move(s, level, pawn, &real, 1, 1);
                if startanchor == 0 && single == 0 {
                    end_points.find_alt_end_point(s, pawn, best_path);
                }
                set_route_cache(pawn, (*best_path).cast());
                return 1;
            }
        }
        far_move(s, level, pawn, &real, 1, 1);
        0
    }
}

/// `APawn::findPathTowardBestInventory(...)`.
///
/// # Safety
/// `s` valid; `pawn` live; `best_path` writable.
pub unsafe fn find_path_toward_best_inventory(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    best_path: &mut *mut AActor,
    clear: i32,
    min_weight: f32,
    predict: i32,
) -> f32 {
    unsafe {
        *best_path = ptr::null_mut();
        let level = level_of(pawn);
        if nav_list(s, level).is_null() || (*level).reach_specs.num == 0 {
            return 0.0;
        }
        let real = (*pawn).base.location;
        let mut end_points = Hp2SortedPathList::empty();
        let mut dest_points = Hp2SortedPathList::empty();
        let mut startanchor: i32 = 0;
        let mut endanchor: i32 = 1;
        end_points.find_visible_paths(
            s,
            pawn,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            &mut dest_points,
            clear,
            &mut startanchor,
            &mut endanchor,
        );
        if end_points.num_points == 0 {
            return 0.0;
        }
        if startanchor == 0 && end_points.find_end_point(s, pawn, &mut startanchor) == 0 {
            far_move(s, level, pawn, &real, 1, 1);
            return 0.0;
        }
        if startanchor == 0 {
            *best_path = end_points.path[0].cast();
            far_move(s, level, pawn, &real, 1, 1);
            return 0.00005;
        }
        end_points.expand_anchor(s, pawn);
        let mut new_path: *mut AActor = ptr::null_mut();
        let move_flags = calc_move_flags(pawn);
        let p0: *mut ANavigationPoint = end_points.path[0].cast();
        (*p0).visited_weight = 10.max(end_points.dist[0]);
        let best_inv = breadth_path_to_inventory(
            s,
            pawn,
            end_points.path[0].cast(),
            &mut new_path,
            move_flags,
            min_weight,
            predict,
        );
        if best_inv > min_weight {
            *best_path = new_path;
            set_route_cache(pawn, (*best_path).cast());
            far_move(s, level, pawn, &real, 1, 1);
            return best_inv;
        }
        far_move(s, level, pawn, &real, 1, 1);
        0.0
    }
}

/// `APawn::findPathTo(Dest, bSinglePath, bestPath, bClearPaths)`.
///
/// # Safety
/// `s` valid; `pawn` live; `best_path` writable.
pub unsafe fn find_path_to(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    dest: FVector,
    single: i32,
    best_path: &mut *mut AActor,
    clear: i32,
) -> i32 {
    unsafe {
        *best_path = ptr::null_mut();
        let level = level_of(pawn);
        if nav_list(s, level).is_null() || (*level).reach_specs.num == 0 {
            return 0;
        }
        let real = (*pawn).base.location;
        let mut end_points = Hp2SortedPathList::empty();
        let mut dest_points = Hp2SortedPathList::empty();
        let mut startanchor: i32 = 0;
        let mut endanchor: i32 = 0;
        end_points.find_visible_paths(
            s,
            pawn,
            dest,
            &mut dest_points,
            clear,
            &mut startanchor,
            &mut endanchor,
        );
        if end_points.num_points == 0 || dest_points.num_points == 0 {
            return 0;
        }
        if startanchor == 0 && end_points.find_end_point(s, pawn, &mut startanchor) == 0 {
            far_move(s, level, pawn, &real, 1, 1);
            return 0;
        }
        if startanchor != 0 {
            if end_points.check_anchor_path(s, pawn, dest) != 0 {
                *best_path = end_points.path[0].cast();
                far_move(s, level, pawn, &real, 1, 1);
                return 1;
            } else {
                end_points.expand_anchor(s, pawn);
            }
        }
        if endanchor == 0 {
            let mut j: i32 = 0;
            while j < dest_points.num_points {
                let dj: *mut AActor = dest_points.path[j as usize].cast();
                let dj_loc = (*dj).location;
                if fast_line(s, level, &dest, &dj_loc)
                    && far_move(s, level, pawn, &dj_loc, 1, 0) != 0
                    && point_reachable(s, pawn, &dest, 1)
                {
                    endanchor = 1;
                    dest_points.path[0] = dest_points.path[j as usize];
                    dest_points.dist[0] = round_sqrt(dest_points.dist[j as usize]);
                    j = dest_points.num_points;
                }
                j += 1;
            }
        }
        if endanchor != 0 {
            let mut new_path: *mut AActor = ptr::null_mut();
            let move_flags = calc_move_flags(pawn);
            let d0: *mut ANavigationPoint = dest_points.path[0].cast();
            (*d0).visited_weight = dest_points.dist[0];
            if breadth_path_from(
                s,
                pawn,
                dest_points.path[0].cast(),
                &mut new_path,
                single,
                move_flags,
            ) != 0
            {
                *best_path = new_path;
                far_move(s, level, pawn, &real, 1, 1);
                if startanchor == 0 && single == 0 {
                    end_points.find_alt_end_point(s, pawn, best_path);
                }
                set_route_cache(pawn, (*best_path).cast());
                return 1;
            }
        }
        far_move(s, level, pawn, &real, 1, 1);
        0
    }
}

/// `APawn::findRandomDest(bestDest)`.
///
/// # Safety
/// `s` valid; `pawn` live; `best` writable.
pub unsafe fn find_random_dest(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    best: &mut *mut AActor,
) -> i32 {
    unsafe {
        *best = ptr::null_mut();
        let level = level_of(pawn);
        let nav0 = nav_list(s, level);
        if nav0.is_null() || (*level).reach_specs.num == 0 {
            return 0;
        }
        let mut end_points = Hp2SortedPathList::empty();
        let a_loc = (*pawn).base.location;
        let mut view = a_loc;
        view.z += (*pawn).base_eye_height;
        let mut actor = nav0;
        while !actor.is_null() {
            let act_loc = (*actor).base.location;
            if vec::size_squared(vec::sub(a_loc, act_loc)) < 250_000.0
                && fast_line(s, level, &act_loc, &view)
            {
                let dir = vec::sub(act_loc, a_loc);
                let dist = app_round(vec::size(dir));
                end_points.add_path(actor.cast(), dist);
                if end_points.num_points == 4 {
                    break;
                }
            }
            actor = (*actor).next_navigation_point.cast();
        }

        let move_flags = calc_move_flags(pawn);
        let mut num_reached: i32 = 0;
        for i in 0..end_points.num_points as usize {
            let ei: *mut ANavigationPoint = end_points.path[i].cast();
            if !(*ei).b_end_point() && actor_reachable(s, pawn, end_points.path[i].cast(), 1) {
                num_reached = num_reached.wrapping_add(traverse_from(
                    s,
                    pawn,
                    end_points.path[i].cast(),
                    move_flags,
                ));
            }
        }
        if num_reached == 0 {
            return 0;
        }
        let mut result = 0;
        actor = nav0;
        while !actor.is_null() {
            if (*actor).b_end_point() {
                result = 1;
                *best = actor.cast();
                if (s.app_frand)() * (num_reached as f32) <= 1.0 {
                    break;
                }
                num_reached -= 1;
            }
            actor = (*actor).next_navigation_point.cast();
        }
        result
    }
}

/// `APawn::traverseFrom(start, moveFlags)`.
///
/// # Safety
/// `s` valid; `pawn` live; `start` a live navigation point.
pub unsafe fn traverse_from(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    start: *mut AActor,
    move_flags: i32,
) -> i32 {
    unsafe {
        let mut num_marked: i32 = 1;
        let node: *mut ANavigationPoint = start.cast();
        let i_radius = app_round((*pawn).base.collision_radius);
        let i_height = app_round((*pawn).base.collision_height);
        (*node).set_b_end_point(true);
        let level = level_of(pawn);
        let mut i: usize = 0;
        while i < 16 {
            if (*node).paths[i] == -1 {
                i = 16;
            } else {
                let spec = spec_at(level, (*node).paths[i]);
                let end: *mut AActor = (*spec).end.cast();
                if is_a(s, end.cast(), CLASS_NAVIGATION_POINT) {
                    let next: *mut ANavigationPoint = end.cast();
                    if !(*next).b_end_point()
                        && (!(*next).b_player_only() || (*pawn).b_is_player())
                        && supports(spec, i_radius, i_height, move_flags)
                    {
                        num_marked =
                            num_marked.wrapping_add(traverse_from(s, pawn, end, move_flags));
                    }
                }
                i += 1;
            }
        }
        num_marked
    }
}

/// `APawn::breadthPathFrom(start, bestPath, bSinglePath, moveFlags)`.
///
/// # Safety
/// `s` valid; `pawn` live; `start` a live navigation point; `best` writable.
pub unsafe fn breadth_path_from(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    start: *mut AActor,
    best: &mut *mut AActor,
    single: i32,
    move_flags: i32,
) -> i32 {
    unsafe {
        let level = level_of(pawn);
        let start_nav: *mut ANavigationPoint = start.cast();
        let mut currentnode: *mut ANavigationPoint = start_nav;
        let mut bin_tree: *mut ANavigationPoint = currentnode;
        let i_radius = app_round((*pawn).base.collision_radius);
        let i_height = app_round((*pawn).base.collision_height);
        let mut real_split: i32 = 1;
        let mut p: i32 = 0;
        let mut n: i32 = 0;
        let mut num_list: i32;

        while !currentnode.is_null() {
            if (*currentnode).b_end_point() {
                (*start_nav).previous_path = ptr::null_mut();
                *best = currentnode.cast();
                return 1;
            }
            if !(*currentnode).b_player_only()
                || (*pawn).b_is_player()
                || currentnode.cast::<AActor>() == start
            {
                let mut i: usize = 0;
                while i < 16 {
                    if (*currentnode).upstream_paths[i] == -1 {
                        i = 16;
                    } else {
                        let spec = spec_at(level, (*currentnode).upstream_paths[i]);
                        if supports(spec, i_radius, i_height, move_flags) {
                            let startnode: *mut ANavigationPoint = (*spec).start.cast();
                            let nextweight = (*spec).distance.wrapping_add((*startnode).cost);
                            let new_visit = nextweight
                                .wrapping_add((*currentnode).visited_weight)
                                .wrapping_add(
                                    (*startnode).b_end_point() as i32
                                        * (*startnode).best_path_weight,
                                );
                            if (*startnode).visited_weight > new_visit {
                                if !(*startnode).prev_ordered.is_null() {
                                    let prev = (*startnode).prev_ordered;
                                    (*prev).next_ordered = (*startnode).next_ordered;
                                    if !(*startnode).next_ordered.is_null() {
                                        (*(*startnode).next_ordered).prev_ordered =
                                            (*startnode).prev_ordered;
                                    }
                                    if bin_tree == startnode {
                                        if (*(*startnode).prev_ordered).visited_weight > new_visit {
                                            bin_tree = (*startnode).prev_ordered;
                                        }
                                    } else if (*startnode).visited_weight
                                        > (*bin_tree).visited_weight
                                        && new_visit < (*bin_tree).visited_weight
                                    {
                                        real_split -= 1;
                                    }
                                } else if new_visit > (*bin_tree).visited_weight {
                                    real_split += 1;
                                } else {
                                    real_split -= 1;
                                }
                                (*startnode).previous_path = currentnode.cast();
                                (*startnode).visited_weight = new_visit;
                                let mut nextnode =
                                    if (*bin_tree).visited_weight < (*startnode).visited_weight {
                                        bin_tree
                                    } else {
                                        currentnode
                                    };
                                num_list = 0;
                                while !(*nextnode).next_ordered.is_null()
                                    && (*(*nextnode).next_ordered).visited_weight
                                        < (*startnode).visited_weight
                                {
                                    num_list += 1;
                                    if num_list > 500 {
                                        (s.log_actor)(LOG_BREADTH_OVERFLOW, start.cast());
                                        return 0;
                                    }
                                    nextnode = (*nextnode).next_ordered;
                                }
                                if (*nextnode).next_ordered != startnode {
                                    if !(*nextnode).next_ordered.is_null() {
                                        (*(*nextnode).next_ordered).prev_ordered = startnode;
                                    }
                                    (*startnode).next_ordered = (*nextnode).next_ordered;
                                    (*nextnode).next_ordered = startnode;
                                    (*startnode).prev_ordered = nextnode;
                                }
                            }
                        }
                        i += 1;
                    }
                }
                real_split += 1;
                let mv = app_round(0.5_f32 * real_split as f32);
                while p < mv {
                    p += 1;
                    if !(*bin_tree).next_ordered.is_null() {
                        bin_tree = (*bin_tree).next_ordered;
                    }
                }
            }
            n += 1;
            if single != 0 && n > 4 {
                return 0;
            }
            if n > 1000 {
                (s.log_actor)(LOG_1000_SEARCHED, start.cast());
                return 0;
            }
            currentnode = (*currentnode).next_ordered;
        }
        0
    }
}

/// `APawn::breadthPathToInventory(start, bestPath, moveFlags, bestWeight, bPredictRespawns)`.
///
/// # Safety
/// `s` valid; `pawn` live; `start` a live navigation point; `best` writable.
pub unsafe fn breadth_path_to_inventory(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    start: *mut AActor,
    best: &mut *mut AActor,
    move_flags: i32,
    best_weight: f32,
    predict: i32,
) -> f32 {
    unsafe {
        let level = level_of(pawn);
        let i_radius = app_round((*pawn).base.collision_radius);
        let i_height = app_round((*pawn).base.collision_height);
        let mut best_inv = best_weight;
        let mut currentnode: *mut ANavigationPoint = start.cast();
        let mut bin_tree = currentnode;
        let mut best_dest: *mut ANavigationPoint = ptr::null_mut();
        let mut real_split: i32 = 1;
        let mut p: i32 = 0;
        let mut n: i32 = 0;

        while !currentnode.is_null() {
            if (*currentnode).b_end_point() {
                (*currentnode).start_path = currentnode.cast();
                (*currentnode).previous_path = ptr::null_mut();
            }
            if is_a(s, currentnode.cast(), CLASS_INVENTORY_SPOT) {
                let item: *mut AInventory =
                    (*currentnode.cast::<AInventorySpot>()).marked_item.cast();
                if !item.is_null()
                    && ((s.item_is_probing_touch)(item.cast()) != 0
                        || (predict != 0 && (*item).base.latent_float < 5.0))
                    && (*item).max_desireability / ((*currentnode).visited_weight as f32) > best_inv
                {
                    let this_item_weight = (s.item_bot_desirability)(item.cast(), pawn.cast())
                        / ((*currentnode).visited_weight as f32);
                    if this_item_weight > best_inv {
                        best_inv = this_item_weight;
                        *best = (*currentnode).start_path.cast();
                        best_dest = currentnode;
                    }
                }
            }

            let mut i: usize = 0;
            let mut nextweight: i32 = 0;
            while i < 16 {
                let mut endnode: *mut ANavigationPoint = ptr::null_mut();
                if (*currentnode).paths[i] == -1 {
                    i = 16;
                } else {
                    let spec = spec_at(level, (*currentnode).paths[i]);
                    if supports(spec, i_radius, i_height, move_flags) {
                        endnode = (*spec).end.cast();
                        nextweight = (*spec).distance;
                    }
                    i += 1;
                }
                if !endnode.is_null() {
                    nextweight = nextweight.wrapping_add((*endnode).cost);
                    let new_visit = nextweight.wrapping_add((*currentnode).visited_weight);
                    if (*endnode).visited_weight > new_visit {
                        (*endnode).start_path = (*currentnode).start_path;
                        (*endnode).previous_path = currentnode.cast();
                        if !(*endnode).prev_ordered.is_null() {
                            let prev = (*endnode).prev_ordered;
                            (*prev).next_ordered = (*endnode).next_ordered;
                            if !(*endnode).next_ordered.is_null() {
                                (*(*endnode).next_ordered).prev_ordered = (*endnode).prev_ordered;
                            }
                            if bin_tree == endnode {
                                if (*(*endnode).prev_ordered).visited_weight > new_visit {
                                    bin_tree = (*endnode).prev_ordered;
                                }
                            } else if (*endnode).visited_weight > (*bin_tree).visited_weight
                                && new_visit < (*bin_tree).visited_weight
                            {
                                real_split -= 1;
                            }
                        } else if new_visit > (*bin_tree).visited_weight {
                            real_split += 1;
                        } else {
                            real_split -= 1;
                        }
                        (*endnode).visited_weight = new_visit;
                        let mut nextnode = if (*bin_tree).visited_weight < (*endnode).visited_weight
                        {
                            bin_tree
                        } else {
                            currentnode
                        };
                        let mut num_list: i32 = 0;
                        while !(*nextnode).next_ordered.is_null()
                            && (*(*nextnode).next_ordered).visited_weight
                                < (*endnode).visited_weight
                        {
                            num_list += 1;
                            if num_list > 500 {
                                return 0.0;
                            }
                            nextnode = (*nextnode).next_ordered;
                        }
                        if (*nextnode).next_ordered != endnode {
                            if !(*nextnode).next_ordered.is_null() {
                                (*(*nextnode).next_ordered).prev_ordered = endnode;
                            }
                            (*endnode).next_ordered = (*nextnode).next_ordered;
                            (*nextnode).next_ordered = endnode;
                            (*endnode).prev_ordered = nextnode;
                        }
                    }
                }
            }
            real_split += 1;
            let mv = app_round(0.5_f32 * real_split as f32);
            while p < mv {
                p += 1;
                if !(*bin_tree).next_ordered.is_null() {
                    bin_tree = (*bin_tree).next_ordered;
                }
            }
            n += 1;
            if n > 250 {
                if best_inv > 0.0 {
                    reverse_route_for(best_dest);
                    return best_inv;
                }
                n = 200;
            }
            currentnode = (*currentnode).next_ordered;
        }
        reverse_route_for(best_dest);
        best_inv
    }
}

/// `APawn::setRouteCache(bestPath)`.
///
/// # Safety
/// `pawn` live; `best` a live navigation point or null.
pub unsafe fn set_route_cache(pawn: *mut APawn, best: *mut c_void) {
    unsafe {
        let mut b: *mut ANavigationPoint = best.cast();
        for i in 0..16 {
            (*pawn).route_cache[i] = b.cast();
            if !b.is_null() {
                b = (*b).previous_path.cast();
            }
        }
    }
}

/// `APawn::reverseRouteFor(bestDest)`.
///
/// # Safety
/// `best` must be null or a live navigation point with a valid `previousPath` chain.
pub unsafe fn reverse_route_for(best: *mut ANavigationPoint) {
    unsafe {
        if best.is_null() {
            return;
        }
        let mut route = best;
        let mut next: *mut ANavigationPoint = ptr::null_mut();
        while !(*route).previous_path.is_null() {
            let last: *mut ANavigationPoint = (*route).previous_path.cast();
            (*route).previous_path = next.cast();
            next = route;
            route = last;
        }
        (*route).previous_path = next.cast();
    }
}

/// `APawn::findPath(next, startPoint, DestName)`; `DestName` as its FName index.
/// The C++ return value `status` stays `false` even when a path is found
/// (preserved); `next` is set when a path is found.
///
/// # Safety
/// `s` valid; `pawn` live; `start` a live actor; `next` writable.
pub unsafe fn find_path(
    s: &Hp2RouteServices,
    pawn: *mut APawn,
    next: &mut *mut ANavigationPoint,
    start: *mut AActor,
    dest_name: i32,
) -> bool {
    unsafe {
        *next = ptr::null_mut();
        if start.is_null() {
            return false;
        }
        let level = level_of(pawn);
        let mut test_list: [*mut AActor; SEARCH_LIST_LEN] = [ptr::null_mut(); SEARCH_LIST_LEN];
        let mut visited: [*mut AActor; SEARCH_LIST_LEN] = [ptr::null_mut(); SEARCH_LIST_LEN];
        let mut tested_path: [i32; SEARCH_LIST_LEN] = [-1; SEARCH_LIST_LEN];
        let mut visit_count: usize = 0;
        let mut curr: i32 = 0;
        let path_class = (s.actor_class)(start.cast());
        test_list[0] = start;
        visited[0] = start;
        visit_count += 1;
        let mut status = false;
        let mut upstream = false;
        let mut break_out = true;

        while break_out {
            if curr < 0 {
                status = false;
                break;
            }
            loop {
                let c = curr as usize;
                tested_path[c] += 1;
                let nav: *mut ANavigationPoint = test_list[c].cast();
                let idx = tested_path[c];
                let cur = if (0..16).contains(&idx) {
                    (*nav).paths[idx as usize]
                } else {
                    -1
                };
                if cur < 0 {
                    if upstream {
                        tested_path[c] = -1;
                        test_list[c] = ptr::null_mut();
                        curr -= 1;
                        if curr >= 0 {
                            tested_path[curr as usize] = -1;
                        }
                        upstream = false;
                        break;
                    } else {
                        upstream = true;
                        tested_path[c] = -1;
                        break;
                    }
                }
                let spec = spec_at(level, cur);
                let spec_start: *mut AActor = (*spec).start.cast();
                let spec_end: *mut AActor = (*spec).end.cast();
                (s.log_actor)(LOG_START_NAME, spec_start.cast());
                (s.log_actor)(LOG_END_NAME, spec_end.cast());
                let in_list = |list: &[*mut AActor], t: *mut AActor| {
                    list.iter().take(visit_count).any(|p| *p == t)
                };
                let p_act = if !in_list(&visited, spec_start) {
                    spec_start
                } else if !in_list(&visited, spec_end) {
                    spec_end
                } else {
                    ptr::null_mut()
                };
                if !p_act.is_null() {
                    if visit_count < SEARCH_LIST_LEN {
                        visited[visit_count] = p_act;
                        visit_count += 1;
                    }
                    if (s.actor_name_index)(p_act.cast()) == dest_name {
                        break_out = false;
                        *next = test_list[1].cast();
                        if next.is_null() {
                            *next = p_act.cast();
                        }
                        break;
                    } else {
                        let this_class = (s.actor_class)(p_act.cast());
                        if (this_class == path_class || path_class.is_null())
                            && (curr as usize) + 1 < SEARCH_LIST_LEN
                        {
                            curr += 1;
                            test_list[curr as usize] = p_act;
                        }
                    }
                }
            }
        }
        // C++ `return status;` (unchanged by the path-found branch).
        status
    }
}
