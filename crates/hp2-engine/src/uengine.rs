//! Port of `HarryPotter2/Unreal/Engine/Src/UnEngine.cpp` (`UEngine`,
//! `UServerCommandlet::Main`): audio init, engine init and exit, destroy,
//! flush, tick rate, command exec, key and input event routing, and the
//! dedicated-server main loop.
//!
//! The C++ shells in `UnEngine.cpp` keep the UObject boilerplate
//! (`IMPLEMENT_CLASS`, `StaticConstructor`, `NAMES_ONLY` expansion,
//! `Serialize`, the empty `SetProgress`/`ChallengeResponse`, and
//! `Super::Destroy()`) plus the `guard`/`unguard` wrappers, and forward here.
//! Engine state and Core helpers go through [`Hp2UEngineServices`], filled by
//! `Hp2UEngineServices.cpp`. Engine fields are read and written through the
//! table, so this module reads no engine layout.
//!
//! `TCHAR` is `wchar_t` = 4 bytes on this build (`HP2_HOST_WCHAR_TCHAR`), so
//! text buffers are `u32` units throughout. `FTime` crosses as its raw 64-bit
//! representation (`i64`); only `FTime::operator-` is evaluated, via
//! [`Hp2UEngineServices::ftime_sub`].
//!
//! Divergence: a C++ exception (`appErrorf`) thrown from a service unwinds into
//! a Rust `extern "C"` frame and aborts, where the original propagated it to
//! `appMain`. This matches the existing console and input ports; Init and Exec
//! keep the same exposure. The dedicated-server loop runs in Rust; the
//! `guard(UpdateWorld)` and `guard(EnforceTickRate)` labels live in their C++
//! adapters.

use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

/// `IK_Y` / `IK_N` (`EngineClasses.h:220`, `:209`).
const IK_N: i32 = 78;
const IK_Y: i32 = 89;
/// `EInputAction::IST_Release` (`EngineClasses.h`).
const IST_RELEASE: i32 = 3;
/// `GIsEditor ? 30 : 0` in `UEngine::GetMaxTickRate`.
const EDITOR_MAX_TICK_RATE: f32 = 30.0;
/// `GEngineMem.Init` size (`UnEngine.cpp:106`).
const ENGINE_MEM_BYTES: i32 = 65536;
/// `GCache.Init` alignment (`UnEngine.cpp:107`).
const CACHE_ALIGN: i32 = 4096;
/// `Clamp(..., 1, 1024)` bounds on `CacheSizeMegs` (`UnEngine.cpp:107`).
const CACHE_MEGS_MIN: i32 = 1;
const CACHE_MEGS_MAX: i32 = 1024;
/// `TCHAR Temp[256]` in `UServerCommandlet::Main`.
const LANGUAGE_BUF_LEN: i32 = 256;

/// `static TCHAR YesKey=0, NoKey=0;//oldver` (`UnEngine.cpp:90`). Process-wide,
/// as in C++.
static YES_KEY: AtomicU32 = AtomicU32::new(0);
static NO_KEY: AtomicU32 = AtomicU32::new(0);

/// Engine calls made by the engine port, in the C++ adapter table
/// `GHp2UEngineServices` (`Engine/Src/Hp2UEngineServices.cpp`). Pointer
/// arguments are `void*` so the table stays a plain C ABI. `*const u32`
/// arguments are NUL-terminated `TCHAR` strings. `FTime` values are `i64`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2UEngineServices {
    /// `GIsClient`. Returns `UBOOL`.
    pub is_client: unsafe extern "C" fn() -> i32,
    /// `GIsEditor`. Returns `UBOOL`.
    pub is_editor: unsafe extern "C" fn() -> i32,
    /// `GIsRunning`. Returns `UBOOL`.
    pub is_running: unsafe extern "C" fn() -> i32,
    /// `GIsRunning = v`.
    pub set_is_running: unsafe extern "C" fn(v: i32),
    /// `GIsRequestingExit`. Returns `UBOOL`.
    pub is_requesting_exit: unsafe extern "C" fn() -> i32,
    /// `GLog`, as `FOutputDevice*`.
    pub glog: unsafe extern "C" fn() -> *mut c_void,

    /// `UEngine::CacheSizeMegs`.
    pub cache_size_megs: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::UseSound`. Returns `UBOOL`.
    pub use_sound: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::Audio`.
    pub audio: unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void,
    /// `UEngine::Audio = v`.
    pub set_audio: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void),
    /// `UEngine::Render`.
    pub render: unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void,
    /// `UEngine::Render = v`.
    pub set_render: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void),
    /// `UEngine::Client`.
    pub client: unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void,
    /// `UEngine::Client = v`.
    pub set_client: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void),
    /// `UEngine::CurrentTickRate = v`.
    pub set_current_tick_rate: unsafe extern "C" fn(engine: *mut c_void, v: f32),
    /// `UEngine::Cylinder = new UCylinder`.
    pub create_cylinder: unsafe extern "C" fn(engine: *mut c_void),
    /// `UEngine::OCylinder = new UOrientedCylinder`.
    pub create_ocylinder: unsafe extern "C" fn(engine: *mut c_void),
    /// `UEngine::OvalCylinder = new UAlignedOvalCylinder`.
    pub create_oval_cylinder: unsafe extern "C" fn(engine: *mut c_void),
    /// `UEngine::OOvalCylinder = new UOrientedOvalCylinder`.
    pub create_ooval_cylinder: unsafe extern "C" fn(engine: *mut c_void),
    /// `UEngine::Box = new UBox`.
    pub create_box: unsafe extern "C" fn(engine: *mut c_void),
    /// `AddToRoot()` on the engine object.
    pub add_to_root: unsafe extern "C" fn(engine: *mut c_void),
    /// `RemoveFromRoot()` on the engine object.
    pub remove_from_root: unsafe extern "C" fn(engine: *mut c_void),
    /// Virtual `engine->Init()`.
    pub engine_init: unsafe extern "C" fn(engine: *mut c_void),
    /// Virtual `engine->Tick(delta)`.
    pub engine_tick: unsafe extern "C" fn(engine: *mut c_void, delta: f32),
    /// Virtual `engine->GetMaxTickRate()`.
    pub engine_get_max_tick_rate: unsafe extern "C" fn(engine: *mut c_void) -> f32,
    /// Virtual `engine->Flush(AllowPrecache)`.
    pub engine_flush: unsafe extern "C" fn(engine: *mut c_void, allow_precache: i32),
    /// `UObject::StaticLoadClass(UEngine::StaticClass(), ..., "ini:Engine.Engine.GameEngine", ...)`.
    pub load_engine_class: unsafe extern "C" fn() -> *mut c_void,
    /// `ConstructObject<UEngine>(class)`.
    pub construct_engine: unsafe extern "C" fn(cls: *mut c_void) -> *mut c_void,
    /// `UObject::StaticLoadClass(UAudioSubsystem::StaticClass(), ..., "ini:Engine.Engine.AudioDevice", ...)`.
    pub load_audio_class: unsafe extern "C" fn() -> *mut c_void,
    /// `ConstructObject<UAudioSubsystem>(class)`.
    pub construct_audio: unsafe extern "C" fn(cls: *mut c_void) -> *mut c_void,
    /// `delete audio` (virtual destructor). Wrapped in `guard(ExitSound)` in C++.
    pub audio_delete: unsafe extern "C" fn(audio: *mut c_void),
    /// `audio->Init()`. Returns `UBOOL`.
    pub audio_init: unsafe extern "C" fn(audio: *mut c_void) -> i32,
    /// `debugf(NAME_Log, TEXT("Audio initialization failed."))`.
    pub log_audio_init_failed: unsafe extern "C" fn(),
    /// `debugf(NAME_Init, TEXT("Unreal engine initialized"))`.
    pub log_engine_initialized: unsafe extern "C" fn(),
    /// `ParseParam(appCmdLine(), param)`. Returns `UBOOL`.
    pub parse_cmdline_param: unsafe extern "C" fn(param: *const u32) -> i32,

    /// `GSys`, as `USystem*`.
    pub gsys: unsafe extern "C" fn() -> *mut c_void,
    /// `GSys->Exec(cmd, ar)`. Returns `UBOOL`.
    pub gsys_exec: unsafe extern "C" fn(sys: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `UObject::StaticExec(cmd, ar)`. Returns `UBOOL`.
    pub static_exec: unsafe extern "C" fn(cmd: *const u32, ar: *mut c_void) -> i32,
    /// `GCache.Init(bytes, align)`.
    pub gcache_init: unsafe extern "C" fn(bytes: i32, align: i32),
    /// `GCache.Exit(free_memory)`.
    pub gcache_exit: unsafe extern "C" fn(free_memory: i32),
    /// `GCache.Flush()`.
    pub gcache_flush: unsafe extern "C" fn(),
    /// `GCache.Exec(cmd, ar)`. Returns `UBOOL`.
    pub gcache_exec: unsafe extern "C" fn(cmd: *const u32, ar: *mut c_void) -> i32,
    /// `GExec`, as `FExec*`.
    pub gexec: unsafe extern "C" fn() -> *mut c_void,
    /// `GExec->Exec(cmd, ar)`. Returns `UBOOL`.
    pub gexec_exec:
        unsafe extern "C" fn(exec: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `Client->Exec(cmd, ar)`. Returns `UBOOL`.
    pub client_exec:
        unsafe extern "C" fn(client: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `Client->Flush(AllowPrecache)`.
    pub client_flush: unsafe extern "C" fn(client: *mut c_void, allow_precache: i32),
    /// `Render->Exec(cmd, ar)`. Returns `UBOOL`.
    pub render_exec:
        unsafe extern "C" fn(render: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `Audio->Exec(cmd, ar)`. Returns `UBOOL`.
    pub audio_exec:
        unsafe extern "C" fn(audio: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `GEngineMem.Init(bytes, name)`.
    pub engine_mem_init: unsafe extern "C" fn(bytes: i32, name: *const u32),
    /// `GEngineMem.Exit()`.
    pub engine_mem_exit: unsafe extern "C" fn(),
    /// `FURL::StaticInit()`.
    pub url_static_init: unsafe extern "C" fn(),
    /// `FURL::StaticExit()`.
    pub url_static_exit: unsafe extern "C" fn(),

    /// `Localize(section, key, package)` with ANSI section and key, as `const TCHAR*`.
    pub localize: unsafe extern "C" fn(
        section: *const c_char,
        key: *const c_char,
        package: *const u32,
    ) -> *const u32,
    /// `appToUpper(c)`.
    pub app_to_upper: unsafe extern "C" fn(c: u32) -> u32,
    /// `appStricmp(a, b)`.
    pub app_stricmp: unsafe extern "C" fn(a: *const u32, b: *const u32) -> i32,
    /// `GConfig->GetString("Engine.Engine", "Language", buf, len)`. Returns `UBOOL`.
    pub config_get_language: unsafe extern "C" fn(buf: *mut u32, len: i32) -> i32,
    /// `UObject::SetLanguage(text)`.
    pub set_language: unsafe extern "C" fn(text: *const u32),
    /// `appSeconds()` as the raw `FTime` value.
    pub app_seconds: unsafe extern "C" fn() -> i64,
    /// `FTime::operator-(FTime) -> FLOAT` on raw values.
    pub ftime_sub: unsafe extern "C" fn(a: i64, b: i64) -> f32,
    /// `appSleep(seconds)`.
    pub app_sleep: unsafe extern "C" fn(seconds: f32),

    /// `Viewport->Console`.
    pub viewport_console: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `Viewport->Actor`.
    pub viewport_actor: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `Viewport->Input`.
    pub viewport_input: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `APlayerPawn::myHUD` (the viewport's `Actor` is an `APlayerPawn`, `UnPlayer.h:21`).
    pub actor_hud: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `HUD->MainMenu`.
    pub hud_main_menu: unsafe extern "C" fn(hud: *mut c_void) -> *mut c_void,
    /// `UObject::GetClass()`.
    pub object_class: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    /// `UClass::GetSuperClass()`.
    pub class_super: unsafe extern "C" fn(cls: *mut c_void) -> *mut c_void,
    /// `UClass::GetName()`.
    pub class_name: unsafe extern "C" fn(cls: *mut c_void) -> *const u32,
    /// `Console->eventKeyType(key)`. Returns `UBOOL`.
    pub console_event_key_type: unsafe extern "C" fn(console: *mut c_void, key: i32) -> i32,
    /// `Console->eventKeyEvent(key, state, delta)`. Returns `UBOOL`.
    pub console_event_key_event:
        unsafe extern "C" fn(console: *mut c_void, key: i32, state: i32, delta: f32) -> i32,
    /// `(FOutputDevice&)*Console`, as `FOutputDevice*`.
    pub console_as_output: unsafe extern "C" fn(console: *mut c_void) -> *mut c_void,
    /// `Input->PreProcess(key, state, delta)`. Returns `UBOOL`.
    pub input_pre_process:
        unsafe extern "C" fn(input: *mut c_void, key: i32, state: i32, delta: f32) -> i32,
    /// `Input->Process(ar, key, state, delta)`. Returns `UBOOL`.
    pub input_process: unsafe extern "C" fn(
        input: *mut c_void,
        ar: *mut c_void,
        key: i32,
        state: i32,
        delta: f32,
    ) -> i32,

    /// `ParseCommand(stream, match)`. Returns `UBOOL`.
    pub parse_command: unsafe extern "C" fn(stream: *mut *const u32, m: *const u32) -> i32,
    /// `Ar.Log(text)`.
    pub output_log: unsafe extern "C" fn(ar: *mut c_void, text: *const u32),
    /// `Ar.Logf(text)` for literal text with no arguments.
    pub output_logf: unsafe extern "C" fn(ar: *mut c_void, text: *const u32),
    /// `Ar.Logf(fmt, *s)` with one `%s` argument.
    pub output_logf_s: unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, s: *const u32),
    /// `Ar.Logf(fmt, i)` with one `%i` argument.
    pub output_logf_i: unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, i: i32),
    /// `Ar.Logf(fmt, i, *s)` with `%i` then `%s`.
    pub output_logf_is:
        unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, i: i32, s: *const u32),

    /// `FURL URL(NULL, cmd, TRAVEL_Absolute)` on the heap. Freed by `url_free`.
    pub url_crack: unsafe extern "C" fn(cmd: *const u32) -> *mut c_void,
    /// Frees a handle from `url_crack`.
    pub url_free: unsafe extern "C" fn(url: *mut c_void),
    /// `URL.Valid`. Returns `UBOOL`.
    pub url_valid: unsafe extern "C" fn(url: *mut c_void) -> i32,
    /// `*URL.Protocol`.
    pub url_protocol: unsafe extern "C" fn(url: *mut c_void) -> *const u32,
    /// `*URL.Host`.
    pub url_host: unsafe extern "C" fn(url: *mut c_void) -> *const u32,
    /// `URL.Port`.
    pub url_port: unsafe extern "C" fn(url: *mut c_void) -> i32,
    /// `*URL.Map`.
    pub url_map: unsafe extern "C" fn(url: *mut c_void) -> *const u32,
    /// `URL.Op.Num()`.
    pub url_num_options: unsafe extern "C" fn(url: *mut c_void) -> i32,
    /// `*URL.Op(i)`.
    pub url_option: unsafe extern "C" fn(url: *mut c_void, i: i32) -> *const u32,
    /// `*URL.Portal`.
    pub url_portal: unsafe extern "C" fn(url: *mut c_void) -> *const u32,
    /// `**URL.String()`. Valid until `url_free`.
    pub url_string: unsafe extern "C" fn(url: *mut c_void) -> *const u32,
}

/// Builds a NUL-terminated `TCHAR` (`u32`) array at compile time.
const fn wide<const N: usize>(s: &str) -> [u32; N] {
    let b = s.as_bytes();
    assert!(b.len() < N);
    let mut out = [0u32; N];
    let mut i = 0;
    while i < b.len() {
        assert!(b[i] < 0x80);
        out[i] = b[i] as u32;
        i += 1;
    }
    out
}

/// `TEXT("...")`: a pointer to a static NUL-terminated `TCHAR` string.
macro_rules! w {
    ($s:literal) => {{
        const N: usize = $s.len() + 1;
        static WIDE: [u32; N] = wide::<N>($s);
        WIDE.as_ptr()
    }};
}

/// `UEngine::InitAudio()` (`UnEngine.cpp:62-85`). The `NO_SOUND_PLEASE` early
/// return is compiled out in the original, so it is not ported.
///
/// # Safety
/// `e` must be a live `UEngine`. Services must be valid.
pub unsafe fn init_audio(svc: &Hp2UEngineServices, e: *mut c_void) {
    unsafe {
        if (svc.use_sound)(e) != 0
            && (svc.is_client)() != 0
            && (svc.parse_cmdline_param)(w!("NOSOUND")) == 0
        {
            let class = (svc.load_audio_class)();
            let audio = (svc.construct_audio)(class);
            (svc.set_audio)(e, audio);
            if (svc.audio_init)(audio) == 0 {
                (svc.log_audio_init_failed)();
                (svc.audio_delete)(audio);
                (svc.set_audio)(e, ptr::null_mut());
            }
        }
    }
}

/// `UEngine::Init()` body after the `NAMES_ONLY` block (`UnEngine.cpp:105-123`).
///
/// # Safety
/// `e` must be a live `UEngine`. Services must be valid.
pub unsafe fn init(svc: &Hp2UEngineServices, e: *mut c_void) {
    unsafe {
        (svc.url_static_init)();
        (svc.engine_mem_init)(ENGINE_MEM_BYTES, w!("GEngineMem"));
        let megs = if (svc.is_client)() != 0 {
            (svc.cache_size_megs)(e)
        } else {
            CACHE_MEGS_MIN
        };
        let megs = megs.clamp(CACHE_MEGS_MIN, CACHE_MEGS_MAX);
        (svc.gcache_init)(1024 * 1024 * megs, CACHE_ALIGN);

        let yes = (svc.localize)(c"General".as_ptr(), c"Yes".as_ptr(), w!("Core"));
        YES_KEY.store((svc.app_to_upper)(*yes), Ordering::Relaxed);
        let no = (svc.localize)(c"General".as_ptr(), c"No".as_ptr(), w!("Core"));
        NO_KEY.store((svc.app_to_upper)(*no), Ordering::Relaxed);

        (svc.create_cylinder)(e);
        (svc.create_ocylinder)(e);
        (svc.create_oval_cylinder)(e);
        (svc.create_ooval_cylinder)(e);
        (svc.create_box)(e);

        (svc.add_to_root)(e);
        (svc.log_engine_initialized)();
    }
}

/// `UEngine::Exit()` body (`UnEngine.cpp:130-144`). The `guard(ExitSound)`
/// label lives in the `delete` adapter in C++.
///
/// # Safety
/// `e` must be a live `UEngine`.
pub unsafe fn exit(svc: &Hp2UEngineServices, e: *mut c_void) {
    unsafe {
        let audio = (svc.audio)(e);
        if !audio.is_null() {
            (svc.audio_delete)(audio);
            (svc.set_audio)(e, ptr::null_mut());
        }
    }
}

/// `UEngine::Destroy()` body before `Super::Destroy()` (`UnEngine.cpp:151-165`).
/// The `Audio` pointer is cleared without `delete`, as in the original.
///
/// # Safety
/// `e` must be a live `UEngine`.
pub unsafe fn destroy(svc: &Hp2UEngineServices, e: *mut c_void) {
    unsafe {
        (svc.remove_from_root)(e);
        (svc.set_audio)(e, ptr::null_mut());
        (svc.set_render)(e, ptr::null_mut());
        (svc.set_client)(e, ptr::null_mut());
        (svc.url_static_exit)();
        (svc.engine_mem_exit)();
        (svc.gcache_exit)(1);
    }
}

/// `UEngine::Flush(AllowPrecache)` body (`UnEngine.cpp:173-180`).
///
/// # Safety
/// `e` must be a live `UEngine`.
pub unsafe fn flush(svc: &Hp2UEngineServices, e: *mut c_void, allow_precache: i32) {
    unsafe {
        (svc.gcache_flush)();
        let client = (svc.client)(e);
        if !client.is_null() {
            (svc.client_flush)(client, allow_precache);
        }
    }
}

/// `UEngine::GetMaxTickRate()` (`UnEngine.cpp:185-190`).
///
/// # Safety
/// Services must be valid.
pub unsafe fn get_max_tick_rate(svc: &Hp2UEngineServices) -> f32 {
    if unsafe { (svc.is_editor)() } != 0 {
        EDITOR_MAX_TICK_RATE
    } else {
        0.0
    }
}

/// `UEngine::Exec(Cmd, Ar)` (`UnEngine.cpp:222-262`). Returns `UBOOL`.
///
/// # Safety
/// `e` must be a live `UEngine`; `cmd` a NUL-terminated `TCHAR` string; `ar` a
/// live `FOutputDevice`.
pub unsafe fn exec(
    svc: &Hp2UEngineServices,
    e: *mut c_void,
    cmd: *const u32,
    ar: *mut c_void,
) -> i32 {
    unsafe {
        let sys = (svc.gsys)();
        if !sys.is_null() && (svc.gsys_exec)(sys, cmd, ar) != 0 {
            return 1;
        }
        if (svc.static_exec)(cmd, ar) != 0 {
            return 1;
        }
        if (svc.gcache_exec)(cmd, ar) != 0 {
            return 1;
        }
        let exec_dev = (svc.gexec)();
        if !exec_dev.is_null() && (svc.gexec_exec)(exec_dev, cmd, ar) != 0 {
            return 1;
        }
        let client = (svc.client)(e);
        if !client.is_null() && (svc.client_exec)(client, cmd, ar) != 0 {
            return 1;
        }
        let render = (svc.render)(e);
        if !render.is_null() && (svc.render_exec)(render, cmd, ar) != 0 {
            return 1;
        }
        let audio = (svc.audio)(e);
        if !audio.is_null() && (svc.audio_exec)(audio, cmd, ar) != 0 {
            return 1;
        }

        let mut stream = cmd;
        if (svc.parse_command)(&mut stream, w!("FLUSH")) != 0 {
            (svc.engine_flush)(e, 1);
            (svc.output_log)(ar, w!("Flushed engine caches"));
            1
        } else if (svc.parse_command)(&mut stream, w!("CRACKURL")) != 0 {
            let url = (svc.url_crack)(stream);
            if (svc.url_valid)(url) != 0 {
                (svc.output_logf_s)(ar, w!("     Protocol: %s"), (svc.url_protocol)(url));
                (svc.output_logf_s)(ar, w!("         Host: %s"), (svc.url_host)(url));
                (svc.output_logf_i)(ar, w!("         Port: %i"), (svc.url_port)(url));
                (svc.output_logf_s)(ar, w!("          Map: %s"), (svc.url_map)(url));
                let num = (svc.url_num_options)(url);
                (svc.output_logf_i)(ar, w!("   NumOptions: %i"), num);
                for i in 0..num {
                    (svc.output_logf_is)(ar, w!("     Option %i: %s"), i, (svc.url_option)(url, i));
                }
                (svc.output_logf_s)(ar, w!("       Portal: %s"), (svc.url_portal)(url));
                (svc.output_logf_s)(ar, w!("       String: '%s'"), (svc.url_string)(url));
            } else {
                (svc.output_logf)(ar, w!("BAD URL"));
            }
            (svc.url_free)(url);
            1
        } else {
            0
        }
    }
}

/// `UEngine::Key(Viewport, Key)` (`UnEngine.cpp:267-272`). Returns `UBOOL`.
///
/// # Safety
/// `viewport` must be a live `UViewport`.
pub unsafe fn key(svc: &Hp2UEngineServices, viewport: *mut c_void, key: i32) -> i32 {
    unsafe {
        let console = (svc.viewport_console)(viewport);
        (!console.is_null() && (svc.console_event_key_type)(console, key) != 0) as i32
    }
}

/// `UEngine::InputEvent(Viewport, iKey, State, Delta)` (`UnEngine.cpp:277-320`).
/// Returns `UBOOL`.
///
/// # Safety
/// `viewport` must be a live `UViewport`.
pub unsafe fn input_event(
    svc: &Hp2UEngineServices,
    viewport: *mut c_void,
    mut i_key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    unsafe {
        // Translate yes/no if necessary (`oldver`).
        if !(svc.viewport_console)(viewport).is_null() && !(svc.viewport_actor)(viewport).is_null()
        {
            let actor = (svc.viewport_actor)(viewport);
            let hud = (svc.actor_hud)(actor);
            if !hud.is_null() {
                let menu = (svc.hud_main_menu)(hud);
                if !menu.is_null() {
                    let mut class = (svc.object_class)(menu);
                    while !class.is_null() {
                        let name = (svc.class_name)(class);
                        if (svc.app_stricmp)(name, w!("UnrealQuitMenu")) == 0
                            || (svc.app_stricmp)(name, w!("UnrealYesNoMenu")) == 0
                        {
                            break;
                        }
                        class = (svc.class_super)(class);
                    }
                    let found = !class.is_null();
                    if found && (svc.app_to_upper)(i_key as u32) == YES_KEY.load(Ordering::Relaxed)
                    {
                        i_key = IK_Y;
                    } else if found
                        && (svc.app_to_upper)(i_key as u32) == NO_KEY.load(Ordering::Relaxed)
                    {
                        i_key = IK_N;
                    }
                }
            }
        }

        // Process it.
        let console = (svc.viewport_console)(viewport);
        if !console.is_null() && (svc.console_event_key_event)(console, i_key, state, delta) != 0 {
            // !! fix for continuous mouse-up events!
            if state == IST_RELEASE {
                let input = (svc.viewport_input)(viewport);
                (svc.input_pre_process)(input, i_key, state, delta);
            }
            // Player console handled it.
            return 1;
        }

        let input = (svc.viewport_input)(viewport);
        if (svc.input_pre_process)(input, i_key, state, delta) != 0 {
            let input = (svc.viewport_input)(viewport);
            let console = (svc.viewport_console)(viewport);
            let ar = if console.is_null() {
                (svc.glog)()
            } else {
                (svc.console_as_output)(console)
            };
            if (svc.input_process)(input, ar, i_key, state, delta) != 0 {
                // Input system handled it.
                return 1;
            }
        }
        // Nobody handled it.
        0
    }
}

/// `UServerCommandlet::Main(Parms)` (`UnEngine.cpp:346-394`). `Parms` is
/// unused in the original and not passed. Returns `INT`.
///
/// # Safety
/// Services must be valid.
pub unsafe fn commandlet_main(svc: &Hp2UEngineServices) -> i32 {
    unsafe {
        // Language.
        let mut temp = [0u32; LANGUAGE_BUF_LEN as usize];
        if (svc.config_get_language)(temp.as_mut_ptr(), LANGUAGE_BUF_LEN) != 0 {
            (svc.set_language)(temp.as_ptr());
        }

        // Create the engine class.
        let class = (svc.load_engine_class)();
        let engine = (svc.construct_engine)(class);
        (svc.engine_init)(engine);

        // Main loop.
        (svc.set_is_running)(1);
        let mut old_time = (svc.app_seconds)();
        let mut second_start_time = old_time;
        let mut tick_count: i32 = 0;
        while (svc.is_running)() != 0 && (svc.is_requesting_exit)() == 0 {
            // Update the world.
            let new_time = (svc.app_seconds)();
            (svc.engine_tick)(engine, (svc.ftime_sub)(new_time, old_time));
            old_time = new_time;
            tick_count += 1;
            let since_second = (svc.ftime_sub)(old_time, second_start_time);
            if since_second > 1.0 {
                (svc.set_current_tick_rate)(engine, tick_count as f32 / since_second);
                second_start_time = old_time;
                tick_count = 0;
            }

            // Enforce optional maximum tick rate.
            let max_tick_rate = (svc.engine_get_max_tick_rate)(engine);
            if max_tick_rate > 0.0 {
                let delta =
                    (1.0f32 / max_tick_rate) - (svc.ftime_sub)((svc.app_seconds)(), old_time);
                // `Max(0.f, Delta)` = `(0.f >= Delta) ? 0.f : Delta`.
                let sleep = if 0.0f32 >= delta { 0.0f32 } else { delta };
                (svc.app_sleep)(sleep);
            }
        }
        (svc.set_is_running)(0);
        0
    }
}
