//! Rust timing/state policy for `URender::PostRender`.

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderPostTiming {
    pub frame_time: f32,
    pub render_time: f32,
    pub seconds_per_cycle: f32,
    pub process_time: f32,
    pub last_process_time: f32,
    pub stat_frame_time: i32,
    pub stat_render_time: i32,
    pub stat_external_time: i32,
}

#[inline]
fn min_native(left: f32, right: f32) -> f32 {
    if left <= right { left } else { right }
}

/// Applies the legacy PostRender timing update after C++ reads its clock leaves.
pub fn update(timing: &mut RenderPostTiming) {
    timing.frame_time = min_native(timing.frame_time, 0.2);
    timing.stat_frame_time = (timing.frame_time / timing.seconds_per_cycle) as i32;
    timing.stat_render_time = (timing.render_time / timing.seconds_per_cycle) as i32;
    if timing.last_process_time == 0.0 {
        timing.last_process_time = timing.process_time;
        timing.process_time = 0.0;
    } else {
        timing.process_time -= timing.last_process_time;
        timing.last_process_time += timing.process_time;
        timing.process_time = min_native(timing.process_time, 0.2);
        timing.stat_external_time = (timing.stat_external_time as f32
            + (timing.stat_frame_time as f32 - timing.process_time / timing.seconds_per_cycle))
            as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing() -> RenderPostTiming {
        RenderPostTiming {
            frame_time: 0.3,
            render_time: 0.04,
            seconds_per_cycle: 0.01,
            process_time: 1.5,
            last_process_time: 0.0,
            stat_frame_time: 0,
            stat_render_time: 0,
            stat_external_time: 7,
        }
    }

    #[test]
    fn first_sample_initializes_process_baseline_without_external_time() {
        let mut state = timing();
        update(&mut state);
        assert_eq!(state.frame_time, 0.2);
        assert_eq!(state.stat_frame_time, 20);
        assert_eq!(state.stat_render_time, 4);
        assert_eq!(state.last_process_time, 1.5);
        assert_eq!(state.process_time, 0.0);
        assert_eq!(state.stat_external_time, 7);
    }

    #[test]
    fn later_sample_updates_baseline_before_capping_process_delta() {
        let mut state = timing();
        state.frame_time = 0.03;
        state.process_time = 2.0;
        state.last_process_time = 1.5;
        update(&mut state);
        assert_eq!(state.last_process_time, 2.0);
        assert_eq!(state.process_time, 0.2);
        assert_eq!(state.stat_external_time, -10);
    }

    #[test]
    fn external_time_preserves_legacy_delta_grouping_at_float_precision_boundary() {
        let mut state = RenderPostTiming {
            frame_time: 0.1,
            render_time: 0.0,
            seconds_per_cycle: 0.1,
            process_time: 1.1,
            last_process_time: 1.0,
            stat_frame_time: 0,
            stat_render_time: 0,
            stat_external_time: 16_777_216,
        };
        update(&mut state);
        assert_eq!(state.stat_external_time, 16_777_216);
    }
}
