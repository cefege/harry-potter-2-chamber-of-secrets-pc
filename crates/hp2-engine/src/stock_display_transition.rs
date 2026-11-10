//! Live SDL/WGPU mode transition backend for the stock graphics page.
//!
//! The owner must keep the backend and transaction alive through confirmation
//! or successful rollback. This module does not install a menu or startup hook.
use hp2_abi::engine::generated::{UObject, USDLClient, USDLViewport};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_platform::live_display_mode::{Backend, DisplayMode};
use hp2_platform::{viewport, window_lifecycle};

fn units(s: &str) -> Vec<u32> { s.chars().map(u32::from).collect() }
fn name(mode: DisplayMode) -> &'static str {
    match mode { DisplayMode::Windowed => "Windowed", DisplayMode::ExclusiveFullscreen => "ExclusiveFullscreen", DisplayMode::BorderlessFullscreen => "BorderlessFullscreen" }
}
pub(crate) fn startup_mode(explicit: Option<DisplayMode>, startup_fullscreen: bool, temporary: bool) -> DisplayMode {
    if temporary { return DisplayMode::Windowed; }
    explicit.unwrap_or(if startup_fullscreen { DisplayMode::BorderlessFullscreen } else { DisplayMode::Windowed })
}
fn parse(s: &str) -> Option<DisplayMode> {
    if s.eq_ignore_ascii_case("Windowed") { Some(DisplayMode::Windowed) }
    else if s.eq_ignore_ascii_case("ExclusiveFullscreen") { Some(DisplayMode::ExclusiveFullscreen) }
    else if s.eq_ignore_ascii_case("BorderlessFullscreen") { Some(DisplayMode::BorderlessFullscreen) }
    else { None }
}

/// Read only the explicit new preference. Absence preserves legacy startup.
pub(crate) fn explicit_mode(access: CoreAccess<'_>) -> RuntimeResult<Option<DisplayMode>> {
    let value = access.config_get_units_from(None, &units("SDLDrv.SDLClient"), &units("HP2DisplayMode"))?;
    let Some(value) = value else { return Ok(None) };
    let text: String = value.into_iter().map(char::from_u32).collect::<Option<_>>().ok_or_else(|| RuntimeError::new("Invalid display mode config text"))?;
    parse(&text).map(Some).ok_or_else(|| RuntimeError::new("Invalid HP2DisplayMode preference"))
}
/// Missing new preference retains the historic StartupFullscreen behaviour.
pub(crate) fn saved_mode(access: CoreAccess<'_>, client: *mut USDLClient) -> RuntimeResult<DisplayMode> {
    if client.is_null() { return Err(RuntimeError::new("Display mode client unavailable")); }
    Ok(explicit_mode(access)?.unwrap_or_else(|| if unsafe { (*client).startup_fullscreen != 0 } { DisplayMode::BorderlessFullscreen } else { DisplayMode::Windowed }))
}

/// Weak snapshot retained across ticks. Revalidate it before every use.
#[derive(Clone, Copy)]
pub(crate) struct ViewportSnapshot {
    viewport: *mut USDLViewport, viewport_index: i32, client: *mut USDLClient, client_index: i32,
    window: *mut window_lifecycle::SdlWindow,
    original: DisplayMode, logical: (i32, i32), window_size: (i32, i32), window_position: (i32, i32),
}
impl ViewportSnapshot {
    /// Compare registry identities before dereferencing either saved pointer.
    pub(crate) unsafe fn owner_alive(&self, access: CoreAccess<'_>) -> bool {
        if self.viewport_index < 0 || self.client_index < 0
            || access.object_at(self.viewport_index) != self.viewport.cast()
            || access.object_at(self.client_index) != self.client.cast() { return false; }
        unsafe {
            if (*self.viewport.cast::<UObject>()).object_flags & hp2_core::object_lifecycle::RF_DESTROYED != 0
                || (*self.client.cast::<UObject>()).object_flags & hp2_core::object_lifecycle::RF_DESTROYED != 0 { return false; }
            (*self.viewport).base.outer.cast::<USDLClient>() == self.client
                && (*self.viewport).sdl_window.cast::<window_lifecycle::SdlWindow>() == self.window
        }
    }
}
/// A backend borrows Core only for one synchronous operation.
pub(crate) struct ViewportBackend<'a> {
    access: CoreAccess<'a>, viewport: *mut USDLViewport, client: *mut USDLClient,
    original: DisplayMode, logical: (i32, i32), window_size: (i32, i32), window_position: (i32, i32),
}
impl<'a> ViewportBackend<'a> {
    pub(crate) unsafe fn new(access: CoreAccess<'a>, viewport: *mut USDLViewport) -> RuntimeResult<Self> {
        if viewport.is_null() { return Err(RuntimeError::new("SDL viewport unavailable")); }
        let window = unsafe { (*viewport).sdl_window.cast::<window_lifecycle::SdlWindow>() };
        let client = unsafe { (*viewport).base.outer.cast::<USDLClient>() };
        if window.is_null() || client.is_null() || unsafe { (*viewport).base.ren_dev.is_null() } { return Err(RuntimeError::new("SDL viewport or renderer unavailable")); }
        let flags = unsafe { window_lifecycle::get_window_flags(window) };
        let original = DisplayMode::from_sdl_flags(flags);
        if (original != DisplayMode::Windowed) != unsafe { viewport::is_fullscreen((*viewport).blit_flags) } { return Err(RuntimeError::new("SDL and viewport fullscreen disagree")); }
        let logical = unsafe { ((*viewport).base.size_x, (*viewport).base.size_y) };
        if logical.0 <= 0 || logical.1 <= 0 { return Err(RuntimeError::new("Invalid viewport size")); }
        unsafe extern "C" { fn SDL_GetWindowPosition(window: *mut window_lifecycle::SdlWindow, x: *mut i32, y: *mut i32); }
        let mut x = 0; let mut y = 0;
        unsafe { SDL_GetWindowPosition(window, &mut x, &mut y); }
        let window_size = unsafe { window_lifecycle::get_window_size(window) };
        Ok(Self { access, viewport, client, original, logical, window_size, window_position: (x, y) })
    }
    #[cfg(test)]
    pub(crate) unsafe fn snapshot_for_test(viewport: *mut USDLViewport, client: *mut USDLClient) -> ViewportSnapshot {
        ViewportSnapshot { viewport, viewport_index: unsafe { (*viewport.cast::<UObject>()).index }, client,
            client_index: unsafe { (*client.cast::<UObject>()).index }, window: unsafe { (*viewport).sdl_window.cast() },
            original: DisplayMode::Windowed, logical: (640, 480), window_size: (640, 480), window_position: (0, 0) }
    }
    pub(crate) fn snapshot(&self) -> ViewportSnapshot {
        ViewportSnapshot { viewport: self.viewport, viewport_index: unsafe { (*self.viewport.cast::<UObject>()).index }, client: self.client,
            client_index: unsafe { (*self.client.cast::<UObject>()).index }, window: self.window(), original: self.original,
            logical: self.logical, window_size: self.window_size, window_position: self.window_position }
    }
    /// Rebuild only while both weak identities and the SDL window still match.
    pub(crate) unsafe fn from_snapshot(access: CoreAccess<'a>, snapshot: ViewportSnapshot) -> RuntimeResult<Self> {
        if !unsafe { snapshot.owner_alive(access) } { return Err(RuntimeError::new("Display transaction owner disappeared")); }
        if unsafe { (*snapshot.viewport).base.ren_dev.is_null() } { return Err(RuntimeError::new("Display transaction renderer unavailable")); }
        Ok(Self { access, viewport: snapshot.viewport, client: snapshot.client, original: snapshot.original,
            logical: snapshot.logical, window_size: snapshot.window_size, window_position: snapshot.window_position })
    }
    fn window(&self) -> *mut window_lifecycle::SdlWindow { unsafe { (*self.viewport).sdl_window.cast() } }
}
impl Backend for ViewportBackend<'_> {
    type Error = RuntimeError;
    fn current(&mut self) -> RuntimeResult<DisplayMode> {
        let mode = DisplayMode::from_sdl_flags(unsafe { window_lifecycle::get_window_flags(self.window()) });
        if (mode != DisplayMode::Windowed) != unsafe { viewport::is_fullscreen((*self.viewport).blit_flags) } { return Err(RuntimeError::new("SDL and viewport fullscreen disagree")); }
        Ok(mode)
    }
    fn saved(&mut self) -> RuntimeResult<DisplayMode> { saved_mode(self.access, self.client) }
    fn apply(&mut self, mode: DisplayMode) -> RuntimeResult<()> {
        unsafe extern "C" { fn SDL_SetWindowPosition(window: *mut window_lifecycle::SdlWindow, x: i32, y: i32); }
        let vp = self.viewport;
        let fullscreen = mode != DisplayMode::Windowed;
        let device = unsafe { (*vp).base.ren_dev };
        // Allocate fullscreen targets before changing the SDL window.
        let staged = if fullscreen { Some(unsafe { crate::render_device_owner::prepare_fullscreen_targets(self.access, device.cast(), &raw mut (*vp).base, self.logical.0 as u32, self.logical.1 as u32)? }) } else { None };
        unsafe { hp2_platform::live_display_mode::set_sdl_mode(self.window(), mode) }.map_err(|e| RuntimeError::new(&e))?;
        if mode == DisplayMode::Windowed && self.original == DisplayMode::Windowed {
            unsafe { window_lifecycle::set_window_size(self.window(), self.window_size.0, self.window_size.1); SDL_SetWindowPosition(self.window(), self.window_position.0, self.window_position.1); }
        }
        let (w, h) = unsafe { window_lifecycle::get_window_size(self.window()) };
        if w <= 0 || h <= 0 { return Err(RuntimeError::new("SDL window size unavailable")); }
        let logical = if fullscreen { self.logical } else { (w, h) };
        let scale = if fullscreen { viewport::fullscreen_mouse_mapping(w, h, logical.0, logical.1).map(|(x,y,_,_)| (x,y)).ok_or_else(|| RuntimeError::new("Invalid fullscreen mouse mapping"))? }
            else { viewport::mouse_scale(w, h, logical.0, logical.1) };
        unsafe {
            (*vp).base.size_x = logical.0; (*vp).base.size_y = logical.1;
            if fullscreen { (*vp).blit_flags |= viewport::BLIT_FULLSCREEN; } else { (*vp).blit_flags &= !viewport::BLIT_FULLSCREEN; }
            ((*vp).mouse_scale_x, (*vp).mouse_scale_y) = scale;
        }
        let color = unsafe { (*vp).base.color_bytes };
        if !unsafe { crate::render_device_owner::set_res(self.access, device.cast(), &raw mut (*vp).base, logical.0, logical.1, color, fullscreen)? } { return Err(RuntimeError::new("Renderer rejected display mode")); }
        if let Some(staged) = staged { unsafe { crate::render_device_owner::commit_fullscreen_targets(self.access, device.cast(), &raw mut (*vp).base, staged)?; } }
        Ok(())
    }
    // Core's staged update leaves the complete prior cache and file unchanged
    // on error. Do not synthesize a rollback value for an absent legacy key.
    fn persist_is_atomic(&self) -> bool { true }
    fn persist(&mut self, mode: DisplayMode) -> RuntimeResult<()> {
        let section = units("SDLDrv.SDLClient");
        let fullscreen = if mode == DisplayMode::Windowed { "False" } else { "True" };
        self.access.config_update_system_staged(&[(section.clone(), units("HP2DisplayMode"), units(name(mode))), (section, units("StartupFullscreen"), units(fullscreen))])?;
        unsafe { (*self.client).startup_fullscreen = u32::from(mode != DisplayMode::Windowed); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn startup_false_is_windowed_even_without_a_new_preference() {
        assert_eq!(startup_mode(None, false, false), DisplayMode::Windowed);
        assert_eq!(startup_mode(None, true, false), DisplayMode::BorderlessFullscreen);
        assert_eq!(startup_mode(Some(DisplayMode::Windowed), true, false), DisplayMode::Windowed);
        assert_eq!(startup_mode(Some(DisplayMode::ExclusiveFullscreen), false, false), DisplayMode::ExclusiveFullscreen);
        assert_eq!(startup_mode(Some(DisplayMode::ExclusiveFullscreen), true, true), DisplayMode::Windowed);
    }
    #[test] fn names_are_distinct_and_round_trip() {
        for mode in [DisplayMode::Windowed, DisplayMode::ExclusiveFullscreen, DisplayMode::BorderlessFullscreen] { assert_eq!(parse(name(mode)), Some(mode)); }
        assert_eq!(parse("fullscreen"), None);
    }
    #[test] fn persisted_mode_survives_reopen_and_conflict_preserves_file() {
        use hp2_core::core_access::{CoreOptions, CoreOwner};
        use std::{fs, path::PathBuf, ptr};
        fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR")).join(format!("display-config-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap(); let ini = root.join("Game.ini");
        fs::write(&ini, b"[SDLDrv.SDLClient]\nStartupFullscreen=False\nOther=unchanged\n").unwrap();
        let open = || CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: ini.clone(), user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
        let first = open(); let mut client: USDLClient = unsafe { std::mem::zeroed() };
        let mut backend = ViewportBackend { access: first.access(), viewport: ptr::null_mut(), client: &mut client, original: DisplayMode::Windowed, logical: (640, 480), window_size: (640, 480), window_position: (0, 0) };
        assert_eq!(backend.saved().unwrap(), DisplayMode::Windowed);
        backend.persist(DisplayMode::ExclusiveFullscreen).unwrap();
        assert_eq!(client.startup_fullscreen, 1);
        drop(backend); first.shutdown().unwrap();
        let second = open();
        assert_eq!(saved_mode(second.access(), &mut client).unwrap(), DisplayMode::ExclusiveFullscreen);
        let external = b"[SDLDrv.SDLClient]\nHP2DisplayMode=Windowed\nStartupFullscreen=False\nOther=external\n";
        fs::write(&ini, external).unwrap();
        let mut backend = ViewportBackend { access: second.access(), viewport: ptr::null_mut(), client: &mut client, original: DisplayMode::ExclusiveFullscreen, logical: (640, 480), window_size: (640, 480), window_position: (0, 0) };
        assert!(backend.persist(DisplayMode::BorderlessFullscreen).is_err());
        assert_eq!(fs::read(&ini).unwrap(), external);
        assert_eq!(client.startup_fullscreen, 1);
        drop(backend); second.shutdown().unwrap();
    }
}
