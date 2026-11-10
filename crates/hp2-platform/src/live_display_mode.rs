//! Transaction policy and checked SDL operations for a live display-mode selector.
//!
//! The owner must integrate `Backend::apply` with viewport flags, renderer
//! targets and mouse mapping. SDL alone cannot establish a usable game mode.
//! Run this policy on the video thread and retain it until rollback succeeds.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayMode { Windowed, ExclusiveFullscreen, BorderlessFullscreen }

impl DisplayMode {
    pub fn from_sdl_flags(flags: u32) -> Self {
        if flags & 0x1001 == 0x1001 { Self::BorderlessFullscreen }
        else if flags & 1 != 0 { Self::ExclusiveFullscreen }
        else { Self::Windowed }
    }
    pub fn sdl_flags(self) -> u32 {
        match self { Self::Windowed => 0, Self::ExclusiveFullscreen => 1, Self::BorderlessFullscreen => 0x1001 }
    }
}

/// `apply` must set the complete runtime state, including the renderer and
/// input geometry. `persist` must write the complete startup preference.
pub trait Backend {
    type Error;
    fn current(&mut self) -> Result<DisplayMode, Self::Error>;
    fn saved(&mut self) -> Result<DisplayMode, Self::Error>;
    fn apply(&mut self, mode: DisplayMode) -> Result<(), Self::Error>;
    fn persist(&mut self, mode: DisplayMode) -> Result<(), Self::Error>;
    /// Return true only when a failed write leaves the complete saved state unchanged.
    fn persist_is_atomic(&self) -> bool { false }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> { Busy, NoPending, Expired, Changed, Backend(E), Rollback { cause: E, rollback: E } }
impl<E: std::fmt::Display> std::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str("display confirmation already pending"),
            Self::NoPending => f.write_str("no display confirmation pending"),
            Self::Expired => f.write_str("display confirmation expired"),
            Self::Changed => f.write_str("display mode changed before confirmation"),
            Self::Backend(error) => write!(f, "{error}"),
            Self::Rollback { cause, rollback } => write!(f, "{cause}; rollback failed: {rollback}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pending { previous: DisplayMode, saved: DisplayMode, requested: DisplayMode, deadline: u64, restore_config: bool }

/// A deadline uses caller-supplied monotonic milliseconds, not wall time.
#[derive(Default)]
pub struct Transaction { pending: Option<Pending> }
impl Transaction {
    pub fn is_pending(&self) -> bool { self.pending.is_some() }
    /// The owning viewport and its window no longer exist or cannot be
    /// identified. Do not call the backend through a stale pointer or claim
    /// that runtime or persistent restoration succeeded.
    pub fn abandon_lost_owner(&mut self) { self.pending = None; }
    pub fn begin<B: Backend>(&mut self, backend: &mut B, requested: DisplayMode, now_ms: u64, timeout_ms: u64) -> Result<(), Error<B::Error>> {
        if self.is_pending() { return Err(Error::Busy); }
        let previous = backend.current().map_err(Error::Backend)?;
        let saved = backend.saved().map_err(Error::Backend)?;
        self.pending = Some(Pending { previous, saved, requested, deadline: now_ms.saturating_add(timeout_ms), restore_config: false });
        if let Err(cause) = backend.apply(requested) {
            return match self.rollback(backend) {
                Ok(()) => Err(Error::Backend(cause)),
                Err(Error::Backend(rollback)) => Err(Error::Rollback { cause, rollback }),
                _ => unreachable!(),
            };
        }
        Ok(())
    }
    pub fn confirm<B: Backend>(&mut self, backend: &mut B, now_ms: u64) -> Result<(), Error<B::Error>> {
        let pending = self.pending.ok_or(Error::NoPending)?;
        if now_ms >= pending.deadline { self.rollback(backend)?; return Err(Error::Expired); }
        let current = backend.current().map_err(Error::Backend)?;
        if current != pending.requested { self.rollback(backend)?; return Err(Error::Changed); }
        // A failed persistence operation may have partially written config.
        self.pending.as_mut().unwrap().restore_config = !backend.persist_is_atomic();
        if let Err(cause) = backend.persist(current) {
            return match self.rollback(backend) {
                Ok(()) => Err(Error::Backend(cause)),
                Err(Error::Backend(rollback)) => Err(Error::Rollback { cause, rollback }),
                _ => unreachable!(),
            };
        }
        self.pending = None;
        Ok(())
    }
    pub fn tick<B: Backend>(&mut self, backend: &mut B, now_ms: u64) -> Result<bool, Error<B::Error>> {
        if self.pending.is_some_and(|p| now_ms >= p.deadline) {
            self.rollback(backend)?;
            Ok(true)
        } else { Ok(false) }
    }
    pub fn rollback<B: Backend>(&mut self, backend: &mut B) -> Result<(), Error<B::Error>> {
        let pending = self.pending.ok_or(Error::NoPending)?;
        // Try both independently, even if either fails. Keep the transaction
        // available for another restoration attempt if any operation fails.
        let runtime = backend.apply(pending.previous);
        let config = if pending.restore_config { backend.persist(pending.saved) } else { Ok(()) };
        match (runtime, config) {
            (Ok(()), Ok(())) => { self.pending = None; Ok(()) }
            (Err(e), _) | (_, Err(e)) => Err(Error::Backend(e)),
        }
    }
}

/// SDL-only checked mode switch. The caller must update and verify the game
/// renderer, viewport, mouse mapping, and saved configuration separately.
/// Exclusive is never silently replaced by desktop fullscreen.
///
/// # Safety
/// `window` must refer to a live SDL window, and this must run on SDL's video thread.
#[cfg(feature = "sdl")]
pub unsafe fn set_sdl_mode(window: *mut crate::window_lifecycle::SdlWindow, mode: DisplayMode) -> Result<(), String> {
    unsafe extern "C" {
        fn SDL_SetWindowFullscreen(window: *mut crate::window_lifecycle::SdlWindow, flags: u32) -> i32;
        fn SDL_GetWindowFlags(window: *mut crate::window_lifecycle::SdlWindow) -> u32;
        fn SDL_GetError() -> *const std::ffi::c_char;
    }
    if window.is_null() { return Err("SDL window unavailable".into()); }
    // Copy SDL's thread-local error before any rollback call can replace it.
    if unsafe { SDL_SetWindowFullscreen(window, mode.sdl_flags()) } != 0 {
        let error = unsafe { SDL_GetError() };
        let detail = if error.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(error) }.to_string_lossy().into_owned() };
        return Err(if detail.is_empty() { "SDL rejected display mode without an error detail".into() } else { format!("SDL rejected display mode: {detail}") });
    }
    if DisplayMode::from_sdl_flags(unsafe { SDL_GetWindowFlags(window) }) != mode {
        return Err("SDL did not apply requested display mode".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Mock { runtime: DisplayMode, saved: DisplayMode, fail_apply: bool, fail_save: bool }
    impl Backend for Mock {
        type Error = &'static str;
        fn current(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.runtime) }
        fn saved(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.saved) }
        fn apply(&mut self, m: DisplayMode) -> Result<(), Self::Error> { self.runtime = m; if self.fail_apply { Err("apply") } else { Ok(()) } }
        fn persist(&mut self, m: DisplayMode) -> Result<(), Self::Error> { self.saved = m; if self.fail_save { Err("save") } else { Ok(()) } }
    }
    fn mock() -> Mock { Mock { runtime: DisplayMode::Windowed, saved: DisplayMode::Windowed, fail_apply: false, fail_save: false } }
    #[cfg(feature = "sdl")]
    #[test] fn real_sdl_dummy_checks_mode_results() {
        if std::env::var_os("HP2_MODE_SDL_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "live_display_mode::tests::real_sdl_dummy_checks_mode_results", "--nocapture"])
                .env("HP2_MODE_SDL_CHILD", "1").env("SDL_VIDEODRIVER", "dummy").status().unwrap();
            assert!(status.success()); return;
        }
        unsafe extern "C" { fn SDL_Init(flags: u32) -> i32; fn SDL_Quit(); }
        unsafe {
            assert_eq!(SDL_Init(0x20), 0);
            let window = crate::window_lifecycle::create_window(640, 480, 0x8);
            assert!(!window.is_null());
            for mode in [DisplayMode::BorderlessFullscreen, DisplayMode::Windowed, DisplayMode::ExclusiveFullscreen, DisplayMode::Windowed] {
                let result = set_sdl_mode(window, mode);
                if result.is_ok() { assert_eq!(DisplayMode::from_sdl_flags(crate::window_lifecycle::get_window_flags(window)), mode); }
                else { eprintln!("SDL dummy rejected {mode:?}: {result:?}"); }
            }
            assert!(set_sdl_mode(window, DisplayMode::Windowed).is_ok());
            crate::window_lifecycle::destroy_window(window); SDL_Quit();
        }
    }
    #[test] fn rollback_error_preserves_both_readable_causes() {
        let error = Error::Rollback { cause: "SDL refused the mode", rollback: "renderer failed" };
        assert_eq!(error.to_string(), "SDL refused the mode; rollback failed: renderer failed");
    }
    #[test] fn flags_distinguish_all_three() {
        for mode in [DisplayMode::Windowed, DisplayMode::ExclusiveFullscreen, DisplayMode::BorderlessFullscreen] { assert_eq!(DisplayMode::from_sdl_flags(mode.sdl_flags() | 0x20), mode); }
    }
    #[test] fn timeout_restores_without_saving_and_confirm_saves() {
        let mut b = mock(); let mut t = Transaction::default();
        t.begin(&mut b, DisplayMode::BorderlessFullscreen, 100, 50).unwrap();
        assert_eq!(b.saved, DisplayMode::Windowed);
        assert_eq!(t.begin(&mut b, DisplayMode::ExclusiveFullscreen, 101, 50), Err(Error::Busy));
        assert!(!t.tick(&mut b, 149).unwrap()); assert!(t.tick(&mut b, 150).unwrap());
        assert_eq!(b.runtime, DisplayMode::Windowed);
        t.begin(&mut b, DisplayMode::ExclusiveFullscreen, 200, 50).unwrap(); t.confirm(&mut b, 201).unwrap();
        assert_eq!(b.saved, DisplayMode::ExclusiveFullscreen); assert!(!t.is_pending());
    }
    #[test] fn expired_confirmation_and_changed_mode_restore() {
        let mut b = mock(); let mut t = Transaction::default();
        t.begin(&mut b, DisplayMode::BorderlessFullscreen, 0, 10).unwrap();
        assert_eq!(t.confirm(&mut b, 10), Err(Error::Expired));
        assert_eq!(b.runtime, DisplayMode::Windowed);
        t.begin(&mut b, DisplayMode::BorderlessFullscreen, 20, 10).unwrap();
        b.runtime = DisplayMode::ExclusiveFullscreen;
        assert_eq!(t.confirm(&mut b, 21), Err(Error::Changed));
        assert_eq!(b.runtime, DisplayMode::Windowed);
    }
    #[test] fn config_rollback_uses_saved_not_live_mode() {
        let mut b = mock(); let mut t = Transaction::default();
        b.saved = DisplayMode::ExclusiveFullscreen;
        t.begin(&mut b, DisplayMode::BorderlessFullscreen, 0, 10).unwrap();
        b.fail_save = true;
        assert!(t.confirm(&mut b, 1).is_err());
        assert_eq!(b.runtime, DisplayMode::Windowed);
        assert_eq!(b.saved, DisplayMode::ExclusiveFullscreen);
    }
    #[test] fn atomic_save_failure_does_not_rewrite_a_legacy_preference() {
        struct Atomic { runtime: DisplayMode, calls: usize }
        impl Backend for Atomic {
            type Error = &'static str;
            fn current(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.runtime) }
            fn saved(&mut self) -> Result<DisplayMode, Self::Error> { Ok(DisplayMode::Windowed) }
            fn apply(&mut self, mode: DisplayMode) -> Result<(), Self::Error> { self.runtime = mode; Ok(()) }
            fn persist(&mut self, _: DisplayMode) -> Result<(), Self::Error> { self.calls += 1; Err("conflict") }
            fn persist_is_atomic(&self) -> bool { true }
        }
        let mut b = Atomic { runtime: DisplayMode::Windowed, calls: 0 };
        let mut t = Transaction::default();
        t.begin(&mut b, DisplayMode::BorderlessFullscreen, 0, 10).unwrap();
        assert_eq!(t.confirm(&mut b, 1), Err(Error::Backend("conflict")));
        assert_eq!(b.runtime, DisplayMode::Windowed);
        assert_eq!(b.calls, 1);
        assert!(!t.is_pending());
    }
    #[test] fn rejected_exclusive_after_partial_change_restores_borderless() {
        struct RejectExclusive { runtime: DisplayMode, saved: DisplayMode, calls: Vec<DisplayMode> }
        impl Backend for RejectExclusive {
            type Error = &'static str;
            fn current(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.runtime) }
            fn saved(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.saved) }
            fn apply(&mut self, mode: DisplayMode) -> Result<(), Self::Error> {
                self.calls.push(mode);
                self.runtime = mode; // Model an SDL or renderer failure after a partial switch.
                if mode == DisplayMode::ExclusiveFullscreen { Err("exclusive unavailable") } else { Ok(()) }
            }
            fn persist(&mut self, mode: DisplayMode) -> Result<(), Self::Error> { self.saved = mode; Ok(()) }
        }
        let mut backend = RejectExclusive { runtime: DisplayMode::BorderlessFullscreen, saved: DisplayMode::BorderlessFullscreen, calls: Vec::new() };
        let mut transaction = Transaction::default();
        assert_eq!(transaction.begin(&mut backend, DisplayMode::ExclusiveFullscreen, 0, 20_000), Err(Error::Backend("exclusive unavailable")));
        assert_eq!(backend.calls, [DisplayMode::ExclusiveFullscreen, DisplayMode::BorderlessFullscreen]);
        assert_eq!(backend.runtime, DisplayMode::BorderlessFullscreen);
        assert_eq!(backend.saved, DisplayMode::BorderlessFullscreen);
        assert!(!transaction.is_pending());
    }
    #[test] fn failed_non_atomic_save_rewrites_saved_mode_and_restores_runtime() {
        struct PartialSave { runtime: DisplayMode, saved: DisplayMode, calls: usize }
        impl Backend for PartialSave {
            type Error = &'static str;
            fn current(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.runtime) }
            fn saved(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.saved) }
            fn apply(&mut self, mode: DisplayMode) -> Result<(), Self::Error> { self.runtime = mode; Ok(()) }
            fn persist(&mut self, mode: DisplayMode) -> Result<(), Self::Error> {
                self.calls += 1; self.saved = mode;
                if self.calls == 1 { Err("partial write") } else { Ok(()) }
            }
        }
        let mut backend = PartialSave { runtime: DisplayMode::Windowed, saved: DisplayMode::ExclusiveFullscreen, calls: 0 };
        let mut transaction = Transaction::default();
        transaction.begin(&mut backend, DisplayMode::BorderlessFullscreen, 0, 20_000).unwrap();
        assert_eq!(transaction.confirm(&mut backend, 1), Err(Error::Backend("partial write")));
        assert_eq!(backend.runtime, DisplayMode::Windowed);
        assert_eq!(backend.saved, DisplayMode::ExclusiveFullscreen);
        assert_eq!(backend.calls, 2);
        assert!(!transaction.is_pending());
    }
    #[test] fn timeout_retries_a_failed_runtime_rollback() {
        let mut backend = mock(); let mut transaction = Transaction::default();
        transaction.begin(&mut backend, DisplayMode::ExclusiveFullscreen, 100, 10).unwrap();
        backend.fail_apply = true;
        assert_eq!(transaction.tick(&mut backend, 110), Err(Error::Backend("apply")));
        assert!(transaction.is_pending());
        assert_eq!(backend.saved, DisplayMode::Windowed);
        backend.fail_apply = false;
        assert_eq!(transaction.tick(&mut backend, 111), Ok(true));
        assert_eq!(backend.runtime, DisplayMode::Windowed);
        assert!(!transaction.is_pending());
    }
    #[test] fn atomic_save_failure_retains_failed_runtime_rollback_for_retry() {
        struct Atomic { runtime: DisplayMode, rollback_fails: bool, saves: usize }
        impl Backend for Atomic {
            type Error = &'static str;
            fn current(&mut self) -> Result<DisplayMode, Self::Error> { Ok(self.runtime) }
            fn saved(&mut self) -> Result<DisplayMode, Self::Error> { Ok(DisplayMode::Windowed) }
            fn apply(&mut self, mode: DisplayMode) -> Result<(), Self::Error> {
                if mode == DisplayMode::Windowed && self.rollback_fails { return Err("restore"); }
                self.runtime = mode; Ok(())
            }
            fn persist(&mut self, _: DisplayMode) -> Result<(), Self::Error> { self.saves += 1; Err("save") }
            fn persist_is_atomic(&self) -> bool { true }
        }
        let mut backend = Atomic { runtime: DisplayMode::Windowed, rollback_fails: false, saves: 0 };
        let mut transaction = Transaction::default();
        transaction.begin(&mut backend, DisplayMode::ExclusiveFullscreen, 0, 10).unwrap();
        backend.rollback_fails = true;
        assert_eq!(transaction.confirm(&mut backend, 1), Err(Error::Rollback { cause: "save", rollback: "restore" }));
        assert!(transaction.is_pending());
        assert_eq!(backend.runtime, DisplayMode::ExclusiveFullscreen);
        backend.rollback_fails = false;
        transaction.rollback(&mut backend).unwrap();
        assert_eq!(backend.runtime, DisplayMode::Windowed);
        assert_eq!(backend.saves, 1);
        assert!(!transaction.is_pending());
    }
    #[test] fn failed_apply_keeps_failed_rollback_retryable() {
        let mut b = mock(); let mut t = Transaction::default(); b.fail_apply = true;
        assert_eq!(t.begin(&mut b, DisplayMode::BorderlessFullscreen, 0, 1), Err(Error::Rollback { cause: "apply", rollback: "apply" }));
        assert!(t.is_pending()); b.fail_apply = false; t.rollback(&mut b).unwrap(); assert_eq!(b.runtime, DisplayMode::Windowed);
    }
    #[test] fn failed_save_restores_both_and_retains_retry_on_failure() {
        let mut b = mock(); let mut t = Transaction::default();
        t.begin(&mut b, DisplayMode::BorderlessFullscreen, 0, 1).unwrap(); b.fail_save = true;
        assert_eq!(t.confirm(&mut b, 0), Err(Error::Rollback { cause: "save", rollback: "save" }));
        assert_eq!(b.runtime, DisplayMode::Windowed); assert_eq!(b.saved, DisplayMode::Windowed); assert!(t.is_pending());
        b.fail_save = false; t.rollback(&mut b).unwrap(); assert!(!t.is_pending());
    }
}
