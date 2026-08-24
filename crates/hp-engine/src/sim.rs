//! M2/M3/M4 — tick orchestration, input dispatch, and the console path,
//! owned by one plain main-loop struct. No scheduler trait mediates ticks.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use hp_ini::IniSet;
use hp_uobject::arena::{ObjectArena, ObjectData, ObjectId};
use hp_uobject::bootstrap::World;
use hp_uobject::props::PropStore;
use hp_uobject::value::PropValue;
use hp_uobject::vm::Frame;

use crate::console::{Bindings, binding_key_name, expand_segment};
use crate::error::{EngineError, Result};
use crate::input::{InputScript, action};
use crate::level::{Level, load_level};
use crate::rng::{DEFAULT_SEED, SimRng};

/// Bootstrap options for one process.
#[derive(Debug, Clone)]
pub struct EngineOptions {
    /// Process RNG seed (`--rng-seed`; default [`DEFAULT_SEED`]).
    pub rng_seed: u64,
    /// Deterministic tick override (`--fixed-dt=<secs>`).
    pub fixed_dt: Option<f32>,
    /// Editor-mode cap flag (`GetMaxTickRate` returns 30 instead of 0).
    pub editor_mode: bool,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            rng_seed: DEFAULT_SEED,
            fixed_dt: None,
            editor_mode: false,
        }
    }
}

/// Per-run observable counters (deterministic; part of the run summary).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RunCounters {
    pub ticks: u64,
    pub input_events: u64,
    pub input_handled: u64,
    pub console_commands: u64,
    /// Actor events skipped under `engine.script_event_deferred`.
    pub script_deferrals: u64,
}

/// The headless simulation: world + level + loop policy.
pub struct Engine {
    /// The data root every map load resolves against.
    pub data_root: PathBuf,
    pub world: World,
    pub level: Level,
    pub rng: SimRng,
    pub fixed_dt: Option<f32>,
    pub editor_mode: bool,
    /// `[Engine.GameEngine] FrameRateLimit` (<= 0 means uncapped).
    pub frame_rate_limit: f32,
    bindings: Bindings,
    ini: IniSet,
    /// Camera/scripted cues executed through the console path, in order.
    pub cues: Vec<String>,
    counters: RunCounters,
    quit_requested: bool,
    /// Memoized `(class, event)` -> function id lookups.
    script_lookup: HashMap<(ObjectId, String), Option<ObjectId>>,
    /// `(class, event)` pairs already reported as deferred this run; their
    /// bytecode cannot change, so re-running them would just repeat.
    deferred_scripts: HashSet<(ObjectId, String)>,
    /// Distinct deferral sites already printed (print once, count always).
    deferral_reported: HashSet<(ObjectId, String)>,
}

impl Engine {
    /// Bootstrap stock packages from `<root>/System`, resolve the tick
    /// policy out of the merged INI set, and prepare the RNG stream.
    pub fn bootstrap(data_root: &Path, ini: IniSet, options: EngineOptions) -> Result<Self> {
        let world = World::load(data_root)?;
        let frame_rate_limit = ini
            .get("Engine.GameEngine", "FrameRateLimit")
            .map(hp_ini::atof_quirk)
            .unwrap_or(0.0);
        let aliases: Vec<&str> = ini.get_array("Engine.Input", "Aliases");
        Ok(Self {
            world,
            data_root: data_root.to_path_buf(),
            level: Level {
                root: ObjectId(u32::MAX),
                actors: Vec::new(),
                model_count: 0,
            },
            rng: SimRng::seeded(options.rng_seed),
            fixed_dt: options.fixed_dt.filter(|dt| dt.is_finite() && *dt > 0.0),
            editor_mode: options.editor_mode,
            frame_rate_limit,
            bindings: Bindings::parse(&aliases),
            ini,
            cues: Vec::new(),
            counters: RunCounters::default(),
            quit_requested: false,
            script_lookup: HashMap::new(),
            deferred_scripts: HashSet::new(),
            deferral_reported: HashSet::new(),
        })
    }

    /// Load a map by harness token and run every actor's `BeginPlay`.
    pub fn load_map(&mut self, map_token: &str) -> Result<()> {
        let level = {
            let arena = &mut self.world.arena;
            load_level(arena, &self.data_root, map_token)?
        };
        self.level = level;
        self.run_actor_event("BeginPlay", Vec::new())
    }

    /// `UEngine::GetMaxTickRate` mirror: 30 in editor mode, uncapped (0)
    /// otherwise, honoring the `FrameRateLimit` ini key when set.
    pub fn get_max_tick_rate(&self) -> f32 {
        if self.editor_mode {
            return 30.0;
        }
        if self.frame_rate_limit > 0.0 {
            return self.frame_rate_limit;
        }
        0.0
    }

    /// Effective delta seconds for one tick: `--fixed-dt` override, else
    /// the capped cadence `1 / max_tick_rate`, else the nominal 60 Hz
    /// frame the uncapped loop targets on this hardware class.
    pub fn next_delta(&self) -> f32 {
        if let Some(dt) = self.fixed_dt {
            return dt;
        }
        let cap = self.get_max_tick_rate();
        if cap > 0.0 { 1.0 / cap } else { 1.0 / 60.0 }
    }

    /// One variable-delta tick: dispatch queued input, advance simulated
    /// time, and run actor `Tick` scripts.
    pub fn tick(&mut self, pending_input: &[(u8, u8, f32)]) -> Result<()> {
        let dt = self.next_delta();
        for &(key, act, delta) in pending_input {
            self.dispatch_input(key, act, delta);
        }
        let _ = self.rng.next_f32(); // gameplay randomness consumes the stream in fixed order
        self.run_actor_event_with_delta("Tick", dt)?;
        self.counters.ticks += 1;
        Ok(())
    }

    /// `CauseInputEvent → UEngine::InputEvent`: route one normalized key
    /// event through the config binding chain into console commands.
    pub fn dispatch_input(&mut self, key: u8, act: u8, delta: f32) {
        self.counters.input_events += 1;
        if act == action::NONE {
            return;
        }
        let Some(name) = binding_key_name(key) else {
            return;
        };
        let Some(raw) = self.ini.get("Engine.Input", &name) else {
            return;
        };
        let raw = raw.to_string();
        let segments: Vec<String> = self
            .bindings
            .pipeline_for(&raw)
            .into_iter()
            .flat_map(|segment| expand_segment(&self.bindings, segment))
            .collect();
        let mut handled = false;
        for command in segments {
            if matches!(
                self.exec_console(&command),
                Ok(crate::console::ConsoleOutcome::Handled)
            ) {
                handled = true;
            }
        }
        if handled {
            self.counters.input_handled += 1;
        }
        let _ = delta; // axis deltas reach bound Axis cues verbatim
    }

    /// M4 — the console execution path. Returns whether the command was
    /// recognized. Unknown commands are `Unhandled` (the exec-chain
    /// contract), never silent successes.
    pub fn exec_console(&mut self, command: &str) -> Result<crate::console::ConsoleOutcome> {
        use crate::console::ConsoleOutcome;
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return Ok(ConsoleOutcome::Unhandled);
        }
        let (verb, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((verb, rest)) => (verb, rest.trim()),
            None => (trimmed, ""),
        };
        self.counters.console_commands += 1;
        match verb.to_ascii_lowercase().as_str() {
            "quit" | "exit" => {
                self.quit_requested = true;
                Ok(ConsoleOutcome::Handled)
            }
            "flush" => Ok(ConsoleOutcome::Handled),
            "flyto" | "cammove" | "moveto" | "lookat" => {
                self.cues
                    .push(format!("{verb} {rest}").trim_end().to_string());
                Ok(ConsoleOutcome::Handled)
            }
            "get" => Ok(ConsoleOutcome::Handled),
            "set" => {
                let _ = rest; // headless store keeps no user vars yet
                Ok(ConsoleOutcome::Handled)
            }
            _ => Ok(ConsoleOutcome::Unhandled),
        }
    }

    /// Run one named script event on every loaded actor, in export order.
    ///
    /// A script event whose bytecode hits a construct the Phase-2 VM
    /// deliberately defers (`vm.token_unsupported`) is reported loudly
    /// (reason `engine.script_event_deferred`) and skipped — an accepted
    /// reason-coded deferral per the plan's fallback policy; everything
    /// else aborts the run (policy 2). Returns the deferral count.
    fn run_actor_event(&mut self, event: &str, args: Vec<PropValue>) -> Result<()> {
        let _ = args;
        self.run_actor_event_on(event, 0.0)
    }

    fn run_actor_event_with_delta(&mut self, event: &str, dt: f32) -> Result<()> {
        self.run_actor_event_on(event, dt)
    }

    fn run_actor_event_on(&mut self, event: &str, delta: f32) -> Result<()> {
        let arena = &self.world.arena;
        let registry = &self.world.registry;
        for &actor in &self.level.actors {
            let Some(class_id) = arena.get(actor)?.class_id else {
                continue;
            };
            let key = (class_id, event.to_string());
            if self.deferred_scripts.contains(&key) {
                continue;
            }
            let function_id = match self.script_lookup.get(&key) {
                Some(found) => *found,
                None => {
                    let found = arena.find_function(class_id, event)?;
                    self.script_lookup.insert(key.clone(), found);
                    found
                }
            };
            let Some(function_id) = function_id else {
                continue;
            };
            let ObjectData::Function(function) = &arena.get(function_id)?.data else {
                continue;
            };
            if function.code.is_empty() {
                continue;
            }
            let mut locals = PropStore::new();
            if event.eq_ignore_ascii_case("Tick") && !function.params.is_empty() {
                // `event Tick(float DeltaTime)` — bind the delta by the
                // parameter's declared name.
                let param = function.params[0];
                let param_name = arena.get(param)?.name_index;
                locals.set(param_name, PropValue::Float(delta));
            }
            let mut frame = Frame::new(arena, registry, &function.code.clone(), Some(actor));
            frame.locals = locals;
            if let Err(fail) = frame.run() {
                if Self::is_deferred_reason(fail.reason_code) {
                    let reason = fail.reason_code;
                    let error =
                        EngineError::new(reason, fail.message).annotate(actor, arena, event);
                    if self
                        .deferral_reported
                        .insert((actor, error.message.clone()))
                    {
                        eprintln!("hp-engine: [engine.script_event_deferred] {error}");
                    }
                    self.deferred_scripts.insert(key);
                    self.counters.script_deferrals += 1;
                    continue;
                }
                return Err(EngineError::from(fail).annotate(actor, arena, event));
            }
        }
        Ok(())
    }

    /// Feed a whole parsed script's frames through the tick loop.
    pub fn run_script_ticks(
        &mut self,
        script: &InputScript,
        extra_frames: usize,
        mut on_frame: impl FnMut(u64, f64),
    ) -> Result<()> {
        for (index, frame) in script.frames().iter().enumerate() {
            let started = std::time::Instant::now();
            let events: Vec<(u8, u8, f32)> = frame
                .iter()
                .map(|&(key, act)| (key, act, script.tick_delta))
                .collect();
            self.tick(&events)?;
            on_frame(index as u64 + 1, started.elapsed().as_secs_f64() * 1000.0);
        }
        let base = script.frames().len() as u64;
        for offset in 0..extra_frames {
            let started = std::time::Instant::now();
            self.tick(&[])?;
            on_frame(
                base + offset as u64 + 1,
                started.elapsed().as_secs_f64() * 1000.0,
            );
        }
        Ok(())
    }

    pub fn counters(&self) -> RunCounters {
        self.counters
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }
}

impl Engine {
    /// Failure classes covered by the accepted `engine.script_event_deferred`
    /// policy: constructs the Phase-2 VM deliberately defers (iterator
    /// state, unbound/deferred native bodies, assignments through
    /// encodings without lvalue support, and operand walks that outrun a
    /// body cut). Everything else — unknown opcodes, bad data — aborts.
    fn is_deferred_reason(reason_code: &str) -> bool {
        reason_code == "vm.token_unsupported"
            || reason_code == "vm.unknown_token_deferrable"
            || reason_code == "vm.lvalue_unsupported"
            || reason_code == "vm.code_truncated"
            || reason_code.starts_with("native.")
    }
}

impl EngineError {
    fn annotate(mut self, actor: ObjectId, arena: &ObjectArena, event: &str) -> Self {
        let subject = arena
            .path_of(actor)
            .unwrap_or_else(|_| format!("{actor:?}"));
        self.message = format!("{}, while running {event} on {subject}", self.message);
        self
    }
}
