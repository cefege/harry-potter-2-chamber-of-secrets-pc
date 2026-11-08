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
//! Native leaves return checked status; Rust returns the first failure without
//! unwinding. Typed native continuations retain ExitSound, UpdateWorld,
//! EnforceTickRate and CRACKURL stack lifetimes at their original boundaries.

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
pub(crate) const ENGINE_MEM_BYTES: i32 = 65536;
/// `GCache.Init` alignment (`UnEngine.cpp:107`).
pub(crate) const CACHE_ALIGN: i32 = 4096;
/// `Clamp(..., 1, 1024)` bounds on `CacheSizeMegs` (`UnEngine.cpp:107`).
pub(crate) const CACHE_MEGS_MIN: i32 = 1;
pub(crate) const CACHE_MEGS_MAX: i32 = 1024;
/// `UServerCommandlet::Main` language buffer (native commandlet oracle).
const LANGUAGE_BUF_LEN: i32 = 256;

/// Publishes the process-wide localized Yes/No initials used by native key handling.
pub(crate) fn publish_yes_no_keys(yes: u32, no: u32) {
    YES_KEY.store(yes, Ordering::Relaxed);
    NO_KEY.store(no, Ordering::Relaxed);
}

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
    pub is_client: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GIsEditor`. Returns `UBOOL`.
    pub is_editor: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GIsRunning`. Returns `UBOOL`.
    pub is_running: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GIsRunning = v`.
    pub set_is_running: unsafe extern "C" fn(v: i32) -> i32,
    /// Reads the Rust-owned graceful-exit request. Returns `UBOOL`.
    pub is_requesting_exit: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GLog`, as `FOutputDevice*`.
    pub glog: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,

    /// `UEngine::CacheSizeMegs`.
    pub cache_size_megs: unsafe extern "C" fn(engine: *mut c_void, out: *mut i32) -> i32,
    /// `UEngine::UseSound`. Returns `UBOOL`.
    pub use_sound: unsafe extern "C" fn(engine: *mut c_void, out: *mut i32) -> i32,
    /// `UEngine::Audio`.
    pub audio: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UEngine::Audio = v`.
    pub set_audio: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void) -> i32,
    /// `UEngine::Render`.
    pub render: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UEngine::Render = v`.
    pub set_render: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void) -> i32,
    /// `UEngine::Client`.
    pub client: unsafe extern "C" fn(engine: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UEngine::Client = v`.
    pub set_client: unsafe extern "C" fn(engine: *mut c_void, v: *mut c_void) -> i32,
    /// `UEngine::CurrentTickRate = v`.
    pub set_current_tick_rate: unsafe extern "C" fn(engine: *mut c_void, v: f32) -> i32,
    /// `UEngine::Cylinder = new UCylinder`.
    pub create_cylinder: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::OCylinder = new UOrientedCylinder`.
    pub create_ocylinder: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::OvalCylinder = new UAlignedOvalCylinder`.
    pub create_oval_cylinder: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::OOvalCylinder = new UOrientedOvalCylinder`.
    pub create_ooval_cylinder: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `UEngine::Box = new UBox`.
    pub create_box: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `AddToRoot()` on the engine object.
    pub add_to_root: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// `RemoveFromRoot()` on the engine object.
    pub remove_from_root: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// Virtual `engine->Init()`.
    pub engine_init: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// Virtual `engine->Tick(delta)`.
    pub engine_tick: unsafe extern "C" fn(engine: *mut c_void, delta: f32) -> i32,
    /// Virtual `engine->GetMaxTickRate()`.
    pub engine_get_max_tick_rate: unsafe extern "C" fn(engine: *mut c_void, out: *mut f32) -> i32,
    /// Virtual `engine->Flush(AllowPrecache)`.
    pub engine_flush: unsafe extern "C" fn(engine: *mut c_void, allow_precache: i32) -> i32,
    /// `UObject::StaticLoadClass(UEngine::StaticClass(), ..., "ini:Engine.Engine.GameEngine", ...)`.
    pub load_engine_class: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    /// `ConstructObject<UEngine>(class)`.
    pub construct_engine: unsafe extern "C" fn(cls: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UObject::StaticLoadClass(UAudioSubsystem::StaticClass(), ..., "ini:Engine.Engine.AudioDevice", ...)`.
    pub load_audio_class: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    /// `ConstructObject<UAudioSubsystem>(class)`.
    pub construct_audio: unsafe extern "C" fn(cls: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `delete audio` (virtual destructor); the caller owns any native guard.
    pub audio_delete: unsafe extern "C" fn(audio: *mut c_void) -> i32,
    /// `audio->Init()`. Returns `UBOOL`.
    pub audio_init: unsafe extern "C" fn(audio: *mut c_void, out: *mut i32) -> i32,
    /// `debugf(NAME_Log, TEXT("Audio initialization failed."))`.
    pub log_audio_init_failed: unsafe extern "C" fn() -> i32,
    /// `debugf(NAME_Init, TEXT("Unreal engine initialized"))`.
    pub log_engine_initialized: unsafe extern "C" fn() -> i32,
    /// `ParseParam(appCmdLine(), param)`. Returns `UBOOL`.
    pub parse_cmdline_param: unsafe extern "C" fn(param: *const u32, out: *mut i32) -> i32,

    /// `GSys`, as `USystem*`.
    pub gsys: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    /// `GSys->Exec(cmd, ar)`. Returns `UBOOL`.
    pub gsys_exec: unsafe extern "C" fn(
        sys: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `UObject::StaticExec(cmd, ar)`. Returns `UBOOL`.
    pub static_exec: unsafe extern "C" fn(cmd: *const u32, ar: *mut c_void, out: *mut i32) -> i32,
    /// `GCache.Init(bytes, align)`.
    pub gcache_init: unsafe extern "C" fn(bytes: i32, align: i32) -> i32,
    /// `GCache.Exit(free_memory)`.
    pub gcache_exit: unsafe extern "C" fn(free_memory: i32) -> i32,
    /// `GCache.Flush()`.
    pub gcache_flush: unsafe extern "C" fn() -> i32,
    /// `GCache.Exec(cmd, ar)`. Returns `UBOOL`.
    pub gcache_exec: unsafe extern "C" fn(cmd: *const u32, ar: *mut c_void, out: *mut i32) -> i32,
    /// `GExec`, as `FExec*`.
    pub gexec: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    /// `GExec->Exec(cmd, ar)`. Returns `UBOOL`.
    pub gexec_exec: unsafe extern "C" fn(
        exec: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `Client->Exec(cmd, ar)`. Returns `UBOOL`.
    pub client_exec: unsafe extern "C" fn(
        client: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `Client->Flush(AllowPrecache)`.
    pub client_flush: unsafe extern "C" fn(client: *mut c_void, allow_precache: i32) -> i32,
    /// `Render->Exec(cmd, ar)`. Returns `UBOOL`.
    pub render_exec: unsafe extern "C" fn(
        render: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `Audio->Exec(cmd, ar)`. Returns `UBOOL`.
    pub audio_exec: unsafe extern "C" fn(
        audio: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `GEngineMem.Init(bytes, name)`.
    pub engine_mem_init: unsafe extern "C" fn(bytes: i32, name: *const u32) -> i32,
    /// `GEngineMem.Exit()`.
    pub engine_mem_exit: unsafe extern "C" fn() -> i32,
    /// `FURL::StaticInit()`.
    pub url_static_init: unsafe extern "C" fn() -> i32,
    /// `FURL::StaticExit()`.
    pub url_static_exit: unsafe extern "C" fn() -> i32,

    /// `Localize(section, key, package)` with ANSI section and key, as `const TCHAR*`.
    pub localize: unsafe extern "C" fn(
        section: *const c_char,
        key: *const c_char,
        package: *const u32,
        out: *mut *const u32,
    ) -> i32,
    /// `appToUpper(c)`.
    pub app_to_upper: unsafe extern "C" fn(c: u32, out: *mut u32) -> i32,
    /// `appStricmp(a, b)`.
    pub app_stricmp: unsafe extern "C" fn(a: *const u32, b: *const u32, out: *mut i32) -> i32,
    /// `GConfig->GetString("Engine.Engine", "Language", buf, len)`. Returns `UBOOL`.
    pub config_get_language: unsafe extern "C" fn(buf: *mut u32, len: i32, out: *mut i32) -> i32,
    /// `UObject::SetLanguage(text)`.
    pub set_language: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `appSeconds()` as the raw `FTime` value.
    pub app_seconds: unsafe extern "C" fn(out: *mut i64) -> i32,
    /// `FTime::operator-(FTime) -> FLOAT` on raw values.
    pub ftime_sub: unsafe extern "C" fn(a: i64, b: i64, out: *mut f32) -> i32,
    /// `appSleep(seconds)`.
    pub app_sleep: unsafe extern "C" fn(seconds: f32) -> i32,

    /// `Viewport->Console`.
    pub viewport_console: unsafe extern "C" fn(viewport: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `Viewport->Actor`.
    pub viewport_actor: unsafe extern "C" fn(viewport: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `Viewport->Input`.
    pub viewport_input: unsafe extern "C" fn(viewport: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `APlayerPawn::myHUD` (the viewport's `Actor` is an `APlayerPawn`, `UnPlayer.h:21`).
    pub actor_hud: unsafe extern "C" fn(actor: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `HUD->MainMenu`.
    pub hud_main_menu: unsafe extern "C" fn(hud: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UObject::GetClass()`.
    pub object_class: unsafe extern "C" fn(obj: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UClass::GetSuperClass()`.
    pub class_super: unsafe extern "C" fn(cls: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UClass::GetName()`.
    pub class_name: unsafe extern "C" fn(cls: *mut c_void, out: *mut *const u32) -> i32,
    /// `Console->eventKeyType(key)`. Returns `UBOOL`.
    pub console_event_key_type:
        unsafe extern "C" fn(console: *mut c_void, key: i32, out: *mut i32) -> i32,
    /// `Console->eventKeyEvent(key, state, delta)`. Returns `UBOOL`.
    pub console_event_key_event: unsafe extern "C" fn(
        console: *mut c_void,
        key: i32,
        state: i32,
        delta: f32,
        out: *mut i32,
    ) -> i32,
    /// `(FOutputDevice&)*Console`, as `FOutputDevice*`.
    pub console_as_output: unsafe extern "C" fn(console: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `Input->PreProcess(key, state, delta)`. Returns `UBOOL`.
    pub input_pre_process: unsafe extern "C" fn(
        input: *mut c_void,
        key: i32,
        state: i32,
        delta: f32,
        out: *mut i32,
    ) -> i32,
    /// `Input->Process(ar, key, state, delta)`. Returns `UBOOL`.
    pub input_process: unsafe extern "C" fn(
        input: *mut c_void,
        ar: *mut c_void,
        key: i32,
        state: i32,
        delta: f32,
        out: *mut i32,
    ) -> i32,

    /// `ParseCommand(stream, match)`. Returns `UBOOL`.
    pub parse_command:
        unsafe extern "C" fn(stream: *mut *const u32, m: *const u32, out: *mut i32) -> i32,
    /// `Ar.Log(text)`.
    pub output_log: unsafe extern "C" fn(ar: *mut c_void, text: *const u32) -> i32,
    /// `Ar.Logf(text)` for literal text with no arguments.
    pub output_logf: unsafe extern "C" fn(ar: *mut c_void, text: *const u32) -> i32,
    /// `Ar.Logf(fmt, *s)` with one `%s` argument.
    pub output_logf_s: unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, s: *const u32) -> i32,
    /// `Ar.Logf(fmt, i)` with one `%i` argument.
    pub output_logf_i: unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, i: i32) -> i32,
    /// `Ar.Logf(fmt, i, *s)` with `%i` then `%s`.
    pub output_logf_is:
        unsafe extern "C" fn(ar: *mut c_void, fmt: *const u32, i: i32, s: *const u32) -> i32,

    /// Typed native FURL stack scope; resumes `crack_url` and destroys on failure.
    pub crack_url_scope: unsafe extern "C" fn(cmd: *const u32, ar: *mut c_void) -> i32,
    /// `URL.Valid`. Returns `UBOOL`.
    pub url_valid: unsafe extern "C" fn(url: *mut c_void, out: *mut i32) -> i32,
    /// `*URL.Protocol`.
    pub url_protocol: unsafe extern "C" fn(url: *mut c_void, out: *mut *const u32) -> i32,
    /// `*URL.Host`.
    pub url_host: unsafe extern "C" fn(url: *mut c_void, out: *mut *const u32) -> i32,
    /// `URL.Port`.
    pub url_port: unsafe extern "C" fn(url: *mut c_void, out: *mut i32) -> i32,
    /// `*URL.Map`.
    pub url_map: unsafe extern "C" fn(url: *mut c_void, out: *mut *const u32) -> i32,
    /// `URL.Op.Num()`.
    pub url_num_options: unsafe extern "C" fn(url: *mut c_void, out: *mut i32) -> i32,
    /// `*URL.Op(i)`.
    pub url_option: unsafe extern "C" fn(url: *mut c_void, i: i32, out: *mut *const u32) -> i32,
    /// `*URL.Portal`.
    pub url_portal: unsafe extern "C" fn(url: *mut c_void, out: *mut *const u32) -> i32,
    /// Log the temporary `URL.String()` within its native full-expression lifetime.
    pub url_log_string: unsafe extern "C" fn(url: *mut c_void, ar: *mut c_void) -> i32,
    /// Native `guard(ExitSound)` enclosing the complete exit-audio policy.
    pub exit_audio_scope: unsafe extern "C" fn(engine: *mut c_void) -> i32,
    /// Native UpdateWorld (0) / EnforceTickRate (1) guard scopes.
    pub commandlet_scope: unsafe extern "C" fn(context: *mut c_void, entry: u32) -> i32,
}

// These macros return immediately on native failure. Outputs are read only after
// successful callbacks; C++ owns the exception and all native object lifetimes.
macro_rules! call {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let status = ($f)($($arg),*);
        if status != 0 { return Err(status); }
    }};
}
macro_rules! value {
    ($f:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = std::mem::MaybeUninit::uninit();
        let status = ($f)($($arg,)* out.as_mut_ptr());
        if status != 0 { return Err(status); }
        out.assume_init()
    }};
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
pub unsafe fn init_audio(svc: &Hp2UEngineServices, e: *mut c_void) -> Result<(), i32> {
    unsafe {
        if value!(svc.use_sound, e) != 0
            && value!(svc.is_client) != 0
            && value!(svc.parse_cmdline_param, w!("NOSOUND")) == 0
        {
            let class = value!(svc.load_audio_class);
            let audio = value!(svc.construct_audio, class);
            call!(svc.set_audio, e, audio);
            if value!(svc.audio_init, audio) == 0 {
                call!(svc.log_audio_init_failed);
                call!(svc.audio_delete, value!(svc.audio, e));
                call!(svc.set_audio, e, ptr::null_mut());
            }
        }
    }
    Ok(())
}

/// `UEngine::Init()` body after the `NAMES_ONLY` block (`UnEngine.cpp:105-123`).
///
/// # Safety
/// `e` must be a live `UEngine`. Services must be valid.
pub unsafe fn init(svc: &Hp2UEngineServices, e: *mut c_void) -> Result<(), i32> {
    unsafe {
        call!(svc.url_static_init);
        call!(svc.engine_mem_init, ENGINE_MEM_BYTES, w!("GEngineMem"));
        let megs = if value!(svc.is_client) != 0 {
            value!(svc.cache_size_megs, e)
        } else {
            CACHE_MEGS_MIN
        };
        let megs = megs.clamp(CACHE_MEGS_MIN, CACHE_MEGS_MAX);
        call!(svc.gcache_init, 1024 * 1024 * megs, CACHE_ALIGN);

        let yes = value!(
            svc.localize,
            c"General".as_ptr(),
            c"Yes".as_ptr(),
            w!("Core")
        );
        YES_KEY.store(value!(svc.app_to_upper, *yes), Ordering::Relaxed);
        let no = value!(
            svc.localize,
            c"General".as_ptr(),
            c"No".as_ptr(),
            w!("Core")
        );
        NO_KEY.store(value!(svc.app_to_upper, *no), Ordering::Relaxed);

        call!(svc.create_cylinder, e);
        call!(svc.create_ocylinder, e);
        call!(svc.create_oval_cylinder, e);
        call!(svc.create_ooval_cylinder, e);
        call!(svc.create_box, e);

        call!(svc.add_to_root, e);
        call!(svc.log_engine_initialized);
    }
    Ok(())
}

/// `UEngine::Exit()` body (`UnEngine.cpp:130-144`). The `guard(ExitSound)`
/// scope encloses all exit-audio operations, but not failed InitAudio cleanup.
///
/// # Safety
/// `e` must be a live `UEngine`.
pub unsafe fn exit(svc: &Hp2UEngineServices, e: *mut c_void) -> Result<(), i32> {
    unsafe {
        call!(svc.exit_audio_scope, e);
    }
    Ok(())
}

/// Owner-agnostic operations for the source `UEngine::Exit` audio body.
pub(crate) trait ExitAudioOps {
    type Error;
    fn audio(&mut self, engine: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn delete_audio(&mut self, audio: *mut c_void) -> Result<(), Self::Error>;
    fn set_audio(&mut self, engine: *mut c_void, audio: *mut c_void) -> Result<(), Self::Error>;
}

/// Shared source `UEngine::Exit` operation over owner-provided audio leaves.
pub(crate) unsafe fn exit_audio_with_ops<O: ExitAudioOps>(
    ops: &mut O,
    engine: *mut c_void,
) -> Result<(), O::Error> {
    let audio = ops.audio(engine)?;
    if !audio.is_null() {
        ops.delete_audio(audio)?;
        ops.set_audio(engine, std::ptr::null_mut())?;
    }
    Ok(())
}

struct NativeExitAudioOps<'a>(&'a Hp2UEngineServices);
impl ExitAudioOps for NativeExitAudioOps<'_> {
    type Error = i32;
    fn audio(&mut self, engine: *mut c_void) -> Result<*mut c_void, i32> {
        unsafe { Ok(value!(self.0.audio, engine)) }
    }
    fn delete_audio(&mut self, audio: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.audio_delete, audio); }
        Ok(())
    }
    fn set_audio(&mut self, engine: *mut c_void, audio: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.set_audio, engine, audio); }
        Ok(())
    }
}

/// Policy resumed inside the original native `ExitSound` guard.
///
/// # Safety
/// `e` and the service table must remain live throughout the native scope.
pub unsafe fn exit_audio(svc: &Hp2UEngineServices, e: *mut c_void) -> Result<(), i32> {
    unsafe { exit_audio_with_ops(&mut NativeExitAudioOps(svc), e) }
}

/// `UEngine::Destroy()` body before `Super::Destroy()` (`UnEngine.cpp:151-165`).
/// The `Audio` pointer is cleared without `delete`, as in the original.
///
/// # Safety
/// `e` must be a live `UEngine`.
pub unsafe fn destroy(svc: &Hp2UEngineServices, e: *mut c_void) -> Result<(), i32> {
    unsafe {
        call!(svc.remove_from_root, e);
        call!(svc.set_audio, e, ptr::null_mut());
        call!(svc.set_render, e, ptr::null_mut());
        call!(svc.set_client, e, ptr::null_mut());
        call!(svc.url_static_exit);
        call!(svc.engine_mem_exit);
        call!(svc.gcache_exit, 1);
    }
    Ok(())
}

/// `UEngine::Flush(AllowPrecache)` body (`UnEngine.cpp:173-180`).
///
/// # Safety
/// `e` must be a live `UEngine`.
pub unsafe fn flush(
    svc: &Hp2UEngineServices,
    e: *mut c_void,
    allow_precache: i32,
) -> Result<(), i32> {
    unsafe {
        flush_with::<i32>(
            e,
            allow_precache,
            |_| { call!(svc.gcache_flush); Ok(()) },
            |engine| Ok(value!(svc.client, engine)),
            |client, allow| { call!(svc.client_flush, client, allow); Ok(()) },
        )
    }
}

/// Shared `UEngine::Flush` order for native and canonical adapters.
pub(crate) unsafe fn flush_with<E>(
    engine: *mut c_void,
    allow_precache: i32,
    mut cache_flush: impl FnMut(*mut c_void) -> Result<(), E>,
    mut current_client: impl FnMut(*mut c_void) -> Result<*mut c_void, E>,
    mut client_flush: impl FnMut(*mut c_void, i32) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        cache_flush(engine)?;
        let client = current_client(engine)?;
        if !client.is_null() { client_flush(client, allow_precache)?; }
    }
    Ok(())
}

/// `UEngine::GetMaxTickRate()` (`UnEngine.cpp:185-190`).
///
/// # Safety
/// Services must be valid.
pub unsafe fn get_max_tick_rate(svc: &Hp2UEngineServices) -> Result<f32, i32> {
    unsafe {
        Ok(if value!(svc.is_editor) != 0 {
            EDITOR_MAX_TICK_RATE
        } else {
            0.0
        })
    }
}

/// Operations behind the shared source-ordered `UEngine::Exec` policy.
pub(crate) trait EngineExecOps {
    type Command;
    type Error;

    unsafe fn system(&mut self, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn static_exec(&mut self, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn cache(&mut self, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn global_exec(&mut self, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn client(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn render(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn audio(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, Self::Error>;
    unsafe fn flush(&mut self, engine: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn flushed(&mut self, command: &mut Self::Command) -> Result<(), Self::Error>;
    unsafe fn crack_url(&mut self, command: &mut Self::Command) -> Result<(), Self::Error>;
    unsafe fn parse_command(&mut self, command: &mut Self::Command, keyword: &[u32]) -> Result<bool, Self::Error>;
}

/// Runs the shared, source-ordered `UEngine::Exec` dispatch.
///
/// # Safety
/// Engine, command and operation-owned records must remain valid through synchronous reentry.
pub(crate) unsafe fn exec_with_ops<O: EngineExecOps>(
    ops: &mut O, engine: *mut c_void, command: &mut O::Command,
) -> Result<bool, O::Error> {
    if unsafe { ops.system(command)? } { return Ok(true); }
    if unsafe { ops.static_exec(command)? } { return Ok(true); }
    if unsafe { ops.cache(command)? } { return Ok(true); }
    if unsafe { ops.global_exec(command)? } { return Ok(true); }
    if unsafe { ops.client(engine, command)? } { return Ok(true); }
    if unsafe { ops.render(engine, command)? } { return Ok(true); }
    if unsafe { ops.audio(engine, command)? } { return Ok(true); }

    if unsafe { ops.parse_command(command, &[70, 76, 85, 83, 72, 0])? } {
        unsafe { ops.flush(engine)?; }
        unsafe { ops.flushed(command)?; }
        return Ok(true);
    }
    if unsafe { ops.parse_command(command, &[67, 82, 65, 67, 75, 85, 82, 76, 0])? } {
        unsafe { ops.crack_url(command)?; }
        return Ok(true);
    }
    Ok(false)
}

struct NativeEngineExecCommand {
    cmd: *const u32,
    ar: *mut c_void,
}

struct NativeEngineExecOps<'a>(&'a Hp2UEngineServices);
impl EngineExecOps for NativeEngineExecOps<'_> {
    type Command = NativeEngineExecCommand;
    type Error = i32;

    unsafe fn system(&mut self, command: &mut Self::Command) -> Result<bool, i32> {
        let system = unsafe { value!(self.0.gsys) };
        Ok(!system.is_null() && unsafe { value!(self.0.gsys_exec, system, command.cmd, command.ar) } != 0)
    }
    unsafe fn static_exec(&mut self, command: &mut Self::Command) -> Result<bool, i32> {
        Ok(unsafe { value!(self.0.static_exec, command.cmd, command.ar) } != 0)
    }
    unsafe fn cache(&mut self, command: &mut Self::Command) -> Result<bool, i32> {
        Ok(unsafe { value!(self.0.gcache_exec, command.cmd, command.ar) } != 0)
    }
    unsafe fn global_exec(&mut self, command: &mut Self::Command) -> Result<bool, i32> {
        let device = unsafe { value!(self.0.gexec) };
        Ok(!device.is_null() && unsafe { value!(self.0.gexec_exec, device, command.cmd, command.ar) } != 0)
    }
    unsafe fn client(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, i32> {
        let client = unsafe { value!(self.0.client, engine) };
        Ok(!client.is_null() && unsafe { value!(self.0.client_exec, client, command.cmd, command.ar) } != 0)
    }
    unsafe fn render(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, i32> {
        let render = unsafe { value!(self.0.render, engine) };
        Ok(!render.is_null() && unsafe { value!(self.0.render_exec, render, command.cmd, command.ar) } != 0)
    }
    unsafe fn audio(&mut self, engine: *mut c_void, command: &mut Self::Command) -> Result<bool, i32> {
        let audio = unsafe { value!(self.0.audio, engine) };
        Ok(!audio.is_null() && unsafe { value!(self.0.audio_exec, audio, command.cmd, command.ar) } != 0)
    }
    unsafe fn flush(&mut self, engine: *mut c_void) -> Result<(), i32> {
        unsafe { call!(self.0.engine_flush, engine, 1); }
        Ok(())
    }
    unsafe fn flushed(&mut self, command: &mut Self::Command) -> Result<(), i32> {
        unsafe { call!(self.0.output_log, command.ar, w!("Flushed engine caches")); }
        Ok(())
    }
    unsafe fn crack_url(&mut self, command: &mut Self::Command) -> Result<(), i32> {
        unsafe { call!(self.0.crack_url_scope, command.cmd, command.ar); }
        Ok(())
    }
    unsafe fn parse_command(&mut self, command: &mut Self::Command, keyword: &[u32]) -> Result<bool, i32> {
        let mut stream = command.cmd;
        let parsed = unsafe { value!(self.0.parse_command, &mut stream, keyword.as_ptr()) } != 0;
        if parsed { command.cmd = stream; }
        Ok(parsed)
    }
}

/// `UEngine::Exec(Cmd, Ar)` (`UnEngine.cpp:222-262`). Returns `UBOOL`.
///
/// # Safety
/// `e` must be a live `UEngine`; `cmd` a NUL-terminated `TCHAR` string; `ar` a
/// live `FOutputDevice`.
pub unsafe fn exec(
    svc: &Hp2UEngineServices, e: *mut c_void, cmd: *const u32, ar: *mut c_void,
) -> Result<i32, i32> {
    let mut command = NativeEngineExecCommand { cmd, ar };
    Ok(i32::from(unsafe { exec_with_ops(&mut NativeEngineExecOps(svc), e, &mut command)? }))
}

#[derive(Clone, Copy)]
pub(crate) enum CrackUrlField {
    Protocol, Host, Port, Map, NumOptions, Option, Portal, String, BadUrl,
}

pub(crate) trait CrackUrlOps {
    type Error;
    unsafe fn valid(&mut self) -> Result<bool, Self::Error>;
    unsafe fn option_count(&mut self) -> Result<i32, Self::Error>;
    unsafe fn emit(&mut self, field: CrackUrlField, index: i32) -> Result<(), Self::Error>;
}

/// The native and canonical URL scopes share the complete diagnostic order.
pub(crate) unsafe fn crack_url_with<O: CrackUrlOps>(ops: &mut O) -> Result<(), O::Error> {
    if unsafe { ops.valid()? } {
        for field in [
            CrackUrlField::Protocol, CrackUrlField::Host, CrackUrlField::Port,
            CrackUrlField::Map, CrackUrlField::NumOptions,
        ] {
            unsafe { ops.emit(field, 0)?; }
        }
        let mut index = 0;
        while index < unsafe { ops.option_count()? } {
            unsafe { ops.emit(CrackUrlField::Option, index)?; }
            index += 1;
        }
        unsafe {
            ops.emit(CrackUrlField::Portal, 0)?;
            ops.emit(CrackUrlField::String, 0)?;
        }
    } else {
        unsafe { ops.emit(CrackUrlField::BadUrl, 0)?; }
    }
    Ok(())
}

struct NativeCrackUrlOps<'a> {
    services: &'a Hp2UEngineServices,
    url: *mut c_void,
    output: *mut c_void,
}

impl CrackUrlOps for NativeCrackUrlOps<'_> {
    type Error = i32;
    unsafe fn valid(&mut self) -> Result<bool, i32> {
        Ok(unsafe { value!(self.services.url_valid, self.url) } != 0)
    }
    unsafe fn option_count(&mut self) -> Result<i32, i32> {
        Ok(unsafe { value!(self.services.url_num_options, self.url) })
    }
    unsafe fn emit(&mut self, field: CrackUrlField, index: i32) -> Result<(), i32> {
        let svc = self.services;
        let url = self.url;
        let ar = self.output;
        unsafe {
            match field {
                CrackUrlField::Protocol => call!(svc.output_logf_s, ar, w!("     Protocol: %s"), value!(svc.url_protocol, url)),
                CrackUrlField::Host => call!(svc.output_logf_s, ar, w!("         Host: %s"), value!(svc.url_host, url)),
                CrackUrlField::Port => call!(svc.output_logf_i, ar, w!("         Port: %i"), value!(svc.url_port, url)),
                CrackUrlField::Map => call!(svc.output_logf_s, ar, w!("          Map: %s"), value!(svc.url_map, url)),
                CrackUrlField::NumOptions => call!(svc.output_logf_i, ar, w!("   NumOptions: %i"), value!(svc.url_num_options, url)),
                CrackUrlField::Option => call!(svc.output_logf_is, ar, w!("     Option %i: %s"), index, value!(svc.url_option, url, index)),
                CrackUrlField::Portal => call!(svc.output_logf_s, ar, w!("       Portal: %s"), value!(svc.url_portal, url)),
                CrackUrlField::String => call!(svc.url_log_string, url, ar),
                CrackUrlField::BadUrl => call!(svc.output_logf, ar, w!("BAD URL")),
            }
        }
        Ok(())
    }
}

/// CRACKURL policy resumed with a native stack-owned FURL.
///
/// # Safety
/// `url` and `ar` must remain valid through the checked continuation.
pub unsafe fn crack_url(
    svc: &Hp2UEngineServices,
    url: *mut c_void,
    ar: *mut c_void,
) -> Result<(), i32> {
    unsafe { crack_url_with(&mut NativeCrackUrlOps { services: svc, url, output: ar }) }
}

/// `UEngine::Key(Viewport, Key)` (`UnEngine.cpp:267-272`). Returns `UBOOL`.
///
/// # Safety
/// `viewport` must be a live `UViewport`.
pub unsafe fn key(svc: &Hp2UEngineServices, viewport: *mut c_void, key: i32) -> Result<i32, i32> {
    unsafe {
        let console = value!(svc.viewport_console, viewport);
        Ok((!console.is_null() && value!(svc.console_event_key_type, console, key) != 0) as i32)
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
) -> Result<i32, i32> {
    unsafe {
        // Translate yes/no if necessary (`oldver`).
        if !value!(svc.viewport_console, viewport).is_null()
            && !value!(svc.viewport_actor, viewport).is_null()
        {
            let actor = value!(svc.viewport_actor, viewport);
            let hud = value!(svc.actor_hud, actor);
            if !hud.is_null() {
                let menu = value!(svc.hud_main_menu, hud);
                if !menu.is_null() {
                    let mut class = value!(svc.object_class, menu);
                    while !class.is_null() {
                        let name = value!(svc.class_name, class);
                        if value!(svc.app_stricmp, name, w!("UnrealQuitMenu")) == 0
                            || value!(svc.app_stricmp, name, w!("UnrealYesNoMenu")) == 0
                        {
                            break;
                        }
                        class = value!(svc.class_super, class);
                    }
                    let found = !class.is_null();
                    if found
                        && value!(svc.app_to_upper, i_key as u32) == YES_KEY.load(Ordering::Relaxed)
                    {
                        i_key = IK_Y;
                    } else if found
                        && value!(svc.app_to_upper, i_key as u32) == NO_KEY.load(Ordering::Relaxed)
                    {
                        i_key = IK_N;
                    }
                }
            }
        }

        // Process it.
        let console = value!(svc.viewport_console, viewport);
        if !console.is_null()
            && value!(svc.console_event_key_event, console, i_key, state, delta) != 0
        {
            // !! fix for continuous mouse-up events!
            if state == IST_RELEASE {
                let input = value!(svc.viewport_input, viewport);
                value!(svc.input_pre_process, input, i_key, state, delta);
            }
            // Player console handled it.
            return Ok(1);
        }

        let input = value!(svc.viewport_input, viewport);
        if value!(svc.input_pre_process, input, i_key, state, delta) != 0 {
            let input = value!(svc.viewport_input, viewport);
            let console = value!(svc.viewport_console, viewport);
            let ar = if console.is_null() {
                value!(svc.glog)
            } else {
                value!(svc.console_as_output, console)
            };
            if value!(svc.input_process, input, ar, i_key, state, delta) != 0 {
                // Input system handled it.
                return Ok(1);
            }
        }
        // Nobody handled it.
        Ok(0)
    }
}
/// Shares the source Console-first, release-preprocess, then Input processing order.
unsafe fn input_route_with<E>(
    state: i32,
    mut console_event: impl FnMut() -> Result<bool, E>,
    mut preprocess: impl FnMut() -> Result<bool, E>,
    mut process: impl FnMut() -> Result<bool, E>,
) -> Result<bool, E> {
    if console_event()? {
        if state == IST_RELEASE {
            let _ = preprocess()?;
        }
        return Ok(true);
    }
    if preprocess()? && process()? {
        return Ok(true);
    }
    Ok(false)
}

/// Dispatches canonical `UEngine::Key` through the Core-owned Console event.
///
/// # Safety
/// `viewport` must be a live canonical viewport owned by `access`.
pub(crate) unsafe fn key_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    viewport: *mut hp2_abi::engine::generated::UViewport,
    key: i32,
) -> hp2_core::core_access::RuntimeResult<bool> {
    use hp2_abi::engine::generated::{UConsole, UObject};
    use hp2_core::name_owner::FindName;

    if viewport.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Null canonical UViewport"));
    }
    let console = unsafe { (*viewport).console.cast::<UConsole>() };
    if console.is_null() {
        return Ok(false);
    }
    #[repr(C)]
    struct Params { key: u8, _pad: [u8; 3], result: u32 }
    let mut params = Params { key: key as u8, _pad: [0; 3], result: 0 };
    let object = console.cast::<UObject>();
    let class: *mut hp2_abi::engine::generated::UClass = unsafe { (*object).class.cast() };
    if class.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Canonical Console has no UClass"));
    }
    let name = access.intern_str("KeyType", FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(object, name, false)? };
    if function.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Canonical Console KeyType function is missing"));
    }
    unsafe { access.execute_event(object, function, (&mut params as *mut Params).cast(), ptr::null_mut())?; }
    Ok(params.result != 0)
}

/// Dispatches canonical `UEngine::InputEvent` through Core Console and Input owners.
///
/// # Safety
/// `viewport` must be a live canonical viewport owned by `access`.
pub(crate) unsafe fn input_event_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    viewport: *mut hp2_abi::engine::generated::UViewport,
    mut key: i32,
    state: i32,
    delta: f32,
) -> hp2_core::core_access::RuntimeResult<bool> {
    use hp2_abi::engine::generated::{AActor, UClass, UConsole, UInput, UObject};
    use hp2_core::name_owner::FindName;

    if viewport.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Null canonical UViewport"));
    }
    let console = unsafe { (*viewport).console.cast::<UConsole>() };
    let input = unsafe { (*viewport).input.cast::<UInput>() };
    if !console.is_null() {
        let actor = unsafe { (*viewport).actor.cast::<AActor>() };
        if !actor.is_null() {
            let hud = unsafe { reflected_object(access, actor.cast::<UObject>(), "myHUD")? };
            if !hud.is_null() {
                let menu = unsafe { reflected_object(access, hud, "MainMenu")? };
                if !menu.is_null() {
                    let mut class = unsafe { (*menu).class.cast::<UClass>() };
                    let mut found = false;
                    while !class.is_null() {
                        let name = i32::from_ne_bytes(unsafe { (*class.cast::<UObject>()).name });
                        let text = access.name_text(name)?;
                        if ascii_name_eq(&text, "UnrealQuitMenu")
                            || ascii_name_eq(&text, "UnrealYesNoMenu")
                        {
                            found = true;
                            break;
                        }
                        class = unsafe { (*class).super_field.cast::<UClass>() };
                    }
                    let upper = found.then(|| hp2_core::name::to_upper(key) as u32);
                    key = remap_yes_no_key(key, found, upper);
                }
            }
        }
    }
    unsafe {
        input_route_with(
            state,
            || {
                if console.is_null() { return Ok(false); }
                #[repr(C)]
                struct Params { key: u8, state: u8, delta: f32, result: u32 }
                let mut params = Params { key: key as u8, state: state as u8, delta, result: 0 };
                let object = console.cast::<UObject>();
                let class = (*object).class.cast::<hp2_abi::engine::generated::UStruct>();
                if class.is_null() {
                    return Err(hp2_core::core_access::RuntimeError::new("Canonical Console has no UClass"));
                }
                let name = access.intern_str("KeyEvent", FindName::Find)?;
                let function = access.find_object_function_by_name(object, name, false)?;
                if function.is_null() {
                    return Err(hp2_core::core_access::RuntimeError::new("Canonical Console KeyEvent function is missing"));
                }
                access.execute_event(object, function, (&mut params as *mut Params).cast(), ptr::null_mut())?;
                Ok(params.result != 0)
            },
            || Ok(!input.is_null() && crate::input::pre_process(input, key, state) != 0),
            || if input.is_null() {
                Ok(false)
            } else {
                run_core_input(access, viewport, input, key, state, delta)
            },
        )
    }
}

unsafe fn reflected_object(
    access: hp2_core::core_access::CoreAccess<'_>,
    object: *mut hp2_abi::engine::generated::UObject,
    name: &str,
) -> hp2_core::core_access::RuntimeResult<*mut hp2_abi::engine::generated::UObject> {
    use hp2_abi::engine::generated::{UClass, UProperty};
    use hp2_core::name_owner::FindName;
    if object.is_null() { return Ok(ptr::null_mut()); }
    let class = unsafe { (*object).class.cast::<UClass>() };
    if class.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Canonical input object has no UClass"));
    }
    let field_name = access.intern_str(name, FindName::Find)?;
    let property_class = access.native_class("ObjectProperty")?;
    let property = unsafe { access.find_field(class.cast(), field_name, property_class) }.cast::<UProperty>();
    if property.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Canonical input object property is missing").with_context(name));
    }
    let offset = unsafe { (*property).offset };
    if offset < 0 {
        return Err(hp2_core::core_access::RuntimeError::new("Canonical input object property has a negative offset").with_context(name));
    }
    Ok(unsafe { ptr::read_unaligned(object.cast::<u8>().add(offset as usize).cast()) })
}

unsafe fn run_core_input(
    access: hp2_core::core_access::CoreAccess<'_>,
    _viewport: *mut hp2_abi::engine::generated::UViewport,
    input: *mut hp2_abi::engine::generated::UInput,
    key: i32,
    state: i32,
    delta: f32,
) -> hp2_core::core_access::RuntimeResult<bool> {
    let category = access.intern_str("Log", hp2_core::name_owner::FindName::Find)?;
    let mut output = |message: &[u32]| access.emit_log(category, message);
    Ok(crate::input_owner::process(access, input, key, state, delta, &mut output)? != 0)
}

fn remap_yes_no_key(key: i32, found_menu: bool, upper_key: Option<u32>) -> i32 {
    if !found_menu { return key; }
    match upper_key {
        Some(key) if key == YES_KEY.load(Ordering::Relaxed) => IK_Y,
        Some(key) if key == NO_KEY.load(Ordering::Relaxed) => IK_N,
        _ => key,
    }
}

fn ascii_name_eq(text: &[u32], expected: &str) -> bool {
    text.len() == expected.len()
        && text.iter().zip(expected.bytes()).all(|(left, right)| {
            hp2_core::name::to_upper(*left as i32) == hp2_core::name::to_upper(u32::from(right) as i32)
        })
}


/// `UServerCommandlet::Main(Parms)` (`UnEngine.cpp:346-394`). `Parms` is
/// unused in the original and not passed. Returns `INT`.
///
/// # Safety
/// Services must be valid.
pub unsafe fn commandlet_main(svc: &Hp2UEngineServices) -> Result<i32, i32> {
    unsafe {
        let mut temp = [0u32; LANGUAGE_BUF_LEN as usize];
        if value!(svc.config_get_language, temp.as_mut_ptr(), LANGUAGE_BUF_LEN) != 0 {
            call!(svc.set_language, temp.as_ptr());
        }
        let class = value!(svc.load_engine_class);
        let engine = value!(svc.construct_engine, class);
        call!(svc.engine_init, engine);
        call!(svc.set_is_running, 1);
        let old_time = value!(svc.app_seconds);
        let mut frame = CommandletFrame {
            svc,
            engine,
            old_time,
            second_start_time: old_time,
            tick_count: 0,
        };
        let context = (&raw mut frame).cast();
        while value!(svc.is_running) != 0 && value!(svc.is_requesting_exit) == 0 {
            call!(svc.commandlet_scope, context, 0);
            call!(svc.commandlet_scope, context, 1);
        }
        call!(svc.set_is_running, 0);
        Ok(0)
    }
}

struct CommandletFrame<'a> {
    svc: &'a Hp2UEngineServices,
    engine: *mut c_void,
    old_time: i64,
    second_start_time: i64,
    tick_count: i32,
}

/// Resume the Rust loop policy inside the native per-iteration guard.
///
/// # Safety
/// `context` is the live stack frame created by `commandlet_main`; `entry` is 0 or 1.
pub unsafe fn commandlet_scope(context: *mut c_void, entry: u32) -> Result<(), i32> {
    unsafe {
        let frame = context.cast::<CommandletFrame<'_>>();
        let svc = (*frame).svc;
        let engine = (*frame).engine;
        if entry == 0 {
            let new_time = value!(svc.app_seconds);
            call!(
                svc.engine_tick,
                engine,
                value!(svc.ftime_sub, new_time, (*frame).old_time)
            );
            (*frame).old_time = new_time;
            (*frame).tick_count = (*frame).tick_count.wrapping_add(1);
            let since_second = value!(svc.ftime_sub, (*frame).old_time, (*frame).second_start_time);
            if since_second > 1.0 {
                let denominator =
                    value!(svc.ftime_sub, (*frame).old_time, (*frame).second_start_time);
                call!(
                    svc.set_current_tick_rate,
                    engine,
                    (*frame).tick_count as f32 / denominator
                );
                (*frame).second_start_time = (*frame).old_time;
                (*frame).tick_count = 0;
            }
        } else {
            let max_tick_rate = value!(svc.engine_get_max_tick_rate, engine);
            if max_tick_rate > 0.0 {
                let delta = (1.0f32 / max_tick_rate)
                    - value!(svc.ftime_sub, value!(svc.app_seconds), (*frame).old_time);
                let sleep = if 0.0f32 >= delta { 0.0f32 } else { delta };
                call!(svc.app_sleep, sleep);
            }
        }
        Ok(())
    }
}
