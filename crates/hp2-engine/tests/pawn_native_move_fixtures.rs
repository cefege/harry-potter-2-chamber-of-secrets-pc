//! Bit-exact scripted replay for the move natives of `pawn_native_move`.
//!
//! The mock services mirror the `PawnMove*` hooks in
//! `Build/pawn_native_move_fixture_harness/pawn_native_move_harness.cpp`. The
//! scenario loop mirrors its `main`: 32 cases, each drawing a fresh pool and
//! script, then the 18 native entries in order. The fixture was generated from
//! the oracle `UnPawn.cpp` (`32dcfda`), so every byte of it is the C++ result.
//! The `*(T*)Result` writes of the thunks are modelled in `call_native`.

use hp2_abi::{
    FVector,
    engine::generated::{AActor, AInventory, ANavigationPoint, APawn, AZoneInfo, ULevel},
};
use hp2_engine::pawn_native_move::{
    Hp2PawnNativeMoveServices, actor_reachable, find_path_to, find_path_toward, find_random_dest,
    make_noise, move_to, move_toward, poll_destination, poll_move_toward, poll_strafe_facing,
    poll_turn_to, poll_turn_toward, show_self, strafe_facing, strafe_to, turn_to, turn_toward,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::size_of;

const FIXTURE: &str = include_str!("fixtures/pawn_native_move.txt");
const POOL: usize = 4;
const CASES: usize = 32;
const KINDS: usize = 8;
const K_REACH: usize = 0;
const K_CANMOVE: usize = 1;
const K_FINDTO: usize = 2;
const K_FINDTOWARD: usize = 3;
const K_RANDOMDEST: usize = 4;
const K_LOS: usize = 5;
const K_ROTATE: usize = 6;
const K_MOVE: usize = 7;
const ENTRIES: [&str; 18] = [
    "actorReachable",
    "findPathTo",
    "findPathToward",
    "findRandomDest",
    "makeNoise",
    "moveTo",
    "pollMoveTo",
    "moveToward",
    "pollMoveToward",
    "strafeTo",
    "pollStrafeTo",
    "strafeFacing",
    "pollStrafeFacing",
    "turnToward",
    "pollTurnToward",
    "turnTo",
    "pollTurnTo",
    "showSelf",
];

const fn larger(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

const OBJ_BYTES: usize = larger(
    larger(size_of::<APawn>(), size_of::<ANavigationPoint>()),
    larger(size_of::<AInventory>(), size_of::<AActor>()),
);
const LEVEL_BYTES: usize = size_of::<ULevel>();
const ZONE_BYTES: usize = size_of::<AZoneInfo>();

/// Zeroed, 16-byte aligned storage for one engine object.
#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct Blob<const N: usize> {
    _bytes: [u8; N],
}

/// The harness LCG (`g_seed`), drawn in the same order as the oracle driver.
struct Rng(u32);

impl Rng {
    fn next15(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 16) & 0x7fff
    }

    fn ri(&mut self, n: u32) -> u32 {
        self.next15() % n
    }

    fn frand(&mut self) -> f32 {
        ((self.next15() & 4095) as f32 / 256.0 - 8.0) / 3.0
    }
}

/// One queued argument, as the oracle `P_GET_*` macros would pop it.
#[derive(Clone, Copy)]
enum Arg {
    Vec(FVector),
    /// Pool index, `None` for NULL.
    Actor(Option<usize>),
    Float(f32),
    /// `present`, `ival`.
    OptInt(bool, i32),
    OptBool(bool, i32),
    /// `present`, `fval`.
    OptFloat(bool, f32),
}

#[derive(Default)]
struct World {
    /// Pool slot addresses; slot 0 is the pawn.
    pool: [usize; POOL],
    level: usize,
    zone: usize,
    calls: [u32; KINDS],
    script: [u32; KINDS],
    isa: [u32; POOL],
    probe: [u32; POOL],
    netmode: i32,
    reach_count: i32,
    nav_head: i32,
    pawn_head: i32,
    latent: i32,
    rotvec: [f32; 3],
    trace: Vec<String>,
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World::default());
}

fn with<R>(f: impl FnOnce(&mut World) -> R) -> R {
    WORLD.with(|w| f(&mut w.borrow_mut()))
}

fn tr(s: impl Into<String>) {
    let s = s.into();
    with(|w| w.trace.push(s));
}

fn pool_addr(i: usize) -> usize {
    with(|w| w.pool[i])
}

fn pool_index(addr: usize) -> Option<usize> {
    if addr == 0 {
        return None;
    }
    with(|w| w.pool.iter().position(|&p| p == addr))
}

/// Pool index of a live object; aborts like the harness on NULL or foreign objects.
fn must_index(addr: usize) -> usize {
    pool_index(addr).unwrap_or_else(|| panic!("object {addr:#x} is not a pool object"))
}

/// Harness `id_of`: `-` for NULL, `p<i>` for a pool object.
fn id_of(addr: usize) -> String {
    if addr == 0 {
        return "-".to_string();
    }
    format!("p{}", must_index(addr))
}

/// Harness `id_idx`: `-` for a negative index, `p<i>` otherwise.
fn id_idx(i: i32) -> String {
    if i < 0 {
        "-".to_string()
    } else {
        format!("p{i}")
    }
}

fn hex32(u: u32) -> String {
    format!("{u:08x}")
}

fn hexf(f: f32) -> String {
    hex32(f.to_bits())
}

fn xyz(v: FVector) -> String {
    format!("{},{},{}", hexf(v.x), hexf(v.y), hexf(v.z))
}

fn opt_or<T>(present: bool, value: T, default: T) -> T {
    if present { value } else { default }
}

fn addr(slot: Option<usize>) -> *mut c_void {
    slot.map_or(std::ptr::null_mut(), |i| pool_addr(i) as *mut c_void)
}

/// Draws the next scripted call result for `kind` and its call index.
fn next_call(kind: usize) -> (bool, u32) {
    with(|w| {
        let k = w.calls[kind];
        w.calls[kind] += 1;
        ((((w.script[kind] >> (k & 31)) & 1) != 0), k)
    })
}

/// Harness pick for path hooks: the `1 + k % 3`-th pool actor on a hit, NULL on a miss.
fn pick_best(hit: bool, k: u32) -> usize {
    if hit {
        pool_addr(1 + (k as usize) % 3)
    } else {
        0
    }
}

// Hooks mirroring the PawnMove* functions of the harness.

unsafe extern "C" fn mock_is_a(obj: *mut c_void, kind: u32) -> i32 {
    let i = must_index(obj as usize);
    let r = with(|w| (w.isa[i] >> kind) & 1);
    tr(format!("isA(p{i},{kind})={r}"));
    r as i32
}

unsafe extern "C" fn mock_is_probing(obj: *mut c_void, name: u32) -> i32 {
    let i = must_index(obj as usize);
    let r = with(|w| (w.probe[i] >> name) & 1);
    tr(format!("probing(p{i},{name})={r}"));
    r as i32
}

unsafe extern "C" fn mock_net_mode(_actor: *mut c_void) -> i32 {
    let v = with(|w| w.netmode);
    tr(format!("netmode()={v}"));
    v
}

unsafe extern "C" fn mock_reach_spec_count(_pawn: *mut c_void) -> i32 {
    let v = with(|w| w.reach_count);
    tr(format!("reachspecs()={v}"));
    v
}

unsafe extern "C" fn mock_level_pawn_list(_pawn: *mut c_void) -> *mut c_void {
    let h = with(|w| w.pawn_head);
    tr(format!("pawnlist()={}", id_idx(h)));
    if h < 0 {
        std::ptr::null_mut()
    } else {
        pool_addr(h as usize) as *mut c_void
    }
}

unsafe extern "C" fn mock_level_nav_list(_pawn: *mut c_void) -> *mut c_void {
    let h = with(|w| w.nav_head);
    tr(format!("navlist()={}", id_idx(h)));
    if h < 0 {
        std::ptr::null_mut()
    } else {
        pool_addr(h as usize) as *mut c_void
    }
}

unsafe extern "C" fn mock_actor_reachable(_pawn: *mut c_void, actor: *mut c_void) -> i32 {
    let (hit, _) = next_call(K_REACH);
    tr(format!(
        "reach(p{})={}",
        must_index(actor as usize),
        u8::from(hit)
    ));
    i32::from(hit)
}

unsafe extern "C" fn mock_can_move_to(
    _pawn: *mut c_void,
    anchor: *mut c_void,
    dest: *mut c_void,
) -> i32 {
    let (hit, _) = next_call(K_CANMOVE);
    tr(format!(
        "canmove(p{},p{})={}",
        must_index(anchor as usize),
        must_index(dest as usize),
        u8::from(hit)
    ));
    i32::from(hit)
}

unsafe extern "C" fn mock_find_path_to(
    _pawn: *mut c_void,
    point: *const c_void,
    single: i32,
    clear: i32,
    best: *mut *mut c_void,
) -> i32 {
    let (hit, k) = next_call(K_FINDTO);
    let chosen = pick_best(hit, k);
    // SAFETY: `point` is the live argument of the Rust caller; `best` is its out-slot.
    let point = unsafe { *(point as *const FVector) };
    unsafe { *best = chosen as *mut c_void };
    tr(format!(
        "findto({},{single},{clear})={}:{}",
        xyz(point),
        u8::from(hit),
        id_of(chosen)
    ));
    i32::from(hit)
}

unsafe extern "C" fn mock_find_path_toward(
    _pawn: *mut c_void,
    goal: *mut c_void,
    single: i32,
    clear: i32,
    best: *mut *mut c_void,
) -> i32 {
    let (hit, k) = next_call(K_FINDTOWARD);
    let chosen = pick_best(hit, k);
    let goal_index = must_index(goal as usize);
    // SAFETY: `best` is the live out-slot of the Rust caller.
    unsafe { *best = chosen as *mut c_void };
    tr(format!(
        "findtoward(p{goal_index},{single},{clear})={}:{}",
        u8::from(hit),
        id_of(chosen)
    ));
    i32::from(hit)
}

unsafe extern "C" fn mock_find_random_dest(_pawn: *mut c_void, best: *mut *mut c_void) -> i32 {
    let (hit, k) = next_call(K_RANDOMDEST);
    let chosen = pick_best(hit, k);
    // SAFETY: `best` is the live out-slot of the Rust caller.
    unsafe { *best = chosen as *mut c_void };
    tr(format!("randomdest()={}:{}", u8::from(hit), id_of(chosen)));
    i32::from(hit)
}

unsafe extern "C" fn mock_clear_paths(_pawn: *mut c_void) {
    tr("clear()");
}

unsafe extern "C" fn mock_handle_special(_pawn: *mut c_void, best: *mut *mut c_void) {
    // SAFETY: `best` is the live in-out slot of the Rust caller.
    let target = unsafe { *best } as usize;
    tr(format!("special(p{})", must_index(target)));
}

unsafe extern "C" fn mock_set_move_timer(_pawn: *mut c_void, move_size: f32) {
    tr(format!("timer({})", hexf(move_size)));
}

unsafe extern "C" fn mock_rotate_toward(_pawn: *mut c_void, focal: *const c_void) -> i32 {
    let (hit, _) = next_call(K_ROTATE);
    // SAFETY: `focal` is the live `Focus` field passed by the Rust caller.
    let v = unsafe { *(focal as *const FVector) };
    tr(format!("rotate({})={}", xyz(v), u8::from(hit)));
    i32::from(hit)
}

unsafe extern "C" fn mock_move_toward(_pawn: *mut c_void, dest: *const c_void) -> i32 {
    let (hit, _) = next_call(K_MOVE);
    // SAFETY: `dest` is the live `Destination` field passed by the Rust caller.
    let v = unsafe { *(dest as *const FVector) };
    tr(format!("move({})={}", xyz(v), u8::from(hit)));
    i32::from(hit)
}

unsafe extern "C" fn mock_set_latent_action(_pawn: *mut c_void, latent: i32) {
    with(|w| w.latent = latent);
    tr(format!("latent({latent})"));
}

unsafe extern "C" fn mock_event_alter_destination(pawn: *mut c_void) {
    // SAFETY: `pawn` is the live pool-0 pawn.
    unsafe { (*(pawn as *mut APawn)).destination.x += 3.0 };
    tr("alter()");
}

unsafe extern "C" fn mock_event_see_player(pawn: *mut c_void, seer: *mut c_void) {
    tr(format!(
        "seeplayer(p{},p{})",
        must_index(pawn as usize),
        must_index(seer as usize)
    ));
}

unsafe extern "C" fn mock_line_of_sight_to(
    pawn: *mut c_void,
    other: *mut c_void,
    show_self: i32,
) -> u32 {
    let (hit, _) = next_call(K_LOS);
    tr(format!(
        "los(p{},p{},{show_self})={}",
        must_index(pawn as usize),
        must_index(other as usize),
        u8::from(hit)
    ));
    u32::from(hit)
}

unsafe extern "C" fn mock_check_noise_hearing(_actor: *mut c_void, loudness: f32) {
    tr(format!("noise({})", hexf(loudness)));
}

unsafe extern "C" fn mock_rotation_vector(_actor: *mut c_void, out: *mut c_void) {
    let [x, y, z] = with(|w| w.rotvec);
    let v = FVector { x, y, z };
    tr(format!("rotvec()={}", xyz(v)));
    // SAFETY: `out` is the live `FVector` slot of the Rust caller.
    unsafe { *(out as *mut FVector) = v };
}

/// Draws one argument `Vec` (X, Y, Z).
fn push_vec(rng: &mut Rng) -> Arg {
    Arg::Vec(FVector {
        x: rng.frand(),
        y: rng.frand(),
        z: rng.frand(),
    })
}

/// Draws an actor argument: `nullable` maps pick 4 to NULL.
fn push_actor(rng: &mut Rng, nullable: bool) -> Arg {
    if nullable {
        let pick = rng.ri(5);
        Arg::Actor(if pick == 4 { None } else { Some(pick as usize) })
    } else {
        Arg::Actor(Some(rng.ri(4) as usize))
    }
}

fn push_opt_int(rng: &mut Rng) -> Arg {
    let present = rng.ri(2) != 0;
    let value = rng.ri(2) as i32;
    Arg::OptInt(present, value)
}

fn push_opt_bool(rng: &mut Rng) -> Arg {
    let present = rng.ri(2) != 0;
    let value = rng.ri(2) as i32;
    Arg::OptBool(present, value)
}

fn push_opt_float(rng: &mut Rng) -> Arg {
    let present = rng.ri(2) != 0;
    let value = rng.frand();
    Arg::OptFloat(present, value)
}

/// Draws the arguments of one entry, in the harness `queue_for` order.
fn queue_for(name: &str, rng: &mut Rng) -> Vec<Arg> {
    match name {
        "actorReachable" => vec![push_actor(rng, false)],
        "findPathTo" => vec![push_vec(rng), push_opt_int(rng), push_opt_bool(rng)],
        "findPathToward" => vec![
            push_actor(rng, false),
            push_opt_int(rng),
            push_opt_bool(rng),
        ],
        "findRandomDest" => vec![push_opt_bool(rng)],
        "makeNoise" => vec![Arg::Float(rng.frand())],
        "moveTo" => vec![push_vec(rng), push_opt_float(rng)],
        "moveToward" => vec![push_actor(rng, true), push_opt_float(rng)],
        "strafeTo" => vec![push_vec(rng), push_vec(rng)],
        "strafeFacing" => vec![push_vec(rng), push_actor(rng, true)],
        "turnToward" => vec![push_actor(rng, true)],
        "turnTo" => vec![push_vec(rng)],
        _ => Vec::new(),
    }
}

fn render(a: &Arg) -> String {
    match a {
        Arg::Vec(v) => format!("v={}", xyz(*v)),
        Arg::Actor(slot) => format!(
            "a={}",
            slot.map_or_else(|| "-".to_string(), |i| format!("p{i}"))
        ),
        Arg::Float(f) => format!("f={}", hexf(*f)),
        Arg::OptInt(present, v) => format!("i={}:{v}", u8::from(*present)),
        Arg::OptBool(present, v) => format!("u={}:{v}", u8::from(*present)),
        Arg::OptFloat(present, f) => format!("g={}:{}", u8::from(*present), hexf(*f)),
    }
}

/// Calls the Rust entry point with the oracle's defaults and writes `Result` as the thunk does.
fn call_native(
    svc: &Hp2PawnNativeMoveServices,
    name: &str,
    args: &[Arg],
    pawn: *mut APawn,
    result: &mut [u8; 16],
) {
    let self_actor = pool_addr(0) as *mut c_void;
    // SAFETY: `pawn` and every actor address are live pool slots of the current case.
    unsafe {
        match (name, args) {
            ("actorReachable", [Arg::Actor(a)]) => {
                let r = actor_reachable(svc, pawn, addr(*a));
                result[..4].copy_from_slice(&r.to_ne_bytes());
            }
            ("findPathTo", [Arg::Vec(point), Arg::OptInt(sp, sv), Arg::OptBool(cp, cv)]) => {
                let best = find_path_to(
                    svc,
                    pawn,
                    std::ptr::from_ref(point).cast(),
                    opt_or(*sp, *sv, 0),
                    opt_or(*cp, *cv, 1),
                );
                result[..8].copy_from_slice(&(best as usize).to_ne_bytes());
            }
            ("findPathToward", [Arg::Actor(goal), Arg::OptInt(sp, sv), Arg::OptBool(cp, cv)]) => {
                let best = find_path_toward(
                    svc,
                    pawn,
                    addr(*goal),
                    opt_or(*sp, *sv, 0),
                    opt_or(*cp, *cv, 1),
                );
                result[..8].copy_from_slice(&(best as usize).to_ne_bytes());
            }
            ("findRandomDest", [Arg::OptBool(p, v)]) => {
                let best = find_random_dest(svc, pawn, opt_or(*p, *v, 1));
                result[..8].copy_from_slice(&(best as usize).to_ne_bytes());
            }
            ("makeNoise", [Arg::Float(loudness)]) => make_noise(svc, self_actor, *loudness),
            ("moveTo", [Arg::Vec(dest), Arg::OptFloat(p, f)]) => {
                move_to(svc, pawn, *dest, opt_or(*p, *f, 1.0), 501);
            }
            ("pollMoveTo" | "pollStrafeTo", []) => poll_destination(svc, pawn),
            ("moveToward", [Arg::Actor(goal), Arg::OptFloat(p, f)]) => {
                move_toward(svc, pawn, addr(*goal), opt_or(*p, *f, 1.0), 503);
            }
            ("pollMoveToward", []) => poll_move_toward(svc, pawn),
            ("strafeTo", [Arg::Vec(dest), Arg::Vec(focal)]) => {
                strafe_to(svc, pawn, *dest, *focal, 505);
            }
            ("strafeFacing", [Arg::Vec(dest), Arg::Actor(goal)]) => {
                strafe_facing(svc, pawn, *dest, addr(*goal), 507);
            }
            ("pollStrafeFacing", []) => poll_strafe_facing(svc, pawn),
            ("turnToward", [Arg::Actor(goal)]) => turn_toward(svc, pawn, addr(*goal), 511),
            ("pollTurnToward", []) => poll_turn_toward(svc, pawn),
            ("turnTo", [Arg::Vec(focal)]) => turn_to(svc, pawn, *focal, 509),
            ("pollTurnTo", []) => poll_turn_to(svc, pawn),
            ("showSelf", []) => show_self(svc, pawn),
            _ => panic!("unexpected entry {name} with {} args", args.len()),
        }
    }
}

/// Record `Result` as the harness prints it for each entry.
fn ret_string(name: &str, result: &[u8; 16]) -> String {
    match name {
        "findPathTo" | "findPathToward" | "findRandomDest" => {
            id_of(usize::from_ne_bytes(result[..8].try_into().unwrap()))
        }
        "makeNoise" | "showSelf" => "-".to_string(),
        _ => hex32(u32::from_ne_bytes(result[..4].try_into().unwrap())),
    }
}

/// Pawn and actor fields the record reports, read after the call.
fn state_str(pawn: *mut APawn) -> String {
    // SAFETY: `pawn` is the live pool-0 pawn.
    unsafe {
        format!(
            "mt={} fa={} sg={} dest={} focus={} acc={} ds={} mtm={} red={} sp={} shoot={}",
            id_of((*pawn).move_target as usize),
            id_of((*pawn).face_target as usize),
            id_of((*pawn).special_goal as usize),
            xyz((*pawn).destination),
            xyz((*pawn).focus),
            xyz((*pawn).base.acceleration),
            hexf((*pawn).desired_speed),
            hexf((*pawn).move_timer),
            u8::from((*pawn).b_reduced_speed()),
            hexf((*pawn).special_pause),
            u8::from((*pawn).b_shoot_special()),
        )
    }
}

fn run_entry(svc: &Hp2PawnNativeMoveServices, rng: &mut Rng, name: &str, case: usize) -> String {
    let args = queue_for(name, rng);
    let pawn = pool_addr(0) as *mut APawn;
    let mut result = [0u8; 16];
    with(|w| w.trace.clear());
    call_native(svc, name, &args, pawn, &mut result);
    let mut rec = format!("{name} c={case}");
    for a in &args {
        rec.push(' ');
        rec.push_str(&render(a));
    }
    let ret = ret_string(name, &result);
    let lat = with(|w| w.latent);
    let calls = with(|w| w.trace.join(";"));
    format!(
        "{rec} ret={ret} lat={lat} {} calls={calls}",
        state_str(pawn)
    )
}

fn pick_target(rng: &mut Rng) -> usize {
    let pick = rng.ri(5) as usize;
    if pick == 4 { 0 } else { pool_addr(pick) }
}

/// Links the set bits of `mask` ascending through `link(prev, next)`; returns the head or -1.
fn link_chain(mask: u32, mut link: impl FnMut(usize, usize)) -> i32 {
    let mut head = -1;
    let mut prev: Option<usize> = None;
    for i in 0..POOL {
        if ((mask >> i) & 1) == 0 {
            continue;
        }
        match prev {
            None => head = i as i32,
            Some(p) => link(pool_addr(p), pool_addr(i)),
        }
        prev = Some(i);
    }
    head
}

/// Harness `setup_case`: the same draws, memsets and writes, in the same order.
fn setup_case(rng: &mut Rng, pool_base: *mut Blob<OBJ_BYTES>) {
    const K_PHYS: [u8; 5] = [1, 3, 4, 10, 2];
    const K_LATENT: [i32; 3] = [0, 501, 511];
    let (level, zone) = with(|w| {
        w.calls = [0; KINDS];
        (w.level, w.zone)
    });
    let netmode = rng.ri(4) as i32;
    let reach = rng.ri(3) as i32;
    let mut isa = [0u32; POOL];
    for v in &mut isa {
        *v = rng.ri(8);
    }
    let mut probe = [0u32; POOL];
    for v in &mut probe {
        *v = rng.ri(4);
    }
    let water = rng.ri(2);
    with(|w| {
        w.netmode = netmode;
        w.reach_count = reach;
        w.isa = isa;
        w.probe = probe;
    });
    // SAFETY: the storage outlives the test; zeroing matches the harness `memset`s.
    unsafe {
        std::ptr::write_bytes(pool_base, 0, POOL);
        std::ptr::write_bytes(level as *mut u8, 0, LEVEL_BYTES);
        std::ptr::write_bytes(zone as *mut u8, 0, ZONE_BYTES);
        (*(zone as *mut AZoneInfo)).set_b_water_zone(water != 0);
    }
    for i in 0..POOL {
        let loc = FVector {
            x: rng.frand(),
            y: rng.frand(),
            z: rng.frand(),
        };
        let radius = rng.ri(160) as f32 / 3.0;
        let height = rng.ri(160) as f32 / 3.0;
        let phys = K_PHYS[rng.ri(5) as usize];
        let marker = rng.ri(4) as usize;
        let marker_addr = if marker == 0 { 0 } else { pool_addr(marker) };
        let a = pool_addr(i) as *mut AActor;
        // SAFETY: `a` is a live zeroed pool slot.
        unsafe {
            (*a).x_level = level as _;
            (*a).region.zone = zone as _;
            (*a).physics = phys;
            (*a).location = loc;
            (*a).collision_radius = radius;
            (*a).collision_height = height;
            (*(a as *mut AInventory)).my_marker = marker_addr as _;
        }
    }

    let pw = pool_addr(0) as *mut APawn;
    // SAFETY: `pw` is the live pool-0 pawn.
    unsafe {
        (*pw).set_b_is_player(rng.ri(2) != 0);
        (*pw).set_b_can_swim(rng.ri(2) != 0);
        (*pw).set_b_can_strafe(rng.ri(2) != 0);
        (*pw).set_b_reduced_speed(rng.ri(2) != 0);
        (*pw).set_b_advanced_tactics(rng.ri(2) != 0);
        (*pw).set_b_shoot_special(rng.ri(2) != 0);
        (*pw).special_pause = rng.frand();
        (*pw).max_desired_speed = rng.frand();
        (*pw).ground_speed = rng.frand();
        (*pw).accel_rate = rng.frand();
        (*pw).desired_speed = rng.frand();
        (*pw).move_timer = rng.frand();
        (*pw).sight_counter = rng.frand();
        (*pw).floor = FVector {
            x: rng.frand(),
            y: rng.frand(),
            z: rng.frand(),
        };
        (*pw).base.acceleration = FVector {
            x: rng.frand(),
            y: rng.frand(),
            z: rng.frand(),
        };
        (*pw).destination = FVector {
            x: rng.frand(),
            y: rng.frand(),
            z: rng.frand(),
        };
        (*pw).focus = FVector {
            x: rng.frand(),
            y: rng.frand(),
            z: rng.frand(),
        };
        (*pw).move_target = pick_target(rng) as _;
        (*pw).face_target = pick_target(rng) as _;
        (*pw).special_goal = pick_target(rng) as _;
    }

    let nav_mask = rng.ri(16);
    let pawn_mask = rng.ri(16);
    let nav_head = link_chain(nav_mask, |p, n| {
        // SAFETY: `p` and `n` are live pool slots.
        unsafe { (*(p as *mut ANavigationPoint)).next_navigation_point = n as _ };
    });
    let pawn_head = link_chain(pawn_mask, |p, n| {
        // SAFETY: `p` and `n` are live pool slots.
        unsafe { (*(p as *mut APawn)).next_pawn = n as _ };
    });
    with(|w| {
        w.nav_head = nav_head;
        w.pawn_head = pawn_head;
    });

    let mut script = [0u32; KINDS];
    for s in &mut script {
        let hi = rng.next15();
        let lo = rng.next15();
        *s = (hi << 15) ^ lo;
    }
    let rotvec = [rng.frand(), rng.frand(), rng.frand()];
    let latent = K_LATENT[rng.ri(3) as usize];
    with(|w| {
        w.script = script;
        w.rotvec = rotvec;
        w.latent = latent;
    });
}

#[test]
fn pawn_native_move_replay_is_bit_exact() {
    let mut pool = [Blob {
        _bytes: [0u8; OBJ_BYTES],
    }; POOL];
    let mut level = Blob {
        _bytes: [0u8; LEVEL_BYTES],
    };
    let mut zone = Blob {
        _bytes: [0u8; ZONE_BYTES],
    };
    let pool_base = pool.as_mut_ptr();
    let pools: [usize; POOL] = std::array::from_fn(|i| pool_base.wrapping_add(i) as usize);
    let level_addr = std::ptr::from_mut(&mut level) as usize;
    let zone_addr = std::ptr::from_mut(&mut zone) as usize;
    with(|w| {
        w.pool = pools;
        w.level = level_addr;
        w.zone = zone_addr;
    });

    let svc = Hp2PawnNativeMoveServices {
        is_a: mock_is_a,
        is_probing: mock_is_probing,
        net_mode: mock_net_mode,
        reach_spec_count: mock_reach_spec_count,
        level_pawn_list: mock_level_pawn_list,
        level_nav_list: mock_level_nav_list,
        actor_reachable: mock_actor_reachable,
        can_move_to: mock_can_move_to,
        find_path_to: mock_find_path_to,
        find_path_toward: mock_find_path_toward,
        find_random_dest: mock_find_random_dest,
        clear_paths: mock_clear_paths,
        handle_special: mock_handle_special,
        set_move_timer: mock_set_move_timer,
        rotate_toward: mock_rotate_toward,
        move_toward: mock_move_toward,
        set_latent_action: mock_set_latent_action,
        event_alter_destination: mock_event_alter_destination,
        event_see_player: mock_event_see_player,
        line_of_sight_to: mock_line_of_sight_to,
        check_noise_hearing: mock_check_noise_hearing,
        rotation_vector: mock_rotation_vector,
    };

    let mut rng = Rng(0x5eed_0001);
    let mut out = String::new();
    for case in 0..CASES {
        setup_case(&mut rng, pool_base);
        for name in ENTRIES {
            out.push_str(&run_entry(&svc, &mut rng, name, case));
            out.push('\n');
        }
    }

    assert_eq!(out, FIXTURE);
    for name in ENTRIES {
        let count = out
            .lines()
            .filter(|l| l.split(' ').next() == Some(name))
            .count();
        assert_eq!(count, CASES, "record count for {name}");
    }
}
