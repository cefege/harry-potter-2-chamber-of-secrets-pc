//! `UnPlayer.cpp` exports (`crates/hp2-engine/src/player.rs`). The C++
//! `UPlayer::Destroy` and `UPlayer::Exec` shells in `Engine/Src/UnPlayer.cpp` forward
//! here. `svc` is the C++ `Hp2PlayerServices` table.

use std::ffi::c_void;

use hp2_engine::params::TChar;
use hp2_engine::player::{self, PlayerServices};
use hp2_ffi_support::guard;

/// `UPlayer::Destroy` body before `Super::Destroy()` forward.
///
/// # Safety
/// `svc` must point to a live `PlayerServices`. `player` must be a live `UPlayer`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_player_destroy(svc: *const c_void, player: *mut c_void) {
    guard("hp2_rs_engine_player_destroy", (), move || unsafe {
        let Some(svc) = svc.cast::<PlayerServices>().as_ref() else {
            return;
        };
        player::destroy(svc, player);
    })
}

/// `UPlayer::Exec(const TCHAR* Cmd, FOutputDevice& Ar)` forward.
///
/// # Safety
/// `svc` must point to a live `PlayerServices`. `player` must be a live `UPlayer`.
/// `cmd` must be NUL-terminated. `ar` must point to a live `FOutputDevice`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_player_exec(
    svc: *const c_void,
    player: *mut c_void,
    cmd: *const c_void,
    ar: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_player_exec", 0, move || unsafe {
        let Some(svc) = svc.cast::<PlayerServices>().as_ref() else {
            return 0;
        };
        player::exec(svc, player, cmd.cast::<TChar>(), ar)
    })
}
