//! Listener projection and velocity, ported from `UALAudioSubsystem::Update`
//! (`ALAudioSubsystem.cpp` @06bfdd3 lines 1341-1374 for orientation,
//! 1736-1744 for velocity).
//!
//! The C++ shell still resolves the `SceneNode` coordinates, the
//! `UseEAX`/`UseMMSYSTEM`/`ReverseStereo` config, and the Viewport. This
//! module owns the arithmetic between those and the `alListenerfv` calls.

use crate::vec3::{self, Vec3};

/// `MAX_LISTENER_VELOCITY` (`ALAudioSubsystem.h`).
pub const MAX_LISTENER_VELOCITY: f32 = 50.0;

/// `MAX_SOURCE_VELOCITY` (`ALAudioSubsystem.h`).
pub const MAX_SOURCE_VELOCITY: f32 = 300.0;

/// `DISTANCE_FACTOR` (`ALAudioSubsystem.h`): `0.0254f/2.f`.
pub const DISTANCE_FACTOR: f32 = 0.0254f32 / 2.0f32;

/// The three projection vectors fed to `alListenerfv(AL_ORIENTATION)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orientation {
    pub up: Vec3,
    pub right: Vec3,
    pub front: Vec3,
}

/// Computes `ProjUp`, `ProjRight`, `ProjFront` from the scene-node axes.
///
/// `ds3d` is `UseEAX && !UseMMSYSTEM` (DirectSound3D coordinates). The
/// non-DS3D branch negates the Z of `up`, the Y of `right`, and the Z of
/// `front` after the cross product; the DS3D branch negates only the Z of
/// `right`. Each vector is then normalized and a failed normalize leaves
/// the vector as-is, as `FVector::Normalize` does.
pub fn orientation(
    x_axis: Vec3,
    y_axis: Vec3,
    z_axis: Vec3,
    ds3d: bool,
    reverse_stereo: bool,
) -> Orientation {
    let mut up;
    let mut right;
    let mut front;
    if ds3d {
        up = z_axis;

        right = if reverse_stereo {
            vec3::neg(x_axis)
        } else {
            x_axis
        };
        right[2] = -right[2];

        front = vec3::cross(right, up);
    } else {
        up = vec3::neg(y_axis);
        up[2] = -up[2];

        right = if reverse_stereo {
            vec3::neg(x_axis)
        } else {
            x_axis
        };
        right[1] = -right[1];

        front = vec3::cross(right, up);
        front[2] = -front[2];
    }

    vec3::normalize(&mut up);
    vec3::normalize(&mut right);
    vec3::normalize(&mut front);

    Orientation { up, right, front }
}

/// `(ViewLocation - LastPosition) / DeltaTime * DISTANCE_FACTOR`
/// (`ALAudioSubsystem.cpp:1736`).
pub fn listener_velocity(view_location: Vec3, last_position: Vec3, delta_time: f32) -> Vec3 {
    vec3::scale(
        vec3::div(vec3::sub(view_location, last_position), delta_time),
        DISTANCE_FACTOR,
    )
}

/// `Velocity.Size() < MAX_LISTENER_VELOCITY` (`ALAudioSubsystem.cpp:1742`).
#[inline]
pub fn listener_velocity_ok(velocity: Vec3) -> bool {
    vec3::size(velocity) < MAX_LISTENER_VELOCITY
}

/// `Velocity.Size() < MAX_SOURCE_VELOCITY` (`ALAudioSubsystem.cpp:1719`).
#[inline]
pub fn source_velocity_ok(velocity: Vec3) -> bool {
    vec3::size(velocity) < MAX_SOURCE_VELOCITY
}

/// `DeltaTime = Clamp(Current - Last, 0.0001f, 1.0f)` (`ALAudioSubsystem.cpp:1383-1385`).
#[inline]
pub fn delta_time(current_seconds: f32, last_seconds: f32) -> f32 {
    vec3::clamp(current_seconds - last_seconds, 0.0001, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Inputs are the `SceneNode->Coords` axes of the fixed-case oracle; expected bits
    // come from a build with the TU's real flags (`-O3`, default `-ffp-contract=on`).
    const X: Vec3 = [0.6, 0.8, 0.0];
    const Y: Vec3 = [-0.2, 0.5, 0.84];
    const Z: Vec3 = [0.1, -0.7, 0.7];

    fn bits(v: Vec3) -> [u32; 3] {
        [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]
    }

    /// Oracle: `fixed_orient 0` (`EAX=0`, `MMSYS=0`, `REV=0`), built with the TU's
    /// real flags (`-O3`, default `-ffp-contract=on`).
    #[test]
    fn orientation_matches_original_non_ds3d() {
        let o = orientation(X, Y, Z, false, false);
        assert_eq!(bits(o.up), [0x3e4d4087, 0xbf004854, 0x3f5783c0]);
        assert_eq!(bits(o.right), [0x3f199999, 0xbf4ccccc, 0x00000000]);
        assert_eq!(bits(o.front), [0xbf4a0373, 0xbf178297, 0x3e285835]);
    }

    /// Oracle: `fixed_orient 1` (`EAX=1`, `MMSYS=0`, `REV=0`).
    #[test]
    fn orientation_matches_original_ds3d() {
        let o = orientation(X, Y, Z, true, false);
        assert_eq!(bits(o.up), [0x3dcdd4ed, 0xbf341a4f, 0x3f341a4f]);
        assert_eq!(bits(o.right), [0x3f199999, 0x3f4ccccc, 0x80000000]);
        assert_eq!(bits(o.front), [0x3f26a712, 0xbef9fa9c, 0xbf14cc07]);
    }

    /// Oracle: `fixed_orient 4` (`EAX=0`, `MMSYS=0`, `REV=1`).
    #[test]
    fn orientation_matches_original_reverse_stereo() {
        let o = orientation(X, Y, Z, false, true);
        assert_eq!(bits(o.up), [0x3e4d4087, 0xbf004854, 0x3f5783c0]);
        assert_eq!(bits(o.right), [0xbf199999, 0x3f4ccccc, 0x80000000]);
        assert_eq!(bits(o.front), [0x3f4a0373, 0x3f178297, 0xbe285835]);
    }

    /// Oracle: `fixed_orient 5` (`EAX=1`, `MMSYS=0`, `REV=1`).
    #[test]
    fn orientation_matches_original_ds3d_reverse_stereo() {
        let o = orientation(X, Y, Z, true, true);
        assert_eq!(bits(o.up), [0x3dcdd4ed, 0xbf341a4f, 0x3f341a4f]);
        assert_eq!(bits(o.right), [0xbf199999, 0xbf4ccccc, 0x00000000]);
        assert_eq!(bits(o.front), [0xbf26a712, 0x3ef9fa9c, 0x3f14cc07]);
    }

    /// Oracle: `fixed_lisvel` row 0: View (1,2,3), Last 0, DeltaTime 0.5.
    #[test]
    fn listener_velocity_matches_original_small() {
        let v = listener_velocity([1.0, 2.0, 3.0], [0.0; 3], 0.5);
        assert_eq!(bits(v), [0x3cd013a9, 0x3d5013a9, 0x3d9c0ebf]);
        assert!(listener_velocity_ok(v));
    }

    /// Oracle: `fixed_lisvel` row 1: View (100,200,300), Last 0, DeltaTime 0.1.
    #[test]
    fn listener_velocity_matches_original_big() {
        let v = listener_velocity([100.0, 200.0, 300.0], [0.0; 3], 0.1);
        assert_eq!(bits(v), [0x414b3333, 0x41cb3333, 0x42186666]);
        assert!(listener_velocity_ok(v));
    }

    /// Oracle: `fixed_srcvel` rows: `Velocity *= DISTANCE_FACTOR`, then `Size() < 300`.
    #[test]
    fn source_velocity_matches_original() {
        let cases: [(Vec3, [u32; 3], bool); 4] = [
            ([10.0, 0.0, 0.0], [0x3e020c4a, 0, 0], true),
            ([23000.0, 0.0, 0.0], [0x43920ccd, 0, 0], true),
            ([30000.0, 0.0, 0.0], [0x43be8000, 0, 0], false),
            ([3.0, 4.0, 12.0], [0x3d1c0ebf, 0x3d5013a9, 0x3e1c0ebf], true),
        ];
        for (input, expected_bits, expected_ok) in cases {
            let v = vec3::scale(input, DISTANCE_FACTOR);
            assert_eq!(bits(v), expected_bits);
            assert_eq!(source_velocity_ok(v), expected_ok);
        }
    }

    #[test]
    fn velocity_gates_use_strict_less_than() {
        assert!(listener_velocity_ok([49.9, 0.0, 0.0]));
        assert!(!listener_velocity_ok([50.0, 0.0, 0.0]));
        assert!(source_velocity_ok([299.9, 0.0, 0.0]));
        assert!(!source_velocity_ok([300.0, 0.0, 0.0]));
    }

    #[test]
    fn delta_time_clamps_to_range() {
        assert_eq!(delta_time(10.0, 10.0), 0.0001);
        assert_eq!(delta_time(12.0, 10.0), 1.0);
        assert_eq!(delta_time(10.5, 10.0), 0.5);
    }
}
