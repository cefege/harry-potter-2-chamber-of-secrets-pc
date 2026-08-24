//! `hp2rs` — the engine binary.
//!
//! Phase 3 headless/smoke mode plus G4 offscreen rendering: speaks the
//! frozen harness protocol of `Build/game_test.py`. The harness launches
//!
//! ```text
//! hp2rs -datadir=<root> <map_token> (-xopengl|-vulkan) -NOFRONTEND -window
//!       [-nosound] -testticks=N -log
//! ```
//!
//! By default every tick end renders the loaded map offscreen (GPU); pass
//! `--null-render` to skip all GPU work. Optional engine extensions:
//! `--smoke`, `--rng-seed=<u64>`, `--fixed-dt=<secs>`, `--render-every=N`.
//! When `HP2_CAPTURE_FRAMES` names a directory and rendering is on, each
//! rendered frame is written there as `frame_%06d.png` plus a
//! `frame_meta.json` sidecar.

use std::path::{Path, PathBuf};
use std::time::Instant;

mod windowed;

use hp_engine::error::EngineError;
use hp_engine::input::InputScript;
use hp_engine::rng::DEFAULT_SEED;
use hp_engine::sim::{Engine, EngineOptions};

/// Wall-clock seconds after which an interactive session closes itself and
/// prints `<HP2_RES> presented_frames=N` (scripted verification hook).
const RUN_SECONDS_ENV: &str = "HP2_RUN_SECONDS";

const FRAME_TIMING_ENV: &str = "HP2_FRAME_TIMING";
const CAPTURE_FRAMES_ENV: &str = "HP2_CAPTURE_FRAMES";

struct Args {
    data_root: Option<PathBuf>,
    map_token: Option<String>,
    test_ticks: Option<u64>,
    rng_seed: u64,
    fixed_dt: Option<f32>,
    input_script: Option<PathBuf>,
    /// `--smoke`: the headless harness path. Its ABSENCE (with no other
    /// headless marker) selects interactive windowed mode.
    smoke: bool,
    /// `-NOFRONTEND`: harness switch that also pins the headless path.
    no_front_end: bool,
    /// `-nosound` / `--nosound`: boot without audio (default ENABLED).
    nosound: bool,
    /// `--null-render`: skip all GPU work (the old CI behavior, now explicit).
    null_render: bool,
    /// Render offscreen every Nth tick (default 1) when rendering is on.
    render_every: u64,
}

fn usage() -> String {
    "usage: hp2rs -datadir=<root> <map_token> [options]\n\
     interactive windowed mode (default): [--nosound] [--null-render] \
     [-xopengl|-vulkan] -window -testticks=N -log\n\
     headless harness mode: add --smoke [-nosound] [--render-every=N] \
     [--rng-seed=<u64>] [--fixed-dt=<secs>] [--input-script=<file>]"
        .to_string()
}

fn parse_args(argv: Vec<String>) -> Result<Args, EngineError> {
    let mut args = Args {
        data_root: None,
        map_token: None,
        test_ticks: None,
        rng_seed: DEFAULT_SEED,
        fixed_dt: None,
        input_script: None,
        smoke: false,
        no_front_end: false,
        nosound: false,
        null_render: false,
        render_every: 1,
    };
    for arg in argv {
        if let Some(value) = arg.strip_prefix("-datadir=") {
            args.data_root = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("-testticks=") {
            args.test_ticks = Some(value.parse::<u64>().map_err(|_| {
                EngineError::new("engine.arg_ticks", format!("bad tick count {value:?}"))
            })?);
        } else if let Some(value) = arg.strip_prefix("--rng-seed=") {
            let text = value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"));
            let seed = match text {
                Some(hex) => u64::from_str_radix(hex, 16),
                None => value.parse::<u64>(),
            };
            args.rng_seed = seed
                .map_err(|_| EngineError::new("engine.arg_seed", format!("bad seed {value:?}")))?;
        } else if let Some(value) = arg.strip_prefix("--fixed-dt=") {
            let dt: f32 = value.parse().map_err(|_| {
                EngineError::new("engine.arg_fixed_dt", format!("bad delta {value:?}"))
            })?;
            if !dt.is_finite() || dt <= 0.0 {
                return Err(EngineError::new(
                    "engine.arg_fixed_dt",
                    format!("delta must be finite and positive, got {value:?}"),
                ));
            }
            args.fixed_dt = Some(dt);
        } else if let Some(value) = arg.strip_prefix("--input-script=") {
            args.input_script = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--render-every=") {
            let n: u64 = value.parse().map_err(|_| {
                EngineError::new(
                    "engine.arg_render_every",
                    format!("bad render-every count {value:?}"),
                )
            })?;
            if n == 0 {
                return Err(EngineError::new(
                    "engine.arg_render_every",
                    "render-every must be at least 1",
                ));
            }
        } else if arg == "--null-render" {
            args.null_render = true;
        } else if arg == "--smoke" {
            // The engine marker selecting the frozen headless harness path.
            args.smoke = true;
        } else if arg == "-nosound" || arg == "--nosound" {
            args.nosound = true;
        } else if arg == "-NOFRONTEND" {
            args.no_front_end = true;
        } else if arg.starts_with('-') {
            // Remaining accepted-and-ignored harness switches (-xopengl,
            // -vulkan, -window, -log). Unknown switches are loud.
            const KNOWN: [&str; 4] = ["-xopengl", "-vulkan", "-window", "-log"];
            if !KNOWN.contains(&arg.as_str()) {
                return Err(EngineError::new(
                    "engine.arg_unknown",
                    format!("unrecognized switch {arg:?}; {}", usage()),
                ));
            }
        } else {
            // The bare map token (`..\Maps\PrivetDr.unr` style).
            if args.map_token.is_some() {
                return Err(EngineError::new(
                    "engine.arg_map_twice",
                    format!("unexpected extra argument {arg:?}"),
                ));
            }
            args.map_token = Some(arg);
        }
    }
    Ok(args)
}

fn load_ini_set(data_root: &Path) -> Result<hp_ini::IniSet, EngineError> {
    let system = data_root.join("System");
    let default_ini = system.join("Default.ini");
    if !default_ini.is_file() {
        return Err(EngineError::new(
            "engine.ini_missing",
            format!("{} is not a file", default_ini.display()),
        ));
    }
    let user_ini = system.join("User.ini");
    let user_layer = if user_ini.is_file() {
        Some(user_ini)
    } else {
        Some(system.join("DefUser.ini"))
    };
    hp_ini::load_pair(&default_ini, user_layer.as_deref())
        .map_err(|error| EngineError::new("engine.ini_parse", error.to_string()))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), EngineError> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse_args(argv)?;

    let Some(data_root) = args.data_root else {
        return Err(EngineError::new(
            "engine.arg_datadir",
            format!("-datadir=<root> is required; {}", usage()),
        ));
    };
    let Some(map_token) = args.map_token else {
        return Err(EngineError::new(
            "engine.arg_map",
            format!("a map token is required; {}", usage()),
        ));
    };
    let ticks = args.test_ticks.unwrap_or(0);

    // Interactive windowed mode is the default for a plain invocation
    // (e.g. `-window -testticks=0`): a live window over the rendered level
    // with the audio subsystem initialized. The headless path below is the
    // frozen harness protocol and stays byte-stable; it is selected by
    // `--smoke` or by the harness' own markers (`-NOFRONTEND`, a positive
    // `-testticks=N`).
    let headless = args.smoke || args.no_front_end || ticks > 0;
    if !headless {
        return run_interactive(
            &data_root,
            &map_token,
            args.rng_seed,
            args.fixed_dt,
            args.null_render,
            args.nosound,
        );
    }

    let ini = load_ini_set(&data_root)?;
    let options = EngineOptions {
        rng_seed: args.rng_seed,
        fixed_dt: args.fixed_dt,
        editor_mode: false,
    };

    let mut engine = Engine::bootstrap(&data_root, ini, options)?;
    engine.load_map(&map_token)?;
    // No wall-clock values on success paths: two identical runs must
    // produce byte-identical output (Docs/RNG_TICK_DIVERGENCE.md §2).
    println!(
        "hp2rs: bootstrapped {} objects, map {} loaded with {} actors",
        engine.world.arena.len(),
        map_token.replace('\\', "/"),
        engine.level.actors.len(),
    );

    // Optional scripted-input feed (fixture grammar); otherwise idle ticks.
    let script = match &args.input_script {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|error| {
                EngineError::new(
                    "engine.input_script_unreadable",
                    format!("{}: {error}", path.display()),
                )
            })?;
            Some(InputScript::parse(&text)?)
        }
        None => None,
    };

    let timing = std::env::var(FRAME_TIMING_ENV).is_ok();
    let rng_probe = engine.rng.next_u32();
    println!("<HP2_RES> rng_probe={rng_probe:08x}");

    // G4: real offscreen rendering unless explicitly disabled. The scene is
    // extracted once after map load; brush geometry never mutates under the
    // smoke tick loop.
    let mut session = if args.null_render {
        None
    } else {
        // Scene extraction is still incomplete for retail maps: HP2's
        // UModel->Polys linkage uses a wire layout we have not fully
        // reversed (see Tests/Fixtures/render-baseline/rust_vs_cpp.md).
        // Degrade loudly to null rendering instead of failing the run.
        match hp_engine::scene::build_render_scene(&engine.world.arena, &engine.level).and_then(
            |scene| {
                println!(
                    "hp2rs: render scene built with {} polys, camera {}",
                    scene.polys.len(),
                    match scene.camera {
                        Some(camera) => format!("at {:?}", camera.location),
                        None => "at default origin".to_string(),
                    },
                );
                let mut session = hp_engine::render_bridge::RendererSession::new(&data_root)?;
                session.load_scene(&scene)?;
                Ok(session)
            },
        ) {
            Ok(session) => Some(session),
            Err(error) => {
                eprintln!(
                    "hp-engine: [renderer.scene_unavailable] falling back to null render: {error}"
                );
                None
            }
        }
    };

    let capture_dir = std::env::var(CAPTURE_FRAMES_ENV).ok();
    let mut capture = match (&capture_dir, session.as_ref()) {
        (Some(dir), Some(_)) => Some(
            hp_render::FrameCapture::new(dir)
                .map_err(|error| EngineError::new("renderer.capture_dir", error.to_string()))?,
        ),
        _ => None,
    };

    let map_name = map_token
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("Map")
        .trim_end_matches(".unr")
        .to_string();

    let mut emitted: u64 = 0;
    let mut rendered_frames: u64 = 0;
    match &script {
        Some(script) => {
            let frames = script.frames().len();
            let total = frames + ticks.saturating_sub(frames as u64) as usize;
            for index in 0..total {
                let started_frame = Instant::now();
                if index < frames {
                    let events: Vec<(u8, u8, f32)> = script.frames()[index]
                        .iter()
                        .map(|&(key, act)| (key, act, script.tick_delta))
                        .collect();
                    engine.tick(&events)?;
                } else {
                    engine.tick(&[])?;
                }
                rendered_frames += render_tick_end(
                    session.as_mut(),
                    capture.as_mut(),
                    args.render_every,
                    index as u64 + 1,
                    &map_name,
                )?;
                if timing {
                    let ms = started_frame.elapsed().as_secs_f64() * 1000.0;
                    println!("<HP2_RES> frame_ms={ms:.3}");
                    emitted += 1;
                }
            }
        }
        None => {
            for index in 0..ticks {
                let started_frame = Instant::now();
                engine.tick(&[])?;
                rendered_frames += render_tick_end(
                    session.as_mut(),
                    capture.as_mut(),
                    args.render_every,
                    index + 1,
                    &map_name,
                )?;
                if timing {
                    let ms = started_frame.elapsed().as_secs_f64() * 1000.0;
                    println!("<HP2_RES> frame_ms={ms:.3}");
                    emitted += 1;
                }
            }
        }
    }

    // Renderer teardown first so the texture counters balance before the
    // summary markers are emitted (the harness keeps the last occurrence).
    let texture_markers = match session.as_mut() {
        Some(session) => {
            session.shutdown();
            session
                .textures()
                .assert_balanced()
                .map_err(|error| EngineError::new("renderer.texture_leak", error.to_string()))?;
            Some(session.resource_marker_lines())
        }
        None => None,
    };
    if let Some(lines) = texture_markers {
        for line in lines {
            println!("{line}");
        }
    }

    if let (Some(capture), Some(session)) = (capture.take(), session.take()) {
        let meta = capture
            .finish()
            .map_err(|error| EngineError::new("renderer.capture_meta", error.to_string()))?;
        drop(session);
        println!(
            "hp2rs: captured {rendered_frames} frames to {}",
            meta.parent().unwrap_or(Path::new(".")).display(),
        );
    }

    let counters = engine.counters();
    println!(
        "hp2rs: complete ticks={} input_events={} console_commands={} cues={} script_deferred={} timing_lines={emitted}",
        counters.ticks,
        counters.input_events,
        counters.console_commands,
        engine.cues.len(),
        counters.script_deferrals,
    );
    Ok(())
}

/// Interactive windowed mode: bootstrap the engine, load the map, build
/// the render scene (degrading loudly to null render when scene extraction
/// cannot), then hand over to the winit game handler.
fn run_interactive(
    data_root: &Path,
    map_token: &str,
    rng_seed: u64,
    fixed_dt: Option<f32>,
    null_render: bool,
    nosound: bool,
) -> Result<(), EngineError> {
    let ini = load_ini_set(data_root)?;
    let options = EngineOptions {
        rng_seed,
        fixed_dt,
        editor_mode: false,
    };
    let mut engine = Engine::bootstrap(data_root, ini, options)?;
    engine.load_map(map_token)?;
    println!(
        "hp2rs: bootstrapped {} objects, map {} loaded with {} actors",
        engine.world.arena.len(),
        map_token.replace('\\', "/"),
        engine.level.actors.len(),
    );

    // Scene extraction is still incomplete for retail maps (see
    // Tests/Fixtures/render-baseline/rust_vs_cpp.md): degrade loudly to a
    // black window instead of failing the run.
    let mut camera = hp_engine::scene::SceneCamera::default();
    let session = if null_render {
        None
    } else {
        match hp_engine::scene::build_render_scene(&engine.world.arena, &engine.level).and_then(
            |scene| {
                println!(
                    "hp2rs: render scene built with {} polys, camera {}",
                    scene.polys.len(),
                    match scene.camera {
                        Some(found) => format!("at {:?}", found.location),
                        None => "at default origin".to_string(),
                    },
                );
                camera = scene.camera_or_default();
                let mut session = hp_engine::render_bridge::RendererSession::new(data_root)?;
                session.load_scene(&scene)?;
                Ok(session)
            },
        ) {
            Ok(session) => Some(session),
            Err(error) => {
                // Geometry extraction is still incomplete for retail maps
                // (Tests/Fixtures/render-baseline/rust_vs_cpp.md). Keep the
                // GPU session so the swapchain still presents cleared frames
                // — a live black window, loudly explained — instead of
                // dropping to no rendering at all.
                eprintln!(
                    "hp-engine: [renderer.scene_unavailable] presenting cleared frames without world geometry: {error}"
                );
                Some(hp_engine::render_bridge::RendererSession::new(data_root)?)
            }
        }
    };

    let run_seconds = std::env::var(RUN_SECONDS_ENV)
        .ok()
        .and_then(|text| text.parse::<f64>().ok());

    println!("hp2rs: interactive mode — WASD/arrows fly the camera, Q/E up/down, ESC quits");
    let app = windowed::GameApp::new(engine, session, camera, data_root, !nosound, run_seconds);
    let event_loop = winit::event_loop::EventLoop::builder()
        .build()
        .map_err(|error| EngineError::new("engine.event_loop_unavailable", error.to_string()))?;
    app.run(event_loop)
}

/// Render-at-tick-boundary policy shared by both loop paths. Returns 1 when a
/// frame was produced this tick, 0 otherwise (also the no-renderer answer).
fn render_tick_end(
    session: Option<&mut hp_engine::render_bridge::RendererSession>,
    capture: Option<&mut hp_render::FrameCapture>,
    render_every: u64,
    tick: u64,
    map_name: &str,
) -> hp_engine::error::Result<u64> {
    let Some(session) = session else {
        return Ok(0);
    };
    if !tick.is_multiple_of(render_every) {
        return Ok(0);
    }
    session.render_frame();
    if let Some(capture) = capture {
        session.capture_frame(capture, tick as u32, map_name)?;
    }
    Ok(1)
}
