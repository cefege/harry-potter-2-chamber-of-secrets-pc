//! Port of `Engine/Src/UnWind.cpp`: AWind fluctuation, per-actor wind sampling
//! and the shared `FIntRandom` noise table.
//!
//! Registry allocation/lifetime remains in the native `TArray`; all policy uses
//! live leaf reads. Permutation storage is raw across reentrant RNG callbacks.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AWind, FFrame, FRotator, ULevel};
use hp2_core::vm_frame;
use hp2_core::vm_registry::Hp2VmNativeHandler;

use crate::math::{app_exp, app_floor, app_sqrt};
use crate::services::{EngineThrown, Hp2EngineServices, checked};
use crate::vec;

/// `ELDType` values stored in `AWind::WindSource` (`EngineClasses.h`).
pub const LD_POINT: u8 = 0;
pub const LD_PLANE: u8 = 1;

/// `static float Clug = 1.f;`
const CLUG: f32 = 1.0;
/// `static float NoiseGrain = 32.f;`
const NOISE_GRAIN: f32 = 32.0;
/// `static float SpatialFluc = 1.f;`
const SPATIAL_FLUC: f32 = 1.0;

/// `FIntRandom`: a 256-entry permutation seeded from `appRand()`.
#[repr(C)]
pub struct IntRandom {
    table: [u8; 256],
    initialized: u8,
}

impl Default for IntRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl IntRandom {
    pub const fn new() -> Self {
        IntRandom {
            table: [0; 256],
            initialized: 0,
        }
    }

    /// Initializes in place, publishing the initialized flag before any RNG call.
    /// Reentry therefore observes exactly the partially shuffled native table.
    ///
    /// # Safety
    /// `tab` is engine-thread storage. No reference to it may span a callback.
    pub unsafe fn init(tab: *mut Self, svc: &Hp2EngineServices) -> Result<(), EngineThrown> {
        unsafe { Self::init_with(tab, || {
            let mut random = 0;
            checked((svc.app_rand)(&mut random))?;
            Ok(random)
        }) }
    }

    /// Initializes using a caller-supplied signed random source.
    ///
    /// # Safety
    /// `tab` is stable engine-thread storage, and no reference to it may span `next`.
    pub(crate) unsafe fn init_with<E>(
        tab: *mut Self,
        mut next: impl FnMut() -> Result<i32, E>,
    ) -> Result<(), E> {
        unsafe {
            if (*tab).initialized != 0 {
                return Ok(());
            }
            (*tab).initialized = 1;
            for i in 0..256 {
                (*tab).table[i] = i as u8;
            }
            for j in 0..256 {
                let k = (next()? % 256) as isize;
                let base = ptr::addr_of_mut!((*tab).table).cast::<u8>();
                ptr::swap(base.add(j), base.offset(k));
            }
            Ok(())
        }
    }

    /// Native signed remainder semantics; callers must use valid lattice indices.
    unsafe fn get(tab: *const Self, i: i32) -> u8 {
        unsafe {
            *ptr::addr_of!((*tab).table)
                .cast::<u8>()
                .offset((i % 256) as isize)
        }
    }
}

/// `AWind::Radius()`: `Square(INT(WindRadius))`.
///
/// # Safety
/// `w` must point to a live `AWind`.
unsafe fn radius(w: *const AWind) -> f32 {
    unsafe {
        let r = i32::from((*w).wind_radius);
        (r * r) as f32
    }
}

/// `AWind::InnerRadiusFrac()`: `WindRadiusInner / 256.0f`.
///
/// # Safety
/// `w` must point to a live `AWind`.
unsafe fn inner_radius_frac(w: *const AWind) -> f32 {
    unsafe { f32::from((*w).wind_radius_inner) / 256.0 }
}

/// `AWind::FlucFraction()`: `WindFluctuation / 255.0f`.
///
/// # Safety
/// `w` must point to a live `AWind`.
unsafe fn fluc_fraction(w: *const AWind) -> f32 {
    unsafe { f32::from((*w).wind_fluctuation) / 255.0 }
}

/// `AWind::FlucPeriod()`: `Max((INT)WindFlucPeriod, 1) / 64.0f`.
///
/// # Safety
/// `w` must point to a live `AWind`.
unsafe fn fluc_period(w: *const AWind) -> f32 {
    unsafe {
        let m = i32::from((*w).wind_fluc_period).max(1);
        m as f32 / 64.0
    }
}

/// `Min(A, B)` from `UnTemplate.h`.
#[inline]
fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `VecRand(int iX, int iY, int iZ)`: one lattice corner of the noise field.
unsafe fn vec_rand_cell(tab: *const IntRandom, ix: i32, iy: i32, iz: i32) -> FVector {
    let hz = i32::from(unsafe { IntRandom::get(tab, iz) });
    let hy = i32::from(unsafe { IntRandom::get(tab, iy + hz) });
    let mut hash = i32::from(unsafe { IntRandom::get(tab, ix + hy) });
    let x = hash as f32 / 255.0 - 0.5;
    hash = i32::from(unsafe { IntRandom::get(tab, hash) });
    let y = hash as f32 / 255.0 - 0.5;
    hash = i32::from(unsafe { IntRandom::get(tab, hash) });
    let z = hash as f32 / 255.0 - 0.5;
    FVector { x, y, z }
}

/// `VecRand(const FVector& Loc)`: trilinear 3D noise at `loc`.
///
/// # Safety
/// `tab` is stable owner-thread storage with no reference held across `next`.
unsafe fn vec_rand_at_with<E>(
    tab: *mut IntRandom,
    loc: *const FVector,
    next: impl FnMut() -> Result<i32, E>,
) -> Result<FVector, E> {
    unsafe {
        IntRandom::init_with(tab, next)?;
        let l = vec::div(*loc, NOISE_GRAIN);
        let ix = app_floor(l.x);
        let iy = app_floor(l.y);
        let iz = app_floor(l.z);
        let r = FVector { x: l.x - ix as f32, y: l.y - iy as f32, z: l.z - iz as f32 };
        let s = FVector { x: 1.0 - r.x, y: 1.0 - r.y, z: 1.0 - r.z };
        let terms = [
            (ix, iy, iz, s.x * s.y * s.z),
            (ix, iy, iz + 1, s.x * s.y * r.z),
            (ix, iy + 1, iz, s.x * r.y * s.z),
            (ix, iy + 1, iz + 1, s.x * r.y * r.z),
            (ix + 1, iy, iz, r.x * s.y * s.z),
            (ix + 1, iy, iz + 1, r.x * s.y * r.z),
            (ix + 1, iy + 1, iz, r.x * r.y * s.z),
            (ix + 1, iy + 1, iz + 1, r.x * r.y * r.z),
        ];
        let mut acc = {
            let (x, y, z, w) = terms[0];
            vec::scale(vec_rand_cell(tab, x, y, z), w)
        };
        for &(x, y, z, w) in &terms[1..] {
            acc = vec::add(acc, vec::scale(vec_rand_cell(tab, x, y, z), w));
        }
        Ok(acc)
    }
}

/// `VRand()`: uniform direction from rejection sampling in the unit cube.
fn vrand_with<E>(mut next: impl FnMut() -> Result<f32, E>) -> Result<FVector, E> {
    let v = loop {
        let x = next()? * 2.0 - 1.0;
        let y = next()? * 2.0 - 1.0;
        let z = next()? * 2.0 - 1.0;
        let v = FVector { x, y, z };
        if vec::size_squared(v) <= 1.0 { break v; }
    };
    Ok(vec::unsafe_normal(v))
}


/// `AWind::GetWind` shared policy with caller-owned engine operations.
///
/// The model callback runs only for non-permeating winds within range; `rotation`
/// runs only for plane winds or point winds at their source.
///
/// # Safety
/// `wind` must be live; its level/model must be live when line checking is requested.
pub(crate) unsafe fn get_wind_with<E>(
    tab: *mut IntRandom,
    wind: *mut AWind,
    loc: &FVector,
    mut next_rand: impl FnMut() -> Result<i32, E>,
    mut line_check: impl FnMut(*mut ULevel, &FVector, &FVector) -> Result<bool, E>,
    mut rotation: impl FnMut(FRotator) -> Result<FVector, E>,
) -> Result<FVector, E> {
    unsafe {
        let location = (*wind).base.location;
        let dist_sq = vec::size_squared(vec::sub(*loc, location));
        let r = radius(wind);
        if (*wind).wind_speed == 0.0 || dist_sq > r * r { return Ok(vec::zero()); }
        if !(*wind).b_permeating() {
            let level = (*wind).base.x_level.cast::<ULevel>();
            if !line_check(level, &location, loc)? { return Ok(vec::zero()); }
        }
        let dist = app_sqrt(f64::from(dist_sq)) as f32;
        let mut v = match (*wind).wind_source {
            LD_POINT if dist > 0.0 => vec::div(vec::sub(*loc, location), dist),
            LD_POINT | LD_PLANE => rotation((*wind).base.rotation)?,
            _ => vec::zero(),
        };
        v = vec::add(v, (*wind).fluc);
        if f32::from((*wind).wind_fluctuation) * SPATIAL_FLUC != 0.0 {
            let noise = vec_rand_at_with(tab, loc, &mut next_rand)?;
            let k = vec::size(v) * fluc_fraction(wind) * SPATIAL_FLUC;
            v = vec::add(v, vec::scale(noise, k));
        }
        let falloff = min_f(
            (1.0 - dist / radius(wind)) / (1.0 - inner_radius_frac(wind)),
            1.0,
        );
        Ok(vec::scale(v, (*wind).wind_speed * falloff))
    }
}

/// Native-services adapter using the same typed GetWind policy.
///
/// # Safety
/// `wind` and `loc` must be live, and native service callbacks must be valid.
pub unsafe fn get_wind(
    svc: &Hp2EngineServices,
    tab: *mut IntRandom,
    wind: *mut AWind,
    loc: *const FVector,
) -> Result<FVector, EngineThrown> {
    unsafe {
        get_wind_with(
            tab, wind, &*loc,
            || { let mut n = 0; checked((svc.app_rand)(&mut n))?; Ok(n) },
            |level, start, end| Ok((svc.model_fast_line_check)(
                (*level).model, ptr::from_ref(start).cast(), ptr::from_ref(end).cast(),
            ) != 0),
            |rot| {
                let mut out = vec::zero();
                (svc.rotator_vector)(ptr::from_ref(&rot).cast(), ptr::from_mut(&mut out).cast());
                Ok(out)
            },
        )
    }
}

/// `AWind::Tick` body after `Super::Tick`: the wind gust integration.
///
/// # Safety
/// `wind` must point to a live `AWind`. `svc` must be valid.
pub unsafe fn tick(
    svc: &Hp2EngineServices,
    wind: *mut AWind,
    delta_time: f32,
) -> Result<(), EngineThrown> {
    unsafe {
        tick_with(wind, delta_time, || {
            let mut random = 0.0;
            checked((svc.app_frand)(&mut random))?;
            Ok(random)
        })
    }
}

/// Shared gust integration after the caller has run genuine Actor ticking.
///
/// # Safety
/// `wind` remains live across `next_rand`, which may reenter owner state.
pub(crate) unsafe fn tick_with<E>(
    wind: *mut AWind,
    delta_time: f32,
    next_rand: impl FnMut() -> Result<f32, E>,
) -> Result<(), E> {
    unsafe {
        if (*wind).wind_fluctuation == 0 {
            return Ok(());
        }

        // Exponentially average FlucVel over the period.
        let frac = delta_time / fluc_period(wind);
        let decay = app_exp(f64::from(-frac * CLUG)) as f32;

        // Decay fluctuation velocity slowly, then randomly perturb it.
        let decayed_velocity = vec::scale((*wind).fluc_vel, decay);
        let noise = vrand_with(next_rand)?;
        let fluc_vel = vec::add(
            decayed_velocity,
            vec::scale(noise, fluc_fraction(wind) * (1.0 - decay)),
        );
        (*wind).fluc_vel = fluc_vel;

        // Decay the actual fluctuation, and apply velocity.
        let fluc = vec::scale((*wind).fluc, app_exp(f64::from(-frac)) as f32);
        (*wind).fluc = vec::add(fluc, vec::scale(fluc_vel, delta_time));
        Ok(())
    }
}

/// `AWind::GetTotalWind(ULevel*, const FVector& Loc)`: sum over `GWindActors`.
///
/// # Safety
/// Every registered wind must still be live. `svc` must be valid.
pub unsafe fn get_total_wind(
    svc: &Hp2EngineServices,
    loc: *const FVector,
) -> Result<FVector, EngineThrown> {
    get_total_wind_with(
        || Ok(unsafe { (svc.wind_count)() }.max(0) as usize),
        |index| unsafe { get_wind_shared(svc, (svc.wind_at)(index as i32).cast(), loc) },
    )
}

/// Reads registry length again after every sample, matching native reentry.
pub(crate) fn get_total_wind_with<E>(
    mut count: impl FnMut() -> Result<usize, E>,
    mut sample: impl FnMut(usize) -> Result<FVector, E>,
) -> Result<FVector, E> {
    let mut total = vec::zero();
    let mut index = 0;
    while index < count()? {
        total = vec::add(total, sample(index)?);
        index += 1;
    }
    Ok(total)
}

/// `AWind::GetWind` against the process-global `GIntRandom`.
///
/// # Safety
/// See [`get_wind`].
pub unsafe fn get_wind_shared(
    svc: &Hp2EngineServices,
    wind: *mut AWind,
    loc: *const FVector,
) -> Result<FVector, EngineThrown> {
    unsafe { get_wind(svc, (svc.wind_random)().cast(), wind, loc) }
}

/// `AWind::execGetWind` body (`UnWind.cpp:46`): `*(FVector*)Result =
/// GetWind(Loc)`. The original has no `guard`/`unguard`: nothing to unwind
/// on error.
///
/// # Safety
/// Frame/result/native_table satisfy vm_frame::step_argument/finish_arguments.
/// `wind` must point to a live `AWind`. `svc` must be valid.
pub unsafe fn exec_get_wind(
    wind: *mut AWind,
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    svc: &Hp2EngineServices,
) -> Result<(), i32> {
    unsafe {
        let mut loc = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        vm_frame::step_argument(frame, (&raw mut loc).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        match get_wind_shared(svc, wind, &raw const loc) {
            Ok(v) => {
                result.cast::<FVector>().write(v);
                Ok(())
            }
            Err(e) => Err(e.0),
        }
    }
}
