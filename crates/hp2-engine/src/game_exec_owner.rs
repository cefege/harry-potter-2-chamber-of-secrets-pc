//! Core-owned adapter for the shared `UGameEngine::Exec` policy.

use crate::game_exec::{self, ExecAction, ExecCursor, ExecOps, ExecStep};
use hp2_abi::engine::generated::{
    TArrayRaw, UChannel, UDownload, UGameEngine, ULevel, UNetConnection, UNetDriver, UPendingLevel,
    UObject,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::c_void, marker::PhantomData, ptr, sync::atomic::{AtomicI32, Ordering}};
static IN_CANCEL: AtomicI32 = AtomicI32::new(0);

unsafe fn close_server_channel(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    pending: bool,
) -> RuntimeResult<()> {
    let root = unsafe {
        if pending { (*engine).g_pending_level } else { (*engine).g_level }
    };
    if root.is_null() { return Ok(()); }
    let driver = unsafe {
        if pending { (*root.cast::<UPendingLevel>()).net_driver }
        else { (*root.cast::<ULevel>()).base.net_driver }
    }.cast::<UNetDriver>();
    if driver.is_null() { return Ok(()); }
    let connection = unsafe { (*driver).server_connection.cast::<UNetConnection>() };
    if connection.is_null() { return Ok(()); }
    let channel = unsafe { (*connection).channels[0].cast::<UChannel>() };
    if channel.is_null() { return Ok(()); }
    unsafe { crate::channel_owner::close(access, channel)?; }

    let root = unsafe {
        if pending { (*engine).g_pending_level } else { (*engine).g_level }
    };
    if root.is_null() { return Ok(()); }
    let driver = unsafe {
        if pending { (*root.cast::<UPendingLevel>()).net_driver }
        else { (*root.cast::<ULevel>()).base.net_driver }
    }.cast::<UNetDriver>();
    if driver.is_null() { return Ok(()); }
    let connection = unsafe { (*driver).server_connection.cast::<UNetConnection>() };
    if connection.is_null() { return Ok(()); }
    unsafe { crate::connection_owner::flush_net(access, connection) }
}

struct Command<'borrow, 'output> {
    cmd: *const u32,
    tail: *const u32,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
}

struct CoreExecOps<'access, 'borrow, 'output> {
    access: CoreAccess<'access>,
    _output: PhantomData<&'borrow mut crate::exec_output::ExecOutput<'output>>,
}

fn text(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain(std::iter::once(0)).collect()
}

unsafe fn browse_command(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    command: *const u32,
    travel: i32,
    prefix: &str,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    use hp2_abi::engine::generated::{FURL, TArrayRaw};
    use std::mem::MaybeUninit;

    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let mut url = MaybeUninit::<FURL>::uninit();
    unsafe { crate::url_owner::UrlOwner::construct_default(access, &defaults, url.as_mut_ptr())?; }
    let url = unsafe { url.assume_init_mut() };
    let valid = match unsafe {
        crate::url_owner::UrlOwner::parse_update(
            access, &defaults, url, ptr::addr_of_mut!((*engine).last_url), command, travel,
        )
    } {
        Ok(valid) => valid,
        Err(error) => {
            unsafe { crate::url_owner::UrlOwner::destroy(access, url)?; }
            return Err(error);
        }
    };
    if valid == 0 {
        unsafe { crate::url_owner::UrlOwner::destroy(access, url)?; }
        return Ok(true);
    }
    let mut error = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let result = unsafe {
        crate::game_travel_owner::browse(access, engine, url, ptr::null(), &mut error)
    };
    let log = if matches!(&result, Ok(false)) && error.num > 1 && !error.data.is_null() {
        let mut line = text(prefix);
        line.pop();
        let message = unsafe {
            std::slice::from_raw_parts(error.data.cast::<u32>(), error.num as usize)
        };
        let length = message.iter().position(|unit| *unit == 0).unwrap_or(message.len());
        line.extend_from_slice(&message[..length]);
        output(&line)
    } else {
        Ok(())
    };
    let error_cleanup = unsafe { access.destroy_owned_fstring(&mut error) };
    let url_cleanup = unsafe { crate::url_owner::UrlOwner::destroy(access, url) };
    let value = result?;
    log?;
    error_cleanup?;
    url_cleanup?;
    Ok(value)
}

unsafe fn browse_last_url(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    let mut error = hp2_abi::engine::generated::TArrayRaw {
        data: ptr::null_mut(), num: 0, max: 0,
    };
    let result = unsafe {
        crate::game_travel_owner::browse(
            access, engine, ptr::addr_of!((*engine).last_url), ptr::null(), &mut error,
        )
    };
    let log = if matches!(&result, Ok(false)) && error.num > 1 && !error.data.is_null() {
        let mut line = text("Reconnect failed: ");
        line.pop();
        let message = unsafe {
            std::slice::from_raw_parts(error.data.cast::<u32>(), error.num as usize)
        };
        let length = message.iter().position(|unit| *unit == 0).unwrap_or(message.len());
        line.extend_from_slice(&message[..length]);
        output(&line)
    } else {
        Ok(())
    };
    let cleanup = unsafe { access.destroy_owned_fstring(&mut error) };
    let value = result?;
    log?;
    cleanup?;
    Ok(value)
}
unsafe fn actor_event_function(
    access: CoreAccess<'_>,
    actor: *mut hp2_abi::engine::generated::AActor,
    event_name: &str,
    parameters: *mut c_void,
) -> RuntimeResult<()> {
    if actor.is_null() {
        return Err(RuntimeError::new("Canonical GameEngine Exec has no event target"));
    }
    let name = access.intern_str(event_name, hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(actor.cast(), name, false)? };
    if function.is_null() {
        return Err(RuntimeError::new("Canonical GameEngine Exec event is missing").with_context(event_name));
    }
    unsafe { crate::actor_owner::process_event(access, actor.cast(), function, parameters, ptr::null_mut()) }
}

#[repr(C)]
struct ServerTravelParams {
    url: TArrayRaw,
    items: i32,
}
/// Dispatches the checked GameInfo ServerTravel event for a live canonical level.
///
/// # Safety
/// `level` is live and `url` is a NUL-terminated UTF-32 string for this synchronous call.
pub(crate) unsafe fn server_travel_event(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    url: *const u32,
    items: i32,
) -> RuntimeResult<()> {
    if url.is_null() {
        return Err(RuntimeError::new("Null canonical ServerTravel URL"));
    }
    let mut length = 0usize;
    while unsafe { *url.add(length) } != 0 {
        length = length.checked_add(1)
            .ok_or_else(|| RuntimeError::new("Canonical ServerTravel URL length overflow"))?;
    }
    let count = i32::try_from(length.checked_add(1)
        .ok_or_else(|| RuntimeError::new("Canonical ServerTravel URL length overflow"))?)
        .map_err(|_| RuntimeError::new("Canonical ServerTravel URL exceeds FString length range"))?;
    let info = unsafe { crate::package_map_level_owner::level_info(level)? };
    let mut parameters = ServerTravelParams {
        url: TArrayRaw { data: url.cast_mut().cast(), num: count, max: count },
        items,
    };
    unsafe {
        actor_event_function(
            access,
            info.cast(),
            "ServerTravel",
            (&mut parameters as *mut ServerTravelParams).cast(),
        )
    }
}

#[repr(C)]
struct BroadcastParams {
    message: *const u32,
    beep: i32,
    message_type: hp2_abi::engine::generated::FName,
}
unsafe fn client_viewport0(game: *mut UGameEngine) -> RuntimeResult<Option<*mut hp2_abi::engine::generated::UViewport>> {
    let client = unsafe { (*game).client.cast::<hp2_abi::engine::generated::UClient>() };
    if client.is_null() { return Ok(None); }
    let viewports = unsafe { ptr::addr_of!((*client).viewports).read_unaligned() };
    if viewports.num <= 0 { return Ok(None); }
    if viewports.data.is_null() || viewports.max < viewports.num {
        return Err(RuntimeError::new("Invalid canonical GameEngine viewport array"));
    }
    let viewport = unsafe { viewports.data.cast::<*mut hp2_abi::engine::generated::UViewport>().read_unaligned() };
    if viewport.is_null() { return Ok(None); }
    Ok(Some(viewport))
}

unsafe fn travel_action(
    access: CoreAccess<'_>,
    game: *mut UGameEngine,
    command: &mut Command<'_, '_>,
    branch: i32,
) -> RuntimeResult<bool> {
    const ABSOLUTE: i32 = 0;
    const PARTIAL: i32 = 1;
    let travel = if branch == 0 { PARTIAL } else { ABSOLUTE };
    if let Some(viewport) = unsafe { client_viewport0(game)? } {
        if branch == 2 {
            unsafe { close_server_channel(access, game, false)?; }
            unsafe { close_server_channel(access, game, true)?; }
        }
        let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
        let mut owned_url = None;
        let url = if branch == 2 {
            hp2_core::lit!("?failed").as_ptr()
        } else if branch == 3 {
            let value = unsafe {
                crate::url_owner::UrlOwner::string(access, &defaults, ptr::addr_of!((*game).last_url), false)?
            };
            let pointer = value.data.cast::<u32>();
            owned_url = Some(value);
            pointer
        } else {
            command.tail
        };
        let result = unsafe { crate::game_engine_owner::set_client_travel(access, viewport.cast(), url, 0, travel) };
        let cleanup = if let Some(mut value) = owned_url {
            unsafe { access.destroy_owned_fstring(&mut value) }
        } else {
            Ok(())
        };
        result.and(cleanup)?;
        Ok(true)
    } else {
        if branch == 3 {
            unsafe { browse_last_url(access, game, command.output)?; }
        } else {
            let (url, prefix) = match branch {
                0 => (command.tail, "Open failed: "),
                1 => (command.tail, "Start failed: "),
                _ => (hp2_core::lit!("?failed").as_ptr(), "Disconnect failed: "),
            };
            unsafe { browse_command(access, game, url, travel, prefix, command.output)?; }
        }
        Ok(true)
    }
}

impl<'access, 'borrow, 'output> ExecOps for CoreExecOps<'access, 'borrow, 'output> {
    type Command = Command<'borrow, 'output>;
    type Error = RuntimeError;

    unsafe fn parse_command(
        &mut self,
        command: &mut Self::Command,
        cursor: ExecCursor,
        keyword: &[u32],
    ) -> RuntimeResult<bool> {
        let stream = match cursor {
            ExecCursor::Command => &mut command.cmd,
            ExecCursor::Tail => &mut command.tail,
        };
        let mut cursor: *const i32 = (*stream).cast();
        let matched = unsafe { hp2_core::command::parse_command(&mut cursor, keyword.as_ptr().cast()) } != 0;
        *stream = cursor.cast();
        Ok(matched)
    }

    unsafe fn is_server_only(&mut self) -> RuntimeResult<bool> {
        let mode = self.access.process_mode();
        Ok(mode.is_server && !mode.is_client)
    }

    unsafe fn action(
        &mut self,
        action: ExecAction,
        engine: *mut c_void,
        command: &mut Self::Command,
    ) -> RuntimeResult<ExecStep> {
        use ExecAction::*;
        let game = engine.cast::<UGameEngine>();
        match action {
            ServerTravel => {
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                unsafe { server_travel_event(self.access, level, command.tail, 0)?; }
                Ok(ExecStep::Return(true))
            }
            Say => {
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                let info = unsafe { crate::package_map_level_owner::level_info(level)? };
                let mut parameters = BroadcastParams {
                    message: command.tail, beep: 1,
                    message_type: hp2_abi::engine::generated::FName { index: 0 },
                };
                unsafe {
                    actor_event_function(
                        self.access, info.cast(), "BroadcastMessage",
                        &mut parameters as *mut _ as *mut c_void,
                    )?;
                }
                Ok(ExecStep::Return(true))
            }
            Open => Ok(ExecStep::Return(unsafe {
                travel_action(self.access, game, command, 0)?
            })),
            Start => Ok(ExecStep::Return(unsafe {
                travel_action(self.access, game, command, 1)?
            })),
            Disconnect => Ok(ExecStep::Return(unsafe {
                travel_action(self.access, game, command, 2)?
            })),
            Reconnect => Ok(ExecStep::Return(unsafe {
                travel_action(self.access, game, command, 3)?
            })),
            Exit => {
                unsafe { close_server_channel(self.access, game, false)?; }
                unsafe { close_server_channel(self.access, game, true)?; }
                (command.output)(hp2_core::lit!("Closing by request"))?;
                let category = self.access.intern_str("Log", FindName::Add)?;
                self.access.emit_log(category, hp2_core::lit!("appRequestExit(0)"))?;
                hp2_core::unix_signal::set_exit_requested(true);
                Ok(ExecStep::Return(true))
            }
            GSpyLite => {
                std::process::Command::new("GSpyLite.exe")
                    .spawn()
                    .map_err(|_| RuntimeError::new("Unable to launch GSpyLite.exe"))?;
                Ok(ExecStep::Return(true))
            }
            ObjectClean => {
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                if !level.is_null() {
                    unsafe { crate::level_lifecycle_owner::cleanup_destroyed(self.access, level)?; }
                }
                Ok(ExecStep::Return(true))
            }
            CurrentTickRate => {
                let value = unsafe { ptr::addr_of!((*game).current_tick_rate).read_unaligned() };
                let line = format!("{value:.6}").chars().map(u32::from).collect::<Vec<_>>();
                (command.output)(&line)?;
                Ok(ExecStep::Return(true))
            }
            MaxTickRate => {
                let value = unsafe { crate::game_engine_owner::get_max_tick_rate(self.access, game)? };
                let line = format!("{value:.6}").chars().map(u32::from).collect::<Vec<_>>();
                (command.output)(&line)?;
                Ok(ExecStep::Return(true))
            }
            Cancel => {
                if IN_CANCEL.load(Ordering::Relaxed) == 0 {
                    IN_CANCEL.store(1, Ordering::Relaxed);
                    let pending = unsafe { (*game).g_pending_level };
                    if !pending.is_null() {
                        let net_pending_class = self.access.native_class("Engine.NetPendingLevel")?;
                        if unsafe { hp2_core::object_model::is_a(pending, net_pending_class.cast()) } {
                            let driver = unsafe { (*pending.cast::<UPendingLevel>()).net_driver.cast::<UNetDriver>() };
                            if !driver.is_null() {
                                let connection = unsafe { (*driver).server_connection.cast::<UNetConnection>() };
                                if !connection.is_null() {
                                    let download = unsafe { (*connection).download.cast::<UDownload>() };
                                    if !download.is_null()
                                        && unsafe { crate::download_owner::try_skip(self.access, download)? } != 0
                                    {
                                        IN_CANCEL.store(0, Ordering::Relaxed);
                                        return Ok(ExecStep::Return(true));
                                    }
                                }
                            }
                        }
                        let localized = self.access.localize_text(
                            hp2_core::lit!("Progress"), hp2_core::lit!("CancelledConnect"),
                            Some(hp2_core::lit!("Engine")), None, false,
                        )?;
                        let empty = [0u32];
                        unsafe { crate::game_engine_owner::set_progress(
                            self.access, game, localized.as_ptr(), empty.as_ptr(), 2.0,
                        )?; }
                    } else {
                        let empty = [0u32];
                        unsafe { crate::game_engine_owner::set_progress(
                            self.access, game, empty.as_ptr(), empty.as_ptr(), 0.0,
                        )?; }
                    }
                    unsafe { crate::game_engine_owner::cancel_pending(self.access, game)?; }
                    IN_CANCEL.store(0, Ordering::Relaxed);
                }
                Ok(ExecStep::Return(true))
            }
            LevelExec => {
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                if !level.is_null() && unsafe {
                    crate::level_exec_owner::exec(self.access, level, command.cmd, command.output)?
                } {
                    Ok(ExecStep::Return(true))
                } else {
                    Ok(ExecStep::Continue)
                }
            }
            GameInfoExec => {
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                if level.is_null() { return Ok(ExecStep::Continue); }
                let info = unsafe { crate::package_map_level_owner::level_info(level)? };
                if info.is_null() { return Ok(ExecStep::Continue); }
                let target = unsafe { (*info).game.cast::<UObject>() };
                if target.is_null() { return Ok(ExecStep::Continue); }
                if unsafe { crate::engine_exec_owner::process_console_exec(
                    self.access, target, ptr::null_mut(), command.cmd, command.output,
                )? } {
                    Ok(ExecStep::Return(true))
                } else {
                    Ok(ExecStep::Continue)
                }
            }
            QualifiedEngineExec => {
                let handled = unsafe { crate::engine_exec_owner::exec(
                    self.access, game.cast(), command.cmd, command.output,
                )? };
                Ok(ExecStep::Return(handled))
            }
            RestrictSet => {
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                if level.is_null() { return Ok(ExecStep::Allow); }
                let info = unsafe { crate::package_map_level_owner::level_info(level)? };
                if info.is_null() || unsafe { (*info).net_mode } != 2 {
                    return Ok(ExecStep::Allow);
                }
                let mut tail = command.cmd;
                let mut cursor: *const i32 = tail.cast();
                let is_set = unsafe {
                    hp2_core::command::parse_command(&mut cursor, hp2_core::lit!("SET").as_ptr().cast())
                };
                tail = cursor.cast();
                if is_set == 0 {
                    return Ok(ExecStep::Allow);
                }
                let mut name = vec![0u32; 256];
                let mut cursor: *const i32 = tail.cast();
                let tokenized = unsafe { hp2_core::tokenize::parse_token(&mut cursor, name.as_mut_ptr().cast(), name.len() as i32, true) };
                tail = cursor.cast();
                let _ = tail;
                if !tokenized {
                    return Ok(ExecStep::Allow);
                }
                let end = name.iter().position(|unit| *unit == 0).unwrap_or(name.len());
                if end == name.len() { return Ok(ExecStep::Allow); }
                let class_meta = self.access.native_class("Class")?;
                let class = unsafe { self.access.find_object_by_name(class_meta, &name[..=end])? };
                let actor_class = self.access.native_class("Engine.Actor")?;
                let game_info_class = self.access.native_class("Engine.GameInfo")?;
                if let Some(class) = class {
                    if unsafe { hp2_core::object_model::is_child_of(class.cast(), actor_class.cast()) }
                        && !unsafe { hp2_core::object_model::is_child_of(class.cast(), game_info_class.cast()) }
                    {
                        return Ok(ExecStep::Return(false));
                    }
                }
                Ok(ExecStep::Allow)
            }
            SaveGame => {
                let first = unsafe { *command.tail } as i32;
                if !(i32::from(b'0')..=i32::from(b'9')).contains(&first) {
                    return Ok(ExecStep::Return(true));
                }
                crate::game_save_owner::make_save_directories(self.access)?;
                let slot = unsafe { crate::game_save_owner::app_atoi(command.tail)? };
                let filename = crate::game_save_owner::slot_filename(self.access, slot)?;
                let filename = crate::game_save_owner::filename_text(&filename)?;
                let message = format!("Saving game...(before call) Filename: {filename}");
                let category = self.access.intern_str("Log", FindName::Add)?;
                self.access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())?;
                unsafe { crate::game_engine_owner::save_game(self.access, game, slot)?; }
                let restart_sounds = hp2_core::lit!("RestartSounds");
                let _ = unsafe {
                    crate::engine_exec_owner::exec(
                        self.access, game.cast(), restart_sounds.as_ptr(), command.output,
                    )?
                };
                Ok(ExecStep::Return(true))
            }
            LoadGame => {
                let first = unsafe { *command.tail } as i32;
                if !(i32::from(b'0')..=i32::from(b'9')).contains(&first) {
                    return Ok(ExecStep::Return(true));
                }
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                if level.is_null() { return Ok(ExecStep::Return(true)); }
                let info = unsafe { crate::package_map_level_owner::level_info(level)? };
                if info.is_null() { return Ok(ExecStep::Return(true)); }
                let next_url = unsafe { ptr::addr_of!((*info).next_url).read_unaligned() };
                if next_url.num > 1 { return Ok(ExecStep::Return(true)); }
                unsafe { crate::game_load_map_owner::load_persistent_actor_cache(self.access)?; }
                let slot = unsafe { crate::game_save_owner::app_atoi(command.tail)? };
                crate::game_save_owner::make_save_directories(self.access)?;
                let category = self.access.intern_str("Log", FindName::Add)?;
                let message = format!("Loading game slot {slot}");
                self.access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())?;
                let mut load_url = [0u32; 32];
                if unsafe { crate::game_save::format_load_game_url(slot, load_url.as_mut_ptr(), load_url.len() as i32) } {
                    unsafe { server_travel_event(self.access, level, load_url.as_ptr(), 0)?; }
                }
                Ok(ExecStep::Return(true))
            }
            SavePersistentActors => {
                let level = unsafe { (*game).g_level.cast::<ULevel>() };
                if level.is_null() { return Ok(ExecStep::Return(false)); }
                crate::game_save_owner::make_save_directories(self.access)?;
                let info = unsafe { crate::package_map_level_owner::level_info(level)? };
                if info.is_null() { return Err(RuntimeError::new("SavePersistentActors requires canonical LevelInfo")); }
                let map_name = unsafe { crate::game_save_owner::level_enter_text(self.access, info)? };
                let _ = unsafe { crate::level_persist_owner::save_persistent_actors(self.access, level, map_name)? };
                Ok(ExecStep::Return(true))
            }
        }
    }
}

/// Executes canonical GameEngine Exec without constructing a native output device.
///
/// # Safety
/// `engine` and `cmd` must identify a live canonical engine and NUL-terminated UTF-32 command.
pub unsafe fn exec(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    cmd: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if engine.is_null() || cmd.is_null() {
        return Err(RuntimeError::new("Invalid canonical GameEngine Exec arguments"));
    }
    let mut command = Command { cmd, tail: cmd, output };
    let mut ops = CoreExecOps { access, _output: PhantomData };
    unsafe { game_exec::exec_with_ops(&mut ops, engine.cast(), &mut command) }
}
