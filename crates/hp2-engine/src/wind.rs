//! Port of `Engine/Src/UnWind.cpp`: AWind fluctuation, per-actor wind sampling
//! and the shared `FIntRandom` noise table.
//!
//! `GWindActors` and `GIntRandom` are process globals in C++; here they live
//! behind mutexes. Lock order is `WIND_ACTORS` then `INT_RANDOM`.

use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, MutexGuard, PoisonError};

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AWind, ULevel};

use crate::math::{app_exp, app_floor, app_sqrt};
use crate::services::Hp2EngineServices;
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
pub struct IntRandom {
    table: [u8; 256],
    initialized: bool,
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
            initialized: false,
        }
    }

    /// `FIntRandom::Init()`: builds the permutation on first use.
    ///
    /// # Safety
    /// `svc.app_rand` must be a valid callback into the engine.
    pub unsafe fn init(&mut self, svc: &Hp2EngineServices) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        for i in 0..256 {
            self.table[i] = i as u8;
        }
        for j in 0..256 {
            let k = unsafe { (svc.app_rand)() }.rem_euclid(256) as usize;
            self.table.swap(j, k);
        }
    }

    /// `FIntRandom::operator()(int i)`: `IntRand[i % 256]`.
    ///
    /// The C++ `%` keeps the sign of `i`, so a negative index reads outside
    /// the table there. Here `rem_euclid` wraps it into range instead.
    pub fn get(&self, i: i32) -> u8 {
        self.table[i.rem_euclid(256) as usize]
    }
}

static WIND_ACTORS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static INT_RANDOM: Mutex<IntRandom> = Mutex::new(IntRandom::new());

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `AWind::AWind()`: `GWindActors.AddItem(this)`.
pub fn register(wind: *mut AWind) {
    lock(&WIND_ACTORS).push(wind as usize);
}

/// `AWind::Destroy()`: `GWindActors.RemoveItem(this)`.
pub fn unregister(wind: *mut AWind) {
    lock(&WIND_ACTORS).retain(|&a| a != wind as usize);
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
fn vec_rand_cell(tab: &IntRandom, ix: i32, iy: i32, iz: i32) -> FVector {
    let hz = i32::from(tab.get(iz));
    let hy = i32::from(tab.get(iy + hz));
    let mut hash = i32::from(tab.get(ix + hy));
    let x = hash as f32 / 255.0 - 0.5;
    hash = i32::from(tab.get(hash));
    let y = hash as f32 / 255.0 - 0.5;
    hash = i32::from(tab.get(hash));
    let z = hash as f32 / 255.0 - 0.5;
    FVector { x, y, z }
}

/// `VecRand(const FVector& Loc)`: trilinear 3D noise at `loc`.
///
/// # Safety
/// `svc.app_rand` must be a valid callback into the engine.
unsafe fn vec_rand_at(svc: &Hp2EngineServices, tab: &mut IntRandom, loc: FVector) -> FVector {
    unsafe {
        tab.init(svc);

        let l = vec::div(loc, NOISE_GRAIN);
        let ix = app_floor(l.x);
        let iy = app_floor(l.y);
        let iz = app_floor(l.z);
        let r = FVector {
            x: l.x - ix as f32,
            y: l.y - iy as f32,
            z: l.z - iz as f32,
        };
        let s = FVector {
            x: 1.0 - r.x,
            y: 1.0 - r.y,
            z: 1.0 - r.z,
        };

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
        acc
    }
}

/// `VRand()`: uniform direction from rejection sampling in the unit cube.
///
/// # Safety
/// `svc.app_frand` must be a valid callback into the engine.
unsafe fn vrand(svc: &Hp2EngineServices) -> FVector {
    unsafe {
        let v = loop {
            let x = (svc.app_frand)() * 2.0 - 1.0;
            let y = (svc.app_frand)() * 2.0 - 1.0;
            let z = (svc.app_frand)() * 2.0 - 1.0;
            let v = FVector { x, y, z };
            if vec::size_squared(v) > 1.0 {
                continue;
            }
            break v;
        };
        vec::unsafe_normal(v)
    }
}

/// `FRotator::Vector()` of `wind.rotation`, through the services table.
///
/// # Safety
/// `wind` must point to a live `AWind`.
unsafe fn rotation_vector(svc: &Hp2EngineServices, wind: *const AWind) -> FVector {
    unsafe {
        let mut out = vec::zero();
        (svc.rotator_vector)(
            ptr::addr_of!((*wind).base.rotation).cast::<c_void>(),
            ptr::addr_of_mut!(out).cast::<c_void>(),
        );
        out
    }
}

/// `AWind::GetWind(const FVector& Loc)`.
///
/// # Safety
/// `wind` must point to a live `AWind`. Unless `b_permeating` is set, its
/// `base.x_level` must point to a live `ULevel` whose `model` is valid.
pub unsafe fn get_wind(
    svc: &Hp2EngineServices,
    tab: &mut IntRandom,
    wind: *mut AWind,
    loc: FVector,
) -> FVector {
    unsafe {
        let location = (*wind).base.location;

        // Check range.
        let dist_sq = vec::size_squared(vec::sub(loc, location));
        let r = radius(wind);
        if (*wind).wind_speed == 0.0 || dist_sq > r * r {
            return vec::zero();
        }

        // Check line of sight.
        if !(*wind).b_permeating() {
            let level = (*wind).base.x_level.cast::<ULevel>();
            let model = (*level).model;
            if (svc.model_fast_line_check)(
                model,
                ptr::addr_of!(location).cast::<c_void>(),
                ptr::addr_of!(loc).cast::<c_void>(),
            ) == 0
            {
                return vec::zero();
            }
        }

        let dist = app_sqrt(f64::from(dist_sq)) as f32;

        let mut v = match (*wind).wind_source {
            LD_POINT if dist > 0.0 => vec::div(vec::sub(loc, location), dist),
            LD_POINT | LD_PLANE => rotation_vector(svc, wind),
            _ => vec::zero(),
        };

        // Add time fluctuation.
        v = vec::add(v, (*wind).fluc);

        if f32::from((*wind).wind_fluctuation) * SPATIAL_FLUC != 0.0 {
            let noise = vec_rand_at(svc, tab, loc);
            let k = vec::size(v) * fluc_fraction(wind) * SPATIAL_FLUC;
            v = vec::add(v, vec::scale(noise, k));
        }

        // Scale by strength and distance.
        let falloff = min_f((1.0 - dist / r) / (1.0 - inner_radius_frac(wind)), 1.0);
        vec::scale(v, (*wind).wind_speed * falloff)
    }
}

/// `AWind::Tick` body after `Super::Tick`: the wind gust integration.
///
/// # Safety
/// `wind` must point to a live `AWind`. `svc` must be valid.
pub unsafe fn tick(svc: &Hp2EngineServices, wind: *mut AWind, delta_time: f32) {
    unsafe {
        if (*wind).wind_fluctuation == 0 {
            return;
        }

        // Exponentially average FlucVel over the period.
        let frac = delta_time / fluc_period(wind);
        let decay = app_exp(f64::from(-frac * CLUG)) as f32;

        // Decay fluctuation velocity slowly, then randomly perturb it.
        let noise = vrand(svc);
        let fluc_vel = vec::add(
            vec::scale((*wind).fluc_vel, decay),
            vec::scale(noise, fluc_fraction(wind) * (1.0 - decay)),
        );
        (*wind).fluc_vel = fluc_vel;

        // Decay the actual fluctuation, and apply velocity.
        let fluc = vec::scale((*wind).fluc, app_exp(f64::from(-frac)) as f32);
        (*wind).fluc = vec::add(fluc, vec::scale(fluc_vel, delta_time));
    }
}

/// `AWind::GetTotalWind(ULevel*, const FVector& Loc)`: sum over `GWindActors`.
///
/// # Safety
/// Every registered wind must still be live. `svc` must be valid.
pub unsafe fn get_total_wind(svc: &Hp2EngineServices, loc: FVector) -> FVector {
    unsafe {
        let actors = lock(&WIND_ACTORS);
        let mut tab = lock(&INT_RANDOM);
        let mut total = vec::zero();
        for &a in actors.iter() {
            total = vec::add(total, get_wind(svc, &mut tab, a as *mut AWind, loc));
        }
        total
    }
}

/// `AWind::GetWind` against the process-global `GIntRandom`.
///
/// # Safety
/// See [`get_wind`].
pub unsafe fn get_wind_shared(svc: &Hp2EngineServices, wind: *mut AWind, loc: FVector) -> FVector {
    let mut tab = lock(&INT_RANDOM);
    unsafe { get_wind(svc, &mut tab, wind, loc) }
}
