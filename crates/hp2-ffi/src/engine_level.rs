//! LevelCore exports (`crates/hp2-engine/src/level.rs`). The C++ shells in
//! `Engine/Src/UnLevel.cpp` forward the `ULevelBase`/`ULevel` Core bodies here.
//! Engine calls go through the `Hp2LevelCoreServices` table passed on every call.
//! The table is the static `GHp2LevelCoreServices` and is never null, so it is
//! dereferenced directly. Engine objects, FVector and FCheckResult cross as `void*`.

use std::ffi::c_void;

use hp2_engine::level::{self, Hp2LevelCoreServices};
use hp2_engine::level_net::{self, Hp2LevelNetServices};
use hp2_engine::level_persist::{self, Hp2LevelPersistServices};
use hp2_ffi_support::guard;

/// `ULevelBase::Serialize(FArchive&)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_base_serialize(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    ar: *mut c_void,
) {
    guard("hp2_rs_engine_level_base_serialize", (), move || unsafe {
        level::level_base_serialize(&*svc, obj, ar)
    })
}

/// `ULevelBase::Destroy()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_base_destroy(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
) {
    guard("hp2_rs_engine_level_base_destroy", (), move || unsafe {
        level::level_base_destroy(&*svc, obj)
    })
}

/// `ULevel::ULevel` body after `ULevelBase(InEngine)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_init(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    root_outside: i32,
) -> i32 {
    guard("hp2_rs_engine_level_init", 1, move || unsafe {
        match level::level_init(&*svc, obj, root_outside) {
            Ok(()) => 0,
            Err(()) => 1,
        }
    })
}

/// `ULevel::ScreenActorsByGameState()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_screen_actors_by_game_state(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
) {
    guard(
        "hp2_rs_engine_level_screen_actors_by_game_state",
        (),
        move || unsafe { level::level_screen_actors_by_game_state(&*svc, obj) },
    )
}

/// `ULevel::ShrinkLevel()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_shrink(svc: *const Hp2LevelCoreServices, obj: *mut c_void) {
    guard("hp2_rs_engine_level_shrink", (), move || unsafe {
        level::level_shrink(&*svc, obj)
    })
}

/// `ULevel::DetailChange(UBOOL)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_detail_change(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    new_detail: i32,
) {
    guard("hp2_rs_engine_level_detail_change", (), move || unsafe {
        level::level_detail_change(&*svc, obj, new_detail)
    })
}

/// `ULevel::Modify(UBOOL)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_modify(svc: *const Hp2LevelCoreServices, obj: *mut c_void) {
    guard("hp2_rs_engine_level_modify", (), move || unsafe {
        level::level_modify(&*svc, obj)
    })
}

/// `ULevel::PostLoad()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_post_load(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
) {
    guard("hp2_rs_engine_level_post_load", (), move || unsafe {
        level::level_post_load(&*svc, obj)
    })
}

/// `ULevel::SetActorCollision(UBOOL)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_set_actor_collision(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    collision: i32,
) {
    guard(
        "hp2_rs_engine_level_set_actor_collision",
        (),
        move || unsafe { level::level_set_actor_collision(&*svc, obj, collision) },
    )
}

/// `ULevel::Serialize(FArchive&)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_serialize(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    ar: *mut c_void,
) {
    guard("hp2_rs_engine_level_serialize", (), move || unsafe {
        level::level_serialize(&*svc, obj, ar)
    })
}

/// `ULevel::Destroy()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_destroy(svc: *const Hp2LevelCoreServices, obj: *mut c_void) {
    guard("hp2_rs_engine_level_destroy", (), move || unsafe {
        level::level_destroy(&*svc, obj)
    })
}

/// `ULevel::RememberActors()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_remember_actors(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
) {
    guard("hp2_rs_engine_level_remember_actors", (), move || unsafe {
        level::level_remember_actors(&*svc, obj)
    })
}

/// `ULevel::ReconcileActors()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_reconcile_actors(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_level_reconcile_actors", 1, move || unsafe {
        match level::level_reconcile_actors(&*svc, obj) {
            Ok(()) => 0,
            Err(()) => 1,
        }
    })
}

/// `ULevel::InitStats()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_init_stats(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
) {
    guard("hp2_rs_engine_level_init_stats", (), move || unsafe {
        level::level_init_stats(&*svc, obj)
    })
}

/// `ULevel::GetStats(TCHAR*)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_get_stats(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    result: *mut u32,
) {
    guard("hp2_rs_engine_level_get_stats", (), move || unsafe {
        level::level_get_stats(&*svc, obj, result)
    })
}

/// `ULevel::UpdateTime(ALevelInfo*)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_update_time(
    svc: *const Hp2LevelCoreServices,
    info: *mut c_void,
) {
    guard("hp2_rs_engine_level_update_time", (), move || unsafe {
        level::level_update_time(&*svc, info)
    })
}

/// `ULevel::IsAudibleAt(FVector, FVector, AActor*, ESoundOcclusion)`. `hit` is the C++ `Hit` local.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_is_audible_at(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    sound_location: *const c_void,
    listener_location: *const c_void,
    sound_actor: *mut c_void,
    sound_occlusion: i32,
    hit: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_level_is_audible_at", 0, move || unsafe {
        level::level_is_audible_at(
            &*svc,
            obj,
            sound_location.cast(),
            listener_location.cast(),
            sound_actor,
            sound_occlusion,
            hit.cast(),
        )
    })
}

/// `ULevel::CalculateRadiusMultiplier(INT, INT)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_calculate_radius_multiplier(
    obj: *mut c_void,
    zone1: i32,
    zone2: i32,
) -> f32 {
    guard(
        "hp2_rs_engine_level_calculate_radius_multiplier",
        0.0,
        move || unsafe { level::level_calculate_radius_multiplier(obj, zone1, zone2) },
    )
}

/// `ULevel::TraceVisible(...)`. `hit` and `first_hit` are C++ `FCheckResult` locals.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn hp2_rs_engine_level_trace_visible(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    trace_direction: *const c_void,
    hit: *mut c_void,
    source: *mut c_void,
    start: *const c_void,
    trace_flags: u32,
    distance: i32,
    first_hit: *mut c_void,
) {
    guard("hp2_rs_engine_level_trace_visible", (), move || unsafe {
        level::level_trace_visible(
            &*svc,
            obj,
            trace_direction.cast(),
            hit.cast(),
            source,
            start.cast(),
            trace_flags,
            distance,
            first_hit.cast(),
        )
    })
}

/// `ULevel::TraceVisibleObjects(...)`. `hit` and `first_hit` are C++ `FCheckResult` locals.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn hp2_rs_engine_level_trace_visible_objects(
    svc: *const Hp2LevelCoreServices,
    obj: *mut c_void,
    parent_class: *mut c_void,
    trace_direction: *const c_void,
    hit: *mut c_void,
    source: *mut c_void,
    start: *const c_void,
    trace_flags: u32,
    distance: i32,
    first_hit: *mut c_void,
) {
    guard(
        "hp2_rs_engine_level_trace_visible_objects",
        (),
        move || unsafe {
            level::level_trace_visible_objects(
                &*svc,
                obj,
                parent_class,
                trace_direction.cast(),
                hit.cast(),
                source,
                start.cast(),
                trace_flags,
                distance,
                first_hit.cast(),
            )
        },
    )
}

/// Borrows the services table. A null table is a bug in the caller and panics.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2LevelNetServices`.
unsafe fn svc<'a>(p: *const Hp2LevelNetServices) -> &'a Hp2LevelNetServices {
    unsafe { p.as_ref() }.expect("hp2_rs level net: null services table")
}

/// `ULevel::Exec(Cmd, Ar)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_exec(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
    cmd: *const u32,
    ar: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_level_exec", 0, move || unsafe {
        level_net::exec(svc(svc_p), level, cmd, ar)
    })
}

/// `ULevel::Listen(Error)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_listen(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
    error: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_level_listen", 0, move || unsafe {
        level_net::listen(svc(svc_p), level, error)
    })
}

/// `ULevel::IsServer()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_is_server(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_level_is_server", 0, move || unsafe {
        level_net::is_server(svc(svc_p), level)
    })
}

/// `ULevel::NotifyAcceptingConnection()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_notify_accepting_connection(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
    result: *mut i32,
) -> i32 {
    guard(
        "hp2_rs_engine_level_notify_accepting_connection",
        1,
        move || unsafe {
            match level_net::notify_accepting_connection(svc(svc_p), level) {
                Ok(value) => {
                    *result = value;
                    0
                }
                Err(()) => 1,
            }
        },
    )
}

/// `ULevel::NotifyAcceptedConnection(Connection)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_notify_accepted_connection(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
    conn: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_level_notify_accepted_connection",
        1,
        move || unsafe {
            match level_net::notify_accepted_connection(svc(svc_p), level, conn) {
                Ok(()) => 0,
                Err(()) => 1,
            }
        },
    )
}

/// `ULevel::NotifyAcceptingChannel(Channel)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_notify_accepting_channel(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
    ch: *mut c_void,
    result: *mut i32,
) -> i32 {
    guard(
        "hp2_rs_engine_level_notify_accepting_channel",
        1,
        move || unsafe {
            match level_net::notify_accepting_channel(svc(svc_p), level, ch) {
                Ok(value) => {
                    *result = value;
                    0
                }
                Err(()) => 1,
            }
        },
    )
}
/// `ULevel::WelcomePlayer(Connection, Optional)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_welcome_player(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
    conn: *mut c_void,
    optional: *const u32,
) {
    guard("hp2_rs_engine_level_welcome_player", (), move || unsafe {
        level_net::welcome_player(svc(svc_p), level, conn, optional)
    })
}

/// `ULevel::NotifyReceivedText(Connection, Text)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_notify_received_text(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
    conn: *mut c_void,
    text: *const u32,
) {
    guard(
        "hp2_rs_engine_level_notify_received_text",
        (),
        move || unsafe { level_net::notify_received_text(svc(svc_p), level, conn, text) },
    )
}

/// `ULevel::NotifySendingFile(Connection, Guid)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_notify_sending_file(
    svc_p: *const Hp2LevelNetServices,
    level: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_level_notify_sending_file",
        0,
        move || unsafe { level_net::notify_sending_file(svc(svc_p), level) },
    )
}

/// `ULevel::NotifyReceivedFile(...)`: unconditional `appErrorf`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_notify_received_file(
    svc_p: *const Hp2LevelNetServices,
) -> i32 {
    guard(
        "hp2_rs_engine_level_notify_received_file",
        1,
        move || unsafe {
            match level_net::notify_received_file(svc(svc_p)) {
                Ok(()) => 0,
                Err(()) => 1,
            }
        },
    )
}

// ---- PERSIST exports (crates/hp2-engine/src/level_persist.rs) ----

/// `ULevel::SavePersistentActors(const FString&)`; returns the bError flag as i32.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_save_persistent_actors(
    svc: *const Hp2LevelPersistServices,
    level: *mut c_void,
    map_name: *const u32,
) -> i32 {
    guard(
        "hp2_rs_engine_level_save_persistent_actors",
        0,
        move || unsafe { level_persist::save_persistent_actors(&*svc, level, map_name) as i32 },
    )
}

/// `ULevel::LoadPersistentActors(const FString&)`; returns the bError flag as i32.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_level_load_persistent_actors(
    svc: *const Hp2LevelPersistServices,
    level: *mut c_void,
    map_name: *const u32,
) -> i32 {
    guard(
        "hp2_rs_engine_level_load_persistent_actors",
        0,
        move || unsafe { level_persist::load_persistent_actors(&*svc, level, map_name) as i32 },
    )
}
