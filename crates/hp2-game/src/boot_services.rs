use hp2_abi::engine::generated::UGameEngine;
use hp2_core::core_access::{CoreOptions, CoreProcessMode, RuntimeError, RuntimeResult};
use hp2_engine::EngineOwner;
use hp2_launcher::boot::{
    Hp2LauncherBootLeaf as Leaf, Hp2LauncherBootServices, Hp2LauncherBootValue,
};
use std::{ffi::c_void, fs, io::Write, path::PathBuf, ptr};

#[cfg(test)]
mod unwind_tests;

static LOG_TO_STDOUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) struct BootHost {
    pub(super) args: Vec<String>,
    pub(super) data_root: Option<PathBuf>,
    pub(super) user_root: Option<PathBuf>,
    pub(super) system_ini: Option<PathBuf>,
    pub(super) user_ini: Option<PathBuf>,
    pub(super) override_root: Option<String>,
    pub(super) forwarded: String,
    pub(super) engine_command_line: String,
    // Reporters must detach before their owner is dropped during unwinding.
    pub(super) reporters: Option<crate::reporters::RuntimeReporters>,
    pub(super) owner: Option<EngineOwner>,
    pub(super) engine: *mut UGameEngine,
    pub(super) splash: Option<hp2_platform::launcher_splash::SplashHandle>,
    pub(super) is_client: bool,
    pub(super) lazy_load: bool,
    pub(super) is_started: bool,
    pub(super) is_guarded: bool,
    pub(super) exit_requested: bool,
    pub(super) fixed_delta: f32,
    pub(super) test_ticks: i32,
    pub(super) first_error: Option<RuntimeError>,
}

impl BootHost {
    pub fn new(args: Vec<String>) -> Self {
        Self {
            args,
            data_root: None,
            user_root: None,
            system_ini: None,
            user_ini: None,
            forwarded: String::new(),
            engine_command_line: String::new(),
            override_root: None,
            owner: None,
            engine: ptr::null_mut(),
            reporters: None,
            splash: None,
            is_client: false,
            lazy_load: false,
            is_started: false,
            is_guarded: false,
            exit_requested: false,
            fixed_delta: 0.0,
            test_ticks: 0,
            first_error: None,
        }
    }

    pub fn services(&mut self) -> Hp2LauncherBootServices {
        Hp2LauncherBootServices {
            context: (self as *mut Self).cast(),
            call: Self::invoke,
        }
    }

    unsafe extern "C" fn invoke(
        context: *mut c_void,
        leaf: Leaf,
        arg: i32,
        scalar: f32,
        out: *mut Hp2LauncherBootValue,
    ) -> i32 {
        let host = unsafe { &mut *context.cast::<Self>() };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            host.call(leaf, arg, scalar, unsafe { &mut *out })
        }));
        match result {
            Ok(Ok(())) => 0,
            Ok(Err(error)) => {
                host.retain(error);
                1
            }
            Err(_) => {
                host.retain(RuntimeError::new("panic in Rust game boot service"));
                1
            }
        }
    }

    pub(super) fn retain(&mut self, error: RuntimeError) {
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }
    pub(super) fn fail_status(&mut self, status: i32) -> i32 {
        if let Some(error) = self.first_error.take() {
            eprintln!("hp2_game: {error}");
        } else {
            eprintln!("hp2_game: launcher service failed ({status})");
        }
        1
    }
    fn call(
        &mut self,
        leaf: Leaf,
        arg: i32,
        scalar: f32,
        out: &mut Hp2LauncherBootValue,
    ) -> RuntimeResult<()> {
        if let Some(value) = crate::fire_probe::boot_leaf(leaf, arg)? {
            out.integer = value;
            return Ok(());
        }
        if let Some(reporters) = self.reporters.as_mut() {
            if let Some(value) = reporters.boot_leaf(leaf, arg)? {
                out.integer = value;
                return Ok(());
            }
        }
        match leaf {
            Leaf::SetIsStarted1 => self.is_started = true,
            Leaf::ArgCount => out.integer = self.args.len() as i32,
            Leaf::BootstrapPaths => self.bootstrap_paths(out)?,
            Leaf::InstallCrashReporter => self.install_crash_reporter(),
            Leaf::DataDirectoryArgument => {
                if let Some(value) = self.args.get(arg as usize).and_then(|s| {
                    hp2_bootstrap_paths::bootstrap_paths::argument(s.as_bytes())
                        .map(|offset| &s[offset..])
                }) {
                    self.override_root = Some(value.to_owned());
                    out.integer = 1;
                }
            }
            Leaf::ForwardCommandLine => self.forward_command_line()?,
            Leaf::PlatformStaticInit => {
                unsafe {
                    hp2_core::unix_signal::initialize(i32::MAX as u32);
                }
                self.install_crash_reporter();
            }
            Leaf::SetIsGuarded1 => self.is_guarded = true,
            // EngineOwner construction installs the canonical native catalog and
            // class registry during appInit; these source callbacks verify that
            // those owner operations completed before their later use.
            Leaf::InstallNativeLookups => {
                if !self.is_started {
                    return Err(RuntimeError::new(
                        "Native lookups installed before platform startup",
                    ));
                }
            }
            Leaf::RegisterClasses => {
                if self.owner.is_none() {
                    return Err(RuntimeError::new(
                        "Core owner must be initialized before class registration",
                    ));
                }
            }
            Leaf::AppInit => {
                if !self.is_started || !self.is_guarded {
                    return Err(RuntimeError::new(
                        "appInit called outside the guarded boot phase",
                    ));
                }
                self.app_init()?;
            }
            Leaf::CreateEngine => {
                self.create_engine()?;
                out.integer = i32::from(!self.engine.is_null());
            }
            Leaf::ErrorEngineInitFailed => {
                return Err(RuntimeError::new("Could not initialize the game engine"));
            }
            Leaf::SetVideoHint => hp2_platform::launcher_display_modes::set_video_driver_hint(),
            Leaf::SdlInit => {
                out.integer = hp2_platform::launcher_boot_video::init_video_and_joystick()
            }
            Leaf::LogSdlInitFailure => self.log_sdl_failure()?,
            Leaf::QueryNoFrontendParam => out.integer = i32::from(self.has_param("NOFRONTEND")),
            Leaf::SplashCandidateExists => out.integer = i32::from(self.splash_path(arg).is_file()),
            Leaf::ShowSplashFile => self.show_splash(arg),
            Leaf::HideSplashLeaf => self.hide_splash(),
            Leaf::QueryLogParam => out.integer = i32::from(self.has_param("LOG")),
            Leaf::SetLogHookStdout => {
                LOG_TO_STDOUT.store(true, std::sync::atomic::Ordering::Relaxed)
            }
            Leaf::QueryServerParam => out.integer = i32::from(self.has_param("SERVER")),
            Leaf::QueryLazyParam => out.integer = i32::from(self.has_param("LAZY")),
            Leaf::SetContextFlags => {
                self.is_client = arg & 1 != 0;
                self.lazy_load = arg & 2 != 0;
                let owner = self.owner.as_ref().ok_or_else(|| {
                    RuntimeError::new("EngineOwner is unavailable while setting process flags")
                })?;
                owner.access().set_process_mode(CoreProcessMode {
                    is_client: self.is_client,
                    is_server: true,
                    is_editor: false,
                    is_scriptable: true,
                    lazy_load: self.lazy_load,
                });
            }
            Leaf::LifecycleEnabled
            | Leaf::CreateLifecycle
            | Leaf::WorldCollisionFullEnabled
            | Leaf::WorldCollisionTouchEnabled
            | Leaf::WorldCollisionBumpEnabled
            | Leaf::ErrorWorldCollisionConflict
            | Leaf::CreateWorldCollision
            | Leaf::StaticBspEnabled
            | Leaf::CreateStaticBsp => {
                return Err(RuntimeError::new(
                    "RuntimeReporters did not handle a reporter-owned boot operation",
                ));
            }
            Leaf::ResolveFireTextureTarget
            | Leaf::ErrorFireTextureLoadFailed
            | Leaf::CheckFireTextureProvenance
            | Leaf::ErrorFireTextureProvenance
            | Leaf::SetFireTextureTarget
            | Leaf::RunFireTextureProbe => {
                return Err(RuntimeError::new(
                    "FireProbe did not handle a fire reporter boot operation",
                ));
            }
            Leaf::LogEnteringMainLoop => {
                hp2_platform::input_pump::gameplay_gate::set_active();
                self.log_message("Entering main loop.")?;
            }
            Leaf::ParseTestTicks => {
                self.test_ticks = self.parse_integer_param("TESTTICKS=").unwrap_or(0);
                out.integer = self.test_ticks;
            }
            Leaf::ParseFixedDeltaSeconds => {
                self.fixed_delta = self.parse_float_param("fixed-dt=").unwrap_or(0.0);
                out.scalar = self.fixed_delta;
            }
            Leaf::ErrorFixedDtNegative => {
                return Err(RuntimeError::new("fixed-dt must be positive"));
            }
            Leaf::IsRequestingExit => {
                out.integer =
                    i32::from(self.exit_requested || hp2_core::unix_signal::exit_requested())
            }
            Leaf::RunMainLoop => crate::main_loop_services::run(self, arg, scalar)?,
            Leaf::CleanUpOnExitLeaf => self.clean_engine()?,
        }
        Ok(())
    }

    fn bootstrap_paths(&mut self, out: &mut Hp2LauncherBootValue) -> RuntimeResult<()> {
        let args = self.args.iter().map(|s| s.as_bytes()).collect::<Vec<_>>();
        let mut installed = None;
        let mut failure = None;
        let outcome = hp2_bootstrap_paths::bootstrap_paths::run(0, b"", b"", &args, |system, user| {
            match bootstrap_root_paths(system, user) {
                Ok(paths) => { installed = Some(paths); 1 }
                Err(error) => { failure = Some(error); -1 }
            }
        });
        if let Some(error) = failure { return Err(error); }
        if !outcome.success && !args.iter().any(|a| a.to_ascii_lowercase().starts_with(b"-datadir")) {
            show_first_run_help();
        }
        out.integer = i32::from(outcome.success);
        if outcome.success {
            let (data, user) = installed.ok_or_else(|| RuntimeError::new("bootstrap path installer did not publish roots"))?;
            self.data_root = Some(data);
            self.user_root = Some(user);
        }
        Ok(())
    }

    fn install_crash_reporter(&mut self) {
        #[cfg(unix)]
        {
            static PROCESS: &[u8] = b"hp2_game\0";
            static LOG: &[u8] = b"HarryPotter2.log\0";
            let config = hp2_platform::crash_reporter::Hp2CrashReporterConfig {
                process_name: PROCESS.as_ptr().cast(),
                log_file_base: LOG.as_ptr().cast(),
                watchdog_seconds: 0.0,
            };
            unsafe {
                hp2_platform::crash_reporter::install(&config);
            }
        }
    }

    fn app_init(&mut self) -> RuntimeResult<()> {
        let root = self
            .data_root
            .clone()
            .ok_or_else(|| RuntimeError::new("Game data root was not installed"))?;
        let paths = hp2_bootstrap_paths::bootstrap_paths::runtime_configuration_paths(
            &root,
            self.forwarded.as_bytes(),
        )
        .map_err(|e| RuntimeError::new(&e))?;
        self.system_ini = Some(paths.system_ini.clone());
        self.user_ini = Some(paths.user_ini.clone());
        self.user_root = Some(paths.user_root);
        let log_path = self
            .user_root
            .as_ref()
            .ok_or_else(|| RuntimeError::new("User root was not initialized"))?
            .join("HarryPotter2.log");
        let mut log = fs::File::create(log_path)
            .map_err(|e| RuntimeError::new(&format!("could not open game log: {e}")))?;
        log.write_all(&[0xff, 0xfe])
            .map_err(|e| RuntimeError::new(&format!("could not initialize game log: {e}")))?;
        let options = CoreOptions {
            data_root: root,
            system_ini: paths.system_ini,
            user_ini: paths.user_ini,
            command_line: self
                .engine_command_line
                .chars()
                .map(u32::from)
                .chain([0])
                .collect(),
            output: core_output,
            log_file: Some(log),
        };
        let owner =
            EngineOwner::new_with_fire_trace_observer(options, crate::reporters::fire_observer)?;
        owner.access().initialize_application_system()?;
        self.owner = Some(owner);
        apply_portable_config(
            self.owner
                .as_ref()
                .ok_or_else(|| RuntimeError::new("EngineOwner creation failed"))?,
        )?;
        self.reporters = Some(unsafe {
            crate::reporters::RuntimeReporters::new(
                self.owner.as_ref().unwrap() as *const EngineOwner
            )?
        });
        Ok(())
    }

    fn create_engine(&mut self) -> RuntimeResult<()> {
        let owner = self
            .owner
            .as_ref()
            .ok_or_else(|| RuntimeError::new("Engine owner is not initialized"))?;
        let access = owner.access();
        // Apply [Engine.Engine] Language before any package load. Retail data
        // ships localized archives (e.g. Sounds/AllDialog.USA_uax) that only
        // resolve when the active language matches the retail Default.ini's
        // Language=usa; without this the engine keeps the default "int" and
        // every localized voice archive is skipped (no character voices).
        {
            let section: Vec<u32> = "Engine.Engine".chars().map(u32::from).collect();
            let key: Vec<u32> = "Language".chars().map(u32::from).collect();
            if let Some(language) = access.config_get_units(&section, &key)? {
                access.set_language(&language)?;
            }
        }
        let name = "ini:Engine.Engine.GameEngine"
            .chars()
            .map(u32::from)
            .chain([0])
            .collect::<Vec<_>>();
        let class = unsafe {
            access.static_load_class(
                access.native_class("Engine.Engine")?,
                ptr::null_mut(),
                &name,
                1,
            )?
        };
        self.engine = unsafe {
            access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())?
        }
        .cast();
        unsafe {
            hp2_engine::game_engine_owner::init(access, self.engine)?;
        }
        self.reporters
            .as_mut()
            .ok_or_else(|| {
                RuntimeError::new("RuntimeReporters is unavailable after Engine construction")
            })?
            .bind_engine(self.engine.cast());
        Ok(())
    }

    fn forward_command_line(&mut self) -> RuntimeResult<()> {
        self.forwarded.clear();
        let services = hp2_launcher::command_policy::Hp2LauncherCommandServices {
            context: (self as *mut Self).cast(),
            invoke: Some(command_callback),
        };
        unsafe { hp2_launcher::command_policy::forward(&services, self.args.len() as i32) }
            .map_err(|_| RuntimeError::new("Could not forward game command-line arguments"))?;
        self.engine_command_line.clone_from(&self.forwarded);
        Ok(())
    }
    fn has_param(&self, parameter: &str) -> bool {
        let stream = self
            .engine_command_line
            .chars()
            .map(u32::from)
            .chain([0])
            .collect::<Vec<_>>();
        let param = parameter
            .chars()
            .map(u32::from)
            .chain([0])
            .collect::<Vec<_>>();
        unsafe { hp2_core::search::parse_param(stream.as_ptr().cast(), param.as_ptr().cast()) }
    }
    fn parse_integer_param(&self, parameter: &str) -> Option<i32> {
        self.parameter_value(parameter)?.parse().ok()
    }
    fn parse_float_param(&self, parameter: &str) -> Option<f32> {
        self.parameter_value(parameter)?.parse().ok()
    }
    fn parameter_value(&self, parameter: &str) -> Option<&str> {
        self.args.iter().skip(1).find_map(|argument| {
            let argument =
                argument.trim_start_matches(|character| character == '-' || character == '/');
            let (key, value) = argument.split_at_checked(parameter.len())?;
            key.eq_ignore_ascii_case(parameter).then_some(value)
        })
    }
    fn splash_path(&self, candidate: i32) -> PathBuf {
        let root = self
            .data_root
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new("."));
        root.join("..").join("Help").join(if candidate == 0 {
            "Splash.bmp"
        } else {
            "Logo.bmp"
        })
    }
    fn show_splash(&mut self, candidate: i32) {
        if let Ok(bytes) = fs::read(self.splash_path(candidate)) {
            self.splash = Some(hp2_platform::launcher_splash::show_splash(&bytes));
        }
    }
    fn hide_splash(&mut self) {
        if let Some(handle) = self.splash.take() {
            unsafe {
                hp2_platform::launcher_splash::hide_splash(handle);
            }
        }
    }
    fn clean_engine(&mut self) -> RuntimeResult<()> {
        let owner = self
            .owner
            .as_ref()
            .ok_or_else(|| RuntimeError::new("EngineOwner unavailable during shutdown"))?;
        owner.access().set_running(false);
        unsafe {
            hp2_engine::game_engine_owner::exit(owner.access(), self.engine)?;
        }
        owner.pre_exit()
    }
    pub fn end_loop_scope(&mut self) {
        if let Some(reporters) = self.reporters.as_mut() {
            reporters.end_loop_scope();
        }
    }
    fn log_message(&self, text: &str) -> RuntimeResult<()> {
        self.log_message_category("Log", text)
    }
    fn log_sdl_failure(&self) -> RuntimeResult<()> {
        let error = unsafe { hp2_platform::sdl_runtime::last_error() };
        let error = if error.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        };
        self.log_message_category("Warning", &format!("SDL_Init failed: {error}"))
    }
    fn log_message_category(&self, category_name: &str, text: &str) -> RuntimeResult<()> {
        let owner = self
            .owner
            .as_ref()
            .ok_or_else(|| RuntimeError::new("EngineOwner unavailable while logging"))?;
        let access = owner.access();
        let category = access.intern_str(category_name, hp2_core::name_owner::FindName::Add)?;
        let text = text.chars().map(u32::from).collect::<Vec<_>>();
        access.emit_log(category, &text)
    }
    pub fn finish(&mut self) -> bool {
        self.hide_splash();
        if let Some(reporters) = self.reporters.as_mut() {
            if let Err(error) = reporters.detach() {
                self.retain(error);
            }
        }
        self.reporters.take();
        let result = self.owner.take().map_or(Ok(()), EngineOwner::shutdown);
        if let Err(error) = result {
            self.retain(error);
        }
        hp2_platform::launcher_boot_video::quit();
        self.first_error.is_some()
    }
}

unsafe extern "C" fn command_callback(
    raw: *mut c_void,
    leaf: hp2_launcher::command_policy::Hp2LauncherCommandLeaf,
    index: i32,
    out: *mut i32,
) -> i32 {
    let host = unsafe { &mut *raw.cast::<BootHost>() };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        use hp2_launcher::command_policy::Hp2LauncherCommandLeaf as L;
        let value = match leaf {
            L::IsDataDirectory => i32::from(host.args.get(index as usize).is_some_and(|arg| {
                hp2_bootstrap_paths::bootstrap_paths::argument(arg.as_bytes()).is_some()
            })),
            L::ConvertArgument => i32::from(host.args.get(index as usize).is_some()),
            L::ReportInvalid => {
                host.retain(RuntimeError::new(
                    "Could not convert a command-line argument to TCHAR",
                ));
                0
            }
            L::CommandLineLength => host.forwarded.chars().count() as i32,
            L::AppendSeparator => {
                host.forwarded.push(' ');
                0
            }
            L::AppendArgument => {
                if let Some(argument) = host.args.get(index as usize) {
                    host.forwarded.push_str(argument);
                }
                0
            }
            L::ConvertStatus => 0,
        };
        unsafe {
            out.write(value);
        }
    }));
    match result {
        Ok(()) => 0,
        Err(_) => {
            host.retain(RuntimeError::new(
                "panic in command-line forwarding callback",
            ));
            1
        }
    }
}

fn core_output(
    access: hp2_core::core_access::CoreAccess<'_>,
    category: i32,
    text: &[u32],
) -> RuntimeResult<()> {
    let name = access.name_text(category)?;
    access.write_log_record(&name, text)?;
    if LOG_TO_STDOUT.load(std::sync::atomic::Ordering::Relaxed) {
        let line = text
            .iter()
            .copied()
            .take_while(|&unit| unit != 0)
            .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>();
        println!("{line}");
    }
    Ok(())
}
struct ConfigContext<'a> {
    access: hp2_core::core_access::CoreAccess<'a>,
    failure: Option<RuntimeError>,
}

fn config_key(key: hp2_launcher::boot_config::Hp2LauncherConfigKey) -> &'static str {
    use hp2_launcher::boot_config::Hp2LauncherConfigKey::*;
    match key {
        ViewportManager => "ViewportManager",
        GameRenderDevice => "GameRenderDevice",
        WindowedRenderDevice => "WindowedRenderDevice",
        AudioDevice => "AudioDevice",
    }
}

unsafe extern "C" fn config_get(
    raw: *mut c_void,
    key: hp2_launcher::boot_config::Hp2LauncherConfigKey,
    buffer: *mut u8,
    capacity: usize,
    length: *mut i32,
) -> i32 {
    let context = unsafe { &mut *raw.cast::<ConfigContext<'_>>() };
    let section = "Engine.Engine".chars().map(u32::from).collect::<Vec<_>>();
    let name = config_key(key).chars().map(u32::from).collect::<Vec<_>>();
    match context.access.config_get_units(&section, &name) {
        Ok(Some(value)) => {
            let value = String::from_iter(
                value
                    .into_iter()
                    .take_while(|&unit| unit != 0)
                    .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)),
            )
            .into_bytes();
            if value.len() > capacity {
                return 1;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(value.as_ptr(), buffer, value.len());
                length.write(value.len() as i32);
            }
            0
        }
        Ok(None) => {
            unsafe {
                length.write(-1);
            }
            0
        }
        Err(error) => {
            context.failure = Some(error);
            1
        }
    }
}

unsafe extern "C" fn config_set(
    raw: *mut c_void,
    key: hp2_launcher::boot_config::Hp2LauncherConfigKey,
    value: *const u8,
    length: usize,
) -> i32 {
    let context = unsafe { &mut *raw.cast::<ConfigContext<'_>>() };
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(value, length) }
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        context.failure = Some(RuntimeError::new(
            "Portable configuration value is not UTF-8",
        ));
        return 1;
    };
    let section = "Engine.Engine".chars().map(u32::from).collect::<Vec<_>>();
    let name = config_key(key).chars().map(u32::from).collect::<Vec<_>>();
    let value = text.chars().map(u32::from).collect::<Vec<_>>();
    match context.access.config_set_units(&section, &name, &value) {
        Ok(()) => 0,
        Err(error) => {
            context.failure = Some(error);
            1
        }
    }
}

unsafe extern "C" fn config_log(raw: *mut c_void, bytes: *const u8, length: usize) -> i32 {
    let context = unsafe { &mut *raw.cast::<ConfigContext<'_>>() };
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, length) }
    };
    let text = bytes.iter().copied().map(u32::from).collect::<Vec<_>>();
    match context
        .access
        .intern_str("Init", hp2_core::name_owner::FindName::Add)
        .and_then(|category| context.access.emit_log(category, &text))
    {
        Ok(()) => 0,
        Err(error) => {
            context.failure = Some(error);
            1
        }
    }
}

fn apply_portable_config(owner: &EngineOwner) -> RuntimeResult<()> {
    let access = owner.access();
    let mut context = ConfigContext {
        access,
        failure: None,
    };
    let services = hp2_launcher::boot_config::Hp2LauncherConfigServices {
        context: (&mut context as *mut ConfigContext<'_>).cast(),
        get_string: config_get,
        set_string: config_set,
        log_init: config_log,
    };
    match unsafe { hp2_launcher::boot_config::apply(&services) } {
        Ok(()) => context.failure.map_or(Ok(()), Err),
        Err(_) => Err(context
            .failure
            .unwrap_or_else(|| RuntimeError::new("ApplyPortableConfig failed"))),
    }
}

fn bootstrap_root_paths(system: &[u8], user: &[u8]) -> RuntimeResult<(PathBuf, PathBuf)> {
    use std::os::unix::ffi::OsStrExt;
    // The native installer publishes BaseDir=.../System/, then appUserDir().
    publish_app_paths(system, user)?;
    // Outcome.text[0] is intentionally unused for installation operations.
    let system = std::path::Path::new(std::ffi::OsStr::from_bytes(system.strip_suffix(&[0]).unwrap_or(system)));
    let data = system.parent().ok_or_else(|| RuntimeError::new("bootstrap System directory has no data parent"))?;
    let user = std::ffi::OsStr::from_bytes(user.strip_suffix(&[0]).unwrap_or(user));
    Ok((data.to_path_buf(), PathBuf::from(user)))
}

fn publish_app_paths(system: &[u8], user: &[u8]) -> RuntimeResult<()> {
    let services = hp2_core::unix_text::Hp2UnixTextServices {
        opaque: ptr::null_mut(),
        allocate: bootstrap_unix_allocate,
        fatal: bootstrap_unix_fatal,
    };
    let system_text = unsafe { hp2_core::unix_text::from_utf8(&services, system.as_ptr()) }
        .map_err(|_| RuntimeError::new("appSetBaseDir path conversion failed"))?;
    let user_text = match unsafe { hp2_core::unix_text::from_utf8(&services, user.as_ptr()) } {
        Ok(text) => text,
        Err(_) => {
            unsafe { libc::free(system_text.cast()) };
            return Err(RuntimeError::new("appSetUserDir path conversion failed"));
        }
    };
    let base_result = unsafe { hp2_core::unix_identity::set_base_dir(&services, system_text) };
    unsafe { libc::free(system_text.cast()) };
    if base_result.is_err() {
        unsafe { libc::free(user_text.cast()) };
        return Err(RuntimeError::new("appSetBaseDir failed"));
    }
    let user_result = unsafe { hp2_core::unix_identity::set_user_dir(&services, user_text) };
    unsafe { libc::free(user_text.cast()) };
    user_result.map_err(|_| RuntimeError::new("appSetUserDir failed"))
}

unsafe extern "C" fn bootstrap_unix_allocate(
    _opaque: *mut c_void,
    size: usize,
    out: *mut *mut c_void,
) -> i32 {
    if out.is_null() { return 1; }
    let allocation = unsafe { libc::malloc(size.max(1)) };
    if allocation.is_null() {
        1
    } else {
        unsafe { out.write(allocation) };
        0
    }
}

unsafe extern "C" fn bootstrap_unix_fatal(
    _opaque: *mut c_void,
    _failure: hp2_core::unix_text::Hp2UnixTextFailure,
) -> i32 {
    1
}

/// Without the launcher, a player who opens the app before importing their game sees nothing
/// (the reason goes to stderr). From the macOS app bundle, explain the one import command.
fn show_first_run_help() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(dir) = exe.parent() else { return };
    if !dir.to_string_lossy().contains(".app/Contents/MacOS") {
        return;
    }
    let importer = dir.join("hp2_import");
    let output = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join("Library/Application Support/Harry Potter 2/Data/Retail");
    let message = format!(
        "This game needs your own copy of Harry Potter and the Chamber of Secrets (PC).\n\n\
         Open Terminal and run:\n\n\"{}\" --retail-root \"/path/to/your game folder\" --profile retail-only --output \"{}\"\n\n\
         The game folder is the one containing Maps, System and Textures (for a .zip or .7z of it, \
         use --archive instead of --retail-root). Then open the game again.",
        importer.display(),
        output.display()
    );
    hp2_platform::message_box::show_simple_error(b"Import your game first", message.as_bytes());
}
