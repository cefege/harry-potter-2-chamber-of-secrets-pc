//! Interpolation point and manager physics (`UnPhysic.cpp`).

use core::ffi::c_void;
use hp2_abi::engine::generated::{AActor, AInterpolationManager, AInterpolationPoint, FRotator};
use hp2_abi::{FCoords, FVector};

use super::{Hp2PhysicServices, add, app_sqrt, blank_hit, div, dot, normalize, scale, size, sub};
use crate::coords::{self, Trig};

fn rot_scale(s: f32, r: FRotator) -> FRotator {
    FRotator {
        pitch: (r.pitch as f32 * s) as i32,
        yaw: (r.yaw as f32 * s) as i32,
        roll: (r.roll as f32 * s) as i32,
    }
}
fn rot_add(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_add(b.pitch),
        yaw: a.yaw.wrapping_add(b.yaw),
        roll: a.roll.wrapping_add(b.roll),
    }
}
fn rot_sub(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_sub(b.pitch),
        yaw: a.yaw.wrapping_sub(b.yaw),
        roll: a.roll.wrapping_sub(b.roll),
    }
}
unsafe fn point_rotation(point: *mut AInterpolationPoint) -> FRotator {
    unsafe { core::ptr::addr_of!((*point).base.base.rotation).read_unaligned() }
}
unsafe fn point_location(point: *mut AInterpolationPoint) -> FVector {
    unsafe { core::ptr::addr_of!((*point).base.base.location).read_unaligned() }
}

/// `AInterpolationPoint::GetDesiredRotationAtPause`.
///
/// # Safety
/// `this` and every actor it links must be live engine objects.
pub unsafe fn get_desired_rotation_at_pause(
    this: *mut AInterpolationPoint,
    svc: &Hp2PhysicServices,
    pause_num: i32,
    new_location: FVector,
) -> FRotator {
    let tag_ptr = unsafe {
        core::ptr::addr_of!((*this).view_target_tag)
            .cast::<[u8; 4]>()
            .add(pause_num as usize)
    };
    let tag = unsafe { tag_ptr.read_unaligned() };
    if tag != [0; 4] {
        let cached_ptr = unsafe {
            core::ptr::addr_of!((*this).view_target)
                .cast::<*mut c_void>()
                .add(pause_num as usize)
        };
        let mut target = unsafe { cached_ptr.read_unaligned() };
        if target.is_null()
            || unsafe {
                core::ptr::addr_of!((*(target.cast::<AActor>())).tag).read_unaligned() != tag
            }
        {
            let found = unsafe { (svc.find_view_target)(this.cast(), tag_ptr) };
            if !found.is_null() {
                target = found;
                unsafe { core::ptr::write_unaligned(cached_ptr as *mut *mut c_void, target) };
            }
        }
        if !target.is_null() {
            let location = unsafe {
                core::ptr::addr_of!((*(target.cast::<AActor>())).location).read_unaligned()
            };
            let direction = sub(location, new_location);
            let mut rotation = FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            };
            unsafe { (svc.rotation_from_vector)(&direction, &mut rotation) };
            return rotation;
        }
    }
    unsafe { point_rotation(this) }
}

/// `AInterpolationPoint::GetDesiredRotationAtPosition`.
///
/// # Safety
/// `this` and every actor it links must be live engine objects.
pub unsafe fn get_desired_rotation_at_position(
    this: *mut AInterpolationPoint,
    svc: &Hp2PhysicServices,
    mut pos_offset: i32,
    mut pause_num: i32,
    new_location: FVector,
) -> FRotator {
    if pos_offset < 0 {
        if pause_num > 0 {
            return unsafe {
                get_desired_rotation_at_position(
                    this,
                    svc,
                    pos_offset + 1,
                    pause_num - 1,
                    new_location,
                )
            };
        }
        let prev = unsafe {
            core::ptr::addr_of!((*this).prev)
                .read_unaligned()
                .cast::<AInterpolationPoint>()
        };
        if unsafe { (*this).b_instant_next_path() } || unsafe { (*prev).b_end_of_path() } {
            return unsafe { get_desired_rotation_at_pause(this, svc, pause_num, new_location) };
        }
        let mut i = 0;
        while unsafe {
            core::ptr::addr_of!((*prev).pause)
                .cast::<f32>()
                .add(i as usize)
                .read_unaligned()
        } > 0.0
        {
            i += 1;
        }
        return unsafe {
            get_desired_rotation_at_position(prev, svc, pos_offset + 1, i, new_location)
        };
    }
    if pos_offset == 0 {
        return unsafe { get_desired_rotation_at_pause(this, svc, pause_num, new_location) };
    }
    while unsafe {
        core::ptr::addr_of!((*this).pause)
            .cast::<f32>()
            .add((pause_num + 1) as usize)
            .read_unaligned()
    } > 0.0
        && pos_offset > 0
    {
        pause_num += 1;
        pos_offset -= 1;
    }
    let next = unsafe {
        core::ptr::addr_of!((*this).next)
            .read_unaligned()
            .cast::<AInterpolationPoint>()
    };
    if unsafe { (*this).b_end_of_path() || pos_offset == 0 || (*next).b_instant_next_path() } {
        unsafe { get_desired_rotation_at_pause(this, svc, pause_num, new_location) }
    } else {
        unsafe { get_desired_rotation_at_position(next, svc, pos_offset - 1, 0, new_location) }
    }
}

fn rotate_coords(rot: FRotator, svc: &Hp2PhysicServices) -> FCoords {
    let mut coords = coords::unit_coords();
    let trig = Trig {
        sin: &|i| unsafe { (svc.sin_tab)(i) },
        cos: &|i| unsafe { (svc.cos_tab)(i) },
    };
    coords::div_rot(&mut coords, rot, &trig);
    coords
}
unsafe fn direction_rotation(svc: &Hp2PhysicServices, vector: FVector) -> FRotator {
    let mut rotation = FRotator {
        pitch: 0,
        yaw: 0,
        roll: 0,
    };
    unsafe { (svc.rotation_from_vector)(&vector, &mut rotation) };
    rotation
}
unsafe fn find_target(
    this: *mut AInterpolationPoint,
    svc: &Hp2PhysicServices,
    pause_num: i32,
) -> *mut AActor {
    let tagp = unsafe {
        core::ptr::addr_of!((*this).view_target_tag)
            .cast::<[u8; 4]>()
            .add(pause_num as usize)
    };
    let tag = unsafe { tagp.read_unaligned() };
    let cache = unsafe {
        core::ptr::addr_of!((*this).view_target)
            .cast::<*mut c_void>()
            .add(pause_num as usize)
    };
    let mut target = unsafe { cache.read_unaligned() };
    if target.is_null()
        || unsafe { core::ptr::addr_of!((*(target.cast::<AActor>())).tag).read_unaligned() != tag }
    {
        let found = unsafe { (svc.find_view_target)(this.cast(), tagp) };
        if !found.is_null() {
            target = found;
            unsafe { core::ptr::write_unaligned(cache as *mut *mut c_void, target) };
        }
    }
    target.cast()
}

/// `AInterpolationPoint::GetInterpolatedPosition`.
///
/// # Safety
/// `this` and every actor it links must be live engine objects.
pub unsafe fn get_interpolated_position(
    this: *mut AInterpolationPoint,
    svc: &Hp2PhysicServices,
    old_coords: FCoords,
    start_control: FVector,
    phys_alpha: f32,
    pause_num: i32,
) -> FCoords {
    let rotation = unsafe { point_rotation(this) };
    if unsafe { (*this).b_instant_next_path() } {
        let mut out = rotate_coords(rotation, svc);
        out.origin = unsafe { point_location(this) };
        return out;
    }
    let location = unsafe { point_location(this) };
    let end_control = unsafe { core::ptr::addr_of!((*this).end_control_point).read_unaligned() };
    let r = 1.0 - phys_alpha;
    let w0 = r * r * r;
    let w1 = 3.0 * phys_alpha * r * r;
    let w2 = 3.0 * phys_alpha * phys_alpha * r;
    let w3 = phys_alpha * phys_alpha * phys_alpha;
    let new_location = add(
        add(
            add(
                scale(old_coords.origin, w0),
                scale(add(old_coords.origin, start_control), w1),
            ),
            scale(add(location, end_control), w2),
        ),
        scale(location, w3),
    );
    let mut out;
    if unsafe { (*this).b_face_move_direction() } {
        let back = phys_alpha - 0.001;
        if back <= 0.0 {
            let prev = unsafe {
                core::ptr::addr_of!((*this).prev)
                    .read_unaligned()
                    .cast::<AInterpolationPoint>()
            };
            out = rotate_coords(
                if prev.is_null() {
                    rotation
                } else {
                    unsafe { point_rotation(prev) }
                },
                svc,
            );
        } else {
            let br = 1.0 - back;
            let bw0 = br * br * br;
            let bw1 = 3.0 * back * br * br;
            let bw2 = 3.0 * back * back * br;
            let bw3 = back * back * back;
            let p = add(
                add(
                    add(
                        scale(old_coords.origin, bw0),
                        scale(add(old_coords.origin, start_control), bw1),
                    ),
                    scale(add(location, end_control), bw2),
                ),
                scale(location, bw3),
            );
            out = rotate_coords(
                unsafe { direction_rotation(svc, sub(new_location, p)) },
                svc,
            );
        }
    } else if unsafe { (*this).b_new_rotation_smoothing() } {
        let a = phys_alpha * phys_alpha;
        let three = a * phys_alpha;
        let rots = [
            unsafe { get_desired_rotation_at_position(this, svc, -2, pause_num, new_location) },
            unsafe { get_desired_rotation_at_position(this, svc, -1, pause_num, new_location) },
            unsafe { get_desired_rotation_at_position(this, svc, 0, pause_num, new_location) },
            unsafe { get_desired_rotation_at_position(this, svc, 1, pause_num, new_location) },
        ];
        let (aa, b, c, d) = (rots[0], rots[1], rots[2], rots[3]);
        let term0 = rot_scale(2.0 * three - 3.0 * a + 1.0, b);
        let term1 = rot_scale(-2.0 * three + 3.0 * a, c);
        let term2 = rot_scale((three - 2.0 * a + phys_alpha) * 0.5, rot_sub(c, aa));
        let term3 = rot_scale((three - a) * 0.5, rot_sub(d, b));
        out = rotate_coords(rot_add(rot_add(rot_add(term0, term1), term2), term3), svc);
    } else {
        let tag = unsafe {
            core::ptr::addr_of!((*this).view_target_tag)
                .cast::<[u8; 4]>()
                .add(pause_num as usize)
                .read_unaligned()
        };
        if tag != [0; 4] {
            let target = unsafe { find_target(this, svc, pause_num) };
            if target.is_null() {
                out = old_coords;
            } else {
                let loc = unsafe { core::ptr::addr_of!((*target).location).read_unaligned() };
                out = rotate_coords(
                    unsafe { direction_rotation(svc, sub(loc, new_location)) },
                    svc,
                );
            }
        } else {
            let turn = if unsafe { this.cast::<u8>().add(960).read() & 2 != 0 } {
                app_sqrt(phys_alpha as f64) as f32
            } else {
                phys_alpha
            };
            let current = rotate_coords(rotation, svc);
            out = coords::unit_coords();
            out.x_axis = add(scale(old_coords.x_axis, r), scale(current.x_axis, turn));
            normalize(&mut out.x_axis);
            out.z_axis = add(scale(old_coords.z_axis, r), scale(current.z_axis, turn));
            out.y_axis = crate::prim::cross(out.z_axis, out.x_axis);
            normalize(&mut out.y_axis);
            out.z_axis = crate::prim::cross(out.x_axis, out.y_axis);
            normalize(&mut out.z_axis);
        }
    }
    out.origin = new_location;
    out
}

/// `FCoords::OrthoRotation` (`UnMath.cpp`).
pub(super) fn ortho_rotation(c: FCoords, svc: &Hp2PhysicServices) -> FRotator {
    let atan =
        |y: f32, x: f32| (f64::from(y).atan2(f64::from(x)) * 32768.0 / std::f64::consts::PI) as i32;
    let horizontal = app_sqrt((c.x_axis.x * c.x_axis.x + c.x_axis.y * c.x_axis.y) as f64) as f32;
    let mut r = FRotator {
        pitch: atan(c.x_axis.z, horizontal),
        yaw: atan(c.x_axis.y, c.x_axis.x),
        roll: 0,
    };
    let mut s = coords::unit_coords();
    let trig = Trig {
        sin: &|i| unsafe { (svc.sin_tab)(i) },
        cos: &|i| unsafe { (svc.cos_tab)(i) },
    };
    coords::div_rot(&mut s, r, &trig);
    r.roll = atan(dot(c.z_axis, s.y_axis), dot(c.y_axis, s.y_axis));
    r
}

/// `AInterpolationManager::performPhysics`.
///
/// # Safety
/// The manager, its points, and `svc` callbacks must be live.
pub unsafe fn perform_physics(
    this: *mut AInterpolationManager,
    svc: &Hp2PhysicServices,
    delta_time: f32,
) {
    let actor = this.cast::<AActor>();
    let mut owner = unsafe {
        core::ptr::addr_of!((*actor).owner)
            .read_unaligned()
            .cast::<AActor>()
    };
    if owner.is_null() || unsafe { (*owner).b_delete_me() } {
        unsafe { (svc.destroy_actor)(this.cast()) };
        return;
    }
    let mut old_location = unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
    let old_velocity = unsafe { core::ptr::addr_of!((*owner).velocity).read_unaligned() };
    let mut remaining = delta_time;
    let mut desired_speed = 0.0f32;
    while unsafe { core::ptr::addr_of!((*this).phys_rate).read_unaligned() } != 0.0
        && unsafe { (*owner).b_interpolating() }
        && remaining > 0.0
    {
        let mut segment = unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
        let mut dest = unsafe {
            core::ptr::addr_of!((*this).dest)
                .read_unaligned()
                .cast::<AInterpolationPoint>()
        };
        let alpha = unsafe { core::ptr::addr_of!((*this).phys_alpha).read_unaligned() };
        let rate = unsafe { core::ptr::addr_of!((*this).phys_rate).read_unaligned() };
        let mut rate_mod = 1.0;
        if !dest.is_null() {
            let own_speed = unsafe { core::ptr::addr_of!((*owner).ip_speed).read_unaligned() };
            if own_speed != 0.0 {
                desired_speed = own_speed;
            } else {
                let next = unsafe {
                    core::ptr::addr_of!((*dest).next)
                        .read_unaligned()
                        .cast::<AInterpolationPoint>()
                };
                if !next.is_null() {
                    let ds = unsafe { core::ptr::addr_of!((*dest).desired_speed).read_unaligned() };
                    let current = if ds > 0.0 {
                        ds
                    } else {
                        size(unsafe { core::ptr::addr_of!((*owner).velocity).read_unaligned() })
                    };
                    let nd = unsafe { core::ptr::addr_of!((*next).desired_speed).read_unaligned() };
                    desired_speed = if nd > 0.0 {
                        current.mul_add(1.0 - alpha, nd * alpha)
                    } else if ds > 0.0 {
                        ds
                    } else {
                        0.0
                    };
                }
            }
            let path = unsafe { core::ptr::addr_of!((*dest).path_dist).read_unaligned() };
            if desired_speed > 0.0 {
                if path > 0.0 {
                    rate_mod = desired_speed / path;
                }
            } else {
                let end = size(unsafe {
                    core::ptr::addr_of!((*dest).end_control_point).read_unaligned()
                });
                let start = size(unsafe {
                    core::ptr::addr_of!((*dest).start_control_point).read_unaligned()
                });
                rate_mod = alpha.mul_add(end / start - 1.0, 1.0);
            }
        }
        let old_alpha = alpha;
        let mut dest_alpha = (rate * rate_mod).mul_add(remaining, alpha);
        let mut new_alpha = dest_alpha.clamp(0.0, 1.0);
        unsafe { core::ptr::addr_of_mut!((*this).phys_alpha).write_unaligned(new_alpha) };
        let mut actual = remaining;
        let mut done = false;
        if dest_alpha > 1.0
            && unsafe { core::ptr::addr_of!((*this).remaining_pause).read_unaligned() } <= 0.0
        {
            actual = (rate * rate_mod).mul_add(remaining, -dest_alpha) + 1.0;
        }
        if dest.is_null() {
            done = true;
        } else {
            let pause = unsafe { core::ptr::addr_of!((*this).remaining_pause).read_unaligned() };
            let pause_num = unsafe { core::ptr::addr_of!((*this).pause_num).read_unaligned() };
            let mut newloc = segment;
            let mut coords;
            if pause > 0.0 {
                desired_speed = 0.0;
                let mut remaining_pause = pause - remaining;
                if remaining_pause < 0.0 {
                    remaining = -remaining_pause;
                    remaining_pause = 0.0;
                    done = true;
                }
                unsafe {
                    core::ptr::addr_of_mut!((*this).phys_alpha).write_unaligned(0.0);
                    core::ptr::addr_of_mut!((*this).remaining_pause)
                        .write_unaligned(remaining_pause);
                }
                let start_pause =
                    unsafe { core::ptr::addr_of!((*this).start_pause).read_unaligned() };
                unsafe {
                    (svc.event_update_camera)(
                        this.cast(),
                        (start_pause - remaining_pause) / start_pause,
                    )
                };
                owner = unsafe {
                    core::ptr::addr_of!((*actor).owner)
                        .read_unaligned()
                        .cast::<AActor>()
                };
                if owner.is_null() {
                    return;
                }
                dest = unsafe {
                    core::ptr::addr_of!((*this).dest)
                        .read_unaligned()
                        .cast::<AInterpolationPoint>()
                };
                new_alpha = unsafe { core::ptr::addr_of!((*this).phys_alpha).read_unaligned() };
                coords = coords::unit_coords();
                coords.origin = unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
            } else {
                unsafe { (svc.event_update_camera)(this.cast(), old_alpha) };
                owner = unsafe {
                    core::ptr::addr_of!((*actor).owner)
                        .read_unaligned()
                        .cast::<AActor>()
                };
                if owner.is_null() {
                    return;
                }
                dest = unsafe {
                    core::ptr::addr_of!((*this).dest)
                        .read_unaligned()
                        .cast::<AInterpolationPoint>()
                };
                new_alpha = unsafe { core::ptr::addr_of!((*this).phys_alpha).read_unaligned() };
                if dest.is_null() {
                    newloc = unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
                    coords = coords::unit_coords();
                    coords.origin = newloc;
                    done = true;
                } else {
                    let prev = unsafe {
                        core::ptr::addr_of!((*dest).prev)
                            .read_unaligned()
                            .cast::<AInterpolationPoint>()
                    };
                    let oldrot = if prev.is_null() {
                        unsafe { point_rotation(dest) }
                    } else {
                        unsafe { point_rotation(prev) }
                    };
                    let mut oldcoords = rotate_coords(oldrot, svc);
                    let start = if prev.is_null() {
                        FVector {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        }
                    } else {
                        unsafe { core::ptr::addr_of!((*prev).start_control_point).read_unaligned() }
                    };
                    if prev.is_null() {
                        oldcoords.origin =
                            unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
                    } else {
                        oldcoords.origin = unsafe { point_location(prev) };
                    }
                    coords = unsafe {
                        get_interpolated_position(dest, svc, oldcoords, start, new_alpha, pause_num)
                    };
                    newloc = coords.origin;
                    if unsafe { (*dest).b_constant_speed() }
                        && desired_speed > 0.0
                        && new_alpha < 1.0
                        && !unsafe { (*dest).b_instant_next_path() }
                    {
                        let new_speed = size(sub(newloc, segment)) / actual;
                        if (new_speed - desired_speed).abs() as f64 > 0.05f64 * desired_speed as f64
                        {
                            dest_alpha =
                                old_alpha + rate * rate_mod * actual * desired_speed / new_speed;
                            let a = dest_alpha.clamp(0.0, 1.0);
                            unsafe {
                                core::ptr::addr_of_mut!((*this).phys_alpha).write_unaligned(a)
                            };
                            coords = unsafe {
                                get_interpolated_position(dest, svc, oldcoords, start, a, pause_num)
                            };
                            newloc = coords.origin;
                            if dest_alpha > 1.0
                                && unsafe {
                                    core::ptr::addr_of!((*this).remaining_pause).read_unaligned()
                                } <= 0.0
                            {
                                actual = (rate * rate_mod).mul_add(remaining, -dest_alpha) + 1.0;
                            }
                        }
                    }
                    if unsafe { (*dest).b_face_move_direction() } {
                        coords = rotate_coords(
                            unsafe { direction_rotation(svc, sub(coords.origin, old_location)) },
                            svc,
                        );
                        coords.origin = newloc;
                    }
                }
            }
            if !dest.is_null() && !unsafe { (*dest).b_new_rotation_smoothing() } {
                let owner_rot = unsafe { core::ptr::addr_of!((*owner).rotation).read_unaligned() };
                let current = rotate_coords(owner_rot, svc);
                let old_x = unsafe { core::ptr::addr_of!((*this).old_desired_x).read_unaligned() };
                let old_z = unsafe { core::ptr::addr_of!((*this).old_desired_z).read_unaligned() };
                let mut rate_x = div(sub(coords.x_axis, old_x), actual);
                let mut rate_z = div(sub(coords.z_axis, old_z), actual);
                unsafe {
                    core::ptr::addr_of_mut!((*this).old_desired_x).write_unaligned(coords.x_axis);
                    core::ptr::addr_of_mut!((*this).old_desired_z).write_unaligned(coords.z_axis);
                }
                let prev = unsafe {
                    core::ptr::addr_of!((*dest).prev)
                        .read_unaligned()
                        .cast::<AInterpolationPoint>()
                };
                let smoothing = if prev.is_null() {
                    1.0
                } else {
                    let s = unsafe { core::ptr::addr_of!((*prev).smoothing).read_unaligned() };
                    if s == 0.0 { 0.0 } else { 1.0 / s }
                };
                let mut turn_x =
                    unsafe { core::ptr::addr_of!((*this).turn_rate_x).read_unaligned() };
                let mut turn_z =
                    unsafe { core::ptr::addr_of!((*this).turn_rate_z).read_unaligned() };
                let smooth_x = div(sub(coords.x_axis, current.x_axis), smoothing);
                let smooth_z = div(sub(coords.z_axis, current.z_axis), smoothing);
                if dot(smooth_x, rate_x) <= 0.0 {
                    rate_x = smooth_x;
                }
                if dot(smooth_z, rate_z) <= 0.0 {
                    rate_z = smooth_z;
                }
                if smoothing != 0.0 {
                    let t = smoothing * actual;
                    let omt = (-smoothing).mul_add(actual, 1.0);
                    turn_x = add(scale(rate_x, t), scale(turn_x, omt));
                    turn_z = add(scale(rate_z, t), scale(turn_z, omt));
                    coords.x_axis = FVector {
                        x: current.x_axis.x + actual * turn_x.x,
                        y: current.x_axis.y + actual * turn_x.y,
                        z: current.x_axis.z + actual * turn_x.z,
                    };
                    normalize(&mut coords.x_axis);
                    coords.z_axis = FVector {
                        x: current.z_axis.x + actual * turn_z.x,
                        y: current.z_axis.y + actual * turn_z.y,
                        z: current.z_axis.z + actual * turn_z.z,
                    };
                    normalize(&mut coords.z_axis);
                    coords.y_axis = crate::prim::cross(coords.z_axis, coords.x_axis);
                    normalize(&mut coords.y_axis);
                    coords.z_axis = crate::prim::cross(coords.x_axis, coords.y_axis);
                    normalize(&mut coords.z_axis);
                }
                turn_x = div(sub(coords.x_axis, current.x_axis), actual);
                turn_z = div(sub(coords.z_axis, current.z_axis), actual);
                unsafe {
                    core::ptr::addr_of_mut!((*this).turn_rate_x).write_unaligned(turn_x);
                    core::ptr::addr_of_mut!((*this).turn_rate_z).write_unaligned(turn_z);
                }
                let _ = (smooth_x, smooth_z);
            }
            if !dest.is_null() {
                coords.origin = FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                };
                let mut hit = blank_hit(1.0);
                let delta = sub(newloc, unsafe {
                    core::ptr::addr_of!((*owner).location).read_unaligned()
                });
                let rot = if unsafe { (*owner).b_interpolating_ignore_rot() } {
                    unsafe { core::ptr::addr_of!((*owner).rotation).read_unaligned() }
                } else {
                    ortho_rotation(coords, svc)
                };
                unsafe { (svc.move_actor)(owner.cast(), &delta, &rot, &mut hit) };
                owner = unsafe {
                    core::ptr::addr_of!((*actor).owner)
                        .read_unaligned()
                        .cast::<AActor>()
                };
                if owner.is_null() {
                    return;
                }
            }
        }
        let forward = rate > 0.0;
        if forward && dest_alpha > 1.0 {
            unsafe { core::ptr::addr_of_mut!((*this).phys_alpha).write_unaligned(0.0) };
            remaining *= (dest_alpha - 1.0) / (dest_alpha - old_alpha);
            done = true
        } else if !forward && dest_alpha < 0.0 {
            unsafe { core::ptr::addr_of_mut!((*this).phys_alpha).write_unaligned(1.0) };
            remaining *= (0.0 - dest_alpha) / (old_alpha - dest_alpha);
            done = true
        }
        if done {
            if !dest.is_null() {
                unsafe { (svc.event_interpolate_end)(dest.cast(), this.cast(), forward as i32) };
            } else {
                unsafe { (svc.event_finished_interpolation)(this.cast()) };
            }
            owner = unsafe {
                core::ptr::addr_of!((*actor).owner)
                    .read_unaligned()
                    .cast::<AActor>()
            };
            if owner.is_null() {
                return;
            }
            dest = unsafe {
                core::ptr::addr_of!((*this).dest)
                    .read_unaligned()
                    .cast::<AInterpolationPoint>()
            };
            let instant = unsafe { this.cast::<u8>().add(1008).read() & 1 != 0 };
            if instant {
                unsafe {
                    this.cast::<u8>()
                        .add(1008)
                        .write(this.cast::<u8>().add(1008).read() & !1)
                };
                old_location = unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
                segment = old_location;
            }
            let location = unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
            let vel = div(sub(location, segment), delta_time);
            unsafe { core::ptr::addr_of_mut!((*owner).velocity).write_unaligned(vel) };
            if size(vel) >= desired_speed {
                remaining = 0.0
            }
        } else {
            remaining = 0.0;
        }
    }
    let location = unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() };
    let mut velocity = div(sub(location, old_location), delta_time);
    unsafe { core::ptr::addr_of_mut!((*owner).velocity).write_unaligned(velocity) };
    let new_speed = size(velocity);
    if desired_speed > 0.0 && new_speed > 1.05 * desired_speed {
        let rotation = unsafe { core::ptr::addr_of!((*owner).rotation).read_unaligned() };
        let factor = desired_speed / new_speed;
        let delta = add(
            add(scale(sub(location, old_location), factor), old_location),
            scale(location, -1.0),
        );
        let mut hit = blank_hit(1.0);
        unsafe { (svc.move_actor)(owner.cast(), &delta, &rotation, &mut hit) };
        owner = unsafe {
            core::ptr::addr_of!((*actor).owner)
                .read_unaligned()
                .cast::<AActor>()
        };
        if owner.is_null() {
            return;
        }
        velocity = div(
            sub(
                unsafe { core::ptr::addr_of!((*owner).location).read_unaligned() },
                old_location,
            ),
            delta_time,
        );
        unsafe { core::ptr::addr_of_mut!((*owner).velocity).write_unaligned(velocity) };
    }
    let accel = div(sub(velocity, old_velocity), delta_time);
    unsafe { core::ptr::addr_of_mut!((*owner).acceleration).write_unaligned(accel) };
}
