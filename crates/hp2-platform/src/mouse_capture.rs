//! Replaces `HP2Capture::OnLoss`/`OnRestore`
//! (`SDLDrv/Inc/HP2MouseCapturePolicy.h:81-102`), the pure
//! mouse-grab-state decision seam `USDLViewport`'s
//! `SDL_WINDOWEVENT` handler consults. Enum/struct field names and
//! doc wording below mirror the real header's own comments closely
//! -- it is the authoritative description of this behavior, not
//! reworded.

/// Replaces `HP2Capture::EAction`: what the caller must do to the
/// grab state. `enum EAction { Action_None, Action_Release,
/// Action_Restore };` assigns no explicit values in the real C++,
/// so its ordinals are the default 0/1/2 -- matching Rust's own
/// default enum numbering already, but pinned explicitly here (not
/// relying on declaration order staying untouched) since this value
/// is meant to cross an eventual FFI boundary as a raw `i32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Action {
    /// No grab-state change.
    None = 0,
    /// `UpdateMouseGrabState(FALSE); LostGrab = 1`.
    Release = 1,
    /// `UpdateMouseGrabState(CaptureMouse config); LostGrab = 0`.
    Restore = 2,
}

/// Replaces `HP2Capture::ECursor`: expected cursor-visibility effect
/// of the transition. Informational -- the real call site gates
/// Show/Hide on its own `ActiveCursor != NULL` check. Unlike
/// `EAction`, the real C++ `enum ECursor { Cursor_Hide = -1,
/// Cursor_Leave = 0, Cursor_Show = 1 };` assigns explicit,
/// non-default values -- Rust's own default enum numbering
/// (`Hide=0, Leave=1, Show=2`) would NOT match, and an eventual
/// `cursor as i32` at an FFI cutover would hand the real C++ caller
/// the wrong action (`Hide` crossing as `0`, the real `Leave`
/// value) without these pinned explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Cursor {
    /// Transition expects the cursor to become hidden.
    Hide = -1,
    /// No cursor-visibility change expected.
    Leave = 0,
    /// Transition expects the cursor to become visible.
    Show = 1,
}

/// Replaces `HP2Capture::FState`.
pub struct State {
    /// `USDLViewport::MouseIsGrabbed` before the event.
    pub currently_grabbed: bool,
    /// `USDLViewport::LostGrab` before the event.
    pub loss_pending: bool,
    /// `UClient::CaptureMouse` configuration.
    pub capture_mouse_config: bool,
}

/// Replaces `HP2Capture::FWindowFlags`.
pub struct WindowFlags {
    /// `SDL_WINDOW_INPUT_FOCUS` set.
    pub has_input_focus: bool,
    /// `SDL_WINDOW_MINIMIZED` or `SDL_WINDOW_HIDDEN` set.
    pub minimized_or_hidden: bool,
}

/// Replaces `HP2Capture::FDecision`.
#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Decision {
    pub action: Action,
    pub cursor: Cursor,
}

const _: () = assert!(std::mem::size_of::<Decision>() == 8);
const _: () = assert!(std::mem::offset_of!(Decision, action) == 0);
const _: () = assert!(std::mem::offset_of!(Decision, cursor) == 4);

/// Replaces `HP2Capture::OnLoss`: decision for one loss event
/// (`FOCUS_LOST` / `MINIMIZED` / `HIDDEN`). The grab is released only
/// on the first loss while grabbed; repeated loss events while a loss
/// is already pending leave capture state alone.
pub fn on_loss(state: &State) -> Decision {
    if !state.loss_pending && state.currently_grabbed {
        return Decision {
            action: Action::Release,
            cursor: Cursor::Show,
        };
    }
    Decision {
        action: Action::None,
        cursor: Cursor::Leave,
    }
}

/// Replaces `HP2Capture::OnRestore`: decision for one restore event
/// (`FOCUS_GAINED` / `RESTORED` / `SHOWN`). The grab returns only when
/// a loss is pending AND SDL reports input focus AND the window is
/// neither minimized nor hidden. The restored grab follows the
/// `CaptureMouse` configuration rather than the pre-loss state.
pub fn on_restore(state: &State, window: &WindowFlags) -> Decision {
    if state.loss_pending && window.has_input_focus && !window.minimized_or_hidden {
        return Decision {
            action: Action::Restore,
            cursor: if state.capture_mouse_config {
                Cursor::Hide
            } else {
                Cursor::Leave
            },
        };
    }
    Decision {
        action: Action::None,
        cursor: Cursor::Leave,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Matches `HP2MouseCapturePolicy.h:45-59`'s explicit C++
    // discriminants exactly: `EAction` assigns none (so its real
    // ordinals are the default 0/1/2), `ECursor` assigns
    // `Hide=-1, Leave=0, Show=1`. This guards the `#[repr(i32)]`
    // values above against silent drift (e.g. a reordered variant)
    // at whatever point these cross an FFI boundary as a raw `i32`.
    #[test]
    fn action_and_cursor_discriminants_match_the_real_cpp_enums() {
        assert_eq!(Action::None as i32, 0);
        assert_eq!(Action::Release as i32, 1);
        assert_eq!(Action::Restore as i32, 2);
        assert_eq!(Cursor::Hide as i32, -1);
        assert_eq!(Cursor::Leave as i32, 0);
        assert_eq!(Cursor::Show as i32, 1);
    }

    // Literal transcription of the real, unmodified C++ oracle test
    // tables (`Tests/InputContractTests.cpp:501-568`,
    // `TestMouseCapturePolicy`, registered as the `mouse_capture_policy`
    // ctest -- one of Phase 2's named gates, part of the project's
    // ctest baseline already). Header-only `inline` functions have no
    // linkage distinction to check via `nm` the way a `.cpp`-defined
    // function would; the real behavior is pinned by this already-
    // passing, already-table-driven C++ test, which these cases
    // reproduce row for row, same names, same inputs, same expected
    // outputs.

    #[test]
    fn grabbed_first_loss_releases_and_shows_cursor() {
        let state = State {
            currently_grabbed: true,
            loss_pending: false,
            capture_mouse_config: true,
        };
        assert_eq!(
            on_loss(&state),
            Decision {
                action: Action::Release,
                cursor: Cursor::Show
            }
        );
    }

    #[test]
    fn grabbed_repeat_loss_is_a_no_op() {
        let state = State {
            currently_grabbed: true,
            loss_pending: true,
            capture_mouse_config: true,
        };
        assert_eq!(
            on_loss(&state),
            Decision {
                action: Action::None,
                cursor: Cursor::Leave
            }
        );
    }

    #[test]
    fn ungrabbed_loss_is_a_no_op() {
        let state = State {
            currently_grabbed: false,
            loss_pending: false,
            capture_mouse_config: true,
        };
        assert_eq!(
            on_loss(&state),
            Decision {
                action: Action::None,
                cursor: Cursor::Leave
            }
        );
    }

    #[test]
    fn ungrabbed_stale_loss_is_a_no_op() {
        let state = State {
            currently_grabbed: false,
            loss_pending: true,
            capture_mouse_config: false,
        };
        assert_eq!(
            on_loss(&state),
            Decision {
                action: Action::None,
                cursor: Cursor::Leave
            }
        );
    }

    #[test]
    fn capture_config_is_ignored_on_the_loss_side() {
        let state = State {
            currently_grabbed: true,
            loss_pending: false,
            capture_mouse_config: false,
        };
        assert_eq!(
            on_loss(&state),
            Decision {
                action: Action::Release,
                cursor: Cursor::Show
            }
        );
    }

    #[test]
    fn restore_with_capture_on_grabs_and_hides_cursor() {
        let state = State {
            currently_grabbed: true,
            loss_pending: true,
            capture_mouse_config: true,
        };
        let window = WindowFlags {
            has_input_focus: true,
            minimized_or_hidden: false,
        };
        assert_eq!(
            on_restore(&state, &window),
            Decision {
                action: Action::Restore,
                cursor: Cursor::Hide
            }
        );
    }

    #[test]
    fn restore_with_capture_off_grabs_but_leaves_cursor_alone() {
        // `UpdateMouseGrabState(FALSE)` sees no transition, so the
        // cursor keeps the visibility it gained during the release --
        // a frozen subtlety per the real header's own comment, not a
        // simplification in this port.
        let state = State {
            currently_grabbed: true,
            loss_pending: true,
            capture_mouse_config: false,
        };
        let window = WindowFlags {
            has_input_focus: true,
            minimized_or_hidden: false,
        };
        assert_eq!(
            on_restore(&state, &window),
            Decision {
                action: Action::Restore,
                cursor: Cursor::Leave
            }
        );
    }

    #[test]
    fn restore_without_input_focus_waits() {
        let state = State {
            currently_grabbed: true,
            loss_pending: true,
            capture_mouse_config: true,
        };
        let window = WindowFlags {
            has_input_focus: false,
            minimized_or_hidden: false,
        };
        assert_eq!(
            on_restore(&state, &window),
            Decision {
                action: Action::None,
                cursor: Cursor::Leave
            }
        );
    }

    #[test]
    fn restore_while_minimized_waits() {
        let state = State {
            currently_grabbed: true,
            loss_pending: true,
            capture_mouse_config: true,
        };
        let window = WindowFlags {
            has_input_focus: true,
            minimized_or_hidden: true,
        };
        assert_eq!(
            on_restore(&state, &window),
            Decision {
                action: Action::None,
                cursor: Cursor::Leave
            }
        );
    }

    #[test]
    fn restore_without_a_pending_loss_is_a_no_op() {
        // The real test loop hardcodes `bCurrentlyGrabbed = true` for
        // every restore row regardless of the row's own fields (see
        // `InputContractTests.cpp:552`) -- matched here for literal
        // fidelity, though `on_restore` never actually reads this
        // field (only `on_loss` does).
        let state = State {
            currently_grabbed: true,
            loss_pending: false,
            capture_mouse_config: true,
        };
        let window = WindowFlags {
            has_input_focus: true,
            minimized_or_hidden: false,
        };
        assert_eq!(
            on_restore(&state, &window),
            Decision {
                action: Action::None,
                cursor: Cursor::Leave
            }
        );
    }

    /// Menu state (`USDLClient::InMenuLoop`) deliberately does not
    /// take part in this decision -- today's driver never consults it
    /// when grabbing or releasing. A documented gap in the real
    /// header, not a policy input; this test exists so a future
    /// change that starts threading menu state through can't silently
    /// change this function's meaning without a test noticing.
    #[test]
    fn menu_state_does_not_change_the_decision() {
        let state = State {
            currently_grabbed: true,
            loss_pending: false,
            capture_mouse_config: true,
        };
        let window = WindowFlags {
            has_input_focus: true,
            minimized_or_hidden: false,
        };
        assert_eq!(on_restore(&state, &window).action, Action::None);
    }
}
