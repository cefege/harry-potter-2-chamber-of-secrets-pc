//! Viewport policy and checked service-table adapters. The canonical Engine
//! path uses `hp2-engine::platform_owner::set_mouse_capture` to call
//! `set_mouse_capture_with` when `sdl` is enabled. That owner supplies the
//! live viewport state, window-flags query, and mouse-grab update callbacks.
//! Other policies and adapters in this module have their own call contracts.
use crate::input_pump as pump;
use crate::viewport;
use std::ffi::c_void;

/// Runs the source focus-only mouse capture transition against live owner state.
/// A missing window does not suppress capture, matching USDLViewport.
pub fn set_mouse_capture_with<E>(
    capture: i32,
    focus_only: i32,
    mut set_lost_grab: impl FnMut(bool) -> Result<(), E>,
    mut window_flags: impl FnMut() -> Result<Option<u32>, E>,
    mut update_grab: impl FnMut(i32) -> Result<(), E>,
) -> Result<(), E> {
    let mut capture = capture;
    if capture == 0 {
        set_lost_grab(false)?;
    } else if focus_only != 0
        && window_flags()?.is_some_and(|flags| flags & pump::sdl_window_flags::INPUT_FOCUS == 0)
    {
        capture = 0;
        set_lost_grab(true)?;
    }
    update_grab(capture)
}

/// One live scalar access, not an offset. Client fields use the invocation's
/// saved C, established by BindClient; viewport fields always use this.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformViewportField {
    SizeX,
    SizeY,
    ColorBytes,
    BlitFlags,
    LostGrab,
    LostFullscreen,
    MouseIsGrabbed,
    WindowsMouseAvailable,
    LastJoyHat,
    SelectedCursor,
    MouseScaleX,
    MouseScaleY,
    WindowedViewportX,
    WindowedViewportY,
    WindowedColorBits,
    FullscreenViewportX,
    FullscreenViewportY,
    FullscreenColorBits,
    BorderlessWindow,
    StartupFullscreen,
    UseDesktopResolution,
    /// Readable via the client's existing CaptureMouse field (SDLClient.cpp).
    CaptureMouse,
    /// Readable via the client's existing JoyHatFoldsToButtons field.
    JoyHatFoldsToButtons,
    /// Readable via the client's existing JoystickDeadZone field.
    JoystickDeadZone,
    /// Readable via the client's existing ScaleJBX field.
    ScaleJBX,
    /// Readable via the client's existing ScaleJBY field.
    ScaleJBY,
}

/// Checked leaves never contain policy loops or substitute engine behavior.
/// Inputs and outputs name members of Hp2PlatformViewportValue. Unmentioned
/// inputs are zero; unmentioned outputs are ignored. Store output only on success.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformViewportLeaf {
    /// Save C = GetOuterUSDLClient() in this invocation's native frame.
    BindClient,
    /// Output.object = this->SdlWindow (live, including null).
    Window,
    /// Output.integer = this->RenDev != NULL.
    HasRenderDevice,
    /// Output.integer = this->Actor != NULL.
    HasActor,
    /// Output.integer = this->Input != NULL.
    HasInput,
    /// Output.object = GetOuterUSDLClient()->Engine; do not use saved C.
    Engine,
    /// RenDev->Exit(), reading RenDev at this call, not at HasRenderDevice.
    RenderExit,
    /// delete RenDev, reading it AFTER Exit or failure logging can replace it.
    RenderDelete,
    /// RenDev = NULL.
    RenderClear,
    /// RenDev->ConditionalShutdownAfterError(), not bare ShutdownAfterError.
    RenderShutdownAfterError,
    /// SdlWindow = NULL.
    WindowClear,
    /// Write-only: SystemCursors[input.cursor.slot] = input.cursor.object. No
    /// SDL call; stores the pointer Rust already created or cleared.
    CursorStore,
    /// Input.integer is slot; `output.object = SystemCursors[slot]`.
    CursorAt,
    /// Input.integer is slot; `SystemCursors[slot] = NULL`.
    CursorClear,
    /// Caps |= CC_RGB565; no other cap changes.
    AddRgb565Cap,
    /// Original NAME_Init compiled/linked SDL version debugf; input.version
    /// carries both triples (Rust computes/reads them; SDL_GetVersion and
    /// the pinned compiled-against constants are no longer native).
    LogVersion,
    /// Super::Destroy().
    SuperDestroy,
    /// Super::ShutdownAfterError().
    SuperShutdownAfterError,
    /// RenDev->Lock with original invocation's FPlanes/flags/hit pointers.
    RenderLock,
    /// CurrentTime = appSeconds(), preserving the native FTime assignment.
    StampCurrentTime,
    /// FrameCount++, live value.
    IncrementFrameCount,
    /// RenDev->Unlock(input.integer), retaining exact UBOOL value.
    RenderUnlock,
    /// LastUpdateTime = CurrentTime, native FTime assignment after Unlock.
    CopyLastUpdateTime,
    /// input.draw.engine is saved ClientEngine; Draw(this,input.draw.blit),
    /// not a fresh C->Engine read.
    EngineDraw,
    /// Exact native expression:
    /// RenDev->SetRes(SizeX,SizeY,ColorBytes,IsFullscreen()).
    /// Preserve compiler/platform argument evaluation and virtual IsFullscreen
    /// redispatch via an independent frame; catch override failures below Rust.
    /// Ignore the native result. Rust owns the surrounding presence condition.
    RenderSetRes,
    /// Write-only: SdlWindow = input.object. No SDL call; stores the pointer
    /// Rust already created (possibly null on failure).
    WindowStore,
    /// Original appErrorf("SDL_CreateWindow failed: %s", ANSI_TO_TCHAR(
    /// input.object)). input.object is SDL_GetError()'s NUL-terminated
    /// buffer, read by Rust (SDL_GetError is no longer native); only the
    /// appErrorf/ANSI_TO_TCHAR engine-string reporting stays native.
    WindowCreateError,
    /// ParseParam(appCmdLine(),TEXT("nohard")); output.integer.
    NoHardware,
    /// Original appErrorf("No render device could be initialized (see the
    /// WgpuDrv messages above for the failing reason)").
    NoRenderDeviceError,
    /// Native this->TryRenderDevice with input.render; candidate selects the
    /// original ClassName or the exact ini string. Preserves the native call's
    /// unwind label (now emitted by hp2-ffi's `native_guard::guarded` around
    /// this reentry's own action, not a C++ method guard).
    InvokeTryRenderDevice,
    /// Exact OpenWindow expression: this->TryRenderDevice(game ini,SizeX,
    /// SizeY,ColorBytes,IsFullscreen()). Keep native argument evaluation and
    /// virtual IsFullscreen reentry; Rust owns the nohard/presence conditions.
    InvokeOpenRenderDevice,
    /// Native this->ResizeViewport(input.resize...), output.integer is UBOOL.
    InvokeResize,
    /// Native this->UpdateMouseGrabState(input.integer); the unwind label is
    /// hp2-ffi's `native_guard::guarded`, not a C++ method guard.
    InvokeMouseGrab,
    /// Native this->ReleaseAllInput(); the unwind label is hp2-ffi's
    /// `native_guard::guarded`, not a C++ method guard.
    InvokeReleaseAll,
    /// Native this->CauseInputEvent(input.cause...), output.integer is UBOOL.
    InvokeCauseInput,
    /// Native this->ToggleFullscreen(); the unwind label is hp2-ffi's
    /// `native_guard::guarded`, not a C++ method guard.
    InvokeToggleFullscreen,
    /// Native this->EndFullscreen(); the unwind label is hp2-ffi's
    /// `native_guard::guarded`, not a C++ method guard.
    InvokeEndFullscreen,
    /// Native this->IsFullscreen(), retaining virtual dispatch; output.integer.
    InvokeIsFullscreen,
    /// saved C->Engine->Key(this,input.integer), live Engine, no new null gate.
    EngineKey,
    /// saved C->Engine->MousePosition(this,input.mouse...), no new null gate.
    EngineMousePosition,
    /// GetOuterUSDLClient()->Engine->InputEvent(this,input.cause...), live Engine
    /// after CauseInputEvent's checks; output.integer is exact native UBOOL.
    EngineInputEvent,
    /// Input->KeyDown(input.integer), live Input; output.integer. No new gate.
    KeyDown,
    /// `ConstructObject<UFont>(UFont::StaticClass(),GetOuter())`; output.object.
    ConstructFont,
    /// input.object is saved Font; Font->FontName = original FontName argument.
    SetFontName,
    /// input.object is saved Font; Font->FontHeight = original Height argument.
    SetFontHeight,
    /// input.object is saved Font; Font->NativeFont = NULL.
    ClearNativeFont,
    /// ParseCommand(&frame.Cmd, typed input.command's exact text), output.integer.
    /// Keep the possibly advanced Cmd pointer for every later parse/SuperExec.
    ParseCommand,
    /// Ar.Logf using input.log's original format and live native fields/global
    /// config (including its null ternary). Color bits use ColorBytes * 8.
    LogCommand,
    /// Super::Exec(frame.Cmd,frame.Ar), output.integer is original UBOOL.
    SuperExec,
    /// appStricmp(original ClassName, input.candidate ini text); output.integer
    /// is comparison result. Called once only for the applicable fallback.
    CompareRenderClass,
    /// StaticLoadClass(URenderDevice::StaticClass(),NULL,candidate,NULL,
    /// LOAD_NoFail,NULL); output.object. Native class pointer stays opaque.
    LoadRenderClass,
    /// `RenDev = ConstructObject<URenderDevice>((UClass*)input.object,this)`.
    ConstructRenderDevice,
    /// RenDev->Init(this,input.render.x,y,color,fullscreen), live RenDev;
    /// output.integer is exact UBOOL, not RenDev presence.
    RenderInit,
    /// Output.integer = GIsRunning (read only after successful Init).
    IsRunning,
    /// Actor->GetLevel()->DetailChange(RenDev->HighDetailActors), live fields.
    DetailChange,
    /// debugf(NAME_Log,"Render device %s failed to initialize",candidate).
    LogRenderFailure,
    /// GRenderDevice = RenDev, live pointer, even when null.
    PublishRenderDevice,
    /// Within TitleUtf8 scope: TextToUtf8(original Title,actual TArray Utf8),
    /// retaining its exact AddZeroed/appToUtf8InPlace behavior.
    ConvertTitle,
    /// output.bytes = {&Utf8(0),strlen(&Utf8(0))}, valid throughout TitleUtf8.
    TitleBytes,
}

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformViewportCandidate {
    Primary,
    Windowed,
    Game,
}
/// Exact ParseCommand text, in source order.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformViewportCommand {
    GetRes,
    GetCurrentRes,
    GetCurrentColorDepth,
    GetCurrentBitDepth,
    ToggleFullscreen,
    DumpCapturedMouse,
    GetSystemIni,
    GetUserIni,
    RelaunchSupport,
}
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformViewportLog {
    Resolution,
    ColorDepth,
    BitDepth,
    Mouse,
    SystemIni,
    UserIni,
}

/// Native scope callback constructs its actual typed local, then invokes the
/// matching Rust phase synchronously with a service table pointing at that
/// scope's native frame. It returns the nested status unchanged. Destruction
/// occurs on scope exit, never before the nested call. TitleUtf8 owns
/// `TArray<ANSICHAR>` from before conversion until after SDL title. Native
/// exceptions/signals must be contained at EVERY native callback and stored
/// in the original invocation; rethrow only after the Rust caller exits.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformViewportPhase {
    TitleUtf8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportCursor {
    pub slot: i32,
    pub object: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportVersion {
    pub compiled_major: i32,
    pub compiled_minor: i32,
    pub compiled_patch: i32,
    pub linked_major: i32,
    pub linked_minor: i32,
    pub linked_patch: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportOpen {
    pub temporary: i32,
    pub x: i32,
    pub y: i32,
    pub open_x: i32,
    pub open_y: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportResize {
    pub flags: u32,
    pub x: i32,
    pub y: i32,
    pub color: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportRender {
    pub candidate: Hp2PlatformViewportCandidate,
    pub x: i32,
    pub y: i32,
    pub color: i32,
    pub fullscreen: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportCause {
    pub key: i32,
    pub action: i32,
    pub delta: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportMouse {
    pub buttons: u32,
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportCapture {
    pub capture: i32,
    pub focus_only: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportDraw {
    pub engine: *mut c_void,
    pub blit: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportBytes {
    pub data: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Hp2PlatformViewportValue {
    pub integer: i32,
    pub object: *mut c_void,
    pub cursor: Hp2PlatformViewportCursor,
    pub version: Hp2PlatformViewportVersion,
    pub open: Hp2PlatformViewportOpen,
    pub resize: Hp2PlatformViewportResize,
    pub render: Hp2PlatformViewportRender,
    pub cause: Hp2PlatformViewportCause,
    pub mouse: Hp2PlatformViewportMouse,
    pub capture: Hp2PlatformViewportCapture,
    pub draw: Hp2PlatformViewportDraw,
    pub bytes: Hp2PlatformViewportBytes,
    pub candidate: Hp2PlatformViewportCandidate,
    pub command: Hp2PlatformViewportCommand,
    pub log: Hp2PlatformViewportLog,
}
impl Default for Hp2PlatformViewportValue {
    fn default() -> Self {
        Self { integer: 0 }
    }
}

/// All callbacks return zero or the exact first native failure status. No
/// exception may unwind through Rust. Scalar callbacks do one live access;
/// scope/call pointers and input/output storage must not be retained. Copy the
/// table before invoking Rust; callback reentry must not reuse invocation frames.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformViewportServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(
        *mut c_void,
        Hp2PlatformViewportLeaf,
        *const Hp2PlatformViewportValue,
        *mut Hp2PlatformViewportValue,
    ) -> i32,
    pub read_integer: unsafe extern "C" fn(*mut c_void, Hp2PlatformViewportField, *mut i32) -> i32,
    pub write_integer: unsafe extern "C" fn(*mut c_void, Hp2PlatformViewportField, i32) -> i32,
    pub read_float: unsafe extern "C" fn(*mut c_void, Hp2PlatformViewportField, *mut f32) -> i32,
    pub write_float: unsafe extern "C" fn(*mut c_void, Hp2PlatformViewportField, f32) -> i32,
    pub scope: unsafe extern "C" fn(*mut c_void, Hp2PlatformViewportPhase) -> i32,
}

/// Each action's unwind label is emitted by hp2-ffi's `native_guard::guarded`
/// around the matching export, replacing the native shell's original C++
/// method guard. Constructor follows unchanged native member initialization.
/// Native no-ops and speech stay native.
/// Inputs: Resize.resize, Open.open, TryRenderDevice.render, CauseInput.cause,
/// SetMouseCapture.capture; Unlock/Repaint/UpdateInput/UpdateMouseGrab.integer.
/// All other original arguments stay in the native invocation frame.
/// Outputs: Lock/Resize/CauseInput/Exec/IsFullscreen.integer; CreateNativeFont/
/// GetWindow.object; GetMouseState.mouse; GetViewportButtonFlags.integer (bits).
/// Other actions return integer zero. Clip and ParentWindow are intentionally
/// unused like the original; legacy ParentWindow widening has no side effect.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformViewportAction {
    Constructor,
    Destroy,
    ShutdownAfterError,
    Lock,
    Resize,
    Unlock,
    Repaint,
    SetModeCursor,
    UpdateWindowFrame,
    Open,
    UpdateInput,
    SetMouseCapture,
    CreateNativeFont,
    CauseInput,
    UpdateMouseGrab,
    ReleaseAllInput,
    Exec,
    TryRenderDevice,
    IsFullscreen,
    ToggleFullscreen,
    EndFullscreen,
    SetTitleBar,
    GetWindow,
    GetMouseState,
    GetViewportButtonFlags,
}

fn checked(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}
impl Hp2PlatformViewportServices {
    fn invoke(
        &self,
        leaf: Hp2PlatformViewportLeaf,
        input: Hp2PlatformViewportValue,
    ) -> Result<Hp2PlatformViewportValue, i32> {
        let mut output = Hp2PlatformViewportValue::default();
        checked(unsafe { (self.call)(self.context, leaf, &input, &mut output) })?;
        Ok(output)
    }
    fn simple(&self, leaf: Hp2PlatformViewportLeaf) -> Result<(), i32> {
        self.invoke(leaf, Hp2PlatformViewportValue::default())?;
        Ok(())
    }
    fn int_leaf(&self, leaf: Hp2PlatformViewportLeaf) -> Result<i32, i32> {
        Ok(unsafe {
            self.invoke(leaf, Hp2PlatformViewportValue::default())?
                .integer
        })
    }
    fn object(&self, leaf: Hp2PlatformViewportLeaf) -> Result<*mut c_void, i32> {
        Ok(unsafe {
            self.invoke(leaf, Hp2PlatformViewportValue::default())?
                .object
        })
    }
    fn int_call(&self, leaf: Hp2PlatformViewportLeaf, integer: i32) -> Result<(), i32> {
        self.invoke(leaf, Hp2PlatformViewportValue { integer })?;
        Ok(())
    }
    fn read(&self, field: Hp2PlatformViewportField) -> Result<i32, i32> {
        let mut value = 0;
        checked(unsafe { (self.read_integer)(self.context, field, &mut value) })?;
        Ok(value)
    }
    fn write(&self, field: Hp2PlatformViewportField, value: i32) -> Result<(), i32> {
        checked(unsafe { (self.write_integer)(self.context, field, value) })
    }
    fn real(&self, field: Hp2PlatformViewportField) -> Result<f32, i32> {
        let mut value = 0.0;
        checked(unsafe { (self.read_float)(self.context, field, &mut value) })?;
        Ok(value)
    }
    fn write_real(&self, field: Hp2PlatformViewportField, value: f32) -> Result<(), i32> {
        checked(unsafe { (self.write_float)(self.context, field, value) })
    }
    fn scope(&self, phase: Hp2PlatformViewportPhase) -> Result<(), i32> {
        checked(unsafe { (self.scope)(self.context, phase) })
    }
}

#[cfg(feature = "sdl")]
mod execution {
    use super::*;
    use Hp2PlatformViewportField as F;
    use Hp2PlatformViewportLeaf as L;
    use Hp2PlatformViewportValue as V;

    fn fullscreen(s: &Hp2PlatformViewportServices) -> Result<i32, i32> {
        s.int_leaf(L::InvokeIsFullscreen)
    }
    fn dispose_renderer(s: &Hp2PlatformViewportServices) -> Result<(), i32> {
        if s.int_leaf(L::HasRenderDevice)? != 0 {
            s.simple(L::RenderExit)?;
            s.simple(L::RenderDelete)?;
            s.simple(L::RenderClear)?;
        }
        Ok(())
    }
    fn mouse_scale(s: &Hp2PlatformViewportServices) -> Result<(), i32> {
        let window = s.object(L::Window)?;
        let (window_x, window_y) = if !window.is_null() {
            // SAFETY: non-null checked above; `window` is the live SdlWindow
            // this invocation's Window leaf returned.
            unsafe { crate::window_lifecycle::get_window_size(window.cast()) }
        } else {
            (0, 0)
        };
        let (x, y) = viewport::mouse_scale(window_x, window_y, s.read(F::SizeX)?, s.read(F::SizeY)?);
        s.write_real(F::MouseScaleX, x)?;
        s.write_real(F::MouseScaleY, y)
    }
    fn resize(s: &Hp2PlatformViewportServices, r: Hp2PlatformViewportResize) -> Result<i32, i32> {
        let state = viewport::resize_viewport(
            viewport::ViewportSize {
                blit_flags: s.read(F::BlitFlags)? as u32,
                size_x: s.read(F::SizeX)?,
                size_y: s.read(F::SizeY)?,
                color_bytes: s.read(F::ColorBytes)?,
            },
            r.flags,
            r.x,
            r.y,
            r.color,
        );
        s.write(F::BlitFlags, state.blit_flags as i32)?;
        s.write(F::SizeX, state.size_x)?;
        s.write(F::SizeY, state.size_y)?;
        s.write(F::ColorBytes, state.color_bytes)?;
        let window = s.object(L::Window)?;
        if !window.is_null() {
            let is_fullscreen = fullscreen(s)? != 0;
            // SAFETY: non-null checked above; `window` is the live SdlWindow
            // this invocation's Window leaf returned.
            unsafe {
                crate::window_lifecycle::set_window_size(window.cast(), state.size_x, state.size_y);
                crate::window_lifecycle::set_window_fullscreen(window.cast(), is_fullscreen);
            }
            mouse_scale(s)?;
        }
        if s.int_leaf(L::HasRenderDevice)? != 0 {
            s.simple(L::RenderSetRes)?;
        }
        Ok((s.int_leaf(L::HasRenderDevice)? != 0) as i32)
    }
    fn open(s: &Hp2PlatformViewportServices, o: Hp2PlatformViewportOpen) -> Result<(), i32> {
        s.simple(L::BindClient)?;
        let (width, height) = viewport::open_window_size(
            o.x,
            o.y,
            o.open_x,
            o.open_y,
            s.read(F::WindowedViewportX)?,
            s.read(F::WindowedViewportY)?,
        );
        if s.object(L::Window)?.is_null() {
            use crate::window_lifecycle as wl;
            for (attr, value) in [
                (wl::SDL_GL_DOUBLEBUFFER, 1),
                (wl::SDL_GL_DEPTH_SIZE, 24),
                (wl::SDL_GL_RED_SIZE, 8),
                (wl::SDL_GL_GREEN_SIZE, 8),
                (wl::SDL_GL_BLUE_SIZE, 8),
            ] {
                wl::set_gl_attribute(attr, value);
            }
            let flags = viewport::window_flags(o.temporary != 0, s.read(F::BorderlessWindow)? != 0);
            let window = crate::window_lifecycle::create_window(width, height, flags);
            s.invoke(L::WindowStore, V { object: window.cast() })?;
            if s.object(L::Window)?.is_null() {
                let error = crate::sdl_runtime::last_error();
                s.invoke(
                    L::WindowCreateError,
                    V {
                        object: error as *mut c_void,
                    },
                )?;
            }
        }
        s.write(F::SizeX, width)?;
        s.write(F::SizeY, height)?;
        s.write(
            F::ColorBytes,
            viewport::window_color_bytes(s.read(F::WindowedColorBits)?),
        )?;
        s.write(
            F::BlitFlags,
            (s.read(F::BlitFlags)? as u32 & !viewport::BLIT_FULLSCREEN) as i32,
        )?;
        if viewport::startup_fullscreen(
            s.read(F::StartupFullscreen)? != 0,
            s.read(F::UseDesktopResolution)? != 0,
        ) {
            s.write(
                F::BlitFlags,
                (s.read(F::BlitFlags)? as u32 | viewport::BLIT_FULLSCREEN) as i32,
            )?;
            let window = s.object(L::Window)?;
            // SAFETY: `window` is the live SdlWindow just created/stored
            // above; the original called this unconditionally (see
            // `start_fullscreen`'s own doc comment for why a null window
            // here is still sound).
            unsafe { crate::window_lifecycle::start_fullscreen(window.cast()) };
        }
        s.write(F::LostFullscreen, 0)?;
        mouse_scale(s)?;
        if s.int_leaf(L::NoHardware)? == 0 {
            s.simple(L::InvokeOpenRenderDevice)?;
            if s.int_leaf(L::HasRenderDevice)? == 0 {
                s.simple(L::NoRenderDeviceError)?;
            }
        }
        Ok(())
    }
    struct NativeInputBackend<'a>(&'a Hp2PlatformViewportServices);
    impl pump::UpdateInputBackend for NativeInputBackend<'_> {
        type Error = i32;

        fn bind_client(&mut self) -> Result<(), Self::Error> {
            self.0.simple(L::BindClient)
        }
        fn mouse_is_grabbed(&mut self) -> Result<bool, Self::Error> {
            Ok(self.0.read(F::MouseIsGrabbed)? != 0)
        }
        fn context(&mut self) -> Result<pump::Context, Self::Error> {
            context(self.0)
        }
        fn has_input(&mut self) -> Result<bool, Self::Error> {
            Ok(self.0.int_leaf(L::HasInput)? != 0)
        }
        fn game_time(&mut self) -> Result<f32, Self::Error> {
            // The policy harness has no live level clock; replay never runs here.
            Ok(0.0)
        }
        fn window(&mut self) -> Result<*mut c_void, Self::Error> {
            self.0.object(L::Window)
        }
        fn key_down(&mut self, key: i32) -> Result<bool, Self::Error> {
            Ok(unsafe { self.0.invoke(L::KeyDown, V { integer: key })?.integer } != 0)
        }
        fn cause_input(&mut self, key: i32, action: i32, delta: f32) -> Result<(), Self::Error> {
            self.0.invoke(
                L::InvokeCauseInput,
                V { cause: Hp2PlatformViewportCause { key, action, delta } },
            )?;
            Ok(())
        }
        fn execute_action(&mut self, action: pump::Action) -> Result<(), Self::Error> {
            match action {
                pump::Action::EngineKey { key } => self.0.int_call(L::EngineKey, key),
                pump::Action::EngineMousePosition { buttons, x, y } => {
                    self.0.invoke(L::EngineMousePosition, V {
                        mouse: Hp2PlatformViewportMouse { buttons, x, y },
                    })?;
                    Ok(())
                }
                pump::Action::UpdateMouseGrab { capture } => {
                    self.0.int_call(L::InvokeMouseGrab, i32::from(capture))
                }
                pump::Action::SetLostGrab(value) => self.0.write(F::LostGrab, i32::from(value)),
                pump::Action::ResizeViewport { width, height } => {
                    self.0.invoke(L::InvokeResize, V {
                        resize: Hp2PlatformViewportResize {
                            flags: viewport::BLIT_NO_WINDOW_CHANGE,
                            x: width,
                            y: height,
                            color: viewport::INDEX_NONE,
                        },
                    })?;
                    Ok(())
                }
                pump::Action::Cause { .. }
                | pump::Action::SetLastJoyHat(_)
                | pump::Action::ReleaseAll => unreachable!("shared input executor consumes this action"),
            }
        }
        fn set_last_joy_hat(&mut self, key: i32) -> Result<(), Self::Error> {
            self.0.write(F::LastJoyHat, key)
        }
    }
    fn context(s: &Hp2PlatformViewportServices) -> Result<pump::Context, i32> {
        Ok(pump::Context {
            mouse_is_grabbed: s.read(F::MouseIsGrabbed)? != 0,
            lost_grab: s.read(F::LostGrab)? != 0,
            capture_mouse: s.read(F::CaptureMouse)? != 0,
            has_window: !s.object(L::Window)?.is_null(),
            has_engine: !s.object(L::Engine)?.is_null(),
            has_actor: s.int_leaf(L::HasActor)? != 0,
            joy_hat_folds_to_buttons: s.read(F::JoyHatFoldsToButtons)? != 0,
            mouse_scale_x: s.real(F::MouseScaleX)?,
            mouse_scale_y: s.real(F::MouseScaleY)?,
            size_x: s.read(F::SizeX)?,
            size_y: s.read(F::SizeY)?,
            joystick_dead_zone: s.real(F::JoystickDeadZone)?,
            scale_jbx: s.real(F::ScaleJBX)?,
            scale_jby: s.real(F::ScaleJBY)?,
            last_joy_hat: s.read(F::LastJoyHat)?,
        })
    }
    fn update_input(s: &Hp2PlatformViewportServices, reset: i32) -> Result<(), i32> {
        let mut backend = NativeInputBackend(s);
        // SAFETY: Window() returns the live SDK window pointer for each poll,
        // and every other operation is checked through the invocation services.
        unsafe { pump::update_input_with(&mut backend, reset != 0) }
    }
    fn try_render(
        s: &Hp2PlatformViewportServices,
        mut r: Hp2PlatformViewportRender,
    ) -> Result<(), i32> {
        use Hp2PlatformViewportCandidate as C;
        s.simple(L::BindClient)?;
        dispose_renderer(s)?;
        if r.x == viewport::INDEX_NONE {
            r.x = s.read(if r.fullscreen != 0 {
                F::FullscreenViewportX
            } else {
                F::WindowedViewportX
            })?;
        }
        if r.y == viewport::INDEX_NONE {
            r.y = s.read(if r.fullscreen != 0 {
                F::FullscreenViewportY
            } else {
                F::WindowedViewportY
            })?;
        }
        if r.color == viewport::INDEX_NONE {
            r.color = if r.fullscreen != 0 {
                s.read(F::FullscreenColorBits)? / 8
            } else {
                s.read(F::ColorBytes)?
            };
        }
        let fallback = if r.fullscreen != 0 {
            C::Game
        } else {
            C::Windowed
        };
        let has_fallback = unsafe {
            s.invoke(
                L::CompareRenderClass,
                V {
                    candidate: fallback,
                },
            )?
            .integer
        } != 0;
        for attempt in 0..2 {
            if s.int_leaf(L::HasRenderDevice)? != 0 {
                break;
            }
            if attempt == 1 && !has_fallback {
                break;
            }
            r.candidate = if attempt == 0 { C::Primary } else { fallback };
            let class = unsafe {
                s.invoke(
                    L::LoadRenderClass,
                    V {
                        candidate: r.candidate,
                    },
                )?
                .object
            };
            if class.is_null() {
                continue;
            }
            s.invoke(L::ConstructRenderDevice, V { object: class })?;
            if s.int_leaf(L::HasRenderDevice)? == 0 {
                continue;
            }
            if unsafe { s.invoke(L::RenderInit, V { render: r })?.integer } != 0 {
                if s.int_leaf(L::IsRunning)? != 0 && s.int_leaf(L::HasActor)? != 0 {
                    s.simple(L::DetailChange)?;
                }
            } else {
                s.invoke(
                    L::LogRenderFailure,
                    V {
                        candidate: r.candidate,
                    },
                )?;
                s.simple(L::RenderDelete)?;
                s.simple(L::RenderClear)?;
            }
        }
        s.simple(L::PublishRenderDevice)
    }
    fn parse(
        s: &Hp2PlatformViewportServices,
        command: Hp2PlatformViewportCommand,
    ) -> Result<bool, i32> {
        Ok(unsafe { s.invoke(L::ParseCommand, V { command })?.integer } != 0)
    }
    fn exec(s: &Hp2PlatformViewportServices) -> Result<i32, i32> {
        use Hp2PlatformViewportCommand as C;
        use Hp2PlatformViewportLog as Log;
        let log = if parse(s, C::GetRes)? || parse(s, C::GetCurrentRes)? {
            Some(Log::Resolution)
        } else if parse(s, C::GetCurrentColorDepth)? {
            Some(Log::ColorDepth)
        } else if parse(s, C::GetCurrentBitDepth)? {
            Some(Log::BitDepth)
        } else if parse(s, C::ToggleFullscreen)? {
            s.simple(L::InvokeToggleFullscreen)?;
            return Ok(1);
        } else if parse(s, C::DumpCapturedMouse)? {
            Some(Log::Mouse)
        } else if parse(s, C::GetSystemIni)? {
            Some(Log::SystemIni)
        } else if parse(s, C::GetUserIni)? {
            Some(Log::UserIni)
        } else if parse(s, C::RelaunchSupport)? {
            s.simple(L::InvokeEndFullscreen)?;
            s.invoke(
                L::InvokeTryRenderDevice,
                V {
                    render: Hp2PlatformViewportRender {
                        candidate: Hp2PlatformViewportCandidate::Game,
                        x: -1,
                        y: -1,
                        color: -1,
                        fullscreen: fullscreen(s)?,
                    },
                },
            )?;
            return Ok(1);
        } else {
            None
        };
        if let Some(log) = log {
            s.invoke(L::LogCommand, V { log })?;
            Ok(1)
        } else {
            s.int_leaf(L::SuperExec)
        }
    }

    /// # Safety
    /// The copied service table and native invocation satisfy the documented
    /// callback/lifetime contracts; SDL C ABI callbacks never unwind into Rust.
    pub unsafe fn run(
        s: Hp2PlatformViewportServices,
        action: Hp2PlatformViewportAction,
        input: V,
    ) -> Result<V, i32> {
        use Hp2PlatformViewportAction as A;
        let mut result = V { integer: 0 };
        match action {
            A::Constructor => {
                for slot in 0..viewport::NUM_SYSTEM_CURSORS {
                    let object = crate::cursor::create_system_cursor(slot);
                    s.invoke(
                        L::CursorStore,
                        V {
                            cursor: Hp2PlatformViewportCursor {
                                slot,
                                object: object.cast(),
                            },
                        },
                    )?;
                }
                let bits = crate::display_mode::desktop_color_bits();
                let depth = viewport::desktop_color_depth(bits);
                s.write(F::ColorBytes, depth.color_bytes)?;
                if depth.rgb565 {
                    s.simple(L::AddRgb565Cap)?;
                }
                let (compiled_major, compiled_minor, compiled_patch) = crate::sdl_runtime::COMPILED_VERSION;
                let (linked_major, linked_minor, linked_patch) = crate::sdl_runtime::linked_version();
                s.invoke(
                    L::LogVersion,
                    V {
                        version: Hp2PlatformViewportVersion {
                            compiled_major,
                            compiled_minor,
                            compiled_patch,
                            linked_major,
                            linked_minor,
                            linked_patch,
                        },
                    },
                )?;
            }
            A::Destroy => {
                s.int_call(L::InvokeMouseGrab, 0)?;
                dispose_renderer(&s)?;
                let window = s.object(L::Window)?;
                if !window.is_null() {
                    // SAFETY: non-null checked above; `window` is the live
                    // SdlWindow this invocation's Window leaf returned; not
                    // used again after.
                    unsafe { crate::window_lifecycle::destroy_window(window.cast()) };
                    s.simple(L::WindowClear)?;
                }
                for slot in 0..viewport::NUM_SYSTEM_CURSORS {
                    let object = unsafe { s.invoke(L::CursorAt, V { integer: slot })?.object };
                    if !object.is_null() {
                        // SAFETY: non-null checked above; `object` is the
                        // live SdlCursor this invocation's CursorAt leaf
                        // returned; not used again after.
                        unsafe { crate::cursor::free_cursor(object.cast()) };
                        s.int_call(L::CursorClear, slot)?;
                    }
                }
                s.simple(L::SuperDestroy)?;
            }
            A::ShutdownAfterError => {
                if s.int_leaf(L::HasRenderDevice)? != 0 {
                    s.simple(L::RenderShutdownAfterError)?;
                }
                crate::client_policy::sdl_quit();
                s.simple(L::SuperShutdownAfterError)?;
            }
            A::Lock => {
                // The original pure helper evaluates all three arguments.
                if viewport::lock_allowed(
                    s.int_leaf(L::HasRenderDevice)? != 0,
                    s.read(F::SizeX)?,
                    s.read(F::SizeY)?,
                ) {
                    s.simple(L::RenderLock)?;
                    s.simple(L::StampCurrentTime)?;
                    s.simple(L::IncrementFrameCount)?;
                    result = V { integer: 1 };
                }
            }
            A::Resize => {
                result = V {
                    integer: resize(&s, unsafe { input.resize })?,
                }
            }
            A::Unlock => {
                let blit = unsafe { input.integer };
                s.int_call(L::RenderUnlock, blit)?;
                if blit != 0 {
                    s.simple(L::CopyLastUpdateTime)?;
                }
            }
            A::Repaint => {
                let engine = s.object(L::Engine)?;
                if !engine.is_null() && s.read(F::SizeX)? != 0 && s.read(F::SizeY)? != 0 {
                    s.invoke(
                        L::EngineDraw,
                        V {
                            draw: Hp2PlatformViewportDraw {
                                engine,
                                blit: unsafe { input.integer },
                            },
                        },
                    )?;
                }
            }
            A::SetModeCursor => {
                let slot = viewport::mode_cursor_index(s.read(F::SelectedCursor)?) as i32;
                let cursor = unsafe { s.invoke(L::CursorAt, V { integer: slot })?.object };
                if !cursor.is_null() {
                    // SAFETY: non-null checked above; `cursor` is the live
                    // SdlCursor this invocation's CursorAt leaf returned.
                    unsafe { crate::cursor::set_cursor(cursor.cast()) };
                }
            }
            A::UpdateWindowFrame => {
                // This method has no saved C in the source: BindClient resolves
                // the getter at this invocation only, not a prior method's C.
                s.simple(L::BindClient)?;
                let window = s.object(L::Window)?;
                let borderless = s.read(F::BorderlessWindow)? != 0;
                unsafe { crate::window_chrome::update_window_frame(window.cast(), borderless) };
            }
            A::Open => open(&s, unsafe { input.open })?,
            A::UpdateInput => update_input(&s, unsafe { input.integer })?,
            A::SetMouseCapture => {
                let capture = unsafe { input.capture };
                set_mouse_capture_with(
                    capture.capture,
                    capture.focus_only,
                    |lost| s.write(F::LostGrab, i32::from(lost)),
                    || {
                        let window = s.object(L::Window)?;
                        Ok(if window.is_null() { None } else {
                            Some(unsafe { crate::window_lifecycle::get_window_flags(window.cast()) })
                        })
                    },
                    |capture| s.int_call(L::InvokeMouseGrab, capture),
                )?;
            }
            A::CreateNativeFont => {
                let font = create_native_font_with(
                    || s.object(L::ConstructFont),
                    |font| {
                        s.invoke(L::SetFontName, V { object: font })?;
                        s.invoke(L::SetFontHeight, V { object: font })?;
                        s.invoke(L::ClearNativeFont, V { object: font })?;
                        Ok(())
                    },
                )?;
                result = V { object: font };
            }
            A::CauseInput => {
                if !s.object(L::Engine)?.is_null() && s.int_leaf(L::HasActor)? != 0 {
                    result = s.invoke(
                        L::EngineInputEvent,
                        V {
                            cause: unsafe { input.cause },
                        },
                    )?;
                }
            }
            A::UpdateMouseGrab => {
                let window = s.object(L::Window)?;
                let capture = unsafe { input.integer } != 0;
                let grabbed = s.read(F::MouseIsGrabbed)? != 0;
                let (grabbed, available) = unsafe {
                    crate::mouse_state::update_mouse_grab_state(window.cast(), capture, grabbed)
                };
                s.write(F::WindowsMouseAvailable, available as i32)?;
                s.write(F::MouseIsGrabbed, grabbed as i32)?;
            }
            A::ReleaseAllInput => pump::release_all_with(&mut NativeInputBackend(&s))?,
            A::Exec => result = V { integer: exec(&s)? },
            A::TryRenderDevice => try_render(&s, unsafe { input.render })?,
            A::IsFullscreen => {
                result = V {
                    integer: viewport::is_fullscreen(s.read(F::BlitFlags)? as u32) as i32,
                }
            }
            A::ToggleFullscreen | A::EndFullscreen => {
                s.write(F::LostFullscreen, (fullscreen(&s)? != 0) as i32)?;
                let flags = if matches!(action, A::ToggleFullscreen) && fullscreen(&s)? == 0 {
                    viewport::BLIT_FULLSCREEN
                } else {
                    0
                };
                s.invoke(
                    L::InvokeResize,
                    V {
                        resize: Hp2PlatformViewportResize {
                            flags,
                            x: -1,
                            y: -1,
                            color: -1,
                        },
                    },
                )?;
            }
            A::SetTitleBar => s.scope(Hp2PlatformViewportPhase::TitleUtf8)?,
            A::GetWindow => {
                result = V {
                    object: s.object(L::Window)?,
                }
            }
            A::GetMouseState => {
                let (x, y, buttons) = crate::mouse_state::get_mouse_state(
                    s.real(F::MouseScaleX)?,
                    s.real(F::MouseScaleY)?,
                );
                result = V {
                    mouse: Hp2PlatformViewportMouse { buttons, x, y },
                };
            }
            A::GetViewportButtonFlags => {
                result = V {
                    integer: crate::mouse_state::get_viewport_button_flags() as i32,
                }
            }
        }
        Ok(result)
    }

    /// # Safety
    /// Must be called synchronously from the matching native scope callback,
    /// with its actual typed temporary alive. `phase` itself installs no
    /// unwind label of its own; it always runs nested inside the `run` call
    /// whose action guard (in hp2-ffi) already covers this reentry, so a
    /// second label here would double-log.
    pub unsafe fn phase(
        s: Hp2PlatformViewportServices,
        phase: Hp2PlatformViewportPhase,
    ) -> Result<(), i32> {
        use Hp2PlatformViewportPhase as P;
        match phase {
            P::TitleUtf8 => {
                s.simple(L::ConvertTitle)?;
                let bytes = unsafe { s.invoke(L::TitleBytes, V::default())?.bytes };
                // Matches str_from_raw in the existing title export: native
                // conversion is UTF-8, but an invalid byte sequence is a no-op.
                let title = if bytes.len == 0 {
                    Some("")
                } else if bytes.data.is_null() {
                    None
                } else {
                    std::str::from_utf8(unsafe {
                        std::slice::from_raw_parts(bytes.data, bytes.len)
                    })
                    .ok()
                };
                let window = s.object(L::Window)?;
                if let Some(title) = title {
                    unsafe { crate::window_chrome::set_title_bar(window.cast(), title) };
                }
            }
        }
        Ok(())
    }
}
#[cfg(feature = "sdl")]
pub use execution::{phase, run};

/// Shared SDL viewport native-font construction and non-null initialization gate.
pub fn create_native_font_with<E>(
    create: impl FnOnce() -> Result<*mut c_void, E>,
    initialize: impl FnOnce(*mut c_void) -> Result<(), E>,
) -> Result<*mut c_void, E> {
    let font = create()?;
    if !font.is_null() { initialize(font)?; }
    Ok(font)
}

#[cfg(test)]
mod viewport_capture_contracts;
