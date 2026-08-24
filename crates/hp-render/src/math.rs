//! Projection and clip math ported from the first-party renderer behavior
//! (ctest `projection_fov`, `render_clip`).
//!
//! Authored horizontal FOV is preserved at 4:3 and narrower aspects; when
//! vertical-FOV maintenance is enabled on wider aspects the horizontal FOV
//! widens so the authored *vertical* view survives, clamped to a safe
//! ultrawide limit. Clip-edge classification treats IEEE signed zero as one
//! plane side (`-0.0 >= 0.0` is true), so a sign flip through ±0.0 alone is
//! never an edge crossing.

use std::f64::consts::PI;

pub const MAX_EFFECTIVE_FOV_DEGREES: f32 = 169.999;

/// Effective horizontal FOV in degrees for a viewport.
///
/// `maintain_vertical_fov == false` (or any non-wider-than-4:3 viewport)
/// returns the authored angle unchanged; wider aspects scale the tangent of
/// the half-angle by the 4:3-relative aspect ratio. Computed in f64 like the
/// reference path so repeated viewport resizes do not drift.
pub fn effective_fov_angle(
    authored_hfov_degrees: f32,
    viewport_x: i32,
    viewport_y: i32,
    maintain_vertical_fov: bool,
) -> f32 {
    if !maintain_vertical_fov
        || viewport_x <= 0
        || viewport_y <= 0
        || (3.0 * viewport_x as f64) <= (4.0 * viewport_y as f64)
    {
        return authored_hfov_degrees;
    }
    let aspect_scale = (3.0 * viewport_x as f64) / (4.0 * viewport_y as f64);
    let half_angle = (authored_hfov_degrees as f64 * PI / 360.0).tan() * aspect_scale;
    let effective = 2.0 * half_angle.atan() * 180.0 / PI;
    (effective as f32).min(MAX_EFFECTIVE_FOV_DEGREES)
}

/// Console/menu UI scale fitting the authored 640x480 layout into a
/// viewport, multiplied by the user's preference (clamped 0.75..=2.0;
/// NaN treated as 1.0; degenerate viewports yield 1.0).
pub fn console_ui_scale(width: f32, height: f32, user_scale: f32) -> f32 {
    if width <= 0.0 || height <= 0.0 {
        return 1.0;
    }
    let user_scale = if user_scale.is_nan() { 1.0 } else { user_scale };
    let user_scale = user_scale.clamp(0.75, 2.0);
    (width / 640.0).min(height / 480.0) * user_scale
}

/// True when a polygon edge crosses the clipping plane between two dot
/// products. Signed zero describes ONE side: `-0.0 >= 0.0` holds, so a
/// transition through ±0.0 is not a crossing; opposite strict signs are.
#[inline]
pub fn render_clip_edge_crosses(previous_dot: f32, current_dot: f32) -> bool {
    (previous_dot >= 0.0) != (current_dot >= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported ctest projection_fov boundary cases.
    #[test]
    fn authored_fov_preserved_at_4_3() {
        const AUTHORED: f32 = 90.0;
        assert_eq!(effective_fov_angle(AUTHORED, 800, 600, true), AUTHORED);
        assert_eq!(effective_fov_angle(AUTHORED, 1920, 1080, false), AUTHORED);
        assert_eq!(effective_fov_angle(AUTHORED, 1280, 1024, true), AUTHORED);
    }

    #[test]
    fn widescreen_widens_horizontally_retaining_vertical_view() {
        const AUTHORED: f32 = 90.0;
        let wide = effective_fov_angle(AUTHORED, 1920, 1080, true);
        assert!(
            (wide - 106.2602).abs() <= 0.001,
            "16:9 90-degree projection expected 106.2602, got {wide:.6}"
        );
        let vertical_scale = (wide as f64 * PI / 360.0).tan() / (4.0f64 / 3.0);
        assert!(
            (vertical_scale - 1.0).abs() <= 1e-4,
            "16:9 projection did not retain the authored vertical view: {vertical_scale}"
        );
    }

    #[test]
    fn ultrawide_projection_respects_safe_limit() {
        let fov = effective_fov_angle(160.0, 7680, 1080, true);
        assert!(
            fov < 170.0,
            "ultrawide projection exceeded the safe FOV limit: {fov}"
        );
        assert_eq!(fov, MAX_EFFECTIVE_FOV_DEGREES);
    }

    #[test]
    fn console_ui_scale_matches_authored_layout_fit() {
        assert_eq!(console_ui_scale(1920.0, 1080.0, 1.0), 2.25);
        assert_eq!(console_ui_scale(1280.0, 1024.0, 1.0), 2.0);
    }

    // Ported ctest render_clip boundary cases.
    #[test]
    fn clip_signed_zero_describes_one_plane_side() {
        assert!(!render_clip_edge_crosses(-0.0, 0.0));
        assert!(!render_clip_edge_crosses(0.0, -0.0));
    }

    #[test]
    fn clip_epsilon_sides_classify_crossings() {
        const EPSILON: f32 = 0.000_001;
        assert!(render_clip_edge_crosses(-EPSILON, EPSILON));
        assert!(render_clip_edge_crosses(EPSILON, -EPSILON));
        assert!(!render_clip_edge_crosses(-EPSILON, -EPSILON));
        assert!(!render_clip_edge_crosses(EPSILON, EPSILON));
    }
}
