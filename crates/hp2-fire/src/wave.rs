//! Port of `UWaveTexture::SetWaveLight` (`UnFractal.cpp`): builds the 1024-entry
//! Phong/diffuse bump-map light table in `RenderTable` from `WaveAmp`,
//! `BumpMapLight`, `BumpMapAngle`, `PhongRange` and `PhongSize`.
//! C++ keeps `PostLoad`, `Init`, `Clear`, `ConstantTimeTick` and the palette
//! setup; `SetWaveLight` is a forwarder to `set_wave_light`.

use crate::abi::FireTraceHooks;
use crate::globals;

/// `PI` from `Core/Inc/UnMath.h`.
const PI: f32 = std::f32::consts::PI;

/// Scalar inputs read by `SetWaveLight` from the texture object.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireWaveLight {
    pub wave_amp: u8,
    pub bump_map_light: u8,
    pub bump_map_angle: u8,
    pub phong_range: u8,
    pub phong_size: u8,
}

/// `Abs` template from `Core/Inc/UnTemplate.h`: `(A>=(T)0) ? A : -A`.
fn abs_f32(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `FakeAtan` (`UnFractal.cpp`): approximate arctangent.
#[allow(clippy::approx_constant)] // C literal 3.1415F, value kept exact
fn fake_atan(x: f32) -> f32 {
    3.1415f32 * 0.5f32 * x / (abs_f32(x) + 1.0f32)
}

/// Fills `render_table[0..1024]` with the wave bump-map light table.
/// `render_table` must hold at least 1024 entries (`RenderTable` is 1028).
#[allow(clippy::needless_range_loop)] // index loop mirrors the C `for (I...)`
pub fn set_wave_light(trace: FireTraceHooks, p: &FireWaveLight, render_table: &mut [u8]) {
    let mut g = globals::lock();
    g.trace = trace;
    assert!(
        render_table.len() >= 1024,
        "set_wave_light: need 1024 entries"
    );

    // Compute shades for surface normals.
    let lamp: f32 = PI * (p.bump_map_light as f32) / 255.0f32;
    let viewer: f32 = PI * (p.bump_map_angle as f32) / 255.0f32;
    let wave_amp: f32 = (p.wave_amp as f32) / 255.0f32;
    let phong_range = p.phong_range as i32;

    for i in 0..1024usize {
        // Get reflection magnitude.
        let normal: f32 = fake_atan(wave_amp * (512.0f32 - i as f32) / 196.0f32) + (PI * 0.5f32);

        // Max in this palette is 255, diffuse light reaches to 256-(PhongRange/2).
        // C: appCos is DOUBLE cos(DOUBLE), so the product is formed in double.
        let mut temp_light: i32 =
            ((256 - (phong_range / 2)) as f64 * ((normal - lamp) as f64).cos()) as i32;

        // Create a phong-ish highlight. Based on angle between viewer direction
        // and reflected light: reflected light angle = Normal*2 - Lamp.
        let phong_radius: f32 = (p.phong_size as f32) / 512.0f32;
        let reflected: f32 = normal * 2.0f32 - lamp;
        let delta: f32 = reflected - viewer;
        if delta * delta < phong_radius * phong_radius {
            // C: (PhongRange * 2) is int, promoted to FLOAT before the multiply.
            temp_light += (((phong_range * 2) as f32)
                * (phong_radius - abs_f32(reflected - viewer))
                / phong_radius) as i32;
        }

        render_table[i] = temp_light.clamp(0, 255) as u8;
    }
}
