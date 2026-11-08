//! Canonical `URender::DrawStats` with shared numeric and panel-order policy.

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, FSceneNode, FStats, UCanvas, UClient, UEngine, ULevel, UViewport,
    UNetConnection, UNetDriver, URender, URenderDevice, UFont,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::ptr;

const STATS: bool = hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0;

/// Runs the source stats update and presents the enabled panels on the live Canvas.
///
/// # Safety
/// `render` and `frame` and all records linked from them must remain live for this call.
pub(super) unsafe fn draw_stats(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical DrawStats requires live Render and SceneNode records"));
    }
    let viewport = unsafe { ptr::addr_of!((*frame).viewport).read_unaligned().cast::<UViewport>() };
    if viewport.is_null() {
        return Err(RuntimeError::new("Canonical DrawStats SceneNode has no Viewport"));
    }
    let canvas = unsafe { ptr::addr_of!((*viewport).canvas).read_unaligned().cast::<UCanvas>() };
    if canvas.is_null() {
        return Err(RuntimeError::new("Canonical DrawStats Viewport has no Canvas"));
    }
    let owner = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine owner context is unavailable"))?;
    let graph = &owner.render_graph;
    let mut stats = graph.stats.get();
    let mut previous = graph.previous_stats.get();
    let seconds_per_cycle = hp2_core::unix_clock::seconds_per_cycle();
    let level = unsafe { ptr::addr_of!((*frame).level).read_unaligned().cast::<ULevel>() };
    let average_time = unsafe { ptr::addr_of!((*render).average_time).read_unaligned() };
    let level_time = if average_time != 0.0 {
        if level.is_null() {
            return Err(RuntimeError::new("Canonical averaged DrawStats SceneNode has no Level"));
        }
        level_info(level)?.0
    } else {
        0.0
    };
    let stat_count = std::mem::offset_of!(FStats, eari_actor_names) / std::mem::size_of::<i32>();
    let particle_render_index = std::mem::offset_of!(FStats, particle_render_time) / std::mem::size_of::<i32>();
    let particle_raster_index = std::mem::offset_of!(FStats, particle_raster_time) / std::mem::size_of::<i32>();
    let mut state = hp2_render::stats::RenderStatsState {
        average_time,
        level_time_seconds: level_time,
        frame_time: stats.frame_time,
        external_time: stats.external_time,
        seconds_per_cycle,
        total_time: unsafe { ptr::addr_of!((*render).total_time).read_unaligned() },
        particle_render_index,
        particle_raster_index,
    };
    // Keep both records in local snapshots; no owner borrow survives into Canvas calls.
    let values = unsafe { std::slice::from_raw_parts_mut((&mut stats as *mut FStats).cast::<i32>(), stat_count) };
    let prior = unsafe { std::slice::from_raw_parts_mut((&mut previous as *mut FStats).cast::<i32>(), stat_count) };
    hp2_render::stats::update(
        &mut state,
        values,
        prior,
        |value| Ok::<_, ()>(value.exp()),
        || Ok::<_, ()>(access.rand_range_float(0.0, 1.0)),
    ).map_err(|_| RuntimeError::new("Canonical renderer stats update failed"))?;
    graph.stats.set(stats);
    graph.previous_stats.set(previous);
    unsafe { ptr::addr_of_mut!((*render).total_time).write_unaligned(state.total_time) };

    let flags = hp2_render::stats_presentation::RenderStatsPresentationState {
        fps: unsafe { ptr::addr_of!((*render).fps_stats).read_unaligned() != 0 },
        global: STATS && unsafe { ptr::addr_of!((*render).global_stats).read_unaligned() != 0 },
        net: STATS && unsafe { ptr::addr_of!((*render).net_stats).read_unaligned() != 0 },
        hardware: STATS && unsafe { ptr::addr_of!((*render).hardware_stats).read_unaligned() != 0 },
        mesh: STATS && unsafe { ptr::addr_of!((*render).mesh_stats).read_unaligned() != 0 },
        particle: STATS && unsafe { ptr::addr_of!((*render).particle_stats).read_unaligned() != 0 },
        zone: STATS && unsafe { ptr::addr_of!((*render).zone_stats).read_unaligned() != 0 },
        occlusion: STATS && unsafe { ptr::addr_of!((*render).occlusion_stats).read_unaligned() != 0 },
        game: STATS && unsafe { ptr::addr_of!((*render).game_stats).read_unaligned() != 0 },
        cache: STATS && unsafe { ptr::addr_of!((*render).cache_stats).read_unaligned() != 0 },
        eari: STATS && unsafe { ptr::addr_of!((*render).eari_stats).read_unaligned() != 0 },
        eari_details: STATS && unsafe { ptr::addr_of!((*render).eari_details).read_unaligned() != 0 },
        anim: STATS && unsafe { ptr::addr_of!((*render).anim_stats).read_unaligned() != 0 },
    };
    let nodes = unsafe { ptr::addr_of!((*render).nodes_draw).read_unaligned() as i32 };
    let polys = unsafe { ptr::addr_of!((*render).polys_draw).read_unaligned() as i32 };
    let old_net_stats = &graph.old_net_stats;
    let network_saved_time = &graph.network_saved_time;
    hp2_render::stats_presentation::present(&flags, |leaf| {
        if !STATS && (leaf as u32) >= hp2_render::stats_presentation::RenderStatsPresentationLeaf::SetStatsCursor as u32 {
            return Ok::<_, RuntimeError>(());
        }
        unsafe {
            present_leaf(access, render, frame, canvas, viewport, level, leaf, &stats,
                seconds_per_cycle, nodes, polys, old_net_stats, network_saved_time)
        }
    })?;

    if STATS {
        // SAFETY: FStats is a C record of scalar values and byte arrays; zero is valid.
        graph.stats.set(unsafe { std::mem::zeroed() });
    }
    Ok(())
}

unsafe fn present_leaf(
    access: CoreAccess<'_>, render: *mut URender, frame: *mut FSceneNode,
    canvas: *mut UCanvas, viewport: *mut UViewport, level: *mut ULevel,
    leaf: hp2_render::stats_presentation::RenderStatsPresentationLeaf,
    stats: &FStats, seconds_per_cycle: f32, nodes: i32, polys: i32,
    old_net_stats: &std::cell::Cell<bool>, network_saved_time: &std::cell::Cell<[u8; 8]>,
) -> RuntimeResult<()> {
    use hp2_render::stats_presentation::RenderStatsPresentationLeaf as L;
    match leaf {
        L::SetColor => unsafe { ptr::addr_of_mut!((*canvas).color).write_unaligned([255, 255, 255, 255]) },
        L::SetStatsCursor => {
            unsafe {
                ptr::addr_of_mut!((*canvas).cur_x).write_unaligned(0.0);
                ptr::addr_of_mut!((*canvas).cur_y).write_unaligned(16.0);
            }
        }
        L::Fps => {
            let x = unsafe { ptr::addr_of!((*frame).x).read_unaligned() };
            let y = unsafe { ptr::addr_of!((*frame).y).read_unaligned() };
            unsafe {
                ptr::addr_of_mut!((*canvas).cur_x).write_unaligned((x / 2) as f32);
                ptr::addr_of_mut!((*canvas).cur_y).write_unaligned((y - 10) as f32);
            }
            let line = hp2_render::stats::fps_line(stats.frame_time, stats.render_time, seconds_per_cycle);
            show(access, canvas, &format!("Frame={:05.1} MSEC Render={:05.1} MSEC Nodes={nodes:03} Polys={polys:03} LODAdj={:.2} MaxParticle={:.3}",
                line.frame_milliseconds, line.render_milliseconds,
                unsafe { ptr::addr_of!((*render).global_shape_lod_adjust).read_unaligned() },
                unsafe { ptr::addr_of!((*render).max_particle_size).read_unaligned() }), 1, true)?;
        }
        L::Global => global_panel(access, canvas, viewport, stats, seconds_per_cycle)?,
        L::Net => network_panel(access, canvas, level, old_net_stats, network_saved_time)?,
        L::FinishNet => old_net_stats.set(unsafe { ptr::addr_of!((*render).net_stats).read_unaligned() != 0 }),
        L::Hardware => {
            show(access, canvas, "HARDWARE:", 0, false)?;
            let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
            if device.is_null() { return Err(RuntimeError::new("Canonical stats Viewport has no RenderDevice")); }
            let device_stats = crate::render_device_owner::get_stats(device)?;
            show(access, canvas, &format!("  {device_stats}"), 0, false)?;
            show(access, canvas, " ", 0, false)?;
            // WGPU inherits URenderDevice::DrawStats' source no-op implementation.
        }
        L::Mesh => {
            let n = hp2_render::stats::mesh_numbers(seconds_per_cycle, stats.mesh_time, stats.mesh_get_frame_time,
                stats.mesh_process_time, stats.mesh_light_setup_time, stats.mesh_light_time,
                stats.mesh_sub_time, stats.mesh_clip_time, stats.mesh_tmap_time,
                unsafe { ptr::addr_of!((*render).global_shape_lod_adjust).read_unaligned() },
                unsafe { ptr::addr_of!((*render).global_shape_lod).read_unaligned() });
            show(access, canvas, &format!("MESH: {:04.1}", n.total_milliseconds), 0, false)?;
            show(access, canvas, &format!("  GetFrame={:04.1} Process={:04.1} LightSet={:04.1} Light={:04.1}", n.get_frame_milliseconds, n.process_milliseconds, n.light_setup_milliseconds, n.light_milliseconds), 0, false)?;
            show(access, canvas, &format!("  Sub={:04.1} Clip={:04.1} Tmap={:04.1}", n.sub_milliseconds, n.clip_milliseconds, n.tmap_milliseconds), 0, false)?;
            show(access, canvas, &format!("  Mesh Count={} Polys={} Verts={}/{} Lights={} Vtrics={} VertLights={} LODAdjust={:.2}", stats.mesh_count, stats.mesh_poly_count, stats.mesh_sub_count, stats.mesh_vert_count, stats.mesh_light_count, stats.mesh_vtric_count, stats.mesh_vert_light_count, n.lod_adjust), 0, false)?;
            show(access, canvas, " ", 0, false)?;
        }
        L::Particle => {
            let n = hp2_render::stats::particle_numbers(seconds_per_cycle, stats.particle_time, stats.particle_emit_time,
                stats.particles_emitted, stats.particle_update_time, stats.particles_updated,
                stats.particle_render_time, stats.particles_seen, stats.particle_raster_time,
                stats.particle_area_rendered, stats.particle_area);
            show(access, canvas, &format!("PART: {:04.1}", n.total_milliseconds), 0, false)?;
            show(access, canvas, &format!("  Emit={:04.1}/{} ({} c/p) Update={:04.1}/{} ({} c/p) Render={:04.1}/{} ({} c/p) Raster={:04.1}/{} ({} c/p)", n.emit_milliseconds, stats.particles_emitted, n.emit_cycles_per_particle, n.update_milliseconds, stats.particles_updated, n.update_cycles_per_particle, n.render_milliseconds, stats.particles_seen, n.render_cycles_per_particle, n.raster_milliseconds, stats.particles_seen, n.raster_cycles_per_particle), 0, false)?;
            show(access, canvas, &format!("  Num={} / {} KPix={} / {}", stats.particles_rendered, stats.particles_seen, n.rendered_area_kpix, n.total_area_kpix), 0, false)?;
            show(access, canvas, " ", 0, false)?;
        }
        L::Zone => {
            show(access, canvas, &format!("Zones: Visible={}/{} Reject={}", stats.visible_zones, stats.num_zones, stats.mask_reject_zones), 0, false)?;
            show(access, canvas, " ", 0, false)?;
        }
        L::Occlusion => {
            let input = hp2_render::stats::OcclusionStatsInput { occlusion_cycles: stats.occlusion_time,
                clip_cycles: stats.clip_time, raster_cycles: stats.raster_time, span_cycles: stats.span_time,
                box_cycles: stats.box_time, raster_polys: stats.num_raster_polys,
                raster_box_rejects: stats.num_raster_box_reject };
            let n = hp2_render::stats::occlusion_numbers(seconds_per_cycle, &input);
            show(access, canvas, &format!("Occlusion={:04.1}:", n.occlusion_milliseconds), 0, false)?;
            show(access, canvas, &format!("   Clip={:04.1} Raster={:04.1} Span={:04.1} Visit={}/{} Points={}", n.clip_milliseconds, n.raster_milliseconds, n.span_milliseconds, stats.nodes_done, stats.nodes_total, stats.num_points), 0, false)?;
            show(access, canvas, &format!("   Transform={} Clip={} Raster={} RasterAccept={} DrawNodes={nodes}", stats.num_transform, stats.num_clip, n.raster_attempts, stats.num_raster_polys), 0, false)?;
            show(access, canvas, &format!("   BoxTime={:04.1} BoxChecks={} BoxBacks={} BoxIn={} BoxOutPyr={} BoxSpanOcc={}", n.box_milliseconds, stats.box_checks, stats.box_backs, stats.box_in, stats.box_out_of_pyramid, stats.box_span_occluded), 0, false)?;
            show(access, canvas, " ", 0, false)?;
        }
        L::Game => {
            let game_level = actor_level(viewport)?;
            show(access, canvas, "GAME:", 0, false)?;
            show(access, canvas, &format!("   {}", level_stats(game_level)?), 0, false)?;
            show(access, canvas, " ", 0, false)?;
        }
        L::Cache => { show(access, canvas, "CACHE:", 0, false)?; show(access, canvas, &cache_status(access)?, 0, false)?; show(access, canvas, " ", 0, false)?; }
        L::Eari => {
            let n = hp2_render::stats::eari_numbers(seconds_per_cycle, stats.total_eari_time, stats.eari_actor_draw_time, true, true);
            show(access, canvas, &format!("EARI: {:05.2}", n.total_milliseconds), 0, false)?;
            show(access, canvas, &format!("   EARIActors={} EARISubActors={} DrawTime={:05.2} SetupTime={:05.2}", stats.total_eari_actors, stats.total_eari_sub_actors, n.draw_milliseconds, n.setup_milliseconds), 0, false)?;
            show(access, canvas, " ", 0, false)?;
        }
        L::EariDetails => {
            show(access, canvas, "EARI Details: ", 0, false)?;
            show(access, canvas, "Name               Tag                Sub Total Draw  Setup", 0, false)?;
            show(access, canvas, "------------------ ------------------ --- ----- ----- -----", 0, false)?;
            for i in 0..stats.total_eari_actors.max(0) as usize {
                let name = if i < stats.eari_actor_names.len() {
                    name_string(access, stats.eari_actor_names[i])?
                } else {
                    "XXX".to_owned()
                };
                let tag = if i < stats.eari_actor_tags.len() {
                    name_string(access, stats.eari_actor_tags[i])?
                } else {
                    "XXX".to_owned()
                };
                let sub_actors = stats.eari_sub_actors.get(i).copied().unwrap_or(999);
                let time = stats.eari_time.get(i).copied().unwrap_or(0);
                let draw_time = stats.eari_draw_time.get(i).copied().unwrap_or(0);
                let n = hp2_render::stats::eari_numbers(seconds_per_cycle, time, draw_time,
                    i < stats.eari_time.len(), i < stats.eari_draw_time.len());
                show(access, canvas, &format!("{name:<18} {tag:<18} {sub_actors:3} {:05.2} {:05.2} {:05.2}",
                    n.total_milliseconds, n.draw_milliseconds, n.setup_milliseconds), 0, false)?;
            }
            show(access, canvas, " ", 0, false)?;
        }
        L::Anim => unsafe { ptr::addr_of_mut!((*render).stat_line).write_unaligned(ptr::addr_of!((*frame).y).read_unaligned() / 5) },
    }
    Ok(())
}

fn show(access: CoreAccess<'_>, canvas: *mut UCanvas, text: &str, style: i32, center: bool) -> RuntimeResult<()> {
    let text = if style == 0 { format!(" {text}\n") } else { text.to_owned() };
    let mut wide = text.encode_utf16().collect::<Vec<u16>>();
    wide.push(0);
    let font = unsafe { ptr::addr_of!((*canvas).small_font).read_unaligned().cast::<UFont>() };
    unsafe { crate::canvas_owner::wrapped_print(access, canvas, font, style, center, &wide, 0)?; }
    Ok(())
}

fn level_info(level: *mut ULevel) -> RuntimeResult<(f32, *mut ALevelInfo)> {
    let actors = unsafe { ptr::addr_of!((*level).base.actors).read_unaligned() };
    if actors.array_num <= 0 || actors.data.is_null() {
        return Err(RuntimeError::new("Canonical stats Level has no LevelInfo actor"));
    }
    let info = unsafe { actors.data.cast::<*mut ALevelInfo>().read_unaligned() };
    if info.is_null() { return Err(RuntimeError::new("Canonical stats LevelInfo actor is null")); }
    Ok((unsafe { ptr::addr_of!((*info).time_seconds).read_unaligned() }, info))
}

fn level_stats(level: *mut ULevel) -> RuntimeResult<String> {
    let (_, _) = level_info(level)?;
    let gspc = hp2_core::unix_clock::seconds_per_cycle() * 1000.0;
    // SAFETY: DrawStats runs on the engine app thread that owns the source counter.
    let scripts = unsafe { hp2_core::timing_globals::script_cycles() } as f32;
    let level = unsafe { &*level };
    let vals = [gspc * scripts, gspc * level.actor_tick_cycles as f32,
        gspc * level.find_path_cycles as f32, gspc * level.see_player as f32,
        gspc * level.spawning as f32, gspc * level.audio_tick_cycles as f32,
        gspc * level.unused as f32, gspc * level.move_cycles as f32,
        gspc * level.net_tick_cycles as f32];
    Ok(format!("Script={:05.1} Actor={:04.1} Path={:04.1} See={:04.1} Spawn={:04.1} Audio={:04.1} Un={:04.1} Move={:04.1} ({}) Net={:04.1}",
        vals[0], vals[1], vals[2], vals[3], vals[4], vals[5], vals[6], vals[7], level.num_moves, vals[8]))
}

fn name_string(access: CoreAccess<'_>, index: [u8; 4]) -> RuntimeResult<String> {
    let index = i32::from_ne_bytes(index);
    let text = access.name_text(index)?;
    Ok(text.iter().take_while(|&&unit| unit != 0)
        .filter_map(|&unit| char::from_u32(unit as u32))
        .collect())
}

fn cache_status(access: CoreAccess<'_>) -> RuntimeResult<String> {
    let owner = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine owner context is unavailable"))?;
    let mut out = vec![0i32; 1024];
    {
        let mut cache = owner.cache.borrow_mut();
        let cache = cache.as_mut().ok_or_else(|| RuntimeError::new("Canonical Engine cache owner is unavailable"))?;
        unsafe { cache.status(access, out.as_mut_ptr()); }
    }
    Ok(out.iter().take_while(|&&unit| unit != 0)
        .filter_map(|&unit| char::from_u32(unit as u32))
        .collect())
}

fn actor_level(viewport: *mut UViewport) -> RuntimeResult<*mut ULevel> {
    let actor = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned().cast::<AActor>() };
    if actor.is_null() {
        return Err(RuntimeError::new("Canonical stats Viewport has no Actor"));
    }
    let level = unsafe { ptr::addr_of!((*actor).x_level).read_unaligned().cast::<ULevel>() };
    if level.is_null() {
        return Err(RuntimeError::new("Canonical stats Actor has no Level"));
    }
    Ok(level)
}

fn global_panel(access: CoreAccess<'_>, canvas: *mut UCanvas, viewport: *mut UViewport,
    stats: &FStats, seconds_per_cycle: f32) -> RuntimeResult<()> {
    let level = actor_level(viewport)?;
    let engine = unsafe { ptr::addr_of!((*level).base.engine).read_unaligned().cast::<UEngine>() };
    if engine.is_null() { return Err(RuntimeError::new("Canonical stats Level has no Engine")); }
    let client = unsafe { ptr::addr_of!((*engine).client).read_unaligned().cast::<UClient>() };
    if client.is_null() { return Err(RuntimeError::new("Canonical stats Engine has no Client")); }
    let draw_cycles = unsafe { ptr::addr_of!((*client).draw_cycles).read_unaligned() };
    let g = unsafe { &*engine };
    let input = hp2_render::stats::GlobalStatsInput {
        frame_cycles: stats.frame_time, game_cycles: g.game_cycles, client_cycles: g.client_cycles,
        client_draw_cycles: draw_cycles, memory_cycles: stats.mem_time, stat_memory_cycles: stats.stat_mem_time,
        engine_tick_cycles: g.tick_cycles, external_cycles: stats.external_time, render_cycles: stats.render_time,
        mesh_cycles: stats.mesh_time, particle_cycles: stats.particle_time, polyv_cycles: stats.poly_v_time,
        illumination_cycles: stats.illum_time, occlusion_cycles: stats.occlusion_time, extra_cycles: stats.extra_time,
        filter_cycles: stats.filter_time, dynamic_filter_cycles: stats.dyn_filter_time,
        span_pixels: stats.span_pix, decal_cycles: stats.decal_time, decal_update_cycles: stats.decal_update_time,
        decal_clip_cycles: stats.decal_clip_time,
    };
    let n = hp2_render::stats::global_numbers(seconds_per_cycle, &input);
    show(access, canvas, "GLOBAL:", 0, false)?;
    show(access, canvas, &format!("  FRAME={:04.1}: GAME={:04.1} CLI={:04.1} BLIT={:04.1} MEM={:04.1}/{:04.1} PLAT={:04.1} EXT={:04.1}", n.frame_milliseconds, n.game_milliseconds, n.client_milliseconds, n.blit_milliseconds, n.memory_milliseconds, n.stat_memory_milliseconds, n.platform_milliseconds, n.external_milliseconds), 0, false)?;
    show(access, canvas, &format!("  {}", level_stats(level)?), 0, false)?;
    show(access, canvas, &format!("  RENDER={:04.1} MESH={:04.1} PART={:04.1} POLYV={:04.1} ILLUM={:04.1} OCC={:04.1} EX={:04.1}", n.render_milliseconds, n.mesh_milliseconds, n.particle_milliseconds, n.polyv_milliseconds, n.illumination_milliseconds, n.occlusion_milliseconds, n.extra_milliseconds), 0, false)?;
    show(access, canvas, &format!("  DYNAMICS: SETUP={:04.1} FILTER={:04.1} #SPRITES={} SPAN={:.0}Kpix", n.filter_milliseconds, n.dynamic_filter_milliseconds, stats.num_sprites, n.span_kpixels), 0, false)?;
    show(access, canvas, &format!("  DECALS:   TOTAL={:04.1} UPDATE={:04.1} DRAW={:04.1} CLIP={:04.1} #DECALS={}", n.decal_total_milliseconds, n.decal_update_milliseconds, n.decal_draw_milliseconds, n.decal_clip_milliseconds, stats.decal_count), 0, false)?;
    show(access, canvas, " ", 0, false)
}

fn network_panel(access: CoreAccess<'_>, canvas: *mut UCanvas, level: *mut ULevel,
    old_net_stats: &std::cell::Cell<bool>, saved_time: &std::cell::Cell<[u8; 8]>) -> RuntimeResult<()> {
    if level.is_null() {
        return Err(RuntimeError::new("Canonical NET stats panel has no Level"));
    }
    let base = unsafe { &(*level).base };
    let mut connection = ptr::null_mut();
    for driver in [base.net_driver.cast::<UNetDriver>(), base.demo_rec_driver.cast::<UNetDriver>()] {
        if !driver.is_null() {
            let conn = unsafe { ptr::addr_of!((*driver).server_connection).read_unaligned().cast::<UNetConnection>() };
            if !conn.is_null() { connection = conn; break; }
        }
    }
    if connection.is_null() { return Ok(()); }
    let c = unsafe { &*connection };
    let input = hp2_render::stats::NetworkStatsInput {
        best_lag: c.best_lag, in_order: c.in_order, out_order: c.out_order,
        in_loss: c.in_loss, out_loss: c.out_loss, in_packets: c.in_packets,
        out_packets: c.out_packets, in_bunches: c.in_bunches, out_bunches: c.out_bunches,
        in_rate: c.in_rate, out_rate: c.out_rate, current_net_speed: c.current_net_speed,
    };
    let channels = c.channels.map(|channel| channel.cast_const());
    let n = hp2_render::stats::network_numbers(&channels, &input);
    let update_time = c.stat_update_time;
    let log_update = saved_time.get() != update_time;
    let net_driver = base.net_driver.cast::<UNetDriver>();
    let lan_connection = if net_driver.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of!((*net_driver).server_connection).read_unaligned().cast::<UNetConnection>() }
    };
    let is_lan = if lan_connection.is_null() {
        false
    } else {
        let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
        const LAN_OPTION: [u32; 4] = [76, 65, 78, 0];
        unsafe {
            crate::url_owner::UrlOwner::has_option(
                access, &defaults, ptr::addr_of!((*lan_connection).url), LAN_OPTION.as_ptr(),
            )?
        }
    };
    show(access, canvas, "NET (IN/OUT):", 0, false)?;
    show(access, canvas, &format!("  {:4}          Ping", n.ping_milliseconds), 0, false)?;
    show(access, canvas, &format!("  {:4}          Channels", n.channel_count), 0, false)?;
    show(access, canvas, &format!("  {:4}    {:4}  Unordered/Sec", n.in_order, n.out_order), 0, false)?;
    show(access, canvas, &format!(" {:4}%   {:4}%  Packet Loss", n.in_loss, n.out_loss), 0, false)?;
    show(access, canvas, &format!("{:6}  {:6}  Packets/Sec", n.in_packets, n.out_packets), 0, false)?;
    show(access, canvas, &format!("{:6}  {:6}  Bunches/Sec", n.in_bunches, n.out_bunches), 0, false)?;
    show(access, canvas, &format!("{:6}{} {:6}{} Bytes/Sec", n.in_rate, if n.in_rate_saturated { "*" } else { " " }, n.out_rate, if n.out_rate_saturated { "*" } else { " " }), 0, false)?;
    show(access, canvas, &format!("{:6}  {:6}  {}Speed", n.net_speed, n.net_speed, if is_lan { "Lan" } else { "Net" }), 0, false)?;
    show(access, canvas, " ", 0, false)?;
    if log_update {
        saved_time.set(update_time);
        if !old_net_stats.get() {
            log_stats(access, "Ping  Ch Loss IRate ORate IBnch OBnch  IPkt  OPkt ILoss OLoss")?;
            log_stats(access, "---- --- ---- ----- ----- ----- ----- ----- ----- ----- -----")?;
        }
        log_stats(access, &format!("{:4} {:3} {:3}% {:5} {:5} {:5} {:5} {:5} {:5} {:5} {:5}",
            n.ping_milliseconds, n.channel_count, n.in_loss, n.in_rate, n.out_rate,
            n.in_bunches, n.out_bunches, n.in_packets, n.out_packets, n.in_loss, n.out_loss))?;
    }
    Ok(())
}

fn log_stats(access: CoreAccess<'_>, text: &str) -> RuntimeResult<()> {
    let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
    access.emit_log(category, &text.chars().map(u32::from).collect::<Vec<_>>())
}
