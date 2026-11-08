//! Canonical Core host for the source-shared `ULevel::TickDemoRecord` and
//! `ULevel::TickDemoPlayback` policy.
//!
//! The policy, its branch order and its source checks live in
//! [`crate::lev_tic_demo`]. This module supplies the operations against
//! Core-owned level, actor, connection and channel records; no native
//! service table is consulted.

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, UActorChannel, UChannel, UClient, UGameEngine, ULevel, UNetConnection,
    UNetDriver, UObject, UViewport,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::CStr, ptr};

/// `TRAVEL_Absolute` (`Engine/Inc/UnURL.h`).
const TRAVEL_ABSOLUTE: i32 = 0;
/// `INDEX_NONE`.
const INDEX_NONE: i32 = -1;
/// `"?entry"` (`HP2_LEVTIC_TRAVEL_ENTRY`), NUL terminated UTF-32.
const TRAVEL_ENTRY: [u32; 7] = [63, 101, 110, 116, 114, 121, 0];

fn failed_check(expression: &CStr, line: i32) -> RuntimeError {
    RuntimeError::new(&expression.to_string_lossy())
        .with_context(&format!("UnLevTic.cpp:{line}"))
}

struct CoreLevelTickDemoOps<'a> {
    access: CoreAccess<'a>,
}

impl CoreLevelTickDemoOps<'_> {
    fn level_engine(level: *mut ULevel) -> RuntimeResult<*mut UGameEngine> {
        if level.is_null() {
            return Err(RuntimeError::new("Null canonical ULevel in demo tick"));
        }
        Ok(unsafe { ptr::addr_of!((*level).base.engine).read_unaligned() }.cast::<UGameEngine>())
    }
    fn client_viewport0(level: *mut ULevel) -> RuntimeResult<*mut UViewport> {
        let engine = Self::level_engine(level)?;
        if engine.is_null() {
            return Err(RuntimeError::new("Demo tick level has no canonical Engine"));
        }
        let client = unsafe { ptr::addr_of!((*engine).client).read_unaligned() }.cast::<UClient>();
        if client.is_null() {
            return Err(RuntimeError::new("Demo tick Engine has no canonical Client"));
        }
        let viewports = unsafe { ptr::addr_of!((*client).viewports).read_unaligned() };
        if viewports.num <= 0 {
            return Ok(ptr::null_mut());
        }
        if viewports.data.is_null() || viewports.max < viewports.num {
            return Err(RuntimeError::new("Invalid canonical Client viewport array"));
        }
        Ok(unsafe { viewports.data.cast::<*mut UViewport>().read_unaligned() })
    }
}

impl crate::lev_tic_demo::LevelTickDemoOps for CoreLevelTickDemoOps<'_> {
    type Error = RuntimeError;

    unsafe fn driver_client_connection(
        &mut self,
        driver: *mut std::ffi::c_void,
        index: i32,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        if driver.is_null() {
            return Err(RuntimeError::new("TickDemoRecord has no demo driver"));
        }
        let connections = unsafe {
            ptr::addr_of!((*driver.cast::<UNetDriver>()).client_connections).read_unaligned()
        };
        if index < 0 || index >= connections.num {
            // `DemoRecDriver->ClientConnections(0)` dereferences an absent slot in
            // the original; reject it instead of reading past the array.
            return Err(RuntimeError::new("TickDemoRecord has no demo client connection"));
        }
        if connections.data.is_null() || connections.max < connections.num {
            return Err(RuntimeError::new("Invalid demo driver client-connection array"));
        }
        Ok(unsafe {
            connections
                .data
                .cast::<*mut UNetConnection>()
                .add(index as usize)
                .read_unaligned()
                .cast()
        })
    }
    unsafe fn level_actor_count(&mut self, level: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        unsafe { crate::level_owner::actor_count(level.cast::<ULevel>()) }
    }
    unsafe fn level_actor(
        &mut self,
        level: *mut std::ffi::c_void,
        index: i32,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        let count = unsafe { crate::level_owner::actor_count(level.cast::<ULevel>())? };
        if index < 0 || index >= count {
            return Err(RuntimeError::new("Demo tick actor index is out of range"));
        }
        Ok(unsafe { crate::level_owner::actor_at(level.cast::<ULevel>(), index).cast() })
    }
    unsafe fn level_info(
        &mut self,
        level: *mut std::ffi::c_void,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { crate::package_map_level_owner::level_info(level.cast::<ULevel>())? }
            .cast::<ALevelInfo>()
            .cast())
    }
    unsafe fn actor_is_zone_info(
        &mut self,
        actor: *mut std::ffi::c_void,
    ) -> RuntimeResult<bool> {
        let class = self.access.native_class("Engine.ZoneInfo")?;
        Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
    }
    unsafe fn sent_temp_index(
        &mut self,
        connection: *mut std::ffi::c_void,
        actor: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        if connection.is_null() {
            return Err(RuntimeError::new("Null canonical demo connection"));
        }
        let temporaries = unsafe {
            ptr::addr_of!((*connection.cast::<UNetConnection>()).sent_temporaries).read_unaligned()
        };
        if temporaries.num < 0 || temporaries.max < temporaries.num
            || (temporaries.num > 0 && temporaries.data.is_null())
        {
            return Err(RuntimeError::new("Invalid canonical SentTemporaries"));
        }
        let entries = temporaries.data.cast::<*mut AActor>();
        let mut index = 0;
        while index < temporaries.num {
            if unsafe { entries.add(index as usize).read_unaligned() }.cast() == actor {
                return Ok(index);
            }
            index += 1;
        }
        Ok(INDEX_NONE)
    }
    unsafe fn class_default_is_static(
        &mut self,
        actor: *mut std::ffi::c_void,
    ) -> RuntimeResult<bool> {
        if actor.is_null() {
            return Err(RuntimeError::new("Null canonical demo actor"));
        }
        let class = unsafe { ptr::addr_of!((*actor.cast::<UObject>()).class).read_unaligned() };
        let defaults = unsafe { self.access.default_object(class.cast::<hp2_abi::engine::generated::UClass>())? }.cast::<AActor>();
        Ok(unsafe { (*defaults).b_static() })
    }
    unsafe fn actor_channel(
        &mut self,
        connection: *mut std::ffi::c_void,
        actor: *mut std::ffi::c_void,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        let channel = unsafe {
            crate::network_owner::actor_channel(
                connection.cast::<UNetConnection>(),
                actor.cast::<AActor>(),
            )?
        };
        Ok(channel.cast())
    }
    unsafe fn package_map_class_index(
        &mut self,
        connection: *mut std::ffi::c_void,
        actor: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        if connection.is_null() || actor.is_null() {
            return Err(RuntimeError::new("Null canonical demo package-map argument"));
        }
        let package_map = unsafe {
            ptr::addr_of!((*connection.cast::<UNetConnection>()).package_map).read_unaligned()
        };
        let class = unsafe { ptr::addr_of!((*actor.cast::<UObject>()).class).read_unaligned() };
        unsafe { self.access.package_map_object_to_index(package_map.cast::<hp2_abi::engine::generated::UPackageMap>(), class.cast::<hp2_abi::engine::generated::UObject>()) }
    }
    unsafe fn create_actor_channel(
        &mut self,
        connection: *mut std::ffi::c_void,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        let channel = unsafe {
            crate::connection_owner::create_channel(
                self.access,
                connection.cast::<UNetConnection>(),
                crate::level_net::CHTYPE_ACTOR,
                1,
                INDEX_NONE,
            )?
        };
        Ok(channel.cast())
    }
    unsafe fn set_channel_actor(
        &mut self,
        channel: *mut std::ffi::c_void,
        actor: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::actor_channel_owner::set_actor(
                self.access,
                channel.cast::<UActorChannel>(),
                actor.cast::<AActor>(),
            )
        }
    }
    unsafe fn channel_closing(&mut self, channel: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        if channel.is_null() {
            return Err(RuntimeError::new("Null canonical demo actor channel"));
        }
        Ok(unsafe {
            ptr::addr_of!((*channel.cast::<UChannel>()).closing).read_unaligned()
        })
    }
    unsafe fn channel_is_net_ready(
        &mut self,
        channel: *mut std::ffi::c_void,
        saturate: i32,
    ) -> RuntimeResult<i32> {
        unsafe {
            crate::actor_channel_owner::is_net_ready(
                self.access,
                channel.cast::<UActorChannel>(),
                saturate,
            )
        }
    }
    unsafe fn replicate_actor(&mut self, channel: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe {
            crate::actor_channel_owner::replicate_actor(
                self.access,
                channel.cast::<UActorChannel>(),
            )
        }
    }
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut std::ffi::c_void,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        if driver.is_null() {
            return Err(RuntimeError::new("TickDemoPlayback has no demo driver"));
        }
        Ok(unsafe {
            ptr::addr_of!((*driver.cast::<UNetDriver>()).server_connection).read_unaligned()
        })
    }
    unsafe fn connection_state(
        &mut self,
        connection: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        if connection.is_null() {
            return Err(RuntimeError::new("TickDemoPlayback has no demo server connection"));
        }
        Ok(unsafe {
            ptr::addr_of!((*connection.cast::<UNetConnection>()).state).read_unaligned()
        })
    }
    unsafe fn set_progress_empty(&mut self, level: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let engine = Self::level_engine(level.cast::<ULevel>())?;
        let empty = [0u32];
        unsafe {
            crate::game_engine_owner::set_progress(
                self.access,
                engine,
                empty.as_ptr(),
                empty.as_ptr(),
                0.0,
            )
        }
    }
    unsafe fn client_viewports_num(
        &mut self,
        level: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        let engine = Self::level_engine(level.cast::<ULevel>())?;
        if engine.is_null() {
            return Err(RuntimeError::new("Demo tick level has no canonical Engine"));
        }
        let client = unsafe { ptr::addr_of!((*engine).client).read_unaligned() }.cast::<UClient>();
        if client.is_null() {
            return Err(RuntimeError::new("Demo tick Engine has no canonical Client"));
        }
        Ok(unsafe { ptr::addr_of!((*client).viewports).read_unaligned() }.num)
    }
    unsafe fn client_travel_entry(&mut self, level: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let viewport = Self::client_viewport0(level.cast::<ULevel>())?;
        if viewport.is_null() {
            return Err(RuntimeError::new("Demo tick Engine->Client->Viewports has no entry"));
        }
        unsafe {
            self.access
                .assign_fstring(ptr::addr_of_mut!((*viewport).travel_url), &TRAVEL_ENTRY)?;
            (*viewport).set_b_travel_items(false);
            (*viewport).travel_type = TRAVEL_ABSOLUTE;
        }
        Ok(())
    }
    unsafe fn check(
        &mut self,
        condition: bool,
        expression: &CStr,
        line: i32,
    ) -> RuntimeResult<()> {
        if condition {
            Ok(())
        } else {
            Err(failed_check(expression, line))
        }
    }
}

/// Executes the canonical Core implementation of `ULevel::TickDemoRecord`.
///
/// # Safety
/// `level`, its demo driver and every actor and channel reached from it are live
/// canonical records owned by `access` for the synchronous call.
pub(crate) unsafe fn tick_demo_record(
    access: CoreAccess<'_>,
    level: *mut ULevel,
) -> RuntimeResult<i32> {
    if level.is_null() {
        return Err(RuntimeError::new("Null canonical ULevel in TickDemoRecord"));
    }
    let mut ops = CoreLevelTickDemoOps { access };
    unsafe { crate::lev_tic_demo::tick_demo_record_with(&mut ops, level.cast()) }
}

/// Executes the canonical Core implementation of `ULevel::TickDemoPlayback`.
///
/// # Safety
/// `level`, its demo driver and the engine/client/viewport records reached from
/// it are live canonical records owned by `access` for the synchronous call.
pub(crate) unsafe fn tick_demo_playback(
    access: CoreAccess<'_>,
    level: *mut ULevel,
) -> RuntimeResult<i32> {
    if level.is_null() {
        return Err(RuntimeError::new("Null canonical ULevel in TickDemoPlayback"));
    }
    let mut ops = CoreLevelTickDemoOps { access };
    unsafe { crate::lev_tic_demo::tick_demo_playback_with(&mut ops, level.cast()) }
}
