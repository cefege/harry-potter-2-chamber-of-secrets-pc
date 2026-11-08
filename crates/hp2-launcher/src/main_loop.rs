//! Application-loop ordering shared by the native driver and browser phases.
//! Native services own FMainLoopState, genuine SDK reporters and exact FTime
//! operations. No native object is borrowed across a callback.
use std::ffi::c_void;

/// Each accessor performs exactly one live INT/UBOOL read or assignment.
/// `IsRunning` and `IsRequestingExit` name globals; the others name state fields.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherMainLoopField {
    IsRunning,
    IsRequestingExit,
    ActorSlotDumpPending,
    TestTicks,
    TickCount,
    RemainingTestTicks,
}

/// Presence tests and operations independently reread the named native pointer.
/// Fidelity is S.Reporter; Lifecycle, WorldCollision and StaticBsp are state
/// pointers; the remaining names are their original global reporter pointers.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherMainLoopReporter {
    Fidelity,
    Lifecycle,
    WorldCollision,
    StaticBsp,
    ShadowAdmission,
    ActorTransition,
    CreatureGenerator,
    GlobalTick,
}

/// No operation tests presence internally: Rust supplies the original branch.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherMainLoopReporterOperation {
    /// Return whether the pointer is nonnull through the integer output.
    Present,
    /// reporter->Run(S.Engine), for StaticBsp and WorldCollision only.
    Run,
    /// reporter->SetFrame(S.TraceTickIndex), using a fresh native index read.
    SetFrame,
    /// Lifecycle->BeginTick((INT)S.TraceTickIndex); narrow in native code.
    BeginTick,
    /// S.Reporter->AfterTick(S.Engine, argument), preserving the saved delta.
    AfterTick,
    /// Fidelity/Lifecycle: Flush(S.TestTicks, S.FixedDeltaSeconds), with live
    /// argument reads. All other reporters: Flush() with no arguments.
    Flush,
}

/// The only native lexical scopes. Each callback synchronously enters the
/// corresponding Rust phase using a fresh invocation context/service table.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherMainLoopScope {
    /// Directly construct `FMainLoopState State;` on the native stack, then call
    /// phase NativeState. Its unique_ptr member owns the real FidelityReporter
    /// until this scope ends. Do not value-initialize, heap-shadow or prefill it.
    /// The invocation context retains the original six MainLoop arguments.
    NativeState,
    /// Directly construct `FTime NewTime = appSeconds();`, then call TickTime.
    /// NewTime remains this exact native local throughout the nested call,
    /// including Engine->Tick/reentry and the eventual OldTime assignment.
    TickTime,
}

/// Browser entry points use Begin/Tick/End directly on their existing state.
/// TickTime and NativeState may only be entered by their matching native scope.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherMainLoopPhase {
    Begin,
    Tick,
    TickTime,
    FrameCap,
    End,
    NativeState,
}

/// Exact SDK/storage leaves, not alternative implementations of loop policy.
/// Unspecified arguments are zero and unspecified output members are ignored.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherMainLoopLeaf {
    /// In NativeState only, assign Engine, TestTicks, FixedDeltaSeconds,
    /// LifecycleReporter, WorldCollisionReporter and StaticBspProbe from the
    /// original arguments, in that order. Then assign ActorSlotDumpPending=0,
    /// OldTime=0.0f, SecondStartTime=0.0f, TickCount=0, RemainingTestTicks=0,
    /// TraceTickIndex=0, in that order. No reporter reset or other work.
    InitializeNativeState,
    /// check(S.Engine), retaining the native check configuration/behavior.
    CheckEngine,
    /// ActorSlotDump::IsEnabled(); output.integer is its UBOOL.
    ActorSlotDumpEnabled,
    /// ActorSlotDump::CapturePostStartupBeforeFirstTick(S.Engine).
    CapturePostStartup,
    /// FidelityReporter::IsEnabled(); output.integer is its UBOOL.
    FidelityEnabled,
    /// S.Reporter.reset(new FidelityReporter(S.Engine)), preserving allocation,
    /// construction, reset and old reporter destruction order exactly.
    CreateFidelityReporter,
    /// S.OldTime = appSeconds(), with the native FTime conversion/assignment.
    SetOldTimeNow,
    /// S.SecondStartTime = S.OldTime, with native FTime assignment.
    CopySecondStartFromOld,
    /// S.TraceTickIndex = 0, retaining its native unsigned long long type.
    ResetTraceTickIndex,
    /// Read S.FixedDeltaSeconds exactly once; output.scalar is FLOAT.
    FixedDeltaSeconds,
    /// TickTime scope only: (FLOAT)(NewTime - S.OldTime), native subtraction
    /// followed by native FLOAT narrowing; output.scalar. No appSeconds call.
    TickElapsed,
    /// appSetRandTraceTick(S.TraceTickIndex, "engine_tick"), with live index.
    RandEngineTick,
    /// S.Engine->Tick(argument), with live Engine and native virtual dispatch.
    EngineTick,
    /// appSetRandTraceTick(S.TraceTickIndex, "after_tick"), with live index.
    RandAfterTick,
    /// ActorSlotDump::WriteIfReady(S.Engine); output.integer is its UBOOL.
    WriteActorSlotDump,
    /// TickTime scope only: S.OldTime = NewTime, using the original FTime local.
    CopyOldFromNew,
    /// ++S.TickCount, using the native INT operation on the live field.
    IncrementTickCount,
    /// ++S.TraceTickIndex, using the native unsigned long long operation.
    IncrementTraceTickIndex,
    /// --S.RemainingTestTicks; output.integer is the resulting native INT.
    DecrementRemainingTestTicks,
    /// appRequestExit(0); do not set any extra flags or skip frame capping.
    RequestExit,
    /// S.OldTime - S.SecondStartTime > 1, including original FTime arithmetic
    /// and comparison precision; output.integer is the condition's truth value.
    RateIntervalElapsed,
    /// S.Engine->CurrentTickRate = (FLOAT)S.TickCount /
    /// (FLOAT)(S.OldTime - S.SecondStartTime), preserving native subtraction,
    /// both FLOAT conversions, division and assignment. Reread all fields.
    PublishTickRate,
    /// S.Engine->GetMaxTickRate(), retaining virtual dispatch; output.scalar.
    GetMaxTickRate,
    /// SDL_GetKeyboardFocus() == NULL; output.integer is the condition.
    KeyboardFocusAbsent,
    /// appSleep(Max(0.f, (1.f / argument) -
    /// (FLOAT)(appSeconds() - S.OldTime))). Keep this exact native expression:
    /// FTime subtraction, FLOAT narrowing, Max operand order and sleep argument
    /// conversion must not be replaced by copied timestamps/double arithmetic.
    SleepFrameCap,
    /// debugf(TEXT("<HP2_RES> script_deferred=0")).
    LogScriptDeferred,
    /// debugf(TEXT("<HP2_RES> script_deferral_reasons=none")).
    LogScriptDeferralReasons,
    /// debugf(TEXT("<HP2_RES> script_deferral_subjects=none")).
    LogScriptDeferralSubjects,
}

/// A successful callback writes only its documented output member.
#[repr(C)]
#[derive(Clone, Copy)]
pub union Hp2LauncherMainLoopValue {
    pub integer: i32,
    pub scalar: f32,
}

/// Copied before entering policy; context is an opaque invocation-local native
/// frame, never an exclusive Rust borrow of state. Reentrant calls use separate
/// frames and may modify the same native state. Every callback returns zero on
/// success or its exact nonzero captured failure; output is used only on success.
/// Accessors and presence tests do not invoke unrelated behavior or cache fields.
///
/// Native callbacks contain C++ exceptions and guarded signals below Rust.
/// A scope catches failures from constructing its local or executing its nested
/// Rust phase, retains the original failure, and returns its checked status.
/// If necessary it rethrows a nested captured native exception only inside its
/// own native catch, after the nested Rust call returns, so native automatic
/// destruction has the original unwind behavior without crossing a Rust frame.
/// The outer shell rethrows after Rust returns inside the original native guard.
/// Failure stops immediately: never synthesize End, Flush, retry or rollback.
/// Only genuine native automatic object destruction occurs on scope exit.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherMainLoopServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(
        *mut c_void,
        Hp2LauncherMainLoopLeaf,
        f32,
        *mut Hp2LauncherMainLoopValue,
    ) -> i32,
    pub read_integer: unsafe extern "C" fn(*mut c_void, Hp2LauncherMainLoopField, *mut i32) -> i32,
    pub write_integer: unsafe extern "C" fn(*mut c_void, Hp2LauncherMainLoopField, i32) -> i32,
    pub reporter: unsafe extern "C" fn(
        *mut c_void,
        Hp2LauncherMainLoopReporter,
        Hp2LauncherMainLoopReporterOperation,
        f32,
        *mut i32,
    ) -> i32,
    pub scope: unsafe extern "C" fn(*mut c_void, Hp2LauncherMainLoopScope) -> i32,
}

fn checked(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

impl Hp2LauncherMainLoopServices {
    fn call(
        &self,
        leaf: Hp2LauncherMainLoopLeaf,
        argument: f32,
    ) -> Result<Hp2LauncherMainLoopValue, i32> {
        let mut value = Hp2LauncherMainLoopValue { integer: 0 };
        checked(unsafe { (self.call)(self.context, leaf, argument, &mut value) })?;
        Ok(value)
    }

    fn integer(&self, leaf: Hp2LauncherMainLoopLeaf) -> Result<i32, i32> {
        Ok(unsafe { self.call(leaf, 0.0)?.integer })
    }

    fn scalar(&self, leaf: Hp2LauncherMainLoopLeaf) -> Result<f32, i32> {
        Ok(unsafe { self.call(leaf, 0.0)?.scalar })
    }

    fn read(&self, field: Hp2LauncherMainLoopField) -> Result<i32, i32> {
        let mut value = 0;
        checked(unsafe { (self.read_integer)(self.context, field, &mut value) })?;
        Ok(value)
    }

    fn write(&self, field: Hp2LauncherMainLoopField, value: i32) -> Result<(), i32> {
        checked(unsafe { (self.write_integer)(self.context, field, value) })
    }

    fn reporter(
        &self,
        reporter: Hp2LauncherMainLoopReporter,
        operation: Hp2LauncherMainLoopReporterOperation,
        argument: f32,
    ) -> Result<i32, i32> {
        let mut value = 0;
        checked(unsafe {
            (self.reporter)(self.context, reporter, operation, argument, &mut value)
        })?;
        Ok(value)
    }

    fn optional_reporter(
        &self,
        reporter: Hp2LauncherMainLoopReporter,
        operation: Hp2LauncherMainLoopReporterOperation,
        argument: f32,
    ) -> Result<(), i32> {
        if self.reporter(reporter, Hp2LauncherMainLoopReporterOperation::Present, 0.0)? != 0 {
            self.reporter(reporter, operation, argument)?;
        }
        Ok(())
    }

    fn scope(&self, scope: Hp2LauncherMainLoopScope) -> Result<(), i32> {
        checked(unsafe { (self.scope)(self.context, scope) })
    }
}

/// Execute the complete native MainLoop, including its actual native state and
/// unique_ptr lifetime. Browser callers instead keep their session-owned state
/// and invoke the public phases without constructing a second state.
/// # Safety
/// Services satisfy the checked callback and NativeState lexical scope contract.
pub unsafe fn run_native(services: &Hp2LauncherMainLoopServices) -> Result<(), i32> {
    services.scope(Hp2LauncherMainLoopScope::NativeState)
}

/// Execute one original phase, or the body of a native lexical scope.
/// # Safety
/// Services satisfy the callback contract. TickTime has a live native NewTime;
/// NativeState has a freshly constructed state and the six original arguments.
/// Other phases address the original already-owned FMainLoopState, not a copy.
pub unsafe fn run_phase(
    services: &Hp2LauncherMainLoopServices,
    phase: Hp2LauncherMainLoopPhase,
) -> Result<(), i32> {
    use Hp2LauncherMainLoopField as F;
    use Hp2LauncherMainLoopLeaf as L;
    use Hp2LauncherMainLoopPhase as P;
    use Hp2LauncherMainLoopReporter as R;
    use Hp2LauncherMainLoopReporterOperation as O;
    let s = services;
    match phase {
        P::Begin => {
            s.call(L::CheckEngine, 0.0)?;
            s.write(F::ActorSlotDumpPending, s.integer(L::ActorSlotDumpEnabled)?)?;
            s.call(L::CapturePostStartup, 0.0)?;
            s.optional_reporter(R::StaticBsp, O::Run, 0.0)?;
            s.optional_reporter(R::WorldCollision, O::Run, 0.0)?;
            if s.integer(L::FidelityEnabled)? != 0 {
                s.call(L::CreateFidelityReporter, 0.0)?;
            }
            s.write(F::IsRunning, 1)?;
            s.call(L::SetOldTimeNow, 0.0)?;
            s.call(L::CopySecondStartFromOld, 0.0)?;
            s.write(F::TickCount, 0)?;
            s.write(F::RemainingTestTicks, s.read(F::TestTicks)?)?;
            s.call(L::ResetTraceTickIndex, 0.0)?;
        }
        P::Tick => {
            s.optional_reporter(R::ShadowAdmission, O::SetFrame, 0.0)?;
            s.optional_reporter(R::CreatureGenerator, O::SetFrame, 0.0)?;
            s.optional_reporter(R::GlobalTick, O::SetFrame, 0.0)?;
            s.scope(Hp2LauncherMainLoopScope::TickTime)?;
        }
        P::TickTime => {
            // The second read in the positive branch is the original ternary's
            // selected operand. NaN takes the elapsed-time branch, not fixed dt.
            let delta = if s.scalar(L::FixedDeltaSeconds)? > 0.0 {
                s.scalar(L::FixedDeltaSeconds)?
            } else {
                let elapsed = s.scalar(L::TickElapsed)?;
                // Unreal Max(A, B) is A >= B ? A : B, not Rust f32::max.
                if elapsed >= 0.0001 { elapsed } else { 0.0001 }
            };
            s.optional_reporter(R::Lifecycle, O::BeginTick, 0.0)?;
            s.call(L::RandEngineTick, 0.0)?;
            s.call(L::EngineTick, delta)?;
            s.call(L::RandAfterTick, 0.0)?;
            if s.read(F::ActorSlotDumpPending)? != 0 && s.integer(L::WriteActorSlotDump)? != 0 {
                s.write(F::ActorSlotDumpPending, 0)?;
            }
            s.optional_reporter(R::Fidelity, O::AfterTick, delta)?;
            s.call(L::CopyOldFromNew, 0.0)?;
            s.call(L::IncrementTickCount, 0.0)?;
            s.call(L::IncrementTraceTickIndex, 0.0)?;
            if s.read(F::RemainingTestTicks)? > 0 && s.integer(L::DecrementRemainingTestTicks)? == 0
            {
                s.call(L::RequestExit, 0.0)?;
            }
            if s.integer(L::RateIntervalElapsed)? != 0 {
                s.call(L::PublishTickRate, 0.0)?;
                s.call(L::CopySecondStartFromOld, 0.0)?;
                s.write(F::TickCount, 0)?;
            }
        }
        P::FrameCap => {
            let mut target = s.scalar(L::GetMaxTickRate)?;
            // Keep <= rather than !(>), so NaN skips the keyboard focus query.
            if s.scalar(L::FixedDeltaSeconds)? <= 0.0 && s.integer(L::KeyboardFocusAbsent)? != 0 {
                target = if target > 0.0 {
                    if target <= 10.0 { target } else { 10.0 }
                } else {
                    10.0
                };
            }
            if target > 0.0 {
                s.call(L::SleepFrameCap, target)?;
            }
        }
        P::End => {
            s.write(F::IsRunning, 0)?;
            s.call(L::LogScriptDeferred, 0.0)?;
            s.call(L::LogScriptDeferralReasons, 0.0)?;
            s.call(L::LogScriptDeferralSubjects, 0.0)?;
            for reporter in [
                R::Fidelity,
                R::Lifecycle,
                R::WorldCollision,
                R::StaticBsp,
                R::ShadowAdmission,
                R::ActorTransition,
                R::CreatureGenerator,
                R::GlobalTick,
            ] {
                s.optional_reporter(reporter, O::Flush, 0.0)?;
            }
        }
        P::NativeState => {
            s.call(L::InitializeNativeState, 0.0)?;
            unsafe { run_phase(s, P::Begin) }?;
            while s.read(F::IsRunning)? != 0 && s.read(F::IsRequestingExit)? == 0 {
                unsafe { run_phase(s, P::Tick) }?;
                // RequestExit in Tick does not bypass this iteration's cap.
                unsafe { run_phase(s, P::FrameCap) }?;
            }
            unsafe { run_phase(s, P::End) }?;
        }
    }
    Ok(())
}
