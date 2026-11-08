//! Source script console, travel and client-sound policy shared by both object graphs.
use std::{ffi::c_void, ptr};
use hp2_abi::{FVector, engine::generated::{AActor, APlayerPawn, USound, UViewport}};

// UnAudio.h sound flags; keep the native signed bitmask passed to PlaySound.
const SF_NO3D: i32 = 16;
const SF_LOOPING: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ScriptConsoleServices {
    pub context: *mut c_void,
    pub exec: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut c_void) -> i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ScriptErrorServices {
    pub context: *mut c_void,
    pub log: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> i32,
    pub destroy: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ScriptPlayerPawnServices {
    pub context: *mut c_void,
    pub pre_client_travel: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub set_client_travel: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, i32, u8) -> i32,
    pub is_net_connection: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub get_remote_address: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub empty_remote_address: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub copy_to_clipboard: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub paste_from_clipboard: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ScriptLevelInfoServices {
    pub context: *mut c_void,
    pub local_url: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub address_url: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ScriptClientHearSoundServices {
    pub context: *mut c_void,
    pub is_player_pawn: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub is_viewport: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub get_audio: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub is_owned_by: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut i32) -> i32,
    pub audio_default_radius: unsafe extern "C" fn(*mut c_void, *mut f32) -> i32,
    pub play_sound: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, i32, *mut c_void, *const c_void, f32, f32, f32, i32) -> i32,
}

/// Logs first; a failed/strict log must not destroy the actor.
/// # Safety
/// Actor, script frame and terminated text remain live through callbacks.
pub unsafe fn error(services: &Hp2ScriptErrorServices, actor: *mut c_void, frame: *mut c_void, text: *const c_void) -> i32 {
    let status = unsafe { (services.log)(services.context, frame, text) };
    if status != 0 { return status; }
    unsafe { (services.destroy)(services.context, actor) }
}
/// A missing Player does not execute PreClientTravel or mutate travel state.
/// # Safety
/// Pawn/text and reentrant event/travel providers remain live.
pub unsafe fn client_travel(services: &Hp2ScriptPlayerPawnServices, pawn: *mut c_void, url: *const c_void, items: i32, travel_type: u8) -> i32 {
    if unsafe { (*pawn.cast::<APlayerPawn>()).player.is_null() } { return 0; }
    let status = unsafe { (services.pre_client_travel)(services.context, pawn) };
    if status != 0 { return status; }
    unsafe { (services.set_client_travel)(services.context, pawn, url, items, travel_type) }
}
/// Local/non-network players return the source's empty network address.
/// # Safety
/// Pawn/player and writable FString result remain live through providers.
pub unsafe fn player_network_address(services: &Hp2ScriptPlayerPawnServices, pawn: *mut c_void, out: *mut c_void) -> i32 {
    let player = unsafe { (*pawn.cast::<APlayerPawn>()).player };
    let mut connection = 0;
    if !player.is_null() {
        let status = unsafe { (services.is_net_connection)(services.context, player, &mut connection) };
        if status != 0 { return status; }
    }
    unsafe {
        if !player.is_null() && connection != 0 { (services.get_remote_address)(services.context, player, out) }
        else { (services.empty_remote_address)(services.context, out) }
    }
}
/// Source ClientHearSound gating, deleted-actor handling, flags and parameter scaling.
/// # Safety
/// Receiver/world/player and any sound reached after the audio gate are live;
/// providers stop on failure and retain no actor references across reentry.
pub unsafe fn client_hear_sound(
    services: &Hp2ScriptClientHearSoundServices, pawn: *mut c_void,
    target: *mut c_void, id: i32, sound: *mut c_void, location: FVector,
    parameters: FVector, disable_3d: i32, looping: i32,
) -> i32 {
    unsafe {
        let mut player_pawn = 0;
        let status = (services.is_player_pawn)(services.context, pawn, &mut player_pawn);
        if status != 0 { return status; }
        if player_pawn == 0 { return 0; }
        let player = (*pawn.cast::<APlayerPawn>()).player;
        if player.is_null() { return 0; }
        let mut viewport = 0;
        let status = (services.is_viewport)(services.context, player, &mut viewport);
        if status != 0 { return status; }
        if viewport == 0 { return 0; }
        let mut audio = ptr::null_mut();
        let status = (services.get_audio)(services.context, pawn, &mut audio);
        if status != 0 { return status; }
        if audio.is_null() { return 0; }
        let actor = if !target.is_null() && (*target.cast::<AActor>()).b_delete_me() { ptr::null_mut() } else { target };
        let mut flags = (*sound.cast::<USound>()).core_flags;
        let mut no_3d = disable_3d != 0;
        if !no_3d && !actor.is_null() {
            let player_actor = (*player.cast::<UViewport>()).actor.cast::<c_void>();
            if actor == player_actor { no_3d = true; }
            else {
                let mut owned = 0;
                let status = (services.is_owned_by)(services.context, actor, player_actor, &mut owned);
                if status != 0 { return status; }
                no_3d = owned != 0;
            }
        }
        if no_3d { flags |= SF_NO3D; }
        if looping != 0 { flags |= SF_LOOPING; }
        let volume = 0.01_f32 * parameters.x;
        let pitch = 0.01_f32 * parameters.z;
        let radius = if parameters.y != 0.0 { parameters.y } else {
            let mut radius = 0.;
            let status = (services.audio_default_radius)(services.context, &mut radius);
            if status != 0 { return status; }
            radius
        };
        (services.play_sound)(services.context, audio, actor, id, sound, (&location as *const FVector).cast(), volume, radius, pitch, flags)
    }
}
