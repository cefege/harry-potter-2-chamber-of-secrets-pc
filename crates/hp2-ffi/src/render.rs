//! Render subsystem exports (`crates/hp2-render`). C++ shells live in
//! `HarryPotter2/Unreal/Render/Src`; see `Render.cpp` for the services table.

use hp2_ffi_support::guard;
use hp2_render::random::{N_RANDS, tick};
use hp2_render::services::Hp2RenderServices;

/// `FGlobalRandoms::Tick`. C++ owns `RandomBases`, `Randoms`, `RandomDeltas`
/// and `LastTicks`; they are passed by pointer and updated in place.
/// `services` supplies `appFrand` so the engine RNG sequence is shared.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_random_tick(
    services: *const Hp2RenderServices,
    time_seconds: f32,
    bases: *mut f32,
    randoms: *mut f32,
    deltas: *mut f32,
    last_ticks: *mut u32,
) {
    guard("hp2_rs_render_random_tick", (), move || unsafe {
        if services.is_null()
            || bases.is_null()
            || randoms.is_null()
            || deltas.is_null()
            || last_ticks.is_null()
        {
            return;
        }
        let services = *services;
        let bases = &mut *(bases as *mut [f32; N_RANDS]);
        let randoms = &mut *(randoms as *mut [f32; N_RANDS]);
        let deltas = &mut *(deltas as *mut [f32; N_RANDS]);
        let last_ticks = &mut *last_ticks;
        let mut frand = || (services.frand)();
        tick(&mut frand, time_seconds, bases, randoms, deltas, last_ticks);
    });
}
