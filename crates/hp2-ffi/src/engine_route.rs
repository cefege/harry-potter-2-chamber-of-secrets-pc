//! Route-finding exports (`crates/hp2-engine/src/route.rs`). The C++ `APawn`
//! and `FSortedPathList` methods in `Engine/Src/UnRoute.cpp` forward here.
//! Engine objects cross as opaque `void*`; the engine-side calls go through
//! the per-slice `Hp2RouteServices` table passed on every call.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_engine::route::{self, Hp2RouteServices, Hp2SortedPathList};
use hp2_ffi_support::guard;

use hp2_abi::engine::generated::{AActor, ANavigationPoint, APawn};

/// Borrows the route services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2RouteServices`.
unsafe fn svc_of<'a>(p: *const Hp2RouteServices) -> Option<&'a Hp2RouteServices> {
    unsafe { p.as_ref() }
}

/// Borrows a `Hp2SortedPathList`; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2SortedPathList`.
unsafe fn list_of<'a>(p: *mut Hp2SortedPathList) -> Option<&'a mut Hp2SortedPathList> {
    unsafe { p.as_mut() }
}

/// Reads an `FVector` by value; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `FVector`.
unsafe fn read_vec(p: *const c_void) -> Option<FVector> {
    unsafe { p.cast::<FVector>().as_ref().copied() }
}

/// Writes an `i32` through `p`; a null `p` is a no-op.
///
/// # Safety
/// `p`, when non-null, must be valid for writes of an `i32`.
unsafe fn write_i32(p: *mut i32, v: i32) {
    unsafe {
        if let Some(out) = p.as_mut() {
            *out = v;
        }
    }
}

/// `APawn::clearPaths()`.
///
/// # Safety
/// `svc` and `pawn` must be valid engine pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_clear_paths(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
) {
    guard("hp2_rs_engine_route_clear_paths", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), pawn.is_null()) {
            route::clear_paths(s, pawn.cast::<APawn>());
        }
    });
}

/// `APawn::canMoveTo(anchor, dest)`.
///
/// # Safety
/// `svc` and all object pointers must be valid engine pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_can_move_to(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    anchor: *mut c_void,
    dest: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_route_can_move_to", 0, move || unsafe {
        match (
            svc_of(svc),
            pawn.is_null() || anchor.is_null() || dest.is_null(),
        ) {
            (Some(s), false) => route::can_move_to(
                s,
                pawn.cast::<APawn>(),
                anchor.cast::<AActor>(),
                dest.cast::<AActor>(),
            ),
            _ => 0,
        }
    })
}

/// `APawn::findPathToward(goal, bSinglePath, bestPath, bClearPaths)`.
///
/// # Safety
/// `svc` and object pointers must be valid; `best_path` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_find_path_toward(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    goal: *mut c_void,
    single: i32,
    best_path: *mut *mut c_void,
    clear: i32,
) -> i32 {
    guard("hp2_rs_engine_route_find_path_toward", 0, move || unsafe {
        let Some(s) = svc_of(svc) else { return 0 };
        if pawn.is_null() || best_path.is_null() {
            return 0;
        }
        let mut best = std::ptr::null_mut::<AActor>();
        let r = route::find_path_toward(
            s,
            pawn.cast::<APawn>(),
            goal.cast::<AActor>(),
            single,
            &mut best,
            clear,
        );
        *best_path = best.cast();
        r
    })
}

/// `APawn::findPathTowardBestInventory(bestPath, bClearPaths, minWeight, bPredictRespawns)`.
///
/// # Safety
/// `svc` and object pointers must be valid; `best_path` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_find_path_toward_best_inventory(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    best_path: *mut *mut c_void,
    clear: i32,
    min_weight: f32,
    predict: i32,
) -> f32 {
    guard(
        "hp2_rs_engine_route_find_path_toward_best_inventory",
        0.0,
        move || unsafe {
            let Some(s) = svc_of(svc) else { return 0.0 };
            if pawn.is_null() || best_path.is_null() {
                return 0.0;
            }
            let mut best = std::ptr::null_mut::<AActor>();
            let r = route::find_path_toward_best_inventory(
                s,
                pawn.cast::<APawn>(),
                &mut best,
                clear,
                min_weight,
                predict,
            );
            *best_path = best.cast();
            r
        },
    )
}

/// `APawn::findPathTo(Dest, bSinglePath, bestPath, bClearPaths)`.
///
/// # Safety
/// `svc` and object pointers must be valid; `dest` must point to an `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_find_path_to(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    dest: *const c_void,
    single: i32,
    best_path: *mut *mut c_void,
    clear: i32,
) -> i32 {
    guard("hp2_rs_engine_route_find_path_to", 0, move || unsafe {
        let (Some(s), Some(d)) = (svc_of(svc), read_vec(dest)) else {
            return 0;
        };
        if pawn.is_null() || best_path.is_null() {
            return 0;
        }
        let mut best = std::ptr::null_mut::<AActor>();
        let r = route::find_path_to(s, pawn.cast::<APawn>(), d, single, &mut best, clear);
        *best_path = best.cast();
        r
    })
}

/// `APawn::findRandomDest(bestDest)`.
///
/// # Safety
/// `svc` and `pawn` must be valid; `best` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_find_random_dest(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    best: *mut *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_route_find_random_dest", 0, move || unsafe {
        let Some(s) = svc_of(svc) else { return 0 };
        if pawn.is_null() || best.is_null() {
            return 0;
        }
        let mut b = std::ptr::null_mut::<AActor>();
        let r = route::find_random_dest(s, pawn.cast::<APawn>(), &mut b);
        *best = b.cast();
        r
    })
}

/// `APawn::findPath(next, startPoint, DestName)`; `dest_name` is the FName index.
///
/// # Safety
/// `svc` and object pointers must be valid; `next` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_find_path(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    next: *mut *mut c_void,
    start: *mut c_void,
    dest_name: i32,
) -> i32 {
    guard("hp2_rs_engine_route_find_path", 0, move || unsafe {
        let Some(s) = svc_of(svc) else { return 0 };
        if pawn.is_null() || next.is_null() {
            return 0;
        }
        let mut n = std::ptr::null_mut::<ANavigationPoint>();
        let r = route::find_path(
            s,
            pawn.cast::<APawn>(),
            &mut n,
            start.cast::<AActor>(),
            dest_name,
        );
        *next = n.cast();
        i32::from(r)
    })
}

/// `APawn::traverseFrom(start, moveFlags)`.
///
/// # Safety
/// `svc`, `pawn` and `start` must be valid engine pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_traverse_from(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    start: *mut c_void,
    move_flags: i32,
) -> i32 {
    guard("hp2_rs_engine_route_traverse_from", 0, move || unsafe {
        match (svc_of(svc), pawn.is_null() || start.is_null()) {
            (Some(s), false) => {
                route::traverse_from(s, pawn.cast::<APawn>(), start.cast::<AActor>(), move_flags)
            }
            _ => 0,
        }
    })
}

/// `APawn::breadthPathFrom(start, bestPath, bSinglePath, moveFlags)`.
///
/// # Safety
/// `svc` and object pointers must be valid; `best` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_breadth_path_from(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    start: *mut c_void,
    best: *mut *mut c_void,
    single: i32,
    move_flags: i32,
) -> i32 {
    guard("hp2_rs_engine_route_breadth_path_from", 0, move || unsafe {
        let Some(s) = svc_of(svc) else { return 0 };
        if pawn.is_null() || start.is_null() || best.is_null() {
            return 0;
        }
        let mut b = std::ptr::null_mut::<AActor>();
        let r = route::breadth_path_from(
            s,
            pawn.cast::<APawn>(),
            start.cast::<AActor>(),
            &mut b,
            single,
            move_flags,
        );
        *best = b.cast();
        r
    })
}

/// `APawn::breadthPathToInventory(start, bestPath, moveFlags, bestWeight, bPredictRespawns)`.
///
/// # Safety
/// `svc` and object pointers must be valid; `best` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_breadth_path_to_inventory(
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    start: *mut c_void,
    best: *mut *mut c_void,
    move_flags: i32,
    best_weight: f32,
    predict: i32,
) -> f32 {
    guard(
        "hp2_rs_engine_route_breadth_path_to_inventory",
        0.0,
        move || unsafe {
            let Some(s) = svc_of(svc) else { return 0.0 };
            if pawn.is_null() || start.is_null() || best.is_null() {
                return 0.0;
            }
            let mut b = std::ptr::null_mut::<AActor>();
            let r = route::breadth_path_to_inventory(
                s,
                pawn.cast::<APawn>(),
                start.cast::<AActor>(),
                &mut b,
                move_flags,
                best_weight,
                predict,
            );
            *best = b.cast();
            r
        },
    )
}

/// `APawn::setRouteCache(bestPath)`.
///
/// # Safety
/// `pawn` must be a valid `APawn`; `best` null or a live navigation point.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_set_route_cache(pawn: *mut c_void, best: *mut c_void) {
    guard("hp2_rs_engine_route_set_route_cache", (), move || unsafe {
        if !pawn.is_null() {
            route::set_route_cache(pawn.cast::<APawn>(), best);
        }
    });
}

/// `APawn::reverseRouteFor(bestDest)`.
///
/// # Safety
/// `best` null or a live navigation point with a valid `previousPath` chain.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_reverse_route_for(best: *mut c_void) {
    guard(
        "hp2_rs_engine_route_reverse_route_for",
        (),
        move || unsafe {
            route::reverse_route_for(best.cast::<ANavigationPoint>());
        },
    );
}

/// `FSortedPathList::addPath(node, dist)`.
///
/// # Safety
/// `list` must point to a live `Hp2SortedPathList`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_sorted_add_path(
    list: *mut Hp2SortedPathList,
    node: *mut c_void,
    dist: i32,
) {
    guard("hp2_rs_engine_route_sorted_add_path", (), move || unsafe {
        if let Some(l) = list_of(list) {
            l.add_path(node.cast::<AActor>(), dist);
        }
    });
}

/// `FSortedPathList::removePath(p)`.
///
/// # Safety
/// `list` must point to a live `Hp2SortedPathList`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_sorted_remove_path(
    list: *mut Hp2SortedPathList,
    p: i32,
) {
    guard(
        "hp2_rs_engine_route_sorted_remove_path",
        (),
        move || unsafe {
            if let Some(l) = list_of(list) {
                l.remove_path(p);
            }
        },
    );
}

/// `FSortedPathList::FindVisiblePaths(Searcher, Dest, destPoints, bClearPaths, startanchor, endanchor)`.
///
/// # Safety
/// All pointers must be valid engine objects or live lists; `dest` an `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_sorted_find_visible_paths(
    list: *mut Hp2SortedPathList,
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    dest: *const c_void,
    dest_points: *mut Hp2SortedPathList,
    clear: i32,
    startanchor: *mut i32,
    endanchor: *mut i32,
) {
    guard(
        "hp2_rs_engine_route_sorted_find_visible_paths",
        (),
        move || unsafe {
            let (Some(l), Some(s), Some(d), Some(dp)) = (
                list_of(list),
                svc_of(svc),
                read_vec(dest),
                list_of(dest_points),
            ) else {
                return;
            };
            if pawn.is_null() {
                return;
            }
            let mut sa = startanchor.as_ref().copied().unwrap_or(0);
            let mut ea = endanchor.as_ref().copied().unwrap_or(0);
            l.find_visible_paths(s, pawn.cast::<APawn>(), d, dp, clear, &mut sa, &mut ea);
            write_i32(startanchor, sa);
            write_i32(endanchor, ea);
        },
    );
}

/// `FSortedPathList::findEndPoint(Searcher, startanchor)`.
///
/// # Safety
/// All pointers must be valid engine objects or live lists.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_sorted_find_end_point(
    list: *mut Hp2SortedPathList,
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    startanchor: *mut i32,
) -> i32 {
    guard(
        "hp2_rs_engine_route_sorted_find_end_point",
        0,
        move || unsafe {
            let (Some(l), Some(s)) = (list_of(list), svc_of(svc)) else {
                return 0;
            };
            if pawn.is_null() {
                return 0;
            }
            let mut sa = startanchor.as_ref().copied().unwrap_or(0);
            let r = l.find_end_point(s, pawn.cast::<APawn>(), &mut sa);
            write_i32(startanchor, sa);
            r
        },
    )
}

/// `FSortedPathList::checkAnchorPath(Searcher, Dest)`.
///
/// # Safety
/// All pointers must be valid engine objects or live lists; `dest` an `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_sorted_check_anchor_path(
    list: *mut Hp2SortedPathList,
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    dest: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_route_sorted_check_anchor_path",
        0,
        move || unsafe {
            let (Some(l), Some(s), Some(d)) = (list_of(list), svc_of(svc), read_vec(dest)) else {
                return 0;
            };
            if pawn.is_null() {
                return 0;
            }
            l.check_anchor_path(s, pawn.cast::<APawn>(), d)
        },
    )
}

/// `FSortedPathList::expandAnchor(Searcher)`.
///
/// # Safety
/// All pointers must be valid engine objects or live lists.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_sorted_expand_anchor(
    list: *mut Hp2SortedPathList,
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
) {
    guard(
        "hp2_rs_engine_route_sorted_expand_anchor",
        (),
        move || unsafe {
            let (Some(l), Some(s)) = (list_of(list), svc_of(svc)) else {
                return;
            };
            if !pawn.is_null() {
                l.expand_anchor(s, pawn.cast::<APawn>());
            }
        },
    );
}

/// `FSortedPathList::findAltEndPoint(Searcher, bestPath)`.
///
/// # Safety
/// All pointers must be valid engine objects or live lists; `best_path` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_route_sorted_find_alt_end_point(
    list: *mut Hp2SortedPathList,
    svc: *const Hp2RouteServices,
    pawn: *mut c_void,
    best_path: *mut *mut c_void,
) {
    guard(
        "hp2_rs_engine_route_sorted_find_alt_end_point",
        (),
        move || unsafe {
            let (Some(l), Some(s)) = (list_of(list), svc_of(svc)) else {
                return;
            };
            if pawn.is_null() || best_path.is_null() {
                return;
            }
            let mut b = (*best_path).cast::<AActor>();
            l.find_alt_end_point(s, pawn.cast::<APawn>(), &mut b);
            *best_path = b.cast();
        },
    );
}
