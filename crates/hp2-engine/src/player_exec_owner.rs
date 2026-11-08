//! Canonical UPlayer::Exec routes over Core-owned object records.

use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, ULevel, UPlayer};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
};
use std::{ffi::c_void, marker::PhantomData, ptr};

struct Command<'borrow, 'output> {
    cmd: *const u32,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
}

struct CoreExecOps<'access, 'borrow, 'output> {
    access: CoreAccess<'access>,
    _command: PhantomData<&'borrow mut crate::exec_output::ExecOutput<'output>>,
}

impl<'access, 'borrow, 'output> crate::player::PlayerExecOps
    for CoreExecOps<'access, 'borrow, 'output>
{
    type Command = Command<'borrow, 'output>;
    type Error = RuntimeError;

    unsafe fn actor(&mut self, player: *mut c_void) -> RuntimeResult<*mut c_void> {
        if player.is_null() { return Err(RuntimeError::new("Null canonical UPlayer")); }
        Ok(unsafe { ptr::addr_of!((*player.cast::<UPlayer>()).actor).read_unaligned() })
    }

    unsafe fn actor_level(&mut self, actor: *mut c_void) -> RuntimeResult<*mut c_void> {
        if actor.is_null() { return Err(RuntimeError::new("Null canonical Actor")); }
        Ok(unsafe { ptr::addr_of!((*actor.cast::<AActor>()).level).read_unaligned() })
    }

    unsafe fn actor_get_level(&mut self, actor: *mut c_void) -> RuntimeResult<*mut c_void> {
        if actor.is_null() { return Err(RuntimeError::new("Null canonical Actor")); }
        // AActor::GetLevel() is the inline XLevel accessor.
        Ok(unsafe { ptr::addr_of!((*actor.cast::<AActor>()).x_level).read_unaligned() })
    }

    unsafe fn level_exec(&mut self, level: *mut c_void, command: &mut Self::Command) -> RuntimeResult<bool> {
        unsafe { crate::level_exec_owner::exec(self.access, level.cast::<ULevel>(), command.cmd, command.output) }
    }

    unsafe fn level_game(&mut self, level_info: *mut c_void) -> RuntimeResult<*mut c_void> {
        if level_info.is_null() { return Err(RuntimeError::new("Null canonical LevelInfo")); }
        Ok(unsafe { ptr::addr_of!((*level_info.cast::<ALevelInfo>()).game).read_unaligned() })
    }

    unsafe fn actor_hud(&mut self, actor: *mut c_void) -> RuntimeResult<*mut c_void> {
        if actor.is_null() { return Err(RuntimeError::new("Null canonical Actor")); }
        Ok(unsafe { ptr::addr_of!((*actor.cast::<APlayerPawn>()).my_hud).read_unaligned() })
    }

    unsafe fn actor_inventory(&mut self, actor: *mut c_void) -> RuntimeResult<*mut c_void> {
        if actor.is_null() { return Err(RuntimeError::new("Null canonical Actor")); }
        Ok(unsafe { ptr::addr_of!((*actor.cast::<AActor>()).inventory).read_unaligned() })
    }

    unsafe fn inventory_next(&mut self, inventory: *mut c_void) -> RuntimeResult<*mut c_void> {
        if inventory.is_null() { return Err(RuntimeError::new("Null canonical Inventory")); }
        Ok(unsafe { ptr::addr_of!((*inventory.cast::<AActor>()).inventory).read_unaligned() })
    }

    unsafe fn engine(&mut self, level: *mut c_void) -> RuntimeResult<*mut c_void> {
        if level.is_null() { return Err(RuntimeError::new("Null canonical Level")); }
        Ok(unsafe { ptr::addr_of!((*level.cast::<ULevel>()).base.engine).read_unaligned() })
    }

    unsafe fn script_console_exec(
        &mut self, object: *mut c_void, command: &mut Self::Command, executor: *mut c_void,
    ) -> RuntimeResult<bool> {
        unsafe { crate::engine_exec_owner::process_console_exec(self.access, object.cast(), executor.cast(), command.cmd, command.output) }
    }

    unsafe fn engine_exec(&mut self, engine: *mut c_void, command: &mut Self::Command) -> RuntimeResult<bool> {
        unsafe { crate::engine_exec_owner::exec_virtual(self.access, engine.cast(), command.cmd, command.output) }
    }
}

/// Executes the canonical source-ordered `UPlayer::Exec` route chain.
///
/// # Safety
/// `player` must be a live canonical UPlayer; all reachable records remain live through reentry.
pub unsafe fn exec<'access, 'borrow, 'output>(
    access: CoreAccess<'access>, player: *mut UPlayer, cmd: *const u32,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
) -> RuntimeResult<bool> {
    if player.is_null() { return Err(RuntimeError::new("Null canonical UPlayer")); }
    let mut command = Command { cmd, output };
    let mut ops = CoreExecOps { access, _command: PhantomData };
    unsafe { crate::player::exec_with_ops(&mut ops, player.cast(), &mut command) }
}
