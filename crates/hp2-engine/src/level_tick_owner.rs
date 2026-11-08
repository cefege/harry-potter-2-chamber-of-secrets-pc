//! Direct Core ownership of the source-ordered `ULevel::Tick` host.

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APlayerPawn, FActorLink, ULevel, UNetConnection,
    UNetDriver, UObject, UProperty,
};
use hp2_abi::render_span::FMemMark;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::c_void, ptr};

const LEVELTICK_VIEWPORTS_ONLY: i32 = 1;
const LEVELTICK_ALL: i32 = 2;
const USOCK_OPEN: i32 = 3;
/// Runs the canonical `ULevel::Tick(LEVELTICK_All, DeltaSeconds)` path.
///
/// # Errors
/// Returns the first Core VM, ownership, network, collision, or lifecycle error.
///
/// # Safety
/// `level` and every actor, driver, connection, link, and class reachable from it
/// are live canonical records owned by `access` for the synchronous call.
pub(crate) unsafe fn tick_all(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    delta_seconds: f32,
) -> RuntimeResult<()> {
    if level.is_null() {
        return Err(RuntimeError::new("Null canonical ULevel in Tick"));
    }
    let info = unsafe { crate::package_map_level_owner::level_info(level)? };
    unsafe { initialize_tick_stats(level); }
    let engine_state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Level Tick has no canonical Engine memory owner"))?;
    let scratch = access.scratch_memory();
    let mut mark = unsafe { scratch.mark() };
    let mut engine_mark = unsafe { engine_state.memory.mark() };
    let mut ops = CoreLevelTickOps {
        access,
        level,
        scratch,
        engine_memory: &engine_state.memory,
    };
    unsafe {
        crate::lev_tic::level_tick_body_with(
            &mut ops,
            level.cast(),
            LEVELTICK_ALL,
            delta_seconds,
            info.cast(),
            0,
            0,
            ptr::addr_of_mut!(mark).cast(),
            ptr::addr_of_mut!(engine_mark).cast(),
        )
    }
}

unsafe fn initialize_tick_stats(level: *mut ULevel) {
    unsafe {
        ptr::addr_of_mut!((*level).net_tick_cycles).write_unaligned(0);
        ptr::addr_of_mut!((*level).net_diff_cycles).write_unaligned(0);
        ptr::addr_of_mut!((*level).actor_tick_cycles).write_unaligned(0);
        ptr::addr_of_mut!((*level).audio_tick_cycles).write_unaligned(0);
        ptr::addr_of_mut!((*level).find_path_cycles).write_unaligned(0);
        ptr::addr_of_mut!((*level).move_cycles).write_unaligned(0);
        ptr::addr_of_mut!((*level).num_moves).write_unaligned(0);
        ptr::addr_of_mut!((*level).num_reps).write_unaligned(0);
        ptr::addr_of_mut!((*level).num_pv).write_unaligned(0);
        ptr::addr_of_mut!((*level).get_relevant_cycles).write_unaligned(0);
        ptr::addr_of_mut!((*level).num_rpc).write_unaligned(0);
        ptr::addr_of_mut!((*level).see_player).write_unaligned(0);
        ptr::addr_of_mut!((*level).spawning).write_unaligned(0);
        ptr::addr_of_mut!((*level).unused).write_unaligned(0);
        hp2_core::timing_globals::reset_script_cycles();
        hp2_core::timing_globals::reset_script_entry_tag();
    }
}

struct CoreLevelTickOps<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
    scratch: &'a hp2_core::mem_owner::MemStackOwner,
    engine_memory: &'a hp2_core::mem_owner::MemStackOwner,
}

impl crate::lev_tic::LevelTickOps for CoreLevelTickOps<'_> {
    type Error = RuntimeError;

    unsafe fn guarded<T, F>(&mut self, _scope: i32, f: F) -> RuntimeResult<T>
    where
        F: FnOnce(&mut Self) -> RuntimeResult<T>,
    {
        f(self)
    }
    unsafe fn init_runaway(&mut self) -> RuntimeResult<()> {
        self.access.reset_script_runaway();
        Ok(())
    }
    unsafe fn seconds_per_cycle(&mut self) -> RuntimeResult<f32> {
        Ok(hp2_core::unix_clock::seconds_per_cycle())
    }
    unsafe fn driver_tick(&mut self, driver: *mut c_void, delta: f32) -> RuntimeResult<()> {
        let demo_driver = unsafe { ptr::addr_of!((*self.level).base.demo_rec_driver).read_unaligned() };
        if driver == demo_driver {
            unsafe {
                crate::demo_owner::tick_dispatch(
                    self.access,
                    driver.cast::<hp2_abi::engine::generated::UDemoRecDriver>(),
                    delta,
                )
            }
        } else {
            unsafe { crate::network_owner::tick_dispatch(self.access, driver.cast(), delta) }
        }
    }
    unsafe fn driver_server_connection(
        &mut self,
        driver: *mut c_void,
    ) -> RuntimeResult<*mut c_void> {
        Ok(unsafe {
            ptr::addr_of!((*driver.cast::<UNetDriver>()).server_connection).read_unaligned()
        })
    }
    unsafe fn connection_open(&mut self, connection: *mut c_void) -> RuntimeResult<bool> {
        Ok(unsafe { ptr::addr_of!((*connection.cast::<UNetConnection>()).state).read_unaligned() }
            == USOCK_OPEN)
    }
    unsafe fn tick_net_client(&mut self, _level: *mut c_void, _delta: f32) -> RuntimeResult<()> {
        unsafe { crate::level_tick_network_owner::tick_net_client(self.access, self.level) }
    }
    unsafe fn tick_demo_playback(
        &mut self,
        level: *mut c_void,
        _delta: f32,
    ) -> RuntimeResult<()> {
        let _ = unsafe { crate::level_tick_demo_owner::tick_demo_playback(self.access, level.cast())? };
        Ok(())
    }
    unsafe fn hash_tick(&mut self, level: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_tick(level.cast()) }
    }
    unsafe fn update_time(&mut self, info: *mut c_void) -> RuntimeResult<()> {
        unsafe { update_level_system_time(self.access, info.cast()) }
    }
    unsafe fn players_only(&mut self, info: *mut c_void) -> RuntimeResult<bool> {
        Ok(unsafe { (*info.cast::<ALevelInfo>()).b_players_only() })
    }
    unsafe fn pauser_empty(&mut self, info: *mut c_void) -> RuntimeResult<bool> {
        let pauser = unsafe { ptr::addr_of!((*info.cast::<ALevelInfo>()).pauser).read_unaligned() };
        Ok(pauser.num == 0)
    }
    unsafe fn actor_cycles_begin(&mut self, level: *mut c_void) -> RuntimeResult<()> {
        let cycles = hp2_core::unix_clock::cycles() as i32;
        let value = unsafe {
            ptr::addr_of!((*level.cast::<ULevel>()).actor_tick_cycles).read_unaligned()
        };
        unsafe {
            ptr::addr_of_mut!((*level.cast::<ULevel>()).actor_tick_cycles)
                .write_unaligned(value.wrapping_sub(cycles));
        }
        Ok(())
    }
    unsafe fn actor_cycles_end(&mut self, level: *mut c_void) -> RuntimeResult<()> {
        let cycles = hp2_core::unix_clock::cycles().wrapping_sub(34) as i32;
        let value = unsafe {
            ptr::addr_of!((*level.cast::<ULevel>()).actor_tick_cycles).read_unaligned()
        };
        unsafe {
            ptr::addr_of_mut!((*level.cast::<ULevel>()).actor_tick_cycles)
                .write_unaligned(value.wrapping_add(cycles));
        }
        Ok(())
    }
    unsafe fn startup_trace(&mut self, _level: *mut c_void) -> RuntimeResult<()> {
        Ok(())
    }
    unsafe fn actor_count(&mut self, level: *mut c_void) -> RuntimeResult<i32> {
        unsafe { crate::level_owner::actor_count(level.cast()) }
    }
    unsafe fn actor_at(&mut self, level: *mut c_void, index: i32) -> RuntimeResult<*mut c_void> {
        let count = unsafe { crate::level_owner::actor_count(level.cast())? };
        if index < 0 || index >= count {
            return Err(RuntimeError::new("Level Tick actor index is out of range"));
        }
        Ok(unsafe { crate::level_owner::actor_at(level.cast(), index).cast() })
    }
    unsafe fn tick_actor(
        &mut self,
        actor: *mut c_void,
        delta: f32,
        tick_type: i32,
    ) -> RuntimeResult<i32> {
        unsafe {
            crate::actor_tick_owner::tick_core(
                self.access,
                actor.cast(),
                delta,
                tick_type,
                c"direct".as_ptr(),
                c"dynamic".as_ptr(),
            )
        }
    }
    unsafe fn clear_newly_spawned(&mut self, level: *mut c_void) {
        unsafe {
            ptr::addr_of_mut!((*level.cast::<ULevel>()).newly_spawned)
                .write_unaligned(ptr::null_mut());
        }
    }
    unsafe fn newly_spawned(&mut self, level: *mut c_void) -> *mut c_void {
        unsafe {
            ptr::addr_of!((*level.cast::<ULevel>()).newly_spawned)
                .read_unaligned()
                .cast()
        }
    }
    unsafe fn link_actor(&mut self, link: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { ptr::addr_of!((*link.cast::<FActorLink>()).actor).read_unaligned().cast() })
    }
    unsafe fn link_next(&mut self, link: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { ptr::addr_of!((*link.cast::<FActorLink>()).next).read_unaligned().cast() })
    }
    unsafe fn actor_ticked(&mut self, actor: *mut c_void) -> RuntimeResult<bool> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_ticked() })
    }
    unsafe fn actor_always_tick(&mut self, actor: *mut c_void) -> bool {
        unsafe { (*actor.cast::<AActor>()).b_always_tick() }
    }
    unsafe fn player_pawn(&mut self, actor: *mut c_void) -> RuntimeResult<bool> {
        let class = self.access.native_class("Engine.PlayerPawn")?;
        Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
    }
    unsafe fn pawn_player(&mut self, actor: *mut c_void) -> *mut c_void {
        unsafe {
            ptr::addr_of!((*actor.cast::<APlayerPawn>()).player)
                .read_unaligned()
                .cast()
        }
    }
    unsafe fn player_read_input(
        &mut self,
        player: *mut c_void,
        delta: f32,
    ) -> RuntimeResult<()> {
        unsafe { crate::input_owner::player_read_input(self.access, player, delta) }
    }
    unsafe fn player_input_event(
        &mut self,
        actor: *mut c_void,
        delta: f32,
    ) -> RuntimeResult<()> {
        unsafe { execute_float_event(self.access, actor.cast(), "PlayerInput", delta) }
    }
    unsafe fn zero_input_floats(&mut self, actor: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::input_owner::zero_input_floats(self.access, actor.cast()) }
    }
    unsafe fn tick_net_server(
        &mut self,
        level: *mut c_void,
        delta: f32,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::level_tick_network_owner::tick_net_server(self.access, level.cast(), delta)
        }
    }
    unsafe fn driver_flush(&mut self, driver: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::network_owner::tick_flush(self.access, driver.cast()) }
    }
    unsafe fn tick_demo_record(
        &mut self,
        level: *mut c_void,
        _delta: f32,
    ) -> RuntimeResult<()> {
        let _ = unsafe { crate::level_tick_demo_owner::tick_demo_record(self.access, level.cast())? };
        Ok(())
    }
    unsafe fn finish(
        &mut self,
        level: *mut c_void,
        mark: *mut c_void,
        engine_mark: *mut c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            self.scratch.pop(&*mark.cast::<FMemMark>());
            self.engine_memory.pop(&*engine_mark.cast::<FMemMark>());
            crate::level_lifecycle_owner::cleanup_destroyed(self.access, level.cast())
        }
    }
    unsafe fn frame_timing_log(&mut self, _frame_start: i64) -> RuntimeResult<()> {
        Ok(())
    }
}

pub(crate) unsafe fn update_level_system_time(
    access: CoreAccess<'_>,
    info: *mut ALevelInfo,
) -> RuntimeResult<()> {
    let mut timeval = std::mem::MaybeUninit::<libc::timeval>::uninit();
    if unsafe { libc::gettimeofday(timeval.as_mut_ptr(), ptr::null_mut()) } != 0 {
        return Err(RuntimeError::new("LevelInfo UpdateTime gettimeofday failed"));
    }
    let now = unsafe { libc::time(ptr::null_mut()) };
    if now == -1 {
        return Err(RuntimeError::new("LevelInfo UpdateTime time failed"));
    }
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    if unsafe { libc::localtime_r(&now, local.as_mut_ptr()) }.is_null() {
        return Err(RuntimeError::new("LevelInfo UpdateTime localtime failed"));
    }
    let timeval = unsafe { timeval.assume_init() };
    let local = unsafe { local.assume_init() };
    let values = hp2_core::unix_clock::system_time(
        local.tm_year,
        local.tm_mon,
        local.tm_wday,
        local.tm_mday,
        local.tm_hour,
        local.tm_min,
        local.tm_sec,
        timeval.tv_usec as i64,
    );
    const NAMES: [&str; 8] = [
        "Year", "Month", "DayOfWeek", "Day", "Hour", "Minute", "Second", "Millisecond",
    ];
    let class = unsafe { ptr::addr_of!((*info.cast::<UObject>()).class).read_unaligned() };
    let property_class = access.native_class("IntProperty")?;
    for (name, value) in NAMES.into_iter().zip([
        values.0, values.1, values.2, values.3, values.4, values.5, values.6, values.7,
    ]) {
        let field_name = name;
        let name = access.intern_str(field_name, FindName::Find)?;
        let property = unsafe { access.find_field(class.cast(), name, property_class) }
            .cast::<UProperty>();
        if property.is_null() {
            return Err(RuntimeError::new("LevelInfo UpdateTime field is missing").with_context(field_name));
        }
        let address = unsafe { access.property_address(info.cast(), property.cast())? };
        unsafe { address.cast::<i32>().write_unaligned(value); }
    }
    Ok(())
}

unsafe fn execute_float_event(
    access: CoreAccess<'_>,
    object: *mut UObject,
    event: &str,
    delta: f32,
) -> RuntimeResult<()> {
    let name = access.intern_str(event, FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(object, name, false)? };
    if function.is_null() {
        return Err(RuntimeError::new("PlayerInput event is missing"));
    }
    let mut parameter = delta;
    let parameter_size = unsafe { ptr::addr_of!((*function).parms_size).read_unaligned() };
    if parameter_size as usize != std::mem::size_of::<f32>()
        || unsafe { (*function).num_parms } != 1
    {
        return Err(RuntimeError::new("PlayerInput event parameter layout does not match metadata"));
    }
    unsafe { access.execute_event(object, function, ptr::addr_of_mut!(parameter).cast(), ptr::null_mut()) }
}
