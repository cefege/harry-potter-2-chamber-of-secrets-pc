//! Replays the original-C++ oracle from `Build/player_fixture_harness/` against
//! `hp2_engine::player` (`UPlayer::Exec`, `UPlayer::Destroy`).
//!
//! `fixtures/player_exec.txt` holds 600 `exec` records and 4 `destroy` records. Each
//! record stores the scenario inputs, then the expected return value, final actor
//! state, and the ordered callee trace. The mock services below reproduce the harness
//! object graph and forwarders: the same callee order, the same executor argument, and
//! the same mid-call `Actor` clear that the `mut` knob triggers.

use std::cell::RefCell;
use std::ffi::c_void;

use hp2_engine::params::TChar;
use hp2_engine::player::{PlayerServices, destroy, exec};

const FIXTURE: &str = include_str!("fixtures/player_exec.txt");
const EXPECTED_EXEC: usize = 600;
const EXPECTED_DESTROY: usize = 4;

const ACTOR: usize = 0x1000;
const LEVEL: usize = 0x2000;
const LI: usize = 0x3000;
const GAME: usize = 0x4000;
const HUD: usize = 0x5000;
const INV0: usize = 0x6000;
const INV_STRIDE: usize = 0x10;
const ENGINE: usize = 0x7000;
const PLAYER: usize = 0x8000;

#[derive(Default)]
struct Mock {
    player_actor: usize,
    actor_player_set: bool,
    actor_level: usize,
    game: usize,
    hud: usize,
    ninv: usize,
    lvl_ret: i32,
    game_ret: i32,
    hud_ret: i32,
    actor_ret: i32,
    engine_ret: i32,
    ibits: u32,
    mut_idx: i32,
    is_running: bool,
    trace: Vec<String>,
}

thread_local! {
    static MOCK: RefCell<Mock> = RefCell::new(Mock::default());
}

fn with<R>(f: impl FnOnce(&mut Mock) -> R) -> R {
    MOCK.with(|m| f(&mut m.borrow_mut()))
}

unsafe extern "C" fn m_is_running() -> i32 {
    with(|m| i32::from(m.is_running))
}

unsafe extern "C" fn m_player_actor(p: *mut c_void) -> *mut c_void {
    with(|m| {
        if p as usize != PLAYER {
            m.trace.push("?player".to_owned());
        }
        m.player_actor as *mut c_void
    })
}

unsafe extern "C" fn m_player_set_actor(p: *mut c_void, a: *mut c_void) {
    with(|m| {
        if p as usize != PLAYER {
            m.trace.push("?player".to_owned());
        }
        m.player_actor = a as usize;
    });
}

unsafe extern "C" fn m_actor_get_level(a: *mut c_void) -> *mut c_void {
    with(|m| {
        if a as usize != ACTOR {
            m.trace.push("?actor".to_owned());
        }
    });
    LEVEL as *mut c_void
}

unsafe extern "C" fn m_actor_clear_player(a: *mut c_void) {
    with(|m| {
        if a as usize != ACTOR {
            m.trace.push("?actor".to_owned());
        }
        m.actor_player_set = false;
    });
}

unsafe extern "C" fn m_level_destroy_actor(_l: *mut c_void, a: *mut c_void, force: i32) {
    with(|m| {
        let is_actor = i32::from(a as usize == ACTOR);
        m.trace.push(format!("DA{force}{is_actor}"));
    });
}

unsafe extern "C" fn m_level_exec(l: *mut c_void, _cmd: *const TChar, _ar: *mut c_void) -> i32 {
    with(|m| {
        if l as usize != LEVEL {
            m.trace.push("?level".to_owned());
        }
        m.trace.push(format!("LX{}", m.lvl_ret));
        if m.mut_idx == 1 {
            m.player_actor = 0;
        }
        m.lvl_ret
    })
}

unsafe extern "C" fn m_actor_level(a: *mut c_void) -> *mut c_void {
    with(|m| {
        if a as usize != ACTOR {
            m.trace.push("?actor".to_owned());
        }
        m.actor_level as *mut c_void
    })
}

unsafe extern "C" fn m_level_game(li: *mut c_void) -> *mut c_void {
    with(|m| {
        if li as usize != LI {
            m.trace.push("?li".to_owned());
        }
        m.game as *mut c_void
    })
}

unsafe extern "C" fn m_sce(
    obj: *mut c_void,
    _cmd: *const TChar,
    _ar: *mut c_void,
    executor: *mut c_void,
) -> i32 {
    let obj = obj as usize;
    let e = i32::from(!executor.is_null());
    // Executor is either absent or the player's actor (object-identity check).
    let exec_ok = executor.is_null() || executor as usize == ACTOR;
    with(|m| {
        if !exec_ok {
            m.trace.push("?exec".to_owned());
        }
        let (tok, ret, mut_at) = if obj == ACTOR {
            (format!("AS{e}{}", m.actor_ret), m.actor_ret, 4)
        } else if obj == HUD {
            (format!("HS{e}{}", m.hud_ret), m.hud_ret, 3)
        } else if obj == GAME {
            (format!("GS{e}{}", m.game_ret), m.game_ret, 2)
        } else {
            let k = (obj - INV0) / INV_STRIDE;
            let r = ((m.ibits >> k) & 1) as i32;
            let mut_at = if k == 0 { 5 } else { -1 };
            (format!("I{k}S{e}{r}"), r, mut_at)
        };
        m.trace.push(tok);
        if m.mut_idx == mut_at {
            m.player_actor = 0;
        }
        ret
    })
}

unsafe extern "C" fn m_actor_hud(a: *mut c_void) -> *mut c_void {
    with(|m| {
        if a as usize != ACTOR {
            m.trace.push("?actor".to_owned());
        }
        m.hud as *mut c_void
    })
}

unsafe extern "C" fn m_actor_inventory(a: *mut c_void) -> *mut c_void {
    with(|m| {
        if a as usize != ACTOR {
            m.trace.push("?actor".to_owned());
        }
        if m.ninv > 0 {
            INV0 as *mut c_void
        } else {
            std::ptr::null_mut()
        }
    })
}

unsafe extern "C" fn m_inventory_next(inv: *mut c_void) -> *mut c_void {
    let off = (inv as usize).wrapping_sub(INV0);
    let k = off / INV_STRIDE;
    with(|m| {
        if !off.is_multiple_of(INV_STRIDE) || k >= m.ninv {
            m.trace.push("?inv".to_owned());
        }
        if k + 1 < m.ninv {
            (INV0 + (k + 1) * INV_STRIDE) as *mut c_void
        } else {
            std::ptr::null_mut()
        }
    })
}

unsafe extern "C" fn m_level_engine(l: *mut c_void) -> *mut c_void {
    with(|m| {
        if l as usize != LEVEL {
            m.trace.push("?level".to_owned());
        }
    });
    ENGINE as *mut c_void
}

unsafe extern "C" fn m_engine_exec(e: *mut c_void, _cmd: *const TChar, _ar: *mut c_void) -> i32 {
    with(|m| {
        if e as usize != ENGINE {
            m.trace.push("?engine".to_owned());
        }
        m.trace.push(format!("EX{}", m.engine_ret));
        m.engine_ret
    })
}

static SVC: PlayerServices = PlayerServices {
    is_running: m_is_running,
    player_actor: m_player_actor,
    player_set_actor: m_player_set_actor,
    actor_get_level: m_actor_get_level,
    actor_clear_player: m_actor_clear_player,
    level_destroy_actor: m_level_destroy_actor,
    level_exec: m_level_exec,
    actor_level: m_actor_level,
    level_game: m_level_game,
    object_script_console_exec: m_sce,
    actor_hud: m_actor_hud,
    actor_inventory: m_actor_inventory,
    inventory_next: m_inventory_next,
    level_engine: m_level_engine,
    engine_exec: m_engine_exec,
};

fn kv<'a>(toks: &[&'a str], key: &str) -> &'a str {
    let prefix = format!("{key}=");
    toks.iter()
        .find_map(|t| t.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("missing field {key}"))
}

fn num(toks: &[&str], key: &str) -> i64 {
    kv(toks, key)
        .parse()
        .unwrap_or_else(|_| panic!("bad number for {key}"))
}

fn trace_text(trace: &[String]) -> String {
    if trace.is_empty() {
        "-".to_owned()
    } else {
        trace.join(",")
    }
}

#[test]
fn player_exec_replay_matches_original() {
    let mut exec_records = 0usize;
    let mut destroy_records = 0usize;

    for line in FIXTURE.lines().filter(|l| !l.starts_with('#')) {
        let (lhs, rhs) = line
            .split_once(" -> ")
            .unwrap_or_else(|| panic!("record without arrow: {line}"));
        let lhs: Vec<&str> = lhs.split_whitespace().collect();
        let rhs: Vec<&str> = rhs.split_whitespace().collect();

        match lhs[0] {
            "exec" => {
                exec_records += 1;
                let act = num(&lhs, "act");
                let li = num(&lhs, "li");
                let game = num(&lhs, "game");
                let hud = num(&lhs, "hud");
                let ninv = num(&lhs, "ninv") as usize;
                let ibits = num(&lhs, "ibits") as u32;
                let lvl = num(&lhs, "lvl") as i32;
                let mut_idx = num(&lhs, "mut") as i32;
                let gr = num(&lhs, "gr") as i32;
                let hr = num(&lhs, "hr") as i32;
                let acr = num(&lhs, "acr") as i32;
                let er = num(&lhs, "er") as i32;

                with(|m| {
                    *m = Mock {
                        player_actor: if act == 1 { ACTOR } else { 0 },
                        actor_player_set: true,
                        actor_level: if li == 1 { LI } else { 0 },
                        game: if game == 1 { GAME } else { 0 },
                        hud: if hud == 1 { HUD } else { 0 },
                        ninv,
                        lvl_ret: lvl,
                        game_ret: gr,
                        hud_ret: hr,
                        actor_ret: acr,
                        engine_ret: er,
                        ibits,
                        mut_idx,
                        is_running: true,
                        trace: Vec::new(),
                    };
                });

                let cmd: [TChar; 1] = [0];
                let ret = unsafe {
                    exec(
                        &SVC,
                        PLAYER as *mut c_void,
                        cmd.as_ptr(),
                        std::ptr::null_mut(),
                    )
                };

                let final_actor = with(|m| i64::from(m.player_actor != 0));
                let trace = with(|m| trace_text(&m.trace));

                assert_eq!(i64::from(ret), num(&rhs, "ret"), "ret mismatch: {line}");
                assert_eq!(final_actor, num(&rhs, "final"), "final mismatch: {line}");
                assert_eq!(trace, kv(&rhs, "trace"), "trace mismatch: {line}");
            }
            "destroy" => {
                destroy_records += 1;
                let run = num(&lhs, "run");
                let act = num(&lhs, "act");

                with(|m| {
                    *m = Mock {
                        player_actor: if act == 1 { ACTOR } else { 0 },
                        actor_player_set: true,
                        is_running: run == 1,
                        ..Mock::default()
                    };
                });

                unsafe { destroy(&SVC, PLAYER as *mut c_void) };

                let pnull = with(|m| i64::from(!m.actor_player_set));
                let panull = with(|m| i64::from(m.player_actor == 0));
                let trace = with(|m| trace_text(&m.trace));

                assert_eq!(pnull, num(&rhs, "pnull"), "pnull mismatch: {line}");
                assert_eq!(panull, num(&rhs, "panull"), "panull mismatch: {line}");
                assert_eq!(trace, kv(&rhs, "trace"), "trace mismatch: {line}");
            }
            other => panic!("unknown record kind {other}"),
        }
    }

    assert_eq!(exec_records, EXPECTED_EXEC, "exec record count");
    assert_eq!(destroy_records, EXPECTED_DESTROY, "destroy record count");
}
