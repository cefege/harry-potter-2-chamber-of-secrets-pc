//! Replication block exports (`crates/hp2-engine/src/actor_rep.rs`). The
//! `GetOptimizedRepList`, `UpdateFrequency` and script-replication methods in
//! `Engine/Src/UnActor.cpp` forward here. Engine objects cross as `void*` and are
//! cast to the generated mirrors; engine calls go through the
//! `Hp2ActorRepServices` table passed on every call.

use std::ffi::c_void;
use std::sync::{LazyLock, Mutex, PoisonError};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, AAmmo, AGameReplicationInfo, AInventory, ALevelInfo, AMover, APawn, APickup,
    APlayerPawn, APlayerReplicationInfo, AWeapon, AZoneInfo,
};
use hp2_engine::actor_rep::{self, Hp2ActorRepServices, Rep, RepCache};
use hp2_ffi_support::guard;

/// The `RepIndex` cache shared by every replication call in the process, as the
/// C++ block-scope statics are shared by every call of their function.
static REP_CACHE: LazyLock<Mutex<RepCache>> = LazyLock::new(|| Mutex::new(RepCache::default()));

/// Borrows the services table the C++ side passes on every call.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices`.
unsafe fn svc_of<'a>(svc: *const Hp2ActorRepServices) -> &'a Hp2ActorRepServices {
    unsafe { &*svc }
}

/// Runs one replication body against the process-global cache and returns the
/// advanced output cursor.
///
/// # Safety
/// `svc` must point to a live services table, `map` must be the live `UPackageMap`
/// the C++ caller passed, and `ptr` must have room for every index `body` pushes.
unsafe fn run_rep(
    svc: *const Hp2ActorRepServices,
    map: *mut c_void,
    ptr: *mut i32,
    body: impl FnOnce(&mut Rep<'_>),
) -> *mut i32 {
    // A poisoned lock only means an earlier call panicked; the cache is still valid.
    let mut cache = REP_CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    // SAFETY: forwarded from this function's contract.
    let mut rep = unsafe { Rep::new(svc_of(svc), &mut cache, map, ptr) };
    body(&mut rep);
    rep.finish()
}

/// `AActor::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `AActor` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_actor_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_actor_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::actor_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<AActor>(),
                    &*recent.cast::<AActor>(),
                );
            })
        },
    )
}

/// `APawn::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `APawn` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_pawn_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
    num_reps: i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_pawn_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::pawn_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<APawn>(),
                    &*recent.cast::<APawn>(),
                    num_reps,
                );
            })
        },
    )
}

/// `APlayerPawn::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `APlayerPawn` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_player_pawn_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
    num_reps: i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_player_pawn_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::player_pawn_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<APlayerPawn>(),
                    &*recent.cast::<APlayerPawn>(),
                    num_reps,
                );
            })
        },
    )
}

/// `AMover::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `AMover` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_mover_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_mover_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::mover_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<AMover>(),
                    &*recent.cast::<AMover>(),
                );
            })
        },
    )
}

/// `AZoneInfo::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `AZoneInfo` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_zone_info_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_zone_info_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::zone_info_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<AZoneInfo>(),
                    &*recent.cast::<AZoneInfo>(),
                );
            })
        },
    )
}

/// `ALevelInfo::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `ALevelInfo` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_level_info_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_level_info_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::level_info_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<ALevelInfo>(),
                    &*recent.cast::<ALevelInfo>(),
                );
            })
        },
    )
}

/// `APlayerReplicationInfo::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `APlayerReplicationInfo` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_player_replication_info_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_player_replication_info_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::player_replication_info_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<APlayerReplicationInfo>(),
                    &*recent.cast::<APlayerReplicationInfo>(),
                );
            })
        },
    )
}

/// `AGameReplicationInfo::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `AGameReplicationInfo` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_game_replication_info_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_game_replication_info_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::game_replication_info_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<AGameReplicationInfo>(),
                    &*recent.cast::<AGameReplicationInfo>(),
                );
            })
        },
    )
}

/// `AInventory::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `AInventory` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_inventory_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
    num_reps: i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_inventory_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::inventory_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<AInventory>(),
                    &*recent.cast::<AInventory>(),
                    num_reps,
                );
            })
        },
    )
}

/// `AWeapon::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `AWeapon` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_weapon_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
    num_reps: i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_weapon_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::weapon_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<AWeapon>(),
                    &*recent.cast::<AWeapon>(),
                    num_reps,
                );
            })
        },
    )
}

/// `AAmmo::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `AAmmo` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_ammo_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
    num_reps: i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_ammo_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::ammo_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<AAmmo>(),
                    &*recent.cast::<AAmmo>(),
                    num_reps,
                );
            })
        },
    )
}

/// `APickup::GetOptimizedRepList`. Returns the advanced `Ptr`.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` and `recent` must point to live `APickup` objects for the call duration.
/// `map` must be the live `UPackageMap` the C++ caller passed, and `ptr` must have room for every index the body pushes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_pickup_get_optimized_rep_list(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    recent: *const c_void,
    map: *mut c_void,
    ptr: *mut i32,
    num_reps: i32,
) -> *mut i32 {
    guard(
        "hp2_rs_engine_actor_rep_pickup_get_optimized_rep_list",
        ptr,
        move || unsafe {
            run_rep(svc, map, ptr, |rep| {
                actor_rep::pickup_get_optimized_rep_list(
                    rep,
                    &*actor.cast::<APickup>(),
                    &*recent.cast::<APickup>(),
                    num_reps,
                );
            })
        },
    )
}

/// `AActor::UpdateFrequency(Viewer, ViewDir, ViewPos)`. `Viewer` is unused by the
/// C++ body and is not passed.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` must point to a live `AActor` object, and `view_dir` and `view_pos` must point to live `FVector`s, for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_actor_update_frequency(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    view_dir: *const c_void,
    view_pos: *const c_void,
) -> f32 {
    guard(
        "hp2_rs_engine_actor_rep_actor_update_frequency",
        0.0,
        move || unsafe {
            actor_rep::actor_update_frequency(
                svc_of(svc),
                &*actor.cast::<AActor>(),
                &*view_dir.cast::<FVector>(),
                &*view_pos.cast::<FVector>(),
            )
        },
    )
}

/// `AInventory::UpdateFrequency(Viewer, ViewDir, ViewPos)`. `ViewDir`/`ViewPos` are
/// unused by the C++ body and are not passed.
///
/// # Safety
/// `svc` must point to a live `Hp2ActorRepServices` table.
/// `actor` must point to a live `AInventory` object for the call duration.
/// `viewer` is an opaque pointer compared by address only; NULL is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_inventory_update_frequency(
    svc: *const Hp2ActorRepServices,
    actor: *const c_void,
    viewer: *const c_void,
) -> f32 {
    guard(
        "hp2_rs_engine_actor_rep_inventory_update_frequency",
        0.0,
        move || unsafe {
            actor_rep::inventory_update_frequency(svc_of(svc), &*actor.cast::<AInventory>(), viewer)
        },
    )
}

/// `AInventory::ShouldDoScriptReplication`. Returns `UBOOL`.
///
/// # Safety
/// `actor` must point to a live `AInventory` object for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_inventory_should_do_script_replication(
    actor: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_actor_rep_inventory_should_do_script_replication",
        0,
        move || unsafe {
            i32::from(actor_rep::inventory_should_do_script_replication(
                &*actor.cast::<AInventory>(),
            ))
        },
    )
}

/// `AInventory::NoVariablesToReplicate(AActor* OldVer)`. Returns `UBOOL`.
///
/// # Safety
/// `actor` must point to a live `AInventory` object, and `old_ver` to a live `AActor` object, for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_actor_rep_inventory_no_variables_to_replicate(
    actor: *const c_void,
    old_ver: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_actor_rep_inventory_no_variables_to_replicate",
        0,
        move || unsafe {
            i32::from(actor_rep::inventory_no_variables_to_replicate(
                &*actor.cast::<AInventory>(),
                &*old_ver.cast::<AActor>(),
            ))
        },
    )
}
