//! `hp2rs` — the engine binary.
//!
//! Phase 3 headless/smoke mode: speaks the frozen harness protocol of
//! `Build/game_test.py`. The harness launches
//!
//! ```text
//! hp2rs -datadir=<root> <map_token> (-xopengl|-vulkan) -NOFRONTEND -window
//!       [-nosound] -testticks=N -log
//! ```
//!
//! Renderer flags are accepted and ignored (`--null-render` is implicit in
//! this mode). Optional engine extensions: `--smoke` (explicit no-op marker
//! for humans), `--rng-seed=<u64>`, `--fixed-dt=<secs>`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use hp_engine::error::EngineError;
use hp_engine::input::InputScript;
use hp_engine::rng::DEFAULT_SEED;
use hp_engine::sim::{Engine, EngineOptions};

const FRAME_TIMING_ENV: &str = "HP2_FRAME_TIMING";

struct Args {
    data_root: Option<PathBuf>,
    map_token: Option<String>,
    test_ticks: Option<u64>,
    rng_seed: u64,
    fixed_dt: Option<f32>,
    input_script: Option<PathBuf>,
}

fn usage() -> String {
    "usage: hp2rs -datadir=<root> <map_token> [-xopengl|-vulkan] -NOFRONTEND \
     -window [-nosound] -testticks=N -log [--null-render] [--smoke] \
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
        } else if arg.starts_with('-') {
            // Accepted-and-ignored harness switches (-xopengl, -vulkan,
            // -NOFRONTEND, -window, -nosound, -log) plus engine markers
            // (--null-render, --smoke). Unknown switches are loud.
            const KNOWN: [&str; 8] = [
                "-xopengl",
                "-vulkan",
                "-NOFRONTEND",
                "-window",
                "-nosound",
                "-log",
                "--null-render",
                "--smoke",
            ];
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

    let mut emitted: u64 = 0;
    match &script {
        Some(script) => {
            let frames = script.frames().len();
            engine.run_script_ticks(
                script,
                ticks.saturating_sub(frames as u64) as usize,
                |frame, ms| {
                    emitted += 1;
                    if timing {
                        println!("<HP2_RES> frame_ms={ms:.3}");
                    }
                    let _ = frame;
                },
            )?;
        }
        None => {
            for _ in 0..ticks {
                let started_frame = Instant::now();
                engine.tick(&[])?;
                if timing {
                    let ms = started_frame.elapsed().as_secs_f64() * 1000.0;
                    println!("<HP2_RES> frame_ms={ms:.3}");
                    emitted += 1;
                }
            }
        }
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
