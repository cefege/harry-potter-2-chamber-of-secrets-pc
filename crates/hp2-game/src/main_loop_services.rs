use crate::boot_services::BootHost;
use hp2_core::core_access::{RuntimeError, RuntimeResult};
use hp2_launcher::main_loop::{
    self, Hp2LauncherMainLoopField as Field, Hp2LauncherMainLoopLeaf as Leaf,
    Hp2LauncherMainLoopPhase as Phase, Hp2LauncherMainLoopReporter as Reporter,
    Hp2LauncherMainLoopReporterOperation as Operation, Hp2LauncherMainLoopScope as Scope,
    Hp2LauncherMainLoopServices, Hp2LauncherMainLoopValue,
};
use std::{ffi::c_void, time::Instant};

#[derive(Default)]
struct State {
    actor_slot_dump_pending: i32,
    tick_count: i32,
    remaining_test_ticks: i32,
}
struct Context {
    boot: *mut BootHost,
    state: State,
    old_time: i64,
    second_start_time: i64,
    new_time: i64,
    trace_tick: u64,
    fixed_delta: f32,
    test_ticks: i32,
    timing: Option<FrameTiming>,
}

/// Opt-in (`HP2_FRAME_TIMING=1`) per-tick wall timing. Samples are buffered and
/// logged after the loop so measurement does not add log I/O to the frame.
#[derive(Default)]
struct FrameTiming {
    tick_ms: Vec<f32>,
    interval_ms: Vec<f32>,
    last_start: Option<Instant>,
}

fn percentile(sorted: &[f32], fraction: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f32 * fraction).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn summary(label: &str, samples: &[f32]) -> String {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f32::total_cmp);
    let mean = if sorted.is_empty() { 0.0 } else { sorted.iter().sum::<f32>() / sorted.len() as f32 };
    format!(
        "<HP2_PERF> {label} n={} mean_ms={mean:.3} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} max_ms={:.3}",
        sorted.len(), percentile(&sorted, 0.5), percentile(&sorted, 0.95),
        percentile(&sorted, 0.99), sorted.last().copied().unwrap_or(0.0),
    )
}

pub(super) fn run(boot: &mut BootHost, test_ticks: i32, fixed_delta: f32) -> RuntimeResult<()> {
    let mut context = Context {
        boot,
        state: State::default(),
        old_time: 0,
        second_start_time: 0,
        new_time: 0,
        trace_tick: 0,
        fixed_delta,
        test_ticks,
        timing: (std::env::var_os("HP2_FRAME_TIMING").as_deref() == Some(std::ffi::OsStr::new("1")))
            .then(FrameTiming::default),
    };
    let services = services(&mut context);
    let result = unsafe { main_loop::run_native(&services) }
        .map_err(|_| RuntimeError::new("canonical main-loop service failed"));
    if let Some(timing) = context.timing.take() {
        // Best effort: a missing owner after a failed loop must not mask `result`.
        let boot = unsafe { &*context.boot };
        for sample in &timing.tick_ms {
            let _ = log_resolution_record(boot, format!("<HP2_RES> frame_ms={sample:.3}").as_bytes());
        }
        let _ = log_resolution_record(boot, summary("tick", &timing.tick_ms).as_bytes());
        let _ = log_resolution_record(boot, summary("interval", &timing.interval_ms).as_bytes());
    }
    result
}

fn now() -> i64 {
    hp2_core::unix_clock::seconds()
}

fn services(context: &mut Context) -> Hp2LauncherMainLoopServices {
    Hp2LauncherMainLoopServices {
        context: (context as *mut Context).cast(),
        call,
        read_integer,
        write_integer,
        reporter,
        scope,
    }
}
unsafe extern "C" fn call(
    raw: *mut c_void,
    leaf: Leaf,
    argument: f32,
    out: *mut Hp2LauncherMainLoopValue,
) -> i32 {
    let c = unsafe { &mut *raw.cast::<Context>() };
    let boot = unsafe { &mut *c.boot };
    let result = call_leaf(c, boot, leaf, argument, unsafe { &mut *out });
    match result {
        Ok(()) => 0,
        Err(error) => {
            boot.retain(error);
            1
        }
    }
}
unsafe extern "C" fn read_integer(raw: *mut c_void, field: Field, out: *mut i32) -> i32 {
    let c = unsafe { &mut *raw.cast::<Context>() };
    let boot = unsafe { &mut *c.boot };
    let value = match field {
        Field::IsRunning => {
            let Some(owner) = boot.owner.as_ref() else {
                boot.retain(RuntimeError::new("GIsRunning read requires the canonical Core owner"));
                return 1;
            };
            i32::from(owner.access().is_running())
        }
        Field::IsRequestingExit => {
            i32::from(boot.exit_requested || hp2_core::unix_signal::exit_requested())
        }
        Field::ActorSlotDumpPending => c.state.actor_slot_dump_pending,
        Field::TestTicks => c.test_ticks,
        Field::TickCount => c.state.tick_count,
        Field::RemainingTestTicks => c.state.remaining_test_ticks,
    };
    unsafe {
        out.write(value);
    }
    0
}
unsafe extern "C" fn write_integer(raw: *mut c_void, field: Field, value: i32) -> i32 {
    let c = unsafe { &mut *raw.cast::<Context>() };
    let boot = unsafe { &mut *c.boot };
    match field {
        Field::IsRunning => {
            let Some(owner) = boot.owner.as_ref() else {
                boot.retain(RuntimeError::new("GIsRunning write requires the canonical Core owner"));
                return 1;
            };
            owner.access().set_running(value != 0);
        }
        Field::IsRequestingExit => {
            boot.exit_requested = value != 0;
            hp2_core::unix_signal::set_exit_requested(value != 0);
        }
        Field::ActorSlotDumpPending => c.state.actor_slot_dump_pending = value,
        Field::TestTicks => c.test_ticks = value,
        Field::TickCount => c.state.tick_count = value,
        Field::RemainingTestTicks => c.state.remaining_test_ticks = value,
    }
    0
}
unsafe extern "C" fn reporter(
    raw: *mut c_void,
    kind: Reporter,
    operation: Operation,
    argument: f32,
    out: *mut i32,
) -> i32 {
    let c = unsafe { &mut *raw.cast::<Context>() };
    let boot = unsafe { &mut *c.boot };
    let Some(reporters) = boot.reporters.as_mut() else {
        boot.retain(RuntimeError::new(
            "RuntimeReporters is unavailable during the main loop",
        ));
        return 1;
    };
    match reporters.reporter(
        kind,
        operation,
        argument,
        c.trace_tick,
        c.test_ticks,
        c.fixed_delta,
    ) {
        Ok(value) => {
            unsafe {
                out.write(value);
            }
            0
        }
        Err(error) => {
            boot.retain(error);
            1
        }
    }
}
unsafe extern "C" fn scope(raw: *mut c_void, which: Scope) -> i32 {
    let c = unsafe { &mut *raw.cast::<Context>() };
    let services = services(c);
    let result = match which {
        Scope::NativeState => {
            let result = unsafe { main_loop::run_phase(&services, Phase::NativeState) };
            // Fidelity is a native-state lexical local, including on failure.
            unsafe { &mut *c.boot }.end_loop_scope();
            result
        }
        Scope::TickTime => {
            c.new_time = now();
            unsafe { main_loop::run_phase(&services, Phase::TickTime) }
        }
    };
    match result {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
fn call_leaf(
    c: &mut Context,
    boot: &mut BootHost,
    leaf: Leaf,
    argument: f32,
    out: &mut Hp2LauncherMainLoopValue,
) -> RuntimeResult<()> {
    if let Some(value) = crate::fire_probe::loop_leaf(leaf, boot.engine.cast())? {
        out.integer = value;
        return Ok(());
    }
    match leaf {
        Leaf::InitializeNativeState => {
            c.state = State::default();
            c.old_time = 0;
            c.second_start_time = 0;
            c.trace_tick = 0;
        }
        Leaf::CheckEngine => {
            if boot.engine.is_null() {
                return Err(RuntimeError::new("MainLoop requires a live UGameEngine"));
            }
        }
        Leaf::ActorSlotDumpEnabled | Leaf::CapturePostStartup | Leaf::WriteActorSlotDump => {
            return Err(RuntimeError::new(
                "FireProbe did not handle an actor-slot reporter operation",
            ));
        }
        Leaf::FidelityEnabled | Leaf::CreateFidelityReporter => {
            let reporters = boot.reporters.as_mut().ok_or_else(|| {
                RuntimeError::new("RuntimeReporters unavailable during main loop")
            })?;
            out.integer = reporters.loop_leaf(leaf)?.ok_or_else(|| {
                RuntimeError::new("RuntimeReporters did not handle a fidelity operation")
            })?;
        }
        Leaf::SetOldTimeNow => c.old_time = now(),
        Leaf::CopySecondStartFromOld => c.second_start_time = c.old_time,
        Leaf::ResetTraceTickIndex => c.trace_tick = 0,
        Leaf::FixedDeltaSeconds => out.scalar = c.fixed_delta,
        Leaf::TickElapsed => out.scalar = hp2_core::time::subtract(c.new_time, c.old_time),
        Leaf::RandEngineTick => {
            hp2_core::ansi_trace::set_process_tick(c.trace_tick, b"engine_tick")
        }
        Leaf::EngineTick => {
            let owner = boot
                .owner
                .as_ref()
                .ok_or_else(|| RuntimeError::new("EngineOwner is unavailable during tick"))?;
            let start = c.timing.is_some().then(Instant::now);
            let ticked = unsafe {
                hp2_engine::game_engine_owner::tick(owner.access(), boot.engine, argument)
            };
            if let (Some(timing), Some(start)) = (c.timing.as_mut(), start) {
                timing.tick_ms.push(start.elapsed().as_secs_f32() * 1000.0);
                if let Some(previous) = timing.last_start.replace(start) {
                    timing.interval_ms.push(start.duration_since(previous).as_secs_f32() * 1000.0);
                }
            }
            ticked?;
        }
        Leaf::RandAfterTick => hp2_core::ansi_trace::set_process_tick(c.trace_tick, b"after_tick"),
        Leaf::CopyOldFromNew => c.old_time = c.new_time,
        Leaf::IncrementTickCount => c.state.tick_count += 1,
        Leaf::IncrementTraceTickIndex => c.trace_tick += 1,
        Leaf::DecrementRemainingTestTicks => {
            c.state.remaining_test_ticks -= 1;
            out.integer = c.state.remaining_test_ticks;
        }
        Leaf::RequestExit => {
            boot.exit_requested = true;
            hp2_core::unix_signal::set_exit_requested(true);
        }
        Leaf::RateIntervalElapsed => {
            out.integer = i32::from(hp2_core::time::subtract(c.old_time, c.second_start_time) > 1.0)
        }
        Leaf::PublishTickRate => {
            let elapsed = hp2_core::time::subtract(c.old_time, c.second_start_time);
            unsafe {
                (*boot.engine).current_tick_rate = c.state.tick_count as f32 / elapsed;
            }
        }
        Leaf::GetMaxTickRate => {
            let owner = boot
                .owner
                .as_ref()
                .ok_or_else(|| RuntimeError::new("EngineOwner is unavailable"))?;
            out.scalar = unsafe {
                hp2_engine::game_engine_owner::get_max_tick_rate(owner.access(), boot.engine)?
            };
        }
        Leaf::KeyboardFocusAbsent => {
            out.integer = i32::from(hp2_platform::keyboard_focus::keyboard_focus_absent())
        }
        Leaf::SleepFrameCap => {
            let elapsed = hp2_core::time::subtract(now(), c.old_time);
            let requested = (1.0f32 / argument) - elapsed;
            let delay = if 0.0f32 >= requested { 0.0 } else { requested };
            unsafe {
                hp2_core::unix_clock::sleep(delay);
            }
        }
        Leaf::LogScriptDeferred => log_resolution_record(boot, b"<HP2_RES> script_deferred=0")?,
        Leaf::LogScriptDeferralReasons => {
            log_resolution_record(boot, b"<HP2_RES> script_deferral_reasons=none")?
        }
        Leaf::LogScriptDeferralSubjects => {
            log_resolution_record(boot, b"<HP2_RES> script_deferral_subjects=none")?
        }
    }
    Ok(())
}
fn log_resolution_record(boot: &BootHost, text: &[u8]) -> RuntimeResult<()> {
    let owner = boot.owner.as_ref().ok_or_else(|| {
        RuntimeError::new("EngineOwner is unavailable while logging loop results")
    })?;
    let access = owner.access();
    let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
    let message = text.iter().copied().map(u32::from).collect::<Vec<_>>();
    access.emit_log(category, &message)
}
