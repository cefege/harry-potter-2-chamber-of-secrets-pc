//! Process-global fire state: the shift-register RNG and the derived 8-bit
//! phase tables. Mirrors the `extern "C"` globals at the top of `UnFractal.cpp`
//! (`SpeedRindex`, `StaleRindex`, `SpeedRandArr`, `PhaseTable`,
//! `SignedPhaseTable`, `LightPhaseTable`).

use std::sync::{Mutex, MutexGuard};

use crate::abi::{FireCallbackError, FireTraceHooks};

/// `LIGHTPHASEBIAS` from `UnFractal.cpp`.
pub const LIGHTPHASEBIAS: i32 = 32;

pub struct FireGlobals {
    pub speed_rindex: u32,
    pub stale_rindex: u32,
    pub speed_rand_arr: [u8; 512],
    pub phase_table: [u8; 256],
    pub signed_phase_table: [u8; 256],
    pub light_phase_table: [u8; 256],
}

impl FireGlobals {
    const fn new() -> Self {
        FireGlobals {
            speed_rindex: 0,
            stale_rindex: 0,
            speed_rand_arr: [0; 512],
            phase_table: [0; 256],
            signed_phase_table: [0; 256],
            light_phase_table: [0; 256],
        }
    }

    /// `SpeedRand()` shift-register step (C++ non-ASM path).
    pub fn speed_rand(&mut self) -> u8 {
        self.speed_rindex = (self.speed_rindex + 1) & 63;
        let idx = self.speed_rindex as usize;
        let src = self.speed_rand_arr[idx];
        let dst = &mut self.speed_rand_arr[(idx + 31) & 63];
        *dst ^= src;
        *dst
    }
}

static GLOBALS: Mutex<FireGlobals> = Mutex::new(FireGlobals::new());

/// Locks the fire globals. A poisoned lock is recovered: the state is plain
/// data and every writer leaves it consistent.
pub fn lock() -> MutexGuard<'static, FireGlobals> {
    GLOBALS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Per-invocation hooks never live in the global state. Reentry gets its own
/// session and observes the same RNG/tables without overwriting outer hooks.
pub struct FireSession {
    pub trace: FireTraceHooks,
}

impl FireSession {
    pub fn new(trace: FireTraceHooks) -> Self {
        Self { trace }
    }

    pub fn speed_rand(&mut self) -> Result<u8, FireCallbackError> {
        let (index, value) = {
            let mut globals = lock();
            let value = globals.speed_rand();
            (globals.speed_rindex, value)
        };
        self.trace.speed_rand(index, value)?;
        Ok(value)
    }

    pub fn phase(&self, index: usize) -> u8 {
        lock().phase_table[index]
    }
    pub fn light_phase(&self, index: usize) -> u8 {
        lock().light_phase_table[index]
    }
    pub fn signed_phase(&self, index: usize) -> u8 {
        lock().signed_phase_table[index]
    }
}
