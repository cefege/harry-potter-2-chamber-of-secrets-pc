//! Per-page display selector transaction controller.
//!
//! The page adapter must create the stock control and confirmation dialog,
//! route their actions here, and call `tick` every frame. No retail package is
//! modified. This controller alone does not install or expose a control.
use hp2_abi::engine::generated::USDLViewport;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_platform::live_display_mode::{DisplayMode, Transaction};
use crate::stock_display_transition::{ViewportBackend, ViewportSnapshot};

pub(crate) const CHOICES: [(DisplayMode, &str); 3] = [
    (DisplayMode::Windowed, "Windowed"),
    (DisplayMode::ExclusiveFullscreen, "Fullscreen (exclusive)"),
    (DisplayMode::BorderlessFullscreen, "Borderless Fullscreen"),
];

#[derive(Default)]
pub(crate) struct Session { transaction: Transaction, snapshot: Option<ViewportSnapshot> }
impl Session {
    pub(crate) fn pending(&self) -> bool { self.transaction.is_pending() }
    fn abandon_if_lost(&mut self, owner_alive: bool) -> RuntimeResult<()> {
        if owner_alive { return Ok(()); }
        self.transaction.abandon_lost_owner();
        self.snapshot = None;
        Err(RuntimeError::new("Display transaction owner disappeared; restoration could not be verified"))
    }
    /// Begin only after the UI can show a confirmation prompt. All calls run
    /// on the video thread; callers must not invoke them while holding a renderer lock.
    pub(crate) unsafe fn begin(&mut self, access: CoreAccess<'_>, viewport: *mut USDLViewport, mode: DisplayMode, now_ms: u64) -> RuntimeResult<()> {
        if self.pending() { return Err(RuntimeError::new("Display confirmation already pending")); }
        if crate::camera_owner::sdl_display_exec::preview_pending(viewport.cast()) {
            return Err(RuntimeError::new("Finish the resolution confirmation before changing display mode"));
        }
        let mut backend = unsafe { ViewportBackend::new(access, viewport)? };
        let snapshot = backend.snapshot();
        let result = self.transaction.begin(&mut backend, mode, now_ms, 20_000);
        self.snapshot = self.pending().then_some(snapshot);
        result.map_err(|error| RuntimeError::new(&format!("Display mode transition failed: {error}")))
    }
    pub(crate) unsafe fn confirm(&mut self, access: CoreAccess<'_>, now_ms: u64) -> RuntimeResult<()> {
        let snapshot = self.snapshot.ok_or_else(|| RuntimeError::new("No display confirmation pending"))?;
        self.abandon_if_lost(unsafe { snapshot.owner_alive(access) })?;
        let mut backend = unsafe { ViewportBackend::from_snapshot(access, snapshot)? };
        let result = self.transaction.confirm(&mut backend, now_ms);
        if !self.pending() { self.snapshot = None; }
        result.map_err(|error| RuntimeError::new(&format!("Display confirmation failed: {error}")))
    }
    pub(crate) unsafe fn cancel(&mut self, access: CoreAccess<'_>) -> RuntimeResult<()> {
        let snapshot = self.snapshot.ok_or_else(|| RuntimeError::new("No display confirmation pending"))?;
        self.abandon_if_lost(unsafe { snapshot.owner_alive(access) })?;
        let mut backend = unsafe { ViewportBackend::from_snapshot(access, snapshot)? };
        let result = self.transaction.rollback(&mut backend);
        if !self.pending() { self.snapshot = None; }
        result.map_err(|error| RuntimeError::new(&format!("Display rollback failed: {error}")))
    }
    pub(crate) unsafe fn tick(&mut self, access: CoreAccess<'_>, now_ms: u64) -> RuntimeResult<bool> {
        let Some(snapshot) = self.snapshot else { return Ok(false) };
        self.abandon_if_lost(unsafe { snapshot.owner_alive(access) })?;
        let mut backend = unsafe { ViewportBackend::from_snapshot(access, snapshot)? };
        let result = self.transaction.tick(&mut backend, now_ms);
        if !self.pending() { self.snapshot = None; }
        result.map_err(|error| RuntimeError::new(&format!("Display timeout rollback failed: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp2_platform::live_display_mode::Backend;
    use std::ptr;

    struct Mock(DisplayMode);
    impl Backend for Mock {
        type Error = &'static str;
        fn current(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.0) }
        fn saved(&mut self) -> Result<DisplayMode, Self::Error> { Ok(DisplayMode::Windowed) }
        fn apply(&mut self, mode: DisplayMode) -> Result<(), Self::Error> { self.0 = mode; Ok(()) }
        fn persist(&mut self, _: DisplayMode) -> Result<(), Self::Error> { Ok(()) }
    }

    #[test]
    fn destroyed_viewport_terminates_pending_session_without_claiming_rollback() {
        let owner = crate::render_owner::render_test_support::owner("display-owner-lost");
        let access = owner.access();
        let client = crate::render_owner::render_test_support::construct(access, "SDLDrv.SDLClient").cast();
        let class = access.native_class("SDLDrv.SDLViewport").unwrap();
        let viewport = unsafe { access.construct_object(class, client, 0, 0x4000, ptr::null_mut()).unwrap().cast::<USDLViewport>() };
        let mut backend = Mock(DisplayMode::Windowed);
        let mut session = Session::default();
        session.transaction.begin(&mut backend, DisplayMode::BorderlessFullscreen, 0, 20_000).unwrap();
        session.snapshot = Some(unsafe { ViewportBackend::snapshot_for_test(viewport, client.cast()) });
        unsafe { access.conditional_destroy(viewport.cast()).unwrap(); }
        let error = unsafe { session.tick(access, 1) }.unwrap_err();
        assert!(error.to_string().contains("restoration could not be verified"));
        assert!(!session.pending());
        assert!(session.snapshot.is_none());
        assert!(!unsafe { session.tick(access, 2) }.unwrap());
        // No rollback was reported or attempted against the destroyed owner.
        assert_eq!(backend.0, DisplayMode::BorderlessFullscreen);
        session.transaction.begin(&mut backend, DisplayMode::Windowed, 3, 20_000).unwrap();
        session.transaction.rollback(&mut backend).unwrap();
    }
}
