//! Experimental, disconnected capture arbitration for an overlay.
//! All game, focus, overlay and teardown capture requests must use the same
//! arbiter. SDL cursor/relative mode are process-global: a single process owner
//! must also arbitrate viewports before this can be wired into the game.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseState { pub grabbed: bool, pub relative: bool, pub cursor_visible: bool }

pub trait MouseHost {
    type Error;
    fn snapshot(&mut self) -> Result<MouseState, Self::Error>;
    fn set_grab(&mut self, enabled: bool) -> Result<(), Self::Error>;
    fn set_relative(&mut self, enabled: bool) -> Result<(), Self::Error>;
    fn set_cursor_visible(&mut self, visible: bool) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> { Host(E), Rollback { operation: E, rollback: E }, Poisoned }

/// A failed transition rolls back the SDL snapshot. If rollback fails, the
/// arbiter becomes poisoned; callers must stop using it and release ownership.
pub struct CaptureArbiter { game: bool, focused: bool, overlay: bool, prior_cursor: bool, poisoned: bool }
impl CaptureArbiter {
    pub fn new<H: MouseHost>(host: &mut H, game_capture: bool, focused: bool) -> Result<Self, Error<H::Error>> {
        let state = host.snapshot().map_err(Error::Host)?;
        Ok(Self { game: game_capture, focused, overlay: false, prior_cursor: state.cursor_visible, poisoned: false })
    }
    pub fn game_request<H: MouseHost>(&mut self, host: &mut H, capture: bool) -> Result<(), Error<H::Error>> {
        self.update(host, capture, self.focused, self.overlay)
    }
    /// Focus loss closes the overlay. A later focus gain honors the most recent
    /// successful game capture request instead of a stale opening snapshot.
    pub fn focus<H: MouseHost>(&mut self, host: &mut H, focused: bool) -> Result<(), Error<H::Error>> {
        self.update(host, self.game, focused, self.overlay && focused)
    }
    pub fn overlay<H: MouseHost>(&mut self, host: &mut H, open: bool) -> Result<(), Error<H::Error>> {
        self.update(host, self.game, self.focused, open && self.focused)
    }
    fn update<H: MouseHost>(&mut self, host: &mut H, game: bool, focused: bool, overlay: bool) -> Result<(), Error<H::Error>> {
        if self.poisoned { return Err(Error::Poisoned); }
        let before = host.snapshot().map_err(Error::Host)?;
        let prior = if overlay && !self.overlay { before.cursor_visible } else { self.prior_cursor };
        let capture = focused && game && !overlay;
        let target = MouseState { grabbed: capture, relative: capture,
            cursor_visible: if overlay { true } else if capture { false } else { prior } };
        if let Err(operation) = apply(host, target) {
            return match apply(host, before) {
                Ok(()) => Err(Error::Host(operation)),
                Err(rollback) => { self.poisoned = true; Err(Error::Rollback { operation, rollback }) }
            };
        }
        self.game = game; self.focused = focused; self.overlay = overlay; self.prior_cursor = prior;
        Ok(())
    }
}
fn apply<H: MouseHost>(host: &mut H, state: MouseState) -> Result<(), H::Error> {
    host.set_relative(state.relative)?;
    host.set_grab(state.grabbed)?;
    host.set_cursor_visible(state.cursor_visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Host { state: MouseState, fail_relative: bool, fail_cursor: bool, fail_grab_always: bool }
    impl MouseHost for Host {
        type Error = &'static str;
        fn snapshot(&mut self) -> Result<MouseState, Self::Error> { Ok(self.state) }
        fn set_grab(&mut self, v: bool) -> Result<(), Self::Error> { if self.fail_grab_always { return Err("grab"); } self.state.grabbed = v; Ok(()) }
        fn set_relative(&mut self, v: bool) -> Result<(), Self::Error> { if self.fail_relative { self.fail_relative = false; return Err("relative"); } self.state.relative = v; Ok(()) }
        fn set_cursor_visible(&mut self, v: bool) -> Result<(), Self::Error> { if self.fail_cursor { self.fail_cursor = false; return Err("cursor"); } self.state.cursor_visible = v; Ok(()) }
    }
    fn host(cursor: bool) -> Host { Host { state: MouseState { grabbed: true, relative: true, cursor_visible: cursor }, fail_relative: false, fail_cursor: false, fail_grab_always: false } }
    #[test] fn competing_game_focus_overlay_requests_and_cursor_restoration() {
        let mut h = host(false); let mut a = CaptureArbiter::new(&mut h, true, true).unwrap();
        a.overlay(&mut h, true).unwrap(); assert_eq!(h.state, MouseState { grabbed: false, relative: false, cursor_visible: true });
        a.game_request(&mut h, false).unwrap(); a.overlay(&mut h, false).unwrap(); assert!(!h.state.grabbed); assert!(!h.state.cursor_visible);
        a.game_request(&mut h, true).unwrap(); a.overlay(&mut h, true).unwrap(); a.focus(&mut h, false).unwrap();
        assert!(!h.state.grabbed); assert!(!h.state.cursor_visible);
        a.game_request(&mut h, false).unwrap(); a.focus(&mut h, true).unwrap(); assert!(!h.state.grabbed);
    }
    #[test] fn capture_requested_during_overlay_applies_only_on_close() {
        let mut h = Host { state: MouseState { grabbed: false, relative: false, cursor_visible: true }, fail_relative: false, fail_cursor: false, fail_grab_always: false };
        let mut a = CaptureArbiter::new(&mut h, false, true).unwrap();
        a.overlay(&mut h, true).unwrap(); a.game_request(&mut h, true).unwrap();
        assert!(!h.state.grabbed); assert!(h.state.cursor_visible);
        a.overlay(&mut h, false).unwrap(); assert!(h.state.grabbed); assert!(h.state.relative); assert!(!h.state.cursor_visible);
    }
    #[test] fn initially_visible_cursor_restores_when_capture_not_requested() {
        let mut h = Host { state: MouseState { grabbed: false, relative: false, cursor_visible: true }, fail_relative: false, fail_cursor: false, fail_grab_always: false };
        let mut a = CaptureArbiter::new(&mut h, false, true).unwrap();
        a.overlay(&mut h, true).unwrap(); a.overlay(&mut h, false).unwrap(); assert!(h.state.cursor_visible);
    }
    #[test] fn rollback_failure_poisoned_arbiter_refuses_further_changes() {
        let mut h = host(false); let mut a = CaptureArbiter::new(&mut h, true, true).unwrap();
        h.fail_grab_always = true;
        assert_eq!(a.overlay(&mut h, true), Err(Error::Rollback { operation: "grab", rollback: "grab" }));
        assert_eq!(a.game_request(&mut h, false), Err(Error::Poisoned));
    }
    #[test] fn sdl_failures_restore_snapshot_and_do_not_commit_request() {
        let mut h = host(false); let before = h.state; let mut a = CaptureArbiter::new(&mut h, true, true).unwrap();
        h.fail_cursor = true; assert_eq!(a.overlay(&mut h, true), Err(Error::Host("cursor"))); assert_eq!(h.state, before); assert!(!a.overlay);
        h.fail_relative = true; assert_eq!(a.focus(&mut h, false), Err(Error::Host("relative"))); assert_eq!(h.state, before); assert!(a.focused);
    }
}
