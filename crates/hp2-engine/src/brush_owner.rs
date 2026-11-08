//! Canonical Brush identity and the native ABrush coordinate policy.
//!
//! ABrush adds no construction, serialization or post-load work beyond Actor.
//! Its scale fields use compiler-probed FScale storage, including shear and
//! mirrored orientation; no C++ brush virtual is called.

use crate::actor_owner;
use hp2_abi::{FCoords, FVector};
use hp2_abi::engine::generated::{ABrush, FScale, FRotator};
use hp2_core::core_access::{RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::math;
use std::{alloc::Layout, ptr};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Brush", package: "Engine", super_name: Some("Actor"), within: "Object",
    flags: 0, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<ABrush>()), operations: Some(actor_owner::ACTOR_OPERATIONS),
    properties: &[], defaults: &[],
}];

#[inline]
fn multiply_scale(coords: FCoords, scale: FScale) -> FCoords {
    math::coords_mul_scale(coords, scale.scale, scale.sheer_rate, scale.sheer_axis)
}

#[inline]
fn divide_scale(coords: FCoords, scale: FScale) -> FCoords {
    math::coords_div_scale(coords, scale.scale, scale.sheer_rate, scale.sheer_axis)
}

#[inline]
fn rotation(value: FRotator) -> math::FRotator {
    math::FRotator { pitch: value.pitch, yaw: value.yaw, roll: value.roll }
}

/// `ABrush::ToWorld`, preserving native multiplication order.
///
/// # Safety
/// `brush` is a live canonical ABrush (or derived record), with no concurrent
/// mutation of its transform fields.
pub unsafe fn to_world(brush: *const ABrush) -> FCoords {
    unsafe {
        let mut coords = math::coords_mul_point(math::unit_coords(), (*brush).base.location);
        coords = multiply_scale(coords, (*brush).post_scale);
        coords = math::coords_mul_rotator(coords, rotation((*brush).base.rotation), math::trig_table());
        coords = multiply_scale(coords, (*brush).main_scale);
        math::coords_mul_point(coords, math::vector_neg((*brush).base.pre_pivot))
    }
}

/// `ABrush::ToLocal`, including nonuniform scale and shear.
///
/// # Safety
/// Same live-record and engine-thread requirements as [`to_world`].
pub unsafe fn to_local(brush: *const ABrush) -> FCoords {
    unsafe {
        let mut coords = math::coords_div_point(math::unit_coords(), math::vector_neg((*brush).base.pre_pivot));
        coords = divide_scale(coords, (*brush).main_scale);
        coords = math::coords_div_rotator(&coords, rotation((*brush).base.rotation), math::trig_table());
        coords = divide_scale(coords, (*brush).post_scale);
        math::coords_div_point(coords, (*brush).base.location)
    }
}

/// `ABrush::BuildCoords` for the forward point/vector transforms and orientation.
///
/// # Safety
/// Same live-record and engine-thread requirements as [`to_world`].
pub unsafe fn build_coords(brush: *const ABrush) -> (FCoords, FCoords, f32) {
    unsafe {
        let main = (*brush).main_scale;
        let post = (*brush).post_scale;
        let rot = rotation((*brush).base.rotation);
        let point = multiply_scale(
            math::coords_mul_rotator(multiply_scale(math::unit_coords(), post), rot, math::trig_table()),
            main,
        );
        let inverse = divide_scale(
            math::coords_div_rotator(&divide_scale(math::unit_coords(), main), rot, math::trig_table()),
            post,
        );
        (point, math::coords_transpose(inverse),
            math::scale_orientation(main.scale) * math::scale_orientation(post.scale))
    }
}

/// `ABrush::InitPosRotScale` resets the two actual brush scales and actor pose.
///
/// # Safety
/// `brush` is an exclusively mutable live canonical ABrush record.
pub unsafe fn init_pos_rot_scale(brush: *mut ABrush) -> RuntimeResult<()> {
    if brush.is_null() || unsafe { (*brush).base.brush.is_null() } {
        return Err(RuntimeError::new("ABrush::InitPosRotScale requires Brush"));
    }
    unsafe {
        // The generated FScale padding is opaque; copy the genuine immutable
        // native unit record rather than constructing a second scale default.
        const { assert!(std::mem::size_of::<FScale>() == std::mem::size_of::<math::NativeScale>()); }
        let unit = ptr::addr_of!(math::UNIT_SCALE).cast::<FScale>().read_unaligned();
        ptr::addr_of_mut!((*brush).main_scale).write_unaligned(unit);
        ptr::addr_of_mut!((*brush).post_scale).write_unaligned(unit);
        let zero = FVector { x: 0.0, y: 0.0, z: 0.0 };
        ptr::addr_of_mut!((*brush).base.location).write_unaligned(zero);
        ptr::addr_of_mut!((*brush).base.rotation).write_unaligned(FRotator { pitch: 0, yaw: 0, roll: 0 });
        ptr::addr_of_mut!((*brush).base.pre_pivot).write_unaligned(zero);
    }
    Ok(())
}
