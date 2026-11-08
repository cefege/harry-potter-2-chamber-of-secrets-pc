//! Replaces `main()` and `HP2BootAfterLauncher()`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`): the full process
//! boot sequence from argument parsing through entering (and leaving) the
//! main loop. Native retains every SDK/engine call, every global write,
//! and the one C++ try/catch boundary each phase already had; this module
//! owns the call order and every branch between them.
//!
//! Four reporter globals (`shadow_admission`, `actor_transition_ledger`,
//! `creature_generator`, `global_tick`) and `fire_texture`'s
//! enabled/create/flush/reset are called directly, in-crate, with no C++
//! leaf at all: they were already direct `hp2_rs_launcher_*` calls with no
//! native logic of their own (a native leaf would add a round trip with no
//! native-only work on either side). `fire_texture_set_target` keeps
//! calling through the existing native forwarder because only native
//! holds the resolved `UFireTexture*` pointer. `LifecycleGateReporter`,
//! `WorldCollisionGateReporter` and `StaticBspProbeReporter` stay behind
//! their existing C++ classes unmodified: constructing one installs a C++
//! global observer pointer that deep engine code calls through virtual
//! dispatch (rule 2), so their construction and `IsEnabled()` stay native
//! leaves reusing those classes as-is.

use crate::reporters::{actor_transition_ledger, creature_generator, fire_texture, global_tick, shadow_admission};

fn checked(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Hp2LauncherBootValue {
    pub integer: i32,
    pub scalar: f32,
}

/// Every native action `main()`/`HP2BootAfterLauncher()` performed,
/// doc-commented with the exact original statement it replaces. Shared by
/// both phases: native constructs a different lexical frame per phase
/// (holding `ArgC`/`ArgV`/`EngineCommandLine`/`CrashConfig`, or the
/// after-launcher engine/reporter/target state), but the call shape is
/// identical.
///
/// Native callbacks contain C++ exceptions below Rust; every callback
/// returns zero on success or its exact nonzero captured failure.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherBootLeaf {
    // -- main() --
    /// `ArgC`, the live argument count (2469).
    ArgCount,
    /// `BootstrapHP2Paths(ArgC, ArgV)`; output is `Bootstrap.Installed` (2486).
    BootstrapPaths,
    /// `ShouldRunNativeLauncher(ArgC, ArgV)` (2487, 2515).
    ShouldRunNativeLauncher,
    /// `CrashConfig.process_name = "hp2_game"; CrashConfig.log_file_base =
    /// "HarryPotter2.log"; hp2_rs_crash_reporter_install(&CrashConfig);` (2493-2496).
    InstallCrashReporter,
    /// `HP2DataDirectoryArgumentValue(ArgV[Index])` for the given `Index`
    /// (2500-2511); on a match, stores it as `OverrideRoot`/`HasOverride`
    /// and output is 1, else 0.
    DataDirectoryArgument,
    /// `ForwardedCommandLine(ArgC, ArgV)` stored as `ForwardedArguments`,
    /// with `EngineCommandLine = ForwardedArguments` (2513-2514).
    ForwardCommandLine,
    /// `RunNativeLauncherRound(ArgC, ArgV, HasOverride, OverrideRoot,
    /// Outcome)`; output is whether to continue (2550-2551).
    RunLauncherRound,
    /// `Outcome.EnginePrefix.Len()` (2554).
    EnginePrefixLen,
    /// `EngineCommandLine = Outcome.EnginePrefix + TEXT(" ") +
    /// ForwardedArguments;` (2555).
    BuildEngineCommandLineWithPrefix,

    // -- HP2BootAfterLauncher() --
    /// `GIsStarted = 1;` (2201).
    SetIsStarted1,
    /// `hp2_rs_core_unix_initialize(RAND_MAX); hp2_rs_crash_reporter_install(&CrashConfig);
    /// strncpy(GModule, ArgV[0], sizeof(GModule)-1); GModule[...]=0;`
    /// (2202-2209, `#if !_MSC_VER`, unconditional on this platform).
    PlatformStaticInit,
    /// `GIsGuarded = 1;` (2215).
    SetIsGuarded1,
    /// `InstallHP2NativeLookups();` (2218, `#if __STATIC_LINK`,
    /// unconditional on this platform).
    InstallNativeLookups,
    /// `appInit(TEXT("Game"), *EngineCommandLine, &Malloc, &Log, &Error,
    /// &Warn, &FileManager, FConfigCacheIni::Factory, 1);
    /// hp2_rs_log_startup();` (2222-2232), bundled since the log-startup
    /// call is an existing direct native forwarder with no decision of
    /// its own.
    AppInit,
    /// `RegisterHP2RuntimeClasses(); RegisterHP2ClientClasses();` (2239-2240).
    RegisterClasses,
    /// `SDL_SetHint(SDL_HINT_VIDEODRIVER, <platform string>);`
    /// (2251-2261), the platform chain selects the literal unconditionally.
    SetVideoHint,
    /// `SDL_Init(SDL_INIT_VIDEO | SDL_INIT_JOYSTICK)`; output is its
    /// status (2263).
    SdlInit,
    /// `debugf(NAME_Warning, TEXT("SDL_Init failed: %s"),
    /// *SdlStatusText(SDL_GetError()));` (2264).
    LogSdlInitFailure,
    /// `ParseParam(appCmdLine(), TEXT("NOFRONTEND"))` (80-83, used at 2266).
    QueryNoFrontendParam,
    /// `GFileManager->FileSize(*SplashName) >= 0` for the given candidate
    /// (2268-2271).
    SplashCandidateExists,
    /// `ShowSplash(*SplashName)` for the given candidate (2272).
    ShowSplashFile,
    /// `ParseParam(appCmdLine(), TEXT("LOG"))` (2277).
    QueryLogParam,
    /// `GLogHook = &StdoutEcho;` (2278).
    SetLogHookStdout,
    /// `ParseParam(appCmdLine(), TEXT("SERVER"))` (2286).
    QueryServerParam,
    /// `ParseParam(appCmdLine(), TEXT("LAZY"))` (2289).
    QueryLazyParam,
    /// `GIsServer = 1; GIsClient = <arg bit 0>; GIsEditor = 0;
    /// GIsScriptable = 1; GLazyLoad = <arg bit 1>;` (2285-2289); `arg`
    /// packs `is_client` in bit 0 and `lazy_load` in bit 1.
    SetContextFlags,
    /// `LifecycleGateReporter::IsEnabled()` (2294).
    LifecycleEnabled,
    /// `LifecycleReporter.reset(new LifecycleGateReporter);` (2295).
    CreateLifecycle,
    /// `WorldCollisionGateReporter::IsEnabled()` (2298).
    WorldCollisionFullEnabled,
    /// `WorldCollisionGateReporter::IsTouchEnabled()` (2299).
    WorldCollisionTouchEnabled,
    /// `WorldCollisionGateReporter::IsBumpEnabled()` (2300).
    WorldCollisionBumpEnabled,
    /// `appErrorf(TEXT("only one world collision gate protocol may be
    /// enabled"));` (2302-2303).
    ErrorWorldCollisionConflict,
    /// `WorldCollisionReporter.reset(new WorldCollisionGateReporter(
    /// <Protocol>));` (2304-2307); `arg` is the `WorldCollisionProtocol`.
    CreateWorldCollision,
    /// `StaticBspProbeReporter::IsEnabled()` (2309).
    StaticBspEnabled,
    /// `StaticBspProbe.reset(new StaticBspProbeReporter);` (2310).
    CreateStaticBsp,
    /// `Engine = InitEngine();`; output is 1 when non-null (2325).
    CreateEngine,
    /// `appErrorf(TEXT("Could not initialize the game engine"));` (2326-2327).
    ErrorEngineInitFailed,
    /// `FireTextureTraceTarget = Cast<UFireTexture>(StaticLoadObject(...));`
    /// output is 1 when non-null (2329-2330).
    ResolveFireTextureTarget,
    /// `appErrorf(TEXT("Could not load HPParticle.hp_fx.Fire1 for
    /// HP2_FIRE_TEXTURE_TRACE"));` (2331-2332).
    ErrorFireTextureLoadFailed,
    /// `FireTextureTraceTarget->GetLinkerIndex() != 886 ||
    /// appStricmp(FireTextureTraceTarget->GetPathName(),
    /// TEXT("HPParticle.hp_fx.Fire1")) != 0`; output is 1 when the
    /// provenance check fails (2333-2334).
    CheckFireTextureProvenance,
    /// `appErrorf(TEXT("HP2_FIRE_TEXTURE_TRACE target provenance is not
    /// HPParticle.hp_fx.Fire1 export 886"));` (2335-2336).
    ErrorFireTextureProvenance,
    /// `hp2_rs_launcher_fire_texture_set_target(FireTextureTraceTarget);`
    /// (2337), forwarding to the existing native call -- Rust never sees
    /// the pointer value.
    SetFireTextureTarget,
    /// `HP2FireTextureProbe(FireTextureTraceTarget);` (2340).
    RunFireTextureProbe,
    /// `debugf(TEXT("Entering main loop."));` (2353).
    LogEnteringMainLoop,
    /// `HideSplash();` (2354).
    HideSplashLeaf,
    /// `Parse(appCmdLine(), TEXT("TESTTICKS="), TestTicks);`; output is
    /// the parsed (or default zero) value (2358).
    ParseTestTicks,
    /// `Parse(appCmdLine(), TEXT("fixed-dt="), FixedDeltaSeconds);`;
    /// output is the parsed (or default zero) value (2359).
    ParseFixedDeltaSeconds,
    /// `appErrorf(TEXT("fixed-dt must be positive"));` (2360-2361).
    ErrorFixedDtNegative,
    /// Rust-owned graceful-exit request, read through the platform callback.
    IsRequestingExit,
    /// `MainLoop(Engine, Max(0, TestTicks), FixedDeltaSeconds,
    /// LifecycleReporter.get(), WorldCollisionReporter.get(),
    /// StaticBspProbe.get());`; `arg` is the already-clamped test tick
    /// count, `scalar` is the fixed delta (2404).
    RunMainLoop,
    /// `CleanUpOnExit(Engine);` (2407).
    CleanUpOnExitLeaf,
}

/// Native callbacks contain C++ exceptions below Rust; every callback
/// returns zero on success or its exact nonzero captured failure.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherBootServices {
    pub context: *mut std::ffi::c_void,
    pub call: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        Hp2LauncherBootLeaf,
        i32,
        f32,
        *mut Hp2LauncherBootValue,
    ) -> i32,
}

impl Hp2LauncherBootServices {
    fn call(&self, leaf: Hp2LauncherBootLeaf, arg: i32) -> Result<i32, i32> {
        let mut out = Hp2LauncherBootValue { integer: 0 };
        checked(unsafe { (self.call)(self.context, leaf, arg, 0.0, &mut out) })?;
        Ok(unsafe { out.integer })
    }

    fn scalar(&self, leaf: Hp2LauncherBootLeaf) -> Result<f32, i32> {
        let mut out = Hp2LauncherBootValue { scalar: 0.0 };
        checked(unsafe { (self.call)(self.context, leaf, 0, 0.0, &mut out) })?;
        Ok(unsafe { out.scalar })
    }

    fn run_main_loop(&self, test_ticks: i32, fixed_delta_seconds: f32) -> Result<(), i32> {
        let mut out = Hp2LauncherBootValue { integer: 0 };
        checked(unsafe {
            (self.call)(self.context, Hp2LauncherBootLeaf::RunMainLoop, test_ticks, fixed_delta_seconds, &mut out)
        })
    }
}

/// Replaces the free `FrontendSuppressed()` (80-83).
pub fn frontend_suppressed(no_frontend_param: bool) -> bool {
    no_frontend_param
}

/// One of the two splash image candidates `HP2BootAfterLauncher` tries,
/// in order (2268-2271).
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hp2LauncherSplashCandidate {
    Splash,
    Logo,
}

/// Replaces the splash candidate selection (2268-2271): `../Help/Splash.bmp`
/// if present, else `../Help/Logo.bmp` if present, else no splash at all.
/// `logo_exists` is never queried by the caller unless `splash_exists` is
/// false, matching the original's single mutually-exclusive `SplashName`.
pub fn select_splash_candidate(splash_exists: bool, logo_exists: bool) -> Option<Hp2LauncherSplashCandidate> {
    if splash_exists {
        Some(Hp2LauncherSplashCandidate::Splash)
    } else if logo_exists {
        Some(Hp2LauncherSplashCandidate::Logo)
    } else {
        None
    }
}

/// The world collision gate protocol a construction selects (2304-2307).
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherWorldCollisionProtocol {
    Bump,
    Touch,
    Full,
}

/// Replaces the world collision gate validation and protocol selection
/// (2296-2307): at most one of the three gates may be requested; `Err(())`
/// is the conflict `appErrorf`, `Ok(None)` is "no gate requested".
pub fn validate_world_collision_protocol(
    full: bool,
    touch: bool,
    bump: bool,
) -> Result<Option<Hp2LauncherWorldCollisionProtocol>, ()> {
    if (full && (touch || bump)) || (touch && bump) {
        return Err(());
    }
    if bump {
        Ok(Some(Hp2LauncherWorldCollisionProtocol::Bump))
    } else if touch {
        Ok(Some(Hp2LauncherWorldCollisionProtocol::Touch))
    } else if full {
        Ok(Some(Hp2LauncherWorldCollisionProtocol::Full))
    } else {
        Ok(None)
    }
}

fn pack_context_flags(is_client: bool, lazy_load: bool) -> i32 {
    (is_client as i32) | ((lazy_load as i32) << 1)
}

/// Replaces `main()`'s full body up to its tail call into
/// `HP2BootAfterLauncher` (2469-2553). `Ok(None)` signals that native
/// should now call `HP2BootAfterLauncher` (2554); `Ok(Some(code))` is
/// `main()`'s own exit code, returned without ever reaching that call.
/// # Safety
/// `services` satisfies the checked callback contract described on
/// `Hp2LauncherBootServices`.
pub unsafe fn run_main(services: &Hp2LauncherBootServices) -> Result<Option<i32>, i32> {
    use Hp2LauncherBootLeaf::*;

    let installed = services.call(BootstrapPaths, 0)? != 0;
    if !installed && services.call(ShouldRunNativeLauncher, 0)? == 0 {
        return Ok(Some(1));
    }

    services.call(InstallCrashReporter, 0)?;

    let arg_count = services.call(ArgCount, 0)?;
    for index in 1..arg_count {
        if services.call(DataDirectoryArgument, index)? != 0 {
            break;
        }
    }

    services.call(ForwardCommandLine, 0)?;
    let show_launcher = services.call(ShouldRunNativeLauncher, 0)? != 0;

    if show_launcher {
        if services.call(RunLauncherRound, 0)? == 0 {
            return Ok(Some(0));
        }
        if services.call(EnginePrefixLen, 0)? > 0 {
            services.call(BuildEngineCommandLineWithPrefix, 0)?;
        }
    }

    Ok(None)
}

/// Replaces `HP2BootAfterLauncher`'s pre-`try` prologue: fire-texture's
/// conditional construction, `GIsStarted = 1`, and the POSIX-only crash
/// reporter re-install/`GModule` stamp all ran before the native `try`
/// began in the original engine (`git show 9e65d0a`, lines 5818-5845), so a
/// throw there must escape uncaught exactly as before rather than being
/// deferred until `run_after_launcher`'s own try-boundary runs
/// `Frame.Complete`.
/// # Safety
/// `services` satisfies the checked callback contract described on
/// `Hp2LauncherBootServices`.
pub unsafe fn run_boot_pre_try(services: &Hp2LauncherBootServices) -> Result<(), i32> {
    use Hp2LauncherBootLeaf::*;

    if fire_texture::is_enabled() {
        unsafe { fire_texture::create() };
    }

    services.call(SetIsStarted1, 0)?;
    services.call(PlatformStaticInit, 0)?;

    Ok(())
}

/// Replaces `HP2BootAfterLauncher`'s try-guarded body, excluding the
/// pre-`try` prologue (`run_boot_pre_try` above) and the `FinishProcess`
/// call (`finish` below).
/// # Safety
/// `services` satisfies the checked callback contract described on
/// `Hp2LauncherBootServices`.
pub unsafe fn run_after_launcher(services: &Hp2LauncherBootServices) -> Result<i32, i32> {
    use Hp2LauncherBootLeaf::*;

    let fire_texture_active = fire_texture::is_enabled();

    services.call(SetIsGuarded1, 0)?;
    services.call(InstallNativeLookups, 0)?;
    services.call(AppInit, 0)?;
    services.call(RegisterClasses, 0)?;
    services.call(SetVideoHint, 0)?;
    if services.call(SdlInit, 0)? != 0 {
        services.call(LogSdlInitFailure, 0)?;
    }

    if !frontend_suppressed(services.call(QueryNoFrontendParam, 0)? != 0) {
        let splash_exists = services.call(SplashCandidateExists, Hp2LauncherSplashCandidate::Splash as i32)? != 0;
        let logo_exists = if splash_exists {
            false
        } else {
            services.call(SplashCandidateExists, Hp2LauncherSplashCandidate::Logo as i32)? != 0
        };
        if let Some(candidate) = select_splash_candidate(splash_exists, logo_exists) {
            services.call(ShowSplashFile, candidate as i32)?;
        }
    }

    if services.call(QueryLogParam, 0)? != 0 {
        services.call(SetLogHookStdout, 0)?;
    }
    let is_client = services.call(QueryServerParam, 0)? == 0;
    let lazy_load = !is_client || services.call(QueryLazyParam, 0)? != 0;
    services.call(SetContextFlags, pack_context_flags(is_client, lazy_load))?;

    if services.call(LifecycleEnabled, 0)? != 0 {
        services.call(CreateLifecycle, 0)?;
    }

    let full = services.call(WorldCollisionFullEnabled, 0)? != 0;
    let touch = services.call(WorldCollisionTouchEnabled, 0)? != 0;
    let bump = services.call(WorldCollisionBumpEnabled, 0)? != 0;
    match validate_world_collision_protocol(full, touch, bump) {
        Err(()) => {
            services.call(ErrorWorldCollisionConflict, 0)?;
        }
        Ok(Some(protocol)) => {
            services.call(CreateWorldCollision, protocol as i32)?;
        }
        Ok(None) => {}
    }

    if services.call(StaticBspEnabled, 0)? != 0 {
        services.call(CreateStaticBsp, 0)?;
    }

    unsafe {
        shadow_admission::create();
        actor_transition_ledger::create();
        creature_generator::create();
        global_tick::create();
    }

    if services.call(CreateEngine, 0)? == 0 {
        services.call(ErrorEngineInitFailed, 0)?;
    }

    if fire_texture_active {
        if services.call(ResolveFireTextureTarget, 0)? == 0 {
            services.call(ErrorFireTextureLoadFailed, 0)?;
        }
        if services.call(CheckFireTextureProvenance, 0)? != 0 {
            services.call(ErrorFireTextureProvenance, 0)?;
        }
        services.call(SetFireTextureTarget, 0)?;
        services.call(RunFireTextureProbe, 0)?;
        unsafe {
            fire_texture::flush();
            fire_texture::reset();
        }
    }

    services.call(LogEnteringMainLoop, 0)?;
    services.call(HideSplashLeaf, 0)?;

    let test_ticks = services.call(ParseTestTicks, 0)?;
    let fixed_delta_seconds = services.scalar(ParseFixedDeltaSeconds)?;
    if fixed_delta_seconds < 0.0 {
        services.call(ErrorFixedDtNegative, 0)?;
    }

    if services.call(IsRequestingExit, 0)? == 0 {
        services.run_main_loop(test_ticks.max(0), fixed_delta_seconds)?;
    }

    unsafe {
        shadow_admission::reset();
        actor_transition_ledger::reset();
        creature_generator::reset();
        global_tick::reset();
    }
    services.call(CleanUpOnExitLeaf, 0)?;

    Ok(0)
}

/// Replaces `FinishProcess`'s exit-code decision (2157-2184): teardown
/// (`HideSplash(); appExit(); SDL_Quit();`, run by native unconditionally,
/// even when the guarded region above threw) failing always forces exit
/// code 1, overriding whatever `ExitCode` the guarded region produced.
pub fn finish(teardown_failed: bool, exit_code: i32) -> i32 {
    if teardown_failed { 1 } else { exit_code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::ffi::c_void;

    #[test]
    fn frontend_suppressed_is_identity() {
        assert!(frontend_suppressed(true));
        assert!(!frontend_suppressed(false));
    }

    #[test]
    fn splash_candidate_prefers_splash_then_logo_then_none() {
        assert_eq!(select_splash_candidate(true, true), Some(Hp2LauncherSplashCandidate::Splash));
        assert_eq!(select_splash_candidate(true, false), Some(Hp2LauncherSplashCandidate::Splash));
        assert_eq!(select_splash_candidate(false, true), Some(Hp2LauncherSplashCandidate::Logo));
        assert_eq!(select_splash_candidate(false, false), None);
    }

    #[test]
    fn world_collision_protocol_rejects_full_with_touch_or_bump() {
        assert!(validate_world_collision_protocol(true, true, false).is_err());
        assert!(validate_world_collision_protocol(true, false, true).is_err());
    }

    #[test]
    fn world_collision_protocol_rejects_touch_with_bump() {
        assert!(validate_world_collision_protocol(false, true, true).is_err());
    }

    #[test]
    fn world_collision_protocol_selects_bump_over_touch_over_full() {
        assert!(matches!(
            validate_world_collision_protocol(true, false, true),
            Err(())
        ));
        assert!(matches!(
            validate_world_collision_protocol(false, false, true).unwrap(),
            Some(Hp2LauncherWorldCollisionProtocol::Bump)
        ));
        assert!(matches!(
            validate_world_collision_protocol(false, true, false).unwrap(),
            Some(Hp2LauncherWorldCollisionProtocol::Touch)
        ));
        assert!(matches!(
            validate_world_collision_protocol(true, false, false).unwrap(),
            Some(Hp2LauncherWorldCollisionProtocol::Full)
        ));
    }

    #[test]
    fn world_collision_protocol_none_when_nothing_requested() {
        assert!(matches!(validate_world_collision_protocol(false, false, false), Ok(None)));
    }

    #[test]
    fn finish_forces_exit_code_one_when_teardown_failed() {
        assert_eq!(finish(true, 0), 1);
        assert_eq!(finish(true, 1), 1);
        assert_eq!(finish(false, 0), 0);
        assert_eq!(finish(false, 1), 1);
        assert_eq!(finish(false, 42), 42);
    }

    fn leaf_name(leaf: Hp2LauncherBootLeaf) -> &'static str {
        use Hp2LauncherBootLeaf::*;
        match leaf {
            ArgCount => "ArgCount",
            BootstrapPaths => "BootstrapPaths",
            ShouldRunNativeLauncher => "ShouldRunNativeLauncher",
            InstallCrashReporter => "InstallCrashReporter",
            DataDirectoryArgument => "DataDirectoryArgument",
            ForwardCommandLine => "ForwardCommandLine",
            RunLauncherRound => "RunLauncherRound",
            EnginePrefixLen => "EnginePrefixLen",
            BuildEngineCommandLineWithPrefix => "BuildEngineCommandLineWithPrefix",
            SetIsStarted1 => "SetIsStarted1",
            PlatformStaticInit => "PlatformStaticInit",
            SetIsGuarded1 => "SetIsGuarded1",
            InstallNativeLookups => "InstallNativeLookups",
            AppInit => "AppInit",
            RegisterClasses => "RegisterClasses",
            SetVideoHint => "SetVideoHint",
            SdlInit => "SdlInit",
            LogSdlInitFailure => "LogSdlInitFailure",
            QueryNoFrontendParam => "QueryNoFrontendParam",
            SplashCandidateExists => "SplashCandidateExists",
            ShowSplashFile => "ShowSplashFile",
            QueryLogParam => "QueryLogParam",
            SetLogHookStdout => "SetLogHookStdout",
            QueryServerParam => "QueryServerParam",
            QueryLazyParam => "QueryLazyParam",
            SetContextFlags => "SetContextFlags",
            LifecycleEnabled => "LifecycleEnabled",
            CreateLifecycle => "CreateLifecycle",
            WorldCollisionFullEnabled => "WorldCollisionFullEnabled",
            WorldCollisionTouchEnabled => "WorldCollisionTouchEnabled",
            WorldCollisionBumpEnabled => "WorldCollisionBumpEnabled",
            ErrorWorldCollisionConflict => "ErrorWorldCollisionConflict",
            CreateWorldCollision => "CreateWorldCollision",
            StaticBspEnabled => "StaticBspEnabled",
            CreateStaticBsp => "CreateStaticBsp",
            CreateEngine => "CreateEngine",
            ErrorEngineInitFailed => "ErrorEngineInitFailed",
            ResolveFireTextureTarget => "ResolveFireTextureTarget",
            ErrorFireTextureLoadFailed => "ErrorFireTextureLoadFailed",
            CheckFireTextureProvenance => "CheckFireTextureProvenance",
            ErrorFireTextureProvenance => "ErrorFireTextureProvenance",
            SetFireTextureTarget => "SetFireTextureTarget",
            RunFireTextureProbe => "RunFireTextureProbe",
            LogEnteringMainLoop => "LogEnteringMainLoop",
            HideSplashLeaf => "HideSplashLeaf",
            ParseTestTicks => "ParseTestTicks",
            ParseFixedDeltaSeconds => "ParseFixedDeltaSeconds",
            ErrorFixedDtNegative => "ErrorFixedDtNegative",
            IsRequestingExit => "IsRequestingExit",
            RunMainLoop => "RunMainLoop",
            CleanUpOnExitLeaf => "CleanUpOnExitLeaf",
        }
    }

    thread_local! {
        static CALLS: RefCell<Vec<(&'static str, i32)>> = const { RefCell::new(Vec::new()) };
        static SCRIPT: RefCell<std::collections::HashMap<&'static str, i32>> = RefCell::new(std::collections::HashMap::new());
    }

    unsafe extern "C" fn fake_call(
        _ctx: *mut c_void,
        leaf: Hp2LauncherBootLeaf,
        arg: i32,
        _scalar: f32,
        out: *mut Hp2LauncherBootValue,
    ) -> i32 {
        let name = leaf_name(leaf);
        CALLS.with(|c| c.borrow_mut().push((name, arg)));
        let value = SCRIPT.with(|s| s.borrow().get(name).copied().unwrap_or(0));
        unsafe { (*out).integer = value };
        0
    }

    fn run_with_script(script: &[(&'static str, i32)]) -> (Result<Option<i32>, i32>, Vec<(&'static str, i32)>) {
        CALLS.with(|c| c.borrow_mut().clear());
        SCRIPT.with(|s| {
            let mut s = s.borrow_mut();
            s.clear();
            for (k, v) in script {
                s.insert(*k, *v);
            }
        });
        let services = Hp2LauncherBootServices { context: std::ptr::null_mut(), call: fake_call };
        let result = unsafe { run_main(&services) };
        (result, CALLS.with(|c| c.borrow().clone()))
    }

    #[test]
    fn main_returns_one_when_bootstrap_absent_and_no_native_launcher() {
        let (result, calls) =
            run_with_script(&[("BootstrapPaths", 0), ("ShouldRunNativeLauncher", 0)]);
        assert_eq!(result, Ok(Some(1)));
        assert_eq!(calls, vec![("BootstrapPaths", 0), ("ShouldRunNativeLauncher", 0)]);
    }

    #[test]
    fn main_returns_zero_when_launcher_round_declines_to_continue() {
        let (result, calls) = run_with_script(&[
            ("BootstrapPaths", 1),
            ("ShouldRunNativeLauncher", 1),
            ("ArgCount", 1),
            ("RunLauncherRound", 0),
        ]);
        assert_eq!(result, Ok(Some(0)));
        assert_eq!(
            calls,
            vec![
                ("BootstrapPaths", 0),
                ("InstallCrashReporter", 0),
                ("ArgCount", 0),
                ("ForwardCommandLine", 0),
                ("ShouldRunNativeLauncher", 0),
                ("RunLauncherRound", 0),
            ]
        );
    }

    #[test]
    fn main_scans_every_argument_for_a_data_directory_override_until_found() {
        let (_result, calls) = run_with_script(&[
            ("BootstrapPaths", 1),
            ("ShouldRunNativeLauncher", 0),
            ("ArgCount", 4),
            ("DataDirectoryArgument", 0),
        ]);
        let indices: Vec<i32> =
            calls.iter().filter(|(n, _)| *n == "DataDirectoryArgument").map(|(_, a)| *a).collect();
        assert_eq!(indices, vec![1, 2, 3]);
    }

    #[test]
    fn main_stops_scanning_once_a_data_directory_override_is_found() {
        CALLS.with(|c| c.borrow_mut().clear());
        SCRIPT.with(|s| {
            let mut s = s.borrow_mut();
            s.clear();
            s.insert("BootstrapPaths", 1);
            s.insert("ShouldRunNativeLauncher", 0);
            s.insert("ArgCount", 5);
        });
        // DataDirectoryArgument returns 1 (found) on the second call only.
        thread_local! {
            static HITS: RefCell<i32> = const { RefCell::new(0) };
        }
        unsafe extern "C" fn fake_call_with_hit(
            _ctx: *mut c_void,
            leaf: Hp2LauncherBootLeaf,
            arg: i32,
            _scalar: f32,
            out: *mut Hp2LauncherBootValue,
        ) -> i32 {
            let name = leaf_name(leaf);
            CALLS.with(|c| c.borrow_mut().push((name, arg)));
            let value = if name == "DataDirectoryArgument" {
                HITS.with(|h| {
                    let mut h = h.borrow_mut();
                    *h += 1;
                    (*h == 2) as i32
                })
            } else {
                SCRIPT.with(|s| s.borrow().get(name).copied().unwrap_or(0))
            };
            unsafe { (*out).integer = value };
            0
        }
        let services = Hp2LauncherBootServices { context: std::ptr::null_mut(), call: fake_call_with_hit };
        let _ = unsafe { run_main(&services) };
        let indices: Vec<i32> = CALLS.with(|c| {
            c.borrow().iter().filter(|(n, _)| *n == "DataDirectoryArgument").map(|(_, a)| *a).collect()
        });
        assert_eq!(indices, vec![1, 2]);
    }

    #[test]
    fn main_builds_prefixed_engine_command_line_only_when_prefix_is_non_empty() {
        let (_result, calls) = run_with_script(&[
            ("BootstrapPaths", 1),
            ("ShouldRunNativeLauncher", 1),
            ("ArgCount", 1),
            ("RunLauncherRound", 1),
            ("EnginePrefixLen", 0),
        ]);
        assert!(!calls.iter().any(|(n, _)| *n == "BuildEngineCommandLineWithPrefix"));

        let (_result, calls) = run_with_script(&[
            ("BootstrapPaths", 1),
            ("ShouldRunNativeLauncher", 1),
            ("ArgCount", 1),
            ("RunLauncherRound", 1),
            ("EnginePrefixLen", 3),
        ]);
        assert!(calls.iter().any(|(n, _)| *n == "BuildEngineCommandLineWithPrefix"));
    }

    unsafe extern "C" fn fake_call_app_init_fails(
        _ctx: *mut c_void,
        leaf: Hp2LauncherBootLeaf,
        arg: i32,
        _scalar: f32,
        out: *mut Hp2LauncherBootValue,
    ) -> i32 {
        let name = leaf_name(leaf);
        CALLS.with(|c| c.borrow_mut().push((name, arg)));
        if name == "AppInit" {
            return 7;
        }
        let value = SCRIPT.with(|s| s.borrow().get(name).copied().unwrap_or(0));
        unsafe { (*out).integer = value };
        0
    }

    fn run_after_launcher_with_script(
        script: &[(&'static str, i32)],
    ) -> (Result<i32, i32>, Vec<(&'static str, i32)>) {
        CALLS.with(|c| c.borrow_mut().clear());
        SCRIPT.with(|s| {
            let mut s = s.borrow_mut();
            s.clear();
            for (k, v) in script {
                s.insert(*k, *v);
            }
        });
        let services = Hp2LauncherBootServices { context: std::ptr::null_mut(), call: fake_call };
        let result = unsafe { run_after_launcher(&services) };
        (result, CALLS.with(|c| c.borrow().clone()))
    }

    #[test]
    fn run_boot_pre_try_runs_set_is_started_then_platform_static_init_in_order() {
        CALLS.with(|c| c.borrow_mut().clear());
        SCRIPT.with(|s| s.borrow_mut().clear());
        let services = Hp2LauncherBootServices { context: std::ptr::null_mut(), call: fake_call };
        let result = unsafe { run_boot_pre_try(&services) };
        assert_eq!(result, Ok(()));
        let calls = CALLS.with(|c| c.borrow().clone());
        assert_eq!(calls, vec![("SetIsStarted1", 0), ("PlatformStaticInit", 0)]);
    }

    #[test]
    fn run_after_launcher_runs_every_leaf_in_order_on_the_happy_path() {
        let (result, calls) = run_after_launcher_with_script(&[
            ("SdlInit", 0),
            ("QueryNoFrontendParam", 1),
            ("QueryLogParam", 0),
            ("QueryServerParam", 0),
            ("QueryLazyParam", 0),
            ("LifecycleEnabled", 0),
            ("WorldCollisionFullEnabled", 0),
            ("WorldCollisionTouchEnabled", 0),
            ("WorldCollisionBumpEnabled", 0),
            ("StaticBspEnabled", 0),
            ("CreateEngine", 1),
            ("ParseTestTicks", 5),
            ("IsRequestingExit", 0),
        ]);
        assert_eq!(result, Ok(0));
        assert_eq!(
            calls,
            vec![
                ("SetIsGuarded1", 0),
                ("InstallNativeLookups", 0),
                ("AppInit", 0),
                ("RegisterClasses", 0),
                ("SetVideoHint", 0),
                ("SdlInit", 0),
                ("QueryNoFrontendParam", 0),
                ("QueryLogParam", 0),
                ("QueryServerParam", 0),
                ("QueryLazyParam", 0),
                ("SetContextFlags", 1),
                ("LifecycleEnabled", 0),
                ("WorldCollisionFullEnabled", 0),
                ("WorldCollisionTouchEnabled", 0),
                ("WorldCollisionBumpEnabled", 0),
                ("StaticBspEnabled", 0),
                ("CreateEngine", 0),
                ("LogEnteringMainLoop", 0),
                ("HideSplashLeaf", 0),
                ("ParseTestTicks", 0),
                ("ParseFixedDeltaSeconds", 0),
                ("IsRequestingExit", 0),
                ("RunMainLoop", 5),
                ("CleanUpOnExitLeaf", 0),
            ]
        );
    }

    #[test]
    fn run_after_launcher_stops_immediately_on_a_failing_leaf() {
        CALLS.with(|c| c.borrow_mut().clear());
        SCRIPT.with(|s| s.borrow_mut().clear());
        let services = Hp2LauncherBootServices { context: std::ptr::null_mut(), call: fake_call_app_init_fails };
        let result = unsafe { run_after_launcher(&services) };
        assert_eq!(result, Err(7));
        let calls = CALLS.with(|c| c.borrow().clone());
        assert_eq!(
            calls,
            vec![("SetIsGuarded1", 0), ("InstallNativeLookups", 0), ("AppInit", 0)]
        );
    }
}
