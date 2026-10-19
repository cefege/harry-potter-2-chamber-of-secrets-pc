//! Engine exports from `crates/hp2-engine/src/particle_fx.rs` (`UnParticleFX.cpp`).
//! `fx` is the live `AParticleFX*` owned by the UObject shell; `svc` is the
//! `Hp2ParticleFxServices*` table the C++ shell passes. Vectors, params, and
//! output records are passed by address. Bool results are `i32` (0 or 1).

use std::ffi::c_void;

use hp2_abi::FCoords;
use hp2_abi::FVector;
use hp2_abi::engine::generated::AParticleFX;
use hp2_abi::render_scene::FSceneNode;
use hp2_engine::particle_fx::{self, FParams, FParticleParams, Hp2ParticleFxServices};
use hp2_ffi_support::guard;

/// Borrows the `Hp2ParticleFxServices` table the C++ shell passes; `None` if null.
///
/// # Safety
/// A non-null `svc` must point at a live `Hp2ParticleFxServices`.
unsafe fn services<'a>(svc: *const c_void) -> Option<&'a Hp2ParticleFxServices> {
    unsafe { svc.cast::<Hp2ParticleFxServices>().as_ref() }
}

/// Returns the `AParticleFX*` argument; `None` if null.
fn fx_arg(fx: *mut c_void) -> Option<*mut AParticleFX> {
    if fx.is_null() {
        None
    } else {
        Some(fx.cast::<AParticleFX>())
    }
}

/// `AParticleFX::EmitParticles(Tick)`. Returns the number of particles emitted.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_emit_particles(
    fx: *mut c_void,
    tick: f32,
    svc: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_particle_fx_emit_particles",
        0,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return 0;
            };
            particle_fx::emit_particles(fx, tick, svc)
        },
    )
}

/// `AParticleFX::UpdateParticles(Tick)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_update_particles(
    fx: *mut c_void,
    tick: f32,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_particle_fx_update_particles",
        (),
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return;
            };
            particle_fx::update_particles(fx, tick, svc);
        },
    );
}

/// `AParticleFX::Update(Step)`. Returns nonzero for `true`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_update(
    fx: *mut c_void,
    step: f32,
    svc: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_particle_fx_update", 0, move || unsafe {
        let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
            return 0;
        };
        i32::from(particle_fx::update(fx, step, svc))
    })
}

/// `AParticleFX::GetRenderBoundingBox` body; writes the `FCoords` to `out`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_get_render_bounding_box(
    fx: *mut c_void,
    out: *mut c_void,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_particle_fx_get_render_bounding_box",
        (),
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return;
            };
            if out.is_null() {
                return;
            }
            particle_fx::get_render_bounding_box(fx, out.cast::<FCoords>(), svc);
        },
    );
}

/// `AParticleFX::InitExecution` body after `Super::InitExecution()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_init_execution(fx: *mut c_void, svc: *const c_void) {
    guard(
        "hp2_rs_engine_particle_fx_init_execution",
        (),
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return;
            };
            particle_fx::init_execution(fx, svc);
        },
    );
}

/// `AParticleFX::Tick` body before `Super::Tick` (the shell calls `Super::Tick`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_tick(
    fx: *mut c_void,
    delta_seconds: f32,
    tick_type: i32,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_particle_fx_tick", (), move || unsafe {
        let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
            return;
        };
        particle_fx::tick(fx, delta_seconds, tick_type, svc);
    });
}

/// `AParticleFX::MaxLifetime()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_max_lifetime(fx: *mut c_void) -> f32 {
    guard(
        "hp2_rs_engine_particle_fx_max_lifetime",
        0.0,
        move || unsafe {
            let Some(fx) = fx_arg(fx) else {
                return 0.0;
            };
            particle_fx::max_lifetime(fx)
        },
    )
}

/// `AParticleFX::Lod(ScreenFrac)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_lod(
    fx: *mut c_void,
    screen_frac: f32,
    svc: *const c_void,
) -> f32 {
    guard("hp2_rs_engine_particle_fx_lod", 0.0, move || unsafe {
        let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
            return 0.0;
        };
        particle_fx::lod(fx, screen_frac, svc)
    })
}

/// `AParticleFX::LodParticles(ScreenFrac)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_lod_particles(
    fx: *mut c_void,
    screen_frac: f32,
    svc: *const c_void,
) -> f32 {
    guard(
        "hp2_rs_engine_particle_fx_lod_particles",
        0.0,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return 0.0;
            };
            particle_fx::lod_particles(fx, screen_frac, svc)
        },
    )
}

/// `AParticleFX::LodParticleDensity(Frame, ScreenFrac)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_lod_particle_density(
    fx: *mut c_void,
    frame: *const c_void,
    screen_frac: f32,
    svc: *const c_void,
) -> f32 {
    guard(
        "hp2_rs_engine_particle_fx_lod_particle_density",
        0.0,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return 0.0;
            };
            if frame.is_null() {
                return 0.0;
            }
            particle_fx::lod_particle_density(fx, frame.cast::<FSceneNode>(), screen_frac, svc)
        },
    )
}

/// `AParticleFX::GetSysParams(Buffer)`. `buf` must hold `sizeof(AParticleFX)`
/// bytes. Returns the system to read params from: `fx` itself, the parent
/// defaults, or the blended copy in `buf`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_get_sys_params(
    fx: *mut c_void,
    buf: *mut c_void,
    svc: *const c_void,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_particle_fx_get_sys_params",
        std::ptr::null_mut(),
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return std::ptr::null_mut();
            };
            if buf.is_null() {
                return std::ptr::null_mut();
            }
            particle_fx::get_sys_params(fx, buf.cast::<AParticleFX>(), svc).cast()
        },
    )
}

/// `AParticleFX::GetParams(FParams&)` on the system `sys`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_get_params(
    sys: *const c_void,
    out: *mut c_void,
    svc: *const c_void,
) {
    guard("hp2_rs_engine_particle_fx_get_params", (), move || unsafe {
        let Some(svc) = services(svc) else {
            return;
        };
        if sys.is_null() || out.is_null() {
            return;
        }
        particle_fx::get_params(sys.cast::<AParticleFX>(), out.cast::<FParams>(), svc);
    });
}

/// `AParticleFX::AddParticle(Id, Position, Params)`. Returns nonzero on success.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_add_particle(
    fx: *mut c_void,
    id: i32,
    pos: *const c_void,
    params: *const c_void,
    svc: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_particle_fx_add_particle",
        0,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return 0;
            };
            if pos.is_null() {
                return 0;
            }
            i32::from(particle_fx::add_particle(
                fx,
                id,
                pos.cast::<FVector>(),
                params.cast::<FParams>(),
                svc,
            ))
        },
    )
}

/// `AParticleFX::RecomputeDeltas(Id)`. Returns nonzero on success.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_recompute_deltas(
    fx: *mut c_void,
    id: i32,
    svc: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_particle_fx_recompute_deltas",
        0,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return 0;
            };
            i32::from(particle_fx::recompute_deltas(fx, id, svc))
        },
    )
}

/// `AParticleFX::execAddParticle` body: sys params, `GetParams`, `AddParticle`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_exec_add_particle(
    fx: *mut c_void,
    id: i32,
    loc: *const c_void,
    svc: *const c_void,
) {
    guard(
        "hp2_rs_engine_particle_fx_exec_add_particle",
        (),
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return;
            };
            if loc.is_null() {
                return;
            }
            particle_fx::exec_add_particle(fx, id, loc.cast::<FVector>(), svc);
        },
    );
}

/// `AParticleFX::execNumParticles` body. `-1` when there is no particle list.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_num_particles(
    fx: *mut c_void,
    svc: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_particle_fx_num_particles",
        -1,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return -1;
            };
            particle_fx::num_particles(fx, svc)
        },
    )
}

/// `AParticleFX::execGetParticleParams` body. Returns nonzero if `Id` exists.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_get_particle_params(
    fx: *mut c_void,
    id: i32,
    out: *mut c_void,
    svc: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_particle_fx_get_particle_params",
        0,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return 0;
            };
            if out.is_null() {
                return 0;
            }
            i32::from(particle_fx::get_particle_params(
                fx,
                id,
                out.cast::<FParticleParams>(),
                svc,
            ))
        },
    )
}

/// `AParticleFX::execSetParticleParams` body. Returns nonzero if `Id` exists.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_particle_fx_set_particle_params(
    fx: *mut c_void,
    id: i32,
    params: *const c_void,
    svc: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_particle_fx_set_particle_params",
        0,
        move || unsafe {
            let (Some(fx), Some(svc)) = (fx_arg(fx), services(svc)) else {
                return 0;
            };
            if params.is_null() {
                return 0;
            }
            i32::from(particle_fx::set_particle_params(
                fx,
                id,
                params.cast::<FParticleParams>(),
                svc,
            ))
        },
    )
}
