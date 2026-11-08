//! Rust policy for `URender::PostRender` adaptive detail settings.

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderDetailSettings {
    pub min_desired_frame_rate: f32,
    pub frame_time: f32,
    pub global_shape_lod_adjust: f32,
    pub particle_raster_time: i32,
    pub seconds_per_cycle: f32,
    pub max_particle_render_time: f32,
    pub particle_area: i32,
    pub particle_area_rendered: i32,
    pub max_particle_size: f32,
}

#[inline]
fn clamp_native(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value < max {
        value
    } else {
        max
    }
}

#[inline]
fn max_native(left: f32, right: f32) -> f32 {
    if left >= right { left } else { right }
}

/// Updates adaptive mesh and particle detail exactly once for one completed frame.
pub fn adjust_detail(settings: &mut RenderDetailSettings) {
    let min_frame_rate = clamp_native(settings.min_desired_frame_rate, 10.0, 120.0);
    let adjust = min_frame_rate * settings.frame_time;
    if adjust > 1.0 {
        settings.global_shape_lod_adjust =
            clamp_native(settings.global_shape_lod_adjust + 0.1, 1.0, adjust);
    } else {
        settings.global_shape_lod_adjust = max_native(settings.global_shape_lod_adjust - 0.1, 1.0);
    }

    let particle_time =
        settings.particle_raster_time as f32 * settings.seconds_per_cycle * min_frame_rate;
    if particle_time > settings.max_particle_render_time {
        if settings.particle_area_rendered >= settings.particle_area {
            settings.max_particle_size *= 0.5;
        } else if settings.particle_area_rendered > settings.particle_area / 4 {
            settings.max_particle_size *= settings.max_particle_render_time / particle_time;
        } else {
            settings.max_particle_size *=
                settings.particle_area as f32 / 4.0 / settings.particle_area_rendered as f32;
        }
    } else {
        settings.max_particle_size *= 1.25;
    }
    settings.max_particle_size = clamp_native(settings.max_particle_size, 0.01, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> RenderDetailSettings {
        RenderDetailSettings {
            min_desired_frame_rate: 60.0,
            frame_time: 0.01,
            global_shape_lod_adjust: 1.5,
            particle_raster_time: 1,
            seconds_per_cycle: 0.0001,
            max_particle_render_time: 0.05,
            particle_area: 100,
            particle_area_rendered: 100,
            max_particle_size: 0.5,
        }
    }

    #[test]
    fn increases_detail_within_legacy_caps_when_the_frame_is_fast() {
        let mut state = settings();
        adjust_detail(&mut state);
        assert_eq!(state.global_shape_lod_adjust, 1.4);
        assert_eq!(state.max_particle_size, 0.625);
    }

    #[test]
    fn reduces_particle_size_by_the_original_area_branches() {
        let mut state = settings();
        state.frame_time = 0.02;
        state.particle_raster_time = 10;
        state.seconds_per_cycle = 0.001;
        state.particle_area_rendered = 100;
        adjust_detail(&mut state);
        assert!((state.global_shape_lod_adjust - 1.2).abs() < 0.0001);
        assert_eq!(state.max_particle_size, 0.25);

        let mut state = settings();
        state.particle_raster_time = 10;
        state.seconds_per_cycle = 0.001;
        state.particle_area_rendered = 30;
        adjust_detail(&mut state);
        assert!((state.max_particle_size - 0.5 * (0.05 / 0.6)).abs() < 0.000001);

        let mut state = settings();
        state.particle_raster_time = 10;
        state.seconds_per_cycle = 0.001;
        state.particle_area_rendered = 20;
        adjust_detail(&mut state);
        assert!((state.max_particle_size - 0.5 * (25.0 / 20.0)).abs() < 0.000001);
    }

    #[test]
    fn clamps_frame_rate_and_particle_size_with_native_nan_rules() {
        let mut state = settings();
        state.min_desired_frame_rate = f32::NAN;
        state.max_particle_size = f32::NAN;
        adjust_detail(&mut state);
        assert_eq!(state.max_particle_size, 1.0);

        let mut state = settings();
        state.min_desired_frame_rate = 1.0;
        state.max_particle_size = 0.001;
        adjust_detail(&mut state);
        assert_eq!(state.max_particle_size, 0.01);
    }
}
