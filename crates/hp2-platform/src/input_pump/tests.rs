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
    let cand =
        |realtime: bool, size_x: i32, size_y: i32, best_minus_candidate: f32| RepaintCandidate {
            realtime,
            size_x,
            size_y,
            best_minus_candidate,
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

#[test]
fn verification_drops_every_physical_gameplay_producer() {
    let mut c = ctx();
    c.joy_hat_folds_to_buttons = true;
    c.last_joy_hat = ik::NONE;

    let mut key_down = event(sdl_event::KEYDOWN);
    key_down.key_sym = b'a' as i32;
    let mut key_up = event(sdl_event::KEYUP);
    key_up.key_sym = b'a' as i32;
    let mut text = event(sdl_event::TEXTINPUT);
    text.text[0] = b'a';
    let mut motion = event(sdl_event::MOUSEMOTION);
    motion.motion_x = 10;
    motion.motion_y = 20;
    motion.motion_state = 4;
    let mut button_down = event(sdl_event::MOUSEBUTTONDOWN);
    button_down.button = 1;
    let mut button_up = event(sdl_event::MOUSEBUTTONUP);
    button_up.button = 1;
    let mut wheel = event(sdl_event::MOUSEWHEEL);
    wheel.wheel_y = 2;
    let mut axis = event(sdl_event::JOYAXISMOTION);
    axis.joy_axis = 0;
    axis.joy_axis_value = 32767;
    let mut joy_button = event(sdl_event::JOYBUTTONDOWN);
    joy_button.joy_button = 0;
    let mut hat = event(sdl_event::JOYHATMOTION);
    hat.joy_hat = 1;

    let physical = [
        key_down,
        key_up,
        text,
        motion,
        button_down,
        button_up,
        wheel,
        axis,
        joy_button,
        hat,
    ];
    for ev in physical {
        assert!(
            !translate_event_with_verification(&c, &ev, false).is_empty(),
            "production translation must emit actions for event kind {:#x}",
            ev.kind
        );
        assert!(
            translate_event_with_verification(&c, &ev, true).is_empty(),
            "verification must drop physical event kind {:#x}",
            ev.kind
        );
    }
}

#[test]
fn verification_keeps_resize_and_suppresses_focus_transitions() {
    // Resize is real window lifecycle, not hardware input: it still runs.
    let mut resize = window(sdl_window_event::RESIZED, 0);
    resize.window_data1 = 1024;
    resize.window_data2 = 768;
    assert_eq!(
        collect(&translate_event_with_verification(&ctx(), &resize, true)),
        vec![Action::ResizeViewport {
            width: 1024,
            height: 768,
        }]
    );

    // Focus loss normally drops the grab and releases every held key,
    // including keys injected by a controlled replay.
    let mut grabbed = ctx();
    grabbed.mouse_is_grabbed = true;
    grabbed.capture_mouse = true;
    let lost = window(sdl_window_event::FOCUS_LOST, 0);
    assert_eq!(
        collect(&translate_event_with_verification(&grabbed, &lost, false)),
        vec![
            Action::UpdateMouseGrab { capture: false },
            Action::SetLostGrab(true),
            Action::ReleaseAll,
        ]
    );
    assert!(translate_event_with_verification(&grabbed, &lost, true).is_empty());
    for kind in [sdl_window_event::MINIMIZED, sdl_window_event::HIDDEN] {
        assert!(translate_event_with_verification(&grabbed, &window(kind, 0), true).is_empty());
    }

    // Focus regain normally re-enters OS mouse capture.
    let mut pending = ctx();
    pending.lost_grab = true;
    pending.capture_mouse = true;
    let gained = window(
        sdl_window_event::FOCUS_GAINED,
        sdl_window_flags::INPUT_FOCUS,
    );
    assert_eq!(
        collect(&translate_event_with_verification(&pending, &gained, false)),
        vec![
            Action::UpdateMouseGrab { capture: true },
            Action::SetLostGrab(false),
        ]
    );
    assert!(translate_event_with_verification(&pending, &gained, true).is_empty());
}

#[test]
fn constants_for_controller_events_match_pinned_header_values() {
    assert_eq!(sdl_event::CONTROLLERAXISMOTION, 0x650);
    assert_eq!(sdl_event::CONTROLLERBUTTONDOWN, 0x651);
    assert_eq!(sdl_event::CONTROLLERBUTTONUP, 0x652);
    assert_eq!(sdl_event::CONTROLLERDEVICEADDED, 0x653);
    assert_eq!(sdl_event::CONTROLLERDEVICEREMOVED, 0x654);
}

#[test]
fn mouse_sensitivity_multiplies_each_axis_independently() {
    let mut c = ctx();
    c.mouse_scale_x = 2.0;
    c.mouse_scale_y = 0.5;
    // Unit multipliers are bit-identical to the stock path.
    assert_eq!(
        collect(&relative_mouse_scaled(&c, 3, -2, 1.0, 1.0)),
        vec![
            Action::Cause { key: ik::MOUSE_X, action: ist::AXIS, delta: 6.0 },
            Action::Cause { key: ik::MOUSE_Y, action: ist::AXIS, delta: 1.0 },
        ]
    );
    assert_eq!(
        collect(&relative_mouse_scaled(&c, 3, -2, 2.0, 0.5)),
        vec![
            Action::Cause { key: ik::MOUSE_X, action: ist::AXIS, delta: 12.0 },
            Action::Cause { key: ik::MOUSE_Y, action: ist::AXIS, delta: 0.5 },
        ]
    );
}

#[test]
fn controller_events_map_to_joystick_keys_and_verification_drops_them() {
    let mut c = ctx();
    c.joystick_dead_zone = 0.0;
    let stick = SdlEvent {
        joy_axis: crate::game_controller::axis::LEFT_X as u8,
        joy_axis_value: 16384,
        ..event(sdl_event::CONTROLLERAXISMOTION)
    };
    assert_eq!(
        collect(&translate_event_with_verification(&c, &stick, false)),
        vec![Action::Cause { key: ik::JOY_X, action: ist::AXIS, delta: 0.5 }]
    );
    let jump = SdlEvent {
        joy_button: crate::game_controller::button::A as u8,
        ..event(sdl_event::CONTROLLERBUTTONDOWN)
    };
    assert_eq!(
        collect(&translate_event_with_verification(&c, &jump, false)),
        vec![Action::Cause { key: ik::JOY1 + 1, action: ist::PRESS, delta: 0.0 }]
    );
    let release = SdlEvent { kind: sdl_event::CONTROLLERBUTTONUP, ..jump };
    assert_eq!(
        collect(&translate_event_with_verification(&c, &release, false)),
        vec![Action::Cause { key: ik::JOY1 + 1, action: ist::RELEASE, delta: 0.0 }]
    );
    for ev in [stick, jump, release] {
        assert!(collect(&translate_event_with_verification(&c, &ev, true)).is_empty());
    }
    // Device events never become input.
    assert!(collect(&translate_event_with_verification(&c, &event(sdl_event::CONTROLLERDEVICEADDED), false)).is_empty());
}

#[test]
fn controller_pointer_deadzone_motion_clamp_and_mouse_sync() {
    let c = ctx();
    let mut pointer = MenuPointer::default();
    let mut axis = event(sdl_event::CONTROLLERAXISMOTION);
    axis.joy_axis = game_controller::axis::LEFT_X as u8;
    axis.joy_axis_value = 5000;
    let actions = translate_event_inner(&c, &axis, false, Some(&mut pointer), true);
    assert_eq!(actions.len(), 1, "the joystick axis must still be forwarded");
    assert!(pointer.step(&c, 1.0).is_none(), "small stick movements are ignored");
    axis.joy_axis_value = 32767;
    translate_event_inner(&c, &axis, false, Some(&mut pointer), true);
    assert_eq!(pointer.step(&c, 1.0), Some(Action::EngineMousePosition { buttons: 0, x: 799.0, y: 300.0 }));
    let mut motion = event(sdl_event::MOUSEMOTION);
    motion.motion_x = 100;
    motion.motion_y = 80;
    translate_event_inner(&c, &motion, false, Some(&mut pointer), true);
    let Some(Action::EngineMousePosition { x, y, .. }) = pointer.step(&c, 0.1) else { panic!("expected pointer movement") };
    assert!((x - 196.0).abs() < 0.001 && y == 80.0);
    axis.joy_axis_value = 0;
    translate_event_inner(&c, &axis, false, Some(&mut pointer), true);
    let mut right = event(sdl_event::CONTROLLERBUTTONDOWN);
    right.joy_button = game_controller::button::DPAD_RIGHT as u8;
    assert_eq!(collect(&translate_event_inner(&c, &right, false, Some(&mut pointer), true)),
        vec![Action::Cause { key: game_controller::JOY_POV_RIGHT, action: ist::PRESS, delta: 0.0 }]);
    let Some(Action::EngineMousePosition { x, y, .. }) = pointer.step(&c, 0.1) else { panic!("expected D-pad movement") };
    assert!((x - 232.0).abs() < 0.001 && y == 80.0);
}

#[test]
fn controller_menu_buttons_pair_across_mode_changes() {
    let c = ctx();
    let mut pointer = MenuPointer::default();
    for (button, menu_key, game_key) in [
        (game_controller::button::A, ik::LEFT_MOUSE, ik::JOY1 + 1),
        (game_controller::button::B, ik::ESCAPE, ik::JOY1 + 2),
    ] {
        let mut down = event(sdl_event::CONTROLLERBUTTONDOWN);
        down.joy_button = button as u8;
        let up = SdlEvent { kind: sdl_event::CONTROLLERBUTTONUP, ..down };
        let actions = collect(&translate_event_inner(&c, &down, false, Some(&mut pointer), true));
        assert_eq!(actions.last(), Some(&Action::Cause { key: menu_key, action: ist::PRESS, delta: 0.0 }));
        assert_eq!(collect(&translate_event_inner(&c, &up, false, Some(&mut pointer), false)),
            vec![Action::Cause { key: menu_key, action: ist::RELEASE, delta: 0.0 }]);
        assert_eq!(collect(&translate_event_inner(&c, &down, false, Some(&mut pointer), false)),
            vec![Action::Cause { key: game_key, action: ist::PRESS, delta: 0.0 }]);
        assert_eq!(collect(&translate_event_inner(&c, &up, false, Some(&mut pointer), true)),
            vec![Action::Cause { key: game_key, action: ist::RELEASE, delta: 0.0 }]);
        assert!(translate_event_inner(&c, &down, true, Some(&mut pointer), true).is_empty());
    }
}

#[test]
fn controller_labels_replace_only_complete_key_tokens() {
    assert_eq!(game_controller::button_labels("Joy1 or Joy10 / JoyPovUp"),
        Some("X or R3 / D-pad Up".to_owned()));
    assert_eq!(game_controller::button_labels("Joy8 Joy100 xJoy1 JoyPovUp_suffix"), None);
    assert_eq!(game_controller::button_labels("Joy2, Joy3; Joy11 Joy12"),
        Some("A, B; LT RT".to_owned()));
}
