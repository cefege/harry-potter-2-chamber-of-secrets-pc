//! Mouse capture callback order, error, and reentry contracts.

use super::set_mouse_capture_with;
use crate::input_pump::sdl_window_flags::INPUT_FOCUS;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Lost(bool),
    Flags,
    Grab(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Lost,
    Flags,
    Grab,
}

fn exercise(
    capture: i32,
    focus_only: i32,
    flags: Option<u32>,
    fail: Option<Failure>,
) -> (Result<(), Failure>, Vec<Event>) {
    let events = RefCell::new(Vec::new());
    let result = set_mouse_capture_with(
        capture,
        focus_only,
        |lost| {
            events.borrow_mut().push(Event::Lost(lost));
            if fail == Some(Failure::Lost) {
                Err(Failure::Lost)
            } else {
                Ok(())
            }
        },
        || {
            events.borrow_mut().push(Event::Flags);
            if fail == Some(Failure::Flags) {
                Err(Failure::Flags)
            } else {
                Ok(flags)
            }
        },
        |value| {
            events.borrow_mut().push(Event::Grab(value));
            if fail == Some(Failure::Grab) {
                Err(Failure::Grab)
            } else {
                Ok(())
            }
        },
    );
    (result, events.into_inner())
}

#[test]
fn capture_matrix_keeps_arguments_and_callback_order() {
    let cases: &[(i32, i32, Option<u32>, &[Event])] = &[
        (0, 0, None, &[Event::Lost(false), Event::Grab(0)]),
        (0, -7, Some(0), &[Event::Lost(false), Event::Grab(0)]),
        (1, 0, Some(0), &[Event::Grab(1)]),
        (-9, 0, Some(0), &[Event::Grab(-9)]),
        (7, 1, None, &[Event::Flags, Event::Grab(7)]),
        (-9, -2, Some(INPUT_FOCUS), &[Event::Flags, Event::Grab(-9)]),
        (
            7,
            1,
            Some(INPUT_FOCUS | 0x4000_0000),
            &[Event::Flags, Event::Grab(7)],
        ),
        (
            7,
            1,
            Some(0),
            &[Event::Flags, Event::Lost(true), Event::Grab(0)],
        ),
        (
            -9,
            -2,
            Some(0x4000_0000),
            &[Event::Flags, Event::Lost(true), Event::Grab(0)],
        ),
    ];
    for &(capture, focus_only, flags, expected) in cases {
        let (result, events) = exercise(capture, focus_only, flags, None);
        assert_eq!(result, Ok(()));
        assert_eq!(
            events.as_slice(),
            expected,
            "capture={capture}, focus_only={focus_only}, flags={flags:?}"
        );
    }
}

#[test]
fn capture_errors_stop_at_the_failing_callback() {
    let cases: &[(i32, i32, Option<u32>, Failure, &[Event])] = &[
        (0, 1, Some(0), Failure::Lost, &[Event::Lost(false)]),
        (1, 1, Some(0), Failure::Flags, &[Event::Flags]),
        (
            1,
            1,
            Some(0),
            Failure::Lost,
            &[Event::Flags, Event::Lost(true)],
        ),
        (
            0,
            1,
            Some(0),
            Failure::Grab,
            &[Event::Lost(false), Event::Grab(0)],
        ),
        (-9, 0, Some(0), Failure::Grab, &[Event::Grab(-9)]),
        (1, 1, None, Failure::Grab, &[Event::Flags, Event::Grab(1)]),
        (
            1,
            1,
            Some(0),
            Failure::Grab,
            &[Event::Flags, Event::Lost(true), Event::Grab(0)],
        ),
    ];
    for &(capture, focus_only, flags, failure, expected) in cases {
        let (result, events) = exercise(capture, focus_only, flags, Some(failure));
        assert_eq!(result, Err(failure));
        assert_eq!(events.as_slice(), expected);
    }
    // Suppressed callbacks cannot fail or run.
    assert_eq!(
        exercise(0, 1, Some(0), Some(Failure::Flags)),
        (Ok(()), vec![Event::Lost(false), Event::Grab(0)])
    );
    assert_eq!(
        exercise(-9, 0, Some(0), Some(Failure::Lost)),
        (Ok(()), vec![Event::Grab(-9)])
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NestedEvent {
    OuterFlags,
    InnerLost(bool),
    InnerGrab(i32),
    InnerDone,
    OuterLost(bool),
    OuterGrab(i32),
}

#[test]
fn capture_reentry_uses_independent_frame_and_resumes_in_order() {
    let events = RefCell::new(Vec::new());
    let outer_lost = RefCell::new(None);
    let inner_lost = RefCell::new(None);
    let inner_grab = RefCell::new(None);
    let outer_grab = RefCell::new(None);

    let outer_result = set_mouse_capture_with(
        -9,
        1,
        |lost| {
            events.borrow_mut().push(NestedEvent::OuterLost(lost));
            *outer_lost.borrow_mut() = Some(lost);
            Ok::<(), ()>(())
        },
        || {
            events.borrow_mut().push(NestedEvent::OuterFlags);
            // No RefCell borrow survives the nested call. The inner invocation
            // has separate owner state and must not overwrite the outer frame.
            let inner_result = set_mouse_capture_with(
                0,
                1,
                |lost| {
                    events.borrow_mut().push(NestedEvent::InnerLost(lost));
                    *inner_lost.borrow_mut() = Some(lost);
                    Ok::<(), ()>(())
                },
                || -> Result<Option<u32>, ()> { panic!("inner flags must not run") },
                |value| {
                    events.borrow_mut().push(NestedEvent::InnerGrab(value));
                    *inner_grab.borrow_mut() = Some(value);
                    Ok::<(), ()>(())
                },
            );
            assert_eq!(inner_result, Ok(()));
            events.borrow_mut().push(NestedEvent::InnerDone);
            Ok(Some(0))
        },
        |value| {
            events.borrow_mut().push(NestedEvent::OuterGrab(value));
            *outer_grab.borrow_mut() = Some(value);
            Ok::<(), ()>(())
        },
    );

    assert_eq!(outer_result, Ok(()));
    assert_eq!(
        events.into_inner(),
        [
            NestedEvent::OuterFlags,
            NestedEvent::InnerLost(false),
            NestedEvent::InnerGrab(0),
            NestedEvent::InnerDone,
            NestedEvent::OuterLost(true),
            NestedEvent::OuterGrab(0),
        ]
    );
    assert_eq!(inner_lost.into_inner(), Some(false));
    assert_eq!(inner_grab.into_inner(), Some(0));
    assert_eq!(outer_lost.into_inner(), Some(true));
    assert_eq!(outer_grab.into_inner(), Some(0));
}

#[test]
fn capture_reentrant_error_stops_outer_before_lost_or_grab() {
    let events = RefCell::new(Vec::new());
    let result = set_mouse_capture_with(
        5,
        1,
        |_| {
            events.borrow_mut().push("outer lost");
            Ok(())
        },
        || {
            events.borrow_mut().push("outer flags");
            set_mouse_capture_with(
                0,
                0,
                |_| {
                    events.borrow_mut().push("inner lost");
                    Err(Failure::Lost)
                },
                || {
                    events.borrow_mut().push("inner flags");
                    Ok(None)
                },
                |_| {
                    events.borrow_mut().push("inner grab");
                    Ok(())
                },
            )?;
            Ok(Some(0))
        },
        |_| {
            events.borrow_mut().push("outer grab");
            Ok(())
        },
    );
    assert_eq!(result, Err(Failure::Lost));
    assert_eq!(events.into_inner(), ["outer flags", "inner lost"]);
}
