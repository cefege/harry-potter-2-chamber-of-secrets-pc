//! Canonical Core backend for the shared `UnLevTic.cpp` network tick policy
//! (`ULevel::TickNetClient`, `ULevel::ServerTickClient`, `ULevel::TickNetServer`).
//!
//! All ordering, stored-procedure calls and scratch-mark lifetimes stay in
//! [`crate::lev_tic_net`]; this module supplies the operations against Core-owned
//! level, actor, connection and channel records.

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APlayerPawn, UActorChannel, UChannel, UClient, UGameEngine, ULevel,
    UNetConnection, UNetDriver, UObject, UViewport, FActorChannelMapPair, FCheckResult,
};
use hp2_abi::render_span::FMemMark;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::{c_void, CStr}, ptr};

use crate::lev_tic::{ActorPriority, sort_priority_ptrs};
use crate::lev_tic_net::LevelTickNetOps;

const NET_TICK_CYCLES: i32 = 1;
const USOCK_PENDING: i32 = 2;
const USOCK_OPEN: i32 = 3;
const USOCK_CLOSED: i32 = 1;
const INDEX_NONE: i32 = -1;
const KIND_APAWN: i32 = 0;
const KIND_APLAYERPAWN: i32 = 1;
const KIND_AZONEINFO: i32 = 2;
const KIND_ACAMERA: i32 = 3;
const KIND_AWEAPON: i32 = 4;
const TRAVEL_ENTRY: [u32; 7] = [63, 101, 110, 116, 114, 121, 0];
const TRAVEL_FAILED: [u32; 8] = [63, 102, 97, 105, 108, 101, 100, 0];

struct CoreActorChannelIterator {
    pairs: *const FActorChannelMapPair,
    index: i32,
    count: i32,
}

pub(crate) struct CoreLevelTickNetOps<'a> {
    access: CoreAccess<'a>,
    mark: std::cell::RefCell<Option<FMemMark>>,
}

impl CoreLevelTickNetOps<'_> {
    fn level_engine(level: *mut ULevel) -> RuntimeResult<*mut UGameEngine> {
        if level.is_null() {
            return Err(RuntimeError::new("Net tick level is null"));
        }
        Ok(unsafe { ptr::addr_of!((*level).base.engine).read_unaligned() }.cast())
    }
    fn viewport0(level: *mut ULevel) -> RuntimeResult<*mut UViewport> {
        let engine = Self::level_engine(level)?;
        if engine.is_null() {
            return Err(RuntimeError::new("Net tick level has no canonical Engine"));
        }
        let client = unsafe { ptr::addr_of!((*engine).client).read_unaligned() }.cast::<UClient>();
        if client.is_null() {
            return Err(RuntimeError::new("Net tick Engine has no canonical Client"));
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
    fn class_is(&self, object: *mut c_void, class_name: &str) -> RuntimeResult<bool> {
        let class = self.access.native_class(class_name)?;
        Ok(unsafe { hp2_core::object_model::is_a(object, class.cast()) })
    }
    fn clock_field(level: *mut ULevel, which: i32, unclock: bool) {
        let cycles = if unclock {
            hp2_core::unix_clock::cycles().wrapping_sub(34) as i32
        } else {
            hp2_core::unix_clock::cycles() as i32
        };
        let slot = if which == NET_TICK_CYCLES {
            unsafe { ptr::addr_of_mut!((*level).net_tick_cycles) }
        } else {
            unsafe { ptr::addr_of_mut!((*level).actor_tick_cycles) }
        };
        let value = unsafe { slot.read_unaligned() };
        unsafe {
            slot.write_unaligned(if unclock { value.wrapping_add(cycles) } else { value.wrapping_sub(cycles) });
        }
    }
    fn fstring_units(value: &hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<Vec<u32>> {
        if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
            return Err(RuntimeError::new("Invalid canonical FString"));
        }
        let units = if value.num == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) }.to_vec()
        };
        Ok(units)
    }
}

impl LevelTickNetOps for CoreLevelTickNetOps<'_> {
    type Error = RuntimeError;

    unsafe fn guard<T, F>(&self, _scope: i32, f: F) -> RuntimeResult<T>
    where
        F: FnOnce(&Self) -> RuntimeResult<T>,
    {
        f(self)
    }
    unsafe fn check(&self, ok: bool, expression: &CStr, line: i32) -> RuntimeResult<()> {
        if ok {
            Ok(())
        } else {
            Err(RuntimeError::new(&expression.to_string_lossy())
                .with_context(&format!("UnLevTic.cpp:{line}")))
        }
    }
    unsafe fn replicate_player_channels(&self, connection: *mut c_void) -> RuntimeResult<()> {
        let connection = connection.cast::<UNetConnection>();
        if connection.is_null() {
            return Err(RuntimeError::new("ReplicatePlayerChannels connection is null"));
        }
        let map = unsafe { ptr::addr_of!((*connection).actor_channels).read_unaligned() };
        if map.pairs.num < 0 || map.pairs.max < map.pairs.num
            || (map.pairs.num > 0 && map.pairs.data.is_null())
        {
            return Err(RuntimeError::new("Invalid canonical ActorChannels"));
        }
        let mut iterator = CoreActorChannelIterator {
            pairs: map.pairs.data.cast(),
            index: 0,
            count: map.pairs.num,
        };
        let pointer = (&mut iterator as *mut CoreActorChannelIterator).cast::<c_void>();
        unsafe { crate::lev_tic_net::replicate_player_channels_body_with(self, pointer) }
    }
    unsafe fn server_tick_client(
        &self,
        level: *mut c_void,
        connection: *mut c_void,
    ) -> RuntimeResult<i32> {
        unsafe { crate::lev_tic_net::server_tick_client_with(self, level, connection) }
    }
    unsafe fn server_tick_client_scoped(
        &self,
        level: *mut c_void,
        connection: *mut c_void,
    ) -> RuntimeResult<i32> {
        let mark = unsafe { self.access.scratch_memory().mark() };
        *self.mark.borrow_mut() = Some(mark);
        let mut storage = mark;
        let result = unsafe {
            crate::lev_tic_net::server_tick_client_body_with(
                self,
                level,
                connection,
                (&mut storage as *mut FMemMark).cast(),
            )
        };
        if let Some(mark) = self.mark.borrow_mut().take() {
            unsafe { self.access.scratch_memory().pop(&mark) };
        }
        result
    }
    unsafe fn mem_mark_pop(&self, buffer: *mut c_void) -> RuntimeResult<()> {
        if buffer.is_null() {
            return Ok(());
        }
        if self.mark.borrow_mut().take().is_some() {
            let mark = unsafe { buffer.cast::<FMemMark>().read_unaligned() };
            unsafe { self.access.scratch_memory().pop(&mark) };
        }
        Ok(())
    }
    unsafe fn mem_push(&self, _mem: *mut c_void, size: u64, count: i32) -> RuntimeResult<*mut c_void> {
        if count < 0 {
            return Err(RuntimeError::new("Negative canonical scratch element count"));
        }
        let bytes = size
            .checked_mul(count as u64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| RuntimeError::new("Canonical scratch allocation overflow"))?;
        Ok(unsafe { self.access.scratch_memory().push_bytes(bytes, 8)? }.cast())
    }
    unsafe fn level_gmem(&self) -> RuntimeResult<*mut c_void> {
        Ok(self.access.scratch_memory().as_ptr().cast())
    }
    unsafe fn actor_update_frequency(
        &self,
        actor: *mut c_void,
        viewer: *mut c_void,
        view_dir: *mut f32,
        view_pos: *mut f32,
    ) -> RuntimeResult<f32> {
        let class = unsafe { ptr::addr_of!((*actor.cast::<UObject>()).class).read_unaligned() };
        if let Some(f) = hp2_core::class_ops::actor_ops(class).and_then(|ops| ops.update_frequency) {
            return Ok(unsafe { f(actor) });
        }
        unsafe { crate::actor_rep::actor_update_frequency(self.access, actor, viewer, view_dir, view_pos) }
    }
    unsafe fn level_cycles_clock(&self, level: *mut c_void, which: i32) -> RuntimeResult<()> {
        Self::clock_field(level.cast(), which, false);
        Ok(())
    }
    unsafe fn level_cycles_unclock(&self, level: *mut c_void, which: i32) -> RuntimeResult<()> {
        Self::clock_field(level.cast(), which, true);
        Ok(())
    }
    unsafe fn driver_server_connection(&self, driver: *mut c_void) -> RuntimeResult<*mut c_void> {
        if driver.is_null() {
            return Err(RuntimeError::new("Null canonical UNetDriver"));
        }
        Ok(unsafe { ptr::addr_of!((*driver.cast::<UNetDriver>()).server_connection).read_unaligned() })
    }
    unsafe fn conn_state(&self, conn: *mut c_void) -> RuntimeResult<i32> {
        if conn.is_null() {
            return Err(RuntimeError::new("Null canonical UNetConnection"));
        }
        Ok(unsafe { ptr::addr_of!((*conn.cast::<UNetConnection>()).state).read_unaligned() })
    }
    unsafe fn conn_replicate_player_channels(&self, conn: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.replicate_player_channels(conn) }
    }
    unsafe fn level_client_viewports_num(&self, level: *mut c_void) -> RuntimeResult<i32> {
        let engine = Self::level_engine(level.cast())?;
        if engine.is_null() {
            return Err(RuntimeError::new("Net tick level has no canonical Engine"));
        }
        let client = unsafe { ptr::addr_of!((*engine).client).read_unaligned() }.cast::<UClient>();
        if client.is_null() {
            return Err(RuntimeError::new("Net tick Engine has no canonical Client"));
        }
        Ok(unsafe { ptr::addr_of!((*client).viewports).read_unaligned() }.num)
    }
    unsafe fn level_engine_client_travel(&self, level: *mut c_void, url: i32) -> RuntimeResult<()> {
        let viewport = Self::viewport0(level.cast())?;
        if viewport.is_null() {
            return Err(RuntimeError::new("Engine->Client->Viewports has no entry"));
        }
        let text: &[u32] = if url == 1 { &TRAVEL_ENTRY } else { &TRAVEL_FAILED };
        unsafe {
            self.access.assign_fstring(ptr::addr_of_mut!((*viewport).travel_url), text)?;
            (*viewport).set_b_travel_items(false);
            (*viewport).travel_type = 0;
        }
        Ok(())
    }
    unsafe fn conn_actor(&self, conn: *mut c_void) -> RuntimeResult<*mut c_void> {
        if conn.is_null() {
            return Err(RuntimeError::new("Null canonical UNetConnection"));
        }
        Ok(unsafe { ptr::addr_of!((*conn.cast::<UNetConnection>()).actor).read_unaligned() })
    }
    unsafe fn conn_is_net_ready(&self, conn: *mut c_void, saturate: i32) -> RuntimeResult<i32> {
        unsafe { crate::connection_owner::is_net_ready(self.access, conn.cast(), saturate) }
    }
    unsafe fn driver_time(&self, driver: *mut c_void) -> RuntimeResult<i64> {
        if driver.is_null() {
            return Err(RuntimeError::new("Null canonical UNetDriver"));
        }
        Ok(i64::from_ne_bytes(unsafe { ptr::addr_of!((*driver.cast::<UNetDriver>()).time).read_unaligned() }))
    }
    unsafe fn conn_driver(&self, conn: *mut c_void) -> RuntimeResult<*mut c_void> {
        if conn.is_null() {
            return Err(RuntimeError::new("Null canonical UNetConnection"));
        }
        Ok(unsafe { ptr::addr_of!((*conn.cast::<UNetConnection>()).driver).read_unaligned() })
    }
    unsafe fn conn_last_receive_time(&self, conn: *mut c_void) -> RuntimeResult<i64> {
        if conn.is_null() {
            return Err(RuntimeError::new("Null canonical UNetConnection"));
        }
        Ok(i64::from_ne_bytes(unsafe { ptr::addr_of!((*conn.cast::<UNetConnection>()).last_receive_time).read_unaligned() }))
    }
    unsafe fn conn_inc_tick_count(&self, conn: *mut c_void) -> RuntimeResult<i32> {
        let slot = unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).tick_count) };
        let value = unsafe { slot.read_unaligned() }.wrapping_add(1);
        unsafe { slot.write_unaligned(value) };
        Ok(value)
    }
    unsafe fn conn_sent_temp_count(&self, conn: *mut c_void) -> RuntimeResult<i32> {
        let temporaries = unsafe {
            ptr::addr_of!((*conn.cast::<UNetConnection>()).sent_temporaries).read_unaligned()
        };
        if temporaries.num < 0 || (temporaries.num > 0 && temporaries.data.is_null()) {
            return Err(RuntimeError::new("Invalid canonical SentTemporaries"));
        }
        Ok(temporaries.num)
    }
    unsafe fn conn_sent_temp_set_net_tag(
        &self,
        conn: *mut c_void,
        index: i32,
        net_tag: i32,
    ) -> RuntimeResult<()> {
        let temporaries = unsafe {
            ptr::addr_of!((*conn.cast::<UNetConnection>()).sent_temporaries).read_unaligned()
        };
        if index < 0 || index >= temporaries.num || temporaries.data.is_null() {
            return Err(RuntimeError::new("SentTemporaries index is out of range"));
        }
        let actor = unsafe {
            temporaries.data.cast::<*mut AActor>().add(index as usize).read_unaligned()
        };
        if !actor.is_null() {
            unsafe { ptr::addr_of_mut!((*actor).net_tag).write_unaligned(net_tag) };
        }
        Ok(())
    }
    unsafe fn actor_event_player_calc_view(
        &self,
        viewer: *mut c_void,
        location: *mut f32,
        rotation: *mut i32,
    ) -> RuntimeResult<*mut c_void> {
        if viewer.is_null() || location.is_null() || rotation.is_null() {
            return Err(RuntimeError::new("PlayerCalcView requires a live actor and outputs"));
        }
        let mut params = crate::game_draw_owner::PlayerCalcViewParams {
            view_actor: viewer.cast(),
            camera_location: unsafe { ptr::read_unaligned(location.cast::<hp2_abi::FVector>()) },
            camera_rotation: unsafe { ptr::read_unaligned(rotation.cast::<hp2_abi::engine::generated::FRotator>()) },
        };
        unsafe {
            crate::game_draw_owner::player_calc_view_params(self.access, viewer.cast(), &mut params)?;
            ptr::write_unaligned(location.cast::<hp2_abi::FVector>(), params.camera_location);
            ptr::write_unaligned(rotation.cast::<hp2_abi::engine::generated::FRotator>(), params.camera_rotation);
        }
        Ok(params.view_actor.cast())
    }
    unsafe fn conn_tick_count(&self, conn: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { ptr::addr_of!((*conn.cast::<UNetConnection>()).tick_count).read_unaligned() })
    }
    unsafe fn level_model_line_check(
        &self,
        level: *mut c_void,
        out_location: *mut f32,
        start: *const f32,
        end: *const f32,
        node_flags: i32,
    ) -> RuntimeResult<()> {
        let level = level.cast::<ULevel>();
        let model = unsafe { ptr::addr_of!((*level).model).read_unaligned() };
        if model.is_null() || out_location.is_null() || start.is_null() || end.is_null() {
            return Err(RuntimeError::new("Invalid canonical model line-check arguments"));
        }
        let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
        hit.time = 1.0;
        let start_v = unsafe { ptr::read_unaligned(start.cast::<hp2_abi::FVector>()) };
        let end_v = unsafe { ptr::read_unaligned(end.cast::<hp2_abi::FVector>()) };
        hit.location = start_v;
        unsafe {
            crate::collision_owner::world_line_check(
                self.access,
                model.cast::<hp2_abi::engine::generated::UModel>(),
                &mut hit,
                end_v,
                start_v,
                hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 },
                node_flags as u32,
            )?;
            ptr::write_unaligned(out_location.cast::<hp2_abi::FVector>(), hit.location);
        }
        Ok(())
    }
    unsafe fn level_actor_count(&self, level: *mut c_void) -> RuntimeResult<i32> {
        unsafe { crate::level_owner::actor_count(level.cast::<ULevel>()) }
    }
    unsafe fn conn_last_rep_time(&self, conn: *mut c_void) -> RuntimeResult<i64> {
        Ok(i64::from_ne_bytes(unsafe { ptr::addr_of!((*conn.cast::<UNetConnection>()).last_rep_time).read_unaligned() }))
    }
    unsafe fn level_actor(&self, level: *mut c_void, index: i32) -> RuntimeResult<*mut c_void> {
        let count = unsafe { crate::level_owner::actor_count(level.cast::<ULevel>())? };
        if index < 0 || index >= count {
            return Err(RuntimeError::new("Level actor index is out of range"));
        }
        Ok(unsafe { crate::level_owner::actor_at(level.cast::<ULevel>(), index).cast() })
    }
    unsafe fn level_max_tick_rate(&self, level: *mut c_void) -> RuntimeResult<f32> {
        let engine = Self::level_engine(level.cast())?;
        if engine.is_null() {
            return Err(RuntimeError::new("Level has no canonical Engine for GetMaxTickRate"));
        }
        unsafe { crate::game_engine_owner::get_max_tick_rate(self.access, engine) }
    }
    unsafe fn actor_check_recent_changes(&self, actor: *mut c_void) -> RuntimeResult<i32> {
        // `AActor::CheckRecentChanges` returns 0; `AInventory::CheckRecentChanges` returns 1.
        Ok(i32::from(unsafe { self.class_is(actor, "Engine.Inventory")? }))
    }
    unsafe fn conn_actor_channel(
        &self,
        conn: *mut c_void,
        actor: *mut c_void,
    ) -> RuntimeResult<*mut c_void> {
        Ok(unsafe {
            crate::network_owner::actor_channel(conn.cast::<UNetConnection>(), actor.cast::<AActor>())?
        }.cast())
    }
    unsafe fn channel_recent_count(&self, channel: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { ptr::addr_of!((*channel.cast::<UActorChannel>()).recent).read_unaligned() }.num)
    }
    unsafe fn channel_dirty_count(&self, channel: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { ptr::addr_of!((*channel.cast::<UActorChannel>()).dirty).read_unaligned() }.num)
    }
    unsafe fn actor_no_variables_to_replicate(
        &self,
        actor: *mut c_void,
        old_version: *mut c_void,
    ) -> RuntimeResult<i32> {
        if unsafe { self.class_is(actor, "Engine.Inventory")? } {
            Ok(unsafe {
                crate::actor_rep::inventory_no_variables_to_replicate(
                    actor.cast(),
                    old_version.cast(),
                )
            } as i32)
        } else {
            Ok(0)
        }
    }
    unsafe fn channel_recent_first(&self, channel: *mut c_void) -> RuntimeResult<*mut c_void> {
        let recent = unsafe { ptr::addr_of!((*channel.cast::<UActorChannel>()).recent).read_unaligned() };
        if recent.num == 0 || recent.data.is_null() {
            return Ok(ptr::null_mut());
        }
        Ok(unsafe { recent.data })
    }
    unsafe fn channel_set_relevant_time(&self, channel: *mut c_void, time: i64) -> RuntimeResult<()> {
        let slot = unsafe { ptr::addr_of_mut!((*channel.cast::<UActorChannel>()).relevant_time) };
        unsafe { slot.write_unaligned(time.to_ne_bytes()) };
        Ok(())
    }
    unsafe fn actor_class_default_life_span(&self, actor: *mut c_void) -> RuntimeResult<f32> {
        let class = unsafe { ptr::addr_of!((*actor.cast::<UObject>()).class).read_unaligned() };
        let defaults = unsafe { self.access.default_object(class.cast::<hp2_abi::engine::generated::UClass>())? }.cast::<AActor>();
        Ok(unsafe { ptr::addr_of!((*defaults).life_span).read_unaligned() })
    }
    unsafe fn conn_set_last_rep_time(&self, conn: *mut c_void, time: i64) -> RuntimeResult<()> {
        let slot = unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).last_rep_time) };
        unsafe { slot.write_unaligned(time.to_ne_bytes()) };
        Ok(())
    }
    unsafe fn sort_priorities(
        &self,
        actors: *mut *mut ActorPriority,
        count: i32,
    ) -> RuntimeResult<()> {
        if count < 0 || (count > 0 && actors.is_null()) {
            return Err(RuntimeError::new("Invalid canonical priority list"));
        }
        if count == 0 {
            return Ok(());
        }
        let list = unsafe { std::slice::from_raw_parts_mut(actors, count as usize) };
        sort_priority_ptrs(list);
        Ok(())
    }
    unsafe fn channel_relevant_time(&self, channel: *mut c_void) -> RuntimeResult<i64> {
        Ok(i64::from_ne_bytes(unsafe { ptr::addr_of!((*channel.cast::<UActorChannel>()).relevant_time).read_unaligned() }))
    }
    unsafe fn driver_relevant_timeout(&self, driver: *mut c_void) -> RuntimeResult<f32> {
        Ok(unsafe { ptr::addr_of!((*driver.cast::<UNetDriver>()).relevant_timeout).read_unaligned() })
    }
    unsafe fn level_inc_num_pv(&self, level: *mut c_void) -> RuntimeResult<()> {
        let slot = unsafe { ptr::addr_of_mut!((*level.cast::<ULevel>()).num_pv) };
        let value = unsafe { slot.read_unaligned() }.wrapping_add(1);
        unsafe { slot.write_unaligned(value) };
        Ok(())
    }
    unsafe fn conn_package_map_class_index(
        &self,
        conn: *mut c_void,
        actor: *mut c_void,
    ) -> RuntimeResult<i32> {
        let package_map = unsafe {
            ptr::addr_of!((*conn.cast::<UNetConnection>()).package_map).read_unaligned()
        };
        let class = unsafe { ptr::addr_of!((*actor.cast::<UObject>()).class).read_unaligned() };
        unsafe { self.access.package_map_object_to_index(package_map.cast::<hp2_abi::engine::generated::UPackageMap>(), class.cast::<hp2_abi::engine::generated::UObject>()) }
    }
    unsafe fn conn_create_actor_channel(&self, conn: *mut c_void) -> RuntimeResult<*mut c_void> {
        let channel = unsafe {
            crate::connection_owner::create_channel(
                self.access,
                conn.cast::<UNetConnection>(),
                crate::level_net::CHTYPE_ACTOR,
                1,
                INDEX_NONE,
            )?
        };
        Ok(channel.cast())
    }
    unsafe fn conn_set_channel_actor(
        &self,
        channel: *mut c_void,
        actor: *mut c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::actor_channel_owner::set_actor(
                self.access,
                channel.cast::<UActorChannel>(),
                actor.cast(),
            )
        }
    }
    unsafe fn level_app_frand(&self) -> RuntimeResult<f32> {
        Ok(self.access.rand() as f32 / hp2_core::ansi_trace::RAND_MAX as f32)
    }
    unsafe fn channel_is_net_ready(
        &self,
        channel: *mut c_void,
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
    unsafe fn channel_replicate_actor(&self, channel: *mut c_void) -> RuntimeResult<()> {
        unsafe {
            crate::actor_channel_owner::replicate_actor(self.access, channel.cast::<UActorChannel>())
        }
    }
    unsafe fn channel_close(&self, channel: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::close(self.access, channel.cast::<UChannel>()) }
    }
    unsafe fn driver_client_connection_count(&self, driver: *mut c_void) -> RuntimeResult<i32> {
        let connections = unsafe {
            ptr::addr_of!((*driver.cast::<UNetDriver>()).client_connections).read_unaligned()
        };
        if connections.num < 0 || (connections.num > 0 && connections.data.is_null()) {
            return Err(RuntimeError::new("Invalid canonical ClientConnections"));
        }
        Ok(connections.num)
    }
    unsafe fn driver_client_connection(
        &self,
        driver: *mut c_void,
        index: i32,
    ) -> RuntimeResult<*mut c_void> {
        let connections = unsafe {
            ptr::addr_of!((*driver.cast::<UNetDriver>()).client_connections).read_unaligned()
        };
        if index < 0 || index >= connections.num || connections.data.is_null() {
            return Err(RuntimeError::new("ClientConnections index is out of range"));
        }
        Ok(unsafe {
            connections.data.cast::<*mut UNetConnection>().add(index as usize).read_unaligned().cast()
        })
    }
    unsafe fn level_server_tick_client(
        &self,
        level: *mut c_void,
        connection: *mut c_void,
        _delta: f32,
    ) -> RuntimeResult<i32> {
        unsafe { self.server_tick_client(level, connection) }
    }
    unsafe fn level_log_running_net(
        &self,
        level: *mut c_void,
        client_count: i32,
    ) -> RuntimeResult<()> {
        let level = level.cast::<ULevel>();
        let info = unsafe { crate::package_map_level_owner::level_info(level)? };
        let title = unsafe { ptr::addr_of!((*info).title).read_unaligned() };
        let mut message: Vec<u32> = Vec::new();
        if !title.data.is_null() {
            let units = unsafe { std::slice::from_raw_parts(title.data.cast::<u32>(), title.num.max(0) as usize) };
            message.extend(units.iter().copied().take_while(|unit| *unit != 0));
        }
        message.push(u32::from(b' '));
        let url = unsafe { ptr::addr_of!((*level).base.url).read_unaligned() };
        if !url.map.data.is_null() {
            let units = unsafe { std::slice::from_raw_parts(url.map.data.cast::<u32>(), url.map.num.max(0) as usize) };
            message.extend(units.iter().copied().take_while(|unit| *unit != 0));
        }
        message.extend(format!(" {client_count}").chars().map(u32::from));
        let category = self.access.intern_str("Log", FindName::Add)?;
        self.access.emit_log(category, &message)
    }
    unsafe fn level_send_net_stats(
        &self,
        level: *mut c_void,
        updated: i32,
    ) -> RuntimeResult<()> {
        unsafe { crate::lev_tic_net::send_net_stats_with(self, level, updated) }
    }
    unsafe fn channel_iterator_valid(&self, iterator: *mut c_void) -> RuntimeResult<i32> {
        let iterator = iterator.cast::<CoreActorChannelIterator>();
        if iterator.is_null() {
            return Err(RuntimeError::new("Null canonical channel iterator"));
        }
        Ok(i32::from(unsafe { (*iterator).index < (*iterator).count }))
    }
    unsafe fn channel_iterator_value(&self, iterator: *mut c_void) -> RuntimeResult<*mut c_void> {
        let iterator = iterator.cast::<CoreActorChannelIterator>();
        if iterator.is_null() {
            return Err(RuntimeError::new("Null canonical channel iterator"));
        }
        let index = unsafe { (*iterator).index };
        if index < 0 || index >= unsafe { (*iterator).count } {
            return Err(RuntimeError::new("Channel iterator index is out of range"));
        }
        Ok(unsafe {
            (*iterator).pairs.add(index as usize).read_unaligned().value
        })
    }
    unsafe fn channel_iterator_next(&self, iterator: *mut c_void) -> RuntimeResult<()> {
        let iterator = iterator.cast::<CoreActorChannelIterator>();
        if iterator.is_null() {
            return Err(RuntimeError::new("Null canonical channel iterator"));
        }
        unsafe { (*iterator).index += 1 };
        Ok(())
    }
    unsafe fn channel_actor(&self, channel: *mut c_void) -> RuntimeResult<*mut c_void> {
        if channel.is_null() {
            return Err(RuntimeError::new("Null canonical actor channel"));
        }
        Ok(unsafe { ptr::addr_of!((*channel.cast::<UActorChannel>()).actor).read_unaligned() })
    }
    unsafe fn actor_is_a(&self, actor: *mut c_void, kind: i32) -> RuntimeResult<i32> {
        let class = match kind {
            KIND_APAWN => "Engine.Pawn",
            KIND_APLAYERPAWN => "Engine.PlayerPawn",
            KIND_AZONEINFO => "Engine.ZoneInfo",
            KIND_ACAMERA => "Engine.Camera",
            KIND_AWEAPON => "Engine.Weapon",
            _ => return Err(RuntimeError::new("LevTicActorIsA: bad kind")),
        };
        Ok(i32::from(unsafe { self.class_is(actor, class)? }))
    }
    unsafe fn conn_user_flags(&self, connection: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe {
            ptr::addr_of!((*connection.cast::<UNetConnection>()).user_flags).read_unaligned()
        })
    }
    unsafe fn conn_send_stats(
        &self,
        level: *mut c_void,
        connection: *mut c_void,
        kind: i32,
        actor_count: i32,
        max_rate: f32,
    ) -> RuntimeResult<()> {
        let level = level.cast::<ULevel>();
        let connection = connection.cast::<UNetConnection>();
        let actor = unsafe { ptr::addr_of!((*connection).actor).read_unaligned() };
        if actor.is_null() {
            return Err(RuntimeError::new("SendNetStats connection has no actor"));
        }
        let driver = unsafe { ptr::addr_of!((*connection).driver).read_unaligned() }.cast::<UNetDriver>();
        if kind == 0 {
            let connections = unsafe {
                ptr::addr_of!((*driver).client_connections).read_unaligned()
            }.num.max(1);
            let gspc = hp2_core::unix_clock::seconds_per_cycle();
            let actor_cycles = unsafe { ptr::addr_of!((*level).actor_tick_cycles).read_unaligned() };
            let net_cycles = unsafe { ptr::addr_of!((*level).net_tick_cycles).read_unaligned() };
            let num_pv = unsafe { ptr::addr_of!((*level).num_pv).read_unaligned() };
            let num_reps = unsafe { ptr::addr_of!((*level).num_reps).read_unaligned() };
            let num_rpc = unsafe { ptr::addr_of!((*level).num_rpc).read_unaligned() };
            let message = format!(
                "r={} cli={} act={:03.1} ({}) net={:03.1} pv/c={} rep/c={} rpc/c={}",
                crate::math::app_round(max_rate),
                connections,
                gspc * 1000.0 * actor_cycles as f32,
                actor_count,
                gspc * 1000.0 * net_cycles as f32,
                num_pv / connections,
                num_reps / connections,
                num_rpc / connections,
            );
            unsafe { client_message(self.access, actor, &message) }
        } else {
            let send = unsafe { ptr::addr_of!((*driver).send_cycles).read_unaligned() };
            let recv = unsafe { ptr::addr_of!((*driver).recv_cycles).read_unaligned() };
            let message = format!("snd={send:02.1} recv={recv:02.1}");
            unsafe { client_message(self.access, actor, &message) }
        }
    }
    unsafe fn driver_spawn_priority_seconds(&self, driver: *mut c_void) -> RuntimeResult<f32> {
        Ok(unsafe { ptr::addr_of!((*driver.cast::<UNetDriver>()).spawn_priority_seconds).read_unaligned() })
    }
    unsafe fn channel_last_update_time(&self, channel: *mut c_void) -> RuntimeResult<i64> {
        Ok(i64::from_ne_bytes(unsafe { ptr::addr_of!((*channel.cast::<UActorChannel>()).last_update_time).read_unaligned() }))
    }
    unsafe fn actor_net_priority(
        &self,
        actor: *mut c_void,
        sent: *mut c_void,
        time: f32,
        lag: f32,
    ) -> RuntimeResult<f32> {
        if unsafe { self.class_is(actor, "Engine.Pawn")? } {
            unsafe { crate::pawn_sense::get_net_priority(actor.cast(), sent.cast(), time, lag) }
                .map_err(|_| RuntimeError::new("APawn::GetNetPriority failed"))
        } else {
            Ok(unsafe { crate::actor::get_net_priority(actor.cast(), time) })
        }
    }
    unsafe fn conn_best_lag(&self, conn: *mut c_void) -> RuntimeResult<f32> {
        Ok(unsafe { ptr::addr_of!((*conn.cast::<UNetConnection>()).best_lag).read_unaligned() })
    }
    unsafe fn level_fast_line_check(
        &self,
        level: *mut c_void,
        end: *const f32,
        start: *const f32,
    ) -> RuntimeResult<i32> {
        let level = level.cast::<ULevel>();
        let model = unsafe { ptr::addr_of!((*level).model).read_unaligned() };
        if model.is_null() || end.is_null() || start.is_null() {
            return Err(RuntimeError::new("Invalid canonical fast line check"));
        }
        let end_v = unsafe { ptr::read_unaligned(end.cast::<hp2_abi::FVector>()) };
        let start_v = unsafe { ptr::read_unaligned(start.cast::<hp2_abi::FVector>()) };
        Ok(i32::from(unsafe { crate::collision_owner::model_fast_line_check(model.cast::<hp2_abi::engine::generated::UModel>(), end_v, start_v)? }))
    }
    unsafe fn gmath_sin_tab(&self, index: i32) -> RuntimeResult<f32> {
        Ok(hp2_core::math::sin_tab_inline(index, hp2_core::math::trig_table()))
    }
    unsafe fn gmath_cos_tab(&self, index: i32) -> RuntimeResult<f32> {
        Ok(hp2_core::math::cos_tab_inline(index, hp2_core::math::trig_table()))
    }
}


/// `Actor->eventClientMessage(Stats, NAME_None, 0)`.
unsafe fn client_message(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    text: &str,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{FName, TArrayRaw};
    let mut message: Vec<u32> = text.chars().map(u32::from).collect();
    message.push(0);
    #[repr(C)]
    struct ClientMessageParams {
        message: TArrayRaw,
        message_type: FName,
        beep: i32,
    }
    let mut params = ClientMessageParams {
        message: TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 },
        message_type: FName { index: 0 },
        beep: 0,
    };
    let object = actor.cast::<UObject>();
    let name = access.intern_str("ClientMessage", FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(object, name, false)? };
    if function.is_null() {
        return Err(RuntimeError::new("ClientMessage function is missing"));
    }
    if unsafe { (*function).parms_size } as usize != std::mem::size_of::<ClientMessageParams>()
        || unsafe { (*function).num_parms } != 3
    {
        return Err(RuntimeError::new("ClientMessage parameter layout does not match metadata"));
    }
    unsafe { access.assign_fstring(&mut params.message, &message)?; }
    let result = unsafe {
        access.execute_event(object, function, (&mut params as *mut ClientMessageParams).cast(), ptr::null_mut())
    };
    let destroy = unsafe { access.destroy_owned_fstring(&mut params.message) };
    result.and(destroy)
}

/// Canonical `ULevel::TickNetClient`.
///
/// # Safety
/// `level` is a live Core-owned ULevel with initialized network state.
pub(crate) unsafe fn tick_net_client(access: CoreAccess<'_>, level: *mut ULevel) -> RuntimeResult<()> {
    if level.is_null() {
        return Err(RuntimeError::new("Null canonical ULevel in TickNetClient"));
    }
    let mut ops = CoreLevelTickNetOps { access, mark: std::cell::RefCell::new(None) };
    unsafe { crate::lev_tic_net::tick_net_client_with(&mut ops, level.cast()) }
}

/// Canonical `ULevel::TickNetServer`.
///
/// # Safety
/// `level` is a live Core-owned ULevel with initialized network state.
pub(crate) unsafe fn tick_net_server(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    delta_seconds: f32,
) -> RuntimeResult<()> {
    if level.is_null() {
        return Err(RuntimeError::new("Null canonical ULevel in TickNetServer"));
    }
    let delta = delta_seconds;
    // The shared policy expects a service table for its native wrapper; drive the
    // typed backend directly instead.
    let mut ops = CoreLevelTickNetOps { access, mark: std::cell::RefCell::new(None) };
    unsafe { crate::lev_tic_net::tick_net_server_with(&mut ops, level.cast(), delta) }
}
