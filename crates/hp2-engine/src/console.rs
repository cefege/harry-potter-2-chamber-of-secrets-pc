//! Port of `HarryPotter2/Unreal/Engine/Src/UnCon.cpp` (`UConsole`).
//!
//! The C++ `UConsole` methods keep their UObject plumbing (class
//! registration, `P_GET_*`/`P_FINISH`, `RESULT_DECL`, `guard`/`unguard`) and
//! forward here. Every engine call goes through [`Hp2ConsoleServices`] in the
//! original order. `TCHAR` is `wchar_t` (4 bytes) on this build, so text
//! literals are `u32` code-unit arrays (`HP2_HOST_WCHAR_TCHAR`,
//! `Core/Inc/UnGnuG.h`); the C++ side never sees them as `u16`.
//!
//! Engine objects cross as raw `void*` and are re-borrowed per statement,
//! never held across a service call, because a service may mutate the same
//! object (`eventTick` runs script).

use std::ffi::{c_char, c_void};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::math::app_round;
use hp2_abi::engine::generated::{FSceneNode, UConsole};
use hp2_core::core_access::{RuntimeError, RuntimeResult};

/// `UConsole::MAX_BORDER` (`UnCon.h`, private).
pub const MAX_BORDER: i32 = 6;
/// `UConsole::MAX_LINES` (`UnCon.h`, private).
pub const MAX_LINES: i32 = 64;

/// Engine calls made by the console port, in the C++ adapter table
/// `GHp2ConsoleServices` (`Engine/Src/Hp2ConsoleServices.cpp`). Pointer
/// arguments are `void*` so the table stays a plain C ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ConsoleServices {
    /// `UConsole::InitExecution()`.
    pub init_execution: unsafe extern "C" fn(console: *mut c_void),
    /// `LocalizeGeneral(key, package)` with an ANSI key. Returns `const TCHAR*`.
    pub localize_general:
        unsafe extern "C" fn(key: *const c_char, package: *const c_void) -> *const c_void,
    /// `console->Logf(format)` (no arguments). `format` is `const TCHAR*`.
    pub log_format: unsafe extern "C" fn(console: *mut c_void, format: *const c_void),
    /// `console->eventMessage(0, data, 0, name)` (ProcessEvent `Message`).
    pub event_message: unsafe extern "C" fn(console: *mut c_void, data: *const c_void, name: i32),
    /// `console->eventTick(delta_time)` (ProcessEvent `ENGINE_Tick`).
    pub event_tick: unsafe extern "C" fn(console: *mut c_void, delta_time: f32),
    /// `viewport->Exec(cmd, *console)`. Hp2FfiCall-wrapped: returns a
    /// nonzero status on a C++ throw (caught and stashed by the leaf); the
    /// `UBOOL` result is written through `out` only on status `0`.
    pub viewport_exec: unsafe extern "C" fn(
        viewport: *mut c_void,
        cmd: *const c_void,
        console: *mut c_void,
        out: *mut i32,
    ) -> i32,
    /// `viewport->CreateNativeFont(name, height)`. Hp2FfiCall-wrapped: the
    /// `UFont*` result is written through `out` only on status `0`.
    pub viewport_create_native_font: unsafe extern "C" fn(
        viewport: *mut c_void,
        name: *const c_void,
        height: i32,
        out: *mut *mut c_void,
    ) -> i32,
    /// `viewport->CurrentTime - viewport->LastUpdateTime` (`FTime::operator-`,
    /// inlined in the original PreRender; C++ reproduces it in the adapter).
    pub viewport_time_delta: unsafe extern "C" fn(viewport: *mut c_void) -> f32,
    /// `appSaveStringToFile(*string, filename, GFileManager)`. `string` is
    /// `FString*`. Hp2FfiCall-wrapped: returns a nonzero status on a C++
    /// throw.
    pub save_string_to_file:
        unsafe extern "C" fn(string: *const c_void, filename: *const c_void) -> i32,
    /// `FSceneNode::ComputeRenderSize()` on `frame` (`FSceneNode*`).
    pub frame_compute_render_size: unsafe extern "C" fn(frame: *mut c_void),
}

/// `TEXT("...")` under `HP2_HOST_WCHAR_TCHAR`: one `u32` per character,
/// zero-terminated by the zero padding of `N`.
const fn tchar<const N: usize>(s: &[u8]) -> [u32; N] {
    let mut out = [0u32; N];
    let mut i = 0;
    while i < s.len() {
        out[i] = s[i] as u32;
        i += 1;
    }
    out
}

/// `TEXT("Core")`.
static CORE_PACKAGE: [u32; 5] = tchar(b"Core");
/// `TEXT(" ")`.
static SPACE: [u32; 2] = tchar(b" ");
/// `TEXT("fps.txt")`.
pub(crate) const FPS_FILENAME: &str = "fps.txt";
static FPS_FILE: [u32; FPS_FILENAME.len() + 1] = tchar(FPS_FILENAME.as_bytes());

/// `FSceneNode` copy held between PreRender and PostRender (`SavedFrame` in
/// UnCon.cpp). Zero-initialized like the C++ global.
struct SavedFrame(FSceneNode);

// SAFETY: the copy holds raw engine pointers that are only copied, never
// dereferenced by the console port.
unsafe impl Send for SavedFrame {}

static SAVED_FRAME: Mutex<SavedFrame> = Mutex::new(SavedFrame(unsafe { std::mem::zeroed() }));

fn saved_frame() -> MutexGuard<'static, SavedFrame> {
    SAVED_FRAME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `Min(A,B)` (`UnTemplate.h`): `(A<=B)?A:B`. NaN in `A` yields `B`.
#[inline]
pub fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `::Max(A,B)` on `INT` (`UnTemplate.h`): `(A>=B)?A:B`.
#[inline]
pub fn max_i(a: i32, b: i32) -> i32 {
    if a >= b { a } else { b }
}

/// `Fraction = (FLOAT)(BorderSize-1) / (FLOAT)(MAX_BORDER-1)` (UnCon.cpp:201).
#[inline]
pub fn border_fraction(border_size: i32) -> f32 {
    (border_size - 1) as f32 / (MAX_BORDER - 1) as f32
}

/// `UConsole::_Init` body after `VERIFY_CLASS_SIZE` (UnCon.cpp:91-102).
///
/// # Safety
/// `console` must point to a live `UConsole`. `viewport` is stored as-is.
pub unsafe fn init(svc: &Hp2ConsoleServices, console: *mut UConsole, viewport: *mut c_void) {
    unsafe {
        (*console).viewport = viewport;
        (*console).top_line = MAX_LINES - 1;
        (*console).border_size = 1;

        (svc.init_execution)(console.cast());

        let engine = (svc.localize_general)(c"Engine".as_ptr(), CORE_PACKAGE.as_ptr().cast());
        (svc.log_format)(console.cast(), engine);
        let copyright = (svc.localize_general)(c"Copyright".as_ptr(), CORE_PACKAGE.as_ptr().cast());
        (svc.log_format)(console.cast(), copyright);
        (svc.log_format)(console.cast(), SPACE.as_ptr().cast());
        (svc.log_format)(console.cast(), SPACE.as_ptr().cast());
    }
}

/// `UConsole::Serialize(Data, ThisType)` (UnCon.cpp:118): `eventMessage`.
///
/// # Safety
/// `console` must point to a live `UConsole`; `data` must be a `TCHAR*`.
pub unsafe fn serialize(
    svc: &Hp2ConsoleServices,
    console: *mut UConsole,
    data: *const c_void,
    name: i32,
) {
    unsafe { (svc.event_message)(console.cast(), data, name) }
}

/// `execConsoleCommand` body (UnCon.cpp:129): `Viewport->Exec(*S, *this)`.
/// Returns the `UBOOL` the caller stores into `Result`.
///
/// Divergence: a null `Viewport` is dereferenced by the original (crash);
/// the port forwards the null pointer to the same call, which crashes alike.
///
/// `viewport_exec` is Hp2FfiCall-wrapped on the C++ side: `Err` carries the
/// nonzero status on a C++ throw, with no result written.
///
/// # Safety
/// `console` must point to a live `UConsole`; `cmd` must be a `TCHAR*`.
pub unsafe fn exec_command(
    svc: &Hp2ConsoleServices,
    console: *mut UConsole,
    cmd: *const c_void,
) -> Result<i32, i32> {
    unsafe { exec_command_with(console, cmd, |viewport, command, output| {
        let mut value = 0;
        let status = (svc.viewport_exec)(viewport, command, output, &raw mut value);
        if status == 0 { Ok(value) } else { Err(status) }
    }) }
}

/// `execSaveTimeDemo` body (UnCon.cpp:140): writes `fps.txt`.
///
/// `save_string_to_file` is Hp2FfiCall-wrapped on the C++ side: `Err`
/// carries the nonzero status on a C++ throw.
///
/// # Safety
/// `string` must point to a live `FString`.
pub unsafe fn save_time_demo(svc: &Hp2ConsoleServices, string: *const c_void) -> Result<(), i32> {
    save_time_demo_with(string, |value, filename| {
        let status = unsafe { (svc.save_string_to_file)(value, filename) };
        if status == 0 { Ok(()) } else { Err(status) }
    })
}

/// `execCreateNativeFont` body (UnCon.cpp:152): `Viewport->CreateNativeFont`.
///
/// `viewport_create_native_font` is Hp2FfiCall-wrapped on the C++ side:
/// `Err` carries the nonzero status on a C++ throw, with no result written.
///
/// # Safety
/// `console` must point to a live `UConsole`; `name` must be a `TCHAR*`.
pub unsafe fn create_native_font(
    svc: &Hp2ConsoleServices,
    console: *mut UConsole,
    name: *const c_void,
    height: i32,
) -> Result<*mut c_void, i32> {
    unsafe { create_native_font_with(console, name, height, |viewport, name, height| {
        let mut value = std::ptr::null_mut();
        let status = (svc.viewport_create_native_font)(viewport, name, height, &raw mut value);
        if status == 0 { Ok(value) } else { Err(status) }
    }) }
}

/// Shares the Console receiver/output route with a typed viewport implementation.
///
/// # Safety
/// Console, command and its viewport remain live through the callback.
pub(crate) unsafe fn exec_command_with<E>(
    console: *mut UConsole, command: *const c_void,
    exec: impl FnOnce(*mut c_void, *const c_void, *mut c_void) -> Result<i32, E>,
) -> Result<i32, E> {
    unsafe { exec((*console).viewport.cast(), command, console.cast()) }
}

/// Shares the source filename without requiring unrelated Console services.
pub(crate) fn save_time_demo_with<E>(
    string: *const c_void,
    save: impl FnOnce(*const c_void, *const c_void) -> Result<(), E>,
) -> Result<(), E> {
    save(string, FPS_FILE.as_ptr().cast())
}

/// Shares Console-to-viewport native font dispatch.
///
/// # Safety
/// Console, font name and its viewport remain live through the callback.
pub(crate) unsafe fn create_native_font_with<E>(
    console: *mut UConsole, name: *const c_void, height: i32,
    create: impl FnOnce(*mut c_void, *const c_void, i32) -> Result<*mut c_void, E>,
) -> Result<*mut c_void, E> {
    unsafe { create((*console).viewport.cast(), name, height) }
}

/// `UConsole::GetDrawWorld` (UnCon.cpp:165): `!bNoDrawWorld`.
///
/// # Safety
/// `console` must point to a live `UConsole`.
pub unsafe fn get_draw_world(console: *const UConsole) -> i32 {
    unsafe { i32::from(!(*console).b_no_draw_world()) }
}

/// Stores a copy of the frame (`SavedFrame = *Frame`).
fn store_saved_frame(frame: FSceneNode) {
    saved_frame().0 = frame;
}

/// `UConsole::PreRender` body (UnCon.cpp:178-215).
///
/// Divergence: `2 * BorderLines`, `Frame->Y -=`, `Frame->X -=`, `XB +=`,
/// `YB +=` are signed `INT` overflow (UB) in the original; the port wraps.
///
/// # Safety
/// `console` must point to a live `UConsole`; `frame` to a live `FSceneNode`.
pub unsafe fn pre_render(svc: &Hp2ConsoleServices, console: *mut UConsole, frame: *mut FSceneNode) {
    unsafe {
        let viewport = (*console).viewport;
        let delta = (svc.viewport_time_delta)(viewport);
        let result = pre_render_with(
            console,
            frame,
            &mut saved_frame().0,
            delta,
            |console, delta| {
                (svc.event_tick)(console.cast(), delta);
                Ok::<(), RuntimeError>(())
            },
            |frame| {
                (svc.frame_compute_render_size)(frame.cast());
                Ok(())
            },
        );
        result.expect("Native Console callbacks are infallible");
    }
}

/// Applies Console PreRender using caller-owned frame storage and callbacks.
///
/// # Safety
/// Both pointers must identify live Console and scene-frame records.
pub unsafe fn pre_render_with(
    console: *mut UConsole,
    frame: *mut FSceneNode,
    saved_frame: &mut FSceneNode,
    delta: f32,
    event_tick: impl FnOnce(*mut UConsole, f32) -> RuntimeResult<()>,
    compute_render_size: impl FnOnce(*mut FSceneNode) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    unsafe {
        event_tick(console, delta)?;
        *saved_frame = *frame;
        (*console).border_lines = 0;
        (*console).border_pixels = 0;
        (*console).console_lines = 0;
        if (*console).console_pos > 0.0 {
            let y = (*frame).y as f32;
            (*console).console_lines = app_round(min_f((*console).console_pos * y, y));
        }
        let border_size = (*console).border_size;
        if border_size >= 2 {
            let fraction = border_fraction(border_size);
            let y = (*frame).y;
            (*console).border_lines = app_round(min_f(y as f32 * 0.25 * fraction, y as f32));
            (*console).border_lines = max_i(0, (*console).border_lines);
            (*frame).y = (*frame).y.wrapping_sub(2i32.wrapping_mul((*console).border_lines));
            let x = (*frame).x;
            (*console).border_pixels = app_round(min_f(x as f32 * 0.25 * fraction, x as f32)) & !3;
            (*frame).x = (*frame).x.wrapping_sub(2i32.wrapping_mul((*console).border_pixels));
        }
        (*frame).xb = (*frame).xb.wrapping_add((*console).border_pixels);
        (*frame).yb = (*frame).yb.wrapping_add((*console).border_lines);
        compute_render_size(frame)?;
        Ok(())
    }
}

/// `UConsole::PostRender` body (UnCon.cpp:227-230).
///
/// # Safety
/// `console` must point to a live `UConsole`; `frame` to a live `FSceneNode`.
pub unsafe fn post_render(console: *mut UConsole, frame: *mut FSceneNode) {
    unsafe { post_render_with(console, frame, &saved_frame().0) }
}

/// Applies Console PostRender using caller-owned saved frame storage.
///
/// # Safety
/// Both pointers must identify live Console and scene-frame records.
pub unsafe fn post_render_with(
    console: *mut UConsole,
    frame: *mut FSceneNode,
    saved_frame: &FSceneNode,
) {
    unsafe {
        *frame = *saved_frame;
        (*console).frame_x = (*frame).x as f32;
        (*console).frame_y = (*frame).y as f32;
        let drew = !(*console).b_no_draw_world();
        (*console).set_b_drew_world(drew);
    }
}

static mut CONSOLE_SERVICES: Option<Hp2ConsoleServices> = None;

/// # Safety
/// `value` is the real, immutable `GHp2ConsoleServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2ConsoleServices) {
    unsafe { (&raw mut CONSOLE_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Result<Hp2ConsoleServices, i32> {
    unsafe { (&raw const CONSOLE_SERVICES).read().ok_or(1) }
}
