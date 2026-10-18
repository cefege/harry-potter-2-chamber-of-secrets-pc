//! Shared `#[repr(C)]` types for the fire port. Layouts mirror the C++ structs in
//! `HarryPotter2/Unreal/Fire/Inc/UnFractal.h` byte for byte.
//!
//! Names carry a `Fire` prefix so they cannot collide with the engine's C++
//! names (`LineSeg`, `FSpark`, ...) in translation units that include the
//! cbindgen header.

use core::ffi::c_void;

/// Mirrors `FSpark` (all fields `BYTE`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireSpark {
    pub ty: u8,
    pub heat: u8,
    pub x: u8,
    pub y: u8,
    pub byte_a: u8,
    pub byte_b: u8,
    pub byte_c: u8,
    pub byte_d: u8,
}

/// Mirrors `FDrop` (all fields `BYTE`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireDrop {
    pub ty: u8,
    pub depth: u8,
    pub x: u8,
    pub y: u8,
    pub byte_a: u8,
    pub byte_b: u8,
    pub byte_c: u8,
    pub byte_d: u8,
}

/// Mirrors `LineSeg` (`BYTE Xpos,Ypos,Xlen,Ylen`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireLineSeg {
    pub xpos: u8,
    pub ypos: u8,
    pub xlen: u8,
    pub ylen: u8,
}

/// Trace callbacks supplied by the C++ shell on every call. The C++ side passes
/// the `HP2FireTextureTrace*` wrappers, which do their own opt-in checks, so
/// these are always non-null. Rust tests pass no-op functions.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FireTraceHooks {
    /// `HP2FireTextureTraceInitTables(Before, After, Fingerprint)`
    pub init_tables: extern "C" fn(before: u64, after: u64, fingerprint: u64),
    /// `HP2FireTextureTraceSpeedRand(StateIndex, Value)`
    pub speed_rand: extern "C" fn(state_index: u32, value: u8),
    /// `HP2FireTextureTraceBurnWrite(Texture, SparkIndex, Offset, Before, Value)`
    pub burn_write:
        extern "C" fn(texture: *mut c_void, spark_index: i32, offset: u32, before: u8, value: u8),
}

extern "C" fn noop_init_tables(_: u64, _: u64, _: u64) {}
extern "C" fn noop_speed_rand(_: u32, _: u8) {}
extern "C" fn noop_burn_write(_: *mut c_void, _: i32, _: u32, _: u8, _: u8) {}

impl FireTraceHooks {
    pub const NOOP: FireTraceHooks = FireTraceHooks {
        init_tables: noop_init_tables,
        speed_rand: noop_speed_rand,
        burn_write: noop_burn_write,
    };
}
