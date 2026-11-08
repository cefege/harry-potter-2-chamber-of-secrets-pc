//! Replaces the formatting and error/exit policy of `FFeedbackContextSDL`
//! and `FOutputDeviceSDLError`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/FFeedbackContextSDL.h`,
//! `FOutputDeviceSDLError.h`). Both classes stay thin C++ virtual-dispatch
//! shells: `FFeedbackContext`/`FOutputDeviceError` are base classes the
//! engine calls through a pointer throughout the codebase (rule 2), and
//! `GLog`/`AuxOut`/`Context` forwards, every TCHAR buffer and UTF-8
//! conversion, SDL message boxes and `appStrncpy`/`StaticShutdownAfterError`
//! stay native leaves.
//!
//! `FFeedbackContextSDL::Serialize`'s overwhelming majority of calls carry
//! an `EName` (`NAME_Log` and friends) that always takes the same,
//! unconditional passthrough with no decision at all (91-118's final
//! common tail, reached by every event not named below); that passthrough
//! stays fully native and is never round-tripped through Rust. Only the
//! six events with a real decision -- `NAME_Heading`, `NAME_SubHeading`,
//! `NAME_Error`, `NAME_Warning`, `NAME_ExecWarning`, `NAME_ScriptWarning`
//! -- call into this module.

use std::ffi::c_void;

fn checked(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

/*-----------------------------------------------------------------------------
    FFeedbackContextSDL.
-----------------------------------------------------------------------------*/

/// The six `EName`s `Serialize` (74-118) gives real handling; every other
/// event always takes the unconditional native passthrough.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hp2LauncherFeedbackEvent {
    Heading,
    SubHeading,
    Error,
    Warning,
    ExecWarning,
    ScriptWarning,
}

/// Native text-only lookups `Serialize`'s warning-family branch (91-98)
/// uses, plus the formatted-text publish every branch uses.
///
/// Native callbacks contain C++ exceptions below Rust; every callback
/// returns zero on success or its exact nonzero captured failure.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherFeedbackServices {
    pub context: *mut c_void,
    /// `*Context->GetContext()`, converted to UTF-8 (95); called only when
    /// `Context` is non-null and the event is one of the warning family.
    pub get_context_text: unsafe extern "C" fn(*mut c_void, *mut u8, usize, *mut i32) -> i32,
    /// `*FName(Event)`, converted to UTF-8 (95); called only alongside
    /// `get_context_text`.
    pub get_event_name:
        unsafe extern "C" fn(*mut c_void, Hp2LauncherFeedbackEvent, *mut u8, usize, *mut i32) -> i32,
    /// Converts the already fully formatted UTF-8 message and writes it
    /// into `Temp` via `appFromUtf8InPlace` (83-84, 88-89, 95-96), which
    /// the caller then uses in place of `V`.
    pub write_formatted: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32,
}

impl Hp2LauncherFeedbackServices {
    fn context_text(&self) -> Result<String, i32> {
        let mut buf = [0u8; 2048];
        let mut len: i32 = 0;
        checked(unsafe { (self.get_context_text)(self.context, buf.as_mut_ptr(), buf.len(), &mut len) })?;
        Ok(String::from_utf8_lossy(&buf[..len.max(0) as usize]).into_owned())
    }

    fn event_name(&self, event: Hp2LauncherFeedbackEvent) -> Result<String, i32> {
        let mut buf = [0u8; 256];
        let mut len: i32 = 0;
        checked(unsafe { (self.get_event_name)(self.context, event, buf.as_mut_ptr(), buf.len(), &mut len) })?;
        Ok(String::from_utf8_lossy(&buf[..len.max(0) as usize]).into_owned())
    }

    fn write(&self, text: &str) -> Result<(), i32> {
        checked(unsafe { (self.write_formatted)(self.context, text.as_ptr(), text.len()) })
    }
}

/// `Serialize`'s Heading/SubHeading/warning-family decision (81-98,
/// 116-117): the formatted text, whether `WarningCount` increments, and
/// whether the error box should show.
pub struct FeedbackFormat {
    pub text: String,
    pub increment_warning_count: bool,
    pub show_error_box: bool,
}

/// Replaces `Serialize`'s Heading/SubHeading/warning-family branch body
/// (81-98) and the trailing `Event==NAME_Error && !FrontendSuppressed()`
/// check (116), pure given already-fetched native text.
pub fn format_message(
    event: Hp2LauncherFeedbackEvent,
    context: Option<&str>,
    event_name: &str,
    text: &str,
    frontend_suppressed: bool,
) -> FeedbackFormat {
    use Hp2LauncherFeedbackEvent::*;
    match event {
        Heading => FeedbackFormat {
            text: format!("--------------------{text}--------------------"),
            increment_warning_count: false,
            show_error_box: false,
        },
        SubHeading => FeedbackFormat {
            text: format!("{text}..."),
            increment_warning_count: false,
            show_error_box: false,
        },
        Error | Warning | ExecWarning | ScriptWarning => {
            let formatted = match context {
                Some(c) => format!("{c} : {event_name}, {text}"),
                None => text.to_string(),
            };
            FeedbackFormat {
                show_error_box: event == Error && !frontend_suppressed,
                text: formatted,
                increment_warning_count: true,
            }
        }
    }
}

/// Fetches whatever native text the event needs, formats it, and publishes
/// the result. Returns `(increment_warning_count, show_error_box)`.
/// # Safety
/// Services satisfy the checked callback contract; `has_context` is exactly
/// `Context != NULL` at the original call site.
pub unsafe fn run_format(
    services: &Hp2LauncherFeedbackServices,
    event: Hp2LauncherFeedbackEvent,
    has_context: bool,
    text: &str,
    frontend_suppressed: bool,
) -> Result<(bool, bool), i32> {
    use Hp2LauncherFeedbackEvent::*;
    let wants_context = matches!(event, Error | Warning | ExecWarning | ScriptWarning) && has_context;
    let context = if wants_context { Some(services.context_text()?) } else { None };
    let event_name = if wants_context { services.event_name(event)? } else { String::new() };
    let result = format_message(event, context.as_deref(), &event_name, text, frontend_suppressed);
    services.write(&result.text)?;
    Ok((result.increment_warning_count, result.show_error_box))
}

/// Replaces the auto-answer branch of `YesNof` (120-131): headless or
/// non-interactive runs (`-NOFRONTEND`/`-Silent`, or no client/editor
/// context) answer "Yes" without ever opening a dialog.
pub fn yes_no_auto_answer(frontend_suppressed: bool, is_client: bool, is_editor: bool) -> bool {
    frontend_suppressed || !(is_client || is_editor)
}

/*-----------------------------------------------------------------------------
    FOutputDeviceSDLError.
-----------------------------------------------------------------------------*/

/// One native global read/write, virtual call, or fixed `debugf` that
/// `Serialize`/`HandleError` perform. `Msg` is the original call's live
/// TCHAR pointer throughout; every leaf below reuses the single live
/// instance, there is nothing left to pass per-call beyond the optional
/// `arg`.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherErrorLeaf {
    /// `GIsCriticalError`; output is its current UBOOL (81).
    IsCriticalError,
    /// `GIsCriticalError = 1;` (84, 112).
    SetCriticalError,
    /// `ErrorType = Event; debugf(NAME_Critical, TEXT("appError
    /// called:")); debugf(NAME_Critical, Msg);
    /// UObject::StaticShutdownAfterError(); appStrncpy(GErrorHist, Msg,
    /// ARRAY_COUNT(GErrorHist)); appStrncat(GErrorHist, TEXT("\r\n\r\n"),
    /// ARRAY_COUNT(GErrorHist)); ErrorPos = appStrlen(GErrorHist);`
    /// (85-93), the unconditional first-error sequence.
    RunFirstErrorSequence,
    /// `debugf(NAME_Critical, TEXT("Error reentered: %s"), Msg);` (97).
    LogReentered,
    /// `GIsGuarded`; output is its current UBOOL (94, 100).
    IsGuarded,
    /// `HandleError();` (95), the virtual override's own body, called
    /// through `this` exactly as the original inline call.
    RunHandleError,
    /// `throw(1);` (101). Never returns on success; its failure status is
    /// the thrown exception, captured and rethrown by `Complete` exactly
    /// like every other native exception this port captures.
    ThrowError,
    /// `appRequestExit(1);` (103).
    RequestExit,
    /// `GIsGuarded = 0;` (110).
    SetNotGuarded,
    /// `GIsRunning = 0;` (111).
    ClearIsRunning,
    /// `GLogHook = NULL;` (113).
    ClearLogHook,
    /// `UObject::StaticShutdownAfterError();` (114).
    StaticShutdownAfterError,
    /// `ErrorType == NAME_FriendlyError`; output is the UBOOL (115).
    IsFriendlyError,
    /// `GErrorHist[<Position>] = 0;`, `<Position>` being `ErrorPos` when
    /// `arg != 0` or `ARRAY_COUNT(GErrorHist) - 1` otherwise (115).
    TruncateErrorHistory,
    /// `LocalPrint(GErrorHist); LocalPrint(TEXT("\n\nExiting due to
    /// error\n")); fflush(stdout);` (116-118).
    PrintErrorHistory,
    /// `ParseParam(appCmdLine(), TEXT("NOFRONTEND"));` output is the
    /// UBOOL (52).
    QueryNoFrontend,
    /// `isatty(STDIN_FILENO)` / `_isatty(_fileno(stdin))` (57-63); output
    /// is the UBOOL (1 when a platform always shows the box, matching the
    /// original unconditional `#else if(true)` fallback being inverted to
    /// "never a tty" there -- native keeps that platform chain verbatim).
    QueryStdinIsTty,
    /// `ShowErrorBox`'s message-box call (65-70).
    ShowMessageBox,
}

/// Native callbacks contain C++ exceptions below Rust; every callback
/// returns zero on success or its exact nonzero captured failure.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherErrorServices {
    pub context: *mut c_void,
    pub call: unsafe extern "C" fn(*mut c_void, Hp2LauncherErrorLeaf, i32, *mut i32) -> i32,
}

impl Hp2LauncherErrorServices {
    fn call(&self, leaf: Hp2LauncherErrorLeaf, arg: i32) -> Result<i32, i32> {
        let mut value = 0;
        checked(unsafe { (self.call)(self.context, leaf, arg, &mut value) })?;
        Ok(value)
    }
}

/// Replaces `FOutputDeviceSDLError::Serialize` (79-104).
/// # Safety
/// Services satisfy the checked callback contract.
pub unsafe fn serialize(services: &Hp2LauncherErrorServices) -> Result<(), i32> {
    use Hp2LauncherErrorLeaf::*;
    if services.call(IsCriticalError, 0)? == 0 {
        services.call(SetCriticalError, 0)?;
        services.call(RunFirstErrorSequence, 0)?;
        if services.call(IsGuarded, 0)? == 0 {
            services.call(RunHandleError, 0)?;
        }
    } else {
        services.call(LogReentered, 0)?;
    }
    if services.call(IsGuarded, 0)? != 0 {
        services.call(ThrowError, 0)?;
    } else {
        services.call(RequestExit, 0)?;
    }
    Ok(())
}

/// Replaces `ShowErrorBox`'s suppression gate (52-64): a fatal headless
/// (`-NOFRONTEND`) or non-interactive (stdin not a terminal) run never
/// opens the modal box.
pub fn should_show_error_box(no_frontend: bool, stdin_is_tty: bool) -> bool {
    !no_frontend && stdin_is_tty
}

/// Replaces `HandleError`'s guarded body (106-120); the swallow-everything
/// `catch(...) {}` (108, 121-122) stays native, wrapping the whole call
/// into this function so no captured exception is ever rethrown.
/// # Safety
/// Services satisfy the checked callback contract.
pub unsafe fn handle_error(services: &Hp2LauncherErrorServices) -> Result<(), i32> {
    use Hp2LauncherErrorLeaf::*;
    services.call(SetNotGuarded, 0)?;
    services.call(ClearIsRunning, 0)?;
    services.call(SetCriticalError, 0)?;
    services.call(ClearLogHook, 0)?;
    services.call(StaticShutdownAfterError, 0)?;
    let friendly = services.call(IsFriendlyError, 0)?;
    services.call(TruncateErrorHistory, friendly)?;
    services.call(PrintErrorHistory, 0)?;
    let no_frontend = services.call(QueryNoFrontend, 0)? != 0;
    let stdin_is_tty = services.call(QueryStdinIsTty, 0)? != 0;
    if should_show_error_box(no_frontend, stdin_is_tty) {
        services.call(ShowMessageBox, 0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use Hp2LauncherFeedbackEvent::*;

    #[test]
    fn heading_wraps_in_dashes_and_never_shows_error_box_or_counts() {
        let r = format_message(Heading, None, "", "Startup", false);
        assert_eq!(r.text, "--------------------Startup--------------------");
        assert!(!r.increment_warning_count);
        assert!(!r.show_error_box);
    }

    #[test]
    fn subheading_appends_ellipsis() {
        let r = format_message(SubHeading, None, "", "Loading", false);
        assert_eq!(r.text, "Loading...");
        assert!(!r.increment_warning_count);
        assert!(!r.show_error_box);
    }

    #[test]
    fn warning_family_without_context_passes_text_through_but_still_counts() {
        for event in [Error, Warning, ExecWarning, ScriptWarning] {
            let r = format_message(event, None, "Warning", "oops", false);
            assert_eq!(r.text, "oops");
            assert!(r.increment_warning_count);
        }
    }

    #[test]
    fn warning_family_with_context_formats_context_name_text() {
        let r = format_message(Error, Some("MyContext"), "Error", "boom", false);
        assert_eq!(r.text, "MyContext : Error, boom");
        assert!(r.increment_warning_count);
    }

    #[test]
    fn only_error_event_can_show_the_error_box_and_only_unsuppressed() {
        assert!(format_message(Error, None, "Error", "x", false).show_error_box);
        assert!(!format_message(Error, None, "Error", "x", true).show_error_box);
        assert!(!format_message(Warning, None, "Warning", "x", false).show_error_box);
        assert!(!format_message(ExecWarning, None, "ExecWarning", "x", false).show_error_box);
        assert!(!format_message(ScriptWarning, None, "ScriptWarning", "x", false).show_error_box);
    }

    #[test]
    fn yes_no_auto_answers_when_suppressed_or_headless() {
        assert!(yes_no_auto_answer(true, true, true));
        assert!(yes_no_auto_answer(false, false, false));
        assert!(!yes_no_auto_answer(false, true, false));
        assert!(!yes_no_auto_answer(false, false, true));
    }

    #[test]
    fn error_box_suppressed_by_no_frontend_or_non_tty() {
        assert!(should_show_error_box(false, true));
        assert!(!should_show_error_box(true, true));
        assert!(!should_show_error_box(false, false));
        assert!(!should_show_error_box(true, false));
    }

    use std::cell::RefCell;

    thread_local! {
        static ERROR_CALLS: RefCell<Vec<(&'static str, i32)>> = const { RefCell::new(Vec::new()) };
        static ERROR_STATE: RefCell<(bool, bool)> = const { RefCell::new((false, false)) }; // (critical, guarded)
    }

    fn leaf_name(leaf: Hp2LauncherErrorLeaf) -> &'static str {
        use Hp2LauncherErrorLeaf::*;
        match leaf {
            IsCriticalError => "IsCriticalError",
            SetCriticalError => "SetCriticalError",
            RunFirstErrorSequence => "RunFirstErrorSequence",
            LogReentered => "LogReentered",
            IsGuarded => "IsGuarded",
            RunHandleError => "RunHandleError",
            ThrowError => "ThrowError",
            RequestExit => "RequestExit",
            SetNotGuarded => "SetNotGuarded",
            ClearIsRunning => "ClearIsRunning",
            ClearLogHook => "ClearLogHook",
            StaticShutdownAfterError => "StaticShutdownAfterError",
            IsFriendlyError => "IsFriendlyError",
            TruncateErrorHistory => "TruncateErrorHistory",
            PrintErrorHistory => "PrintErrorHistory",
            QueryNoFrontend => "QueryNoFrontend",
            QueryStdinIsTty => "QueryStdinIsTty",
            ShowMessageBox => "ShowMessageBox",
        }
    }

    unsafe extern "C" fn fake_call(
        _ctx: *mut c_void,
        leaf: Hp2LauncherErrorLeaf,
        arg: i32,
        out: *mut i32,
    ) -> i32 {
        use Hp2LauncherErrorLeaf::*;
        ERROR_CALLS.with(|c| c.borrow_mut().push((leaf_name(leaf), arg)));
        if matches!(leaf, ThrowError) {
            // Simulates `throw(1);` being captured by the leaf's own
            // `Checked()` wrapper: the call reports its captured failure
            // status instead of returning normally.
            return 1;
        }
        let value = match leaf {
            IsCriticalError => ERROR_STATE.with(|s| s.borrow().0) as i32,
            SetCriticalError => {
                ERROR_STATE.with(|s| s.borrow_mut().0 = true);
                0
            }
            IsGuarded => ERROR_STATE.with(|s| s.borrow().1) as i32,
            SetNotGuarded => {
                ERROR_STATE.with(|s| s.borrow_mut().1 = false);
                0
            }
            _ => 0,
        };
        unsafe { out.write(value) };
        0
    }

    fn run_serialize(initial_critical: bool, initial_guarded: bool) -> Vec<(&'static str, i32)> {
        ERROR_STATE.with(|s| *s.borrow_mut() = (initial_critical, initial_guarded));
        ERROR_CALLS.with(|c| c.borrow_mut().clear());
        let services = Hp2LauncherErrorServices { context: std::ptr::null_mut(), call: fake_call };
        let result = unsafe { serialize(&services) };
        if initial_guarded {
            assert_eq!(result, Err(1), "guarded first/reentered error must propagate ThrowError's failure");
        } else {
            assert_eq!(result, Ok(()));
        }
        ERROR_CALLS.with(|c| c.borrow().clone())
    }

    #[test]
    fn first_guarded_error_runs_sequence_then_throws_without_handling_inline() {
        let calls = run_serialize(false, true);
        assert_eq!(
            calls,
            vec![
                ("IsCriticalError", 0),
                ("SetCriticalError", 0),
                ("RunFirstErrorSequence", 0),
                ("IsGuarded", 0),
                ("IsGuarded", 0),
                ("ThrowError", 0),
            ]
        );
    }

    #[test]
    fn first_unguarded_error_handles_inline_then_requests_exit() {
        let calls = run_serialize(false, false);
        assert_eq!(
            calls,
            vec![
                ("IsCriticalError", 0),
                ("SetCriticalError", 0),
                ("RunFirstErrorSequence", 0),
                ("IsGuarded", 0),
                ("RunHandleError", 0),
                ("IsGuarded", 0),
                ("RequestExit", 0),
            ]
        );
    }

    #[test]
    fn reentered_error_only_logs_then_follows_guarded_state() {
        let calls = run_serialize(true, false);
        assert_eq!(calls, vec![("IsCriticalError", 0), ("LogReentered", 0), ("IsGuarded", 0), ("RequestExit", 0)]);
    }

    #[test]
    fn handle_error_sequence_uses_friendly_flag_for_truncation_and_gates_the_box() {
        ERROR_CALLS.with(|c| c.borrow_mut().clear());
        unsafe extern "C" fn call_friendly(
            _ctx: *mut c_void,
            leaf: Hp2LauncherErrorLeaf,
            arg: i32,
            out: *mut i32,
        ) -> i32 {
            use Hp2LauncherErrorLeaf::*;
            ERROR_CALLS.with(|c| c.borrow_mut().push((leaf_name(leaf), arg)));
            let value = match leaf {
                IsFriendlyError => 1,
                QueryNoFrontend => 0,
                QueryStdinIsTty => 1,
                _ => 0,
            };
            unsafe { out.write(value) };
            0
        }
        let services = Hp2LauncherErrorServices { context: std::ptr::null_mut(), call: call_friendly };
        unsafe { handle_error(&services) }.unwrap();
        let calls = ERROR_CALLS.with(|c| c.borrow().clone());
        assert_eq!(
            calls,
            vec![
                ("SetNotGuarded", 0),
                ("ClearIsRunning", 0),
                ("SetCriticalError", 0),
                ("ClearLogHook", 0),
                ("StaticShutdownAfterError", 0),
                ("IsFriendlyError", 0),
                ("TruncateErrorHistory", 1),
                ("PrintErrorHistory", 0),
                ("QueryNoFrontend", 0),
                ("QueryStdinIsTty", 0),
                ("ShowMessageBox", 0),
            ]
        );
    }
}
