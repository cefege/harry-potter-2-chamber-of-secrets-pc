//! Stock menu display commands on the canonical SDL viewport.
//!
//! Windowed choices are limited by SDL window constraints and are verified
//! after resizing. They are not evidence of exclusive fullscreen support.
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_abi::engine::generated::{USDLViewport, UViewport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mode { width: i32, height: i32, bits: i32 }

/// SDL display modes are physical output modes, not fullscreen-desktop sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalDisplayMode {
    pub width_px: i32,
    pub height_px: i32,
    pub bits_per_pixel: i32,
    pub refresh_hz: i32,
}

/// Physical display information and conservative logical-window suggestions.
/// Window sizes are SDL window coordinates, not exclusive physical modes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DisplayCapabilities {
    pub display_index: i32,
    pub physical_modes: Vec<PhysicalDisplayMode>,
    pub usable_window_width: i32,
    pub usable_window_height: i32,
    pub max_texture_dimension: u32,
    pub windowed_presets: Vec<(i32, i32)>,
}

const STANDARD_LOGICAL_SIZES: &[(i32, i32)] = &[
    (640, 480), (800, 600), (1024, 768), (1280, 720), (1280, 800),
    (1280, 1024), (1366, 768), (1440, 900), (1600, 900), (1680, 1050),
    (1920, 1080), (1920, 1200), (2560, 1440), (2560, 1600), (3440, 1440), (3840, 2160),
];

fn windowed_presets(usable: (i32, i32), min: (i32, i32), max: (i32, i32), renderer_max: u32) -> Vec<(i32, i32)> {
    STANDARD_LOGICAL_SIZES.iter().copied().filter(|&(w, h)| {
        w <= usable.0 && h <= usable.1 && w >= min.0.max(1) && h >= min.1.max(1)
            && (max.0 <= 0 || w <= max.0) && (max.1 <= 0 || h <= max.1)
            && w as u32 <= renderer_max && h as u32 <= renderer_max
    }).collect()
}

/// SDL physical modes are used only as candidate dimensions. Fullscreen uses
/// them as logical scene sizes, not requests for exclusive display switches.
fn logical_choices(caps: &DisplayCapabilities, current: Mode, fullscreen: bool, window_allowed: impl Fn(i32, i32) -> bool) -> Vec<Mode> {
    let mut choices = vec![current];
    let allowed = |w: i32, h: i32| w > 0 && h > 0 && (w as u32) <= caps.max_texture_dimension
        && (h as u32) <= caps.max_texture_dimension
        && (fullscreen || (w <= caps.usable_window_width && h <= caps.usable_window_height && window_allowed(w, h)));
    let candidates = STANDARD_LOGICAL_SIZES.iter().copied()
        .chain(caps.physical_modes.iter().map(|m| (m.width_px, m.height_px)));
    for (width, height) in candidates {
        if allowed(width, height) {
            let mode = Mode { width, height, bits: current.bits };
            if !choices.contains(&mode) { choices.push(mode); }
        }
    }
    choices
}

trait Display {
    fn modes(&mut self) -> Result<Vec<Mode>, &'static str>;
    fn color_depths(&mut self) -> Result<Vec<i32>, &'static str> {
        Ok(self.modes()?.into_iter().map(|mode| mode.bits).collect())
    }
    fn current(&mut self) -> Result<Mode, &'static str>;
    fn set(&mut self, mode: Mode) -> Result<(), &'static str>;
}

fn response(display: &mut impl Display, command: &str) -> Option<String> {
    let (verb, _args) = command.trim().split_once(char::is_whitespace).unwrap_or((command.trim(), ""));
    if verb.eq_ignore_ascii_case("GETRES") {
        Some(match display.modes() {
            Ok(modes) => {
                let mut values = Vec::new();
                for mode in modes {
                    if mode.width > 0 && mode.height > 0 {
                        let value = format!("{}x{}", mode.width, mode.height);
                        if !values.contains(&value) { values.push(value); }
                    }
                }
                values.join(" ")
            }
            Err(e) => e.into(),
        })
    } else if verb.eq_ignore_ascii_case("GETCOLORDEPTHS") {
        Some(match display.color_depths() {
            Ok(depths) => {
                let mut values = Vec::new();
                for depth in depths {
                    if depth > 0 && !values.contains(&depth) { values.push(depth); }
                }
                values.sort_unstable();
                values.iter().map(i32::to_string).collect::<Vec<_>>().join(" ")
            }
            Err(e) => e.into(),
        })
    } else if verb.eq_ignore_ascii_case("GETCURRENTRES") {
        Some(match display.current() { Ok(m) => format!("{}x{}", m.width, m.height), Err(e) => e.into() })
    } else if verb.eq_ignore_ascii_case("GETCURRENTCOLORDEPTH") {
        Some(match display.current() { Ok(m) => m.bits.to_string(), Err(e) => e.into() })
    } else if verb.eq_ignore_ascii_case("SETRES") {
        let parsed: Vec<_> = _args.trim().split(['x', 'X']).collect();
        let mode = if parsed.len() == 3 {
            parsed[0].parse::<i32>().ok().zip(parsed[1].parse::<i32>().ok()).zip(parsed[2].parse::<i32>().ok())
                .map(|((width, height), bits)| Mode { width, height, bits })
        } else { None };
        Some(match mode {
            Some(mode) if mode.width > 0 && mode.height > 0 && mode.bits > 0 => match display.set(mode) {
                Ok(()) => String::new(),
                Err(e) => format!("SetRes failed: {e}"),
            },
            _ => "SetRes failed: expected WIDTHxHEIGHTxDEPTH".into(),
        })
    } else { None }
}

#[cfg(feature = "sdl")]
mod sdl {
    use super::*;
    use std::{cell::RefCell, ffi::c_void};
    use hp2_abi::engine::generated::{UClient, UObject};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum PreviewPhase { AwaitApply, Applied, RollbackArmed, RollingBack }
    #[derive(Clone, Copy)]
    struct Preview {
        viewport: *mut USDLViewport, viewport_index: i32, client: *mut UClient, client_index: i32,
        owner: usize, window: *mut c_void, old: Mode, requested: Mode, fullscreen: bool,
        display_mode: hp2_platform::live_display_mode::DisplayMode,
        confirmed: (i32, i32), phase: PreviewPhase,
    }
    thread_local! { static PREVIEWS: RefCell<Vec<Preview>> = const { RefCell::new(Vec::new()) }; }
    pub(super) fn has_preview(viewport: *mut USDLViewport) -> bool { snapshot_for(viewport).is_some() }
    fn snapshot_for(viewport: *mut USDLViewport) -> Option<Preview> {
        PREVIEWS.with(|previews| previews.borrow().iter().find(|p| p.viewport == viewport).copied())
    }
    pub(super) fn clear(viewport: *mut USDLViewport) { PREVIEWS.with(|previews| previews.borrow_mut().retain(|p| p.viewport != viewport)); }
    pub(super) fn set_phase(viewport: *mut USDLViewport, phase: PreviewPhase) {
        PREVIEWS.with(|previews| { if let Some(p) = previews.borrow_mut().iter_mut().find(|p| p.viewport == viewport) { p.phase = phase; } });
    }
    fn owner_id(access: CoreAccess<'_>) -> Result<usize, &'static str> {
        access.owner_context::<crate::core_owner::EngineState>().map(|owner| owner as *const _ as usize).ok_or("resolution engine owner unavailable")
    }
    fn validate_snapshot(access: CoreAccess<'_>, p: Preview) -> Result<(), &'static str> {
        if owner_id(access)? != p.owner || p.viewport_index < 0 || p.client_index < 0 || access.object_at(p.viewport_index) != p.viewport.cast()
            || access.object_at(p.client_index) != p.client.cast() { return Err("resolution transaction owner disappeared"); }
        unsafe {
            if (*p.viewport.cast::<UObject>()).object_flags & hp2_core::object_lifecycle::RF_DESTROYED != 0
                || (*p.client.cast::<UObject>()).object_flags & hp2_core::object_lifecycle::RF_DESTROYED != 0
                || (*p.viewport).base.outer.cast::<UClient>() != p.client || (*p.viewport).sdl_window.cast::<c_void>() != p.window {
                return Err("resolution transaction owner disappeared");
            }
        }
        Ok(())
    }
    #[repr(C)]
    struct SdlDisplayMode { format: u32, width: i32, height: i32, refresh_rate: i32, driver_data: *mut c_void }
    #[repr(C)]
    struct SdlRect { x: i32, y: i32, width: i32, height: i32 }
    unsafe extern "C" {
        fn SDL_GetWindowDisplayIndex(window: *mut c_void) -> i32;
        fn SDL_GetNumDisplayModes(display: i32) -> i32;
        fn SDL_GetDisplayMode(display: i32, index: i32, mode: *mut SdlDisplayMode) -> i32;
        fn SDL_GetDisplayUsableBounds(display: i32, rect: *mut SdlRect) -> i32;
        fn SDL_GetWindowSize(window: *mut c_void, w: *mut i32, h: *mut i32);
        fn SDL_GetWindowPixelFormat(window: *mut c_void) -> u32;
        fn SDL_GetWindowFlags(window: *mut c_void) -> u32;
        fn SDL_GetWindowMinimumSize(window: *mut c_void, w: *mut i32, h: *mut i32);
        fn SDL_GetWindowMaximumSize(window: *mut c_void, w: *mut i32, h: *mut i32);
        fn SDL_SetWindowSize(window: *mut c_void, w: i32, h: i32);
    }
    fn bits(format: u32) -> i32 { ((format >> 8) & 0xff) as i32 }
    pub(super) fn physical_display_info(window: *mut c_void) -> Result<(i32, (i32, i32), Vec<PhysicalDisplayMode>), &'static str> {
        let display_index = unsafe { SDL_GetWindowDisplayIndex(window) };
        if display_index < 0 { return Err("SDL window display is unavailable"); }
        let mut usable = SdlRect { x: 0, y: 0, width: 0, height: 0 };
        if unsafe { SDL_GetDisplayUsableBounds(display_index, &mut usable) } != 0 || usable.width <= 0 || usable.height <= 0 {
            return Err("SDL usable display bounds unavailable");
        }
        let count = unsafe { SDL_GetNumDisplayModes(display_index) };
        if count < 0 { return Err("SDL display modes unavailable"); }
        let mut physical_modes = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut mode = SdlDisplayMode { format: 0, width: 0, height: 0, refresh_rate: 0, driver_data: std::ptr::null_mut() };
            if unsafe { SDL_GetDisplayMode(display_index, index, &mut mode) } != 0 { return Err("SDL display mode unavailable"); }
            if mode.width <= 0 || mode.height <= 0 { return Err("SDL returned an invalid display mode"); }
            physical_modes.push(PhysicalDisplayMode { width_px: mode.width, height_px: mode.height,
                bits_per_pixel: bits(mode.format), refresh_hz: mode.refresh_rate });
        }
        Ok((display_index, (usable.width, usable.height), physical_modes))
    }
    pub(super) struct Backend<'a> { pub window: *mut c_void, pub access: CoreAccess<'a>, pub viewport: *mut USDLViewport }
    impl Backend<'_> {
        pub(super) fn capabilities(&self) -> Result<DisplayCapabilities, &'static str> {
            let (display_index, usable, physical_modes) = physical_display_info(self.window)?;
            let (mut min_w, mut min_h, mut max_w, mut max_h) = (0, 0, 0, 0);
            unsafe { SDL_GetWindowMinimumSize(self.window, &mut min_w, &mut min_h); SDL_GetWindowMaximumSize(self.window, &mut max_w, &mut max_h); }
            let max_texture_dimension = self.max_dimension()?;
            Ok(DisplayCapabilities { display_index, physical_modes, usable_window_width: usable.0,
                usable_window_height: usable.1, max_texture_dimension,
                windowed_presets: windowed_presets(usable, (min_w, min_h), (max_w, max_h), max_texture_dimension) })
        }
        fn display_mode(&self) -> hp2_platform::live_display_mode::DisplayMode {
            hp2_platform::live_display_mode::DisplayMode::from_sdl_flags(unsafe { SDL_GetWindowFlags(self.window) })
        }
        fn fullscreen(&self) -> bool { self.display_mode() != hp2_platform::live_display_mode::DisplayMode::Windowed }
        fn window_size_allowed(&self, width: i32, height: i32) -> bool {
            let (mut min_w, mut min_h, mut max_w, mut max_h) = (0, 0, 0, 0);
            unsafe { SDL_GetWindowMinimumSize(self.window, &mut min_w, &mut min_h); SDL_GetWindowMaximumSize(self.window, &mut max_w, &mut max_h); }
            width >= min_w.max(1) && height >= min_h.max(1)
                && (max_w == 0 || width <= max_w) && (max_h == 0 || height <= max_h)
        }
        fn max_dimension(&self) -> Result<u32, &'static str> {
            let device = unsafe { (*self.viewport).base.ren_dev };
            unsafe { crate::render_device_owner::max_texture_dimension_2d(self.access, device.cast(), &raw mut (*self.viewport).base) }
                .map_err(|_| "renderer limits unavailable")
        }
        unsafe fn resize(&self, mode: Mode) -> Result<(), &'static str> {
            let fullscreen = self.fullscreen();
            if !fullscreen { unsafe { SDL_SetWindowSize(self.window, mode.width, mode.height); } }
            let (mut w, mut h) = (0, 0);
            unsafe { SDL_GetWindowSize(self.window, &mut w, &mut h); }
            if !fullscreen && (w, h) != (mode.width, mode.height) { return Err("window manager rejected the size"); }
            let vp = self.viewport;
            unsafe {
                (*vp).base.size_x = mode.width; (*vp).base.size_y = mode.height;
                let scale = if fullscreen {
                    hp2_platform::viewport::fullscreen_mouse_mapping(w, h, mode.width, mode.height)
                        .map(|(x,y,_,_)| (x,y)).ok_or("window dimensions unavailable")?
                } else { hp2_platform::viewport::mouse_scale(w, h, mode.width, mode.height) };
                ((*vp).mouse_scale_x, (*vp).mouse_scale_y) = scale;
            }
            let device = unsafe { (*vp).base.ren_dev };
            if device.is_null() { return Err("renderer unavailable"); }
            let color = unsafe { (*vp).base.color_bytes };
            let result = unsafe { crate::render_device_owner::set_res(self.access, device.cast(), &raw mut (*vp).base, mode.width, mode.height, color, fullscreen) };
            match result { Ok(true) => Ok(()), _ => Err("renderer resize failed") }
        }
    }
    impl Display for Backend<'_> {
        fn modes(&mut self) -> Result<Vec<Mode>, &'static str> {
            let current = self.current()?;
            let caps = match self.capabilities() {
                Ok(caps) => caps,
                Err(error) => {
                    // The current live size is known even when discovery fails.
                    // Report the failure and expose no unverified alternatives.
                    let category = self.access.intern_str("Warning", hp2_core::name_owner::FindName::Add)
                        .map_err(|_| "cannot report display capability failure")?;
                    let message: Vec<u32> = format!("GetRes capability discovery failed: {error}; showing only current size").chars().map(u32::from).collect();
                    self.access.emit_log(category, &message).map_err(|_| "cannot report display capability failure")?;
                    return Ok(vec![current]);
                }
            };
            Ok(logical_choices(&caps, current, self.fullscreen(), |width, height| self.window_size_allowed(width, height)))
        }
        fn color_depths(&mut self) -> Result<Vec<i32>, &'static str> { Ok(vec![self.current()?.bits]) }
        fn current(&mut self) -> Result<Mode, &'static str> {
            let (mut width, mut height) = (0, 0);
            if self.fullscreen() {
                unsafe { width = (*self.viewport).base.size_x; height = (*self.viewport).base.size_y; }
            } else { unsafe { SDL_GetWindowSize(self.window, &mut width, &mut height); } }
            let depth = bits(unsafe { SDL_GetWindowPixelFormat(self.window) });
            if width <= 0 || height <= 0 || depth == 0 { return Err("SDL window mode unavailable"); }
            Ok(Mode { width, height, bits: depth })
        }
        fn set(&mut self, mode: Mode) -> Result<(), &'static str> {
            let preview = snapshot_for(self.viewport);
            if let Some(p) = preview {
                if validate_snapshot(self.access, p).is_err() { clear(self.viewport); return Err("resolution transaction owner disappeared"); }
                if self.display_mode() != p.display_mode {
                    if !matches!(p.phase, PreviewPhase::RollbackArmed | PreviewPhase::RollingBack) {
                        return Err("display mode changed during resolution confirmation");
                    }
                    set_phase(self.viewport, PreviewPhase::RollingBack);
                    if unsafe { hp2_platform::live_display_mode::set_sdl_mode(self.window.cast(), p.display_mode) }.is_err() {
                        let settings = format!("{}x{}x{}", p.old.width, p.old.height, p.old.bits);
                        if unsafe { crate::stock_resolution_deferred::queue_failed_rollback(self.access, self.viewport.cast(), &settings, "could not restore the previous display mode") }.is_err() {
                            return Err("display mode rollback failed and retry could not be queued");
                        }
                        return Err("could not restore the previous display mode");
                    }
                    unsafe {
                        if p.fullscreen { (*self.viewport).blit_flags |= hp2_platform::viewport::BLIT_FULLSCREEN; }
                        else { (*self.viewport).blit_flags &= !hp2_platform::viewport::BLIT_FULLSCREEN; }
                    }
                }
                match p.phase {
                    PreviewPhase::AwaitApply if mode == p.requested => {},
                    PreviewPhase::RollbackArmed | PreviewPhase::RollingBack if mode == p.old => {},
                    _ => return Err("resolution confirmation already pending"),
                }
            }
            let result = self.set_inner(mode, preview.is_none());
            match (preview, result) {
                (Some(p), Ok(())) if matches!(p.phase, PreviewPhase::RollbackArmed | PreviewPhase::RollingBack) => { clear(self.viewport); Ok(()) }
                (Some(_), Ok(())) => { set_phase(self.viewport, PreviewPhase::Applied); Ok(()) }
                (Some(p), Err(error)) => {
                    set_phase(self.viewport, PreviewPhase::RollingBack);
                    if !matches!(p.phase, PreviewPhase::RollbackArmed | PreviewPhase::RollingBack) && self.set_inner(p.old, false).is_ok() {
                        clear(self.viewport);
                        return Err(error);
                    }
                    let settings = format!("{}x{}x{}", p.old.width, p.old.height, p.old.bits);
                    if unsafe { crate::stock_resolution_deferred::queue_failed_rollback(self.access, self.viewport.cast(), &settings, error) }.is_err() {
                        return Err("resolution rollback failed and retry could not be queued");
                    }
                    Err(error)
                }
                (None, result) => result,
            }
        }
    }
    impl Backend<'_> {
        fn set_inner(&mut self, mode: Mode, persist: bool) -> Result<(), &'static str> {
            let fullscreen = self.fullscreen();
            if fullscreen != unsafe { hp2_platform::viewport::is_fullscreen((*self.viewport).blit_flags) } {
                return Err("fullscreen state is inconsistent");
            }
            let old = self.current()?;
            if mode.bits != old.bits { return Err("window color depth cannot be changed"); }
            let max = self.max_dimension()?;
            if mode.width as u32 > max || mode.height as u32 > max { return Err("unsupported renderer size"); }
            if !fullscreen && !self.window_size_allowed(mode.width, mode.height) { return Err("unsupported window size"); }
            let staged = if fullscreen {
                let vp = self.viewport;
                let device = unsafe { (*vp).base.ren_dev };
                Some(unsafe { crate::render_device_owner::prepare_fullscreen_targets(
                    self.access, device.cast(), &raw mut (*vp).base, mode.width as u32, mode.height as u32,
                ) }.map_err(|_| "renderer target allocation failed")?)
            } else { None };
            if let Err(e) = unsafe { self.resize(mode) } {
                if unsafe { self.resize(old) }.is_err() { return Err("resize failed and rollback failed"); }
                return Err(e);
            }
            if !persist {
                if let Some(staged) = staged {
                    let vp = self.viewport;
                    let device = unsafe { (*vp).base.ren_dev };
                    if unsafe { crate::render_device_owner::commit_fullscreen_targets(self.access, device.cast(), &raw mut (*vp).base, staged) }.is_err() {
                        if unsafe { self.resize(old) }.is_err() { return Err("renderer commit failed and rollback incomplete"); }
                        return Err("renderer commit failed");
                    }
                }
                return Ok(());
            }
            let client = unsafe { (*self.viewport).base.outer.cast::<hp2_abi::engine::generated::UClient>() };
            if client.is_null() {
                if unsafe { self.resize(old) }.is_err() { return Err("client unavailable and rollback failed"); }
                return Err("client unavailable");
            }
            let (config_x, config_y) = unsafe { if fullscreen {
                (&raw mut (*client).fullscreen_viewport_x, &raw mut (*client).fullscreen_viewport_y)
            } else { (&raw mut (*client).windowed_viewport_x, &raw mut (*client).windowed_viewport_y) } };
            let old_config = unsafe { (*config_x, *config_y) };
            unsafe { *config_x = mode.width; *config_y = mode.height; }
            if unsafe { self.access.save_config(client.cast(), 0x4000, None) }.is_err() {
                unsafe { *config_x = old_config.0; *config_y = old_config.1; }
                // SaveConfig can change its cache or flush some data before it
                // returns an error. Attempt to restore both config and renderer.
                let config_restored = unsafe { self.access.save_config(client.cast(), 0x4000, None) }.is_ok();
                let renderer_restored = unsafe { self.resize(old) }.is_ok();
                if !config_restored || !renderer_restored { return Err("config save failed and rollback incomplete"); }
                return Err("config save failed");
            }
            if let Some(staged) = staged {
                let vp = self.viewport;
                let device = unsafe { (*vp).base.ren_dev };
                if unsafe { crate::render_device_owner::commit_fullscreen_targets(self.access, device.cast(), &raw mut (*vp).base, staged) }.is_err() {
                    unsafe { *config_x = old_config.0; *config_y = old_config.1; }
                    let config_restored = unsafe { self.access.save_config(client.cast(), 0x4000, None) }.is_ok();
                    let renderer_restored = unsafe { self.resize(old) }.is_ok();
                    if !config_restored || !renderer_restored { return Err("renderer commit failed and rollback incomplete"); }
                    return Err("renderer commit failed");
                }
            }
            Ok(())
        }
    }

    pub(super) unsafe fn begin_preview(access: CoreAccess<'_>, viewport: *mut USDLViewport, width: i32, height: i32) -> RuntimeResult<()> {
        if viewport.is_null() || width <= 0 || height <= 0 { return Err(RuntimeError::new("Invalid resolution preview")); }
        if crate::stock_display_control::display_transition_pending() { return Err(RuntimeError::new("Finish display mode confirmation before changing resolution")); }
        if let Some(p) = snapshot_for(viewport) {
            if validate_snapshot(access, p).is_ok() { return Err(RuntimeError::new("Resolution confirmation already pending")); }
            clear(viewport);
        }
        let vp_index = unsafe { (*viewport.cast::<UObject>()).index };
        let client = unsafe { (*viewport).base.outer.cast::<UClient>() };
        let window = unsafe { (*viewport).sdl_window.cast::<c_void>() };
        if vp_index < 0 || access.object_at(vp_index) != viewport.cast() || client.is_null() || window.is_null() {
            return Err(RuntimeError::new("Resolution preview viewport unavailable"));
        }
        let client_index = unsafe { (*client.cast::<UObject>()).index };
        if client_index < 0 || access.object_at(client_index) != client.cast() { return Err(RuntimeError::new("Resolution preview client unavailable")); }
        let mut backend = Backend { window, access, viewport };
        let old = backend.current().map_err(RuntimeError::new)?;
        let fullscreen = backend.fullscreen();
        let confirmed = unsafe { if fullscreen { ((*client).fullscreen_viewport_x, (*client).fullscreen_viewport_y) }
            else { ((*client).windowed_viewport_x, (*client).windowed_viewport_y) } };
        let p = Preview { viewport, viewport_index: vp_index, client, client_index, owner: owner_id(access).map_err(RuntimeError::new)?, window, old,
            requested: Mode { width, height, bits: old.bits }, fullscreen, display_mode: backend.display_mode(), confirmed, phase: PreviewPhase::AwaitApply };
        validate_snapshot(access, p).map_err(RuntimeError::new)?;
        PREVIEWS.with(|previews| previews.borrow_mut().push(p));
        Ok(())
    }
    pub(super) unsafe fn confirm_preview(access: CoreAccess<'_>, viewport: *mut USDLViewport) -> RuntimeResult<()> {
        let p = snapshot_for(viewport).ok_or_else(|| RuntimeError::new("No resolution confirmation pending"))?;
        if let Err(error) = validate_snapshot(access, p) { clear(viewport); return Err(RuntimeError::new(error)); }
        let mut backend = Backend { window: p.window, access, viewport };
        let failure = if p.phase != PreviewPhase::Applied || backend.display_mode() != p.display_mode {
            Some("Resolution changed before confirmation".to_owned())
        } else {
            match backend.current() {
                Ok(current) if current == p.requested => None,
                Ok(_) => Some("Resolution changed before confirmation".to_owned()),
                Err(error) => Some(format!("Resolution could not be verified before confirmation: {error}")),
            }
        };
        let client_dims = unsafe { if p.fullscreen { ((*p.client).fullscreen_viewport_x, (*p.client).fullscreen_viewport_y) }
            else { ((*p.client).windowed_viewport_x, (*p.client).windowed_viewport_y) } };
        let failure = failure.or_else(|| (client_dims != p.confirmed).then(|| "Confirmed resolution changed during preview".to_owned()));
        if let Some(error) = failure { return Err(rollback_after_confirmation_error(&mut backend, p, &error)); }
        let units = |s: &str| s.chars().map(u32::from).collect::<Vec<_>>();
        let (x, y) = if p.fullscreen { ("FullscreenViewportX", "FullscreenViewportY") } else { ("WindowedViewportX", "WindowedViewportY") };
        let section = units("SDLDrv.SDLClient");
        let updates = [(section.clone(), units(x), units(&p.requested.width.to_string())), (section, units(y), units(&p.requested.height.to_string()))];
        if let Err(error) = access.config_update_system_staged(&updates) {
            return Err(rollback_after_confirmation_error(&mut backend, p, &format!("Resolution save failed: {error}")));
        }
        unsafe {
            if p.fullscreen { (*p.client).fullscreen_viewport_x = p.requested.width; (*p.client).fullscreen_viewport_y = p.requested.height; }
            else { (*p.client).windowed_viewport_x = p.requested.width; (*p.client).windowed_viewport_y = p.requested.height; }
        }
        clear(viewport);
        Ok(())
    }
    fn rollback_after_confirmation_error(backend: &mut Backend<'_>, p: Preview, reason: &str) -> RuntimeError {
        if p.phase == PreviewPhase::RollingBack {
            return RuntimeError::new(&format!("{reason}; previous resolution restore remains pending"));
        }
        set_phase(p.viewport, PreviewPhase::RollingBack);
        match backend.set(p.old) {
            Ok(()) => RuntimeError::new(&format!("{reason}; previous resolution restored")),
            Err(error) => RuntimeError::new(&format!("{reason}; previous resolution restore failed: {error}")),
        }
    }
    pub(super) unsafe fn arm_rollback(access: CoreAccess<'_>, viewport: *mut USDLViewport) -> RuntimeResult<()> {
        let p = snapshot_for(viewport).ok_or_else(|| RuntimeError::new("No resolution confirmation pending"))?;
        if let Err(error) = validate_snapshot(access, p) { clear(viewport); return Err(RuntimeError::new(error)); }
        if p.phase != PreviewPhase::Applied { return Err(RuntimeError::new("Resolution rollback is not ready")); }
        set_phase(viewport, PreviewPhase::RollbackArmed);
        Ok(())
    }
    pub(super) unsafe fn finish_rollback(access: CoreAccess<'_>, viewport: *mut USDLViewport) -> RuntimeResult<()> {
        let Some(p) = snapshot_for(viewport) else { return Ok(()) };
        if let Err(error) = validate_snapshot(access, p) { clear(viewport); return Err(RuntimeError::new(error)); }
        match p.phase {
            PreviewPhase::RollbackArmed => unsafe { rollback_preview(access, viewport) },
            PreviewPhase::RollingBack => Err(RuntimeError::new("Resolution rollback failed and is queued for retry")),
            _ => Err(RuntimeError::new("Resolution rollback was not armed")),
        }
    }
    pub(super) unsafe fn rollback_preview(access: CoreAccess<'_>, viewport: *mut USDLViewport) -> RuntimeResult<()> {
        let p = snapshot_for(viewport).ok_or_else(|| RuntimeError::new("No resolution confirmation pending"))?;
        if let Err(error) = validate_snapshot(access, p) { clear(viewport); return Err(RuntimeError::new(error)); }
        set_phase(viewport, PreviewPhase::RollingBack);
        Backend { window: p.window, access, viewport }.set(p.old).map_err(RuntimeError::new)
    }
}

/// Whether a resolution transaction owns this viewport. The display-mode
/// selector must not change fullscreen state while this is true.
pub(crate) fn preview_pending(viewport: *mut UViewport) -> bool {
    #[cfg(feature = "sdl")]
    { sdl::has_preview(viewport.cast()) }
    #[cfg(not(feature = "sdl"))]
    { let _ = viewport; false }
}

/// Start before the stock preview SetRes. Only the requested size is allowed
/// until confirmation; neither client config fields nor the INI are changed.
/// The caller must pass a live SDL viewport and must pair every successful
/// begin with confirm or rollback on the video thread.
pub(crate) unsafe fn begin_preview(access: CoreAccess<'_>, viewport: *mut UViewport, width: i32, height: i32) -> RuntimeResult<()> {
    #[cfg(feature = "sdl")]
    { unsafe { sdl::begin_preview(access, viewport.cast(), width, height) } }
    #[cfg(not(feature = "sdl"))]
    { let _ = (access, viewport, width, height); Err(RuntimeError::new("SDL display provider is disabled")) }
}
/// Commit only on the stock Yes result.
pub(crate) unsafe fn confirm_preview(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<()> {
    #[cfg(feature = "sdl")]
    { unsafe { sdl::confirm_preview(access, viewport.cast()) } }
    #[cfg(not(feature = "sdl"))]
    { let _ = (access, viewport); Err(RuntimeError::new("SDL display provider is disabled")) }
}
/// Before stock No, permit only its SetRes to the original live resolution.
/// Follow with finish_rollback after the script returns, even if it errors.
pub(crate) unsafe fn arm_rollback(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<()> {
    #[cfg(feature = "sdl")]
    { unsafe { sdl::arm_rollback(access, viewport.cast()) } }
    #[cfg(not(feature = "sdl"))]
    { let _ = (access, viewport); Err(RuntimeError::new("SDL display provider is disabled")) }
}
/// Finish stock No: restore if the script skipped SetRes, and retain failures.
pub(crate) unsafe fn finish_rollback(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<()> {
    #[cfg(feature = "sdl")]
    { unsafe { sdl::finish_rollback(access, viewport.cast()) } }
    #[cfg(not(feature = "sdl"))]
    { let _ = (access, viewport); Err(RuntimeError::new("SDL display provider is disabled")) }
}
/// Restore on No or timeout; a failed restore remains pending and is queued.
pub(crate) unsafe fn rollback_preview(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<()> {
    #[cfg(feature = "sdl")]
    { unsafe { sdl::rollback_preview(access, viewport.cast()) } }
    #[cfg(not(feature = "sdl"))]
    { let _ = (access, viewport); Err(RuntimeError::new("SDL display provider is disabled")) }
}

/// Query the SDL display that currently contains this live viewport.
///
/// Physical modes are SDL-reported pixels and refresh rates. Windowed presets
/// use SDL logical coordinates, work-area bounds, window constraints and the
/// renderer's logical dimension limit. High-DPI drawable allocation can still
/// fail at apply time, so callers must preserve the normal SetRes error path.
/// No physical mode here implies exclusive fullscreen support by the engine.
pub(crate) unsafe fn display_capabilities(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<DisplayCapabilities> {
    if viewport.is_null() || !unsafe { hp2_core::object_model::is_a(viewport.cast(), access.native_class("SDLDrv.SDLViewport")?.cast()) } {
        return Err(RuntimeError::new("Display capabilities require an SDL viewport"));
    }
    #[cfg(feature = "sdl")]
    {
        let viewport = viewport.cast::<USDLViewport>();
        let window = unsafe { (*viewport).sdl_window };
        if window.is_null() { return Err(RuntimeError::new("SDL window unavailable")); }
        sdl::Backend { window: window.cast(), access, viewport }.capabilities().map_err(RuntimeError::new)
    }
    #[cfg(not(feature = "sdl"))]
    { Err(RuntimeError::new("SDL display provider is disabled")) }
}

/// Read the same live dimensions exposed by GetCurrentRes.
pub(crate) unsafe fn current_resolution(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<Option<(i32, i32)>> {
    let class = access.native_class("SDLDrv.SDLViewport")?;
    if viewport.is_null() || !unsafe { hp2_core::object_model::is_a(viewport.cast(), class.cast()) } { return Ok(None); }
    #[cfg(feature = "sdl")]
    {
        let window = unsafe { (*viewport.cast::<USDLViewport>()).sdl_window };
        if window.is_null() { return Ok(None); }
        let mut backend = sdl::Backend { window: window.cast(), access, viewport: viewport.cast() };
        return Ok(backend.current().ok().map(|mode| (mode.width, mode.height)));
    }
    #[cfg(not(feature = "sdl"))]
    { Ok(None) }
}

/// Dispatch only for actual SDL viewports. The command and viewport must be live.
pub(super) unsafe fn exec(access: CoreAccess<'_>, viewport: *mut UViewport, command: *const u32, output: &mut crate::exec_output::ExecOutput<'_>) -> RuntimeResult<bool> {
    let class = access.native_class("SDLDrv.SDLViewport")?;
    if !unsafe { hp2_core::object_model::is_a(viewport.cast(), class.cast()) } { return Ok(false); }
    let mut len = 0usize;
    while unsafe { *command.add(len) } != 0 {
        len += 1;
        if len > 4096 { return Err(RuntimeError::new("SDL display command too long")); }
    }
    let units = unsafe { std::slice::from_raw_parts(command, len) };
    let text: String = units.iter().filter_map(|&c| char::from_u32(c)).collect();
    #[cfg(feature = "sdl")]
    {
        let window = unsafe { (*viewport.cast::<USDLViewport>()).sdl_window };
        if window.is_null() { return Ok(false); }
        let verb = text.trim().split_whitespace().next().unwrap_or("");
        if (verb.eq_ignore_ascii_case("TOGGLEFULLSCREEN") || verb.eq_ignore_ascii_case("ENDFULLSCREEN"))
            && (preview_pending(viewport) || crate::stock_display_control::display_transition_pending()) {
            output(&"Finish the display confirmation before changing fullscreen mode".chars().map(u32::from).collect::<Vec<_>>())?;
            return Ok(true);
        }
        if let Some(reply) = response(&mut sdl::Backend { window: window.cast(), access, viewport: viewport.cast() }, &text) {
            output(&reply.chars().map(u32::from).collect::<Vec<_>>())?;
            return Ok(true);
        }
    }
    #[cfg(not(feature = "sdl"))]
    { let _ = (text, output); }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Mock { current: Mode }
    impl Display for Mock {
        fn modes(&mut self) -> Result<Vec<Mode>, &'static str> { Ok(vec![Mode { width: 800, height: 600, bits: 32 }, Mode { width: 800, height: 600, bits: 32 }, Mode { width: 1920, height: 1080, bits: 24 }]) }
        fn current(&mut self) -> Result<Mode, &'static str> { Ok(self.current) }
        fn set(&mut self, mode: Mode) -> Result<(), &'static str> {
            if mode.width > 2000 || mode.bits != 32 { return Err("unsupported mode"); }
            self.current = mode;
            Ok(())
        }
    }
    fn mock() -> Mock { Mock { current: Mode { width: 777, height: 555, bits: 32 } } }
    #[test]
    fn menu_queries_keep_modes_and_window_distinct() {
        let mut m = mock();
        assert_eq!(response(&mut m, "GetRes").unwrap(), "800x600 1920x1080");
        assert_eq!(response(&mut m, "GetColorDepths").unwrap(), "24 32");
        assert_eq!(response(&mut m, "GetCurrentRes").unwrap(), "777x555");
        assert_eq!(response(&mut m, "GetCurrentColorDepth").unwrap(), "32");
    }
    #[test]
    fn modern_windowed_presets_respect_workarea_window_and_renderer_limits() {
        let choices = windowed_presets((1920, 1080), (700, 500), (1600, 1000), 1500);
        assert!(choices.contains(&(1280, 720)));
        assert!(choices.contains(&(1440, 900)));
        assert!(!choices.contains(&(640, 480))); // below minimum
        assert!(!choices.contains(&(1600, 900))); // above renderer limit
        assert!(!choices.contains(&(1920, 1080))); // above window maximum
        assert!(windowed_presets((500, 400), (0, 0), (0, 0), 4096).is_empty());
    }
    #[test]
    fn logical_choices_include_current_and_real_display_dimensions_with_bounds() {
        let caps = DisplayCapabilities { display_index: 0,
            physical_modes: vec![PhysicalDisplayMode { width_px: 1500, height_px: 900, bits_per_pixel: 32, refresh_hz: 60 },
                                 PhysicalDisplayMode { width_px: 3840, height_px: 2160, bits_per_pixel: 32, refresh_hz: 60 }],
            usable_window_width: 1920, usable_window_height: 1080, max_texture_dimension: 4096, windowed_presets: Vec::new() };
        let current = Mode { width: 2222, height: 1111, bits: 32 };
        let window = logical_choices(&caps, current, false, |w, _| w <= 1600);
        assert_eq!(window[0], current); // retained even outside the work area
        assert!(window.iter().any(|m| (m.width, m.height) == (1500, 900)));
        assert!(window.iter().any(|m| (m.width, m.height) == (1280, 720)));
        assert!(!window.iter().any(|m| (m.width, m.height) == (3840, 2160)));
        assert!(!window.iter().any(|m| (m.width, m.height) == (1920, 1080)));
        let fullscreen = logical_choices(&caps, current, true, |_, _| false);
        assert!(fullscreen.iter().any(|m| (m.width, m.height) == (3840, 2160)));
        assert_eq!(fullscreen.iter().filter(|m| (m.width, m.height) == (3840, 2160)).count(), 1);
        assert!(fullscreen.iter().all(|m| m.bits == current.bits));
        let mut restricted = caps;
        restricted.max_texture_dimension = 1024;
        let fullscreen = logical_choices(&restricted, current, true, |_, _| true);
        assert_eq!(fullscreen[0], current);
        assert!(!fullscreen.iter().any(|m| (m.width, m.height) == (1500, 900)));
    }
    #[cfg(feature = "sdl")]
    #[test]
    fn player_console_command_reaches_sdl_menu_queries() {
        // Cargo executes tests off the main thread. Cocoa SDL video requires
        // the main thread, so isolate this dispatch test in SDL's dummy driver.
        if std::env::var_os("HP2_DISPLAY_TEST_CHILD").is_none() {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "camera_owner::sdl_display_exec::tests::player_console_command_reaches_sdl_menu_queries", "--nocapture"])
                .env("HP2_DISPLAY_TEST_CHILD", "1")
                .env("SDL_VIDEODRIVER", "dummy")
                .status().unwrap();
            assert!(result.success(), "SDL dummy subprocess failed: {result}");
            return;
        }
        use std::{ffi::{c_char, c_void}, ptr};
        use hp2_abi::engine::generated::{APlayerPawn, TArrayRaw};
        unsafe extern "C" {
            fn SDL_Init(flags: u32) -> i32;
            fn SDL_Quit();
            fn SDL_CreateWindow(title: *const c_char, x: i32, y: i32, w: i32, h: i32, flags: u32) -> *mut c_void;
            fn SDL_DestroyWindow(window: *mut c_void);
            fn SDL_SetWindowFullscreen(window: *mut c_void, flags: u32) -> i32;
        }
        unsafe {
            assert_eq!(SDL_Init(0x20), 0);
            let window = SDL_CreateWindow(c"HP2 display command test".as_ptr(), 0, 0, 777, 555, 0x8);
            assert!(!window.is_null());
            let (display, usable, modes) = sdl::physical_display_info(window).unwrap();
            assert!(display >= 0 && usable.0 > 0 && usable.1 > 0);
            assert!(!modes.is_empty());
            assert!(modes.iter().all(|m| m.width_px > 0 && m.height_px > 0));
            let owner = crate::render_owner::render_test_support::owner("menu-display-route");
            let access = owner.access();
            let actor = crate::render_owner::render_test_support::construct(access, "Engine.Camera").cast::<APlayerPawn>();
            let client = crate::render_owner::render_test_support::construct(access, "SDLDrv.SDLClient");
            let class = access.native_class("SDLDrv.SDLViewport").unwrap();
            let vp = access.construct_object(class, client, 0, 0x4000, ptr::null_mut()).unwrap().cast::<USDLViewport>();
            (*vp).sdl_window = window.cast();
            (*vp).base.actor = actor.cast();
            (*actor).player = vp.cast();
            let query = |text: &str| {
                let cmd: Vec<u32> = text.chars().map(u32::from).chain([0]).collect();
                let mut out = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
                crate::script_console_owner::player_console_command(access, actor, cmd.as_ptr().cast(), (&raw mut out).cast()).unwrap();
                let chars = if out.num > 0 { std::slice::from_raw_parts(out.data.cast::<u32>(), out.num as usize - 1) } else { &[] };
                let result: String = chars.iter().filter_map(|&c| char::from_u32(c)).collect();
                access.destroy_owned_fstring(&raw mut out).unwrap();
                result
            };
            assert_eq!(query("GetCurrentRes"), "777x555");
            // With no renderer, discovery reports the failure and retains
            // only the known current size, not fabricated alternatives.
            let choices = query("GetRes");
            assert_eq!(choices, "777x555");
            assert!(query("GetCurrentColorDepth").parse::<u32>().unwrap() > 0);
            assert_eq!(SDL_SetWindowFullscreen(window, 0x1001), 0);
            (*vp).blit_flags |= hp2_platform::viewport::BLIT_FULLSCREEN;
            (*vp).base.size_x = 777;
            (*vp).base.size_y = 555;
            let fullscreen_current = query("GetCurrentRes");
            assert_eq!(fullscreen_current, "777x555");
            assert_eq!(query("GetRes"), fullscreen_current);
            let depth = query("GetCurrentColorDepth");
            assert!(query(&format!("SetRes 800x600x{depth}")).contains("renderer limits unavailable"));
            assert_eq!(query("GetCurrentRes"), fullscreen_current);
            // A timeout is processed after the tick even without another key event.
            // The renderer is deliberately absent, so a failed restoration
            // must remain available for the deferred retry path.
            let engine = crate::render_owner::render_test_support::construct(access, "Engine.GameEngine").cast::<hp2_abi::engine::generated::UGameEngine>();
            let console = crate::render_owner::render_test_support::construct(access, "HGame.HPConsole").cast::<hp2_abi::engine::generated::UConsole>();
            (*engine).client = client.cast();
            let mut list = [vp.cast::<UViewport>()];
            (*client.cast::<hp2_abi::engine::generated::UClient>()).viewports = TArrayRaw { data: list.as_mut_ptr().cast(), num: 1, max: 1 };
            (*vp).base.console = console.cast();
            (*console).viewport = vp.cast();
            let name = access.intern_str("ResTimeOutSettings", hp2_core::name_owner::FindName::Find).unwrap();
            let property = access.find_field((*console.cast::<hp2_abi::engine::generated::UObject>()).class.cast(), name, access.native_class("Core.StrProperty").unwrap());
            assert!(!property.is_null());
            let pending = access.property_address(console.cast(), property.cast()).unwrap().cast::<TArrayRaw>();
            let old: Vec<i32> = format!("800x600x{depth}").chars().map(|ch| ch as i32).collect();
            let mut pending_value = pending.read_unaligned();
            access.write_vm_fstring_result(&mut pending_value, &old).unwrap();
            pending.write_unaligned(pending_value);
            crate::stock_resolution_deferred::process(access, engine).unwrap();
            let expected_pending = pending.read_unaligned();
            assert_eq!(expected_pending.num as usize, old.len() + 1, "failed restore must remain pending");
            let actual = std::slice::from_raw_parts(expected_pending.data.cast::<i32>(), old.len());
            assert_eq!(actual, old.as_slice(), "retry must retain the exact original settings");
            crate::stock_resolution_deferred::process(access, engine).unwrap();
            let still_pending = pending.read_unaligned();
            assert_eq!(still_pending.num as usize, old.len() + 1, "a throttled retry must not consume the failed restore");
            assert_eq!(std::slice::from_raw_parts(still_pending.data.cast::<i32>(), old.len()), old.as_slice());
            // A live stock page belongs to this viewport through Root.Console.
            // Its resolution display must follow current mode, not startup config.
            let construct = |name| crate::render_owner::render_test_support::construct(access, name);
            let page = construct("HGame.FESoundVideoPage");
            let root = construct("UWindow.UWindowRootWindow");
            let combo = construct("UWindow.UWindowComboControl");
            let edit = construct("UWindow.UWindowEditBox");
            let address = |obj: *mut hp2_abi::engine::generated::UObject, name: &str, kind: &str| {
                let n = access.intern_str(name, hp2_core::name_owner::FindName::Find).unwrap();
                let p = access.find_field((*obj).class.cast(), n, access.native_class(kind).unwrap());
                assert!(!p.is_null(), "missing {name}");
                access.property_address(obj, p.cast()).unwrap()
            };
            address(page, "Root", "Core.ObjectProperty").cast::<*mut hp2_abi::engine::generated::UObject>().write_unaligned(root);
            address(page, "ResolutionCombo", "Core.ObjectProperty").cast::<*mut hp2_abi::engine::generated::UObject>().write_unaligned(combo);
            address(root, "Console", "Core.ObjectProperty").cast::<*mut hp2_abi::engine::generated::UObject>().write_unaligned(console.cast());
            address(combo, "EditBox", "Core.ObjectProperty").cast::<*mut hp2_abi::engine::generated::UObject>().write_unaligned(edit);
            let shown = address(edit, "Value", "Core.StrProperty").cast::<TArrayRaw>();
            let mut shown_value = shown.read_unaligned();
            access.write_vm_fstring_result(&mut shown_value, &[54, 52, 48, 120, 52, 56, 48]).unwrap();
            shown.write_unaligned(shown_value);
            crate::stock_resolution_display::sync(access, engine).unwrap();
            let shown_value = shown.read_unaligned();
            let chars = std::slice::from_raw_parts(shown_value.data.cast::<u32>(), shown_value.num as usize - 1);
            assert_eq!(chars.iter().map(|&c| char::from_u32(c).unwrap()).collect::<String>(), "777x555");
            (*vp).base.size_x = 640; (*vp).base.size_y = 480;
            crate::stock_resolution_display::sync(access, engine).unwrap();
            let shown_value = shown.read_unaligned();
            let chars = std::slice::from_raw_parts(shown_value.data.cast::<u32>(), shown_value.num as usize - 1);
            assert_eq!(chars.iter().map(|&c| char::from_u32(c).unwrap()).collect::<String>(), "640x480");
            assert_eq!(SDL_SetWindowFullscreen(window, 0), 0);
            (*vp).blit_flags &= !hp2_platform::viewport::BLIT_FULLSCREEN;
            let windowed_current = query("GetCurrentRes");
            crate::stock_resolution_display::sync(access, engine).unwrap();
            let shown_value = shown.read_unaligned();
            let chars = std::slice::from_raw_parts(shown_value.data.cast::<u32>(), shown_value.num as usize - 1);
            assert_eq!(chars.iter().map(|&c| char::from_u32(c).unwrap()).collect::<String>(), windowed_current);
            (*vp).base.console = ptr::null_mut();
            (*console).viewport = ptr::null_mut();
            (*client.cast::<hp2_abi::engine::generated::UClient>()).viewports = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            (*engine).client = ptr::null_mut();
            (*actor).player = ptr::null_mut();
            (*vp).sdl_window = ptr::null_mut();
            SDL_DestroyWindow(window);
            drop(owner);
            SDL_Quit();
        }
    }
    #[cfg(feature = "sdl")]
    #[test]
    fn preview_failure_keeps_confirmed_client_and_disk_and_queues_rollback() {
        const NAME: &str = "camera_owner::sdl_display_exec::tests::preview_failure_keeps_confirmed_client_and_disk_and_queues_rollback";
        if std::env::var_os("HP2_PREVIEW_TEST_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", NAME, "--nocapture"])
                .env("HP2_PREVIEW_TEST_CHILD", "1").env("SDL_VIDEODRIVER", "dummy").status().unwrap();
            assert!(status.success(), "SDL dummy preview subprocess failed: {status}");
            return;
        }
        use std::{ffi::{c_char, c_void}, ptr};
        use hp2_abi::engine::generated::{UClient, UConsole};
        unsafe extern "C" {
            fn SDL_Init(flags: u32) -> i32;
            fn SDL_Quit();
            fn SDL_CreateWindow(title: *const c_char, x: i32, y: i32, w: i32, h: i32, flags: u32) -> *mut c_void;
            fn SDL_DestroyWindow(window: *mut c_void);
        }
        unsafe {
            assert_eq!(SDL_Init(0x20), 0);
            let window = SDL_CreateWindow(c"Preview failure test".as_ptr(), 0, 0, 777, 555, 0x8);
            assert!(!window.is_null());
            let owner = crate::render_owner::render_test_support::owner("preview-failure");
            let access = owner.access();
            let client = crate::render_owner::render_test_support::construct(access, "SDLDrv.SDLClient").cast::<UClient>();
            (*client).windowed_viewport_x = 1024; (*client).windowed_viewport_y = 768;
            let class = access.native_class("SDLDrv.SDLViewport").unwrap();
            let vp = access.construct_object(class, client.cast(), 0, 0x4000, ptr::null_mut()).unwrap().cast::<USDLViewport>();
            (*vp).sdl_window = window.cast();
            let console = crate::render_owner::render_test_support::construct(access, "HGame.HPConsole").cast::<UConsole>();
            (*console).viewport = vp.cast(); (*vp).base.console = console.cast();
            let ini = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
                .join(format!("render-preview-failure-{}/System/Default.ini", std::process::id()));
            let before = std::fs::read(&ini).unwrap();
            begin_preview(access, vp.cast(), 800, 600).unwrap();
            let mut backend = sdl::Backend { window, access, viewport: vp };
            let current = backend.current().unwrap();
            assert_eq!(backend.set(Mode { width: 900, height: 600, bits: current.bits }), Err("resolution confirmation already pending"));
            assert_eq!(backend.set(Mode { width: 800, height: 600, bits: current.bits }), Err("renderer limits unavailable"));
            assert!(begin_preview(access, vp.cast(), 640, 480).is_err(), "failed rollback must retain the transaction");
            assert!(rollback_preview(access, vp.cast()).is_err(), "renderer remains unavailable");
            assert_eq!(((*client).windowed_viewport_x, (*client).windowed_viewport_y), (1024, 768));
            assert_eq!(std::fs::read(&ini).unwrap(), before, "failed preview or rollback must not write the INI");
            sdl::clear(vp);
            (*vp).base.console = ptr::null_mut(); (*console).viewport = ptr::null_mut(); (*vp).sdl_window = ptr::null_mut();
            SDL_DestroyWindow(window);
            drop(owner); SDL_Quit();
        }
    }
    #[cfg(feature = "sdl")]
    #[test]
    fn preview_confirmation_atomically_commits_config_only_on_yes() {
        const NAME: &str = "camera_owner::sdl_display_exec::tests::preview_confirmation_atomically_commits_config_only_on_yes";
        if std::env::var_os("HP2_CONFIRM_TEST_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", NAME, "--nocapture"])
                .env("HP2_CONFIRM_TEST_CHILD", "1").env("SDL_VIDEODRIVER", "dummy").status().unwrap();
            assert!(status.success(), "SDL dummy confirmation subprocess failed: {status}");
            return;
        }
        use std::{ffi::{c_char, c_void}, ptr};
        use hp2_abi::engine::generated::{UClient, UConsole};
        unsafe extern "C" {
            fn SDL_Init(flags: u32) -> i32; fn SDL_Quit();
            fn SDL_CreateWindow(title: *const c_char, x: i32, y: i32, w: i32, h: i32, flags: u32) -> *mut c_void;
            fn SDL_DestroyWindow(window: *mut c_void); fn SDL_SetWindowSize(window: *mut c_void, w: i32, h: i32);
        }
        unsafe {
            assert_eq!(SDL_Init(0x20), 0);
            let window = SDL_CreateWindow(c"Preview confirmation test".as_ptr(), 0, 0, 777, 555, 0x8);
            assert!(!window.is_null());
            let owner = crate::render_owner::render_test_support::owner("preview-confirm"); let access = owner.access();
            let client = crate::render_owner::render_test_support::construct(access, "SDLDrv.SDLClient").cast::<UClient>();
            (*client).windowed_viewport_x = 1024; (*client).windowed_viewport_y = 768;
            let vp = access.construct_object(access.native_class("SDLDrv.SDLViewport").unwrap(), client.cast(), 0, 0x4000, ptr::null_mut()).unwrap().cast::<USDLViewport>();
            (*vp).sdl_window = window.cast();
            let console = crate::render_owner::render_test_support::construct(access, "HGame.HPConsole").cast::<UConsole>();
            (*console).viewport = vp.cast(); (*vp).base.console = console.cast();
            let ini = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
                .join(format!("render-preview-confirm-{}/System/Default.ini", std::process::id()));
            let before = std::fs::read(&ini).unwrap();
            begin_preview(access, vp.cast(), 800, 600).unwrap();
            SDL_SetWindowSize(window, 800, 600); // Simulate a successful preview; this does not test WGPU.
            sdl::set_phase(vp, sdl::PreviewPhase::Applied);
            assert_eq!(std::fs::read(&ini).unwrap(), before);
            assert_eq!(((*client).windowed_viewport_x, (*client).windowed_viewport_y), (1024, 768));
            confirm_preview(access, vp.cast()).unwrap();
            assert_eq!(((*client).windowed_viewport_x, (*client).windowed_viewport_y), (800, 600));
            let section: Vec<u32> = "SDLDrv.SDLClient".chars().map(u32::from).collect();
            let get = |key: &str| -> String { access.config_get_units_from(None, &section, &key.chars().map(u32::from).collect::<Vec<_>>()).unwrap().unwrap().into_iter().filter_map(char::from_u32).collect() };
            assert_eq!(get("WindowedViewportX"), "800"); assert_eq!(get("WindowedViewportY"), "600");
            assert!(!preview_pending(vp.cast()));
            begin_preview(access, vp.cast(), 900, 600).unwrap();
            SDL_SetWindowSize(window, 900, 600);
            sdl::set_phase(vp, sdl::PreviewPhase::Applied);
            let mut external = std::fs::read(&ini).unwrap(); external.extend_from_slice(b"\n; external edit\n");
            std::fs::write(&ini, &external).unwrap();
            let error = confirm_preview(access, vp.cast()).unwrap_err().to_string();
            assert!(error.contains("Resolution save failed"), "{error}");
            assert_eq!(std::fs::read(&ini).unwrap(), external);
            assert_eq!(((*client).windowed_viewport_x, (*client).windowed_viewport_y), (800, 600));
            assert!(preview_pending(vp.cast()), "failed restore must remain queued");
            sdl::clear(vp);
            (*vp).base.console = ptr::null_mut(); (*console).viewport = ptr::null_mut();
            (*vp).sdl_window = ptr::null_mut(); SDL_DestroyWindow(window); drop(owner); SDL_Quit();
        }
    }
    #[cfg(feature = "sdl")]
    #[test]
    fn resize_before_yes_schedules_restore_and_preserves_confirmed_settings() {
        const NAME: &str = "camera_owner::sdl_display_exec::tests::resize_before_yes_schedules_restore_and_preserves_confirmed_settings";
        if std::env::var_os("HP2_RESIZED_CONFIRM_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", NAME, "--nocapture"])
                .env("HP2_RESIZED_CONFIRM_CHILD", "1").env("SDL_VIDEODRIVER", "dummy").status().unwrap();
            assert!(status.success(), "SDL dummy resized-confirm subprocess failed: {status}"); return;
        }
        use std::{ffi::{c_char, c_void}, ptr};
        use hp2_abi::engine::generated::{UClient, UConsole};
        unsafe extern "C" {
            fn SDL_Init(flags: u32) -> i32; fn SDL_Quit();
            fn SDL_CreateWindow(title: *const c_char, x: i32, y: i32, w: i32, h: i32, flags: u32) -> *mut c_void;
            fn SDL_DestroyWindow(window: *mut c_void); fn SDL_SetWindowSize(window: *mut c_void, w: i32, h: i32);
        }
        unsafe {
            assert_eq!(SDL_Init(0x20), 0);
            let window = SDL_CreateWindow(c"Resized confirmation test".as_ptr(), 0, 0, 777, 555, 0x8);
            assert!(!window.is_null());
            let owner = crate::render_owner::render_test_support::owner("resized-confirm"); let access = owner.access();
            let client = crate::render_owner::render_test_support::construct(access, "SDLDrv.SDLClient").cast::<UClient>();
            (*client).windowed_viewport_x = 1024; (*client).windowed_viewport_y = 768;
            let vp = access.construct_object(access.native_class("SDLDrv.SDLViewport").unwrap(), client.cast(), 0, 0x4000, ptr::null_mut()).unwrap().cast::<USDLViewport>();
            (*vp).sdl_window = window.cast();
            let console = crate::render_owner::render_test_support::construct(access, "HGame.HPConsole").cast::<UConsole>();
            (*console).viewport = vp.cast(); (*vp).base.console = console.cast();
            let ini = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("render-resized-confirm-{}/System/Default.ini", std::process::id()));
            let before = std::fs::read(&ini).unwrap();
            begin_preview(access, vp.cast(), 800, 600).unwrap();
            SDL_SetWindowSize(window, 800, 600); // Simulate successful preview, not WGPU.
            sdl::set_phase(vp, sdl::PreviewPhase::Applied);
            SDL_SetWindowSize(window, 850, 650); // Simulate a manual border resize before Yes.
            let error = confirm_preview(access, vp.cast()).unwrap_err().to_string();
            assert!(error.contains("Resolution changed before confirmation"), "{error}");
            assert!(error.contains("previous resolution restore failed"), "{error}");
            assert!(preview_pending(vp.cast()), "failed rollback must remain queued");
            assert_eq!(((*client).windowed_viewport_x, (*client).windowed_viewport_y), (1024, 768));
            assert_eq!(std::fs::read(&ini).unwrap(), before);
            let retry = confirm_preview(access, vp.cast()).unwrap_err().to_string();
            assert!(retry.contains("restore remains pending"), "{retry}");
            sdl::clear(vp);
            (*vp).base.console = ptr::null_mut(); (*console).viewport = ptr::null_mut();
            (*vp).sdl_window = ptr::null_mut(); SDL_DestroyWindow(window); drop(owner); SDL_Quit();
        }
    }
    #[cfg(feature = "sdl")]
    #[test]
    fn fullscreen_conflict_is_blocked_and_external_change_restores_original_mode() {
        const NAME: &str = "camera_owner::sdl_display_exec::tests::fullscreen_conflict_is_blocked_and_external_change_restores_original_mode";
        if std::env::var_os("HP2_FULLSCREEN_CONFLICT_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", NAME, "--nocapture"])
                .env("HP2_FULLSCREEN_CONFLICT_CHILD", "1").env("SDL_VIDEODRIVER", "dummy").status().unwrap();
            assert!(status.success(), "SDL dummy fullscreen-conflict subprocess failed: {status}"); return;
        }
        use std::{ffi::{c_char, c_void}, ptr};
        use hp2_abi::engine::generated::{UClient, UConsole};
        unsafe extern "C" {
            fn SDL_Init(flags: u32) -> i32; fn SDL_Quit();
            fn SDL_CreateWindow(title: *const c_char, x: i32, y: i32, w: i32, h: i32, flags: u32) -> *mut c_void;
            fn SDL_DestroyWindow(window: *mut c_void); fn SDL_SetWindowFullscreen(window: *mut c_void, flags: u32) -> i32;
            fn SDL_GetWindowFlags(window: *mut c_void) -> u32;
        }
        unsafe {
            assert_eq!(SDL_Init(0x20), 0);
            let window = SDL_CreateWindow(c"Fullscreen conflict test".as_ptr(), 0, 0, 777, 555, 0x8);
            assert!(!window.is_null());
            let owner = crate::render_owner::render_test_support::owner("fullscreen-preview-conflict"); let access = owner.access();
            let client = crate::render_owner::render_test_support::construct(access, "SDLDrv.SDLClient").cast::<UClient>();
            (*client).windowed_viewport_x = 1024; (*client).windowed_viewport_y = 768;
            let vp = access.construct_object(access.native_class("SDLDrv.SDLViewport").unwrap(), client.cast(), 0, 0x4000, ptr::null_mut()).unwrap().cast::<USDLViewport>();
            (*vp).sdl_window = window.cast();
            let console = crate::render_owner::render_test_support::construct(access, "HGame.HPConsole").cast::<UConsole>();
            (*console).viewport = vp.cast(); (*vp).base.console = console.cast();
            begin_preview(access, vp.cast(), 800, 600).unwrap();
            let cmd: Vec<u32> = "TOGGLEFULLSCREEN\0".chars().map(u32::from).collect();
            let mut reply = Vec::new();
            assert!(exec(access, vp.cast(), cmd.as_ptr(), &mut |units| { reply.extend_from_slice(units); Ok(()) }).unwrap());
            assert!(reply.iter().copied().filter_map(char::from_u32).collect::<String>().contains("Finish the display confirmation"));
            assert_eq!(SDL_GetWindowFlags(window) & 0x1001, 0);
            // Simulate an independent OS/SDL change that bypasses the command.
            sdl::set_phase(vp, sdl::PreviewPhase::Applied);
            assert_eq!(SDL_SetWindowFullscreen(window, 0x1001), 0);
            (*vp).blit_flags |= hp2_platform::viewport::BLIT_FULLSCREEN;
            let error = confirm_preview(access, vp.cast()).unwrap_err().to_string();
            assert!(error.contains("Resolution changed before confirmation"), "{error}");
            assert_eq!(SDL_GetWindowFlags(window) & 0x1001, 0, "restore must first recover the original SDL display mode");
            assert!(!hp2_platform::viewport::is_fullscreen((*vp).blit_flags));
            assert!(preview_pending(vp.cast()), "missing renderer queues the remaining logical-size restoration");
            assert_eq!(((*client).windowed_viewport_x, (*client).windowed_viewport_y), (1024, 768));
            sdl::clear(vp);
            (*vp).base.console = ptr::null_mut(); (*console).viewport = ptr::null_mut(); (*vp).sdl_window = ptr::null_mut();
            SDL_DestroyWindow(window); drop(owner); SDL_Quit();
        }
    }
    #[test]
    fn menu_apply_reject_and_revert() {
        let mut m = mock();
        let before = format!("{}x{}", response(&mut m, "GetCurrentRes").unwrap(), response(&mut m, "GetCurrentColorDepth").unwrap());
        assert_eq!(response(&mut m, "SetRes 800x600x32").unwrap(), "");
        assert_eq!(response(&mut m, "GetCurrentRes").unwrap(), "800x600");
        assert!(response(&mut m, "SetRes 3000x4000x32").unwrap().contains("unsupported"));
        assert!(response(&mut m, "SetRes 640x480x16").unwrap().contains("unsupported"));
        assert!(response(&mut m, "SetRes invalid").unwrap().contains("expected"));
        assert_eq!(response(&mut m, &format!("SetRes {before}")).unwrap(), "");
        assert_eq!(response(&mut m, "GetCurrentRes").unwrap(), "777x555");
        assert!(response(&mut m, "GetResExtra").is_none());
    }
}
