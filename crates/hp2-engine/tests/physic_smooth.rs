//! Replays `AActor::moveSmooth` against the original-engine fixture (`physic_smooth.txt`).

include!("physic_scripted.rs");

const SMOOTH_INPUT_WORDS: usize = 24;
const SMOOTH_OUTPUT_WORDS: usize = 4;
const ZERO_V: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

#[derive(Clone, Copy)]
struct Step {
    ret: i32,
    time: f32,
    normal: FVector,
    actor: i32,
}

const NO_STEP: Step = Step {
    ret: 1,
    time: 1.0,
    normal: ZERO_V,
    actor: -1,
};

thread_local! {
    static SMOOTH_PLAN: RefCell<(usize, [Step; 3])> = const { RefCell::new((0, [NO_STEP; 3])) };
}

unsafe extern "C" fn smooth_move(
    _actor: *mut c_void,
    delta: *const FVector,
    rotation: *const FRotator,
    hit: *mut FCheckResult,
) -> i32 {
    // Mirrors the harness's `hook_move_actor` mode 7.
    let step = SMOOTH_PLAN.with(|s| {
        let mut s = s.borrow_mut();
        let call = s.0;
        s.0 += 1;
        if call < 3 { s.1[call] } else { NO_STEP }
    });
    let (d, r) = unsafe { (*delta, *rotation) };
    let hit_actor = with(|s| {
        if step.ret != 0 || step.actor < 0 {
            null_mut()
        } else {
            s.pool[step.actor as usize] as *mut c_void
        }
    });
    unsafe {
        (*hit).time = if step.ret != 0 { 1.0 } else { step.time };
        (*hit).normal = if step.ret != 0 { ZERO_V } else { step.normal };
        (*hit).location = ZERO_V;
        (*hit).actor = hit_actor;
    }
    with(|s| {
        lg(
            s,
            format!(
                "move {:08x} {:08x} {:08x} {} {} {} {}\n",
                bits(d.x),
                bits(d.y),
                bits(d.z),
                r.pitch,
                r.yaw,
                r.roll,
                step.ret
            ),
        )
    });
    step.ret
}

fn smooth_services() -> Hp2PhysicServices {
    Hp2PhysicServices {
        move_actor: smooth_move,
        ..SERVICES
    }
}

/// Returns `[ret, digest hi, digest lo, log length]` and the log text.
fn run_smooth(seed: u64, w: &[u32]) -> ([u32; SMOOTH_OUTPUT_WORDS], String) {
    assert_eq!(w.len(), SMOOTH_INPUT_WORDS);
    let mut actor_mem = vec![0u64; 1024];
    let mut pool = [vec![0u64; 1024], vec![0u64; 1024], vec![0u64; 1024]];
    let actor_p = actor_mem.as_mut_ptr() as usize;
    let pool_p = [
        pool[0].as_mut_ptr() as usize,
        pool[1].as_mut_ptr() as usize,
        pool[2].as_mut_ptr() as usize,
    ];
    with(|s| {
        s.svc = seed;
        s.log.clear();
        s.subj = actor_p;
        s.pool = pool_p;
    });
    let mut plan = [NO_STEP; 3];
    for (k, step) in plan.iter_mut().enumerate() {
        let q = &w[6 + 6 * k..];
        *step = Step {
            ret: q[0] as i32,
            time: f(q[1]),
            normal: FVector {
                x: f(q[2]),
                y: f(q[3]),
                z: f(q[4]),
            },
            actor: q[5] as i32,
        };
    }
    SMOOTH_PLAN.with(|s| *s.borrow_mut() = (0, plan));
    unsafe {
        let a = actor_mem.as_mut_ptr() as *mut AActor;
        (*a).rotation = FRotator {
            pitch: w[3] as i32,
            yaw: w[4] as i32,
            roll: w[5] as i32,
        };
    }
    let delta = v3(&w[0..3]);
    let ret = unsafe { physic::move_smooth(actor_p as *mut AActor, &smooth_services(), delta) };
    let log = with(|s| s.log.clone());
    let h = fnv1a(log.as_bytes());
    (
        [ret as u32, (h >> 32) as u32, h as u32, log.len() as u32],
        log,
    )
}

#[test]
fn physic_smooth_matches_original() {
    let text = include_str!("fixtures/physic_smooth.txt");
    let mut total = 0usize;
    // Key: (ret, move calls, event_hit_wall calls).
    let mut classes: BTreeMap<(i32, usize, usize), usize> = BTreeMap::new();
    for (rec, line) in text.lines().enumerate() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let sep = toks.iter().position(|t| *t == "|").expect("separator");
        assert_eq!(toks[0], "phys_smooth", "record {rec}");
        let seed = u64::from_str_radix(toks[1], 16).expect("seed");
        let input: Vec<u32> = toks[2..sep]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("input word"))
            .collect();
        let expected: Vec<u32> = toks[sep + 1..]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("output word"))
            .collect();
        assert_eq!(input.len(), SMOOTH_INPUT_WORDS, "record {rec} input width");
        assert_eq!(
            expected.len(),
            SMOOTH_OUTPUT_WORDS,
            "record {rec} output width"
        );
        let (got, log) = run_smooth(seed, &input);
        if let Some(i) = (0..SMOOTH_OUTPUT_WORDS).find(|&i| got[i] != expected[i]) {
            panic!(
                "record {rec} seed {seed:016x}: word {i} expected {:08x} got {:08x}\nlog:\n{log}",
                expected[i], got[i]
            );
        }
        let moves = log.lines().filter(|l| l.starts_with("move ")).count();
        let events = log
            .lines()
            .filter(|l| l.starts_with("event_hit_wall "))
            .count();
        *classes
            .entry((expected[0] as i32, moves, events))
            .or_insert(0) += 1;
        total += 1;
    }
    assert_eq!(total, 1200, "fixture case count");
    // No-hit, hit with a negative slide dot, adjusted move blocked, two-wall path.
    assert_eq!(
        classes.get(&(1, 1, 0)).copied().unwrap_or(0),
        300,
        "no-hit class"
    );
    assert_eq!(
        classes.get(&(0, 1, 0)).copied().unwrap_or(0),
        300,
        "negative-dot class"
    );
    assert_eq!(
        classes.get(&(0, 2, 1)).copied().unwrap_or(0),
        300,
        "single-wall class"
    );
    assert_eq!(
        classes.get(&(0, 3, 1)).copied().unwrap_or(0),
        300,
        "two-wall class"
    );
}
