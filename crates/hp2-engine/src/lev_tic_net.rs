//! Port of `ULevel::TickNetClient`, `ULevel::ServerTickClient`, and
//! `ULevel::TickNetServer` (`HarryPotter2/Unreal/Engine/Src/UnLevTic.cpp:667-961`).
//!
//! Native stack continuations retain guards and actual FMemMark lifetimes;
//! checked callbacks pop temporary GMem storage only at the original explicit pop.
//! `ActorCanSee` lives in [`crate::lev_tic::actor_can_see`]. Engine calls go through
//! [`Hp2LevTicServices`] in the original order, so the `eventPlayerCalcView` and
//! `UpdateFrequency` overrides fire. Plain reads use the generated `ULevel`,
//! `AActor`, `APawn`, and `APlayerPawn` mirrors.
//!
//! FMA: none. `/tmp/EngLevTic2/fma.md` lists no fused op in these three functions.
//! libm: none. `FRotator::Vector()` is `lev_tic::rotator_vector` (GMath sin/cos tables).
//! Sort: original value/pointer arrays are allocated separately in GMem.
//! Native sorting observes the legacy stale slot without manufacturing a Rust
//! initialized value. Only test fixtures seed that otherwise uninitialized slot.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, APlayerPawn, ULevel, UObject};

#[cfg(test)]
use crate::lev_tic::sort_priority_ptrs;
use crate::lev_tic::{
    ActorPriority, Hp2LevTicServices, actor_can_see_with, ftime_add_f32, ftime_from_f32,
    ftime_get_float, ftime_sub, make_priority_with, rotator_vector_with,
};
use crate::math::app_round;
use crate::prim::INDEX_NONE;

/// `AActor::UpdateFrequency`, with the ops-first dispatch
/// (`local://m1-dispatch-design.md` §3.2): if `actor`'s class has a Rust-
/// registered `AActor`-family ops record with an `update_frequency`
/// override (currently only Carcass -- `hp2_core::class_ops`), call it
/// directly: a same-process Rust call, no C++ virtual dispatch, no FFI hop.
/// Every other class still goes through the original C++ virtual call via
/// `ops.actor_update_frequency`, unchanged.
unsafe fn actor_update_frequency_with<O: LevelTickNetOps>(
    ops: &O,
    actor: *mut c_void,
    viewer: *mut c_void,
    view_dir: *mut f32,
    view_pos: *mut f32,
) -> Result<f32, O::Error> {
    unsafe {
        let class = (*actor.cast::<UObject>()).class;
        if let Some(f) = hp2_core::class_ops::actor_ops(class).and_then(|ops| ops.update_frequency)
        {
            return Ok(f(actor));
        }
        ops.actor_update_frequency(actor, viewer, view_dir, view_pos)
    }
}

/// `new(*mem, count) T`: the FMemStack placement new, which grows the stack with a new chunk
/// when the current one is full (UnMem.h:143). `count` is the oracle's `Actors.Num()` INT.
unsafe fn gmem_push_with<T, O: LevelTickNetOps>(
    ops: &O,
    mem: *mut c_void,
    count: usize,
) -> Result<*mut T, O::Error> {
    unsafe {
        Ok(ops
            .mem_push(mem, std::mem::size_of::<T>() as u64, count as i32)?
            .cast())
    }
}

unsafe fn append_priority(
    list: *mut ActorPriority,
    actors: *mut *mut ActorPriority,
    count: &mut usize,
    value: ActorPriority,
) {
    unsafe {
        let index = *count;
        list.add(index).write(value);
        *count += 1;
        actors.add(*count).write(list.add(index));
    }
}

/// `HP2_LEVTIC_CYCLES_NET_TICK` (`Hp2LevTicServices.h`).
const NET_TICK_CYCLES: i32 = 1;
/// `HP2_LEVTIC_TRAVEL_FAILED` (`Hp2LevTicServices.h`): the `"?failed"` url.
const TRAVEL_FAILED: i32 = 0;
/// `USOCK_Closed`, `USOCK_Pending`, `USOCK_Open` (`UnConn.h`).
const USOCK_CLOSED: i32 = 1;
const USOCK_PENDING: i32 = 2;
const USOCK_OPEN: i32 = 3;
/// `ROLE_None` (`EngineClasses.h`).
const ROLE_NONE: u8 = 0;
/// `NF_NotVisBlocking` (`UnModel.h`).
const NF_NOT_VIS_BLOCKING: i32 = 0x04;

/// `ULevel::TickNetClient` (`UnLevTic.cpp:667-691`). `DeltaSeconds` is unused in the original.
///
/// # Safety
/// `level` must be a live `ULevel`. Its `NetDriver->ServerConnection` must be live, because
/// the original dereferences it unchecked.
pub(crate) unsafe fn tick_net_client_with<O: LevelTickNetOps>(ops: &O, level: *mut c_void) -> Result<(), O::Error> {
    unsafe {
        let lv = level.cast::<ULevel>();
        ops.level_cycles_clock(level, NET_TICK_CYCLES)?;
        let server = ops.driver_server_connection((*lv).base.net_driver)?;
        let state = ops.conn_state(server)?;
        if state == USOCK_OPEN {
            ops.conn_replicate_player_channels(server)?;
        } else if state == USOCK_CLOSED {
            // `check(Engine->Client->Viewports.Num())` (UnLevTic.cpp:686) runs before the travel thunk.
            ops.check(
                ops.level_client_viewports_num(level)? != 0,
                c"Engine->Client->Viewports.Num()",
                686,
            )?;
            ops.level_engine_client_travel(level, TRAVEL_FAILED)?;
        }
        ops.level_cycles_unclock(level, NET_TICK_CYCLES)?;
        Ok(())
    }
}

/// `ULevel::ServerTickClient` (`UnLevTic.cpp:715-898`). Returns the number of channels
/// replicated. `DeltaSeconds` is unused in the original.
///
/// # Safety
/// `level` must be a live `ULevel`. `connection` must be a live `UNetConnection`; a null
/// connection fails the `check` below.
pub(crate) unsafe fn server_tick_client_with<O: LevelTickNetOps>(
    ops: &O,
    level: *mut c_void,
    connection: *mut c_void,
) -> Result<i32, O::Error> {
    unsafe {
        let lv = level.cast::<ULevel>();
        ops.check(!connection.is_null(), c"Connection", 718)?;
        let state = ops.conn_state(connection)?;
        ops.check(
            state == USOCK_PENDING || state == USOCK_OPEN || state == USOCK_CLOSED,
            c"Connection->State==USOCK_Pending || Connection->State==USOCK_Open || Connection->State==USOCK_Closed",
            719,
        )?;

        // Handle not ready channels.
        let mut updated: i32 = 0;
        let conn_actor = ops.conn_actor(connection)?;
        if !conn_actor.is_null()
            && ops.conn_is_net_ready(connection, 0)? != 0
            && ops.conn_state(connection)? == USOCK_OPEN
            && ftime_sub(
                ops.driver_time(ops.conn_driver(connection)?)?,
                ops.conn_last_receive_time(connection)?,
            ) < 1.5_f32
        {
            updated = ops.server_tick_client_scoped(level, connection)?;
        }
        Ok(updated)
    }
}

/// Replication policy inside the original conditional native GMem mark lifetime.
pub(crate) unsafe fn server_tick_client_body_with<O: LevelTickNetOps>(
    ops: &O,
    level: *mut c_void,
    connection: *mut c_void,
    mem_mark: *mut c_void,
) -> Result<i32, O::Error> {
    unsafe {
        let lv = level.cast::<ULevel>();
        let gmem = ops.level_gmem()?;
        let mut updated = 0i32;
        // Get list of visible/relevant actors.
        (*lv).net_tag = (*lv).net_tag.wrapping_add(1);
        ops.conn_inc_tick_count(connection)?;

        // Set up to skip all sent temporary actors.
        ops.guard(8, |inner| {
        let ops = inner;
            let mut i: i32 = 0;
            while i < ops.conn_sent_temp_count(connection)? {
                ops.conn_sent_temp_set_net_tag(connection, i, (*lv).net_tag)?;
                i += 1;
            }

            Ok(())
        })?;
        // Get viewer coordinates.
        let mut viewer = ops.conn_actor(connection)?;
        let in_viewer = ops.conn_actor(connection)?.cast::<APlayerPawn>();
        let mut location: FVector = (*in_viewer).base.base.location;
        let rot = (*in_viewer).base.view_rotation;
        let mut loc_arr = [location.x, location.y, location.z];
        let mut rot_arr = [rot.pitch, rot.yaw, rot.roll];
        viewer =
            ops.actor_event_player_calc_view(viewer, loc_arr.as_mut_ptr(), rot_arr.as_mut_ptr())?;
        location = FVector {
            x: loc_arr[0],
            y: loc_arr[1],
            z: loc_arr[2],
        };
        ops.check(!viewer.is_null(), c"Viewer", 745)?;

        // Compute ahead-vectors for prediction.
        if ops.conn_tick_count(connection)? & 1 != 0 {
            let predict_seconds: f32 = if ops.conn_tick_count(connection)? & 2 != 0 {
                0.4_f32
            } else {
                0.9_f32
            };
            let vel = (*viewer.cast::<AActor>()).velocity;
            let mut ahead = FVector {
                x: predict_seconds * vel.x,
                y: predict_seconds * vel.y,
                z: predict_seconds * vel.z,
            };
            let base = (*viewer.cast::<AActor>()).base;
            if !base.is_null() {
                let base_vel = (*base.cast::<AActor>()).velocity;
                ahead.x += predict_seconds * base_vel.x;
                ahead.y += predict_seconds * base_vel.y;
                ahead.z += predict_seconds * base_vel.z;
            }
            // `FCheckResult Hit(1.0f); Hit.Location = Location + Ahead;`
            let start = FVector {
                x: location.x + ahead.x,
                y: location.y + ahead.y,
                z: location.z + ahead.z,
            };
            let mut hit = start;
            let viewer_level = (*viewer.cast::<AActor>()).x_level;
            ops.level_model_line_check(
                viewer_level,
                ptr::from_mut(&mut hit).cast::<f32>(),
                ptr::from_ref(&start).cast::<f32>(),
                ptr::from_ref(&location).cast::<f32>(),
                NF_NOT_VIS_BLOCKING,
            )?;
            location = hit;
        }

        // Make list of all actors to consider.
        let priority_list =
            gmem_push_with::<ActorPriority, O>(ops, gmem, ops.level_actor_count(level)? as usize)?;
        let priority_actors =
            gmem_push_with::<*mut ActorPriority, O>(ops, gmem, ops.level_actor_count(level)? as usize)?;
        let mut view_pos: FVector = (*viewer.cast::<AActor>()).location;
        let view_rot = (*in_viewer).base.view_rotation;
        let mut view_dir = rotator_vector_with(ops, view_rot)?;
        let mut last_time = ops.conn_last_rep_time(connection)?;
        let mut this_time = ops.driver_time(ops.conn_driver(connection)?)?;
        let mut consider_count = 0usize;

        ops.guard(9, |inner| {
        let ops = inner;
            // Add LevelInfo to the list.
            for i in 0..2 {
                let actor = ops.level_actor(level, i)?;
                if !actor.is_null() {
                    let a = actor.cast::<AActor>();
                    if (*a).net_tag != (*lv).net_tag && (*a).remote_role != ROLE_NONE {
                        (*a).net_tag = (*lv).net_tag;
                        append_priority(
                            priority_list,
                            priority_actors,
                            &mut consider_count,
                            make_priority_with(ops, view_pos, view_dir, connection, actor)?,
                        );
                    }
                }
            }
            let server_tick_rate = ops.level_max_tick_rate(level)?;
            let mut i: i32 = (*lv).i_first_net_relevant_actor;
            while i < ops.level_actor_count(level)? {
                let actor = ops.level_actor(level, i)?;
                if !actor.is_null() {
                    let a = actor.cast::<AActor>();
                    if (*a).net_tag != (*lv).net_tag && (*a).remote_role != ROLE_NONE {
                        (*a).net_tag = (*lv).net_tag;
                        if (*a).b_always_relevant() {
                            let freq = (*a).net_update_frequency;
                            if app_round(ftime_get_float(last_time) * freq)
                                != app_round(ftime_get_float(this_time) * freq)
                            {
                                let mut channel = ptr::null_mut();
                                if ops.actor_check_recent_changes(actor)? != 0 {
                                    channel = ops.conn_actor_channel(connection, actor)?;
                                }
                                if !channel.is_null()
                                    && ops.channel_recent_count(channel)? != 0
                                    && ops.channel_dirty_count(channel)? == 0
                                    && ops.actor_no_variables_to_replicate(
                                        actor,
                                        ops.channel_recent_first(channel)?,
                                    )? != 0
                                {
                                    ops.channel_set_relevant_time(
                                        channel,
                                        ops.driver_time((*lv).base.net_driver)?,
                                    )?;
                                } else {
                                    append_priority(
                                        priority_list,
                                        priority_actors,
                                        &mut consider_count,
                                        make_priority_with(ops, view_pos, view_dir, connection, actor)?,
                                    );
                                }
                            }
                            last_time = ftime_add_f32(last_time, 0.023_f32);
                            this_time = ftime_add_f32(this_time, 0.023_f32);
                        } else if !(*a).b_net_optional()
                            || (*a).life_span > ops.actor_class_default_life_span(actor)? - 0.15_f32
                        {
                            let upd_freq = actor_update_frequency_with(
                                ops,
                                actor,
                                viewer,
                                ptr::from_mut(&mut view_dir).cast::<f32>(),
                                ptr::from_mut(&mut view_pos).cast::<f32>(),
                            )?;
                            if upd_freq >= server_tick_rate
                                || app_round(ftime_get_float(last_time) * upd_freq)
                                    != app_round(ftime_get_float(this_time) * upd_freq)
                            {
                                append_priority(
                                    priority_list,
                                    priority_actors,
                                    &mut consider_count,
                                    make_priority_with(ops, view_pos, view_dir, connection, actor)?,
                                );
                            }
                            last_time = ftime_add_f32(last_time, 0.023_f32);
                            this_time = ftime_add_f32(this_time, 0.023_f32);
                        }
                    }
                }
                i += 1;
            }
            ops.conn_set_last_rep_time(connection, ops.driver_time(ops.conn_driver(connection)?)?)?;

            Ok(())
        })?;
        // Native Sort observes the original raw/stale pointer slots; Rust never
        // materializes uninitialized native storage as initialized pointer values.
        ops.sort_priorities(priority_actors, consider_count as i32)?;

        ops.guard(11, |inner| {
        let ops = inner;
            // Update all relevant actors in sorted order.
            for j in 0..consider_count {
                let entry = *priority_actors.add(j);
                let actor = (*entry).actor;
                let mut channel = (*entry).channel;
                let mut can_see = false;
                // Only check visibility on already visible actors every 0.3 + 0.2R seconds.
                if channel.is_null()
                    || f64::from(ftime_sub(
                        ops.driver_time((*lv).base.net_driver)?,
                        ops.channel_relevant_time(channel)?,
                    )) > 0.3
                {
                    can_see =
                        actor_can_see_with(ops, actor, ops.conn_actor(connection)?, viewer, location)?;
                }
                if can_see
                    || (!channel.is_null()
                        && ftime_sub(
                            ops.driver_time((*lv).base.net_driver)?,
                            ops.channel_relevant_time(channel)?,
                        ) < ops.driver_relevant_timeout((*lv).base.net_driver)?)
                {
                    // Find or create the channel for this actor.
                    ops.level_inc_num_pv((*actor.cast::<AActor>()).x_level)?;
                    if channel.is_null()
                        && ops.conn_package_map_class_index(connection, actor)? != INDEX_NONE
                    {
                        // Create a new channel for this actor.
                        channel = ops.conn_create_actor_channel(connection)?;
                        if !channel.is_null() {
                            ops.conn_set_channel_actor(channel, actor)?;
                        }
                    }
                    if !channel.is_null() {
                        if ops.conn_is_net_ready(connection, 0)? == 0 {
                            break;
                        }
                        if can_see {
                            ops.channel_set_relevant_time(
                                channel,
                                ftime_add_f32(
                                    ops.driver_time((*lv).base.net_driver)?,
                                    0.2_f32 * ops.level_app_frand()?,
                                ),
                            )?;
                        }
                        if ops.channel_is_net_ready(channel, 0)? != 0 {
                            ops.channel_replicate_actor(channel)?;
                            updated += 1;
                        }
                        if ops.conn_is_net_ready(connection, 0)? == 0 {
                            break;
                        }
                    }
                } else if !channel.is_null() {
                    ops.channel_close(channel)?;
                }
            }
            Ok(())
        })?;
        ops.mem_mark_pop(mem_mark)?;
        Ok(updated)
    }
}

/// `ULevel::TickNetServer` (`UnLevTic.cpp:904-961`). `DeltaSeconds` drives the log check.
///
/// # Safety
/// `level` must be a live `ULevel` whose `NetDriver` is live.
pub(crate) unsafe fn tick_net_server_with<O: LevelTickNetOps>(
    ops: &O,
    level: *mut c_void,
    delta_seconds: f32,
) -> Result<(), O::Error> {
    unsafe {
        let lv = level.cast::<ULevel>();

        // Update all clients.
        ops.level_cycles_clock(level, NET_TICK_CYCLES)?;
        let mut updated: i32 = 0;
        let mut i = ops.driver_client_connection_count((*lv).base.net_driver)? - 1;
        while i >= 0 {
            updated = updated.wrapping_add(ops.level_server_tick_client(
                level,
                ops.driver_client_connection((*lv).base.net_driver, i)?,
                delta_seconds,
            )?);
            i -= 1;
        }
        ops.level_cycles_unclock(level, NET_TICK_CYCLES)?;

        // Log message: `appRound(TimeSeconds-DeltaSeconds)!=appRound(TimeSeconds.GetFloat())`.
        let time_seconds = i64::from_ne_bytes((*lv).time_seconds);
        if app_round(ftime_sub(time_seconds, ftime_from_f32(delta_seconds)))
            != app_round(ftime_get_float(time_seconds))
        {
            ops.level_log_running_net(
                level,
                ops.driver_client_connection_count((*lv).base.net_driver)?,
            )?;
        }

        // Stats.
        if updated != 0 {
            ops.level_send_net_stats(level, updated)?;
        }
        Ok(())
    }
}

/// Iterate while the actual native TMap iterator remains on the native stack.
pub(crate) unsafe fn replicate_player_channels_body_with<O: LevelTickNetOps>(
    ops: &O,
    iterator: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        while ops.channel_iterator_valid(iterator)? != 0 {
            ops.guard(14, |inner| {
        let ops = inner;
                let channel = ops.channel_iterator_value(iterator)?;
                let actor = ops.channel_actor(channel)?;
                if !actor.is_null()
                    && ops.actor_is_a(actor, 1)? != 0
                    && !(*actor.cast::<APlayerPawn>()).player.is_null()
                {
                    ops.channel_replicate_actor(channel)?;
                }
                Ok(())
            })?;
            ops.channel_iterator_next(iterator)?;
        }
        Ok(())
    }
}

/// Network statistics selection remains Rust policy. Each native send callback
/// owns only its typed FString formatting/event lifetime.
pub(crate) unsafe fn send_net_stats_with<O: LevelTickNetOps>(
    ops: &O,
    level: *mut c_void,
    updated: i32,
) -> Result<(), O::Error> {
    unsafe {
        if updated != 0 {
            let lv = level.cast::<ULevel>();
            let mut i = 0;
            while i < ops.driver_client_connection_count((*lv).base.net_driver)? {
                let connection = ops.driver_client_connection((*lv).base.net_driver, i)?;
                if !ops.conn_actor(connection)?.is_null()
                    && ops.conn_state(connection)? == USOCK_OPEN
                {
                    if ops.conn_user_flags(connection)? & 1 != 0 {
                        let mut actors = 0i32;
                        let mut j = 0;
                        while j < ops.level_actor_count(level)? {
                            actors =
                                actors.wrapping_add((!ops.level_actor(level, j)?.is_null()) as i32);
                            j += 1;
                        }
                        let rate = ops.level_max_tick_rate(level)?;
                        ops.conn_send_stats(level, connection, 0, actors, rate)?;
                    }
                    if ops.conn_user_flags(connection)? & 2 != 0 {
                        ops.conn_send_stats(level, connection, 1, 0, 0.0)?;
                    }
                }
                i += 1;
            }
        }
        Ok(())
    }
}

/// Source-shared operations for `ULevel::TickNetClient`, `ULevel::ServerTickClient`
/// and `ULevel::TickNetServer`; Core implements these without native callbacks.
pub(crate) trait LevelTickNetOps {
    type Error;
    /// Source guard scope; canonical hosts execute the body directly.
    unsafe fn guard<T, F>(&self, scope: i32, f: F) -> Result<T, Self::Error>
    where
        F: FnOnce(&Self) -> Result<T, Self::Error>;
    /// `check(ok)` at the policy line.
    unsafe fn check(&self, ok: bool, expression: &std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    /// `Connection->ReplicatePlayerChannels()` driving the shared body.
    unsafe fn replicate_player_channels(&self, connection: *mut c_void) -> Result<(), Self::Error>;
    /// `ULevel::ServerTickClient(Connection)` through the shared policy.
    unsafe fn server_tick_client(&self, level: *mut c_void, connection: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn actor_update_frequency(&self, actor: *mut c_void, viewer: *mut c_void, view_dir: *mut f32, view_pos: *mut f32) -> Result<f32, Self::Error>;
    unsafe fn level_cycles_clock(&self, level: *mut c_void, which: i32) -> Result<(), Self::Error>;
    unsafe fn driver_server_connection(&self, driver: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn conn_state(&self, conn: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn conn_replicate_player_channels(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn level_client_viewports_num(&self, level: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_engine_client_travel(&self, level: *mut c_void, url: i32) -> Result<(), Self::Error>;
    unsafe fn level_cycles_unclock(&self, level: *mut c_void, which: i32) -> Result<(), Self::Error>;
    unsafe fn conn_actor(&self, conn: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn conn_is_net_ready(&self, conn: *mut c_void, saturate: i32) -> Result<i32, Self::Error>;
    unsafe fn driver_time(&self, driver: *mut c_void) -> Result<i64, Self::Error>;
    unsafe fn conn_driver(&self, conn: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn conn_last_receive_time(&self, conn: *mut c_void) -> Result<i64, Self::Error>;
    unsafe fn server_tick_client_scoped(&self, level: *mut c_void, connection: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_gmem(&self) -> Result<*mut c_void, Self::Error>;
    unsafe fn conn_inc_tick_count(&self, conn: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn conn_sent_temp_count(&self, conn: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn conn_sent_temp_set_net_tag(&self, conn: *mut c_void, index: i32, net_tag: i32) -> Result<(), Self::Error>;
    unsafe fn actor_event_player_calc_view(&self, viewer: *mut c_void, location: *mut f32, rotation: *mut i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn conn_tick_count(&self, conn: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_model_line_check(&self, level: *mut c_void, out_location: *mut f32, start: *const f32, end: *const f32, node_flags: i32) -> Result<(), Self::Error>;
    unsafe fn level_actor_count(&self, level: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn conn_last_rep_time(&self, conn: *mut c_void) -> Result<i64, Self::Error>;
    unsafe fn level_actor(&self, level: *mut c_void, index: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn level_max_tick_rate(&self, level: *mut c_void) -> Result<f32, Self::Error>;
    unsafe fn actor_check_recent_changes(&self, actor: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn conn_actor_channel(&self, conn: *mut c_void, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn channel_recent_count(&self, channel: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn channel_dirty_count(&self, channel: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn actor_no_variables_to_replicate(&self, actor: *mut c_void, old_version: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn channel_recent_first(&self, channel: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn channel_set_relevant_time(&self, channel: *mut c_void, time: i64) -> Result<(), Self::Error>;
    unsafe fn actor_class_default_life_span(&self, actor: *mut c_void) -> Result<f32, Self::Error>;
    unsafe fn conn_set_last_rep_time(&self, conn: *mut c_void, time: i64) -> Result<(), Self::Error>;
    unsafe fn sort_priorities(&self, actors: *mut *mut ActorPriority, count: i32) -> Result<(), Self::Error>;
    unsafe fn channel_relevant_time(&self, channel: *mut c_void) -> Result<i64, Self::Error>;
    unsafe fn driver_relevant_timeout(&self, driver: *mut c_void) -> Result<f32, Self::Error>;
    unsafe fn level_inc_num_pv(&self, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn conn_package_map_class_index(&self, conn: *mut c_void, actor: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn conn_create_actor_channel(&self, conn: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn conn_set_channel_actor(&self, channel: *mut c_void, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn level_app_frand(&self) -> Result<f32, Self::Error>;
    unsafe fn channel_is_net_ready(&self, channel: *mut c_void, saturate: i32) -> Result<i32, Self::Error>;
    unsafe fn channel_replicate_actor(&self, channel: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn channel_close(&self, channel: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn mem_mark_pop(&self, buffer: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn mem_push(&self, mem: *mut c_void, size: u64, count: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn driver_client_connection_count(&self, driver: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_server_tick_client(&self, level: *mut c_void, connection: *mut c_void, delta: f32) -> Result<i32, Self::Error>;
    unsafe fn driver_client_connection(&self, driver: *mut c_void, index: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn level_log_running_net(&self, level: *mut c_void, client_count: i32) -> Result<(), Self::Error>;
    unsafe fn level_send_net_stats(&self, level: *mut c_void, updated: i32) -> Result<(), Self::Error>;
    unsafe fn channel_iterator_valid(&self, iterator: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn channel_iterator_value(&self, iterator: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn channel_actor(&self, channel: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_is_a(&self, actor: *mut c_void, kind: i32) -> Result<i32, Self::Error>;
    unsafe fn channel_iterator_next(&self, iterator: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn conn_user_flags(&self, connection: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn conn_send_stats(&self, level: *mut c_void, connection: *mut c_void, kind: i32, actor_count: i32, max_rate: f32) -> Result<(), Self::Error>;
    unsafe fn driver_spawn_priority_seconds(&self, driver: *mut c_void) -> Result<f32, Self::Error>;
    unsafe fn channel_last_update_time(&self, channel: *mut c_void) -> Result<i64, Self::Error>;
    unsafe fn actor_net_priority(&self, actor: *mut c_void, sent: *mut c_void, time: f32, lag: f32) -> Result<f32, Self::Error>;
    unsafe fn conn_best_lag(&self, conn: *mut c_void) -> Result<f32, Self::Error>;
    unsafe fn level_fast_line_check(&self, level: *mut c_void, end: *const f32, start: *const f32) -> Result<i32, Self::Error>;
    unsafe fn gmath_sin_tab(&self, index: i32) -> Result<f32, Self::Error>;
    unsafe fn gmath_cos_tab(&self, index: i32) -> Result<f32, Self::Error>;
}

pub(crate) struct NativeNetTickOps<'a>(pub(crate) &'a Hp2LevTicServices);

impl LevelTickNetOps for NativeNetTickOps<'_> {
    type Error = i32;
    unsafe fn guard<T, F>(&self, _scope: i32, f: F) -> Result<T, i32>
    where
        F: FnOnce(&Self) -> Result<T, i32>,
    { f(self) }
    unsafe fn check(&self, ok: bool, expression: &std::ffi::CStr, line: i32) -> Result<(), i32> {
        unsafe { crate::lev_tic::check(self.0, ok, expression, line) }
    }
    unsafe fn replicate_player_channels(&self, connection: *mut c_void) -> Result<(), i32> {
        unsafe { self.0.conn_replicate_player_channels(connection) }
    }
    unsafe fn server_tick_client(&self, level: *mut c_void, connection: *mut c_void) -> Result<i32, i32> {
        unsafe { self.0.server_tick_client_scoped(level, connection) }
    }
    unsafe fn actor_update_frequency(&self, actor: *mut c_void, viewer: *mut c_void, view_dir: *mut f32, view_pos: *mut f32) -> Result<f32, Self::Error> {
        unsafe { self.0.actor_update_frequency(actor, viewer, view_dir, view_pos) }
    }
    unsafe fn level_cycles_clock(&self, level: *mut c_void, which: i32) -> Result<(), Self::Error> {
        unsafe { self.0.level_cycles_clock(level, which) }
    }
    unsafe fn driver_server_connection(&self, driver: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.driver_server_connection(driver) }
    }
    unsafe fn conn_state(&self, conn: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.conn_state(conn) }
    }
    unsafe fn conn_replicate_player_channels(&self, conn: *mut c_void) -> Result<(), Self::Error> {
        unsafe { self.0.conn_replicate_player_channels(conn) }
    }
    unsafe fn level_client_viewports_num(&self, level: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.level_client_viewports_num(level) }
    }
    unsafe fn level_engine_client_travel(&self, level: *mut c_void, url: i32) -> Result<(), Self::Error> {
        unsafe { self.0.level_engine_client_travel(level, url) }
    }
    unsafe fn level_cycles_unclock(&self, level: *mut c_void, which: i32) -> Result<(), Self::Error> {
        unsafe { self.0.level_cycles_unclock(level, which) }
    }
    unsafe fn conn_actor(&self, conn: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.conn_actor(conn) }
    }
    unsafe fn conn_is_net_ready(&self, conn: *mut c_void, saturate: i32) -> Result<i32, Self::Error> {
        unsafe { self.0.conn_is_net_ready(conn, saturate) }
    }
    unsafe fn driver_time(&self, driver: *mut c_void) -> Result<i64, Self::Error> {
        unsafe { self.0.driver_time(driver) }
    }
    unsafe fn conn_driver(&self, conn: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.conn_driver(conn) }
    }
    unsafe fn conn_last_receive_time(&self, conn: *mut c_void) -> Result<i64, Self::Error> {
        unsafe { self.0.conn_last_receive_time(conn) }
    }
    unsafe fn server_tick_client_scoped(&self, level: *mut c_void, connection: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.server_tick_client_scoped(level, connection) }
    }
    unsafe fn level_gmem(&self) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.level_gmem() }
    }
    unsafe fn conn_inc_tick_count(&self, conn: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.conn_inc_tick_count(conn) }
    }
    unsafe fn conn_sent_temp_count(&self, conn: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.conn_sent_temp_count(conn) }
    }
    unsafe fn conn_sent_temp_set_net_tag(&self, conn: *mut c_void, index: i32, net_tag: i32) -> Result<(), Self::Error> {
        unsafe { self.0.conn_sent_temp_set_net_tag(conn, index, net_tag) }
    }
    unsafe fn actor_event_player_calc_view(&self, viewer: *mut c_void, location: *mut f32, rotation: *mut i32) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.actor_event_player_calc_view(viewer, location, rotation) }
    }
    unsafe fn conn_tick_count(&self, conn: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.conn_tick_count(conn) }
    }
    unsafe fn level_model_line_check(&self, level: *mut c_void, out_location: *mut f32, start: *const f32, end: *const f32, node_flags: i32) -> Result<(), Self::Error> {
        unsafe { self.0.level_model_line_check(level, out_location, start, end, node_flags) }
    }
    unsafe fn level_actor_count(&self, level: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.level_actor_count(level) }
    }
    unsafe fn conn_last_rep_time(&self, conn: *mut c_void) -> Result<i64, Self::Error> {
        unsafe { self.0.conn_last_rep_time(conn) }
    }
    unsafe fn level_actor(&self, level: *mut c_void, index: i32) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.level_actor(level, index) }
    }
    unsafe fn level_max_tick_rate(&self, level: *mut c_void) -> Result<f32, Self::Error> {
        unsafe { self.0.level_max_tick_rate(level) }
    }
    unsafe fn actor_check_recent_changes(&self, actor: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.actor_check_recent_changes(actor) }
    }
    unsafe fn conn_actor_channel(&self, conn: *mut c_void, actor: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.conn_actor_channel(conn, actor) }
    }
    unsafe fn channel_recent_count(&self, channel: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.channel_recent_count(channel) }
    }
    unsafe fn channel_dirty_count(&self, channel: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.channel_dirty_count(channel) }
    }
    unsafe fn actor_no_variables_to_replicate(&self, actor: *mut c_void, old_version: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.actor_no_variables_to_replicate(actor, old_version) }
    }
    unsafe fn channel_recent_first(&self, channel: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.channel_recent_first(channel) }
    }
    unsafe fn channel_set_relevant_time(&self, channel: *mut c_void, time: i64) -> Result<(), Self::Error> {
        unsafe { self.0.channel_set_relevant_time(channel, time) }
    }
    unsafe fn actor_class_default_life_span(&self, actor: *mut c_void) -> Result<f32, Self::Error> {
        unsafe { self.0.actor_class_default_life_span(actor) }
    }
    unsafe fn conn_set_last_rep_time(&self, conn: *mut c_void, time: i64) -> Result<(), Self::Error> {
        unsafe { self.0.conn_set_last_rep_time(conn, time) }
    }
    unsafe fn sort_priorities(&self, actors: *mut *mut ActorPriority, count: i32) -> Result<(), Self::Error> {
        unsafe { self.0.sort_priorities(actors, count) }
    }
    unsafe fn channel_relevant_time(&self, channel: *mut c_void) -> Result<i64, Self::Error> {
        unsafe { self.0.channel_relevant_time(channel) }
    }
    unsafe fn driver_relevant_timeout(&self, driver: *mut c_void) -> Result<f32, Self::Error> {
        unsafe { self.0.driver_relevant_timeout(driver) }
    }
    unsafe fn level_inc_num_pv(&self, level: *mut c_void) -> Result<(), Self::Error> {
        unsafe { self.0.level_inc_num_pv(level) }
    }
    unsafe fn conn_package_map_class_index(&self, conn: *mut c_void, actor: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.conn_package_map_class_index(conn, actor) }
    }
    unsafe fn conn_create_actor_channel(&self, conn: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.conn_create_actor_channel(conn) }
    }
    unsafe fn conn_set_channel_actor(&self, channel: *mut c_void, actor: *mut c_void) -> Result<(), Self::Error> {
        unsafe { self.0.conn_set_channel_actor(channel, actor) }
    }
    unsafe fn level_app_frand(&self) -> Result<f32, Self::Error> {
        unsafe { self.0.level_app_frand() }
    }
    unsafe fn channel_is_net_ready(&self, channel: *mut c_void, saturate: i32) -> Result<i32, Self::Error> {
        unsafe { self.0.channel_is_net_ready(channel, saturate) }
    }
    unsafe fn channel_replicate_actor(&self, channel: *mut c_void) -> Result<(), Self::Error> {
        unsafe { self.0.channel_replicate_actor(channel) }
    }
    unsafe fn channel_close(&self, channel: *mut c_void) -> Result<(), Self::Error> {
        unsafe { self.0.channel_close(channel) }
    }
    unsafe fn mem_push(&self, mem: *mut c_void, size: u64, count: i32) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.mem_push(mem, size, count) }
    }
    unsafe fn mem_mark_pop(&self, buffer: *mut c_void) -> Result<(), Self::Error> {
        unsafe { self.0.mem_mark_pop(buffer) }
    }
    unsafe fn driver_client_connection_count(&self, driver: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.driver_client_connection_count(driver) }
    }
    unsafe fn level_server_tick_client(&self, level: *mut c_void, connection: *mut c_void, delta: f32) -> Result<i32, Self::Error> {
        unsafe { self.0.level_server_tick_client(level, connection, delta) }
    }
    unsafe fn driver_client_connection(&self, driver: *mut c_void, index: i32) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.driver_client_connection(driver, index) }
    }
    unsafe fn level_log_running_net(&self, level: *mut c_void, client_count: i32) -> Result<(), Self::Error> {
        unsafe { self.0.level_log_running_net(level, client_count) }
    }
    unsafe fn level_send_net_stats(&self, level: *mut c_void, updated: i32) -> Result<(), Self::Error> {
        unsafe { self.0.level_send_net_stats(level, updated) }
    }
    unsafe fn channel_iterator_valid(&self, iterator: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.channel_iterator_valid(iterator) }
    }
    unsafe fn channel_iterator_value(&self, iterator: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.channel_iterator_value(iterator) }
    }
    unsafe fn channel_actor(&self, channel: *mut c_void) -> Result<*mut c_void, Self::Error> {
        unsafe { self.0.channel_actor(channel) }
    }
    unsafe fn actor_is_a(&self, actor: *mut c_void, kind: i32) -> Result<i32, Self::Error> {
        unsafe { self.0.actor_is_a(actor, kind) }
    }
    unsafe fn channel_iterator_next(&self, iterator: *mut c_void) -> Result<(), Self::Error> {
        unsafe { self.0.channel_iterator_next(iterator) }
    }
    unsafe fn conn_user_flags(&self, connection: *mut c_void) -> Result<i32, Self::Error> {
        unsafe { self.0.conn_user_flags(connection) }
    }
    unsafe fn conn_send_stats(&self, level: *mut c_void, connection: *mut c_void, kind: i32, actor_count: i32, max_rate: f32) -> Result<(), Self::Error> {
        unsafe { self.0.conn_send_stats(level, connection, kind, actor_count, max_rate) }
    }
    unsafe fn driver_spawn_priority_seconds(&self, driver: *mut c_void) -> Result<f32, i32> {
        unsafe { self.0.driver_spawn_priority_seconds(driver) }
    }
    unsafe fn channel_last_update_time(&self, channel: *mut c_void) -> Result<i64, i32> {
        unsafe { self.0.channel_last_update_time(channel) }
    }
    unsafe fn actor_net_priority(&self, actor: *mut c_void, sent: *mut c_void, time: f32, lag: f32) -> Result<f32, i32> {
        unsafe { self.0.actor_net_priority(actor, sent, time, lag) }
    }
    unsafe fn conn_best_lag(&self, conn: *mut c_void) -> Result<f32, i32> {
        unsafe { self.0.conn_best_lag(conn) }
    }
    unsafe fn level_fast_line_check(&self, level: *mut c_void, end: *const f32, start: *const f32) -> Result<i32, i32> {
        unsafe { self.0.level_fast_line_check(level, end, start) }
    }
    unsafe fn gmath_sin_tab(&self, index: i32) -> Result<f32, i32> {
        unsafe { self.0.gmath_sin_tab(index) }
    }
    unsafe fn gmath_cos_tab(&self, index: i32) -> Result<f32, i32> {
        unsafe { self.0.gmath_cos_tab(index) }
    }
}

/// `ULevel::TickNetClient` via the native service table.
///
/// # Safety
/// `level` and `svc` satisfy the native contracts.
pub unsafe fn tick_net_client(svc: &Hp2LevTicServices, level: *mut c_void) -> Result<(), i32> {
    unsafe { tick_net_client_with(&NativeNetTickOps(svc), level) }
}
/// `ULevel::ServerTickClient` via the native service table.
///
/// # Safety
/// `level`, `connection` and `svc` satisfy the native contracts.
pub unsafe fn server_tick_client(svc: &Hp2LevTicServices, level: *mut c_void, connection: *mut c_void) -> Result<i32, i32> {
    unsafe { server_tick_client_with(&NativeNetTickOps(svc), level, connection) }
}
/// `ServerTickClient` body inside the native stack mark.
///
/// # Safety
/// `mem_mark` is the live native `FMemMark` for this body.
pub unsafe fn server_tick_client_body(svc: &Hp2LevTicServices, level: *mut c_void, connection: *mut c_void, mem_mark: *mut c_void) -> Result<i32, i32> {
    unsafe { server_tick_client_body_with(&NativeNetTickOps(svc), level, connection, mem_mark) }
}
/// `ULevel::TickNetServer` via the native service table.
///
/// # Safety
/// `level` and `svc` satisfy the native contracts.
pub unsafe fn tick_net_server(svc: &Hp2LevTicServices, level: *mut c_void, delta_seconds: f32) -> Result<(), i32> {
    unsafe { tick_net_server_with(&NativeNetTickOps(svc), level, delta_seconds) }
}
/// `ReplicatePlayerChannels` body over the native `TMap` iterator.
///
/// # Safety
/// `iterator` is a live native iterator.
pub unsafe fn replicate_player_channels_body(svc: &Hp2LevTicServices, iterator: *mut c_void) -> Result<(), i32> {
    unsafe { replicate_player_channels_body_with(&NativeNetTickOps(svc), iterator) }
}
/// `Level::SendNetStats` via the native service table.
///
/// # Safety
/// `level` and `svc` satisfy the native contracts.
pub unsafe fn send_net_stats(svc: &Hp2LevTicServices, level: *mut c_void, updated: i32) -> Result<(), i32> {
    unsafe { send_net_stats_with(&NativeNetTickOps(svc), level, updated) }
}
/// `new(Connection->Mem, Count) T` through the native stack.
///
/// # Safety
/// `mem` is the native `FMemStack`.
pub unsafe fn gmem_push<T>(svc: &Hp2LevTicServices, mem: *mut c_void, count: usize) -> Result<*mut T, i32> {
    unsafe { gmem_push_with::<T, NativeNetTickOps>(&NativeNetTickOps(svc), mem, count) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(16))]
    struct Arena([u8; 512]);

    #[test]
    fn server_tick_priority_arrays_keep_rhs_first_stale_slot() {
        let mut arena = Arena([0; 512]);
        let base = arena.0.as_mut_ptr();
        for offset in (0..arena.0.len()).step_by(std::mem::size_of::<usize>()) {
            unsafe {
                base.add(offset)
                    .cast::<usize>()
                    .write_unaligned(base as usize);
            }
        }
        let mut mem = TestMemStack {
            top: base,
            end: unsafe { base.add(arena.0.len()) },
        };
        let mem_ptr = ptr::from_mut(&mut mem).cast::<c_void>();
        let svc = test_services();
        let list = unsafe { gmem_push::<ActorPriority>(&svc, mem_ptr, 4).unwrap() };
        let actors = unsafe { gmem_push::<*mut ActorPriority>(&svc, mem_ptr, 4).unwrap() };
        let mut count = 0;
        unsafe {
            append_priority(
                list,
                actors,
                &mut count,
                ActorPriority {
                    priority: 10,
                    actor: ptr::dangling_mut(),
                    channel: ptr::null_mut(),
                },
            );
            append_priority(
                list,
                actors,
                &mut count,
                ActorPriority {
                    priority: 20,
                    actor: 2usize as *mut c_void,
                    channel: ptr::null_mut(),
                },
            );
            append_priority(
                list,
                actors,
                &mut count,
                ActorPriority {
                    priority: 30,
                    actor: 3usize as *mut c_void,
                    channel: ptr::null_mut(),
                },
            );
            sort_priority_ptrs(std::slice::from_raw_parts_mut(actors, count));
            let order = [
                (**actors.add(0)).actor as usize,
                (**actors.add(1)).actor as usize,
                (**actors.add(2)).actor as usize,
            ];
            assert_eq!(order, [2, 1, 1]);
        }
    }

    use hp2_abi::engine::generated::{
        AActor, APlayerPawn, UActorChannel, ULevel, UNetConnection, UNetDriver,
    };
    use std::cell::{Cell, RefCell};

    #[repr(C)]
    struct TestMemStack {
        top: *mut u8,
        end: *mut u8,
    }

    thread_local! {
        static TEST_GMEM: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
        static TEST_DRIVER: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
        static TEST_CHANNEL: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
        static TEST_ACTORS: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
        static TEST_CHANNEL_ACTOR: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
        static TEST_REPLICATED: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn no_service() {
        eprintln!("no_service\n{}", std::backtrace::Backtrace::force_capture());
        std::process::abort();
    }
    unsafe extern "C" fn test_gmem() -> *mut c_void {
        TEST_GMEM.with(Cell::get)
    }
    unsafe extern "C" fn test_mark_save(mem: *mut c_void, mark: *mut c_void) {
        let stack = mem.cast::<TestMemStack>();
        let saved = [mem, unsafe { (*stack).top.cast() }, ptr::null_mut()];
        unsafe { mark.cast::<[*mut c_void; 3]>().write(saved) };
    }
    unsafe extern "C" fn test_mark_pop(mark: *mut c_void) {
        let saved = unsafe { mark.cast::<[*mut c_void; 3]>().read() };
        let stack = saved[0].cast::<TestMemStack>();
        unsafe { (*stack).top = saved[1].cast() };
    }
    /// `FMemStack::PushBytes` (UnMem.h:35) with `DEFAULT_ALIGNMENT` (8) over a single pre-sized
    /// test chunk; the tests never need `AllocateNewChunk`, so running out aborts.
    unsafe extern "C" fn test_mem_push(mem: *mut c_void, size: u64, count: i32) -> *mut c_void {
        let stack = mem.cast::<TestMemStack>();
        unsafe {
            let start = ((*stack).top as usize + 7) & !7;
            let next = start + size as usize * count as usize;
            if next > (*stack).end as usize {
                std::process::abort();
            }
            (*stack).top = next as *mut u8;
            start as *mut c_void
        }
    }
    unsafe extern "C" fn test_conn_ready(_connection: *mut c_void, _saturate: i32) -> i32 {
        1
    }
    unsafe extern "C" fn test_conn_sent_temp_count(_connection: *mut c_void) -> i32 {
        0
    }
    unsafe extern "C" fn test_player_calc_view(
        viewer: *mut c_void,
        _location: *mut f32,
        _rotation: *mut i32,
    ) -> *mut c_void {
        viewer
    }
    unsafe extern "C" fn test_level_actor_count(_level: *mut c_void) -> i32 {
        TEST_ACTORS.with(|a| a.borrow().len() as i32)
    }
    unsafe extern "C" fn test_level_actor(_level: *mut c_void, index: i32) -> *mut c_void {
        TEST_ACTORS.with(|a| a.borrow()[index as usize])
    }
    unsafe extern "C" fn test_max_tick_rate(_level: *mut c_void) -> f32 {
        30.0
    }
    unsafe extern "C" fn test_update_frequency(
        _actor: *mut c_void,
        _viewer: *mut c_void,
        _view_dir: *mut f32,
        _view_pos: *mut f32,
    ) -> f32 {
        60.0
    }
    /// Sentinel ops-record override distinct from `test_update_frequency`'s
    /// `60.0`, so tests can tell which path answered.
    unsafe extern "C" fn probe_update_frequency(_actor: *const c_void) -> f32 {
        17.0
    }
    unsafe extern "C" fn test_actor_net_priority(
        _actor: *mut c_void,
        _sent: *mut c_void,
        _time: f32,
        _lag: f32,
    ) -> f32 {
        1.0
    }
    unsafe extern "C" fn test_actor_channel(
        _connection: *mut c_void,
        _actor: *mut c_void,
    ) -> *mut c_void {
        ptr::null_mut()
    }
    unsafe extern "C" fn test_fast_line(
        _level: *mut c_void,
        _end: *const f32,
        _start: *const f32,
    ) -> i32 {
        1
    }
    unsafe extern "C" fn test_class_index(_connection: *mut c_void, _actor: *mut c_void) -> i32 {
        1
    }
    unsafe extern "C" fn test_create_channel(_connection: *mut c_void) -> *mut c_void {
        TEST_CHANNEL.with(Cell::get)
    }
    unsafe extern "C" fn test_set_channel_actor(_channel: *mut c_void, actor: *mut c_void) {
        TEST_CHANNEL_ACTOR.with(|a| a.set(actor));
    }
    unsafe extern "C" fn test_channel_ready(_channel: *mut c_void, _saturate: i32) -> i32 {
        1
    }
    unsafe extern "C" fn test_replicate(_channel: *mut c_void) {
        let actor = TEST_CHANNEL_ACTOR.with(Cell::get);
        TEST_REPLICATED.with(|actors| actors.borrow_mut().push(actor));
    }
    unsafe extern "C" fn test_random() -> f32 {
        0.0
    }
    unsafe extern "C" fn test_checks() -> i32 {
        0
    }

    unsafe extern "C" fn test_sin(_index: i32) -> f32 {
        0.0
    }
    unsafe extern "C" fn test_cos(_index: i32) -> f32 {
        1.0
    }

    fn test_services() -> Hp2LevTicServices {
        let bytes = std::mem::size_of::<Hp2LevTicServices>();
        let slots: Vec<unsafe extern "C" fn()> = (0..bytes / std::mem::size_of::<usize>())
            .map(|_| no_service as unsafe extern "C" fn())
            .collect();
        let mut svc = unsafe { ptr::read_unaligned(slots.as_ptr().cast::<Hp2LevTicServices>()) };
        svc.conn_is_net_ready = checked_conn_is_net_ready_test_conn_ready;
        svc.conn_sent_temp_count = checked_conn_sent_temp_count_test_conn_sent_temp_count;
        svc.actor_event_player_calc_view = checked_actor_event_player_calc_view_test_player_calc_view;
        svc.level_actor_count = checked_level_actor_count_test_level_actor_count;
        svc.level_actor = checked_level_actor_test_level_actor;
        svc.level_max_tick_rate = checked_level_max_tick_rate_test_max_tick_rate;
        svc.actor_update_frequency = checked_actor_update_frequency_test_update_frequency;
        svc.actor_net_priority = checked_actor_net_priority_test_actor_net_priority;
        svc.conn_actor_channel = checked_conn_actor_channel_test_actor_channel;
        svc.level_fast_line_check = checked_level_fast_line_check_test_fast_line;
        svc.conn_package_map_class_index = checked_conn_package_map_class_index_test_class_index;
        svc.conn_create_actor_channel = checked_conn_create_actor_channel_test_create_channel;
        svc.conn_set_channel_actor = checked_conn_set_channel_actor_test_set_channel_actor;
        svc.channel_is_net_ready = checked_channel_is_net_ready_test_channel_ready;
        svc.channel_replicate_actor = checked_channel_replicate_actor_test_replicate;
        svc.level_app_frand = checked_level_app_frand_test_random;
        svc.gmath_sin_tab = checked_gmath_sin_tab_test_sin;
        svc.gmath_cos_tab = checked_gmath_cos_tab_test_cos;
        svc.level_gmem = checked_level_gmem_test_gmem;
        svc.mem_mark_pop = checked_mem_mark_pop_test_mark_pop;
        svc.mem_push = checked_mem_push_test_mem_push;
        svc.check_enabled = checked_check_enabled_test_checks;
        svc.guard_scope = fixture_guard_scope;
        svc.server_tick_client_scoped = fixture_server_tick_client_scoped;
        svc.sort_priorities = fixture_sort_priorities;
        svc
    }

    #[test]
    fn server_tick_client_replays_seeded_gmem_priority_array() {
        let mut arena = Arena([0; 512]);
        let base = arena.0.as_mut_ptr();
        for offset in (0..arena.0.len()).step_by(std::mem::size_of::<usize>()) {
            unsafe {
                base.add(offset)
                    .cast::<usize>()
                    .write_unaligned(base as usize);
            }
        }
        let mut gmem = TestMemStack {
            top: base,
            end: unsafe { base.add(arena.0.len()) },
        };
        let gmem_ptr = ptr::from_mut(&mut gmem).cast::<c_void>();
        TEST_GMEM.with(|v| v.set(gmem_ptr));

        let mut level = unsafe { std::mem::zeroed::<ULevel>() };
        let mut level_info = unsafe { std::mem::zeroed::<AActor>() };
        let mut viewer = unsafe { std::mem::zeroed::<APlayerPawn>() };
        let mut first = unsafe { std::mem::zeroed::<AActor>() };
        let mut last = unsafe { std::mem::zeroed::<AActor>() };
        let mut driver = unsafe { std::mem::zeroed::<UNetDriver>() };
        let mut channel = unsafe { std::mem::zeroed::<UActorChannel>() };
        let mut connection = unsafe { std::mem::zeroed::<UNetConnection>() };
        last.remote_role = 2;
        last.x_level = ptr::from_mut(&mut level).cast();
        driver.time = ftime_from_f32(100.0).to_ne_bytes();
        TEST_DRIVER.with(|v| v.set(ptr::from_mut(&mut driver).cast()));
        TEST_CHANNEL.with(|v| v.set(ptr::from_mut(&mut channel).cast()));
        TEST_ACTORS.with(|v| {
            *v.borrow_mut() = vec![
                ptr::from_mut(&mut level_info).cast(),
                ptr::from_mut(&mut viewer.base.base).cast(),
                ptr::from_mut(&mut first).cast(),
                ptr::from_mut(&mut last).cast(),
            ]
        });
        TEST_REPLICATED.with(|v| v.borrow_mut().clear());
        TEST_CHANNEL_ACTOR.with(|v| v.set(ptr::null_mut()));

        level.i_first_net_relevant_actor = 2;
        level.net_tag = 0;
        level.base.net_driver = TEST_DRIVER.with(Cell::get);
        first.remote_role = 2;
        first.x_level = ptr::from_mut(&mut level).cast();

        connection.state = USOCK_OPEN;
        connection.actor = TEST_ACTORS.with(|a| a.borrow()[1]);
        connection.driver = ptr::from_mut(&mut driver).cast();
        connection.last_receive_time = ftime_from_f32(100.0).to_ne_bytes();
        connection.last_rep_time = ftime_from_f32(100.0).to_ne_bytes();
        connection.tick_count = -1;
        let connection = ptr::from_mut(&mut connection).cast::<c_void>();
        let svc = test_services();
        unsafe {
            let updated =
                server_tick_client(&svc, ptr::from_mut(&mut level).cast(), connection).unwrap();
            assert_eq!(updated, 2);
        }
        let replicated = TEST_REPLICATED.with(|v| v.borrow().clone());
        assert_eq!(
            replicated,
            vec![
                ptr::from_mut(&mut first).cast(),
                ptr::from_mut(&mut first).cast(),
            ]
        );
        assert_eq!(
            gmem.top, base,
            "FMemMark restores the temporary GMem arrays"
        );
    }

    /// Ops-first dispatch (`local://m1-dispatch-design.md` §3.2): a class
    /// with a Rust-registered `update_frequency` override -- e.g. Carcass,
    /// registered by `hp2_rs_engine_actor_register_carcass_ops` -- must be
    /// called directly, bypassing `ops.actor_update_frequency` entirely.
    /// `test_services()` wires that svc call to always return `60.0`
    /// (`test_update_frequency`); the override returns `17.0`.
    #[test]
    fn ops_first_dispatch_calls_the_registered_override_for_the_actors_own_class() {
        // Fixed sentinel, not a stack/heap address: a per-test local variable's
        // address can be reused by an unrelated allocation in another test
        // running in this same process once this test's frame/allocation is
        // gone, silently resurrecting a stale `ACTOR_OPS_TABLE` entry under a
        // different class. A literal sentinel is never freed or reused.
        let class = 0x9000_usize as *mut c_void;
        unsafe {
            hp2_core::class_ops::register_actor_ops(
                class,
                ptr::null_mut(),
                hp2_core::class_ops::ActorOps {
                    update_frequency: Some(probe_update_frequency),
                },
            );
        }
        let mut actor = unsafe { std::mem::zeroed::<AActor>() };
        unsafe {
            (*ptr::from_mut(&mut actor).cast::<UObject>()).class = class;
        }
        let svc = test_services();
        let mut view_dir = [0.0_f32; 3];
        let mut view_pos = [0.0_f32; 3];
        let freq = unsafe {
            actor_update_frequency_with(&NativeNetTickOps(&svc), ptr::from_mut(&mut actor).cast(),
            ptr::null_mut(),
            view_dir.as_mut_ptr(),
            view_pos.as_mut_ptr(),)
        }
        .unwrap();
        assert_eq!(
            freq, 17.0,
            "a registered ops override must answer directly instead of ops.actor_update_frequency's 60.0"
        );
    }

    /// A script subclass of a Rust-registered native class (e.g. a map's
    /// Carcass subclass) must inherit the ops override too. `bind()`
    /// (`property_link.rs`) calls `class_ops::inherit_actor_ops` for every
    /// script class exactly where it already inherits `ClassConstructor`
    /// from the nearest bound ancestor; this test drives that same
    /// mechanism directly rather than through the whole `UClass::Bind` port.
    #[test]
    fn ops_first_dispatch_applies_to_a_script_subclass_via_inherited_record() {
        // Fixed sentinels (see the sibling test above), distinct from each
        // other and from that test's 0x9000.
        let native_class = 0x9010_usize as *mut c_void;
        let script_class = 0x9011_usize as *mut c_void;
        unsafe {
            hp2_core::class_ops::register_actor_ops(
                native_class,
                ptr::null_mut(),
                hp2_core::class_ops::ActorOps {
                    update_frequency: Some(probe_update_frequency),
                },
            );
            hp2_core::class_ops::inherit_actor_ops(script_class, native_class);
        }
        let mut actor = unsafe { std::mem::zeroed::<AActor>() };
        unsafe {
            (*ptr::from_mut(&mut actor).cast::<UObject>()).class = script_class;
        }
        let svc = test_services();
        let mut view_dir = [0.0_f32; 3];
        let mut view_pos = [0.0_f32; 3];
        let freq = unsafe {
            actor_update_frequency_with(&NativeNetTickOps(&svc), ptr::from_mut(&mut actor).cast(),
            ptr::null_mut(),
            view_dir.as_mut_ptr(),
            view_pos.as_mut_ptr(),)
        }
        .unwrap();
        assert_eq!(
            freq, 17.0,
            "a script subclass must inherit its native ancestor's ops override, not fall back to ops.actor_update_frequency's 60.0"
        );
    }

    // Checked adapters for the existing deterministic fixture callbacks.
    unsafe extern "C" fn checked_conn_is_net_ready_test_conn_ready(
        arg0: *mut c_void,
        arg1: i32,
        out: *mut i32,
    ) -> i32 {
        unsafe {
            out.write(test_conn_ready(arg0, arg1));
        }
        0
    }
    unsafe extern "C" fn checked_conn_sent_temp_count_test_conn_sent_temp_count(
        arg0: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        unsafe {
            out.write(test_conn_sent_temp_count(arg0));
        }
        0
    }
    unsafe extern "C" fn checked_actor_event_player_calc_view_test_player_calc_view(
        arg0: *mut c_void,
        arg1: *mut f32,
        arg2: *mut i32,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe {
            out.write(test_player_calc_view(arg0, arg1, arg2));
        }
        0
    }
    unsafe extern "C" fn checked_level_actor_count_test_level_actor_count(
        arg0: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        unsafe {
            out.write(test_level_actor_count(arg0));
        }
        0
    }
    unsafe extern "C" fn checked_level_actor_test_level_actor(
        arg0: *mut c_void,
        arg1: i32,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe {
            out.write(test_level_actor(arg0, arg1));
        }
        0
    }
    unsafe extern "C" fn checked_level_max_tick_rate_test_max_tick_rate(
        arg0: *mut c_void,
        out: *mut f32,
    ) -> i32 {
        unsafe {
            out.write(test_max_tick_rate(arg0));
        }
        0
    }
    unsafe extern "C" fn checked_actor_update_frequency_test_update_frequency(
        arg0: *mut c_void,
        arg1: *mut c_void,
        arg2: *mut f32,
        arg3: *mut f32,
        out: *mut f32,
    ) -> i32 {
        unsafe {
            out.write(test_update_frequency(arg0, arg1, arg2, arg3));
        }
        0
    }
    unsafe extern "C" fn checked_actor_net_priority_test_actor_net_priority(
        arg0: *mut c_void,
        arg1: *mut c_void,
        arg2: f32,
        arg3: f32,
        out: *mut f32,
    ) -> i32 {
        unsafe {
            out.write(test_actor_net_priority(arg0, arg1, arg2, arg3));
        }
        0
    }
    unsafe extern "C" fn checked_conn_actor_channel_test_actor_channel(
        arg0: *mut c_void,
        arg1: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe {
            out.write(test_actor_channel(arg0, arg1));
        }
        0
    }
    unsafe extern "C" fn checked_level_fast_line_check_test_fast_line(
        arg0: *mut c_void,
        arg1: *const f32,
        arg2: *const f32,
        out: *mut i32,
    ) -> i32 {
        unsafe {
            out.write(test_fast_line(arg0, arg1, arg2));
        }
        0
    }
    unsafe extern "C" fn checked_conn_package_map_class_index_test_class_index(
        arg0: *mut c_void,
        arg1: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        unsafe {
            out.write(test_class_index(arg0, arg1));
        }
        0
    }
    unsafe extern "C" fn checked_conn_create_actor_channel_test_create_channel(
        arg0: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe {
            out.write(test_create_channel(arg0));
        }
        0
    }
    unsafe extern "C" fn checked_conn_set_channel_actor_test_set_channel_actor(
        arg0: *mut c_void,
        arg1: *mut c_void,
    ) -> i32 {
        unsafe {
            test_set_channel_actor(arg0, arg1);
        }
        0
    }
    unsafe extern "C" fn checked_channel_is_net_ready_test_channel_ready(
        arg0: *mut c_void,
        arg1: i32,
        out: *mut i32,
    ) -> i32 {
        unsafe {
            out.write(test_channel_ready(arg0, arg1));
        }
        0
    }
    unsafe extern "C" fn checked_channel_replicate_actor_test_replicate(arg0: *mut c_void) -> i32 {
        unsafe {
            test_replicate(arg0);
        }
        0
    }
    unsafe extern "C" fn checked_level_app_frand_test_random(out: *mut f32) -> i32 {
        unsafe {
            out.write(test_random());
        }
        0
    }
    unsafe extern "C" fn checked_gmath_sin_tab_test_sin(arg0: i32, out: *mut f32) -> i32 {
        unsafe {
            out.write(test_sin(arg0));
        }
        0
    }
    unsafe extern "C" fn checked_gmath_cos_tab_test_cos(arg0: i32, out: *mut f32) -> i32 {
        unsafe {
            out.write(test_cos(arg0));
        }
        0
    }
    unsafe extern "C" fn checked_level_gmem_test_gmem(out: *mut *mut c_void) -> i32 {
        unsafe {
            out.write(test_gmem());
        }
        0
    }
    unsafe extern "C" fn checked_mem_mark_pop_test_mark_pop(arg0: *mut c_void) -> i32 {
        unsafe {
            test_mark_pop(arg0);
        }
        0
    }
    unsafe extern "C" fn checked_mem_push_test_mem_push(
        arg0: *mut c_void,
        arg1: u64,
        arg2: i32,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe {
            out.write(test_mem_push(arg0, arg1, arg2));
        }
        0
    }
    unsafe extern "C" fn checked_check_enabled_test_checks(out: *mut i32) -> i32 {
        unsafe {
            out.write(test_checks());
        }
        0
    }
    unsafe extern "C" fn fixture_guard_scope(
        _scope: i32,
        context: *mut c_void,
        continuation: unsafe extern "C" fn(*mut c_void) -> i32,
    ) -> i32 {
        unsafe { continuation(context) }
    }
    unsafe extern "C" fn fixture_server_tick_client_scoped(
        level: *mut c_void,
        connection: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let svc = test_services();
        let mut mark = [0usize; 3];
        unsafe {
            test_mark_save(test_gmem(), mark.as_mut_ptr().cast());
            match crate::lev_tic_net::server_tick_client_body(
                &svc,
                level,
                connection,
                mark.as_mut_ptr().cast(),
            ) {
                Ok(value) => {
                    out.write(value);
                    0
                }
                Err(status) => status,
            }
        }
    }
    unsafe extern "C" fn fixture_sort_priorities(
        actors: *mut *mut crate::lev_tic::ActorPriority,
        count: i32,
    ) -> i32 {
        unsafe {
            crate::lev_tic::sort_priority_ptrs(std::slice::from_raw_parts_mut(
                actors,
                count as usize,
            ));
        }
        0
    }
}
