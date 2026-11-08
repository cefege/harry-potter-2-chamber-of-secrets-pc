//! Port of `ULevel::TickDemoRecord` and `ULevel::TickDemoPlayback`
//! (`HarryPotter2/Unreal/Engine/Src/UnLevTic.cpp:967-1033`).
//!
//! Public native wrappers retain the original guard/unguard boundary.
//! Checked engine calls stop immediately on failure, retaining partial role and
//! demo-recording mutations. Engine calls follow the original order. Plain
//! and bitfield reads and writes use the generated `ULevel`, `ALevelInfo`, and
//! `AActor` mirrors. No float math happens here, so no fused or libm sites.

use std::ffi::c_void;

use hp2_abi::engine::generated::{AActor, ALevelInfo, ULevel};

use crate::lev_tic::{Hp2LevTicServices, check};
use crate::prim::INDEX_NONE;

/// `ROLE_None` (`EngineClasses.h`).
const ROLE_NONE: u8 = 0;
/// `ROLE_Authority` (`EngineClasses.h`).
const ROLE_AUTHORITY: u8 = 4;
/// `NM_Client` (`EngineClasses.h`).
const NM_CLIENT: u8 = 3;
/// `USOCK_Closed` (`UnConn.h`).
const USOCK_CLOSED: i32 = 1;
/// `USOCK_Pending` (`UnConn.h`).
const USOCK_PENDING: i32 = 2;
/// `LEVACT_None` (`EngineClasses.h`).
const LEVACT_NONE: u8 = 0;
/// `LEVACT_Connecting` (`EngineClasses.h`).
const LEVACT_CONNECTING: u8 = 3;
/// `HP2_LEVTIC_KIND_AZONEINFO` (`Hp2LevTicServices.h`).
const KIND_AZONEINFO: i32 = 2;
/// `HP2_LEVTIC_TRAVEL_ENTRY` (`Hp2LevTicServices.h`): the `"?entry"` url.
const TRAVEL_ENTRY: i32 = 1;

pub(crate) trait LevelTickDemoOps {
    type Error;

    unsafe fn driver_client_connection(
        &mut self,
        driver: *mut c_void,
        index: i32,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn level_actor_count(&mut self, level: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_actor(
        &mut self,
        level: *mut c_void,
        index: i32,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn level_info(&mut self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_is_zone_info(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn sent_temp_index(
        &mut self,
        connection: *mut c_void,
        actor: *mut c_void,
    ) -> Result<i32, Self::Error>;
    unsafe fn class_default_is_static(
        &mut self,
        actor: *mut c_void,
    ) -> Result<bool, Self::Error>;
    unsafe fn actor_channel(
        &mut self,
        connection: *mut c_void,
        actor: *mut c_void,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn package_map_class_index(
        &mut self,
        connection: *mut c_void,
        actor: *mut c_void,
    ) -> Result<i32, Self::Error>;
    unsafe fn create_actor_channel(
        &mut self,
        connection: *mut c_void,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn set_channel_actor(
        &mut self,
        channel: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), Self::Error>;
    unsafe fn channel_closing(&mut self, channel: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn channel_is_net_ready(
        &mut self,
        channel: *mut c_void,
        saturate: i32,
    ) -> Result<i32, Self::Error>;
    unsafe fn replicate_actor(&mut self, channel: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn connection_state(&mut self, connection: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn set_progress_empty(&mut self, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn client_viewports_num(&mut self, level: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn client_travel_entry(&mut self, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn check(
        &mut self,
        condition: bool,
        expression: &std::ffi::CStr,
        line: i32,
    ) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn tick_demo_record_with<O: LevelTickDemoOps>(
    ops: &mut O,
    level: *mut c_void,
) -> Result<i32, O::Error> {
    unsafe {
        let lv = level.cast::<ULevel>();
        let driver = (*lv).base.demo_rec_driver;
        // All replicatable actors are assumed to be relevant for demo recording.
        let connection = ops.driver_client_connection(driver, 0)?;
        let mut i: i32 = 0;
        while i < ops.level_actor_count(level)? {
            let actor_v = ops.level_actor(level, i)?;
            let is_net_client =
                (*ops.level_info(level)?.cast::<ALevelInfo>()).net_mode == NM_CLIENT;
            if !actor_v.is_null() {
                let actor = actor_v.cast::<AActor>();
                if ((*actor).remote_role != ROLE_NONE
                    || (is_net_client
                        && (*actor).role != ROLE_NONE
                        && (*actor).role != ROLE_AUTHORITY))
                    && (i >= (*lv).i_first_dynamic_actor
                        || ops.actor_is_zone_info(actor_v)?)
                    && (!(*actor).b_net_temporary()
                        || ops.sent_temp_index(connection, actor_v)? == INDEX_NONE)
                    && ((*actor).b_static() || !ops.class_default_is_static(actor_v)?)
                {
                    let mut channel = ops.actor_channel(connection, actor_v)?;
                    if channel.is_null()
                        && ops.package_map_class_index(connection, actor_v)? != INDEX_NONE
                    {
                        channel = ops.create_actor_channel(connection)?;
                        ops.check(!channel.is_null(), c"Channel", 990)?;
                        ops.set_channel_actor(channel, actor_v)?;
                    }
                    if !channel.is_null() {
                        let closing = ops.channel_closing(channel)?;
                        ops.check(closing == 0, c"!Channel->Closing", 996)?;
                        if ops.channel_is_net_ready(channel, 0)? != 0 {
                            (*actor).set_b_demo_recording(true);
                            (*actor).set_b_client_demo_recording(is_net_client);
                            if is_net_client {
                                exchange_roles(actor);
                            }
                            ops.replicate_actor(channel)?;
                            if is_net_client {
                                exchange_roles(actor);
                            }
                            (*actor).set_b_demo_recording(false);
                            (*actor).set_b_client_demo_recording(false);
                        }
                    }
                }
            }
            i += 1;
        }
        Ok(1)
    }
}

pub(crate) unsafe fn tick_demo_playback_with<O: LevelTickDemoOps>(
    ops: &mut O,
    level: *mut c_void,
) -> Result<i32, O::Error> {
    unsafe {
        let lv = level.cast::<ULevel>();
        let info = ops.level_info(level)?.cast::<ALevelInfo>();
        let driver = (*lv).base.demo_rec_driver;
        let server_connection = ops.driver_server_connection(driver)?;
        if (*info).level_action == LEVACT_CONNECTING
            && ops.connection_state(server_connection)? != USOCK_PENDING
        {
            (*info).level_action = LEVACT_NONE;
            ops.set_progress_empty(level)?;
        }
        let server_connection = ops.driver_server_connection(driver)?;
        if ops.connection_state(server_connection)? == USOCK_CLOSED {
            let viewports_num = ops.client_viewports_num(level)?;
            ops.check(
                viewports_num != 0,
                c"Engine->Client->Viewports.Num()",
                1028,
            )?;
            ops.client_travel_entry(level)?;
        }
        Ok(1)
    }
}


struct NativeLevelTickDemoOps<'a> {
    svc: &'a Hp2LevTicServices,
}

impl LevelTickDemoOps for NativeLevelTickDemoOps<'_> {
    type Error = i32;

    unsafe fn driver_client_connection(
        &mut self,
        driver: *mut c_void,
        index: i32,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.driver_client_connection(driver, index) }
    }
    unsafe fn level_actor_count(&mut self, level: *mut c_void) -> Result<i32, i32> {
        unsafe { self.svc.level_actor_count(level) }
    }
    unsafe fn level_actor(&mut self, level: *mut c_void, index: i32) -> Result<*mut c_void, i32> {
        unsafe { self.svc.level_actor(level, index) }
    }
    unsafe fn level_info(&mut self, level: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { self.svc.level_info(level) }
    }
    unsafe fn actor_is_zone_info(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        unsafe { self.svc.actor_is_a(actor, KIND_AZONEINFO) }.map(|value| value != 0)
    }
    unsafe fn sent_temp_index(
        &mut self,
        connection: *mut c_void,
        actor: *mut c_void,
    ) -> Result<i32, i32> {
        unsafe { self.svc.conn_sent_temp_index(connection, actor) }
    }
    unsafe fn class_default_is_static(&mut self, actor: *mut c_void) -> Result<bool, i32> {
        unsafe { self.svc.actor_class_default_is_static(actor) }.map(|value| value != 0)
    }
    unsafe fn actor_channel(
        &mut self,
        connection: *mut c_void,
        actor: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.conn_actor_channel(connection, actor) }
    }
    unsafe fn package_map_class_index(
        &mut self,
        connection: *mut c_void,
        actor: *mut c_void,
    ) -> Result<i32, i32> {
        unsafe { self.svc.conn_package_map_class_index(connection, actor) }
    }
    unsafe fn create_actor_channel(
        &mut self,
        connection: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.conn_create_actor_channel(connection) }
    }
    unsafe fn set_channel_actor(
        &mut self,
        channel: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        unsafe { self.svc.conn_set_channel_actor(channel, actor) }
    }
    unsafe fn channel_closing(&mut self, channel: *mut c_void) -> Result<i32, i32> {
        unsafe { self.svc.channel_closing(channel) }
    }
    unsafe fn channel_is_net_ready(
        &mut self,
        channel: *mut c_void,
        saturate: i32,
    ) -> Result<i32, i32> {
        unsafe { self.svc.channel_is_net_ready(channel, saturate) }
    }
    unsafe fn replicate_actor(&mut self, channel: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.channel_replicate_actor(channel) }
    }
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> Result<*mut c_void, i32> {
        unsafe { self.svc.driver_server_connection(driver) }
    }
    unsafe fn connection_state(&mut self, connection: *mut c_void) -> Result<i32, i32> {
        unsafe { self.svc.conn_state(connection) }
    }
    unsafe fn set_progress_empty(&mut self, level: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.level_engine_set_progress_empty(level) }
    }
    unsafe fn client_viewports_num(&mut self, level: *mut c_void) -> Result<i32, i32> {
        unsafe { self.svc.level_client_viewports_num(level) }
    }
    unsafe fn client_travel_entry(&mut self, level: *mut c_void) -> Result<(), i32> {
        unsafe { self.svc.level_engine_client_travel(level, TRAVEL_ENTRY) }
    }
    unsafe fn check(
        &mut self,
        condition: bool,
        expression: &std::ffi::CStr,
        line: i32,
    ) -> Result<(), i32> {
        unsafe { check(self.svc, condition, expression, line) }
    }
}

/// `Exchange(Actor->RemoteRole, Actor->Role)`.
unsafe fn exchange_roles(actor: *mut AActor) {
    unsafe {
        std::mem::swap(&mut (*actor).remote_role, &mut (*actor).role);
    }
}

/// `ULevel::TickDemoRecord(FLOAT DeltaSeconds)`. Returns `1`.
///
/// `DeltaSeconds` is unused by the original body.
///
/// # Safety
/// `level` is a live `ULevel*` whose `demo_rec_driver` is a live driver with a
/// client connection; every non-null actor in the level list is live.
pub unsafe fn tick_demo_record(
    svc: &Hp2LevTicServices,
    level: *mut c_void,
    _delta_seconds: f32,
) -> Result<i32, i32> {
    unsafe { tick_demo_record_with(&mut NativeLevelTickDemoOps { svc }, level) }
}

/// `ULevel::TickDemoPlayback(FLOAT DeltaSeconds)`. Returns `1`.
///
/// `DeltaSeconds` is unused by the original body.
///
/// # Safety
/// `level` is a live `ULevel*` whose `demo_rec_driver` has a live server
/// connection, matching the unchecked Native dereferences.
pub unsafe fn tick_demo_playback(
    svc: &Hp2LevTicServices,
    level: *mut c_void,
    _delta_seconds: f32,
) -> Result<i32, i32> {
    unsafe { tick_demo_playback_with(&mut NativeLevelTickDemoOps { svc }, level) }
}
