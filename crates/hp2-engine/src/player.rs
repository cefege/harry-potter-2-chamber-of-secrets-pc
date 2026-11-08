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
//! - Native `guard`/`unguard` remain active exception context in the C++ shells.
//!   Checked callbacks stop on the first error; the shell rethrows inside its guard.
//! - The C++ `Exec` returns `UBOOL` from `if(...) return Ok(1); else return 0;`. The port
//!   returns 0 or 1 in the same way.
//! - FMA: `UnPlayer.o` (-O3 -g) has no floating-point ops, so there are no fused sites.

use std::ffi::c_void;

use crate::params::TChar;

/// C++ services (`Hp2PlayerServices` in `Hp2PlayerServices.cpp`), same field order.
/// Each entry is an object member read or a virtual call the port cannot own.
#[repr(C)]
pub struct PlayerServices {
    /// `GIsRunning` (UBOOL, nonzero means running).
    pub is_running: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `UPlayer::Actor` (APlayerPawn*).
    pub player_actor: unsafe extern "C" fn(player: *mut c_void, out: *mut *mut c_void) -> i32,
    /// Store `UPlayer::Actor`.
    pub player_set_actor: unsafe extern "C" fn(player: *mut c_void, actor: *mut c_void) -> i32,
    /// `AActor::GetLevel()` (inline, returns `XLevel`, ULevel*).
    pub actor_get_level: unsafe extern "C" fn(actor: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `AActor::Player = NULL`.
    pub actor_clear_player: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `ULevel::DestroyActor(AActor*, UBOOL bNetForce)` (virtual). Return value unused.
    pub level_destroy_actor:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, force: i32) -> i32,
    /// `ULevel::Exec(const TCHAR*, FOutputDevice&)` (virtual).
    pub level_exec: unsafe extern "C" fn(
        level: *mut c_void,
        cmd: *const TChar,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `AActor::Level` (ALevelInfo*).
    pub actor_level: unsafe extern "C" fn(actor: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ALevelInfo::Game` (AGameInfo*).
    pub level_game: unsafe extern "C" fn(level_info: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UObject::ScriptConsoleExec(const TCHAR*, FOutputDevice&, UObject*)` (virtual).
    pub object_script_console_exec: unsafe extern "C" fn(
        obj: *mut c_void,
        cmd: *const TChar,
        ar: *mut c_void,
        executor: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `AActor::myHUD` (AHUD*).
    pub actor_hud: unsafe extern "C" fn(actor: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `AActor::Inventory` (first AInventory*).
    pub actor_inventory: unsafe extern "C" fn(actor: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `AInventory::Inventory` (next AInventory*).
    pub inventory_next: unsafe extern "C" fn(inv: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `ULevel::Engine` (UEngine*).
    pub level_engine: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UEngine::Exec(const TCHAR*, FOutputDevice&)` (virtual).
    pub engine_exec: unsafe extern "C" fn(
        engine: *mut c_void,
        cmd: *const TChar,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
}

// Status is checked before the out slot is read. No native state is borrowed.
macro_rules! checked {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = std::mem::MaybeUninit::uninit();
        let status = unsafe { ($f)($($arg,)* out.as_mut_ptr()) };
        if status != 0 { return Err(status); }
        unsafe { out.assume_init() }
    }};
}
macro_rules! checked_void {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let status = unsafe { ($f)($($arg),*) };
        if status != 0 { return Err(status); }
    }};
}

/// Operations required by the source-ordered player destruction transition.
pub(crate) trait PlayerDestroyOps {
    type Error;
    fn is_running(&mut self) -> Result<bool, Self::Error>;
    unsafe fn actor(&mut self, player: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn level(&mut self, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn clear_player(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn destroy_actor(&mut self, level: *mut c_void, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn clear_actor(&mut self, player: *mut c_void) -> Result<(), Self::Error>;
}

/// Executes the native body before qualified UObject destruction.
///
/// # Safety
/// All identities reached by the operations remain live throughout callbacks.
pub(crate) unsafe fn destroy_with_ops<O: PlayerDestroyOps>(
    ops: &mut O, player: *mut c_void,
) -> Result<(), O::Error> {
    if ops.is_running()? {
        let actor = unsafe { ops.actor(player)? };
        if !actor.is_null() {
            let level = unsafe { ops.level(actor)? };
            unsafe {
                ops.clear_player(actor)?;
                ops.destroy_actor(level, actor)?;
                ops.clear_actor(player)?;
            }
        }
    }
    Ok(())
}

struct NativeDestroyOps<'a>(&'a PlayerServices);
impl PlayerDestroyOps for NativeDestroyOps<'_> {
    type Error = i32;
    fn is_running(&mut self) -> Result<bool, i32> {
        Ok(unsafe { checked!(self.0.is_running) } != 0)
    }
    unsafe fn actor(&mut self, player: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.player_actor, player) })
    }
    unsafe fn level(&mut self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.actor_get_level, actor) })
    }
    unsafe fn clear_player(&mut self, actor: *mut c_void) -> Result<(), i32> {
        unsafe { checked_void!(self.0.actor_clear_player, actor); }
        Ok(())
    }
    unsafe fn destroy_actor(&mut self, level: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        unsafe { checked_void!(self.0.level_destroy_actor, level, actor, 1); }
        Ok(())
    }
    unsafe fn clear_actor(&mut self, player: *mut c_void) -> Result<(), i32> {
        unsafe { checked_void!(self.0.player_set_actor, player, std::ptr::null_mut()); }
        Ok(())
    }
}

/// `UPlayer::Destroy` before `Super::Destroy()` (UnPlayer.cpp:30-42).
///
/// # Safety
/// `player` must be a live `UPlayer` as far as `svc` is concerned.
pub unsafe fn destroy(svc: &PlayerServices, player: *mut c_void) -> Result<(), i32> {
    unsafe { destroy_with_ops(&mut NativeDestroyOps(svc), player) }
}

/// Operations for the source-ordered `UPlayer::Exec` route chain.
pub(crate) trait PlayerExecOps {
    type Command;
    type Error;

    unsafe fn actor(&mut self, player: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_level(&mut self, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_get_level(&mut self, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn level_exec(&mut self, level: *mut c_void, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn level_game(&mut self, level_info: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_hud(&mut self, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_inventory(&mut self, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn inventory_next(&mut self, inventory: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn engine(&mut self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn script_console_exec(
        &mut self, object: *mut c_void, command: &mut Self::Command, executor: *mut c_void,
    ) -> Result<bool, Self::Error>;
    unsafe fn engine_exec(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, Self::Error>;
}

/// Executes the shared, source-ordered `UPlayer::Exec` route chain.
///
/// # Safety
/// `player` and every identity returned by `ops` must remain live through each synchronous call.
pub(crate) unsafe fn exec_with_ops<O: PlayerExecOps>(
    ops: &mut O, player: *mut c_void, command: &mut O::Command,
) -> Result<bool, O::Error> {
    let actor = unsafe { ops.actor(player)? };
    if !actor.is_null() {
        let level = unsafe { ops.actor_get_level(actor)? };
        if unsafe { ops.level_exec(level, command)? } {
            return Ok(true);
        }
    }

    let actor = unsafe { ops.actor(player)? };
    if !actor.is_null() {
        let level = unsafe { ops.actor_level(actor)? };
        if !level.is_null() {
            let game = unsafe { ops.level_game(level)? };
            if !game.is_null() && unsafe { ops.script_console_exec(game, command, actor)? } {
                return Ok(true);
            }
        }
    }

    let actor = unsafe { ops.actor(player)? };
    if !actor.is_null() {
        let hud = unsafe { ops.actor_hud(actor)? };
        if !hud.is_null() && unsafe { ops.script_console_exec(hud, command, actor)? } {
            return Ok(true);
        }
    }

    let actor = unsafe { ops.actor(player)? };
    if !actor.is_null() && unsafe { ops.script_console_exec(actor, command, actor)? } {
        return Ok(true);
    }

    let actor = unsafe { ops.actor(player)? };
    if !actor.is_null() {
        let mut inventory = unsafe { ops.actor_inventory(actor)? };
        while !inventory.is_null() {
            let executor = unsafe { ops.actor(player)? };
            if unsafe { ops.script_console_exec(inventory, command, executor)? } {
                return Ok(true);
            }
            inventory = unsafe { ops.inventory_next(inventory)? };
        }
    }

    let actor = unsafe { ops.actor(player)? };
    if !actor.is_null() {
        let level = unsafe { ops.actor_get_level(actor)? };
        let engine = unsafe { ops.engine(level)? };
        if unsafe { ops.engine_exec(engine, command)? } {
            return Ok(true);
        }
    }
    Ok(false)
}

struct NativePlayerExecCommand {
    cmd: *const TChar,
    ar: *mut c_void,
}

struct NativePlayerExecOps<'a>(&'a PlayerServices);
impl PlayerExecOps for NativePlayerExecOps<'_> {
    type Command = NativePlayerExecCommand;
    type Error = i32;

    unsafe fn actor(&mut self, player: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.player_actor, player) })
    }
    unsafe fn actor_level(&mut self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.actor_level, actor) })
    }
    unsafe fn actor_get_level(&mut self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.actor_get_level, actor) })
    }
    unsafe fn level_exec(&mut self, level: *mut c_void, command: &mut Self::Command) -> Result<bool, i32> {
        Ok(unsafe { checked!(self.0.level_exec, level, command.cmd, command.ar) } != 0)
    }
    unsafe fn level_game(&mut self, level: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.level_game, level) })
    }
    unsafe fn actor_hud(&mut self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.actor_hud, actor) })
    }
    unsafe fn actor_inventory(&mut self, actor: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.actor_inventory, actor) })
    }
    unsafe fn inventory_next(&mut self, inventory: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.inventory_next, inventory) })
    }
    unsafe fn engine(&mut self, level: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { checked!(self.0.level_engine, level) })
    }
    unsafe fn script_console_exec(
        &mut self, object: *mut c_void, command: &mut Self::Command, executor: *mut c_void,
    ) -> Result<bool, i32> {
        Ok(unsafe { checked!(self.0.object_script_console_exec, object, command.cmd, command.ar, executor) } != 0)
    }
    unsafe fn engine_exec(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, i32> {
        Ok(unsafe { checked!(self.0.engine_exec, engine, command.cmd, command.ar) } != 0)
    }
}

/// `UPlayer::Exec(const TCHAR* Cmd, FOutputDevice& Ar)` (UnPlayer.cpp:43-74).
///
/// # Safety
/// `player` must be a live `UPlayer`. `cmd` must be NUL-terminated. `ar` must be a live
/// `FOutputDevice`.
pub unsafe fn exec(
    svc: &PlayerServices, player: *mut c_void, cmd: *const TChar, ar: *mut c_void,
) -> Result<i32, i32> {
    let mut command = NativePlayerExecCommand { cmd, ar };
    Ok(i32::from(unsafe {
        exec_with_ops(&mut NativePlayerExecOps(svc), player, &mut command)?
    }))
}
