use hp2_engine::binding_foundation::{Capture, CaptureResult::*};

#[test]
fn forwarded_movement_release_passes_while_polling() {
    let mut capture = Capture::default();
    capture.begin([87], [1]); // W reached gameplay; the opening click did not.
    assert_eq!(capture.event(87, true, false), Consume); // repeat, not a binding
    assert_eq!(capture.event(87, false, false), Pass); // gameplay clears movement
    assert_eq!(capture.event(1, false, false), Consume);
    assert_eq!(capture.event(65, true, false), Captured(65));
    assert_eq!(capture.event(65, false, false), Consume);
}

#[test]
fn forwarded_release_passes_after_cancel_and_success() {
    for cancel in [false, true] {
        let mut capture = Capture::default();
        capture.begin([87], [1]);
        assert_eq!(capture.event(27, true, cancel), if cancel { Cancelled } else { Captured(27) });
        assert_eq!(capture.event(87, true, false), Consume);
        assert_eq!(capture.event(87, false, false), Pass);
        assert_eq!(capture.event(27, false, false), Consume);
        assert_eq!(capture.event(1, false, false), Consume);
    }
}

#[test]
fn explicit_cancel_and_restart_retain_release_obligations() {
    let mut capture = Capture::default();
    capture.begin([87], [1]);
    capture.cancel();
    capture.begin([], []);
    assert_eq!(capture.event(87, false, false), Pass);
    assert_eq!(capture.event(1, false, false), Consume);
    assert_eq!(capture.event(2, true, false), Captured(2));
    assert_eq!(capture.event(2, false, false), Consume);
}

#[test]
#[should_panic(expected = "capture key cannot be both forwarded and suppressed")]
fn rejects_ambiguous_press_provenance() {
    Capture::default().begin([1], [1]);
}
