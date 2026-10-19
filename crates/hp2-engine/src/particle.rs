//! Port of `UParticle::Update` (`UnParticle.cpp`) and the `UParticleListPriv`
//! list logic (`UnParticleList.cpp`). The C++ shells keep the UObject classes
//! and forward every method body here.
//!
//! Float sites follow clang's `-ffp-contract=on`: a scalar `a*b + c` inside one
//! C++ expression is fused (`mul_add`); FVector operator calls are separate
//! statements after inlining and are not fused. The fused and unfused sites are
//! listed at each use.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::engine::generated::{AParticleFX, FBox, UParticle};
use hp2_abi::{FPlane, FVector};

use crate::coords::{self, Trig};
use crate::prim::{dot, normalize, transform_vector, v3};
use crate::vec::{add, div, scale, size, sub};

/// `PPRIM_Liquid`.
const PPRIM_LIQUID: u8 = 2;

/// Services the Rust side calls back into C++ through. Mirrors
/// `HarryPotter2/Unreal/Engine/Inc/Hp2ParticleServices.h`; field order must
/// match exactly. Every pointer is `void*`; vectors are passed by address.
#[repr(C)]
pub struct Hp2ParticleServices {
    /// `appFrand()`.
    pub app_frand: unsafe extern "C" fn() -> f32,
    /// `GMath.SinTab(i)`.
    pub sin_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `GMath.CosTab(i)`.
    pub cos_tab: unsafe extern "C" fn(i: i32) -> f32,
    /// `*out = AWind::GetTotalWind(Level, *loc)`.
    pub total_wind: unsafe extern "C" fn(level: *mut c_void, loc: *const c_void, out: *mut c_void),
    /// `Level->Model->FastLineCheck(*end, *start)`. Nonzero = the fast check
    /// reports the line clear.
    pub fast_line_check:
        unsafe extern "C" fn(level: *mut c_void, end: *const c_void, start: *const c_void) -> i32,
    /// `FCheckResult Hit(1.f); System->GetLevel()->SingleLineCheck(Hit, NULL,
    /// *end, *start, TRACE_VisBlocking)`. Returns 1 and writes `*normal` and
    /// `*location` when `Hit.Actor` is non-null and an `ALevelInfo`; else 0.
    pub system_trace_level_hit: unsafe extern "C" fn(
        system: *mut c_void,
        end: *const c_void,
        start: *const c_void,
        normal: *mut c_void,
        location: *mut c_void,
    ) -> i32,
    /// Original palette block of `UParticle::Update` (UnParticle.cpp:153-162): index from the life
    /// fraction when `Lifetime > 0 && Age > ColorDelay`, else `Colors(0)`. Writes `*out`, and writes
    /// nothing when `ColorPalette->Palette` is null.
    pub palette_plane: unsafe extern "C" fn(
        color_palette: *mut c_void,
        age: f32,
        lifetime: f32,
        color_delay: f32,
        out: *mut c_void,
    ),
}

/// `Min(A,B) = (A<=B) ? A : B` (UnTemplate.h).
#[inline]
fn c_min(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `Max(A,B) = (A>=B) ? A : B` (UnTemplate.h).
#[inline]
fn c_max(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `FVector::IsZero()`.
#[inline]
fn is_zero(v: FVector) -> bool {
    v.x == 0.0 && v.y == 0.0 && v.z == 0.0
}

/// `UParticle::Update`. Advances one particle by `timestep`. Returns `false`
/// when the particle has expired (its lifetime ends within this step).
///
/// # Safety
/// `system` must point at a live `AParticleFX`, `level` at a live `ULevel`, and
/// `svc` must be a table whose thunks accept those pointers.
#[allow(clippy::too_many_arguments)]
pub unsafe fn update(
    p: &mut UParticle,
    system_gravity: FVector,
    system_damping: f32,
    system_wind: FVector,
    level: *mut c_void,
    mut timestep: f32,
    system: *mut c_void,
    svc: &Hp2ParticleServices,
) -> bool {
    let sys = unsafe { &*system.cast::<AParticleFX>() };

    // Check for over lifetime (death).
    if p.lifetime > 0.0 && p.age + timestep >= p.lifetime {
        return false;
    }

    // Drip: grow to the designer's size before falling.
    if p.drip_timer > 0.0 {
        p.drip_timer -= timestep;
        if p.drip_timer <= 0.0 {
            timestep = -p.drip_timer;
            p.drip_timer = 0.0;
            p.width_delta = 0.0;
            p.length_delta = 0.0;
        }
    }

    // Update last known position to current.
    p.last_position = p.position;

    let sin = |i: i32| unsafe { (svc.sin_tab)(i) };
    let cos = |i: i32| unsafe { (svc.cos_tab)(i) };
    let trig = Trig {
        sin: &sin,
        cos: &cos,
    };

    // Regular particle behavior.
    if p.drip_timer == 0.0 {
        // Fused: Spin += SpinRate*Timestep.
        p.spin = p.spin_rate.mul_add(timestep, p.spin);

        // Integration is exact, for constant gravity and damping.
        if system_damping < 1.0 {
            // Terminal velocity = gravity / damping, plus wind.
            let mut vfw = div(system_gravity, sys.damping);
            if sys.b_wind_per_particle() {
                let mut total = vec_zero();
                unsafe {
                    (svc.total_wind)(
                        level,
                        ptr::from_ref(&p.position).cast(),
                        ptr::from_mut(&mut total).cast(),
                    );
                }
                vfw = add(vfw, scale(total, sys.wind_modifier));
            } else {
                vfw = add(vfw, system_wind);
            }

            let k = (system_damping - 1.0) / sys.damping;
            p.position = add(
                p.position,
                add(scale(vfw, timestep), scale(sub(vfw, p.velocity), k)),
            );
            p.velocity = add(vfw, scale(sub(p.velocity, vfw), system_damping));
        } else {
            // No wind response.
            p.position = add(
                p.position,
                scale(
                    add(p.velocity, scale(system_gravity, timestep * 0.5)),
                    timestep,
                ),
            );
            p.velocity = add(p.velocity, scale(system_gravity, timestep));
        }

        // Particle locations relative to the system.
        if sys.b_system_relative() {
            let loc = sys.base.location;
            p.position = sub(p.position, sub(sys.last_update_location, loc));
            // GMath.UnitCoords / LastUpdateRotation, then * Rotation.
            let mut inv = coords::unit_coords();
            coords::div_rot(&mut inv, sys.last_update_rotation, &trig);
            let mut fwd = coords::unit_coords();
            coords::mul_rot(&mut fwd, sys.base.rotation, &trig);
            p.position = add(
                loc,
                transform_vector(transform_vector(sub(p.position, loc), &inv), &fwd),
            );
        }

        // System attraction.
        if !is_zero(sys.attraction) {
            let direction = sub(sys.base.location, p.position);
            // Fused per axis: Velocity.X += Attraction.X*Timestep*Direction.X.
            p.velocity.x = (sys.attraction.x * timestep).mul_add(direction.x, p.velocity.x);
            p.velocity.y = (sys.attraction.y * timestep).mul_add(direction.y, p.velocity.y);
            p.velocity.z = (sys.attraction.z * timestep).mul_add(direction.z, p.velocity.z);
        }

        // Decrement ChaosDelay (truthiness test, as in C++).
        if p.chaos_delay != 0.0 {
            p.chaos_delay = if p.chaos_delay > timestep {
                p.chaos_delay - timestep
            } else {
                0.0
            };
        }

        // Perturb velocity by Chaos in a random direction once the delay lapses.
        if sys.chaos != 0.0 && p.chaos_delay <= 0.0 {
            // Argument order: X, Y, Z (C++ leaves argument evaluation unsequenced;
            // the fixture replay pins this order).
            let rx = unsafe { (svc.app_frand)() };
            let ry = unsafe { (svc.app_frand)() };
            let rz = unsafe { (svc.app_frand)() };
            // `-1.0f + 2.0f * appFrand()` is contracted by clang (fmla/fmadd in the
            // pinned build), so each component is fma(2, r, -1).
            let mut chaos_adjust = v3(
                2.0_f32.mul_add(rx, -1.0),
                2.0_f32.mul_add(ry, -1.0),
                2.0_f32.mul_add(rz, -1.0),
            );
            normalize(&mut chaos_adjust);
            p.velocity = add(p.velocity, scale(chaos_adjust, sys.chaos));
            p.chaos_delay = sys.chaos_delay;
        }
    }

    // Regular systems have bUpdate set. Scripted systems might not.
    if sys.b_update() {
        if p.age < sys.alpha_grow_period * p.lifetime {
            // Still fading in. Not fused: the division sits between the add and the multiply.
            p.alpha += p.alpha_full * timestep / (sys.alpha_grow_period * p.lifetime);
            p.alpha = c_min(p.alpha, p.alpha_full);
        } else if p.age > sys.alpha_delay {
            // Fused: Alpha += AlphaDelta*Timestep.
            p.alpha = p.alpha_delta.mul_add(timestep, p.alpha);
        }

        // Fix for -0.0f and -0.1f even with Clamp(Alpha, 0.0f, 1.0f).
        if p.alpha < 0.001 {
            p.alpha = 0.0;
        }

        // With a palette, the C++ thunk picks the index from the particle's life fraction.
        if !sys.color_palette.is_null() {
            unsafe {
                (svc.palette_plane)(
                    sys.color_palette,
                    p.age,
                    p.lifetime,
                    sys.color_delay,
                    ptr::from_mut::<FPlane>(&mut p.color).cast(),
                );
            }
        } else if size(v3(p.color_delta.x, p.color_delta.y, p.color_delta.z)) > 0.0
            && p.age > sys.color_delay
        {
            // Fused per channel: Color.X += ColorDelta.X*Timestep (W untouched).
            p.color.x = p.color_delta.x.mul_add(timestep, p.color.x);
            p.color.y = p.color_delta.y.mul_add(timestep, p.color.y);
            p.color.z = p.color_delta.z.mul_add(timestep, p.color.z);
        }

        if sys.render_primitive != PPRIM_LIQUID || p.drip_timer > 0.0 {
            if p.age < sys.size_grow_period * p.lifetime {
                // Still growing.
                let step = timestep / (sys.size_grow_period * p.lifetime);
                // Fused: Width = Min(Width + WidthFull*Step, WidthFull).
                p.width = c_min(p.width_full.mul_add(step, p.width), p.width_full);
                p.length = c_min(p.length_full.mul_add(step, p.length), p.length_full);
            } else if p.age > sys.size_delay {
                // Fused: Width += WidthDelta*Timestep.
                p.width = p.width_delta.mul_add(timestep, p.width);
                p.length = p.length_delta.mul_add(timestep, p.length);
            }
        }
    }

    // Bounce off level geometry.
    if sys.elasticity > 0.0
        && (unsafe {
            (svc.fast_line_check)(
                level,
                ptr::from_ref(&p.last_position).cast(),
                ptr::from_ref(&p.position).cast(),
            )
        }) == 0
    {
        let mut normal = vec_zero();
        let mut location = vec_zero();
        let hit = unsafe {
            (svc.system_trace_level_hit)(
                system,
                ptr::from_ref(&p.position).cast(),
                ptr::from_ref(&p.last_position).cast(),
                ptr::from_mut(&mut normal).cast(),
                ptr::from_mut(&mut location).cast(),
            )
        };
        if hit != 0 {
            // Velocity = Velocity - 2.0f * Hit.Normal * (Velocity|Hit.Normal).
            // The dot is computed from the pre-update velocity.
            let d = dot(p.velocity, normal);
            p.velocity = sub(p.velocity, scale(scale(normal, 2.0), d));
            p.velocity = scale(p.velocity, sys.elasticity);
            p.position = location;
        } else {
            p.velocity = vec_zero();
            p.position = p.last_position;
        }
    }

    // Age the particle.
    p.age += timestep;

    true
}

#[inline]
fn vec_zero() -> FVector {
    v3(0.0, 0.0, 0.0)
}

/// `UParticleListPriv`'s `Particles` plus its iterator state. Each particle is
/// boxed so the pointers handed out stay valid across `Add`/`Remove`, as they
/// do with `std::list` nodes.
#[derive(Default)]
pub struct ParticleList {
    // Boxes keep each particle at a stable address; `Vec<UParticle>` would move them.
    #[allow(clippy::vec_box, reason = "stable addresses like std::list nodes")]
    particles: Vec<Box<UParticle>>,
    /// `itParticles`: `None` is `end()`.
    cursor: Option<usize>,
    /// `bAdvance`: whether `next` moves the cursor.
    b_advance: bool,
}

impl ParticleList {
    /// Constructor state: `itParticles = begin()`, `bAdvance = true`.
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            cursor: None,
            b_advance: true,
        }
    }

    /// `StartParticle`: rewinds the cursor and returns the first particle.
    pub fn start(&mut self) -> *mut UParticle {
        self.cursor = if self.particles.is_empty() {
            None
        } else {
            Some(0)
        };
        self.current()
    }

    /// `NextParticle`: advances unless the last call hit `end()`.
    pub fn next_particle(&mut self) -> *mut UParticle {
        if self.b_advance {
            self.cursor = match self.cursor {
                Some(i) if i + 1 < self.particles.len() => Some(i + 1),
                Some(_) => None,
                // `++end()`: libc++ and libstdc++ `list` sentinels are circular,
                // so incrementing end() yields begin(). Matched here.
                None => (!self.particles.is_empty()).then_some(0),
            };
        }
        self.b_advance = true;
        self.current()
    }

    /// `GetCurrentParticle`: at `end()` sets `bAdvance = false` and returns null.
    fn current(&mut self) -> *mut UParticle {
        match self.cursor {
            Some(i) => ptr::from_mut(&mut *self.particles[i]),
            None => {
                self.b_advance = false;
                ptr::null_mut()
            }
        }
    }

    /// `Add`: appends a copy of the particle.
    pub fn add(&mut self, particle: &UParticle) {
        self.particles.push(Box::new(*particle));
    }

    /// `Remove(UParticle*)`. The cursor check reads `*itParticles`; at `end()`
    /// there is no current element, so the check is skipped (the C++ compares
    /// against the end sentinel and never matches a real particle).
    pub fn remove(&mut self, particle: *const UParticle) {
        if let Some(i) = self.cursor
            && ptr::eq(ptr::from_ref(&*self.particles[i]), particle)
        {
            self.particles.remove(i);
            self.cursor = (i < self.particles.len()).then_some(i);
            self.b_advance = false;
            return;
        }
        if let Some(i) = self
            .particles
            .iter()
            .position(|q| ptr::eq(ptr::from_ref(&**q), particle))
        {
            self.particles.remove(i);
            // Erasing before the cursor shifts its index; the cursor node itself is untouched.
            if let Some(c) = self.cursor
                && i < c
            {
                self.cursor = Some(c - 1);
            }
        }
    }

    /// `RemoveCurrent`: erases the element under the cursor. At `end()` the C++
    /// erases the sentinel (UB); here it is a no-op.
    pub fn remove_current(&mut self) {
        if let Some(i) = self.cursor {
            self.particles.remove(i);
            self.cursor = (i < self.particles.len()).then_some(i);
        }
        self.b_advance = false;
    }

    /// `RemoveFromFront`: `pop_front`. On an empty list the C++ is UB; here it
    /// is a no-op. If the cursor sat on the erased front node, the C++ iterator
    /// dangles (UB); here it keeps pointing at the new front.
    pub fn remove_from_front(&mut self) {
        if self.particles.is_empty() {
            return;
        }
        self.particles.remove(0);
        self.cursor = match self.cursor {
            Some(0) => (!self.particles.is_empty()).then_some(0),
            Some(i) => Some(i - 1),
            None => None,
        };
    }

    /// `RemoveAll`: `clear()`. The C++ cursor dangles afterwards (UB); here it
    /// is reset to `end()`.
    pub fn remove_all(&mut self) {
        self.particles.clear();
        self.cursor = None;
    }

    /// `Size`.
    pub fn size(&self) -> usize {
        self.particles.len()
    }

    /// `GetParticle(Id)`: the first particle with a matching `Id`, else null.
    pub fn get_particle(&mut self, id: i32) -> *mut UParticle {
        self.particles
            .iter_mut()
            .find(|p| p.id == id)
            .map_or(ptr::null_mut(), |p| ptr::from_mut(&mut **p))
    }

    /// `UpdateBox`: rebuilds the world bounds from every particle's position
    /// and `Max(Width,Length)*0.5` half-extent.
    pub fn update_box(&self, world_box: &mut FBox) {
        // FBox(0): empty, IsValid = 0. Only the public members are written;
        // the mirror's padding is left as-is.
        world_box.min = vec_zero();
        world_box.max = vec_zero();
        world_box.is_valid = 0;
        for p in &self.particles {
            let half = c_max(p.width, p.length) * 0.5;
            let extent = v3(half, half, half);
            box_add(world_box, sub(p.position, extent));
            box_add(world_box, add(p.position, extent));
        }
    }
}

/// `FBox::operator+=(const FVector&)` (`UnMath.h:1413`).
fn box_add(b: &mut FBox, v: FVector) {
    if b.is_valid != 0 {
        b.min = v3(
            c_min(b.min.x, v.x),
            c_min(b.min.y, v.y),
            c_min(b.min.z, v.z),
        );
        b.max = v3(
            c_max(b.max.x, v.x),
            c_max(b.max.y, v.y),
            c_max(b.max.z, v.z),
        );
    } else {
        b.min = v;
        b.max = v;
        b.is_valid = 1;
    }
}
