//! `SDL_ShowSimpleMessageBox`/`SDL_ShowMessageBox` leaves shared by the free
//! function `ShowLauncherError` (`SDLLaunch.cpp`), `FFeedbackContextSDL`'s
//! `ShowErrorBox`/`YesNof`, and `FOutputDeviceSDLError`'s
//! `Hp2LauncherErrorLeaf_ShowMessageBox`. Plain SDL2 C API, no engine or
//! reflection dependency; the classes themselves stay C++ virtual-override
//! shells (they subclass `FFeedbackContext`/`FOutputDeviceError`), but the
//! message-box calls inside their method bodies move here like any other
//! plain SDL2 leaf.

#[cfg(feature = "sdl")]
mod ffi {
    use std::ffi::c_void;
    use std::os::raw::{c_char, c_int};

    #[repr(C)]
    pub struct SdlMessageBoxButtonData {
        pub flags: u32,
        pub buttonid: c_int,
        pub text: *const c_char,
    }

    #[repr(C)]
    pub struct SdlMessageBoxData {
        pub flags: u32,
        pub window: *mut c_void,
        pub title: *const c_char,
        pub message: *const c_char,
        pub numbuttons: c_int,
        pub buttons: *const SdlMessageBoxButtonData,
        pub color_scheme: *const c_void,
    }

    unsafe extern "C" {
        pub fn SDL_ShowSimpleMessageBox(
            flags: u32,
            title: *const c_char,
            message: *const c_char,
            window: *mut c_void,
        ) -> c_int;
        pub fn SDL_ShowMessageBox(data: *const SdlMessageBoxData, buttonid: *mut c_int) -> c_int;
    }
}

/// `SDL_MESSAGEBOX_ERROR` (`SDL_messagebox.h`).
#[cfg(feature = "sdl")]
const SDL_MESSAGEBOX_ERROR: u32 = 0x0000_0010;

/// `SDL_MESSAGEBOX_INFORMATION` (`SDL_messagebox.h`).
#[cfg(feature = "sdl")]
const SDL_MESSAGEBOX_INFORMATION: u32 = 0x0000_0040;

/// `SDL_MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT` (`SDL_messagebox.h`).
#[cfg(feature = "sdl")]
const SDL_MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT: u32 = 0x0000_0001;

/// `SDL_MESSAGEBOX_BUTTON_ESCAPEKEY_DEFAULT` (`SDL_messagebox.h`).
#[cfg(feature = "sdl")]
const SDL_MESSAGEBOX_BUTTON_ESCAPEKEY_DEFAULT: u32 = 0x0000_0002;

/// Replaces `SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, title, message,
/// NULL)` (`SDLLaunch.cpp:83`). `title`/`message` are raw bytes, not `&str`:
/// the original's `std::string`/`const char*` were never validated as UTF-8
/// either. Must not contain an interior NUL byte (the caller truncates at
/// the first one first, matching the original's `c_str()` C-string
/// semantics). Ignores the result, matching the original.
#[cfg(feature = "sdl")]
pub fn show_simple_error(title: &[u8], message: &[u8]) {
    use std::ffi::CString;
    let Ok(title) = CString::new(title) else {
        return;
    };
    let Ok(message) = CString::new(message) else {
        return;
    };
    unsafe {
        ffi::SDL_ShowSimpleMessageBox(
            SDL_MESSAGEBOX_ERROR,
            title.as_ptr(),
            message.as_ptr(),
            std::ptr::null_mut(),
        );
    }
}

/// Replaces `YesNof`'s `SDL_ShowMessageBox` call (`FFeedbackContextSDL.h`):
/// an INFORMATION dialog titled "Confirmation" with Yes (id 1, both default
/// keys) / No (id 0) buttons. `message` must not contain an interior NUL
/// byte (the caller truncates at the first one first, matching the
/// original's `c_str()` C-string semantics). Returns whether Yes was
/// pressed; on an (unexpected) interior-NUL message or an
/// `SDL_ShowMessageBox` failure, returns `true`, matching the original's
/// `if (SDL_ShowMessageBox(...) < 0) return 1;`.
#[cfg(feature = "sdl")]
pub fn confirm_yes_no(message: &[u8]) -> bool {
    use std::ffi::CString;
    let Ok(message) = CString::new(message) else {
        return true;
    };
    let title = c"Confirmation";
    let yes = c"Yes";
    let no = c"No";
    let buttons = [
        ffi::SdlMessageBoxButtonData {
            flags: SDL_MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT | SDL_MESSAGEBOX_BUTTON_ESCAPEKEY_DEFAULT,
            buttonid: 1,
            text: yes.as_ptr(),
        },
        ffi::SdlMessageBoxButtonData {
            flags: 0,
            buttonid: 0,
            text: no.as_ptr(),
        },
    ];
    let data = ffi::SdlMessageBoxData {
        flags: SDL_MESSAGEBOX_INFORMATION,
        window: std::ptr::null_mut(),
        title: title.as_ptr(),
        message: message.as_ptr(),
        numbuttons: buttons.len() as std::os::raw::c_int,
        buttons: buttons.as_ptr(),
        color_scheme: std::ptr::null(),
    };
    let mut pressed: std::os::raw::c_int = 0;
    unsafe {
        if ffi::SDL_ShowMessageBox(&data, &mut pressed) < 0 {
            return true;
        }
    }
    pressed != 0
}

/// An information dialog with several buttons. `buttons[0]` is the default (Return) button and
/// `buttons[cancel]` answers Escape. Returns the index of the pressed button, or `None` when the
/// dialog could not be shown.
#[cfg(feature = "sdl")]
pub fn choose(title: &str, message: &str, buttons: &[&str], cancel: usize) -> Option<usize> {
    use std::ffi::CString;
    let title = CString::new(title).ok()?;
    let message = CString::new(message).ok()?;
    let texts = buttons.iter().map(|b| CString::new(*b)).collect::<Result<Vec<_>, _>>().ok()?;
    let data_buttons: Vec<ffi::SdlMessageBoxButtonData> = texts
        .iter()
        .enumerate()
        .map(|(i, text)| ffi::SdlMessageBoxButtonData {
            flags: if i == 0 { SDL_MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT } else { 0 } | if i == cancel { SDL_MESSAGEBOX_BUTTON_ESCAPEKEY_DEFAULT } else { 0 },
            buttonid: i as std::os::raw::c_int,
            text: text.as_ptr(),
        })
        .collect();
    let data = ffi::SdlMessageBoxData {
        flags: SDL_MESSAGEBOX_INFORMATION,
        window: std::ptr::null_mut(),
        title: title.as_ptr(),
        message: message.as_ptr(),
        numbuttons: data_buttons.len() as std::os::raw::c_int,
        buttons: data_buttons.as_ptr(),
        color_scheme: std::ptr::null(),
    };
    let mut pressed: std::os::raw::c_int = -1;
    if unsafe { ffi::SDL_ShowMessageBox(&data, &mut pressed) } < 0 || pressed < 0 {
        return None;
    }
    usize::try_from(pressed).ok()
}
