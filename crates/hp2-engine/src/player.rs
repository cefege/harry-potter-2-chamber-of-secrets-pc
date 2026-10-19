//! Rust port of the logic in `HarryPotter2/Unreal/Engine/Src/UnPlayer.cpp` (pinned
//! `5782e7f`): `UPlayer::Destroy` and `UPlayer::Exec`.
//!
//! Every object access goes through [`PlayerServices`], filled in by
//! `Hp2PlayerServices.cpp`. Those thunks are the C++ member reads and virtual calls
//! that the port cannot own. `Super::Destroy`, the constructors, `Serialize`,
//! `StaticConstructor` and `IMPLEMENT_CLASS` stay in the C++ shell.
//!
//! Re-reads: the C++ re-reads the member `Actor` at every `&&` operand, and a callee can
//! clear it (the console command handlers do this through `UPlayer`). Each step below
//! therefore calls `player_actor` fresh at its start. The executor argument reuses that
//! value, because no callee runs between the check and the call.
//!
//! Divergences, all intentional:
//! - `guard`/`unguard` are debugging context only. They stay in the C++ shells.
//! - The C++ `Exec` returns `UBOOL` from `if(...) return 1; else return 0;`. The port
//!   returns 0 or 1 in the same way.
//! - FMA: `UnPlayer.o` (-O3 -g) has no floating-point ops, so there are no fused sites.

use std::ffi::c_void;

use crate::params::TChar;

/// C++ services (`Hp2PlayerServices` in `Hp2PlayerServices.cpp`), same field order.
/// Each entry is an object member read or a virtual call the port cannot own.
#[repr(C)]
pub struct PlayerServices {
    /// `GIsRunning` (UBOOL, nonzero means running).
    pub is_running: unsafe extern "C" fn() -> i32,
    /// `UPlayer::Actor` (APlayerPawn*).
    pub player_actor: unsafe extern "C" fn(player: *mut c_void) -> *mut c_void,
    /// Store `UPlayer::Actor`.
    pub player_set_actor: unsafe extern "C" fn(player: *mut c_void, actor: *mut c_void),
    /// `AActor::GetLevel()` (inline, returns `XLevel`, ULevel*).
    pub actor_get_level: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `AActor::Player = NULL`.
    pub actor_clear_player: unsafe extern "C" fn(actor: *mut c_void),
    /// `ULevel::DestroyActor(AActor*, UBOOL bNetForce)` (virtual). Return value unused.
    pub level_destroy_actor:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, force: i32),
    /// `ULevel::Exec(const TCHAR*, FOutputDevice&)` (virtual).
    pub level_exec:
        unsafe extern "C" fn(level: *mut c_void, cmd: *const TChar, ar: *mut c_void) -> i32,
    /// `AActor::Level` (ALevelInfo*).
    pub actor_level: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `ALevelInfo::Game` (AGameInfo*).
    pub level_game: unsafe extern "C" fn(level_info: *mut c_void) -> *mut c_void,
    /// `UObject::ScriptConsoleExec(const TCHAR*, FOutputDevice&, UObject*)` (virtual).
    pub object_script_console_exec: unsafe extern "C" fn(
        obj: *mut c_void,
        cmd: *const TChar,
        ar: *mut c_void,
        executor: *mut c_void,
    ) -> i32,
    /// `AActor::myHUD` (AHUD*).
    pub actor_hud: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `AActor::Inventory` (first AInventory*).
    pub actor_inventory: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `AInventory::Inventory` (next AInventory*).
    pub inventory_next: unsafe extern "C" fn(inv: *mut c_void) -> *mut c_void,
    /// `ULevel::Engine` (UEngine*).
    pub level_engine: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `UEngine::Exec(const TCHAR*, FOutputDevice&)` (virtual).
    pub engine_exec:
        unsafe extern "C" fn(engine: *mut c_void, cmd: *const TChar, ar: *mut c_void) -> i32,
}

/// `UPlayer::Destroy` before `Super::Destroy()` (UnPlayer.cpp:30-42).
///
/// # Safety
/// `player` must be a live `UPlayer` as far as `svc` is concerned.
pub unsafe fn destroy(svc: &PlayerServices, player: *mut c_void) {
    if unsafe { (svc.is_running)() } != 0 {
        let actor = unsafe { (svc.player_actor)(player) };
        if !actor.is_null() {
            let level = unsafe { (svc.actor_get_level)(actor) };
            unsafe {
                (svc.actor_clear_player)(actor);
                (svc.level_destroy_actor)(level, actor, 1);
                (svc.player_set_actor)(player, std::ptr::null_mut());
            }
        }
    }
}

/// `UPlayer::Exec(const TCHAR* Cmd, FOutputDevice& Ar)` (UnPlayer.cpp:43-74).
///
/// # Safety
/// `player` must be a live `UPlayer`. `cmd` must be NUL-terminated. `ar` must be a live
/// `FOutputDevice`.
pub unsafe fn exec(
    svc: &PlayerServices,
    player: *mut c_void,
    cmd: *const TChar,
    ar: *mut c_void,
) -> i32 {
    // Step 1: Actor->GetLevel()->Exec(Cmd, Ar).
    let actor = unsafe { (svc.player_actor)(player) };
    if !actor.is_null() {
        let level = unsafe { (svc.actor_get_level)(actor) };
        if unsafe { (svc.level_exec)(level, cmd, ar) } != 0 {
            return 1;
        }
    }

    // Step 2: Actor->Level->Game->ScriptConsoleExec(Cmd, Ar, Actor).
    let actor = unsafe { (svc.player_actor)(player) };
    if !actor.is_null() {
        let li = unsafe { (svc.actor_level)(actor) };
        if !li.is_null() {
            let game = unsafe { (svc.level_game)(li) };
            if !game.is_null()
                && unsafe { (svc.object_script_console_exec)(game, cmd, ar, actor) } != 0
            {
                return 1;
            }
        }
    }

    // Step 3: Actor->myHUD->ScriptConsoleExec(Cmd, Ar, Actor).
    let actor = unsafe { (svc.player_actor)(player) };
    if !actor.is_null() {
        let hud = unsafe { (svc.actor_hud)(actor) };
        if !hud.is_null() && unsafe { (svc.object_script_console_exec)(hud, cmd, ar, actor) } != 0 {
            return 1;
        }
    }

    // Step 4: Actor->ScriptConsoleExec(Cmd, Ar, Actor).
    let actor = unsafe { (svc.player_actor)(player) };
    if !actor.is_null() && unsafe { (svc.object_script_console_exec)(actor, cmd, ar, actor) } != 0 {
        return 1;
    }

    // Step 5: for each inventory item, ScriptConsoleExec(Cmd, Ar, Actor). The executor
    // is re-read every iteration, because an earlier item may have cleared Actor.
    let actor = unsafe { (svc.player_actor)(player) };
    if !actor.is_null() {
        let mut inv = unsafe { (svc.actor_inventory)(actor) };
        while !inv.is_null() {
            let executor = unsafe { (svc.player_actor)(player) };
            if unsafe { (svc.object_script_console_exec)(inv, cmd, ar, executor) } != 0 {
                return 1;
            }
            inv = unsafe { (svc.inventory_next)(inv) };
        }
    }

    // Step 6: Actor->GetLevel()->Engine->Exec(Cmd, Ar).
    let actor = unsafe { (svc.player_actor)(player) };
    if !actor.is_null() {
        let level = unsafe { (svc.actor_get_level)(actor) };
        let engine = unsafe { (svc.level_engine)(level) };
        if unsafe { (svc.engine_exec)(engine, cmd, ar) } != 0 {
            return 1;
        }
    }
    0
}
