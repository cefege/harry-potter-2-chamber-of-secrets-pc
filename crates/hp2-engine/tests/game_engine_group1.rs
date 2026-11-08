//! Replays the `UGameEngine` group 1 oracle (exit, destroy, cancel_pending,
//! set_progress, challenge_response, mouse_delta, mouse_position, key,
//! input_event, paint_progress) through `hp2_engine::game_engine`.
//!
//! `tests/fixtures/game_group1.txt` is produced by
//! `Build/game_fixture_harness/gen_game.sh` from the unmodified `UnGame.cpp`.
//! This test drives the same 68 cases in the same order against the mock
//! services in `tests/game_engine/mock.rs`, which print the same trace lines.

#[path = "game_engine/mock.rs"]
mod mock;

use hp2_engine::game_engine as ge;
use mock::*;

const EXPECTED: &str = include_str!("fixtures/game_group1.txt");
const EXPECTED_CASES: usize = 68;

const IK_PAUSE: i32 = 19;
const IK_ESCAPE: i32 = 27;
const IK_DELETE: i32 = 46;
const IK_SCROLL_LOCK: i32 = 145;
const IK_BACKSLASH: i32 = 220;
const IST_PRESS: i32 = 1;
const IST_RELEASE: i32 = 3;
const MOUSE_FIRST_HIT: u32 = 0x0008;
const MOUSE_LAST_RELEASE: u32 = 0x0010;

/// Harness `set_replay(recording, playing)` with the default trailing arguments.
fn rp(recording: bool, playing: bool) {
    set_replay(recording, playing, false, false, 0);
}

fn run() {
    let s = services();
    let e = engine();
    let vp0 = ptr(VP0);
    let null = ptr(0);

    // SAFETY: every service is a mock that never dereferences its arguments.
    unsafe {
        // exit
        begin("exit netdriver=1");
        world_base(true);
        model(|m| m.level_net_driver = NETDRV);
        ge::exit(&s, e).expect("checked native operation");
        dump_state();

        begin("exit netdriver=0");
        world_base(true);
        ge::exit(&s, e).expect("checked native operation");
        dump_state();

        // destroy
        begin("destroy pending=0");
        world_base(true);
        ge::destroy(&s, e).expect("checked native operation");
        dump_state();

        begin("destroy pending=1 netdriver=1 conn=1 chan=1");
        world_base(true);
        set_pending(true, true, true);
        ge::destroy(&s, e).expect("checked native operation");
        dump_state();

        begin("destroy pending=1 netdriver=1 conn=1 chan=0");
        world_base(true);
        set_pending(true, true, false);
        ge::destroy(&s, e).expect("checked native operation");
        dump_state();

        begin("destroy pending=1 netdriver=1 conn=0 chan=0");
        world_base(true);
        set_pending(true, false, false);
        ge::destroy(&s, e).expect("checked native operation");
        dump_state();

        begin("destroy pending=1 netdriver=0");
        world_base(true);
        set_pending(false, false, false);
        ge::destroy(&s, e).expect("checked native operation");
        dump_state();

        // cancel_pending
        begin("cancel_pending pending=0");
        world_base(true);
        ge::cancel_pending(&s, e).expect("checked native operation");
        dump_state();

        begin("cancel_pending pending=1 netdriver=1 conn=1 chan=1");
        world_base(true);
        set_pending(true, true, true);
        ge::cancel_pending(&s, e).expect("checked native operation");
        dump_state();

        begin("cancel_pending pending=1 netdriver=1 conn=1 chan=0");
        world_base(true);
        set_pending(true, true, false);
        ge::cancel_pending(&s, e).expect("checked native operation");
        dump_state();

        begin("cancel_pending pending=1 netdriver=0");
        world_base(true);
        set_pending(false, false, false);
        ge::cancel_pending(&s, e).expect("checked native operation");
        dump_state();

        // set_progress
        let hello = tcz("Hello");
        let world = tcz("World");
        let empty = tcz("");
        let x = tcz("x");

        begin("set_progress client=0");
        world_base(true);
        model(|m| m.client = 0);
        ge::set_progress(&s, e, hello.as_ptr(), world.as_ptr(), 2.5)
            .expect("checked native operation");
        dump_state();

        begin("set_progress viewports=0");
        world_base(false);
        ge::set_progress(&s, e, hello.as_ptr(), world.as_ptr(), 2.5)
            .expect("checked native operation");
        dump_state();

        begin("set_progress seconds=2.5");
        world_base(true);
        ge::set_progress(&s, e, hello.as_ptr(), world.as_ptr(), 2.5)
            .expect("checked native operation");
        dump_state();

        begin("set_progress seconds=-1 upgrade=1");
        world_base(true);
        ge::set_progress(&s, e, hello.as_ptr(), world.as_ptr(), -1.0)
            .expect("checked native operation");
        dump_state();

        begin("set_progress seconds=0.1");
        world_base(true);
        ge::set_progress(&s, e, hello.as_ptr(), world.as_ptr(), 0.1)
            .expect("checked native operation");
        dump_state();

        begin("set_progress str2=empty");
        world_base(true);
        ge::set_progress(&s, e, hello.as_ptr(), empty.as_ptr(), 2.5)
            .expect("checked native operation");
        dump_state();

        begin("set_progress old=1 str1=empty");
        world_base(true);
        model(|m| m.pawn.msg[0] = tc("old"));
        ge::set_progress(&s, e, empty.as_ptr(), x.as_ptr(), 0.5).expect("checked native operation");
        dump_state();

        // challenge_response: inputs chosen so the oracle has no signed overflow.
        // No world reset: the harness leaves the previous case's state in place.
        for c in [0, 1, 2, 100, 12345, 32767, -1, -7, -32768] {
            begin(&format!("challenge={c}"));
            result(ge::challenge_response(c));
            dump_state();
        }

        // mouse_delta

        begin("mouse_delta flags=FirstHit");
        world_base(true);
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit viewports=2");
        world_base(true);
        model(|m| {
            m.client_viewports.push(VP1);
            m.vp1.actor = PAWN;
        });
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit pauser=1");
        world_base(true);
        model(|m| m.pauser = tc("Paused"));
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit showmenu=1");
        world_base(true);
        model(|m| m.pawn.show_menu = true);
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit windowsmouse=1");
        world_base(true);
        model(|m| m.vp0.show_windows_mouse = true);
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=LastRelease capture=0");
        world_base(true);
        ge::mouse_delta(&s, e, vp0, MOUSE_LAST_RELEASE).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=LastRelease capture=1");
        world_base(true);
        model(|m| m.client_capture_mouse = true);
        ge::mouse_delta(&s, e, vp0, MOUSE_LAST_RELEASE).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit|LastRelease");
        world_base(true);
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT | MOUSE_LAST_RELEASE)
            .expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit client=0");
        world_base(true);
        model(|m| m.client = 0);
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit level=0");
        world_base(true);
        model(|m| m.g_level = 0);
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=FirstHit viewports=0");
        world_base(false);
        ge::mouse_delta(&s, e, vp0, MOUSE_FIRST_HIT).expect("checked native operation");
        dump_state();

        begin("mouse_delta flags=0");
        world_base(true);
        ge::mouse_delta(&s, e, vp0, 0).expect("checked native operation");
        dump_state();

        // mouse_position
        begin("mouse_position viewport=null recording=0");
        world_base(true);
        ge::mouse_position(&s, e, null, 10.5, -3.25).expect("checked native operation");
        dump_state();

        begin("mouse_position viewport=1 recording=0");
        world_base(true);
        ge::mouse_position(&s, e, vp0, 10.5, -3.25).expect("checked native operation");
        dump_state();

        begin("mouse_position viewport=1 recording=1");
        world_base(true);
        rp(true, false);
        ge::mouse_position(&s, e, vp0, 10.5, -3.25).expect("checked native operation");
        dump_state();

        begin("mouse_position viewport=1 replaying=1");
        world_base(true);
        rp(false, true);
        ge::mouse_position(&s, e, vp0, 10.5, -3.25).expect("checked native operation");
        dump_state();

        begin("mouse_position viewport=null recording=1");
        world_base(true);
        rp(true, false);
        ge::mouse_position(&s, e, null, 10.5, -3.25).expect("checked native operation");
        dump_state();

        // key
        begin("key base=0 recording=0");
        world_base(true);
        set_key_ret(0);
        result(ge::key(&s, e, vp0, IK_ESCAPE).expect("checked native operation"));
        dump_state();

        begin("key base=1 recording=0");
        world_base(true);
        set_key_ret(1);
        result(ge::key(&s, e, vp0, IK_ESCAPE).expect("checked native operation"));
        dump_state();

        begin("key base=1 recording=1");
        world_base(true);
        rp(true, false);
        set_key_ret(1);
        result(ge::key(&s, e, vp0, IK_ESCAPE).expect("checked native operation"));
        dump_state();

        begin("key base=1 replaying=1");
        world_base(true);
        rp(false, true);
        set_key_ret(1);
        result(ge::key(&s, e, vp0, IK_ESCAPE).expect("checked native operation"));
        dump_state();

        begin("key base=0 recording=1");
        world_base(true);
        rp(true, false);
        set_key_ret(0);
        result(ge::key(&s, e, vp0, IK_ESCAPE).expect("checked native operation"));
        dump_state();

        begin("key viewport=null base=1");
        world_base(true);
        set_key_ret(1);
        result(ge::key(&s, e, null, IK_DELETE).expect("checked native operation"));
        dump_state();

        // input_event (delta 0.5 unless noted)
        begin("input_event key=46 state=1 special=0");
        world_base(true);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_DELETE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=46 state=3 special=1");
        world_base(true);
        model(|m| m.level_special_pause = true);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_DELETE, IST_RELEASE, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=46 state=1 special=1");
        world_base(true);
        model(|m| m.level_special_pause = true);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_DELETE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=27 state=1 replaying=1 inreplay=0");
        world_base(true);
        rp(false, true);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_ESCAPE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=19 state=1 replaying=1 paused=0");
        world_base(true);
        rp(false, true);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_PAUSE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=19 state=1 replaying=1 paused=1");
        world_base(true);
        set_replay(false, true, false, true, 0);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_PAUSE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=145 state=1 replaying=1 paused=1 count=0");
        world_base(true);
        set_replay(false, true, false, true, 0);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_SCROLL_LOCK, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=145 state=1 replaying=1 paused=0");
        world_base(true);
        rp(false, true);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_SCROLL_LOCK, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=65 state=1 replaying=1");
        world_base(true);
        rp(false, true);
        set_ie_ret(1);
        result(ge::input_event(&s, e, vp0, 65, IST_PRESS, 0.5).expect("checked native operation"));
        dump_state();

        begin("input_event key=27 state=1 replaying=1 inreplay=1");
        world_base(true);
        set_replay(false, true, true, false, 0);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_ESCAPE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=220 state=1 recording=1");
        world_base(true);
        rp(true, false);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_BACKSLASH, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=220 state=3 recording=1");
        world_base(true);
        rp(true, false);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_BACKSLASH, IST_RELEASE, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=65 state=1 recording=1");
        world_base(true);
        rp(true, false);
        set_ie_ret(1);
        result(ge::input_event(&s, e, vp0, 65, IST_PRESS, 0.5).expect("checked native operation"));
        dump_state();

        begin("input_event key=46 state=1 replaying=1 special=0");
        world_base(true);
        rp(false, true);
        set_ie_ret(1);
        result(
            ge::input_event(&s, e, vp0, IK_DELETE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        begin("input_event key=19 state=1 neither base=0");
        world_base(true);
        set_ie_ret(0);
        result(
            ge::input_event(&s, e, vp0, IK_PAUSE, IST_PRESS, 0.5)
                .expect("checked native operation"),
        );
        dump_state();

        // paint_progress
        begin("paint_progress drawing=1");
        world_base(true);
        model(|m| m.is_drawing = true);
        ge::paint_progress(&s, e).expect("checked native operation");
        dump_state();

        begin("paint_progress drawing=0 fog=default");
        world_base(true);
        ge::paint_progress(&s, e).expect("checked native operation");
        dump_state();

        begin("paint_progress drawing=0 fog=zero");
        world_base(true);
        model(|m| m.pawn.flash_fog = [0.0; 4]);
        ge::paint_progress(&s, e).expect("checked native operation");
        dump_state();
    }
}

#[test]
fn game_group1_replay_matches_oracle() {
    run();
    let out = take_output();

    let cases = out
        .lines()
        .filter(|l| l.starts_with("case group1 "))
        .count();
    assert_eq!(cases, EXPECTED_CASES, "group1 case count");

    if out != EXPECTED {
        let (n, got, want) = out
            .lines()
            .zip(EXPECTED.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i + 1, a, b))
            .unwrap_or((0, "<length mismatch>", "<length mismatch>"));
        panic!(
            "group1 replay diverged at line {n}\n  got:  {got}\n  want: {want}\n  got lines {} / want lines {}",
            out.lines().count(),
            EXPECTED.lines().count()
        );
    }
}
