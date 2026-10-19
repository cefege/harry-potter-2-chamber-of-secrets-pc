//! Replays the original-C++ route oracle fixtures (`fixtures/route_*.txt`)
//! against `hp2_engine::route`.
//!
//! The fixtures are the stdout of a harness that links the pristine 8e34caa
//! `UnRoute.cpp` with `UnReach.h` and drives `APawn::findPathToward`,
//! `findPathTo`, `findPathTowardBestInventory`, `findRandomDest`, `canMoveTo`,
//! `breadthPathFrom`, `breadthPathToInventory`, `findPath`, `clearPaths`,
//! `SetRouteCache` and `calcMoveFlags` on synthetic worlds (grids, lines and a
//! 15-ary tree). Input lines are echoed verbatim; `= ` lines are the oracle's
//! observations. Floats are IEEE-754 bit patterns in hex, so every comparison
//! is bit-exact.
//!
//! The replay re-runs every input line through the Rust port, regenerates the
//! full transcript and requires it to equal the fixture byte for byte. The
//! harness source is scratch and not part of the repo; the fixture files are
//! the committed record of its output.
//!
//! Not covered: the breadth-overflow branch (`numList > 500`,
//! `UnRoute.cpp:972-980`) is not reached by any fixture.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::fmt::Debug;
use std::fmt::Write as _;
use std::mem::zeroed;
use std::ptr;
use std::str::FromStr;
use std::str::SplitWhitespace;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, AInventory, AInventorySpot, AMover, ANavigationPoint, APawn, FReachSpec, TArrayRaw,
    ULevel,
};
use hp2_engine::route::{self, Hp2RouteServices};

/// Name index the replay assigns to node `i` (`1000 + i`). Index 0 is `FName`
/// `NAME_None`, which the harness uses for `find <i> -`.
const NODE_NAME_BASE: i32 = 1000;
/// Log kinds in `route::LOG_*` order.
const LOG_KINDS: [&str; 4] = ["start", "end", "overflow", "1000"];

#[derive(Clone, Copy)]
struct NodeDef {
    spot: bool,
    item: i32,
    flags: i32,
    extra: i32,
    scost: i32,
    pos: [f32; 3],
}

#[derive(Clone, Copy)]
struct SpecDef {
    start: usize,
    end: usize,
    dist: i32,
    radius: i32,
    height: i32,
    rflags: i32,
}

#[derive(Clone, Copy)]
struct ItemDef {
    probing: i32,
    lf: f32,
    maxd: f32,
    desir: f32,
}

#[derive(Clone, Copy)]
struct WallDef {
    x: f32,
    y0: f32,
    y1: f32,
    mover: bool,
    po: i32,
}

#[derive(Clone, Copy)]
struct PawnDef {
    pos: [f32; 3],
    phys: i32,
    radius: f32,
    height: f32,
    eye: f32,
    flags: i32,
    mt: i32,
}

/// Identity of an object the replay has allocated; used for `refname` and for
/// the service lookups.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ent {
    Node(usize),
    Item(usize),
    Mover(usize),
    Pawn,
    GoalPawn,
}

/// Objects allocated by `build`. Owned here so their addresses stay valid.
struct Built {
    _level: Box<ULevel>,
    _model: Box<u8>,
    _items: Vec<AInventory>,
    _nodes: Vec<AInventorySpot>,
    _specs: Vec<FReachSpec>,
    _movers: Vec<AMover>,
    _pawn: Option<Box<APawn>>,
    _gpawn: Option<Box<APawn>>,
    np: Vec<*mut ANavigationPoint>,
    /// Mover actor per wall index; null for static walls.
    mover_of_wall: Vec<*mut c_void>,
    pawn: *mut APawn,
    gpawn: *mut APawn,
}

#[derive(Default)]
struct World {
    seed0: u32,
    seed: u32,
    farz: f32,
    reachy: f32,
    nodes: Vec<NodeDef>,
    specs: Vec<SpecDef>,
    items: Vec<ItemDef>,
    walls: Vec<WallDef>,
    pawn: Option<PawnDef>,
    gpawn: Option<PawnDef>,
    built: Option<Built>,
    ents: HashMap<usize, Ent>,
    log: Vec<String>,
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World::default());
}

fn hexf(tok: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(tok, 16).expect("hex float"))
}

fn next<'a>(ts: &mut SplitWhitespace<'a>) -> &'a str {
    ts.next().expect("missing token")
}

fn num<T: FromStr>(ts: &mut SplitWhitespace<'_>) -> T
where
    T::Err: Debug,
{
    next(ts).parse().expect("integer token")
}

fn reset_world(w: &mut World) {
    w.nodes.clear();
    w.specs.clear();
    w.items.clear();
    w.walls.clear();
    w.pawn = None;
    w.gpawn = None;
    w.seed = w.seed0;
    w.farz = 0.0;
    w.reachy = 0.0;
    w.built = None;
    w.ents.clear();
    w.log.clear();
}

fn directive(w: &mut World, cmd: &str, ts: &mut SplitWhitespace<'_>) {
    match cmd {
        "scenario" => reset_world(w),
        "seed" => {
            w.seed0 = num(ts);
            w.seed = w.seed0;
        }
        "farz" => w.farz = hexf(next(ts)),
        "reachy" => w.reachy = hexf(next(ts)),
        "wall" => {
            let x = hexf(next(ts));
            let y0 = hexf(next(ts));
            let y1 = hexf(next(ts));
            let mover = next(ts) == "M";
            let po = if mover { num(ts) } else { 0 };
            w.walls.push(WallDef {
                x,
                y0,
                y1,
                mover,
                po,
            });
        }
        "item" => {
            let probing = num(ts);
            let lf = hexf(next(ts));
            let maxd = hexf(next(ts));
            let desir = hexf(next(ts));
            w.items.push(ItemDef {
                probing,
                lf,
                maxd,
                desir,
            });
        }
        "node" => {
            let k = next(ts);
            let it = next(ts);
            let flags = num(ts);
            let extra = num(ts);
            let scost = num(ts);
            let pos = [hexf(next(ts)), hexf(next(ts)), hexf(next(ts))];
            w.nodes.push(NodeDef {
                spot: k == "I",
                item: if it == "-" {
                    -1
                } else {
                    it.parse().expect("item")
                },
                flags,
                extra,
                scost,
                pos,
            });
        }
        "spec" => {
            let start = num(ts);
            let end = num(ts);
            let dist = num(ts);
            let radius = num(ts);
            let height = num(ts);
            let rflags = num(ts);
            w.specs.push(SpecDef {
                start,
                end,
                dist,
                radius,
                height,
                rflags,
            });
        }
        "pawn" | "goalpawn" => {
            let pos = [hexf(next(ts)), hexf(next(ts)), hexf(next(ts))];
            let phys = num(ts);
            let radius = hexf(next(ts));
            let height = hexf(next(ts));
            let eye = hexf(next(ts));
            let flags = num(ts);
            let mt = num(ts);
            let p = PawnDef {
                pos,
                phys,
                radius,
                height,
                eye,
                flags,
                mt,
            };
            if cmd == "pawn" {
                w.pawn = Some(p);
            } else {
                w.gpawn = Some(p);
            }
        }
        other => panic!("unknown directive: {other}"),
    }
}

fn place(slots: &mut [i32; 16], value: i32) {
    *slots
        .iter_mut()
        .find(|slot| **slot < 0)
        .expect("free reach-spec slot") = value;
}

fn make_pawn(d: &PawnDef, level: *mut c_void, np: &[*mut ANavigationPoint]) -> Box<APawn> {
    let mut p: Box<APawn> = Box::new(unsafe { zeroed() });
    p.base.physics = d.phys as u8;
    p.base.x_level = level;
    p.base.location = FVector {
        x: d.pos[0],
        y: d.pos[1],
        z: d.pos[2],
    };
    p.base.collision_radius = d.radius;
    p.base.collision_height = d.height;
    p.base_eye_height = d.eye;
    p.set_b_can_walk(d.flags & 1 != 0);
    p.set_b_can_fly(d.flags & 2 != 0);
    p.set_b_can_swim(d.flags & 4 != 0);
    p.set_b_can_jump(d.flags & 8 != 0);
    p.set_b_can_open_doors(d.flags & 16 != 0);
    p.set_b_can_do_special(d.flags & 32 != 0);
    p.set_b_is_player(d.flags & 64 != 0);
    p.set_b_hunting(d.flags & 128 != 0);
    p.move_target = if d.mt >= 0 {
        np[d.mt as usize].cast()
    } else {
        ptr::null_mut()
    };
    p
}

fn build(w: &mut World) {
    w.ents.clear();
    let mut level: Box<ULevel> = Box::new(unsafe { zeroed() });
    let level_ptr: *mut c_void = (&mut *level as *mut ULevel).cast();
    let model: Box<u8> = Box::new(0);
    level.model = (&*model as *const u8).cast_mut().cast();

    let items: Vec<AInventory> = w
        .items
        .iter()
        .map(|d| {
            let mut it: AInventory = unsafe { zeroed() };
            it.base.latent_float = d.lf;
            it.base.x_level = level_ptr;
            it.max_desireability = d.maxd;
            it
        })
        .collect();
    for (i, it) in items.iter().enumerate() {
        w.ents
            .insert(it as *const AInventory as usize, Ent::Item(i));
    }

    let mut nodes: Vec<AInventorySpot> = w
        .nodes
        .iter()
        .map(|d| {
            let mut s: AInventorySpot = unsafe { zeroed() };
            if d.spot && d.item >= 0 {
                s.marked_item = (&items[d.item as usize] as *const AInventory)
                    .cast_mut()
                    .cast();
            }
            let n = &mut s.base;
            n.base.location = FVector {
                x: d.pos[0],
                y: d.pos[1],
                z: d.pos[2],
            };
            n.base.x_level = level_ptr;
            n.extra_cost = d.extra;
            n.cost = 0;
            n.set_b_special_cost(d.flags & 1 != 0);
            n.set_b_player_only(d.flags & 2 != 0);
            n.set_b_end_point(d.flags & 4 != 0);
            n.visited_weight = 0;
            n.best_path_weight = 0;
            n.paths = [-1; 16];
            n.upstream_paths = [-1; 16];
            n.pruned_paths = [-1; 16];
            s
        })
        .collect();
    let np: Vec<*mut ANavigationPoint> = nodes
        .iter_mut()
        .map(|s| &mut s.base as *mut ANavigationPoint)
        .collect();
    for (i, &p) in np.iter().enumerate() {
        w.ents.insert(p as usize, Ent::Node(i));
    }
    for (i, &p) in np.iter().enumerate() {
        let next = np.get(i + 1).copied().unwrap_or(ptr::null_mut());
        unsafe { (*p).next_navigation_point = next };
    }

    let mut specs: Vec<FReachSpec> = Vec::new();
    for (i, s) in w.specs.iter().enumerate() {
        let start = np[s.start];
        let end = np[s.end];
        let mut rs: FReachSpec = unsafe { zeroed() };
        rs.distance = s.dist;
        rs.start = start.cast();
        rs.end = end.cast();
        rs.collision_radius = s.radius;
        rs.collision_height = s.height;
        rs.reach_flags = s.rflags;
        rs.b_pruned = 0;
        specs.push(rs);
        unsafe {
            place(&mut (*start).paths, i as i32);
            place(&mut (*end).upstream_paths, i as i32);
        }
    }
    let spec_count = specs.len() as i32;
    level.reach_specs = TArrayRaw {
        data: specs.as_mut_ptr().cast(),
        num: spec_count,
        max: spec_count,
    };

    let mover_walls: Vec<usize> = w
        .walls
        .iter()
        .enumerate()
        .filter(|(_, wd)| wd.mover)
        .map(|(i, _)| i)
        .collect();
    let mut movers: Vec<AMover> = mover_walls.iter().map(|_| unsafe { zeroed() }).collect();
    let mut mover_of_wall: Vec<*mut c_void> = vec![ptr::null_mut(); w.walls.len()];
    for (k, &i) in mover_walls.iter().enumerate() {
        let actor: *mut AActor = &mut movers[k].base;
        w.ents.insert(actor as usize, Ent::Mover(i));
        mover_of_wall[i] = actor.cast();
    }

    let mut pawn = w.pawn.map(|d| make_pawn(&d, level_ptr, &np));
    let pawn_ptr: *mut APawn = pawn
        .as_deref_mut()
        .map_or(ptr::null_mut(), |p| p as *mut APawn);
    if !pawn_ptr.is_null() {
        w.ents.insert(pawn_ptr as usize, Ent::Pawn);
    }
    let mut gpawn = w.gpawn.map(|d| make_pawn(&d, level_ptr, &np));
    let gpawn_ptr: *mut APawn = gpawn
        .as_deref_mut()
        .map_or(ptr::null_mut(), |p| p as *mut APawn);
    if !gpawn_ptr.is_null() {
        w.ents.insert(gpawn_ptr as usize, Ent::GoalPawn);
    }

    w.built = Some(Built {
        _level: level,
        _model: model,
        _items: items,
        _nodes: nodes,
        _specs: specs,
        _movers: movers,
        _pawn: pawn,
        _gpawn: gpawn,
        np,
        mover_of_wall,
        pawn: pawn_ptr,
        gpawn: gpawn_ptr,
    });
}

fn refname(p: *const c_void) -> String {
    if p.is_null() {
        return "-".into();
    }
    WORLD.with_borrow(|w| match w.ents.get(&(p as usize)) {
        Some(Ent::Node(i)) => format!("n{i}"),
        Some(Ent::Item(i)) => format!("i{i}"),
        Some(Ent::Mover(i)) => format!("M{i}"),
        Some(Ent::Pawn) => "P".into(),
        Some(Ent::GoalPawn) => "G".into(),
        None => "?".into(),
    })
}

/// Same predicate as the harness: a wall is crossed when the endpoints lie on
/// opposite sides of its plane and the segment's Y span overlaps the wall.
fn crosses(w: &WallDef, a: &FVector, b: &FVector) -> bool {
    let sa = a.x < w.x;
    let sb = b.x < w.x;
    if sa == sb {
        return false;
    }
    let lo = a.y.min(b.y);
    let hi = a.y.max(b.y);
    lo <= w.y1 && hi >= w.y0
}

unsafe extern "C" fn svc_app_frand() -> f32 {
    WORLD.with_borrow_mut(|w| {
        w.seed = w.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ((w.seed >> 8) & 0x00FF_FFFF) as f32 / 16_777_216.0
    })
}

unsafe extern "C" fn svc_far_move_actor(
    _level: *mut c_void,
    _actor: *mut c_void,
    dest: *const c_void,
    _test: i32,
    _no_check: i32,
) -> i32 {
    let z = unsafe { (*dest.cast::<FVector>()).z };
    WORLD.with_borrow(|w| i32::from(z < w.farz))
}

unsafe extern "C" fn svc_point_reachable(
    _pawn: *mut c_void,
    point: *const c_void,
    _know: i32,
) -> i32 {
    let y = unsafe { (*point.cast::<FVector>()).y };
    WORLD.with_borrow(|w| i32::from(y <= w.reachy))
}

unsafe extern "C" fn svc_actor_reachable(
    _pawn: *mut c_void,
    other: *mut c_void,
    _know: i32,
) -> i32 {
    let y = unsafe { (*other.cast::<AActor>()).location.y };
    WORLD.with_borrow(|w| i32::from(y <= w.reachy))
}

unsafe extern "C" fn svc_goal_jump_landing(_goal: *mut c_void, dest: *mut c_void) {
    unsafe { (*dest.cast::<FVector>()).z -= 16.0 };
}

unsafe extern "C" fn svc_model_fast_line_check(
    _model: *mut c_void,
    end: *const c_void,
    start: *const c_void,
) -> u8 {
    let (e, s) = unsafe { (*end.cast::<FVector>(), *start.cast::<FVector>()) };
    WORLD.with_borrow(|w| u8::from(!w.walls.iter().any(|wd| !wd.mover && crosses(wd, &s, &e))))
}

unsafe extern "C" fn svc_single_line_hit_actor(
    _level: *mut c_void,
    _source: *mut c_void,
    end: *const c_void,
    start: *const c_void,
) -> *mut c_void {
    let (e, s) = unsafe { (*end.cast::<FVector>(), *start.cast::<FVector>()) };
    WORLD.with_borrow(|w| {
        let b = w.built.as_ref().expect("world built");
        w.walls
            .iter()
            .position(|wd| crosses(wd, &s, &e))
            .map_or(ptr::null_mut(), |i| b.mover_of_wall[i])
    })
}

unsafe extern "C" fn svc_navigation_point_list(_level: *mut c_void) -> *mut c_void {
    WORLD.with_borrow(|w| {
        w.built
            .as_ref()
            .and_then(|b| b.np.first().copied())
            .map_or(ptr::null_mut::<c_void>(), |p| p.cast::<c_void>())
    })
}

unsafe extern "C" fn svc_actor_is_a(actor: *mut c_void, kind: i32) -> i32 {
    WORLD.with_borrow(|w| {
        let hit = match w.ents.get(&(actor as usize)) {
            Some(Ent::Node(i)) => {
                kind == route::CLASS_NAVIGATION_POINT
                    || (w.nodes[*i].spot && kind == route::CLASS_INVENTORY_SPOT)
            }
            Some(Ent::Pawn | Ent::GoalPawn) => kind == route::CLASS_PAWN,
            Some(Ent::Mover(_)) => kind == route::CLASS_MOVER,
            Some(Ent::Item(_)) | None => false,
        };
        i32::from(hit)
    })
}

unsafe extern "C" fn svc_mover_player_only(mover: *mut c_void) -> i32 {
    WORLD.with_borrow(|w| match w.ents.get(&(mover as usize)) {
        Some(Ent::Mover(i)) => w.walls[*i].po,
        _ => 0,
    })
}

unsafe extern "C" fn svc_event_special_cost(node: *mut c_void, _seeker: *mut c_void) -> i32 {
    WORLD.with_borrow(|w| match w.ents.get(&(node as usize)) {
        Some(Ent::Node(i)) => w.nodes[*i].scost,
        _ => 0,
    })
}

unsafe extern "C" fn svc_item_is_probing_touch(item: *mut c_void) -> i32 {
    WORLD.with_borrow(|w| match w.ents.get(&(item as usize)) {
        Some(Ent::Item(i)) => i32::from(w.items[*i].probing != 0),
        _ => 0,
    })
}

unsafe extern "C" fn svc_item_bot_desirability(item: *mut c_void, _bot: *mut c_void) -> f32 {
    WORLD.with_borrow(|w| match w.ents.get(&(item as usize)) {
        Some(Ent::Item(i)) => w.items[*i].desir,
        _ => 0.0,
    })
}

unsafe extern "C" fn svc_actor_name_index(actor: *mut c_void) -> i32 {
    WORLD.with_borrow(|w| match w.ents.get(&(actor as usize)) {
        Some(Ent::Node(i)) => NODE_NAME_BASE + *i as i32,
        _ => -1,
    })
}

unsafe extern "C" fn svc_actor_class(actor: *mut c_void) -> *mut c_void {
    // Class identity only: nav 1, spot 2, pawn 3, mover 4, item 5.
    let code: usize = WORLD.with_borrow(|w| match w.ents.get(&(actor as usize)) {
        Some(Ent::Node(i)) => {
            if w.nodes[*i].spot {
                2
            } else {
                1
            }
        }
        Some(Ent::Pawn | Ent::GoalPawn) => 3,
        Some(Ent::Mover(_)) => 4,
        Some(Ent::Item(_)) => 5,
        None => 0,
    });
    code as *mut c_void
}

unsafe extern "C" fn svc_log_actor(kind: i32, actor: *mut c_void) {
    let name = refname(actor);
    WORLD.with_borrow_mut(|w| {
        w.log
            .push(format!("log {} {name}", LOG_KINDS[kind as usize]))
    });
}

fn services() -> Hp2RouteServices {
    Hp2RouteServices {
        app_frand: svc_app_frand,
        far_move_actor: svc_far_move_actor,
        point_reachable: svc_point_reachable,
        actor_reachable: svc_actor_reachable,
        goal_jump_landing: svc_goal_jump_landing,
        model_fast_line_check: svc_model_fast_line_check,
        single_line_hit_actor: svc_single_line_hit_actor,
        navigation_point_list: svc_navigation_point_list,
        actor_is_a: svc_actor_is_a,
        mover_player_only: svc_mover_player_only,
        event_special_cost: svc_event_special_cost,
        item_is_probing_touch: svc_item_is_probing_touch,
        item_bot_desirability: svc_item_bot_desirability,
        actor_name_index: svc_actor_name_index,
        actor_class: svc_actor_class,
        log_actor: svc_log_actor,
    }
}

fn node_index(tok: &str) -> usize {
    tok.parse().expect("node index")
}

/// Executes one `call` line and appends its observation lines to `out`.
fn run_call(ts: &mut SplitWhitespace<'_>, out: &mut String) {
    let s = services();
    let (pawn, gpawn, np) = WORLD.with_borrow(|w| {
        let b = w.built.as_ref().expect("world built");
        (b.pawn, b.gpawn, b.np.clone())
    });
    WORLD.with_borrow_mut(|w| w.log.clear());

    let op = next(ts);
    let mut ret = String::new();
    let mut best = String::new();
    unsafe {
        match op {
            "toward" => {
                let g = next(ts);
                let single = num::<i32>(ts);
                let clr = num::<i32>(ts);
                let goal: *mut AActor = if g == "G" {
                    gpawn.cast()
                } else {
                    np[node_index(g)].cast()
                };
                let mut bp: *mut AActor = ptr::null_mut();
                let r = route::find_path_toward(&s, pawn, goal, single, &mut bp, clr);
                ret = r.to_string();
                best = refname(bp.cast());
            }
            "to" => {
                let x = hexf(next(ts));
                let y = hexf(next(ts));
                let z = hexf(next(ts));
                let single = num::<i32>(ts);
                let clr = num::<i32>(ts);
                let dest = FVector { x, y, z };
                let mut bp: *mut AActor = ptr::null_mut();
                let r = route::find_path_to(&s, pawn, dest, single, &mut bp, clr);
                ret = r.to_string();
                best = refname(bp.cast());
            }
            "inv" => {
                let clr = num::<i32>(ts);
                let min_weight = hexf(next(ts));
                let predict = num::<i32>(ts);
                let mut bp: *mut AActor = ptr::null_mut();
                let w = route::find_path_toward_best_inventory(
                    &s, pawn, &mut bp, clr, min_weight, predict,
                );
                ret = format!("f:{:08x}", w.to_bits());
                best = refname(bp.cast());
            }
            "random" => {
                let mut bp: *mut AActor = ptr::null_mut();
                let r = route::find_random_dest(&s, pawn, &mut bp);
                ret = r.to_string();
                best = refname(bp.cast());
            }
            "canmove" => {
                let a = next(ts);
                let b = next(ts);
                let aa: *mut AActor = if a == "-" {
                    ptr::null_mut()
                } else {
                    np[node_index(a)].cast()
                };
                let bb: *mut AActor = if b == "-" {
                    ptr::null_mut()
                } else {
                    np[node_index(b)].cast()
                };
                ret = route::can_move_to(&s, pawn, aa, bb).to_string();
            }
            "prime" => {
                let i = node_index(next(ts));
                let d = num::<i32>(ts);
                let p = np[i];
                (*p).visited_weight = d;
            }
            "clear" => route::clear_paths(&s, pawn),
            "bpf" => {
                let i = node_index(next(ts));
                let single = num::<i32>(ts);
                let mf = num::<i32>(ts);
                let mut bp: *mut AActor = ptr::null_mut();
                let r = route::breadth_path_from(&s, pawn, np[i].cast(), &mut bp, single, mf);
                ret = r.to_string();
                best = refname(bp.cast());
            }
            "bpi" => {
                let i = node_index(next(ts));
                let mf = num::<i32>(ts);
                let min_weight = hexf(next(ts));
                let predict = num::<i32>(ts);
                let mut bp: *mut AActor = ptr::null_mut();
                let w = route::breadth_path_to_inventory(
                    &s,
                    pawn,
                    np[i].cast(),
                    &mut bp,
                    mf,
                    min_weight,
                    predict,
                );
                ret = format!("f:{:08x}", w.to_bits());
                best = refname(bp.cast());
            }
            "setcache" => {
                let i = next(ts);
                let best_ptr: *mut c_void = if i == "-" {
                    ptr::null_mut()
                } else {
                    np[node_index(i)].cast()
                };
                route::set_route_cache(pawn, best_ptr);
            }
            "find" => {
                let i = node_index(next(ts));
                let d = next(ts);
                let dest_name = if d == "-" {
                    0
                } else {
                    NODE_NAME_BASE + node_index(d) as i32
                };
                let mut nxt: *mut ANavigationPoint = ptr::null_mut();
                let found = route::find_path(&s, pawn, &mut nxt, np[i].cast(), dest_name);
                ret = if found { "1" } else { "0" }.into();
                best = refname(nxt.cast());
            }
            "mf" => ret = route::calc_move_flags(pawn).to_string(),
            other => panic!("unknown call: {other}"),
        }
    }

    let logs = WORLD.with_borrow_mut(|w| std::mem::take(&mut w.log));
    for line in logs {
        writeln!(out, "= {line}").unwrap();
    }
    writeln!(out, "= ret {ret}").unwrap();
    writeln!(out, "= best {best}").unwrap();
    out.push_str("= cache");
    for slot in unsafe { (*pawn).route_cache } {
        write!(out, " {}", refname(slot)).unwrap();
    }
    out.push('\n');
    for (i, &p) in np.iter().enumerate() {
        let n = unsafe { &*p };
        writeln!(
            out,
            "= node {i} vw={} cost={} ep={} bpw={} prev={} next={} pord={} sp={}",
            n.visited_weight,
            n.cost,
            i32::from(n.b_end_point()),
            n.best_path_weight,
            refname(n.previous_path),
            refname(n.next_ordered.cast()),
            refname(n.prev_ordered.cast()),
            refname(n.start_path),
        )
        .unwrap();
    }
}

/// Replays a fixture's input lines and returns the regenerated transcript.
fn replay(text: &str) -> String {
    WORLD.with_borrow_mut(reset_world);
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with('=') {
            continue;
        }
        out.push_str(line);
        out.push('\n');
        let mut ts = line.split_whitespace();
        match ts.next() {
            None => {}
            Some(cmd) if cmd.starts_with('#') => {}
            Some("call") => {
                WORLD.with_borrow_mut(|w| {
                    if w.built.is_none() {
                        build(w);
                    }
                });
                run_call(&mut ts, &mut out);
            }
            Some(cmd) => WORLD.with_borrow_mut(|w| directive(w, cmd, &mut ts)),
        }
    }
    out
}

fn check(name: &str) {
    let path = format!("{}/tests/fixtures/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let got = replay(&want);
    let got_lines: Vec<&str> = got.lines().collect();
    let want_lines: Vec<&str> = want.lines().collect();
    for (i, (g, w)) in got_lines.iter().zip(&want_lines).enumerate() {
        assert_eq!(g, w, "{name}: line {}", i + 1);
    }
    assert_eq!(got_lines.len(), want_lines.len(), "{name}: line count");
    assert_eq!(got, want, "{name}: transcript");
}

#[test]
fn route_empty() {
    check("route_empty");
}

#[test]
fn route_line() {
    check("route_line");
}

#[test]
fn route_grid_a() {
    check("route_grid_a");
}

#[test]
fn route_grid_walls() {
    check("route_grid_walls");
}

#[test]
fn route_grid_find() {
    check("route_grid_find");
}

#[test]
fn route_grid600() {
    check("route_grid600");
}

#[test]
fn route_grid1089() {
    check("route_grid1089");
}

#[test]
fn route_tree_fanout() {
    check("route_tree_fanout");
}
