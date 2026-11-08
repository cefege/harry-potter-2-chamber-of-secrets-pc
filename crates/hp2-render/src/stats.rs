//! Numeric `URender::DrawStats` policy before native canvas formatting.

use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderStatsState {
    pub average_time: f32,
    pub level_time_seconds: f32,
    pub frame_time: i32,
    pub external_time: i32,
    pub seconds_per_cycle: f32,
    pub total_time: f32,
    pub particle_render_index: usize,
    pub particle_raster_index: usize,
}

/// Canvas-independent values for the `FpsStats` line.
///
/// The native shell chooses fonts, position, and formatting; this preserves
/// the original single-precision cycle-to-millisecond conversion in Rust.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsStatsLine {
    pub frame_milliseconds: f32,
    pub render_milliseconds: f32,
}

/// Canvas-independent values for the mesh statistics panel.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshStatsNumbers {
    pub total_milliseconds: f32,
    pub get_frame_milliseconds: f32,
    pub process_milliseconds: f32,
    pub light_setup_milliseconds: f32,
    pub light_milliseconds: f32,
    pub sub_milliseconds: f32,
    pub clip_milliseconds: f32,
    pub tmap_milliseconds: f32,
    pub lod_adjust: f32,
}

/// Canvas-independent values for the particle statistics panel.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleStatsNumbers {
    pub total_milliseconds: f32,
    pub emit_milliseconds: f32,
    pub update_milliseconds: f32,
    pub render_milliseconds: f32,
    pub raster_milliseconds: f32,
    pub emit_cycles_per_particle: i32,
    pub update_cycles_per_particle: i32,
    pub render_cycles_per_particle: i32,
    pub raster_cycles_per_particle: i32,
    pub rendered_area_kpix: i32,
    pub total_area_kpix: i32,
}

/// Canvas-independent values for the EARI summary and detail panels.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EariStatsNumbers {
    pub total_milliseconds: f32,
    pub draw_milliseconds: f32,
    pub setup_milliseconds: f32,
}

/// Native timing counters required by the DrawStats global panel.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalStatsInput {
    pub frame_cycles: i32,
    pub game_cycles: i32,
    pub client_cycles: i32,
    pub client_draw_cycles: i32,
    pub memory_cycles: i32,
    pub stat_memory_cycles: i32,
    pub engine_tick_cycles: i32,
    pub external_cycles: i32,
    pub render_cycles: i32,
    pub mesh_cycles: i32,
    pub particle_cycles: i32,
    pub polyv_cycles: i32,
    pub illumination_cycles: i32,
    pub occlusion_cycles: i32,
    pub extra_cycles: i32,
    pub filter_cycles: i32,
    pub dynamic_filter_cycles: i32,
    pub span_pixels: i32,
    pub decal_cycles: i32,
    pub decal_update_cycles: i32,
    pub decal_clip_cycles: i32,
}

/// Canvas-independent values for the DrawStats global panel.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalStatsNumbers {
    pub frame_milliseconds: f32,
    pub game_milliseconds: f32,
    pub client_milliseconds: f32,
    pub blit_milliseconds: f32,
    pub memory_milliseconds: f32,
    pub stat_memory_milliseconds: f32,
    pub platform_milliseconds: f32,
    pub external_milliseconds: f32,
    pub render_milliseconds: f32,
    pub mesh_milliseconds: f32,
    pub particle_milliseconds: f32,
    pub polyv_milliseconds: f32,
    pub illumination_milliseconds: f32,
    pub occlusion_milliseconds: f32,
    pub extra_milliseconds: f32,
    pub filter_milliseconds: f32,
    pub dynamic_filter_milliseconds: f32,
    pub span_kpixels: f32,
    pub decal_total_milliseconds: f32,
    pub decal_update_milliseconds: f32,
    pub decal_draw_milliseconds: f32,
    pub decal_clip_milliseconds: f32,
}

/// Raw numeric network counters for the DrawStats network panel.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetworkStatsInput {
    pub best_lag: f32,
    pub in_order: f32,
    pub out_order: f32,
    pub in_loss: f32,
    pub out_loss: f32,
    pub in_packets: f32,
    pub out_packets: f32,
    pub in_bunches: f32,
    pub out_bunches: f32,
    pub in_rate: f32,
    pub out_rate: f32,
    pub current_net_speed: i32,
}

/// Canvas-independent rounded values and saturation markers for network stats.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetworkStatsNumbers {
    pub ping_milliseconds: i32,
    pub channel_count: i32,
    pub in_order: i32,
    pub out_order: i32,
    pub in_loss: i32,
    pub out_loss: i32,
    pub in_packets: i32,
    pub out_packets: i32,
    pub in_bunches: i32,
    pub out_bunches: i32,
    pub in_rate: i32,
    pub out_rate: i32,
    pub net_speed: i32,
    pub in_rate_saturated: bool,
    pub out_rate_saturated: bool,
}
/// Raw timing counters and raster aggregate inputs for the DrawStats occlusion panel.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionStatsInput {
    pub occlusion_cycles: i32,
    pub clip_cycles: i32,
    pub raster_cycles: i32,
    pub span_cycles: i32,
    pub box_cycles: i32,
    pub raster_polys: i32,
    pub raster_box_rejects: i32,
}

/// Canvas-independent timing and raster aggregate values for the occlusion panel.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionStatsNumbers {
    pub occlusion_milliseconds: f32,
    pub clip_milliseconds: f32,
    pub raster_milliseconds: f32,
    pub span_milliseconds: f32,
    pub box_milliseconds: f32,
    pub raster_attempts: i32,
}

pub fn fps_line(frame_cycles: i32, render_cycles: i32, seconds_per_cycle: f32) -> FpsStatsLine {
    FpsStatsLine {
        frame_milliseconds: frame_cycles as f32 * seconds_per_cycle * 1000.0,
        render_milliseconds: render_cycles as f32 * seconds_per_cycle * 1000.0,
    }
}

pub fn mesh_numbers(
    seconds_per_cycle: f32,
    total_cycles: i32,
    get_frame_cycles: i32,
    process_cycles: i32,
    light_setup_cycles: i32,
    light_cycles: i32,
    sub_cycles: i32,
    clip_cycles: i32,
    tmap_cycles: i32,
    shape_lod_adjust: f32,
    shape_lod: f32,
) -> MeshStatsNumbers {
    let milliseconds = |cycles| seconds_per_cycle * 1000.0 * cycles as f32;
    MeshStatsNumbers {
        total_milliseconds: milliseconds(total_cycles),
        get_frame_milliseconds: milliseconds(get_frame_cycles),
        process_milliseconds: milliseconds(process_cycles),
        light_setup_milliseconds: milliseconds(light_setup_cycles),
        light_milliseconds: milliseconds(light_cycles),
        sub_milliseconds: milliseconds(sub_cycles),
        clip_milliseconds: milliseconds(clip_cycles),
        tmap_milliseconds: milliseconds(tmap_cycles),
        lod_adjust: 1.0 / (shape_lod_adjust * shape_lod),
    }
}

pub fn particle_numbers(
    seconds_per_cycle: f32,
    total_cycles: i32,
    emit_cycles: i32,
    emitted: i32,
    update_cycles: i32,
    updated: i32,
    render_cycles: i32,
    seen: i32,
    raster_cycles: i32,
    rendered_area: i32,
    total_area: i32,
) -> ParticleStatsNumbers {
    let milliseconds = |cycles| seconds_per_cycle * 1000.0 * cycles as f32;
    let per_particle = |cycles, count| if count != 0 { cycles / count } else { 0 };
    ParticleStatsNumbers {
        total_milliseconds: milliseconds(total_cycles),
        emit_milliseconds: milliseconds(emit_cycles),
        update_milliseconds: milliseconds(update_cycles),
        render_milliseconds: milliseconds(render_cycles),
        raster_milliseconds: milliseconds(raster_cycles),
        emit_cycles_per_particle: per_particle(emit_cycles, emitted),
        update_cycles_per_particle: per_particle(update_cycles, updated),
        render_cycles_per_particle: per_particle(render_cycles, seen),
        raster_cycles_per_particle: per_particle(raster_cycles, seen),
        rendered_area_kpix: rendered_area / 1000,
        total_area_kpix: total_area / 1000,
    }
}

pub fn eari_numbers(
    seconds_per_cycle: f32,
    total_cycles: i32,
    draw_cycles: i32,
    total_available: bool,
    draw_available: bool,
) -> EariStatsNumbers {
    let milliseconds = |cycles| seconds_per_cycle * 1000.0 * cycles as f32;
    let total_milliseconds = if total_available {
        milliseconds(total_cycles)
    } else {
        99.99
    };
    let draw_milliseconds = if draw_available {
        milliseconds(draw_cycles)
    } else {
        99.99
    };
    EariStatsNumbers {
        total_milliseconds,
        draw_milliseconds,
        setup_milliseconds: total_milliseconds - draw_milliseconds,
    }
}

pub fn global_numbers(seconds_per_cycle: f32, input: &GlobalStatsInput) -> GlobalStatsNumbers {
    let milliseconds = |cycles: i32| seconds_per_cycle * 1000.0 * cycles as f32;
    let render_milliseconds = |cycles: i32| cycles as f32 * seconds_per_cycle * 1000.0;
    GlobalStatsNumbers {
        frame_milliseconds: milliseconds(input.frame_cycles),
        game_milliseconds: milliseconds(input.game_cycles),
        client_milliseconds: milliseconds(input.client_cycles - input.client_draw_cycles),
        blit_milliseconds: milliseconds(input.client_draw_cycles),
        memory_milliseconds: milliseconds(input.memory_cycles),
        stat_memory_milliseconds: milliseconds(input.stat_memory_cycles),
        platform_milliseconds: milliseconds((input.frame_cycles - input.engine_tick_cycles).abs()),
        external_milliseconds: milliseconds(input.external_cycles),
        render_milliseconds: render_milliseconds(input.render_cycles),
        mesh_milliseconds: render_milliseconds(input.mesh_cycles),
        particle_milliseconds: render_milliseconds(input.particle_cycles),
        polyv_milliseconds: render_milliseconds(input.polyv_cycles),
        illumination_milliseconds: render_milliseconds(input.illumination_cycles),
        occlusion_milliseconds: render_milliseconds(input.occlusion_cycles),
        extra_milliseconds: render_milliseconds(input.extra_cycles),
        filter_milliseconds: milliseconds(input.filter_cycles),
        dynamic_filter_milliseconds: milliseconds(input.dynamic_filter_cycles),
        span_kpixels: input.span_pixels as f32 * 0.001,
        decal_total_milliseconds: milliseconds(input.decal_cycles + input.decal_update_cycles),
        decal_update_milliseconds: milliseconds(input.decal_update_cycles),
        decal_draw_milliseconds: milliseconds(input.decal_cycles - input.decal_clip_cycles),
        decal_clip_milliseconds: milliseconds(input.decal_clip_cycles),
    }
}

pub fn network_numbers(
    channels: &[*const c_void],
    input: &NetworkStatsInput,
) -> NetworkStatsNumbers {
    let round = |value: f32| value as i32;
    NetworkStatsNumbers {
        ping_milliseconds: round(1000.0 * input.best_lag),
        channel_count: channels.iter().filter(|channel| !channel.is_null()).count() as i32,
        in_order: round(input.in_order),
        out_order: round(input.out_order),
        in_loss: round(input.in_loss),
        out_loss: round(input.out_loss),
        in_packets: round(input.in_packets),
        out_packets: round(input.out_packets),
        in_bunches: round(input.in_bunches),
        out_bunches: round(input.out_bunches),
        in_rate: round(input.in_rate),
        out_rate: round(input.out_rate),
        net_speed: input.current_net_speed,
        in_rate_saturated: input.in_rate * 0.95 > input.current_net_speed as f32,
        out_rate_saturated: input.out_rate * 0.95 > input.current_net_speed as f32,
    }
}

pub fn occlusion_numbers(
    seconds_per_cycle: f32,
    input: &OcclusionStatsInput,
) -> OcclusionStatsNumbers {
    let milliseconds = |cycles: i32| seconds_per_cycle * 1000.0 * cycles as f32;
    OcclusionStatsNumbers {
        occlusion_milliseconds: milliseconds(input.occlusion_cycles),
        clip_milliseconds: milliseconds(input.clip_cycles),
        raster_milliseconds: milliseconds(input.raster_cycles),
        span_milliseconds: milliseconds(input.span_cycles),
        box_milliseconds: milliseconds(input.box_cycles),
        raster_attempts: input.raster_polys + input.raster_box_rejects,
    }
}

pub fn update<E>(
    state: &mut RenderStatsState,
    stats: &mut [i32],
    previous: &mut [i32],
    mut exp: impl FnMut(f64) -> Result<f64, E>,
    mut frand: impl FnMut() -> Result<f32, E>,
) -> Result<(), E> {
    if state.particle_render_index < stats.len() && state.particle_raster_index < stats.len() {
        stats[state.particle_render_index] -= stats[state.particle_raster_index];
    }
    if state.average_time != 0.0 && state.level_time_seconds > 0.2 {
        let process_time =
            (state.frame_time - state.external_time) as f32 * state.seconds_per_cycle;
        let exponent = -process_time / state.average_time.abs();
        state.total_time = (state.total_time as f64 * exp(exponent as f64)?) as f32;
        let previous_weight = (state.total_time / (state.total_time + 1.0)) as f64;
        for (current, old) in stats.iter_mut().zip(previous.iter_mut()) {
            let mut average =
                *old as f64 * previous_weight + *current as f64 * (1.0 - previous_weight);
            if state.average_time < 0.0 {
                average = ((*old as f64).powi(2) * previous_weight
                    + (*current as f64).powi(2) * (1.0 - previous_weight))
                    / average;
            }
            let value = (average + frand()? as f64) as i32;
            *current = value;
            *old = value;
        }
    }
    state.total_time += 1.0;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> RenderStatsState {
        RenderStatsState {
            average_time: 2.0,
            level_time_seconds: 1.0,
            frame_time: 5,
            external_time: 1,
            seconds_per_cycle: 0.1,
            total_time: 0.0,
            particle_render_index: 0,
            particle_raster_index: 1,
        }
    }

    #[test]
    fn adjusts_particle_time_then_updates_weighted_integer_stats() {
        let mut state = state();
        let mut stats = [10, 3, 8];
        let mut previous = [2, 3, 4];
        update(
            &mut state,
            &mut stats,
            &mut previous,
            |x| Ok::<_, ()>(x.exp()),
            || Ok(0.0),
        )
        .unwrap();
        assert_eq!(stats[0], 7);
        assert_eq!(previous[0], 7);
        assert_eq!(state.total_time, 1.0);
    }

    #[test]
    fn disables_averaging_until_enabled_time_threshold() {
        let mut state = state();
        state.average_time = 0.0;
        let mut stats = [10, 3];
        let mut previous = [2, 3];
        update(
            &mut state,
            &mut stats,
            &mut previous,
            |_| -> Result<f64, ()> { unreachable!() },
            || -> Result<f32, ()> { unreachable!() },
        )
        .unwrap();
        assert_eq!(stats, [7, 3]);
        assert_eq!(previous, [2, 3]);
        assert_eq!(state.total_time, 1.0);
    }

    #[test]
    fn uses_double_exponential_result_then_legacy_float_weight() {
        let mut state = state();
        state.total_time = 2.0;
        let mut stats = [10, 3];
        let mut previous = [4, 3];
        update(
            &mut state,
            &mut stats,
            &mut previous,
            |_| Ok::<_, ()>(0.9_f64),
            || Ok(0.0),
        )
        .unwrap();
        assert_eq!(state.total_time, 2.8);
        assert_eq!(stats[0], 5);
    }

    #[test]
    fn negative_average_uses_rms_before_integer_publication() {
        let mut state = state();
        state.average_time = -2.0;
        state.total_time = 3.0;
        let mut stats = [12, 3];
        let mut previous = [4, 3];
        update(
            &mut state,
            &mut stats,
            &mut previous,
            |_| Ok::<_, ()>(1.0_f64),
            || Ok(0.0),
        )
        .unwrap();
        assert_eq!(stats[0], 6);
        assert_eq!(previous[0], 6);
    }

    #[test]
    fn fps_line_preserves_single_precision_cycle_conversion() {
        assert_eq!(
            fps_line(17, -3, 0.25),
            FpsStatsLine {
                frame_milliseconds: 4250.0,
                render_milliseconds: -750.0,
            }
        );
    }

    #[test]
    fn mesh_numbers_preserve_cycle_scale_and_lod_division() {
        assert_eq!(
            mesh_numbers(0.25, 17, 3, -2, 4, 5, 6, 7, 8, 0.5, 2.0),
            MeshStatsNumbers {
                total_milliseconds: 4250.0,
                get_frame_milliseconds: 750.0,
                process_milliseconds: -500.0,
                light_setup_milliseconds: 1000.0,
                light_milliseconds: 1250.0,
                sub_milliseconds: 1500.0,
                clip_milliseconds: 1750.0,
                tmap_milliseconds: 2000.0,
                lod_adjust: 1.0,
            }
        );
    }

    #[test]
    fn particle_numbers_preserve_zero_guard_and_integer_truncation() {
        assert_eq!(
            particle_numbers(0.25, 17, 9, 2, 11, 0, 13, 3, -7, 1999, -1999),
            ParticleStatsNumbers {
                total_milliseconds: 4250.0,
                emit_milliseconds: 2250.0,
                update_milliseconds: 2750.0,
                render_milliseconds: 3250.0,
                raster_milliseconds: -1750.0,
                emit_cycles_per_particle: 4,
                update_cycles_per_particle: 0,
                render_cycles_per_particle: 4,
                raster_cycles_per_particle: -2,
                rendered_area_kpix: 1,
                total_area_kpix: -1,
            }
        );
    }

    #[test]
    fn eari_numbers_preserve_cycle_scale_and_missing_entry_sentinel() {
        assert_eq!(
            eari_numbers(0.25, 17, 9, true, false),
            EariStatsNumbers {
                total_milliseconds: 4250.0,
                draw_milliseconds: 99.99,
                setup_milliseconds: 4150.01,
            }
        );
    }

    #[test]
    fn global_numbers_preserve_legacy_operation_order() {
        let input = GlobalStatsInput {
            frame_cycles: 17,
            game_cycles: 3,
            client_cycles: 11,
            client_draw_cycles: 2,
            memory_cycles: 4,
            stat_memory_cycles: 5,
            engine_tick_cycles: 7,
            external_cycles: 8,
            render_cycles: 9,
            mesh_cycles: 10,
            particle_cycles: 11,
            polyv_cycles: 12,
            illumination_cycles: 13,
            occlusion_cycles: 14,
            extra_cycles: 15,
            filter_cycles: 16,
            dynamic_filter_cycles: 17,
            span_pixels: 1999,
            decal_cycles: 19,
            decal_update_cycles: 20,
            decal_clip_cycles: 21,
        };
        let numbers = global_numbers(0.25, &input);
        assert_eq!(numbers.frame_milliseconds, 4250.0);
        assert_eq!(numbers.client_milliseconds, 2250.0);
        assert_eq!(numbers.platform_milliseconds, 2500.0);
        assert_eq!(numbers.render_milliseconds, 2250.0);
        assert_eq!(numbers.span_kpixels, 1.9990001);
        assert_eq!(numbers.decal_total_milliseconds, 9750.0);
        assert_eq!(numbers.decal_draw_milliseconds, -500.0);
    }

    #[test]
    fn network_numbers_count_channels_truncate_and_mark_saturation() {
        let channels = [
            std::ptr::null(),
            1usize as *const c_void,
            std::ptr::null(),
            2usize as *const c_void,
        ];
        let input = NetworkStatsInput {
            best_lag: 1.25,
            in_order: -3.9,
            out_order: 4.9,
            in_loss: 5.9,
            out_loss: -6.9,
            in_packets: 7.9,
            out_packets: 8.9,
            in_bunches: 9.9,
            out_bunches: 10.9,
            in_rate: 105.9,
            out_rate: 104.9,
            current_net_speed: 100,
        };
        assert_eq!(
            network_numbers(&channels, &input),
            NetworkStatsNumbers {
                ping_milliseconds: 1250,
                channel_count: 2,
                in_order: -3,
                out_order: 4,
                in_loss: 5,
                out_loss: -6,
                in_packets: 7,
                out_packets: 8,
                in_bunches: 9,
                out_bunches: 10,
                in_rate: 105,
                out_rate: 104,
                net_speed: 100,
                in_rate_saturated: true,
                out_rate_saturated: false,
            }
        );
    }

    #[test]
    fn occlusion_numbers_preserve_cycle_scale_and_raster_aggregate() {
        let input = OcclusionStatsInput {
            occlusion_cycles: 17,
            clip_cycles: -3,
            raster_cycles: 4,
            span_cycles: 5,
            box_cycles: 6,
            raster_polys: 7,
            raster_box_rejects: 8,
        };
        assert_eq!(
            occlusion_numbers(0.25, &input),
            OcclusionStatsNumbers {
                occlusion_milliseconds: 4250.0,
                clip_milliseconds: -750.0,
                raster_milliseconds: 1000.0,
                span_milliseconds: 1250.0,
                box_milliseconds: 1500.0,
                raster_attempts: 15,
            }
        );
    }
}
