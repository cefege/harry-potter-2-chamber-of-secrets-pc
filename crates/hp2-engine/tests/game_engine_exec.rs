//! Record-level replay of `UGameEngine::Exec` (UnGame.cpp 445–675 at oracle 7ddb1d3).
//!
//! Drives `hp2_engine::game_exec::exec` through every case of the `exec` family in
//! `Build/game_fixture_harness/game_harness.cpp` (`run_exec`) and compares the full
//! record sequence against `fixtures/game_exec.txt`.

#[path = "game_engine/exec_mock.rs"]
mod exec_mock;
#[allow(dead_code)]
#[path = "game_engine/mock.rs"]
mod mock;

use exec_mock::{BASE, Wiring, knob, knobs_reset, live_errors, services, wire};
use hp2_engine::game_exec;
use mock::*;

const EXPECTED: &str = include_str!("fixtures/game_exec.txt");
const EXPECTED_CASES: usize = 51;

fn case(label: &str, cmd: &str, w: Wiring) {
    wire(w);
    begin_family("exec", label);
    let s = services();
    let cmd = tcz(cmd);
    // SAFETY: `cmd` is NUL-terminated and outlives the call; every service validates
    // the handles it receives, and `ar` is an opaque sink address.
    let r = unsafe { game_exec::exec(&s, engine(), cmd.as_ptr(), ptr(exec_mock::SINK)) };
    result(r.expect("checked native success"));
    dump_state();
    assert_eq!(live_errors(), 0, "{label}: FString error handle leaked");
}

fn run() {
    // OPEN / START
    knobs_reset();
    case("open vp=1", "OPEN Foo.unr?x=1", Wiring { vp: true, ..BASE });
    knobs_reset();
    knob(|k| k.browse_ret = 1);
    case("open vp=0 browse=1", "OPEN Foo.unr", BASE);
    knobs_reset();
    knob(|k| k.browse_err = Some(tc("Bad")));
    case("open vp=0 browse=0 err=Bad", "OPEN Foo.unr", BASE);
    knobs_reset();
    knob(|k| k.browse_err = Some(Vec::new()));
    case("open vp=0 browse=0 err=empty", "OPEN Foo.unr", BASE);
    knobs_reset();
    case("start vp=1", "START Foo.unr", Wiring { vp: true, ..BASE });
    knobs_reset();
    knob(|k| k.browse_err = Some(tc("Bad")));
    case("start vp=0 browse=0 err=Bad", "START Foo.unr", BASE);
    // SERVERTRAVEL / SAY (server-only gates; a consumed token falls through the else-if chain)
    knobs_reset();
    case(
        "servertravel srv=1 cli=0",
        "SERVERTRAVEL Foo.unr?listen",
        Wiring {
            vp: true,
            srv: true,
            cli: false,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "servertravel srv=0 cli=0 falls through",
        "SERVERTRAVEL Foo.unr",
        Wiring {
            vp: true,
            cli: false,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "servertravel srv=1 cli=1 falls through",
        "SERVERTRAVEL Foo.unr",
        Wiring {
            vp: true,
            srv: true,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "servertravel client DISCONNECT",
        "SERVERTRAVEL DISCONNECT",
        Wiring {
            vp: true,
            srv: false,
            cli: true,
            level_net: 3,
            pend: 3,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "servertravel client EXIT",
        "SERVERTRAVEL EXIT",
        Wiring {
            vp: true,
            srv: false,
            cli: true,
            level_net: 3,
            pend: 3,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "say srv=1 cli=0",
        "SAY hello there",
        Wiring {
            vp: true,
            srv: true,
            cli: false,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "say srv=0 cli=0 falls through",
        "SAY hello",
        Wiring {
            vp: true,
            cli: false,
            ..BASE
        },
    );
    // DISCONNECT
    knobs_reset();
    case(
        "disconnect vp=1 net=3",
        "DISCONNECT",
        Wiring {
            vp: true,
            level_net: 3,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "disconnect vp=1 pending=3",
        "DISCONNECT",
        Wiring {
            vp: true,
            pend: 3,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "disconnect vp=1 none",
        "DISCONNECT",
        Wiring { vp: true, ..BASE },
    );
    knobs_reset();
    knob(|k| k.browse_err = Some(tc("Disc")));
    case("disconnect vp=0 browse=0 err=Disc", "DISCONNECT", BASE);
    knobs_reset();
    knob(|k| k.browse_err = Some(Vec::new()));
    case("disconnect vp=0 browse=0 err=empty", "DISCONNECT", BASE);
    knobs_reset();
    case(
        "disconnect vp=1 net=2 no channel",
        "DISCONNECT",
        Wiring {
            vp: true,
            level_net: 2,
            ..BASE
        },
    );
    // RECONNECT
    knobs_reset();
    case("reconnect vp=1", "RECONNECT", Wiring { vp: true, ..BASE });
    knobs_reset();
    knob(|k| k.browse_err = Some(tc("Rec")));
    case("reconnect vp=0 browse=0 err=Rec", "RECONNECT", BASE);
    knobs_reset();
    knob(|k| k.browse_ret = 1);
    case("reconnect vp=0 browse=1", "RECONNECT", BASE);
    // EXIT / QUIT
    knobs_reset();
    case(
        "exit net=3 pending=3",
        "EXIT",
        Wiring {
            vp: true,
            level_net: 3,
            pend: 3,
            ..BASE
        },
    );
    knobs_reset();
    case(
        "exit net=2 no channel",
        "EXIT",
        Wiring {
            vp: true,
            level_net: 2,
            ..BASE
        },
    );
    knobs_reset();
    case("quit none", "QUIT", BASE);
    // tick rate queries
    knobs_reset();
    case("getcurrenttickrate", "GETCURRENTTICKRATE", BASE);
    knobs_reset();
    case("getmaxtickrate 60", "GETMAXTICKRATE", BASE);
    knobs_reset();
    knob(|k| k.max_tick = 0.5);
    case("getmaxtickrate 0.5", "GETMAXTICKRATE", BASE);
    // GSPYLITE / OBJCLEAN
    knobs_reset();
    case("gspylite", "GSPYLITE", BASE);
    knobs_reset();
    case("objclean lvl=1", "OBJCLEAN", BASE);
    knobs_reset();
    case("objclean lvl=0", "OBJCLEAN", Wiring { lvl: false, ..BASE });
    // SAVEGAME / LOADGAME / SAVEPACTORS
    knobs_reset();
    case("savegame slot=3", "SAVEGAME 3", BASE);
    knobs_reset();
    case("savegame non-digit", "SAVEGAME x", BASE);
    knobs_reset();
    knob(|k| k.format_ret = 1);
    case("loadgame slot=2 format=1", "LOADGAME 2", BASE);
    knobs_reset();
    knob(|k| k.format_ret = 0);
    case("loadgame slot=2 format=0", "LOADGAME 2", BASE);
    knobs_reset();
    knob(|k| {
        k.next_url = Some(tc("Busy.unr"));
        k.format_ret = 1;
    });
    case("loadgame nexturl set", "LOADGAME 2", BASE);
    knobs_reset();
    case(
        "loadgame lvl=0",
        "LOADGAME 2",
        Wiring { lvl: false, ..BASE },
    );
    knobs_reset();
    case(
        "savepactors lvl=0",
        "SAVEPACTORS",
        Wiring { lvl: false, ..BASE },
    );
    knobs_reset();
    knob(|k| k.enter_text = tc("Entry"));
    case("savepactors lvl=1", "SAVEPACTORS", BASE);
    // CANCEL
    knobs_reset();
    case("cancel pending=0", "CANCEL", BASE);
    knobs_reset();
    knob(|k| k.try_skip_ret = 1);
    case(
        "cancel pending=3 skip=1",
        "CANCEL",
        Wiring { pend: 3, ..BASE },
    );
    knobs_reset();
    knob(|k| k.try_skip_ret = 0);
    case(
        "cancel pending=3 skip=0",
        "CANCEL",
        Wiring { pend: 3, ..BASE },
    );
    // level Exec / ScriptConsoleExec / base Exec fallthrough
    knobs_reset();
    knob(|k| k.level_exec_ret = 1);
    case("level exec ret=1", "FOO", BASE);
    knobs_reset();
    knob(|k| k.script_exec_ret = 1);
    case(
        "script console exec ret=1",
        "FOO",
        Wiring { game: true, ..BASE },
    );
    knobs_reset();
    knob(|k| k.exec_ret = 1);
    case("base exec ret=1", "FOO", Wiring { game: true, ..BASE });
    knobs_reset();
    knob(|k| k.exec_ret = 0);
    case(
        "base exec ret=0 nolevel",
        "FOO",
        Wiring { lvl: false, ..BASE },
    );
    // SET under client netmode: AllowSet only drops for actor (non-GameInfo) classes
    knobs_reset();
    knob(|k| {
        k.has_class = true;
        k.child_actor = 1;
        k.child_gi = 0;
    });
    case(
        "set client actor class",
        "SET Pawn bShowMenu 1",
        Wiring { netmode: 3, ..BASE },
    );
    knobs_reset();
    knob(|k| {
        k.has_class = true;
        k.child_actor = 1;
        k.child_gi = 1;
    });
    case(
        "set client gameinfo class",
        "SET GameInfo Foo 1",
        Wiring { netmode: 3, ..BASE },
    );
    knobs_reset();
    case(
        "set client no class",
        "SET Nothing Foo 1",
        Wiring { netmode: 3, ..BASE },
    );
    knobs_reset();
    knob(|k| {
        k.has_class = true;
        k.child_actor = 1;
    });
    case("set non-client netmode", "SET Pawn bShowMenu 1", BASE);
    knobs_reset();
    knob(|k| k.exec_ret = 1);
    case(
        "set no level",
        "SET Pawn bShowMenu 1",
        Wiring { lvl: false, ..BASE },
    );
}

#[test]
fn game_exec_replay_matches_oracle() {
    run();
    let out = take_output();

    let cases = out.lines().filter(|l| l.starts_with("case exec ")).count();
    assert_eq!(cases, EXPECTED_CASES, "exec case count");

    if out != EXPECTED {
        let (n, got, want) = out
            .lines()
            .zip(EXPECTED.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i + 1, a, b))
            .unwrap_or((0, "<length mismatch>", "<length mismatch>"));
        panic!(
            "exec replay diverged at line {n}\n  got:  {got}\n  want: {want}\n  got lines {} / want lines {}",
            out.lines().count(),
            EXPECTED.lines().count()
        );
    }
}
