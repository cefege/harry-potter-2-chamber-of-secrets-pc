//! SDL client lifecycle and traversal policy. Native code retains UObject/SDL
//! ownership, virtual dispatch, typed temporaries, and lexical exception guards.
//! No native object is borrowed by Rust across a callback.
use crate::input_pump::{RepaintCandidate, repaint_replaces_best};
use std::ffi::c_void;
use std::ptr;

/// Replaces `Hp2PlatformClientLeaf_SdlQuit` (`SDLClient.cpp:105-106`) and
/// `Hp2PlatformViewportLeaf_SdlQuit` (`SDLViewport.cpp:169`, now removed
/// from `viewport_policy`'s `execution` module too), called directly
/// instead of through a leaf since `SDL_Quit` touches no UObject/engine
/// state.
#[cfg(feature = "sdl")]
mod sdl_ffi {
    unsafe extern "C" {
        pub fn SDL_Quit();
    }
}

/// `SDL_Quit()`, with no additional joystick close or viewport traversal
/// (replaces `Hp2PlatformClientLeaf_SdlQuit`, `SDLClient.cpp:105-106`, and
/// `Hp2PlatformViewportLeaf_SdlQuit`, `SDLViewport.cpp:169`). A no-op
/// without the `sdl` feature, matching every other guarded SDL entry
/// point in this crate (no SDL is ever initialized in that build). Safe to
/// call at any time, even if SDL was never initialized or `SDL_Quit` was
/// already called (SDL treats both as a no-op).
#[cfg(feature = "sdl")]
pub(crate) fn sdl_quit() {
    unsafe { sdl_ffi::SDL_Quit() }
}
#[cfg(not(feature = "sdl"))]
pub(crate) fn sdl_quit() {}

/// Native scalar fields used by the policy. No layout of USDLClient is exposed.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformClientField {
    WindowedViewportX,
    WindowedViewportY,
    WindowedColorBits,
    FullscreenViewportX,
    FullscreenViewportY,
    FullscreenColorBits,
    Brightness,
    CaptureMouse,
    ScreenFlashes,
    MaintainVerticalFOV,
    Decals,
    ShowFPS,
    MinDesiredFrameRate,
    ParticleDensity,
    StartupFullscreen,
    UIScale,
    JoystickDeadZone,
    UseJoystick,
    JoyButtons,
    JoyHats,
    JoyAxes,
    JoyHatFoldsToButtons,
}

/// Checked native operations. Unspecified arguments are zero/null; unspecified
/// outputs are ignored. `object` and `other` are opaque native pointer tokens.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformClientLeaf {
    /// Super::Init(InEngine), using the invocation's native UEngine*.
    SuperInit,
    /// this->PostEditChange(), retaining virtual dispatch; the unwind label
    /// is now hp2-ffi's `native_guard::guarded`, not a C++ method guard.
    PostEditChange,
    /// ParseParam(appCmdLine(), TEXT("defaultres")); output.integer is UBOOL.
    DefaultResolution,
    /// this->OpenJoystick(); the unwind label is hp2-ffi's
    /// `native_guard::guarded`, not a C++ method guard. Output.integer is UBOOL.
    OpenJoystick,
    /// this->CloseJoystick(); the unwind label is hp2-ffi's
    /// `native_guard::guarded`, not a C++ method guard.
    CloseJoystick,
    /// Super::PostEditChange().
    SuperPostEditChange,
    /// Viewports.Num(), reread on every call; output.integer is signed INT.
    ViewportCount,
    /// Viewports(index), where index is `integer`; output.object is UViewport*.
    ViewportAt,
    /// object->RenDev != NULL; object is the saved UViewport*. Output.integer.
    HasRenderDevice,
    /// object->RenDev->Exit(); reread RenDev, do not use the condition's value.
    RenderDeviceExit,
    /// delete object->RenDev; reread RenDev AFTER Exit (which can replace it).
    DeleteRenderDevice,
    /// object->RenDev = NULL; use the saved viewport, not the array entry.
    ClearRenderDevice,
    /// Viewports.Remove(0), after saving Viewports(0) in Rust.
    RemoveFirstViewport,
    /// delete static_cast<UViewport*>(object), including a possible null.
    DeleteViewport,
    /// Super::Destroy().
    SuperDestroy,
    /// Super::ShutdownAfterError().
    SuperShutdownAfterError,
    /// static_cast<UViewport*>(object)->UpdateInput(0).
    UpdateInput,
    /// object is the candidate viewport, other is the best viewport or null.
    /// First compute the original FTime subtraction (or 0.f without a best),
    /// then call candidate->IsRealtime(), then read SizeX and SizeY, in that
    /// order. Return output.repaint. Keep FTime arithmetic native, not float
    /// timestamp approximations. IsRealtime may mutate the subsequently read
    /// fields; do not snapshot them ahead of that call.
    RepaintCandidate,
    /// static_cast<UViewport*>(object)->Repaint(1).
    Repaint,
    /// Read this->Joystick; output.object is SDL_Joystick* (possibly null).
    Joystick,
    /// Assign this->Joystick = static_cast<SDL_Joystick*>(object).
    SetJoystick,
    /// Read JoystickNumber and IgnoreHat != 0 immediately before Rust calls the
    /// existing joystick::open_joystick helper; output.settings. This leaf does
    /// not perform SDL operations or publish client fields.
    JoystickSettings,
}

/// Native joystick configuration sampled only after CloseJoystick completes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformClientJoystickSettings {
    pub number: i32,
    pub ignore_hat: i32,
}

/// Native FTime subtraction and the subsequent live viewport observations.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformClientRepaint {
    pub realtime: i32,
    pub size_x: i32,
    pub size_y: i32,
    pub best_minus_candidate: f32,
}

/// A leaf writes only its documented member, and only on success.
#[repr(C)]
#[derive(Clone, Copy)]
pub union Hp2PlatformClientValue {
    pub integer: i32,
    pub object: *mut c_void,
    pub settings: Hp2PlatformClientJoystickSettings,
    pub repaint: Hp2PlatformClientRepaint,
}

/// Invocation-local checked host service table, copied before callbacks begin.
///
/// All callbacks return 0 on success or an exact nonzero failure status. Native
/// callbacks catch/store the first native exception; no C++ exception may cross
/// Rust. Stop immediately on failure: no speculative cleanup or rollback. The
/// shell rethrows only AFTER Rust returns, after hp2-ffi's `native_guard::guarded`
/// logs the matching unwind label for the failing action (replacing the
/// original native shell's own `guard()`/`unguard`).
/// Native callback scopes must remain live C++ exception fences. Reentry uses
/// independent contexts. Native state stays host-owned;
/// there is no exclusive native-object reference, arena lock, or cached array.
///
/// SDK callbacks invoked by the already-ported Rust joystick helper obey the
/// SDL C ABI: they must contain their own native exceptions, never unwind into
/// Rust. Native operation failures use the checked service callbacks; ordinary
/// SDL open failures remain a successful policy result with UBOOL zero.
///
/// Scalar accessors perform exactly one live field read/write. The integer
/// accessors serve INT/UBOOL/BITFIELD fields; float accessors serve FLOAT fields.
/// The field enum is a typed name, not an offset. Accessors contain exceptions
/// exactly like `call` and must not invoke unrelated behavior.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PlatformClientServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(
        *mut c_void,
        Hp2PlatformClientLeaf,
        *mut c_void,
        *mut c_void,
        i32,
        *mut Hp2PlatformClientValue,
    ) -> i32,
    pub read_integer: unsafe extern "C" fn(*mut c_void, Hp2PlatformClientField, *mut i32) -> i32,
    pub write_integer: unsafe extern "C" fn(*mut c_void, Hp2PlatformClientField, i32) -> i32,
    pub read_float: unsafe extern "C" fn(*mut c_void, Hp2PlatformClientField, *mut f32) -> i32,
    pub write_float: unsafe extern "C" fn(*mut c_void, Hp2PlatformClientField, f32) -> i32,
}

/// Actions accepted by `run` through the checked client service table.
/// `StaticDefaults` and the other actions use the callbacks and error
/// contracts described by `Hp2PlatformClientServices`. The caller must finish
/// all property placement constructions before running `StaticDefaults`.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2PlatformClientAction {
    StaticDefaults,
    Init,
    Destroy,
    PostEditChange,
    ShutdownAfterError,
    Tick,
}

fn checked(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

impl Hp2PlatformClientServices {
    fn invoke(
        &self,
        leaf: Hp2PlatformClientLeaf,
        object: *mut c_void,
        other: *mut c_void,
        integer: i32,
    ) -> Result<Hp2PlatformClientValue, i32> {
        // Initialize the largest member; no-output leaves need not write it.
        let mut value = Hp2PlatformClientValue {
            repaint: Hp2PlatformClientRepaint {
                realtime: 0,
                size_x: 0,
                size_y: 0,
                best_minus_candidate: 0.0,
            },
        };
        checked(unsafe { (self.call)(self.context, leaf, object, other, integer, &mut value) })?;
        Ok(value)
    }

    fn simple(&self, leaf: Hp2PlatformClientLeaf) -> Result<(), i32> {
        self.invoke(leaf, ptr::null_mut(), ptr::null_mut(), 0)?;
        Ok(())
    }

    fn integer(&self, leaf: Hp2PlatformClientLeaf) -> Result<i32, i32> {
        Ok(unsafe {
            self.invoke(leaf, ptr::null_mut(), ptr::null_mut(), 0)?
                .integer
        })
    }

    fn object_call(&self, leaf: Hp2PlatformClientLeaf, object: *mut c_void) -> Result<(), i32> {
        self.invoke(leaf, object, ptr::null_mut(), 0)?;
        Ok(())
    }

    fn viewport(&self, index: i32) -> Result<*mut c_void, i32> {
        Ok(unsafe {
            self.invoke(
                Hp2PlatformClientLeaf::ViewportAt,
                ptr::null_mut(),
                ptr::null_mut(),
                index,
            )?
            .object
        })
    }

    fn read_int(&self, field: Hp2PlatformClientField) -> Result<i32, i32> {
        let mut value = 0;
        checked(unsafe { (self.read_integer)(self.context, field, &mut value) })?;
        Ok(value)
    }

    fn write_int(&self, field: Hp2PlatformClientField, value: i32) -> Result<(), i32> {
        checked(unsafe { (self.write_integer)(self.context, field, value) })
    }

    fn read_real(&self, field: Hp2PlatformClientField) -> Result<f32, i32> {
        let mut value = 0.0;
        checked(unsafe { (self.read_float)(self.context, field, &mut value) })?;
        Ok(value)
    }

    fn write_real(&self, field: Hp2PlatformClientField, value: f32) -> Result<(), i32> {
        checked(unsafe { (self.write_float)(self.context, field, value) })
    }
}

fn static_defaults(services: &Hp2PlatformClientServices) -> Result<(), i32> {
    use Hp2PlatformClientField as F;
    services.write_int(F::WindowedViewportX, 1024)?;
    services.write_int(F::WindowedViewportY, 768)?;
    services.write_int(F::WindowedColorBits, 32)?;
    services.write_int(F::FullscreenViewportX, 1024)?;
    services.write_int(F::FullscreenViewportY, 768)?;
    services.write_int(F::FullscreenColorBits, 32)?;
    services.write_real(F::Brightness, 0.5)?;
    services.write_int(F::CaptureMouse, 1)?;
    services.write_int(F::ScreenFlashes, 1)?;
    services.write_int(F::MaintainVerticalFOV, 1)?;
    services.write_int(F::Decals, 1)?;
    services.write_int(F::ShowFPS, 0)?;
    services.write_real(F::MinDesiredFrameRate, 30.0)?;
    services.write_int(F::ParticleDensity, 1)?;
    services.write_int(F::StartupFullscreen, 0)?;
    services.write_real(F::UIScale, 1.0)?;
    services.write_real(F::JoystickDeadZone, 0.2)?;
    Ok(())
}

fn init(services: &Hp2PlatformClientServices) -> Result<(), i32> {
    use Hp2PlatformClientField as F;
    use Hp2PlatformClientLeaf as L;
    services.simple(L::SuperInit)?;
    services.write_int(F::CaptureMouse, 1)?;
    services.simple(L::PostEditChange)?;
    if services.integer(L::DefaultResolution)? != 0 {
        services.write_int(F::WindowedViewportX, 640)?;
        services.write_int(F::WindowedViewportY, 480)?;
    }
    if services.read_int(F::UseJoystick)? != 0 && services.integer(L::OpenJoystick)? == 0 {
        services.write_int(F::UseJoystick, 0)?;
    }
    Ok(())
}

fn destroy(services: &Hp2PlatformClientServices) -> Result<(), i32> {
    use Hp2PlatformClientLeaf as L;
    let mut index = 0;
    while index < services.integer(L::ViewportCount)? {
        let viewport = services.viewport(index)?;
        if !viewport.is_null()
            && unsafe {
                services
                    .invoke(L::HasRenderDevice, viewport, ptr::null_mut(), 0)?
                    .integer
            } != 0
        {
            services.object_call(L::RenderDeviceExit, viewport)?;
            services.object_call(L::DeleteRenderDevice, viewport)?;
            services.object_call(L::ClearRenderDevice, viewport)?;
        }
        index += 1;
    }
    while services.integer(L::ViewportCount)? != 0 {
        let viewport = services.viewport(0)?;
        services.simple(L::RemoveFirstViewport)?;
        services.object_call(L::DeleteViewport, viewport)?;
    }
    sdl_quit();
    services.simple(L::SuperDestroy)
}

fn tick(services: &Hp2PlatformClientServices) -> Result<(), i32> {
    use Hp2PlatformClientLeaf as L;
    let mut index = 0;
    while index < services.integer(L::ViewportCount)? {
        if !services.viewport(index)?.is_null() {
            // The native expression indexes twice: preserve the live second
            // read instead of reusing the pointer tested by the if condition.
            let viewport = services.viewport(index)?;
            services.object_call(L::UpdateInput, viewport)?;
        }
        index += 1;
    }
    let mut best = ptr::null_mut();
    index = 0;
    while index < services.integer(L::ViewportCount)? {
        let viewport = services.viewport(index)?;
        if !viewport.is_null() {
            let candidate = unsafe {
                services
                    .invoke(L::RepaintCandidate, viewport, best, 0)?
                    .repaint
            };
            if repaint_replaces_best(
                !best.is_null(),
                RepaintCandidate {
                    realtime: candidate.realtime != 0,
                    size_x: candidate.size_x,
                    size_y: candidate.size_y,
                    best_minus_candidate: candidate.best_minus_candidate,
                },
            ) {
                best = viewport;
            }
        }
        index += 1;
    }
    if !best.is_null() {
        services.object_call(L::Repaint, best)?;
    }
    Ok(())
}

/// Close the current SDL handle before clearing any native client fields.
///
/// # Safety
/// Services satisfy the checked callback contract and Joystick is a valid
/// still-open SDL handle or null. SDL callbacks cannot unwind through Rust.
#[cfg(feature = "sdl")]
pub unsafe fn close_joystick(services: Hp2PlatformClientServices) -> Result<(), i32> {
    use Hp2PlatformClientField as F;
    use Hp2PlatformClientLeaf as L;
    let handle = unsafe {
        services
            .invoke(L::Joystick, ptr::null_mut(), ptr::null_mut(), 0)?
            .object
    };
    unsafe { crate::joystick::close_joystick(handle.cast()) };
    services.object_call(L::SetJoystick, ptr::null_mut())?;
    services.write_int(F::JoyButtons, 0)?;
    services.write_int(F::JoyHats, 0)?;
    services.write_int(F::JoyAxes, 0)?;
    services.write_int(F::JoyHatFoldsToButtons, 0)
}

/// Close first, sample live configuration, reuse the existing SDL open policy,
/// publish the handle even on ordinary failure, then publish counts on success.
///
/// # Safety
/// Services satisfy the checked callback contract and SDL callbacks cannot
/// unwind through Rust. The nested CloseJoystick leaf still reenters
/// `USDLClient::CloseJoystick`, retaining its own unwind label (now hp2-ffi's
/// `native_guard::guarded`, not a C++ method guard).
#[cfg(feature = "sdl")]
pub unsafe fn open_joystick(services: Hp2PlatformClientServices) -> Result<i32, i32> {
    use Hp2PlatformClientField as F;
    use Hp2PlatformClientLeaf as L;
    // Retain the nested USDLClient::CloseJoystick reentry (and its own
    // unwind label) rather than eliding it by directly invoking the Rust
    // close body.
    services.simple(L::CloseJoystick)?;
    let settings = unsafe {
        services
            .invoke(L::JoystickSettings, ptr::null_mut(), ptr::null_mut(), 0)?
            .settings
    };
    let opened =
        unsafe { crate::joystick::open_joystick(settings.number, settings.ignore_hat != 0) };
    let handle = opened.map_or(ptr::null_mut(), |(handle, ..)| handle.cast());
    services.object_call(L::SetJoystick, handle)?;
    let Some((_, buttons, hats, axes, hat_folds_to_buttons)) = opened else {
        return Ok(0);
    };
    services.write_int(F::JoyButtons, buttons)?;
    services.write_int(F::JoyHats, hats)?;
    services.write_int(F::JoyAxes, axes)?;
    services.write_int(F::JoyHatFoldsToButtons, i32::from(hat_folds_to_buttons))?;
    Ok(1)
}

/// Execute one client policy; its caller (hp2-ffi) wraps this in
/// `native_guard::guarded` under the action's original native method name.
/// Returns zero on success; errors are exact native callback statuses. The
/// joystick open/close entry points are separate because they require the
/// SDL feature.
///
/// # Safety
/// Services satisfy their callback/lifetime contract. All native object tokens
/// must remain usable exactly as required by the original native method; this
/// port deliberately adds no lifetime repair for native reentrant invalidation.
pub unsafe fn run(
    services: Hp2PlatformClientServices,
    action: Hp2PlatformClientAction,
) -> Result<i32, i32> {
    use Hp2PlatformClientAction as A;
    use Hp2PlatformClientField as F;
    use Hp2PlatformClientLeaf as L;
    match action {
        A::StaticDefaults => static_defaults(&services)?,
        A::Init => init(&services)?,
        A::Destroy => destroy(&services)?,
        A::PostEditChange => {
            services.simple(L::SuperPostEditChange)?;
            if services.read_real(F::UIScale)?.partial_cmp(&0.0)
                != Some(std::cmp::Ordering::Greater)
            {
                services.write_real(F::UIScale, 1.0)?;
            }
        }
        A::ShutdownAfterError => {
            sdl_quit();
            services.simple(L::SuperShutdownAfterError)?;
        }
        A::Tick => tick(&services)?,
    }
    Ok(0)
}

#[cfg(test)]
mod client_post_edit_contracts;
