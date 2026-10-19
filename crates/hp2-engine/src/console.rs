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
    /// `viewport->Exec(cmd, *console)`. Returns `UBOOL`.
    pub viewport_exec: unsafe extern "C" fn(
        viewport: *mut c_void,
        cmd: *const c_void,
        console: *mut c_void,
    ) -> i32,
    /// `viewport->CreateNativeFont(name, height)`. Returns `UFont*`.
    pub viewport_create_native_font: unsafe extern "C" fn(
        viewport: *mut c_void,
        name: *const c_void,
        height: i32,
    ) -> *mut c_void,
    /// `viewport->CurrentTime - viewport->LastUpdateTime` (`FTime::operator-`,
    /// inlined in the original PreRender; C++ reproduces it in the adapter).
    pub viewport_time_delta: unsafe extern "C" fn(viewport: *mut c_void) -> f32,
    /// `appSaveStringToFile(*string, filename, GFileManager)`. `string` is `FString*`.
    pub save_string_to_file: unsafe extern "C" fn(string: *const c_void, filename: *const c_void),
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
static FPS_FILE: [u32; 8] = tchar(b"fps.txt");

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
/// # Safety
/// `console` must point to a live `UConsole`; `cmd` must be a `TCHAR*`.
pub unsafe fn exec_command(
    svc: &Hp2ConsoleServices,
    console: *mut UConsole,
    cmd: *const c_void,
) -> i32 {
    unsafe {
        let viewport = (*console).viewport;
        (svc.viewport_exec)(viewport, cmd, console.cast())
    }
}

/// `execSaveTimeDemo` body (UnCon.cpp:140): writes `fps.txt`.
///
/// # Safety
/// `string` must point to a live `FString`.
pub unsafe fn save_time_demo(svc: &Hp2ConsoleServices, string: *const c_void) {
    unsafe { (svc.save_string_to_file)(string, FPS_FILE.as_ptr().cast()) }
}

/// `execCreateNativeFont` body (UnCon.cpp:152): `Viewport->CreateNativeFont`.
///
/// # Safety
/// `console` must point to a live `UConsole`; `name` must be a `TCHAR*`.
pub unsafe fn create_native_font(
    svc: &Hp2ConsoleServices,
    console: *mut UConsole,
    name: *const c_void,
    height: i32,
) -> *mut c_void {
    unsafe {
        let viewport = (*console).viewport;
        (svc.viewport_create_native_font)(viewport, name, height)
    }
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
        // Prevent status redraw due to changing.
        let viewport = (*console).viewport;
        let delta = (svc.viewport_time_delta)(viewport);
        (svc.event_tick)(console.cast(), delta);

        // Save the Viewport.
        store_saved_frame(*frame);

        // Compute new status info.
        (*console).border_lines = 0;
        (*console).border_pixels = 0;
        (*console).console_lines = 0;

        // Compute sizing of all visible status bar components.
        if (*console).console_pos > 0.0 {
            // Show console.
            let y = (*frame).y as f32;
            (*console).console_lines = app_round(min_f((*console).console_pos * y, y));
        }

        let border_size = (*console).border_size;
        if border_size >= 2 {
            // Encroach on screen area.
            let fraction = border_fraction(border_size);

            let y = (*frame).y;
            (*console).border_lines = app_round(min_f(y as f32 * 0.25 * fraction, y as f32));
            (*console).border_lines = max_i(0, (*console).border_lines);
            (*frame).y = (*frame)
                .y
                .wrapping_sub(2i32.wrapping_mul((*console).border_lines));

            let x = (*frame).x;
            // `& ~3` clears the low two bits (UnCon.cpp:207).
            (*console).border_pixels = app_round(min_f(x as f32 * 0.25 * fraction, x as f32)) & !3;
            (*frame).x = (*frame)
                .x
                .wrapping_sub(2i32.wrapping_mul((*console).border_pixels));
        }

        (*frame).xb = (*frame).xb.wrapping_add((*console).border_pixels);
        (*frame).yb = (*frame).yb.wrapping_add((*console).border_lines);
        (svc.frame_compute_render_size)(frame.cast());
    }
}

/// `UConsole::PostRender` body (UnCon.cpp:227-230).
///
/// # Safety
/// `console` must point to a live `UConsole`; `frame` to a live `FSceneNode`.
pub unsafe fn post_render(console: *mut UConsole, frame: *mut FSceneNode) {
    unsafe {
        *frame = saved_frame().0;
        (*console).frame_x = (*frame).x as f32;
        (*console).frame_y = (*frame).y as f32;
        let drew = !(*console).b_no_draw_world();
        (*console).set_b_drew_world(drew);
    }
}
