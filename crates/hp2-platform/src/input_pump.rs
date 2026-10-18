//! Rust side of `USDLViewport::UpdateInput` (`SDLDrv/Src/SDLViewport.cpp`,
//! 06bfdd3:351-497), `ReleaseAllInput` (06bfdd3:602-624; the cursor here
//! replaces the old `ReleaseJoystickInput` helper), and the repaint-victim
//! predicate `repaint_replaces_best` used by `USDLClient::Tick`
//! (`SDLClient.cpp`, 06bfdd3:185-211).
//!
//! What is here: the SDL event-type dispatch for every event the pump
//! handles, the normalized key/button/wheel/text/joystick mapping, the
//! window loss/restore/resize decisions, the relative-mouse deltas, the
//! ReleaseAllInput iteration order, and the stalest-realtime-viewport pick.
//!
//! What stays in C++ and why: `SDL_PollEvent`, `SDL_GetRelativeMouseState`,
//! `SDL_GetWindowFlags`, and the flattening of each `SDL_Event` into
//! [`SdlEvent`] are SDL calls that only the C++ shell can make against its
//! own `SDL_Event` union. The engine-global services (`UEngine::Key`,
//! `UEngine::MousePosition`, `UEngine::InputEvent`, `UInput::KeyDown`,
//! `UpdateMouseGrabState`, `ResizeViewport`) are reached through the C++
//! executor, which runs the returned [`Action`]s in order. Rust never holds
//! state across an engine call, so an engine exception unwinds only through
//! C++ frames.
//!
//! SDL constants below are hardcoded from the pinned SDL2 headers
//! (`out/macos-arm64/_deps/hp2_sdl2-src/include/SDL_events.h`,
//! `SDL_video.h`); `SDLViewport.cpp` `static_assert`s every one of them
//! against the same header so a drift breaks the C++ build.

use crate::input_event::{self, HP2InputEvent, ik, ist};
use crate::joystick_input::{self, JOY_AXIS_KEYS, MAX_JOYSTICK_AXES};
use crate::mouse_capture::{self, Action as CaptureAction, State, WindowFlags};
use crate::mouse_state;

/// `SDL_EventType` values used by the pump.
pub mod sdl_event {
    pub const WINDOWEVENT: u32 = 0x200;
    pub const KEYDOWN: u32 = 0x300;
    pub const KEYUP: u32 = 0x301;
    pub const TEXTINPUT: u32 = 0x303;
    pub const MOUSEMOTION: u32 = 0x400;
    pub const MOUSEBUTTONDOWN: u32 = 0x401;
    pub const MOUSEBUTTONUP: u32 = 0x402;
    pub const MOUSEWHEEL: u32 = 0x403;
    pub const JOYAXISMOTION: u32 = 0x600;
    pub const JOYHATMOTION: u32 = 0x602;
    pub const JOYBUTTONDOWN: u32 = 0x603;
    pub const JOYBUTTONUP: u32 = 0x604;
}

/// `SDL_WindowEventID` values used by the pump (`SDL_video.h`, enum counts
/// from `SDL_WINDOWEVENT_NONE = 0`).
pub mod sdl_window_event {
    pub const SHOWN: u8 = 1;
    pub const HIDDEN: u8 = 2;
    pub const RESIZED: u8 = 5;
    pub const MINIMIZED: u8 = 7;
    pub const RESTORED: u8 = 9;
    pub const FOCUS_GAINED: u8 = 12;
    pub const FOCUS_LOST: u8 = 13;
}

/// `SDL_WINDOW_*` flag bits (`SDL_video.h`).
pub mod sdl_window_flags {
    pub const HIDDEN: u32 = 0x0000_0008;
    pub const MINIMIZED: u32 = 0x0000_0040;
    pub const INPUT_FOCUS: u32 = 0x0000_0200;
}

/// `SDL_TEXTINPUTEVENT_TEXT_SIZE` (`SDL_events.h`).
pub const TEXT_SIZE: usize = 32;

/// Upper bound on actions one event can produce. The widest case is a
/// window loss (`UpdateMouseGrab`, `SetLostGrab`, `ReleaseAll`) or a wheel
/// notch with a joystick hat fold; nothing emits more than three.
pub const MAX_ACTIONS: usize = 4;

/// One polled `SDL_Event`, flattened by the C++ shell. Only the fields the
/// event's own `type` uses are meaningful; the rest are zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SdlEvent {
    pub kind: u32,
    /// `keysym.sym` for `KEYDOWN`/`KEYUP`.
    pub key_sym: i32,
    /// `text.text` for `TEXTINPUT`, NUL-padded.
    pub text: [u8; TEXT_SIZE],
    /// `motion.x`/`motion.y`/`motion.state` for `MOUSEMOTION`.
    pub motion_x: i32,
    pub motion_y: i32,
    pub motion_state: u32,
    /// `button.button` for `MOUSEBUTTONDOWN`/`UP`.
    pub button: u8,
    /// `wheel.y` for `MOUSEWHEEL`.
    pub wheel_y: i32,
    /// `jaxis.axis`/`jaxis.value` for `JOYAXISMOTION`.
    pub joy_axis: u8,
    pub joy_axis_value: i32,
    /// `jbutton.button` for `JOYBUTTONDOWN`/`UP`.
    pub joy_button: u8,
    /// `jhat.value` for `JOYHATMOTION`.
    pub joy_hat: u8,
    /// `window.event`, `window.data1`/`data2`, and `SDL_GetWindowFlags` for
    /// `WINDOWEVENT`. `window_flags` is zero when the viewport has no window.
    pub window_event: u8,
    pub window_data1: i32,
    pub window_data2: i32,
    pub window_flags: u32,
}

/// Per-event snapshot of the `USDLViewport`/`USDLClient` fields the pump reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Context {
    /// `MouseIsGrabbed`.
    pub mouse_is_grabbed: bool,
    /// `LostGrab`.
    pub lost_grab: bool,
    /// `UClient::CaptureMouse` (`C->CaptureMouse`).
    pub capture_mouse: bool,
    /// `SdlWindow != NULL`.
    pub has_window: bool,
    /// `C->Engine != NULL`.
    pub has_engine: bool,
    /// `Actor != NULL`.
    pub has_actor: bool,
    /// `C->JoyHatFoldsToButtons`.
    pub joy_hat_folds_to_buttons: bool,
    /// `MouseScaleX`/`MouseScaleY`.
    pub mouse_scale_x: f32,
    pub mouse_scale_y: f32,
    /// `SizeX`/`SizeY`.
    pub size_x: i32,
    pub size_y: i32,
    /// `C->JoystickDeadZone`, `C->ScaleJBX`, `C->ScaleJBY`.
    pub joystick_dead_zone: f32,
    pub scale_jbx: f32,
    pub scale_jby: f32,
    /// `LastJoyHat`.
    pub last_joy_hat: i32,
}

/// One side effect for the C++ executor, in original call order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// `CauseInputEvent(key, action, delta)`.
    Cause { key: i32, action: i32, delta: f32 },
    /// `ClientEngine->Key(this, key)`.
    EngineKey { key: i32 },
    /// `C->Engine->MousePosition(this, buttons, x, y)`.
    EngineMousePosition { buttons: u32, x: f32, y: f32 },
    /// `UpdateMouseGrabState(capture)`.
    UpdateMouseGrab { capture: bool },
    /// `LostGrab = value`.
    SetLostGrab(bool),
    /// `LastJoyHat = key`.
    SetLastJoyHat(i32),
    /// `ReleaseAllInput()`; the C++ executor runs [`release_all_next`].
    ReleaseAll,
    /// `ResizeViewport(BLIT_NoWindowChange, width, height, INDEX_NONE)`.
    ResizeViewport { width: i32, height: i32 },
}

/// Fixed-capacity, order-preserving action list (no allocation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionList {
    items: [Option<Action>; MAX_ACTIONS],
    len: usize,
}

impl ActionList {
    pub fn new() -> Self {
        Self {
            items: [None; MAX_ACTIONS],
            len: 0,
        }
    }

    fn push(&mut self, action: Action) {
        assert!(
            self.len < MAX_ACTIONS,
            "input pump produced more than MAX_ACTIONS actions"
        );
        self.items[self.len] = Some(action);
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = Action> + '_ {
        self.items[..self.len].iter().flatten().copied()
    }
}

impl Default for ActionList {
    fn default() -> Self {
        Self::new()
    }
}

fn push_normalized(out: &mut ActionList, event: HP2InputEvent) {
    if event.key != ik::NONE {
        out.push(Action::Cause {
            key: event.key,
            action: event.action,
            delta: 0.0,
        });
    }
}

/// Translates one polled event. Mirrors the `switch (Event.type)` in
/// `UpdateInput`; every arm keeps the original's guards and call order.
pub fn translate_event(ctx: &Context, ev: &SdlEvent) -> ActionList {
    let mut out = ActionList::new();
    match ev.kind {
        sdl_event::KEYDOWN | sdl_event::KEYUP => {
            let normalized = input_event::make_key_event(ev.key_sym, ev.kind == sdl_event::KEYDOWN);
            push_normalized(&mut out, normalized);
        }
        sdl_event::TEXTINPUT => {
            let len = ev.text.iter().position(|&b| b == 0).unwrap_or(TEXT_SIZE);
            let normalized = input_event::make_text_event(Some(&ev.text[..len]));
            if normalized.key != ik::NONE && ctx.has_engine && ctx.has_actor {
                out.push(Action::EngineKey {
                    key: normalized.key,
                });
            }
        }
        sdl_event::MOUSEMOTION => {
            if !ctx.mouse_is_grabbed && ctx.has_engine {
                out.push(Action::EngineMousePosition {
                    buttons: mouse_state::buttons_from_sdl_state(ev.motion_state),
                    x: ev.motion_x as f32 * ctx.mouse_scale_x,
                    y: ev.motion_y as f32 * ctx.mouse_scale_y,
                });
            }
        }
        sdl_event::MOUSEBUTTONDOWN | sdl_event::MOUSEBUTTONUP => {
            let normalized = input_event::make_mouse_button_event(
                ev.button,
                ev.kind == sdl_event::MOUSEBUTTONDOWN,
            );
            push_normalized(&mut out, normalized);
        }
        sdl_event::MOUSEWHEEL => {
            let mut wheel = [input_event::make_none_event(); 3];
            let count = input_event::make_wheel_events(ev.wheel_y, &mut wheel);
            for event in wheel.iter().take(count as usize) {
                out.push(Action::Cause {
                    key: event.key,
                    action: event.action,
                    delta: event.delta,
                });
            }
        }
        sdl_event::JOYAXISMOTION => {
            if let Some((key, value)) = joystick_input::map_joystick_axis(
                ev.joy_axis as i32,
                ev.joy_axis_value,
                ctx.joystick_dead_zone,
                ctx.scale_jbx,
                ctx.scale_jby,
            ) {
                out.push(Action::Cause {
                    key,
                    action: ist::AXIS,
                    delta: value,
                });
            }
        }
        sdl_event::JOYBUTTONDOWN | sdl_event::JOYBUTTONUP => {
            if let Some((key, action)) = joystick_input::map_joystick_button(
                ev.joy_button as i32,
                ev.kind == sdl_event::JOYBUTTONDOWN,
            ) {
                out.push(Action::Cause {
                    key,
                    action,
                    delta: 0.0,
                });
            }
        }
        sdl_event::JOYHATMOTION => {
            if ctx.joy_hat_folds_to_buttons {
                let (new_last, events, count) =
                    joystick_input::fold_joystick_hat(ev.joy_hat, ctx.last_joy_hat);
                for &(key, action) in events.iter().take(count) {
                    out.push(Action::Cause {
                        key,
                        action,
                        delta: 0.0,
                    });
                }
                out.push(Action::SetLastJoyHat(new_last));
            }
        }
        sdl_event::WINDOWEVENT => translate_window(ctx, ev, &mut out),
        _ => {}
    }
    out
}

fn translate_window(ctx: &Context, ev: &SdlEvent, out: &mut ActionList) {
    use sdl_window_event as we;

    let kind = ev.window_event;
    let loss = kind == we::FOCUS_LOST || kind == we::MINIMIZED || kind == we::HIDDEN;
    let restore = kind == we::FOCUS_GAINED || kind == we::RESTORED || kind == we::SHOWN;
    let state = State {
        currently_grabbed: ctx.mouse_is_grabbed,
        loss_pending: ctx.lost_grab,
        capture_mouse_config: ctx.capture_mouse,
    };

    if loss {
        if matches!(
            mouse_capture::on_loss(&state).action,
            CaptureAction::Release
        ) {
            out.push(Action::UpdateMouseGrab { capture: false });
            out.push(Action::SetLostGrab(true));
        }
        // Held input dies on every loss event, grabbed or not.
        out.push(Action::ReleaseAll);
    } else if restore && ctx.has_window {
        let flags = WindowFlags {
            has_input_focus: ev.window_flags & sdl_window_flags::INPUT_FOCUS != 0,
            minimized_or_hidden: ev.window_flags
                & (sdl_window_flags::MINIMIZED | sdl_window_flags::HIDDEN)
                != 0,
        };
        if matches!(
            mouse_capture::on_restore(&state, &flags).action,
            CaptureAction::Restore
        ) {
            out.push(Action::UpdateMouseGrab {
                capture: ctx.capture_mouse,
            });
            out.push(Action::SetLostGrab(false));
        }
    } else if kind == we::RESIZED {
        // Wayland compositors can resize the xdg-toplevel out from under us
        // (tiling layout, compositor-driven fullscreen) without ever routing
        // through ToggleFullscreen/EndFullscreen. Left unhandled, SizeX/SizeY,
        // MouseScaleX/Y, and the render device's resolution go stale while
        // SDL's own window size (and therefore SDL_MOUSEMOTION coordinates)
        // track the new size, so the menu cursor drifts away from the pointer
        // by the gap between the old and new size.
        let (width, height) = (ev.window_data1, ev.window_data2);
        if width > 0 && height > 0 && (width != ctx.size_x || height != ctx.size_y) {
            out.push(Action::ResizeViewport { width, height });
        }
    }
}

/// Relative-mouse deltas sampled once per pump while grabbed (the C++ shell
/// gates the `SDL_GetRelativeMouseState` call on `MouseIsGrabbed && SdlWindow`).
pub fn relative_mouse(ctx: &Context, delta_x: i32, delta_y: i32) -> ActionList {
    let mut out = ActionList::new();
    if delta_x != 0 {
        out.push(Action::Cause {
            key: ik::MOUSE_X,
            action: ist::AXIS,
            delta: delta_x as f32 * ctx.mouse_scale_x,
        });
    }
    if delta_y != 0 {
        out.push(Action::Cause {
            key: ik::MOUSE_Y,
            action: ist::AXIS,
            delta: -(delta_y as f32) * ctx.mouse_scale_y,
        });
    }
    out
}

/// Mouse axes neutralized last by `ReleaseAllInput`, in original order.
const RELEASE_MOUSE_AXES: [i32; 3] = [ik::MOUSE_X, ik::MOUSE_Y, ik::MOUSE_W];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleasePhase {
    Keys,
    JoyAxes,
    JoyHat,
    Mouse,
    Done,
}

/// Iteration state of `ReleaseAllInput`, advanced by [`release_all_next`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseAllCursor {
    pub phase: ReleasePhase,
    pub index: i32,
    pub awaiting_key: bool,
}

/// One step of `ReleaseAllInput`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReleaseStep {
    /// The executor must answer `Input->KeyDown(key)` and pass it back
    /// as `held` on the next [`release_all_next`] call.
    QueryKeyDown(i32),
    /// `CauseInputEvent(key, action, delta)`.
    Cause { key: i32, action: i32, delta: f32 },
    /// `LastJoyHat = key`.
    SetLastJoyHat(i32),
}

/// Starts `ReleaseAllInput`. `has_input` is `Input != NULL`; when false the
/// per-key loop is skipped, exactly like the original `if( Input )` guard.
pub fn release_all_begin(has_input: bool) -> ReleaseAllCursor {
    ReleaseAllCursor {
        phase: if has_input {
            ReleasePhase::Keys
        } else {
            ReleasePhase::JoyAxes
        },
        index: 0,
        awaiting_key: false,
    }
}

/// Advances `ReleaseAllInput`. `held` answers the most recent
/// [`ReleaseStep::QueryKeyDown`]; it is ignored otherwise. Returns `None`
/// once the sequence is finished.
pub fn release_all_next(cursor: &mut ReleaseAllCursor, held: bool) -> Option<ReleaseStep> {
    loop {
        match cursor.phase {
            ReleasePhase::Keys => {
                if cursor.awaiting_key {
                    cursor.awaiting_key = false;
                    let key = cursor.index;
                    cursor.index += 1;
                    if held {
                        return Some(ReleaseStep::Cause {
                            key,
                            action: ist::RELEASE,
                            delta: 0.0,
                        });
                    }
                } else if cursor.index < ik::MAX {
                    cursor.awaiting_key = true;
                    return Some(ReleaseStep::QueryKeyDown(cursor.index));
                } else {
                    cursor.phase = ReleasePhase::JoyAxes;
                    cursor.index = 0;
                }
            }
            ReleasePhase::JoyAxes => {
                if cursor.index < MAX_JOYSTICK_AXES {
                    let key = JOY_AXIS_KEYS[cursor.index as usize];
                    cursor.index += 1;
                    return Some(ReleaseStep::Cause {
                        key,
                        action: ist::AXIS,
                        delta: 0.0,
                    });
                }
                cursor.phase = ReleasePhase::JoyHat;
            }
            ReleasePhase::JoyHat => {
                cursor.phase = ReleasePhase::Mouse;
                cursor.index = 0;
                return Some(ReleaseStep::SetLastJoyHat(ik::NONE));
            }
            ReleasePhase::Mouse => {
                if (cursor.index as usize) < RELEASE_MOUSE_AXES.len() {
                    let key = RELEASE_MOUSE_AXES[cursor.index as usize];
                    cursor.index += 1;
                    return Some(ReleaseStep::Cause {
                        key,
                        action: ist::AXIS,
                        delta: 0.0,
                    });
                }
                cursor.phase = ReleasePhase::Done;
            }
            ReleasePhase::Done => return None,
        }
    }
}

/// One non-null entry of `USDLClient::Viewports` as seen by `Tick`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RepaintCandidate {
    /// `Viewport->IsRealtime()`.
    pub realtime: bool,
    /// `Viewport->SizeX`/`SizeY`.
    pub size_x: i32,
    pub size_y: i32,
    /// `(victim->LastUpdateTime - Viewport->LastUpdateTime)` as computed by
    /// `FTime::operator-`, which returns the exact sign of the 64-bit fixed-point
    /// difference. Positive means the victim is strictly more recent than this
    /// viewport. Passing the sign, rather than two rounded floats, keeps Tick's
    /// comparison identical to the original `FTime` `>` comparison.
    pub best_minus_candidate: f32,
}

/// Whether `USDLClient::Tick` replaces the current repaint victim with
/// `candidate`. An ineligible candidate never replaces. An eligible one replaces
/// when there is no victim yet, or when the victim's time is strictly greater
/// (`best_minus_candidate > 0`); equal times keep the earlier viewport.
pub fn repaint_replaces_best(has_best: bool, candidate: RepaintCandidate) -> bool {
    if !(candidate.realtime && candidate.size_x != 0 && candidate.size_y != 0) {
        return false;
    }
    !has_best || candidate.best_minus_candidate > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            mouse_is_grabbed: false,
            lost_grab: false,
            capture_mouse: false,
            has_window: true,
            has_engine: true,
            has_actor: true,
            joy_hat_folds_to_buttons: false,
            mouse_scale_x: 1.0,
            mouse_scale_y: 1.0,
            size_x: 800,
            size_y: 600,
            joystick_dead_zone: 0.0,
            scale_jbx: 1.0,
            scale_jby: 1.0,
            last_joy_hat: ik::NONE,
        }
    }

    fn event(kind: u32) -> SdlEvent {
        SdlEvent {
            kind,
            key_sym: 0,
            text: [0; TEXT_SIZE],
            motion_x: 0,
            motion_y: 0,
            motion_state: 0,
            button: 0,
            wheel_y: 0,
            joy_axis: 0,
            joy_axis_value: 0,
            joy_button: 0,
            joy_hat: 0,
            window_event: 0,
            window_data1: 0,
            window_data2: 0,
            window_flags: 0,
        }
    }

    fn window(kind: u8, flags: u32) -> SdlEvent {
        SdlEvent {
            window_event: kind,
            window_flags: flags,
            ..event(sdl_event::WINDOWEVENT)
        }
    }

    fn collect(list: &ActionList) -> Vec<Action> {
        list.iter().collect()
    }

    #[test]
    fn sdl_constants_match_pinned_header_values() {
        assert_eq!(sdl_event::WINDOWEVENT, 0x200);
        assert_eq!(sdl_event::KEYDOWN, 0x300);
        assert_eq!(sdl_event::KEYUP, 0x301);
        assert_eq!(sdl_event::TEXTINPUT, 0x303);
        assert_eq!(sdl_event::MOUSEMOTION, 0x400);
        assert_eq!(sdl_event::MOUSEBUTTONDOWN, 0x401);
        assert_eq!(sdl_event::MOUSEBUTTONUP, 0x402);
        assert_eq!(sdl_event::MOUSEWHEEL, 0x403);
        assert_eq!(sdl_event::JOYAXISMOTION, 0x600);
        assert_eq!(sdl_event::JOYHATMOTION, 0x602);
        assert_eq!(sdl_event::JOYBUTTONDOWN, 0x603);
        assert_eq!(sdl_event::JOYBUTTONUP, 0x604);
        assert_eq!(sdl_window_event::SHOWN, 1);
        assert_eq!(sdl_window_event::HIDDEN, 2);
        assert_eq!(sdl_window_event::RESIZED, 5);
        assert_eq!(sdl_window_event::MINIMIZED, 7);
        assert_eq!(sdl_window_event::RESTORED, 9);
        assert_eq!(sdl_window_event::FOCUS_GAINED, 12);
        assert_eq!(sdl_window_event::FOCUS_LOST, 13);
        assert_eq!(sdl_window_flags::HIDDEN, 0x8);
        assert_eq!(sdl_window_flags::MINIMIZED, 0x40);
        assert_eq!(sdl_window_flags::INPUT_FOCUS, 0x200);
    }

    #[test]
    fn key_down_emits_cause_for_mapped_key() {
        let mut ev = event(sdl_event::KEYDOWN);
        ev.key_sym = b'a' as i32;
        let out = collect(&translate_event(&ctx(), &ev));
        assert_eq!(out.len(), 1);
        assert!(matches!(
            out[0],
            Action::Cause { key, action, delta } if key == ik::A && action == ist::PRESS && delta == 0.0
        ));
    }

    #[test]
    fn key_with_no_mapping_emits_nothing() {
        let mut ev = event(sdl_event::KEYDOWN);
        ev.key_sym = 0x1F600;
        assert!(translate_event(&ctx(), &ev).is_empty());
    }

    #[test]
    fn text_input_stops_at_nul_and_requires_engine_and_actor() {
        let mut ev = event(sdl_event::TEXTINPUT);
        ev.text[0] = b'a';
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![Action::EngineKey { key: b'a' as i32 }]
        );
        let mut no_actor = ctx();
        no_actor.has_actor = false;
        assert!(translate_event(&no_actor, &ev).is_empty());
    }

    #[test]
    fn mouse_motion_matches_oracle_when_ungrabbed() {
        // Oracle: `harness.cpp` scenario `motion_ungrabbed` (motion x=10, y=20,
        // state=4 right button, scale 0.5/0.25). Trace: `MousePosition buttons=2
        // x=5.0 y=5.0`. The grabbed scenario emits nothing.
        let mut ev = event(sdl_event::MOUSEMOTION);
        ev.motion_x = 10;
        ev.motion_y = 20;
        ev.motion_state = 4;
        let mut c = ctx();
        c.mouse_scale_x = 0.5;
        c.mouse_scale_y = 0.25;
        assert_eq!(
            collect(&translate_event(&c, &ev)),
            vec![Action::EngineMousePosition {
                buttons: 2,
                x: 5.0,
                y: 5.0,
            }]
        );
        c.mouse_is_grabbed = true;
        assert!(translate_event(&c, &ev).is_empty());
    }

    #[test]
    fn wheel_matches_oracle_one_pair_per_call() {
        // Oracle: `harness.cpp` scenario `wheel` (wheel_y=2, -1, 0). wheel_y=2
        // traces `231 axis 2.0`, `236 press`, `236 release`; wheel_y=-1 traces
        // `231 axis -1.0`, `237 press`, `237 release`; wheel_y=0 emits nothing.
        let mut ev = event(sdl_event::MOUSEWHEEL);
        ev.wheel_y = 2;
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_W,
                    action: ist::AXIS,
                    delta: 2.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_UP,
                    action: ist::PRESS,
                    delta: 0.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_UP,
                    action: ist::RELEASE,
                    delta: 0.0,
                },
            ]
        );
        ev.wheel_y = -1;
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_W,
                    action: ist::AXIS,
                    delta: -1.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_DOWN,
                    action: ist::PRESS,
                    delta: 0.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_DOWN,
                    action: ist::RELEASE,
                    delta: 0.0,
                },
            ]
        );
        ev.wheel_y = 0;
        assert!(translate_event(&ctx(), &ev).is_empty());
    }

    #[test]
    fn hat_fold_respects_flag_and_writes_last_hat() {
        let mut ev = event(sdl_event::JOYHATMOTION);
        ev.joy_hat = 1;
        let mut c = ctx();
        assert!(translate_event(&c, &ev).is_empty());
        c.joy_hat_folds_to_buttons = true;
        let (new_last, events, count) = joystick_input::fold_joystick_hat(1, c.last_joy_hat);
        let mut expected: Vec<Action> = events[..count]
            .iter()
            .map(|&(key, action)| Action::Cause {
                key,
                action,
                delta: 0.0,
            })
            .collect();
        expected.push(Action::SetLastJoyHat(new_last));
        assert_eq!(collect(&translate_event(&c, &ev)), expected);
    }

    #[test]
    fn keys_match_oracle() {
        // Oracle: `harness.cpp` scenario `keys`. SDLK_a press/release is
        // key 65 (action 1/3); SDLK_UNKNOWN emits nothing; SDLK_LSHIFT
        // (0x400000E1) is key 160 press/release.
        let key = |kind, sym| {
            let mut ev = event(kind);
            ev.key_sym = sym;
            ev
        };
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYDOWN, b'a' as i32)
            )),
            vec![Action::Cause {
                key: 65,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYUP, b'a' as i32)
            )),
            vec![Action::Cause {
                key: 65,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&ctx(), &key(sdl_event::KEYDOWN, 0)).is_empty());
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYDOWN, 0x4000_00E1)
            )),
            vec![Action::Cause {
                key: 160,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYUP, 0x4000_00E1)
            )),
            vec![Action::Cause {
                key: 160,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
    }

    #[test]
    fn mouse_buttons_match_oracle() {
        // Oracle: `harness.cpp` scenario `buttons`. SDL button 1 down is key 1
        // (press), 2 up is key 4 (release), 3 down is key 2 (press), 4 is unmapped.
        let button = |kind, b| {
            let mut ev = event(kind);
            ev.button = b;
            ev
        };
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::MOUSEBUTTONDOWN, 1)
            )),
            vec![Action::Cause {
                key: 1,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::MOUSEBUTTONUP, 2)
            )),
            vec![Action::Cause {
                key: 4,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::MOUSEBUTTONDOWN, 3)
            )),
            vec![Action::Cause {
                key: 2,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&ctx(), &button(sdl_event::MOUSEBUTTONDOWN, 4)).is_empty());
    }

    #[test]
    fn joystick_axis_matches_oracle_scales() {
        // Oracle: `harness.cpp` scenario `joy_axis` (deadzone 0.1, ScaleJBX=2,
        // ScaleJBY=0.5). Axis 0 at 16384 is 1.0; axis 1 at -32768 is -0.5;
        // axis 2 at 1000 is inside the deadzone and emits 0; axis 8 is dropped.
        let mut c = ctx();
        c.joystick_dead_zone = 0.1;
        c.scale_jbx = 2.0;
        c.scale_jby = 0.5;
        let axis = |index: u8, value: i32| {
            let mut ev = event(sdl_event::JOYAXISMOTION);
            ev.joy_axis = index;
            ev.joy_axis_value = value;
            ev
        };
        assert_eq!(
            collect(&translate_event(&c, &axis(0, 16384))),
            vec![Action::Cause {
                key: 224,
                action: ist::AXIS,
                delta: 1.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(&c, &axis(1, -32768))),
            vec![Action::Cause {
                key: 225,
                action: ist::AXIS,
                delta: -0.5,
            }]
        );
        assert_eq!(
            collect(&translate_event(&c, &axis(2, 1000))),
            vec![Action::Cause {
                key: 226,
                action: ist::AXIS,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&c, &axis(8, 16384)).is_empty());
    }

    #[test]
    fn joystick_button_matches_oracle() {
        // Oracle: `harness.cpp` scenario `joy_button`. Button 3 down is key 203
        // (press); button 16 is dropped; button 0 up is key 200 (release).
        let button = |kind, b| {
            let mut ev = event(kind);
            ev.joy_button = b;
            ev
        };
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::JOYBUTTONDOWN, 3)
            )),
            vec![Action::Cause {
                key: 203,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&ctx(), &button(sdl_event::JOYBUTTONDOWN, 16)).is_empty());
        assert_eq!(
            collect(&translate_event(&ctx(), &button(sdl_event::JOYBUTTONUP, 0))),
            vec![Action::Cause {
                key: 200,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
    }

    #[test]
    fn hat_fold_matches_oracle() {
        // Oracle: `harness.cpp` scenario `hat_fold` (UP|RIGHT = 3, then centered).
        // UP|RIGHT presses key 212 only; centered releases 212.
        let mut c = ctx();
        c.joy_hat_folds_to_buttons = true;
        let mut hat = event(sdl_event::JOYHATMOTION);
        hat.joy_hat = 3;
        let pressed = collect(&translate_event(&c, &hat));
        assert_eq!(
            pressed[0],
            Action::Cause {
                key: 212,
                action: ist::PRESS,
                delta: 0.0,
            }
        );
        assert!(matches!(pressed.last(), Some(Action::SetLastJoyHat(_))));
        c.last_joy_hat = 212;
        hat.joy_hat = 0;
        let released = collect(&translate_event(&c, &hat));
        assert_eq!(
            released[0],
            Action::Cause {
                key: 212,
                action: ist::RELEASE,
                delta: 0.0,
            }
        );
    }

    #[test]
    fn loss_while_grabbed_releases_then_releases_all() {
        let mut c = ctx();
        c.mouse_is_grabbed = true;
        let out = collect(&translate_event(
            &c,
            &window(sdl_window_event::FOCUS_LOST, 0),
        ));
        assert_eq!(
            out,
            vec![
                Action::UpdateMouseGrab { capture: false },
                Action::SetLostGrab(true),
                Action::ReleaseAll,
            ]
        );
    }

    #[test]
    fn loss_while_ungrabbed_only_releases_all() {
        let out = collect(&translate_event(
            &ctx(),
            &window(sdl_window_event::MINIMIZED, 0),
        ));
        assert_eq!(out, vec![Action::ReleaseAll]);
    }

    #[test]
    fn restore_completes_pending_grab_when_focused() {
        let mut c = ctx();
        c.lost_grab = true;
        c.capture_mouse = true;
        let out = collect(&translate_event(
            &c,
            &window(
                sdl_window_event::FOCUS_GAINED,
                sdl_window_flags::INPUT_FOCUS,
            ),
        ));
        assert_eq!(
            out,
            vec![
                Action::UpdateMouseGrab { capture: true },
                Action::SetLostGrab(false),
            ]
        );
    }

    #[test]
    fn restore_without_window_is_ignored() {
        let mut c = ctx();
        c.has_window = false;
        c.lost_grab = true;
        assert!(
            translate_event(
                &c,
                &window(sdl_window_event::SHOWN, sdl_window_flags::INPUT_FOCUS),
            )
            .is_empty()
        );
    }

    #[test]
    fn resize_applies_only_positive_changed_sizes() {
        let mut ev = window(sdl_window_event::RESIZED, 0);
        ev.window_data1 = 1024;
        ev.window_data2 = 768;
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![Action::ResizeViewport {
                width: 1024,
                height: 768,
            }]
        );
        ev.window_data1 = 800;
        ev.window_data2 = 600;
        assert!(translate_event(&ctx(), &ev).is_empty());
        ev.window_data1 = 0;
        ev.window_data2 = 600;
        assert!(translate_event(&ctx(), &ev).is_empty());
    }

    #[test]
    fn resize_matches_oracle_640x480_to_800x600() {
        // Oracle: `harness.cpp` scenario `resize` (viewport 640x480). A real size
        // emits `ResizeViewport` (`ResizeViewport vp=1 flags=32 x=800 y=600`);
        // a zero size emits nothing.
        let mut c = ctx();
        c.size_x = 640;
        c.size_y = 480;
        let mut ev = window(sdl_window_event::RESIZED, 0);
        ev.window_data1 = 800;
        ev.window_data2 = 600;
        assert_eq!(
            collect(&translate_event(&c, &ev)),
            vec![Action::ResizeViewport {
                width: 800,
                height: 600,
            }]
        );
        ev.window_data1 = 0;
        ev.window_data2 = 0;
        assert!(translate_event(&c, &ev).is_empty());
    }

    #[test]
    fn relative_mouse_matches_oracle_scales() {
        // Oracle: `harness.cpp` scenario `rel_mouse_grabbed` (scale 0.5/0.25,
        // dx=3, dy=-2) and `rel_mouse_window_scale` (scale 1.6 from 640/400
        // and 480/300; `MouseScaleX=3fcccccd`).
        let mut c = ctx();
        c.mouse_scale_x = 0.5;
        c.mouse_scale_y = 0.25;
        assert_eq!(
            collect(&relative_mouse(&c, 3, -2)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_X,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x3fc0_0000), // 1.5
                },
                Action::Cause {
                    key: ik::MOUSE_Y,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x3f00_0000), // 0.5
                },
            ]
        );
        c.mouse_scale_x = f32::from_bits(0x3fcc_cccd);
        c.mouse_scale_y = f32::from_bits(0x3fcc_cccd);
        assert_eq!(
            collect(&relative_mouse(&c, 3, -2)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_X,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x4099_999a), // 4.8
                },
                Action::Cause {
                    key: ik::MOUSE_Y,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x404c_cccd), // 3.2
                },
            ]
        );
        assert!(collect(&relative_mouse(&c, 0, 0)).is_empty());
    }

    /// Drives [`release_all_next`] like the C++ executor: answers each
    /// `QueryKeyDown` from `held`, and returns every step in order.
    fn drive_release_all(has_input: bool, held: &[i32]) -> Vec<ReleaseStep> {
        let mut cursor = release_all_begin(has_input);
        let mut steps = Vec::new();
        let mut answer = false;
        while let Some(step) = release_all_next(&mut cursor, answer) {
            answer = false;
            if let ReleaseStep::QueryKeyDown(key) = step {
                answer = held.contains(&key);
            }
            steps.push(step);
        }
        steps
    }

    /// Steps after the per-key loop: 8 joystick axes, the hat, then the
    /// three mouse axes. Matches the oracle's `release_all_empty` trace.
    fn joy_and_mouse_release_tail() -> Vec<ReleaseStep> {
        let mut tail = Vec::new();
        for key in [224, 225, 226, 227, 232, 233, 234, 235] {
            tail.push(ReleaseStep::Cause {
                key,
                action: ist::AXIS,
                delta: 0.0,
            });
        }
        tail.push(ReleaseStep::SetLastJoyHat(ik::NONE));
        for key in [228, 229, 231] {
            tail.push(ReleaseStep::Cause {
                key,
                action: ist::AXIS,
                delta: 0.0,
            });
        }
        tail
    }

    #[test]
    fn release_all_matches_oracle_with_held_keys() {
        // Oracle: `harness.cpp` scenario `reset_release` (IK_A=65, IK_Shift=16,
        // IK_Joy1=200 held; LastJoyHat=IK_Joy5; UpdateInput(1)). Trace order:
        // 16, 65, 200 RELEASE; then the joystick and mouse tail above.
        let steps = drive_release_all(true, &[16, 65, 200]);
        let queries = steps
            .iter()
            .filter(|s| matches!(s, ReleaseStep::QueryKeyDown(_)))
            .count();
        assert_eq!(queries as i32, ik::MAX);
        let actions: Vec<ReleaseStep> = steps
            .into_iter()
            .filter(|s| !matches!(s, ReleaseStep::QueryKeyDown(_)))
            .collect();
        let mut expected = vec![
            ReleaseStep::Cause {
                key: 16,
                action: ist::RELEASE,
                delta: 0.0,
            },
            ReleaseStep::Cause {
                key: 65,
                action: ist::RELEASE,
                delta: 0.0,
            },
            ReleaseStep::Cause {
                key: 200,
                action: ist::RELEASE,
                delta: 0.0,
            },
        ];
        expected.extend(joy_and_mouse_release_tail());
        assert_eq!(actions, expected);
    }

    #[test]
    fn release_all_empty_matches_oracle() {
        // Oracle: `harness.cpp` scenario `release_all_empty`. No key is held, so
        // only the joystick and mouse tail is emitted.
        let steps = drive_release_all(true, &[]);
        let actions: Vec<ReleaseStep> = steps
            .into_iter()
            .filter(|s| !matches!(s, ReleaseStep::QueryKeyDown(_)))
            .collect();
        assert_eq!(actions, joy_and_mouse_release_tail());
    }

    #[test]
    fn release_all_without_input_skips_keys() {
        let mut cursor = release_all_begin(false);
        let mut count = 0;
        while let Some(step) = release_all_next(&mut cursor, false) {
            assert!(!matches!(step, ReleaseStep::QueryKeyDown(_)));
            count += 1;
        }
        assert_eq!(
            count,
            MAX_JOYSTICK_AXES as usize + 1 + RELEASE_MOUSE_AXES.len()
        );
    }

    #[test]
    fn repaint_replaces_best_matches_tick_predicate() {
        let cand = |realtime, best_minus_candidate| RepaintCandidate {
            realtime,
            size_x: 4,
            size_y: 4,
            best_minus_candidate,
        };
        assert!(!repaint_replaces_best(false, cand(false, 0.0)));
        assert!(repaint_replaces_best(false, cand(true, 0.0)));
        assert!(!repaint_replaces_best(true, cand(true, 0.0)));
        assert!(repaint_replaces_best(true, cand(true, 3.0)));
        assert!(!repaint_replaces_best(true, cand(true, -3.0)));
        let zero_size = RepaintCandidate {
            realtime: true,
            size_x: 0,
            size_y: 4,
            best_minus_candidate: 1.0,
        };
        assert!(!repaint_replaces_best(false, zero_size));
    }

    /// Index of the viewport `Tick` repaints, given candidates in array order.
    /// Mirrors the fold in `USDLClient::Tick` (710119c `SDLClient.cpp`).
    fn tick_victim(candidates: &[RepaintCandidate]) -> Option<usize> {
        let mut best = None;
        for (index, candidate) in candidates.iter().enumerate() {
            if repaint_replaces_best(best.is_some(), *candidate) {
                best = Some(index);
            }
        }
        best
    }

    #[test]
    fn tick_victim_matches_oracle() {
        // Oracle: `/tmp/hp2-m2/input/harness.cpp` scenarios `tick_victim`,
        // `tick_tie`, `tick_subfloat_order`. Build and run:
        //   clang++ -std=c++17 -w -I shim harness.cpp -o harness && ./harness
        // `best_minus_candidate` is the sign of (best - candidate) that the
        // C++ fold computes with `FTime::operator-`.
        let cand = |realtime: bool, size_x: i32, size_y: i32, best_minus_candidate: f32| {
            RepaintCandidate {
                realtime,
                size_x,
                size_y,
                best_minus_candidate,
            }
        };
        // a t=5.0 (realtime, 4x4), b t=2.0, c t=2.0, d t=0.5 (not realtime),
        // e t=0.1 (SizeX=0). Oracle repaints Id 2 (b).
        let victim = [
            cand(true, 4, 4, 0.0),
            cand(true, 4, 4, 3.0),
            cand(true, 4, 4, 0.0),
            cand(false, 4, 4, 1.5),
            cand(true, 0, 4, 1.9),
        ];
        assert_eq!(tick_victim(&victim), Some(1));
        // a t=3.0, b t=3.0: a tie keeps the earlier viewport (Id 1).
        let tie = [cand(true, 4, 4, 0.0), cand(true, 4, 4, 0.0)];
        assert_eq!(tick_victim(&tie), Some(0));
        // a = 1000.0 + 1e-7, b = 1000.0: a - b is positive (bits 0x33d68000),
        // so b is the stale one (Id 2).
        let subfloat = [
            cand(true, 4, 4, 0.0),
            cand(true, 4, 4, f32::from_bits(0x33d6_8000)),
        ];
        assert_eq!(tick_victim(&subfloat), Some(1));
    }
}
