//! Engine exports (`crates/hp2-engine`). The C++ `AWind` and `FReachSpec`
//! methods in `Engine/Src/UnWind.cpp` and `Engine/Src/UnReach.cpp` forward
//! here. Engine objects cross as opaque `void*`; their layout lives in
//! `hp2-abi::engine` and is asserted on both sides. Engine-global services
//! (`appRand`, `FarMoveActor`, collision, ...) reach C++ through the
//! `Hp2EngineServices` table passed on every call.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AWind, FReachSpec};
use hp2_engine::reach;
use hp2_engine::services::Hp2EngineServices;
use hp2_engine::wind as wind_mod;
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2EngineServices`.
unsafe fn svc_of<'a>(p: *const Hp2EngineServices) -> Option<&'a Hp2EngineServices> {
    unsafe { p.as_ref() }
}

/// Reads an `FVector` by value; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `FVector`.
unsafe fn read_vec(p: *const c_void) -> Option<FVector> {
    unsafe { p.cast::<FVector>().as_ref().copied() }
}

/// Writes an `FVector` through `p`; a null `p` is a no-op.
///
/// # Safety
/// `p`, when non-null, must be valid for writes of an `FVector`.
unsafe fn write_vec(p: *mut c_void, v: FVector) {
    unsafe {
        if let Some(out) = p.cast::<FVector>().as_mut() {
            *out = v;
        }
    }
}

/// `AWind::GetWind(const FVector& Loc)`. Writes the sampled wind to `out`.
///
/// # Safety
/// `services` must be a live `Hp2EngineServices` (or null). `wind` must be a
/// live `AWind` (or null). `loc` and `out` must point to an `FVector` each.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_native_AWind_GetWind(
    services: *const Hp2EngineServices,
    wind: *mut c_void,
    loc: *const c_void,
    out: *mut c_void,
) {
    guard("hp2_rs_native_AWind_GetWind", (), move || unsafe {
        let (Some(svc), Some(loc)) = (svc_of(services), read_vec(loc)) else {
            return;
        };
        if wind.is_null() {
            return;
        }
        let v = wind_mod::get_wind_shared(svc, wind.cast::<AWind>(), loc);
        write_vec(out, v);
    });
}

/// `AWind::AWind()`: adds `this` to the wind list.
///
/// # Safety
/// `wind` must be a live `AWind` that stays valid until unregistered.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_wind_register(wind: *mut c_void) {
    guard("hp2_rs_engine_wind_register", (), move || {
        if !wind.is_null() {
            wind_mod::register(wind.cast::<AWind>());
        }
    });
}

/// `AWind::Destroy()`: removes `this` from the wind list.
///
/// # Safety
/// `wind` must be a pointer previously passed to
/// [`hp2_rs_engine_wind_register`], or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_wind_unregister(wind: *mut c_void) {
    guard("hp2_rs_engine_wind_unregister", (), move || {
        if !wind.is_null() {
            wind_mod::unregister(wind.cast::<AWind>());
        }
    });
}

/// `AWind::Tick` after `Super::Tick`: integrates the gust state.
///
/// # Safety
/// `services` must be a live `Hp2EngineServices`. `wind` must be a live
/// `AWind`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_wind_tick(
    services: *const Hp2EngineServices,
    wind: *mut c_void,
    delta_time: f32,
) {
    guard("hp2_rs_engine_wind_tick", (), move || unsafe {
        let Some(svc) = svc_of(services) else {
            return;
        };
        if wind.is_null() {
            return;
        }
        wind_mod::tick(svc, wind.cast::<AWind>(), delta_time);
    });
}

/// `AWind::GetTotalWind(ULevel*, const FVector& Loc)`. Writes the sum to `out`.
///
/// # Safety
/// `services` must be a live `Hp2EngineServices`. `loc` and `out` must point
/// to an `FVector` each. Every registered wind must still be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_wind_get_total_wind(
    services: *const Hp2EngineServices,
    loc: *const c_void,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_wind_get_total_wind", (), move || unsafe {
        let (Some(svc), Some(loc)) = (svc_of(services), read_vec(loc)) else {
            return;
        };
        let v = wind_mod::get_total_wind(svc, loc);
        write_vec(out, v);
    });
}

/// `FReachSpec::MonsterPath()`.
///
/// # Safety
/// `spec` must be a live `FReachSpec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_reach_monster_path(spec: *const c_void) -> i32 {
    guard("hp2_rs_engine_reach_monster_path", 0, move || unsafe {
        if spec.is_null() {
            return 0;
        }
        reach::monster_path(spec.cast::<FReachSpec>())
    })
}

/// `FReachSpec::BotOnlyPath()`.
///
/// # Safety
/// `spec` must be a live `FReachSpec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_reach_bot_only_path(spec: *const c_void) -> i32 {
    guard("hp2_rs_engine_reach_bot_only_path", 0, move || unsafe {
        if spec.is_null() {
            return 0;
        }
        reach::bot_only_path(spec.cast::<FReachSpec>())
    })
}

/// `FReachSpec::operator+`. Writes the combined spec to `out`.
///
/// # Safety
/// `a` and `b` must be live `FReachSpec`s. `out` must be valid for writes of
/// one `FReachSpec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_reach_combine(
    a: *const c_void,
    b: *const c_void,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_reach_combine", (), move || unsafe {
        if a.is_null() || b.is_null() || out.is_null() {
            return;
        }
        reach::combine(
            a.cast::<FReachSpec>(),
            b.cast::<FReachSpec>(),
            out.cast::<FReachSpec>(),
        );
    });
}

/// `FReachSpec::operator<=`.
///
/// # Safety
/// `a` and `b` must be live `FReachSpec`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_reach_less_equal(a: *const c_void, b: *const c_void) -> i32 {
    guard("hp2_rs_engine_reach_less_equal", 0, move || unsafe {
        if a.is_null() || b.is_null() {
            return 0;
        }
        reach::less_equal(a.cast::<FReachSpec>(), b.cast::<FReachSpec>())
    })
}

/// `FReachSpec::operator==`.
///
/// # Safety
/// `a` and `b` must be live `FReachSpec`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_reach_equal(a: *const c_void, b: *const c_void) -> i32 {
    guard("hp2_rs_engine_reach_equal", 0, move || unsafe {
        if a.is_null() || b.is_null() {
            return 0;
        }
        reach::equal(a.cast::<FReachSpec>(), b.cast::<FReachSpec>())
    })
}

/// `FReachSpec::defineFor(AActor* Begin, AActor* Dest, APawn* Scout)`.
///
/// # Safety
/// `services` must be a live `Hp2EngineServices`. `spec`, `begin`, `dest`
/// and `scout` must be live objects of their types.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_reach_define_for(
    services: *const Hp2EngineServices,
    spec: *mut c_void,
    begin: *mut c_void,
    dest: *mut c_void,
    scout: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_reach_define_for", 0, move || unsafe {
        let Some(svc) = svc_of(services) else {
            return 0;
        };
        if spec.is_null() || begin.is_null() || dest.is_null() || scout.is_null() {
            return 0;
        }
        reach::define_for(
            svc,
            spec.cast::<FReachSpec>(),
            begin.cast(),
            dest.cast(),
            scout.cast(),
        )
    })
}

/// `FReachSpec::findBestReachable(FVector& Begin, FVector& Destination, APawn* Scout)`.
///
/// # Safety
/// `services` must be a live `Hp2EngineServices`. `spec` must be a live
/// `FReachSpec` whose `start`/`end` are live `AActor`s. `begin` and
/// `destination` must each point to an `FVector`. `scout` must be a live
/// `APawn`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_reach_find_best(
    services: *const Hp2EngineServices,
    spec: *mut c_void,
    begin: *const c_void,
    destination: *const c_void,
    scout: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_reach_find_best", 0, move || unsafe {
        let Some(svc) = svc_of(services) else {
            return 0;
        };
        if spec.is_null() || begin.is_null() || destination.is_null() || scout.is_null() {
            return 0;
        }
        reach::find_best_reachable(
            svc,
            spec.cast::<FReachSpec>(),
            begin.cast::<FVector>(),
            destination.cast::<FVector>(),
            scout.cast(),
        )
    })
}
