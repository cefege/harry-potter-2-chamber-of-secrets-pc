//! Canonical providers for the source's script console/travel/client-audio policies.
use crate::script_console::{self, *};
use hp2_abi::{FVector, engine::generated::{AActor, APlayerPawn, FFrame, TArrayRaw, UALAudioSubsystem, UEngine, ULevelBase, UObject, UViewport}};
use hp2_core::{canonical_vm_native, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use std::{ffi::c_void, ptr};

struct Calls<'a> { access: CoreAccess<'a>, error: Option<RuntimeError> }
unsafe fn access<'a>(context: *mut c_void) -> CoreAccess<'a> { unsafe { (*context.cast::<Calls<'a>>()).access } }
unsafe fn complete(context: *mut c_void, result: RuntimeResult<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => { let calls = unsafe { &mut *context.cast::<Calls<'_>>() }; if calls.error.is_none() { calls.error = Some(error); } 1 }
    }
}
fn run(access: CoreAccess<'_>, operation: impl FnOnce(*mut c_void) -> i32) -> RuntimeResult<()> {
    let mut calls = Calls { access, error: None };
    let status = operation((&raw mut calls).cast());
    match calls.error {
        Some(error) => Err(error),
        None if status == 0 => Ok(()),
        None => Err(RuntimeError::new("Canonical script console provider returned an uncaptured failure")),
    }
}
unsafe fn text<'a>(value: *const c_void) -> &'a [u32] {
    let value = value.cast::<u32>(); let mut length = 0;
    unsafe { while value.add(length).read() != 0 { length += 1; } std::slice::from_raw_parts(value, length) }
}
unsafe fn is_a(access: CoreAccess<'_>, object: *mut c_void, class: &str) -> RuntimeResult<bool> {
    let class = access.native_class(class)?;
    Ok(unsafe { hp2_core::object_model::is_a(object.cast(), class.cast()) })
}
unsafe fn actor_engine(actor: *mut AActor) -> RuntimeResult<*mut UEngine> {
    let level = unsafe { (*actor).x_level.cast::<ULevelBase>() };
    if level.is_null() { return Err(RuntimeError::new("Script console actor has no Level")); }
    let engine = unsafe { (*level).engine.cast::<UEngine>() };
    if engine.is_null() { return Err(RuntimeError::new("Script console actor Level has no Engine")); }
    Ok(engine)
}
unsafe extern "C" fn exec(context: *mut c_void, actor: *mut c_void, command: *const c_void, out: *mut c_void) -> i32 {
    let access = unsafe { access(context) };
    let result = unsafe { (|| {
        let engine = actor_engine(actor.cast())?;
        collect_command(access, out, |output| crate::engine_exec_owner::exec_virtual(access, engine, command.cast(), output))
    })() };
    unsafe { complete(context, result) }
}

unsafe fn collect_command(
    access: CoreAccess<'_>, out: *mut c_void,
    execute: impl FnOnce(&mut crate::exec_output::ExecOutput<'_>) -> RuntimeResult<bool>,
) -> RuntimeResult<()> {
    let mut collected = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let result = unsafe { (|| {
        let mut output = |value: &[u32]| access.append_vm_fstring_result(&raw mut collected, hp2_core::native_text::tchar_units(value));
        execute(&mut output)?;
        // FStringOutputDevice concatenates payloads without separators. Transfer
        // its final allocation only after Exec succeeds, retaining the old result on error.
        let previous = out.cast::<TArrayRaw>().read_unaligned();
        out.cast::<TArrayRaw>().write_unaligned(collected);
        collected = previous;
        Ok(())
    })() };
    let release = unsafe { access.destroy_owned_fstring(&raw mut collected) };
    result.and(release)
}
unsafe extern "C" fn log(context: *mut c_void, frame: *mut c_void, message: *const c_void) -> i32 {
    let result = unsafe { canonical_vm_native::log_frame_message(access(context), frame.cast::<FFrame>(), text(message)) };
    unsafe { complete(context, result) }
}
unsafe extern "C" fn destroy(context: *mut c_void, actor: *mut c_void) -> i32 {
    let access = unsafe { access(context) }; let actor = actor.cast::<AActor>();
    let result = unsafe { crate::level_lifecycle_owner::destroy_actor(access, (*actor).x_level.cast(), actor, false) }.map(|_| ());
    unsafe { complete(context, result) }
}
unsafe extern "C" fn pre_travel(context: *mut c_void, pawn: *mut c_void) -> i32 {
    let access = unsafe { access(context) }; let object = pawn.cast::<UObject>();
    let result = unsafe { (|| {
        let name = access.intern_str("PreClientTravel", FindName::Find)?;
        let function = access.find_object_function_by_name(object, name, false)?;
        if function.is_null() { return Err(RuntimeError::new("Failed to find checked function PreClientTravel")); }
        crate::actor_owner::process_event(access, object.cast(), function, ptr::null_mut(), ptr::null_mut())
    })() };
    unsafe { complete(context, result) }
}
unsafe extern "C" fn set_travel(context: *mut c_void, pawn: *mut c_void, url: *const c_void, items: i32, travel: u8) -> i32 {
    let access = unsafe { access(context) }; let pawn = pawn.cast::<APlayerPawn>();
    let result = unsafe { (|| {
        let engine = actor_engine(pawn.cast())?;
        let game = access.native_class("Engine.GameEngine")?;
        if !hp2_core::object_model::is_a(engine.cast(), game.cast()) {
            return Err(RuntimeError::new("Pure virtual UEngine::SetClientTravel invoked"));
        }
        crate::game_engine_owner::set_client_travel(access, (*pawn).player.cast(), url.cast(), items, i32::from(travel))
    })() };
    unsafe { complete(context, result) }
}
unsafe extern "C" fn is_connection(context: *mut c_void, player: *mut c_void, out: *mut i32) -> i32 {
    let result = unsafe { is_a(access(context), player, "Engine.NetConnection") }.map(|value| unsafe { out.write(i32::from(value)); });
    unsafe { complete(context, result) }
}
unsafe extern "C" fn remote_address(context: *mut c_void, connection: *mut c_void, out: *mut c_void) -> i32 {
    let access = unsafe { access(context) };
    let result = unsafe { (|| {
        // DemoRecConnection is the concrete transport supplied by this source tree;
        // its LowLevelGetRemoteAddress returns TEXT(""). NetConnection is abstract.
        if !is_a(access, connection, "Engine.DemoRecConnection")? {
            return Err(RuntimeError::new("Pure virtual UNetConnection::LowLevelGetRemoteAddress invoked"));
        }
        access.write_vm_fstring_result(out.cast(), &[])
    })() };
    unsafe { complete(context, result) }
}
unsafe extern "C" fn empty_address(context: *mut c_void, out: *mut c_void) -> i32 {
    let result = unsafe { access(context).write_vm_fstring_result(out.cast(), &[]) };
    unsafe { complete(context, result) }
}
unsafe extern "C" fn copy(context: *mut c_void, _: *const c_void) -> i32 {
    // The script uses appClipboardCopy, not USDLClient's separate SDL clipboard.
    hp2_core::unix_process::clipboard_copy();
    unsafe { complete(context, Ok(())) }
}
unsafe extern "C" fn paste(context: *mut c_void, out: *mut c_void) -> i32 {
    hp2_core::unix_process::clipboard_paste();
    unsafe { empty_address(context, out) }
}
fn player_services(context: *mut c_void) -> Hp2ScriptPlayerPawnServices {
    Hp2ScriptPlayerPawnServices { context, pre_client_travel: pre_travel, set_client_travel: set_travel, is_net_connection: is_connection,
        get_remote_address: remote_address, empty_remote_address: empty_address, copy_to_clipboard: copy, paste_from_clipboard: paste }
}
pub(crate) unsafe fn console_command(access: CoreAccess<'_>, actor: *mut AActor, command: *const c_void, out: *mut c_void) -> RuntimeResult<()> {
    run(access, |context| unsafe { exec(context, actor.cast(), command, out) })
}
pub(crate) unsafe fn error(access: CoreAccess<'_>, actor: *mut AActor, frame: *mut FFrame, message: *const c_void) -> RuntimeResult<()> {
    run(access, |context| unsafe { script_console::error(&Hp2ScriptErrorServices { context, log, destroy }, actor.cast(), frame.cast(), message) })
}
pub(crate) unsafe fn client_travel(access: CoreAccess<'_>, pawn: *mut APlayerPawn, url: *const c_void, items: i32, travel: u8) -> RuntimeResult<()> {
    run(access, |context| unsafe { script_console::client_travel(&player_services(context), pawn.cast(), url, items, travel) })
}
pub(crate) unsafe fn network_address(access: CoreAccess<'_>, pawn: *mut APlayerPawn, out: *mut c_void) -> RuntimeResult<()> {
    run(access, |context| unsafe { script_console::player_network_address(&player_services(context), pawn.cast(), out) })
}
pub(crate) unsafe fn clipboard_copy(access: CoreAccess<'_>, value: *const c_void) -> RuntimeResult<()> {
    run(access, |context| unsafe { (player_services(context).copy_to_clipboard)(context, value) })
}
pub(crate) unsafe fn clipboard_paste(access: CoreAccess<'_>, out: *mut c_void) -> RuntimeResult<()> {
    run(access, |context| unsafe { (player_services(context).paste_from_clipboard)(context, out) })
}
pub(crate) unsafe fn level_url(access: CoreAccess<'_>, actor: *mut AActor, out: *mut c_void, address: bool) -> RuntimeResult<()> {
    unsafe {
        let level = (*actor).x_level;
        if level.is_null() { return Err(RuntimeError::new("LevelInfo URL has no Level")); }
        let url = ptr::addr_of!((*level.cast::<ULevelBase>()).url);
        if address {
            let host = ptr::addr_of!((*url).host).read_unaligned();
            let host = if host.num <= 1 || host.data.is_null() { &[][..] } else { std::slice::from_raw_parts(host.data.cast::<u32>(), host.num as usize - 1) };
            let port = (*url).port;
            let mut buffer = [0; 11];
            let decimal = crate::exec_output::decimal(port, &mut buffer);
            access.write_vm_fstring_parts(out.cast(), &[
                hp2_core::native_text::tchar_units(host), &[i32::from(b':')],
                hp2_core::native_text::tchar_units(decimal),
            ])
        } else {
            let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
            let mut value = crate::url_owner::UrlOwner::string(access, &defaults, url, false)?;
            let previous = out.cast::<TArrayRaw>().read_unaligned();
            out.cast::<TArrayRaw>().write_unaligned(value);
            value = previous;
            access.destroy_owned_fstring(&raw mut value)
        }
    }
}
unsafe extern "C" fn is_player_pawn(context: *mut c_void, pawn: *mut c_void, out: *mut i32) -> i32 {
    let result = unsafe { is_a(access(context), pawn, "Engine.PlayerPawn") }.map(|value| unsafe { out.write(i32::from(value)); });
    unsafe { complete(context, result) }
}
unsafe extern "C" fn is_viewport(context: *mut c_void, player: *mut c_void, out: *mut i32) -> i32 {
    let result = unsafe { is_a(access(context), player, "Engine.Viewport") }.map(|value| unsafe { out.write(i32::from(value)); });
    unsafe { complete(context, result) }
}
unsafe extern "C" fn audio(context: *mut c_void, pawn: *mut c_void, out: *mut *mut c_void) -> i32 {
    let result = unsafe { actor_engine(pawn.cast()) }.map(|engine| unsafe { out.write((*engine).audio.cast()); });
    unsafe { complete(context, result) }
}
unsafe extern "C" fn owned(context: *mut c_void, actor: *mut c_void, owner: *mut c_void, out: *mut i32) -> i32 {
    unsafe { out.write(crate::actor::is_owned_by(actor.cast(), owner.cast())); complete(context, Ok(())) }
}
unsafe extern "C" fn radius(context: *mut c_void, out: *mut f32) -> i32 {
    unsafe { out.write(hp2_core::audio_globals::GAudioDefaultRadius); complete(context, Ok(())) }
}
unsafe extern "C" fn play(context: *mut c_void, audio: *mut c_void, actor: *mut c_void, id: i32, sound: *mut c_void, location: *const c_void, volume: f32, radius: f32, pitch: f32, flags: i32) -> i32 {
    let point = unsafe { location.cast::<FVector>().read_unaligned() };
    let request = hp2_audio::playback::PlayRequest { actor: actor as usize, id, sound: sound as usize,
        location: [point.x, point.y, point.z], volume, radius, pitch, flags, fade_duration: 0., in_priority: 0. };
    let result = crate::audio_owner::play_sound(unsafe { access(context) }, audio.cast::<UALAudioSubsystem>(), &request).map(|_| ());
    unsafe { complete(context, result) }
}
pub(crate) unsafe fn client_hear_sound(access: CoreAccess<'_>, pawn: *mut c_void, actor: *mut c_void, id: i32, sound: *mut c_void, location: FVector, parameters: FVector, disable_3d: i32, looping: i32) -> RuntimeResult<()> {
    run(access, |context| unsafe {
        script_console::client_hear_sound(&Hp2ScriptClientHearSoundServices { context, is_player_pawn, is_viewport, get_audio: audio,
            is_owned_by: owned, audio_default_radius: radius, play_sound: play }, pawn, actor, id, sound, location, parameters, disable_3d, looping)
    })
}

struct PlayerConsole<'owner, 'borrow, 'output> {
    access: CoreAccess<'owner>,
    pawn: *mut APlayerPawn,
    command: *const u32,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
}
impl crate::pawn_native_files::ConsoleCommandOps for PlayerConsole<'_, '_, '_> {
    type Error = RuntimeError;
    type Output = bool;
    unsafe fn player_exec(&mut self) -> RuntimeResult<bool> {
        unsafe { crate::player_exec_owner::exec(self.access, (*self.pawn).player.cast(), self.command, self.output) }
    }
    unsafe fn engine_exec(&mut self) -> RuntimeResult<bool> {
        let engine = unsafe { actor_engine(self.pawn.cast())? };
        unsafe { crate::engine_exec_owner::exec_virtual(self.access, engine, self.command, self.output) }
    }
}
pub(crate) unsafe fn player_console_command(
    access: CoreAccess<'_>, pawn: *mut APlayerPawn, command: *const c_void, out: *mut c_void,
) -> RuntimeResult<()> {
    unsafe { collect_command(access, out, |output| {
        let mut ops = PlayerConsole { access, pawn, command: command.cast(), output };
        crate::pawn_native_files::console_command_with(&mut ops, pawn)
    }) }
}
