//! Canonical Core host leaves for source-shared `UGameEngine::Tick` policy.

use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};

/// Runs source `UObject::StaticTick` checks without Native dispatch.
pub(crate) fn static_tick(access: CoreAccess<'_>) -> RuntimeResult<()> {
    hp2_core::object_bootstrap::static_tick(access)
}

/// Advances the one cache owner installed in the canonical EngineState.
pub(crate) fn cache_tick(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("UGameEngine Tick has no canonical Engine owner"))?;
    let mut cache = state.cache.try_borrow_mut()
        .map_err(|_| RuntimeError::new("Engine cache tick reentered"))?;
    let cache = cache.as_mut()
        .ok_or_else(|| RuntimeError::new("Engine cache is unavailable during Tick"))?;
    cache.tick(access);
    Ok(())
}
/// Advances the canonical network pending-level branch of GameEngine Tick.
///
/// # Safety
/// `pending` is a live canonical `UNetPendingLevel` with initialized network state.
pub(crate) unsafe fn network_pending_tick(
    access: CoreAccess<'_>,
    pending: *mut hp2_abi::engine::generated::UNetPendingLevel,
    delta: f32,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{UNetConnection, UNetDriver, UPendingLevel};

    if pending.is_null() {
        return Err(RuntimeError::new("Null canonical UNetPendingLevel"));
    }
    let pending_base = pending.cast::<UPendingLevel>();
    let driver = unsafe { (*pending_base).net_driver.cast::<UNetDriver>() };
    if driver.is_null() {
        return Err(RuntimeError::new("UNetPendingLevel has no NetDriver"));
    }
    let server = unsafe { (*driver).server_connection.cast::<UNetConnection>() };
    if server.is_null() {
        return Err(RuntimeError::new("UNetPendingLevel NetDriver has no ServerConnection"));
    }
    let pending_error = unsafe {
        std::ptr::addr_of!((*pending_base).error).read_unaligned()
    };
    if unsafe { (*server).state } == 1 && pending_error.num <= 1 {
        let error = access.localize_text(
            hp2_core::lit!("Errors"),
            hp2_core::lit!("ConnectionFailed"),
            None,
            None,
            false,
        )?;
        unsafe {
            access.assign_fstring(std::ptr::addr_of_mut!((*pending_base).error), &error)?;
        }
        return Ok(());
    }
    unsafe { crate::network_owner::tick_dispatch(access, driver, delta)?; }
    unsafe { crate::network_owner::tick_flush(access, driver) }
}

/// Sends the source `UNetPendingLevel::SendJoin` notification through Core.
///
/// # Safety
/// `pending` is a live canonical `UNetPendingLevel` with a live server connection.
pub(crate) unsafe fn network_pending_send_join(
    access: CoreAccess<'_>,
    pending: *mut hp2_abi::engine::generated::UNetPendingLevel,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{UNetConnection, UNetDriver, UPendingLevel};

    if pending.is_null() {
        return Err(RuntimeError::new("Null canonical UNetPendingLevel"));
    }
    let base = pending.cast::<UPendingLevel>();
    let driver = unsafe { (*base).net_driver.cast::<UNetDriver>() };
    if driver.is_null() {
        return Err(RuntimeError::new("UNetPendingLevel has no NetDriver"));
    }
    let server = unsafe { (*driver).server_connection.cast::<UNetConnection>() };
    if server.is_null() {
        return Err(RuntimeError::new("UNetPendingLevel NetDriver has no ServerConnection"));
    }
    unsafe { std::ptr::addr_of_mut!((*pending).base.sent_join).write_unaligned(1); }
    let channel = unsafe { (*server).channels[0].cast::<hp2_abi::engine::generated::UControlChannel>() };
    if !channel.is_null() && unsafe { (*channel).base.closing == 0 } {
        unsafe { crate::channel_owner::serialize_control(access, channel, hp2_core::lit!("JOIN").as_ptr())?; }
    }
    unsafe { crate::connection_owner::flush_net(access, server) }
}

fn engine_state(access: CoreAccess<'_>) -> RuntimeResult<&crate::core_owner::EngineState> {
    access
        .owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("UGameEngine Tick has no canonical Engine owner"))
}

pub(crate) fn repair_save_pending(access: CoreAccess<'_>) -> RuntimeResult<i32> {
    Ok(engine_state(access)?.repair_save_pending.get())
}

pub(crate) fn repair_save_slot(access: CoreAccess<'_>) -> RuntimeResult<i32> {
    Ok(engine_state(access)?.repair_save_slot.get())
}

pub(crate) fn set_repair_save_pending(access: CoreAccess<'_>, value: i32) -> RuntimeResult<()> {
    engine_state(access)?.repair_save_pending.set(value);
    Ok(())
}

pub(crate) fn set_repair_save_slot(access: CoreAccess<'_>, value: i32) -> RuntimeResult<()> {
    engine_state(access)?.repair_save_slot.set(value);
    Ok(())
}

pub(crate) fn app_cycles() -> u32 {
    hp2_core::unix_clock::cycles()
}

pub(crate) fn request_exit() {
    hp2_core::unix_signal::set_exit_requested(true);
}

pub(crate) fn increment_ticks() {
    unsafe {
        hp2_core::timing_globals::GTicks = hp2_core::timing_globals::GTicks.wrapping_add(1);
    }
}

pub(crate) fn log_all_windows_closed(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
    access.emit_log(category, hp2_core::lit!("All Windows Closed"))
}

pub(crate) unsafe fn string_nonempty(value: *mut hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<i32> {
    if value.is_null() {
        return Err(RuntimeError::new("Null canonical FString"));
    }
    let raw = unsafe { &*value };
    if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical FString storage"));
    }
    if raw.num <= 1 {
        return Ok(0);
    }
    Ok(i32::from(unsafe { *raw.data.cast::<u32>() } != 0))
}

pub(crate) unsafe fn level_info(
    level: *mut hp2_abi::engine::generated::ULevel,
) -> RuntimeResult<*mut hp2_abi::engine::generated::ALevelInfo> {
    unsafe { crate::package_map_level_owner::level_info(level) }
}
/// Dispatches the canonical PlayerPawn `EventViewFlash` script event.
///
/// # Safety
/// `viewport` is a live canonical UViewport whose Actor is a live PlayerPawn.
pub(crate) unsafe fn viewport_event_view_flash(
    access: CoreAccess<'_>,
    viewport: *mut hp2_abi::engine::generated::UViewport,
    delta: f32,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{APlayerPawn, UObject};

    if viewport.is_null() {
        return Err(RuntimeError::new("Null canonical UViewport"));
    }
    let actor = unsafe { (*viewport).actor.cast::<APlayerPawn>() };
    if actor.is_null() {
        return Err(RuntimeError::new("UViewport has no PlayerPawn for EventViewFlash"));
    }
    let class: *mut hp2_abi::engine::generated::UClass = unsafe { (*actor.cast::<UObject>()).class.cast() };
    if class.is_null() {
        return Err(RuntimeError::new("PlayerPawn has no canonical UClass"));
    }
    let name = access.intern_str("ViewFlash", hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(actor.cast(), name, false)? };
    if function.is_null() {
        return Err(RuntimeError::new("PlayerPawn ViewFlash function is missing"));
    }
    #[repr(C)]
    struct ViewFlashParams {
        delta_seconds: f32,
    }
    let mut params = ViewFlashParams { delta_seconds: delta };
    unsafe {
        crate::actor_owner::process_event(
            access,
            actor.cast(),
            function,
            (&mut params as *mut ViewFlashParams).cast(),
            std::ptr::null_mut(),
        )
    }
}
/// Deletes a pending level through its canonical class operation.
///
/// # Safety
/// `pending` is a live Core-owned pending-level object.
pub(crate) unsafe fn pending_level_delete(
    access: CoreAccess<'_>,
    pending: *mut hp2_abi::engine::generated::UPendingLevel,
) -> RuntimeResult<()> {
    if pending.is_null() {
        return Err(RuntimeError::new("Null canonical UPendingLevel"));
    }
    unsafe { access.delete_object(pending.cast()) }
}

pub(crate) unsafe fn level_info_pauser_nonempty(
    info: *mut hp2_abi::engine::generated::ALevelInfo,
) -> RuntimeResult<i32> {
    if info.is_null() {
        return Err(RuntimeError::new("Null canonical ALevelInfo"));
    }
    unsafe { string_nonempty(std::ptr::addr_of_mut!((*info).pauser)) }
}

pub(crate) unsafe fn level_info_next_url_nonempty(
    info: *mut hp2_abi::engine::generated::ALevelInfo,
) -> RuntimeResult<i32> {
    if info.is_null() {
        return Err(RuntimeError::new("Null canonical ALevelInfo"));
    }
    unsafe { string_nonempty(std::ptr::addr_of_mut!((*info).next_url)) }
}

pub(crate) unsafe fn pending_error_nonempty(
    pending: *mut hp2_abi::engine::generated::UPendingLevel,
) -> RuntimeResult<i32> {
    if pending.is_null() {
        return Err(RuntimeError::new("Null canonical UPendingLevel"));
    }
    unsafe { string_nonempty(std::ptr::addr_of_mut!((*pending).error)) }
}

pub(crate) unsafe fn pending_fail_code_nonempty(
    pending: *mut hp2_abi::engine::generated::UPendingLevel,
) -> RuntimeResult<i32> {
    if pending.is_null() {
        return Err(RuntimeError::new("Null canonical UPendingLevel"));
    }
    unsafe { string_nonempty(std::ptr::addr_of_mut!((*pending).fail_code)) }
}

pub(crate) unsafe fn viewport_travel_url_nonempty(
    viewport: *mut hp2_abi::engine::generated::UViewport,
) -> RuntimeResult<i32> {
    if viewport.is_null() {
        return Err(RuntimeError::new("Null canonical UViewport"));
    }
    unsafe { string_nonempty(std::ptr::addr_of_mut!((*viewport).travel_url)) }
}

pub(crate) unsafe fn level_info_next_url_is_restart(
    info: *mut hp2_abi::engine::generated::ALevelInfo,
) -> RuntimeResult<i32> {
    if info.is_null() {
        return Err(RuntimeError::new("Null canonical ALevelInfo"));
    }
    let text = unsafe { std::ptr::addr_of!((*info).next_url).read_unaligned() };
    if text.num <= 1 || text.data.is_null() {
        return Ok(0);
    }
    let restart = hp2_core::lit!("?RESTART");
    Ok(i32::from(unsafe { hp2_core::ansi::stricmp(text.data.cast(), restart.as_ptr().cast()) == 0 }))
}

pub(crate) unsafe fn viewport_travel_is_restart(
    viewport: *mut hp2_abi::engine::generated::UViewport,
) -> RuntimeResult<i32> {
    if viewport.is_null() {
        return Err(RuntimeError::new("Null canonical UViewport"));
    }
    let text = unsafe { std::ptr::addr_of!((*viewport).travel_url).read_unaligned() };
    if text.num <= 1 || text.data.is_null() {
        return Ok(0);
    }
    let restart = hp2_core::lit!("?RESTART");
    Ok(i32::from(unsafe { hp2_core::ansi::stricmp(text.data.cast(), restart.as_ptr().cast()) == 0 }))
}

// ---------------------------------------------------------------------------
// Canonical `UGameEngine::Tick` host.
// ---------------------------------------------------------------------------

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APawn, APlayerPawn, APlayerReplicationInfo, TArrayRaw, TMapStringString,
    UActorChannel, UClient, UConsole, UEngine, UField, UGameEngine, ULevel,
    UNetConnection, UNetDriver, UPendingLevel, UProperty, UStruct, UViewport,
};
use std::cell::RefCell;
use std::ffi::CStr;
use std::ptr;

use hp2_core::name_owner::FindName;

/// `TRAVEL_Relative` (`Engine/Inc/UnURL.h`).
const TRAVEL_RELATIVE: i32 = 1;
/// `IST_Max`, `EInputKey` control keys (`Engine/Inc/UnInput.h`).
const IST_PRESS: i32 = 1;
const IST_MAX: i32 = 5;
const IK_PAUSE: i32 = 19;
const IK_ESCAPE: i32 = 27;
const IK_DELETE: i32 = 46;
const IK_SCROLL_LOCK: i32 = 145;
const IK_BACKSLASH: i32 = 220;
const IK_MOUSE_X: i32 = 228;
const IK_MOUSE_Y: i32 = 229;
/// `CPF_Travel` (`Engine/Inc/UnObjBas.h`).
const CPF_TRAVEL: u32 = 0x0001_0000;
/// `LEVACT_Loading` (`Engine/Inc/UnLevAct.h`).
const LEVACT_LOADING: i32 = 1;

struct CoreGameTickOps<'a> {
    access: CoreAccess<'a>,
    /// Localized texts whose pointers are handed back to the source policy.
    retained: RefCell<Vec<Vec<u32>>>,
}

impl<'a> CoreGameTickOps<'a> {
    fn retain(&self, mut value: Vec<u32>) -> RuntimeResult<*const u32> {
        if value.last().copied() != Some(0) {
            value.push(0);
        }
        let mut retained = self.retained.try_borrow_mut()
            .map_err(|_| RuntimeError::new("GameEngine Tick localized-text reentered"))?;
        retained.push(value);
        Ok(retained.last().expect("retained text was just appended").as_ptr())
    }

    fn log_line(&self, message: &str) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Add)?;
        self.access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())
    }

    fn engine(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<*mut UGameEngine> {
        if engine.is_null() {
            return Err(RuntimeError::new("Null canonical UGameEngine in Tick"));
        }
        Ok(engine.cast::<UGameEngine>())
    }

    /// Source `LocalizeError("ConnectionFailed")`.
    fn localize_connection_failed(&self) -> RuntimeResult<Vec<u32>> {
        self.access.localize_text(
            hp2_core::lit!("Errors"),
            hp2_core::lit!("ConnectionFailed"),
            None,
            None,
            false,
        )
    }
}


/// Source `Object->IsA(Class)` for a canonical class path.
unsafe fn is_kind(access: CoreAccess<'_>, object: *mut std::ffi::c_void, class_name: &str) -> RuntimeResult<bool> {
    let class = access.native_class(class_name)?;
    Ok(unsafe { hp2_core::object_model::is_a(object, class.cast()) })
}

/// Source `ExportTravel(FOutputDevice& Out, AActor* Actor)` into Core text.
unsafe fn export_travel_text(access: CoreAccess<'_>, actor: *mut hp2_abi::engine::generated::AActor) -> RuntimeResult<Vec<u32>> {
    use hp2_abi::engine::generated::UObject;
    let mut text: Vec<u32> = Vec::new();
    if actor.is_null() {
        return Err(RuntimeError::new("ExportTravel requires a canonical actor"));
    }
    let class = unsafe { ptr::addr_of!((*actor.cast::<UObject>()).class).read_unaligned() };
    let class_name = unsafe { access.object_path_text(class.cast::<UObject>())? };
    let actor_name = access.name_text(unsafe { i32::from_ne_bytes((*actor.cast::<UObject>()).name) })?;
    let mut header: Vec<u32> = "Class=".chars().map(u32::from).collect();
    header.extend(class_name.iter().copied().take_while(|unit| *unit != 0));
    header.extend(" Name=".chars().map(u32::from));
    header.extend(actor_name.iter().copied().take_while(|unit| *unit != 0));
    header.extend("\r\n{\r\n".chars().map(u32::from));
    text.extend(header);
    if unsafe { (*actor).b_travel() } {
        let actor_class = access.native_class("Engine.Actor")?;
        let object_property = access.native_class("ObjectProperty")?;
        let defaults = unsafe { access.default_object(class.cast::<hp2_abi::engine::generated::UClass>())? };
        // Source `TFieldIterator<UProperty>(Actor->GetClass())`: iterate the
        // full class hierarchy, most-derived class first, then each
        // superclass's children — inherited travel properties (e.g.
        // PlayerPawn.CurrentGameState) must export too.
        let mut scope = class.cast::<UStruct>();
        while !scope.is_null() {
        let mut link = unsafe {
            ptr::addr_of!((*scope).children).read_unaligned()
        }.cast::<UField>();
        while !link.is_null() {
            let property = link.cast::<UProperty>();
            let flags = unsafe { ptr::addr_of!((*property).property_flags).read_unaligned() };
            if flags & CPF_TRAVEL != 0 {
                let dimension = unsafe { ptr::addr_of!((*property).array_dim).read_unaligned() };
                let mut index = 0;
                while index < dimension {
                    // `UProperty::ExportText(Index, ...)`: empty text skips the property.
                    let value = unsafe {
                        access.property_export_text_indexed(
                            property,
                            actor.cast(),
                            defaults.cast(),
                            index,
                            CPF_TRAVEL,
                        )
                    };
                    if let Ok(value) = value {
                        if value.iter().any(|unit| *unit != 0) {
                            let name = unsafe {
                                access.name_text(i32::from_ne_bytes(ptr::addr_of!((*link.cast::<UObject>()).name).read_unaligned()))?
                            };
                            text.extend(name.iter().copied().take_while(|unit| *unit != 0));
                            if dimension != 1 {
                                text.extend(format!("[{index}]").chars().map(u32::from));
                            }
                            text.push(u32::from(b'='));
                            // Actor references log the referenced object's name;
                            // every other property logs its ExportedText.
                            let property_class = unsafe {
                                ptr::addr_of!((*property.cast::<UObject>()).class).read_unaligned()
                            };
                            let is_object_ref = unsafe {
                                hp2_core::object_model::is_child_of(
                                    property_class.cast(),
                                    object_property.cast(),
                                )
                            };
                            let mut wrote_reference = false;
                            if is_object_ref {
                                let object_property_ptr = property.cast::<hp2_abi::engine::generated::UObjectProperty>();
                                let value_class = unsafe {
                                    ptr::addr_of!((*object_property_ptr).property_class).read_unaligned()
                                };
                                if !value_class.is_null()
                                    && unsafe {
                                        hp2_core::object_model::is_child_of(
                                            value_class,
                                            actor_class.cast(),
                                        )
                                    }
                                {
                                    let offset = unsafe { ptr::addr_of!((*property).offset).read_unaligned() };
                                    let stride = unsafe { ptr::addr_of!((*property).element_size).read_unaligned() };
                                    let address = unsafe {
                                        actor.cast::<u8>().add(
                                            usize::try_from(offset).map_err(|_| RuntimeError::new("Invalid export offset"))?
                                                + usize::try_from(index.saturating_mul(stride)).unwrap_or(0),
                                        )
                                    };
                                    let referenced = unsafe {
                                        ptr::read_unaligned(address.cast::<*mut UObject>())
                                    };
                                    if referenced.is_null() {
                                        text.extend("None".chars().map(u32::from));
                                    } else {
                                        let name = unsafe {
                                            access.name_text(i32::from_ne_bytes((*referenced).name))?
                                        };
                                        text.extend(name.iter().copied().take_while(|unit| *unit != 0));
                                    }
                                    wrote_reference = true;
                                }
                            }
                            if !wrote_reference {
                                text.extend(value.iter().copied().take_while(|unit| *unit != 0));
                            }
                            text.extend("\r\n".chars().map(u32::from));
                        }
                    }
                    index += 1;
                }
            }
            link = unsafe { ptr::addr_of!((*link).next).read_unaligned() }.cast::<UField>();
        }
        scope = unsafe { ptr::addr_of!((*scope.cast::<hp2_abi::engine::generated::UField>()).super_field).read_unaligned() }.cast::<UStruct>();
        }
    }
    text.extend("}\r\n".chars().map(u32::from));
    text.push(0);
    Ok(text)
}

/// Core-facing pending-level operations for `UNetPendingLevel::Tick`/`SendJoin`.
struct CorePendingTickOps<'a> {
    access: CoreAccess<'a>,
    retained: Vec<Vec<u32>>,
}

impl crate::net_pending::PendingTickOps for CorePendingTickOps<'_> {
    type Error = RuntimeError;
    fn check(
        &mut self,
        valid: bool,
        expression: &CStr,
        line: i32,
    ) -> RuntimeResult<()> {
        if valid {
            Ok(())
        } else {
            Err(RuntimeError::new(&expression.to_string_lossy())
                .with_context(&format!("UnPenLev.cpp:{line}")))
        }
    }
    fn localized_error(&mut self, key: &CStr) -> RuntimeResult<*const u32> {
        let key = key.to_string_lossy();
        let mut units: Vec<u32> = key.chars().map(u32::from).collect();
        units.push(0);
        let mut text = self.access.localize_text(
            hp2_core::lit!("Errors"),
            &units,
            None,
            None,
            false,
        )?;
        if text.last().copied() != Some(0) {
            text.push(0);
        }
        self.retained.push(text);
        Ok(self.retained.last().expect("retained text was just appended").as_ptr())
    }
    fn assign_fstring(
        &mut self,
        destination: *mut TArrayRaw,
        text: *const u32,
    ) -> RuntimeResult<()> {
        if text.is_null() {
            return Err(RuntimeError::new("Null pending-level localized text"));
        }
        let mut length = 0usize;
        while unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        unsafe {
            self.access.assign_fstring(
                destination,
                std::slice::from_raw_parts(text, length + 1),
            )
        }
    }
    fn driver_tick(&mut self, driver: *mut std::ffi::c_void, delta: f32) -> RuntimeResult<()> {
        unsafe { crate::network_owner::tick_dispatch(self.access, driver.cast(), delta) }
    }
    fn driver_flush(&mut self, driver: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { crate::network_owner::tick_flush(self.access, driver.cast()) }
    }
    fn log_connection(
        &mut self,
        connection: *mut std::ffi::c_void,
        text: *const u32,
    ) -> RuntimeResult<()> {
        if connection.is_null() {
            return Err(RuntimeError::new("Pending tick has no server connection"));
        }
        let mut length = 0usize;
        while unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        let message = unsafe { std::slice::from_raw_parts(text, length + 1) };
        let category = self.access.intern_str("DevNet", FindName::Add)?;
        self.access.emit_log(category, message)
    }
    fn flush_connection(&mut self, connection: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::flush_net(self.access, connection.cast()) }
    }
}

/// Advances a canonical `UPendingLevel` with its source `Tick` policy.
unsafe fn pending_tick(
    access: CoreAccess<'_>,
    pending: *mut std::ffi::c_void,
    delta: f32,
) -> RuntimeResult<()> {
    let mut ops = CorePendingTickOps { access, retained: Vec::new() };
    unsafe { crate::net_pending::tick_with(&mut ops, pending, delta) }
}

/// Runs the source `TickPending` load continuation, destroying its owned FString.
unsafe fn pending_load_scope(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    let pending = unsafe { ptr::addr_of!((*engine).g_pending_level).read_unaligned() };
    if pending.is_null() {
        return Err(RuntimeError::new("Canonical TickPending load has no GPendingLevel"));
    }
    let pending = pending.cast::<UPendingLevel>();
    let mut error = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let result = unsafe {
        crate::game_load_map_owner::load_map(
            access,
            engine,
            ptr::addr_of!((*pending).url),
            pending,
            ptr::null(),
            &mut error,
        )
    };
    let allowed = matches!(&result, Ok(_));
    let mut continuation = Ok(());
    if allowed {
        if error.num > 1 && !error.data.is_null() {
            let message = unsafe {
                std::slice::from_raw_parts(error.data.cast::<u32>(), error.num as usize)
            };
            let localized = access.localize_text(
                hp2_core::lit!("Errors"),
                hp2_core::lit!("ConnectionFailed"),
                None,
                None,
                false,
            )?;
            continuation = unsafe {
                crate::game_engine_owner::set_progress(
                    access,
                    engine,
                    localized.as_ptr(),
                    message.as_ptr(),
                    4.0,
                )
            };
        } else if unsafe { ptr::addr_of!((*pending).lone_player).read_unaligned() } == 0 {
            let level = unsafe { ptr::addr_of!((*engine).g_level).read_unaligned() }.cast::<ULevel>();
            let entry = unsafe { ptr::addr_of!((*engine).g_entry).read_unaligned() }.cast::<ULevel>();
            for target in [level, entry] {
                if !target.is_null() {
                    let info = unsafe { crate::package_map_level_owner::level_info(target)? };
                    unsafe { (*info).level_action = LEVACT_LOADING as u8; }
                }
            }
            let client = unsafe { ptr::addr_of!((*engine).client).read_unaligned() };
            if !client.is_null() {
                continuation = unsafe { crate::platform_owner::client_tick(access, client.cast::<hp2_abi::engine::generated::USDLClient>()) };
            }
            if continuation.is_ok() {
                let mut ops = CorePendingTickOps { access, retained: Vec::new() };
                continuation = unsafe {
                    crate::net_pending::send_join_with(&mut ops, pending.cast())
                };
            }
            if continuation.is_ok() {
                unsafe {
                    ptr::addr_of_mut!((*pending).net_driver).write_unaligned(ptr::null_mut());
                    ptr::addr_of_mut!((*pending).demo_rec_driver).write_unaligned(ptr::null_mut());
                }
            }
        }
    }
    let cleanup = unsafe { access.destroy_owned_fstring(&mut error) };
    result.map(|_| ())?;
    continuation?;
    cleanup
}

/// Builds the source `TravelInfo` map for a travel mode.
unsafe fn build_travel_info(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    level: *mut ULevel,
    mode: i32,
    players: impl Fn() -> RuntimeResult<Vec<(usize, *mut hp2_abi::engine::generated::AActor)>>,
) -> RuntimeResult<TMapStringString> {
    let mut map: TMapStringString = unsafe { std::mem::zeroed() };
    // `TravelInfo = E->GLevel->TravelInfo;` (modes 0 and 1).
    if mode == 0 || mode == 1 {
        let source = unsafe { ptr::addr_of!((*level).travel_info) };
        unsafe { crate::level_owner::assign_travel_map(access, &mut map, source)?; }
    }
    if mode == 1 {
        for (_, actor) in players()? {
            let text = unsafe { export_travel_text(access, actor)? };
            let info = unsafe {
                ptr::addr_of!((*actor.cast::<APlayerPawn>()).base.player_replication_info)
                    .read_unaligned()
            }.cast::<APlayerReplicationInfo>();
            if info.is_null() {
                continue;
            }
            let key = unsafe { ptr::addr_of!((*info).player_name).read_unaligned() };
            if key.num <= 0 || key.data.is_null() {
                return Err(RuntimeError::new("Travel player has no PlayerName"));
            }
            let key_units = unsafe {
                std::slice::from_raw_parts(key.data.cast::<u32>(), key.num as usize)
            };
            unsafe { crate::level_owner::insert_travel_map(access, &mut map, key_units, &text)?; }
        }
    }
    Ok(map)
}

/// Source `UGameEngine::ServerTravel`-style browse for the `Tick` NextURL branch.
unsafe fn server_switch_travel(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    mode: i32,
) -> RuntimeResult<()> {
    let level = unsafe { ptr::addr_of!((*engine).g_level).read_unaligned() }.cast::<ULevel>();
    if level.is_null() {
        return Err(RuntimeError::new("ServerTravel Tick branch has no GLevel"));
    }
    let info = unsafe { crate::package_map_level_owner::level_info(level)? };
    let next_url = unsafe { ptr::addr_of!((*info).next_url).read_unaligned() };
    if next_url.data.is_null() {
        return Err(RuntimeError::new("ServerTravel Tick branch has no NextURL text"));
    }
    {
        let mut message: Vec<u32> = "Server switch level: ".chars().map(u32::from).collect();
        let units = unsafe { std::slice::from_raw_parts(next_url.data.cast::<u32>(), next_url.num.max(0) as usize) };
        message.extend(units.iter().copied().take_while(|unit| *unit != 0));
        let category = access.intern_str("Log", FindName::Add)?;
        access.emit_log(category, &message)?;
    }
    let players = || -> RuntimeResult<Vec<(usize, *mut hp2_abi::engine::generated::AActor)>> {
        let mut result = Vec::new();
        let count = unsafe { crate::level_owner::actor_count(level)? };
        let mut index = 0;
        while index < count {
            let actor = unsafe { crate::level_owner::actor_at(level, index) };
            let pawn = actor.cast::<APlayerPawn>();
            if !actor.is_null()
                && unsafe { is_kind(access, actor.cast(), "Engine.PlayerPawn")? }
                && !unsafe { ptr::addr_of!((*pawn).player).read_unaligned() }.is_null()
            {
                // `if( Cast<UViewport>( P->Player ) ) TravelURL = TEXT("");`
                let player = unsafe { ptr::addr_of!((*pawn).player).read_unaligned() }.cast::<UViewport>();
                if !player.is_null() {
                    unsafe {
                        access.assign_fstring(ptr::addr_of_mut!((*player).travel_url), &[0])?;
                    }
                }
                result.push((index as usize, actor.cast::<AActor>()));
            }
            index += 1;
        }
        Ok(result)
    };
    let mut map = unsafe { build_travel_info(access, engine, level, mode, players)? };
    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let mut url = unsafe {
        let mut url = std::mem::MaybeUninit::<hp2_abi::engine::generated::FURL>::uninit();
        crate::url_owner::UrlOwner::construct_default(access, &defaults, url.as_mut_ptr())?;
        url.assume_init()
    };
    let parse = unsafe {
        crate::url_owner::UrlOwner::parse_update(
            access,
            &defaults,
            &mut url,
            ptr::addr_of_mut!((*engine).last_url),
            next_url.data.cast::<u32>(),
            TRAVEL_RELATIVE,
        )
    };
    let mut error = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let browse = if parse.is_ok() {
        unsafe {
            crate::game_travel_owner::browse(
                access,
                engine,
                &url,
                if mode == 2 { ptr::null() } else { &map },
                &mut error,
            )
        }
    } else {
        Ok(false)
    };
    let clear = unsafe {
        access.assign_fstring(ptr::addr_of_mut!((*info).next_url), &[0])
    };
    let error_cleanup = unsafe { access.destroy_owned_fstring(&mut error) };
    let url_cleanup = unsafe { crate::url_owner::UrlOwner::destroy(access, &mut url) };
    let map_cleanup = unsafe { crate::level_owner::destroy_travel_map(access, &mut map) };
    parse?;
    browse?;
    clear.and(error_cleanup).and(url_cleanup).and(map_cleanup)
}

/// Source `UGameEngine::ClientTravel` browse for the `Tick` TravelURL branch.
unsafe fn client_travel_browse(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    viewport: *mut UViewport,
    mode: i32,
) -> RuntimeResult<()> {
    if viewport.is_null() {
        return Err(RuntimeError::new("ClientTravel Tick branch has no viewport"));
    }
    let level = unsafe { ptr::addr_of!((*engine).g_level).read_unaligned() }.cast::<ULevel>();
    if level.is_null() {
        return Err(RuntimeError::new("ClientTravel Tick branch has no GLevel"));
    }
    let travel_url = unsafe { ptr::addr_of!((*viewport).travel_url).read_unaligned() };
    if travel_url.data.is_null() {
        return Err(RuntimeError::new("ClientTravel Tick branch has no TravelURL text"));
    }
    let actor = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned() };
    if mode == 1 {
        let mut message: Vec<u32> = "Export travel for: ".chars().map(u32::from).collect();
        let info = unsafe {
            ptr::addr_of!((*actor.cast::<APlayerPawn>()).base.player_replication_info).read_unaligned()
        }.cast::<APlayerReplicationInfo>();
        if !info.is_null() {
            let name = unsafe { ptr::addr_of!((*info).player_name).read_unaligned() };
            if !name.data.is_null() {
                let units = unsafe { std::slice::from_raw_parts(name.data.cast::<u32>(), name.num.max(0) as usize) };
                message.extend(units.iter().copied().take_while(|unit| *unit != 0));
            }
        }
        let category = access.intern_str("Log", FindName::Add)?;
        access.emit_log(category, &message)?;
    }
    let players = || -> RuntimeResult<Vec<(usize, *mut hp2_abi::engine::generated::AActor)>> {
        if mode == 1 && !actor.is_null() {
            Ok(vec![(0, actor.cast::<AActor>())])
        } else {
            Ok(Vec::new())
        }
    };
    let mut map = unsafe { build_travel_info(access, engine, level, mode, players)? };
    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let mut url = unsafe {
        let mut url = std::mem::MaybeUninit::<hp2_abi::engine::generated::FURL>::uninit();
        crate::url_owner::UrlOwner::construct_default(access, &defaults, url.as_mut_ptr())?;
        url.assume_init()
    };
    let travel_type = unsafe { ptr::addr_of!((*viewport).travel_type).read_unaligned() };
    let parse = unsafe {
        crate::url_owner::UrlOwner::parse_update(
            access,
            &defaults,
            &mut url,
            ptr::addr_of_mut!((*engine).last_url),
            travel_url.data.cast::<u32>(),
            travel_type,
        )
    };
    let mut error = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let browse = if parse.is_ok() {
        unsafe {
            crate::game_travel_owner::browse(
                access,
                engine,
                &url,
                if mode == 2 { ptr::null() } else { &map },
                &mut error,
            )
        }
    } else {
        Ok(false)
    };
    let clear = unsafe { access.assign_fstring(ptr::addr_of_mut!((*viewport).travel_url), &[0]) };
    let error_cleanup = unsafe { access.destroy_owned_fstring(&mut error) };
    let url_cleanup = unsafe { crate::url_owner::UrlOwner::destroy(access, &mut url) };
    let map_cleanup = unsafe { crate::level_owner::destroy_travel_map(access, &mut map) };
    parse?;
    browse?;
    clear.and(error_cleanup).and(url_cleanup).and(map_cleanup)
}

impl crate::game_tick::tick::TickOps for CoreGameTickOps<'_> {
    type Error = RuntimeError;

    unsafe fn replay_paused(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let engine = self.engine(engine)?;
        Ok(i32::from(unsafe { crate::replay_owner::paused(&(*engine).replay) }))
    }
    unsafe fn replay_serialize_frame_tick(
        &self,
        engine: *mut std::ffi::c_void,
        delta: f32,
    ) -> RuntimeResult<f32> {
        let engine = self.engine(engine)?;
        let mut tick = delta;
        let _ = unsafe {
            crate::replay_owner::serialize_frame_tick(
                self.access,
                ptr::addr_of_mut!((*engine).replay),
                &mut tick,
            )?
        };
        Ok(tick)
    }
    unsafe fn app_cycles(&self) -> RuntimeResult<u32> {
        Ok(hp2_core::unix_clock::cycles())
    }
    unsafe fn log_all_windows_closed(&self) -> RuntimeResult<()> {
        log_all_windows_closed(self.access)
    }
    unsafe fn app_request_exit(&self, _code: i32) -> RuntimeResult<()> {
        hp2_core::unix_signal::set_exit_requested(true);
        Ok(())
    }
    unsafe fn viewport_set_mouse_capture(
        &self,
        viewport: *mut std::ffi::c_void,
        capture: i32,
        clip: i32,
        focus_only: i32,
    ) -> RuntimeResult<()> {
        if viewport.is_null() {
            return Err(RuntimeError::new("Tick pause toggle has no viewport"));
        }
        unsafe {
            crate::platform_owner::set_mouse_capture(
                self.access,
                viewport.cast::<UViewport>(),
                capture,
                clip,
                focus_only,
            )
        }
    }
    unsafe fn static_tick(&self) -> RuntimeResult<()> {
        static_tick(self.access)
    }
    unsafe fn gcache_tick(&self) -> RuntimeResult<()> {
        cache_tick(self.access)
    }
    unsafe fn level_tick_all(
        &self,
        level: *mut std::ffi::c_void,
        delta: f32,
    ) -> RuntimeResult<()> {
        unsafe { crate::level_tick_owner::tick_all(self.access, level.cast::<ULevel>(), delta) }
    }
    unsafe fn g_repair_save_pending(&self) -> RuntimeResult<i32> {
        repair_save_pending(self.access)
    }
    unsafe fn g_repair_save_slot(&self) -> RuntimeResult<i32> {
        repair_save_slot(self.access)
    }
    unsafe fn set_g_repair_save_pending(&self, value: i32) -> RuntimeResult<()> {
        set_repair_save_pending(self.access, value)
    }
    unsafe fn set_g_repair_save_slot(&self, value: i32) -> RuntimeResult<()> {
        set_repair_save_slot(self.access, value)
    }
    unsafe fn engine_save_game(
        &self,
        engine: *mut std::ffi::c_void,
        slot: i32,
    ) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        unsafe { crate::game_engine_owner::save_game(self.access, engine, slot) }
    }
    unsafe fn log_save_repair(&self, slot: i32) -> RuntimeResult<()> {
        self.log_line(&format!("Save repair completed: {slot}"))
    }
    unsafe fn make_directory(
        &self,
        path: *const std::ffi::c_void,
        tree: i32,
    ) -> RuntimeResult<()> {
        // `GFileManager->MakeDirectory(Path, Tree)`: the source ignores the boolean.
        let path = unsafe { crate::game_save_owner::path_from_wide(path.cast())? };
        let result = if tree != 0 { std::fs::create_dir_all(&path) } else { std::fs::create_dir(&path) };
        let _ = result;
        Ok(())
    }
    unsafe fn system_save_path(&self) -> RuntimeResult<*const std::ffi::c_void> {
        let system = self.access.system()?;
        let path = unsafe { ptr::addr_of!((*system).save_path).read_unaligned() };
        Ok(path.data.cast())
    }
    unsafe fn system_save_slot_path(&self) -> RuntimeResult<*const std::ffi::c_void> {
        let system = self.access.system()?;
        let path = unsafe { ptr::addr_of!((*system).save_slot_path).read_unaligned() };
        Ok(path.data.cast())
    }
    unsafe fn log_saving_game(&self, filename: *const std::ffi::c_void) -> RuntimeResult<()> {
        let path = unsafe { crate::game_save_owner::path_from_wide(filename.cast::<u32>())? };
        self.log_line(&format!("Saving game...(before call) Filename: {}", path.display()))
    }
    unsafe fn engine_exec_restart_sounds(
        &self,
        engine: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        let mut output = |message: &[u32]| -> RuntimeResult<()> {
            let category = self.access.intern_str("Log", FindName::Add)?;
            self.access.emit_log(category, message)
        };
        let command = hp2_core::lit!("RestartSounds");
        let _ = unsafe {
            crate::game_exec_owner::exec(self.access, engine, command.as_ptr(), &mut output)?
        };
        Ok(())
    }
    unsafe fn engine_flush(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        unsafe { crate::engine_owner::flush(self.access, engine.cast::<UEngine>(), true) }
    }
    unsafe fn server_switch_travel(
        &self,
        engine: *mut std::ffi::c_void,
        mode: i32,
    ) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        unsafe { server_switch_travel(self.access, engine, mode) }
    }
    unsafe fn client_travel_browse(
        &self,
        engine: *mut std::ffi::c_void,
        viewport: *mut std::ffi::c_void,
        mode: i32,
    ) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        unsafe { client_travel_browse(self.access, engine, viewport.cast::<UViewport>(), mode) }
    }
    unsafe fn level_info_next_url_is_restart(
        &self,
        info: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        level_info_next_url_is_restart(info.cast::<ALevelInfo>())
    }
    unsafe fn viewport_travel_is_restart(
        &self,
        viewport: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        viewport_travel_is_restart(viewport.cast::<UViewport>())
    }
    unsafe fn pending_tick(
        &self,
        pending: *mut std::ffi::c_void,
        delta: f32,
    ) -> RuntimeResult<()> {
        unsafe { pending_tick(self.access, pending, delta) }
    }
    unsafe fn pending_set_failure_progress(
        &self,
        engine: *mut std::ffi::c_void,
        pending: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        let error = unsafe { ptr::addr_of!((*pending.cast::<UPendingLevel>()).error).read_unaligned() };
        if error.data.is_null() {
            return Err(RuntimeError::new("Pending level failure has no error text"));
        }
        let localized = self.localize_connection_failed()?;
        unsafe {
            crate::game_engine_owner::set_progress(
                self.access,
                engine,
                localized.as_ptr(),
                error.data.cast::<u32>(),
                4.0,
            )
        }
    }
    unsafe fn pending_log_failure(&self, pending: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let pending = pending.cast::<UPendingLevel>();
        let error = unsafe { ptr::addr_of!((*pending).error).read_unaligned() };
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
        let url = unsafe {
            crate::url_owner::UrlOwner::string(
                self.access,
                &defaults,
                ptr::addr_of!((*pending).url),
                true,
            )?
        };
        let mut message: Vec<u32> = Vec::new();
        if !url.data.is_null() {
            let units = unsafe { std::slice::from_raw_parts(url.data.cast::<u32>(), url.num.max(0) as usize) };
            message.extend(units.iter().copied().take_while(|unit| *unit != 0));
        }
        message.push(u32::from(b' '));
        if !error.data.is_null() {
            let units = unsafe { std::slice::from_raw_parts(error.data.cast::<u32>(), error.num.max(0) as usize) };
            message.extend(units.iter().copied().take_while(|unit| *unit != 0));
        }
        let mut owned = url;
        let cleanup = unsafe { self.access.destroy_owned_fstring(&mut owned) };
        let category = self.access.intern_str("Log", FindName::Add)?;
        let result = self.access.emit_log(category, &message);
        cleanup.and(result)
    }
    unsafe fn console_connect_failure(
        &self,
        console: *mut std::ffi::c_void,
        pending: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        if console.is_null() {
            return Err(RuntimeError::new("ConnectFailure has no canonical Console"));
        }
        let pending = pending.cast::<UPendingLevel>();
        let fail_code = unsafe { ptr::addr_of!((*pending).fail_code).read_unaligned() };
        let fail_url = unsafe { ptr::addr_of!((*pending).fail_url).read_unaligned() };
        #[repr(C)]
        struct ConnectFailureParams {
            fail_code: TArrayRaw,
            fail_url: TArrayRaw,
        }
        let mut params = ConnectFailureParams { fail_code, fail_url };
        let object = console.cast::<hp2_abi::engine::generated::UObject>();
        let name = self.access.intern_str("ConnectFailure", FindName::Find)?;
        let function = unsafe { self.access.find_object_function_by_name(object, name, false)? };
        if function.is_null() {
            return Err(RuntimeError::new("Console ConnectFailure function is missing"));
        }
        unsafe {
            self.access.execute_event(
                object,
                function,
                (&mut params as *mut ConnectFailureParams).cast(),
                ptr::null_mut(),
            )
        }
    }
    unsafe fn pending_level_delete(&self, pending: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { pending_level_delete(self.access, pending.cast::<UPendingLevel>()) }
    }
    unsafe fn pending_load_scope(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        unsafe { pending_load_scope(self.access, engine) }
    }
    unsafe fn client_tick(&self, client: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { crate::platform_owner::client_tick(self.access, client.cast::<hp2_abi::engine::generated::USDLClient>()) }
    }
    unsafe fn viewport_event_view_flash(
        &self,
        viewport: *mut std::ffi::c_void,
        delta: f32,
    ) -> RuntimeResult<()> {
        unsafe { viewport_event_view_flash(self.access, viewport.cast::<UViewport>(), delta) }
    }
    unsafe fn replay_replaying(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let engine = self.engine(engine)?;
        Ok(i32::from(unsafe { crate::replay_owner::replaying(&(*engine).replay) }))
    }
    unsafe fn replay_serialize_frame_input(
        &self,
        engine: *mut std::ffi::c_void,
        key: &mut i32,
        state: &mut i32,
        delta: &mut f32,
    ) -> RuntimeResult<i32> {
        let engine = self.engine(engine)?;
        let mut event = hp2_abi::engine::generated::FReplayInputEvent {
            i_key: 0,
            state: 0,
            delta: 0.0,
        };
        let ok = unsafe {
            crate::replay_owner::serialize_frame_input(
                self.access,
                ptr::addr_of_mut!((*engine).replay),
                &mut event,
            )?
        };
        if ok {
            *key = event.i_key;
            *state = event.state;
            *delta = event.delta;
        }
        Ok(i32::from(ok))
    }
    unsafe fn engine_key(
        &self,
        engine: *mut std::ffi::c_void,
        viewport: *mut std::ffi::c_void,
        key: i32,
    ) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        let handled = unsafe {
            crate::uengine::key_core(self.access, viewport.cast::<UViewport>(), key)?
        };
        if handled && unsafe { crate::replay_owner::recording(&(*engine).replay) } {
            let _ = unsafe {
                crate::replay_owner::serialize_frame_input_write(
                    self.access,
                    ptr::addr_of_mut!((*engine).replay),
                    key,
                    IST_MAX,
                    0.0,
                )?
            };
        }
        Ok(())
    }
    unsafe fn engine_input_event(
        &self,
        engine: *mut std::ffi::c_void,
        viewport: *mut std::ffi::c_void,
        key: i32,
        state: i32,
        delta: f32,
    ) -> RuntimeResult<()> {
        let engine = self.engine(engine)?;
        let replay = ptr::addr_of_mut!((*engine).replay);

        // Special pause toggle on the Delete key.
        if state == IST_PRESS && key == IK_DELETE {
            let level = unsafe { ptr::addr_of!((*engine).g_level).read_unaligned() }.cast::<ULevel>();
            if level.is_null() {
                return Err(RuntimeError::new("InputEvent special pause has no GLevel"));
            }
            // `Level->bInSpecialPauseMode = !Level->bInSpecialPauseMode;`
            let paused = unsafe {
                ptr::addr_of!((*level).b_in_special_pause_mode).read_unaligned()
            };
            unsafe {
                ptr::addr_of_mut!((*level).b_in_special_pause_mode)
                    .write_unaligned(i32::from(paused == 0));
            }
        }

        if unsafe { crate::replay_owner::replaying(replay) }
            && !unsafe { crate::replay_owner::in_replay(replay) }
        {
            // Ignore real input, except for replay control keys.
            if state == IST_PRESS && key == IK_ESCAPE {
                let _ = unsafe { crate::replay_owner::stop(self.access, replay)? };
            } else if state == IST_PRESS && key == IK_PAUSE {
                let paused = unsafe { crate::replay_owner::paused(replay) };
                unsafe { crate::replay_owner::pause(replay, !paused, 0) };
            } else if state == IST_PRESS
                && key == IK_SCROLL_LOCK
                && unsafe { crate::replay_owner::paused(replay) }
            {
                unsafe { crate::replay_owner::pause(replay, true, 1) };
            }
            return Ok(());
        } else if unsafe { crate::replay_owner::recording(replay) }
            && state == IST_PRESS
            && key == IK_BACKSLASH
        {
            let _ = unsafe { crate::replay_owner::stop(self.access, replay)? };
            return Ok(());
        }

        let _ = unsafe {
            crate::uengine::input_event_core(self.access, viewport.cast::<UViewport>(), key, state, delta)?
        };
        Ok(())
    }
    unsafe fn g_ticks_increment(&self) -> RuntimeResult<()> {
        increment_ticks();
        Ok(())
    }
    unsafe fn string_nonempty(&self, value: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        unsafe { string_nonempty(value.cast::<TArrayRaw>()) }
    }
    unsafe fn level_info_pauser_nonempty(
        &self,
        info: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        unsafe { level_info_pauser_nonempty(info.cast::<ALevelInfo>()) }
    }
    unsafe fn pending_error_nonempty(
        &self,
        pending: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        unsafe { pending_error_nonempty(pending.cast::<UPendingLevel>()) }
    }
    unsafe fn pending_fail_code_nonempty(
        &self,
        pending: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        unsafe { pending_fail_code_nonempty(pending.cast::<UPendingLevel>()) }
    }
    unsafe fn level_info_next_url_nonempty(
        &self,
        info: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        unsafe { level_info_next_url_nonempty(info.cast::<ALevelInfo>()) }
    }
    unsafe fn viewport_travel_url_nonempty(
        &self,
        viewport: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        unsafe { viewport_travel_url_nonempty(viewport.cast::<UViewport>()) }
    }
    unsafe fn level_info(
        &self,
        level: *mut std::ffi::c_void,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { level_info(level.cast::<ULevel>())? }.cast())
    }
}

/// Runs the complete canonical `UGameEngine::Tick` body.
///
/// # Safety
/// `engine` is a live Core-owned GameEngine; every record reached by the source
/// policy is live canonical storage for the synchronous call.
pub(crate) unsafe fn tick(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    delta_seconds: f32,
) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine in Tick"));
    }
    hp2_audio::verification::advance(delta_seconds)
        .map_err(|error| RuntimeError::new(&error).with_context("Advancing verification audio"))?;
    let ops = CoreGameTickOps { access, retained: RefCell::new(Vec::new()) };
    unsafe { crate::game_tick::tick::tick_with(&ops, engine.cast(), delta_seconds) }
}
