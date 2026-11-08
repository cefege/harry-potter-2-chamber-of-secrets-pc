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

/// Checked native trace callbacks. A missing callback or nonzero status stops
/// the current Rust operation. Native implementations catch with `Hp2FfiCall`;
/// they must never unwind through this ordinary C ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FireTraceHooks {
    pub context: *mut c_void,
    pub init_tables: Option<extern "C" fn(*mut c_void, u64, u64, u64) -> i32>,
    pub speed_rand: Option<extern "C" fn(*mut c_void, u32, u8) -> i32>,
    pub burn_write: Option<extern "C" fn(*mut c_void, *mut c_void, i32, u32, u8, u8) -> i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FireCallbackError;

/// A native-owned arena descriptor. Kernels resolve every access anew
/// through the pointer rather than retaining a slice or an element pointer,
/// so a descriptor whose backing allocation moves between accesses is still
/// read correctly.
#[repr(C)]
pub struct FireArena<T> {
    pub data: *mut T,
    pub len: usize,
}

/// Services used by the `InitTables` decision and loops.
/// `scoped` keeps `FAppRandTracePhaseScope` on a native stack and invokes
/// `hp2_rs_fire_init_tables_scoped` before returning. `publish` is present
/// when a host maintains a shadow copy of the canonical Rust tables.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FireInitServices {
    pub context: *mut c_void,
    pub trace: FireTraceHooks,
    pub ordinal: Option<extern "C" fn(*mut c_void, *mut u64) -> i32>,
    pub scoped: Option<extern "C" fn(*mut c_void, u64) -> i32>,
    pub sine: Option<extern "C" fn(*mut c_void, f32, *mut f64) -> i32>,
    pub random: Option<extern "C" fn(*mut c_void, *mut i32) -> i32>,
    /// Optional copy into host-owned mirror tables; Rust consumers use globals directly.
    pub publish: Option<extern "C" fn(*mut c_void) -> i32>,
}

impl FireTraceHooks {
    pub fn init_tables(
        self,
        before: u64,
        after: u64,
        fingerprint: u64,
    ) -> Result<(), FireCallbackError> {
        checked(self.init_tables.ok_or(FireCallbackError)?(
            self.context,
            before,
            after,
            fingerprint,
        ))
    }

    pub fn speed_rand(self, index: u32, value: u8) -> Result<(), FireCallbackError> {
        checked(self.speed_rand.ok_or(FireCallbackError)?(
            self.context,
            index,
            value,
        ))
    }

    pub fn burn_write(
        self,
        texture: *mut c_void,
        spark: i32,
        offset: u32,
        before: u8,
        value: u8,
    ) -> Result<(), FireCallbackError> {
        checked(self.burn_write.ok_or(FireCallbackError)?(
            self.context,
            texture,
            spark,
            offset,
            before,
            value,
        ))
    }

    pub const NOOP: Self = Self {
        context: core::ptr::null_mut(),
        init_tables: Some(noop_init_tables),
        speed_rand: Some(noop_speed_rand),
        burn_write: Some(noop_burn_write),
    };
}

pub(crate) fn checked(status: i32) -> Result<(), FireCallbackError> {
    if status == 0 {
        Ok(())
    } else {
        Err(FireCallbackError)
    }
}

extern "C" fn noop_init_tables(_: *mut c_void, _: u64, _: u64, _: u64) -> i32 {
    0
}
extern "C" fn noop_speed_rand(_: *mut c_void, _: u32, _: u8) -> i32 {
    0
}
extern "C" fn noop_burn_write(_: *mut c_void, _: *mut c_void, _: i32, _: u32, _: u8, _: u8) -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn fail(_: *mut c_void, _: u32, _: u8) -> i32 {
        17
    }

    #[test]
    fn missing_and_failed_trace_callbacks_are_errors() {
        let mut trace = FireTraceHooks::NOOP;
        trace.speed_rand = None;
        assert_eq!(trace.speed_rand(1, 2), Err(FireCallbackError));
        trace.speed_rand = Some(fail);
        assert_eq!(trace.speed_rand(1, 2), Err(FireCallbackError));
        assert_eq!(FireTraceHooks::NOOP.speed_rand(1, 2), Ok(()));
    }

    extern "C" fn reenter(context: *mut c_void, _: u32, _: u8) -> i32 {
        // A distinct session must not deadlock on an outer global guard, and
        // its no-op hook must not replace the outer invocation's hook.
        if crate::globals::FireSession::new(FireTraceHooks::NOOP)
            .speed_rand()
            .is_err()
        {
            return 1;
        }
        unsafe {
            *context.cast::<u32>() += 1;
        }
        0
    }

    #[test]
    fn rng_callback_can_reenter_without_replacing_outer_hooks() {
        let mut calls = 0u32;
        let trace = FireTraceHooks {
            context: (&mut calls as *mut u32).cast(),
            speed_rand: Some(reenter),
            ..FireTraceHooks::NOOP
        };
        let mut session = crate::globals::FireSession::new(trace);
        assert!(session.speed_rand().is_ok());
        assert!(session.speed_rand().is_ok());
        assert_eq!(calls, 2);
    }

    struct Relocation {
        arena: *mut FireArena<u8>,
        replacement: *mut u8,
        calls: u32,
        first: u8,
    }

    extern "C" fn relocate_then_fail(context: *mut c_void, _: u32, value: u8) -> i32 {
        unsafe {
            let context = &mut *context.cast::<Relocation>();
            context.calls += 1;
            if context.calls == 1 {
                (*context.arena).data = context.replacement;
                context.first = value;
                0
            } else {
                1
            }
        }
    }

    #[test]
    fn callback_relocation_is_live_and_failure_stops_water_writes() {
        let mut old = [77; 8];
        let mut replacement = [99; 8];
        let mut water = FireArena {
            data: old.as_mut_ptr(),
            len: old.len(),
        };
        let mut context = Relocation {
            arena: &raw mut water,
            replacement: replacement.as_mut_ptr(),
            calls: 0,
            first: 0,
        };
        let mut drop_storage = [FireDrop {
            ty: 5,
            ..FireDrop::default()
        }; 2];
        let drops = FireArena {
            data: drop_storage.as_mut_ptr(),
            len: drop_storage.len(),
        };
        let mut state = crate::water::FireWaterState {
            u_size: 4,
            v_size: 4,
            u_bits: 2,
            u_mask: 3,
            v_mask: 3,
            num_drops: 2,
            ..crate::water::FireWaterState::default()
        };
        let trace = FireTraceHooks {
            context: (&mut context as *mut Relocation).cast(),
            speed_rand: Some(relocate_then_fail),
            ..FireTraceHooks::NOOP
        };
        let result = unsafe {
            crate::water::water_redraw_drops(
                trace,
                &raw mut state,
                &raw const drops,
                &raw const water,
            )
        };
        assert_eq!(result, Err(FireCallbackError));
        assert_eq!(context.calls, 2);
        assert_eq!(old, [77; 8]);
        assert_eq!(replacement[0], context.first);
        assert_eq!(&replacement[1..], &[99; 7]);
    }
}
