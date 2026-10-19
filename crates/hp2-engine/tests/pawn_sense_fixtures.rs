//! Bit-exact scripted replay for the ten `pawn_sense` entrypoints.
use hp2_abi::{
    FVector,
    engine::generated::{AActor, APawn, AZoneInfo, FCheckResult, FRotator},
};
use hp2_engine::pawn_sense::*;
use std::cell::RefCell;
use std::ffi::c_void;

const FIXTURE: &str = include_str!("fixtures/pawn_sense.txt");
const LEVEL: usize = 0x1000;
const INPUT_WORDS: usize = 78;
const SNAPSHOT_WORDS: usize = 42;
const KINDS: usize = 27;

#[derive(Clone)]
struct Script {
    kind: u32,
    args: Vec<u32>,
    outs: Vec<u32>,
}
struct Replay {
    scripts: Vec<Script>,
    at: usize,
    actors: Vec<usize>,
    classes: Vec<u32>,
    team_game: u32,
    time: u32,
    pawn_team: u32,
    other_team: u32,
    zone_gravity: u32,
    pawn_pri: u32,
    other_pri: u32,
}
thread_local! {
    static REPLAY: RefCell<Replay> = const { RefCell::new(Replay { scripts: Vec::new(), at: 0, actors: Vec::new(), classes: Vec::new(), team_game: 0, time: 0, pawn_team: 0, other_team: 0, zone_gravity: 0, pawn_pri: 0, other_pri: 0 }) };
    static CLASS_RESULTS: RefCell<[[usize;2];2]> = const { RefCell::new([[0;2];2]) };
    static HITS: RefCell<[usize; KINDS]> = const { RefCell::new([0; KINDS]) };
}

fn hit_kind(kind: u32) {
    HITS.with(|h| h.borrow_mut()[kind as usize] += 1);
}
fn take_script(kind: u32, args: &[u32]) -> Vec<u32> {
    REPLAY.with(|r| {
        let mut r = r.borrow_mut();
        let s = &r.scripts[r.at];
        assert_eq!(s.kind, kind, "callback order");
        assert_eq!(s.args, args, "callback {kind} arguments");
        let outs = s.outs.clone();
        r.at += 1;
        HITS.with(|h| h.borrow_mut()[kind as usize] += 1);
        outs
    })
}
fn pid(p: *mut c_void) -> u32 {
    if p.is_null() {
        return 0;
    }
    REPLAY.with(|r| {
        r.borrow()
            .actors
            .iter()
            .position(|&q| q == p as usize)
            .unwrap_or_else(|| panic!("unknown pointer {p:p}")) as u32
    })
}
fn fw(f: f32) -> u32 {
    f.to_bits()
}
fn vwords(v: &FVector) -> [u32; 3] {
    [fw(v.x), fw(v.y), fw(v.z)]
}
fn read_vec(w: &[u32], p: &mut usize) -> FVector {
    let v = FVector {
        x: f32::from_bits(w[*p]),
        y: f32::from_bits(w[*p + 1]),
        z: f32::from_bits(w[*p + 2]),
    };
    *p += 3;
    v
}
fn out(kind: u32, args: &[u32]) -> Vec<u32> {
    take_script(kind, args)
}

unsafe extern "C" fn fast_line(_l: *mut c_void, a: *const c_void, b: *const c_void) -> i32 {
    let mut x = vwords(unsafe { &*a.cast::<FVector>() }).to_vec();
    x.extend(unsafe { vwords(&*b.cast::<FVector>()) });
    out(1, &x)[0] as i32
}
unsafe extern "C" fn single_line(
    _l: *mut c_void,
    s: *mut c_void,
    h: *mut c_void,
    e: *const c_void,
    b: *const c_void,
    t: u32,
    x: *const c_void,
) {
    let mut a = vec![pid(s)];
    a.extend(unsafe { vwords(&*e.cast::<FVector>()) });
    a.extend(unsafe { vwords(&*b.cast::<FVector>()) });
    a.push(t);
    a.extend(unsafe { vwords(&*x.cast::<FVector>()) });
    let o = out(2, &a);
    unsafe {
        let hit = &mut *h.cast::<FCheckResult>();
        hit.time = f32::from_bits(o[0]);
        hit.normal = FVector {
            x: f32::from_bits(o[1]),
            y: f32::from_bits(o[2]),
            z: f32::from_bits(o[3]),
        };
    }
}
unsafe extern "C" fn app_frand() -> f32 {
    f32::from_bits(out(3, &[])[0])
}
unsafe extern "C" fn set_physics(a: *mut c_void, p: u8) {
    assert!(out(4, &[pid(a), p as u32]).is_empty());
    unsafe { (*a.cast::<AActor>()).physics = p };
}
unsafe extern "C" fn cylinder(a: *mut c_void, o: *mut c_void) {
    let x = out(5, &[pid(a)]);
    unsafe {
        *o.cast::<FVector>() = FVector {
            x: f32::from_bits(x[0]),
            y: f32::from_bits(x[1]),
            z: f32::from_bits(x[2]),
        }
    }
}
unsafe extern "C" fn probing(a: *mut c_void, name: u32) -> i32 {
    out(6, &[pid(a), name])[0] as i32
}
unsafe extern "C" fn hear(a: *mut c_void, l: f32, n: *mut c_void) {
    let args = [pid(a), fw(l), pid(n)];
    assert!(out(23, &args).is_empty());
}
unsafe extern "C" fn not_visible(a: *mut c_void) {
    let args = [pid(a)];
    assert!(out(24, &args).is_empty());
}
unsafe extern "C" fn touch(a: *mut c_void, b: *mut c_void) {
    let args = [pid(a), pid(b)];
    assert!(out(25, &args).is_empty());
}
unsafe extern "C" fn safe_normal(v: *const c_void, o: *mut c_void) {
    let x = out(10, &vwords(unsafe { &*v.cast::<FVector>() }));
    unsafe {
        *o.cast::<FVector>() = FVector {
            x: f32::from_bits(x[0]),
            y: f32::from_bits(x[1]),
            z: f32::from_bits(x[2]),
        }
    };
}
unsafe extern "C" fn vector_rotation(v: *const c_void, o: *mut c_void) {
    let a = vwords(unsafe { &*v.cast::<FVector>() });
    let x = out(11, &a);
    unsafe {
        *o.cast::<FRotator>() = FRotator {
            pitch: x[0] as i32,
            yaw: x[1] as i32,
            roll: x[2] as i32,
        }
    }
}
unsafe extern "C" fn rotation_vector(v: *const c_void, o: *mut c_void) {
    hit_kind(12);
    let r = unsafe { &*v.cast::<FRotator>() };
    let trig = |angle: i32| {
        let i = angle.wrapping_shr(2) & 16383;
        let arg = (i as f32) * (2.0 * std::f32::consts::PI / 16384.0);
        (arg as f64).sin() as f32
    };
    let sp = trig(r.pitch);
    let cp = trig(r.pitch.wrapping_add(16384));
    let sy = trig(r.yaw);
    let cy = trig(r.yaw.wrapping_add(16384));
    unsafe {
        *o.cast::<FVector>() = FVector {
            x: cp * cy,
            y: cp * sy,
            z: sp,
        }
    };
}
fn super_class(class: u32) -> Option<u32> {
    match class {
        1 => Some(0),
        2 => Some(1),
        3 | 4 => Some(0),
        _ => None,
    }
}
fn class_is_a(mut class: u32, target: u32) -> bool {
    loop {
        if class == target {
            return true;
        }
        match super_class(class) {
            Some(parent) => class = parent,
            None => return false,
        }
    }
}
unsafe extern "C" fn is_a(a: *mut c_void, k: u32) -> i32 {
    hit_kind(13);
    let class = REPLAY.with(|r| r.borrow().classes[pid(a) as usize]);
    let value = class_is_a(class, k) as i32;
    CLASS_RESULTS.with(|r| r.borrow_mut()[0][usize::from(value != 0)] += 1);
    value
}
unsafe extern "C" fn is_a_class(a: *mut c_void, b: *mut c_void) -> i32 {
    hit_kind(14);
    let (object, parent) = REPLAY.with(|r| {
        let r = r.borrow();
        (r.classes[pid(a) as usize], r.classes[pid(b) as usize])
    });
    let value = class_is_a(object, parent) as i32;
    CLASS_RESULTS.with(|r| r.borrow_mut()[1][usize::from(value != 0)] += 1);
    value
}
unsafe extern "C" fn pawn_head(a: *mut c_void) -> *mut c_void {
    hit_kind(15);
    assert_eq!(pid(a), 4);
    REPLAY.with(|r| r.borrow().actors[1] as *mut c_void)
}
unsafe extern "C" fn team_game(a: *mut c_void) -> i32 {
    hit_kind(16);
    assert_eq!(pid(a), 4);
    REPLAY.with(|r| r.borrow().team_game as i32)
}
unsafe extern "C" fn level_time(a: *mut c_void) -> f32 {
    hit_kind(17);
    assert_eq!(pid(a), 4);
    REPLAY.with(|r| f32::from_bits(r.borrow().time))
}
unsafe extern "C" fn teams(a: *mut c_void, b: *mut c_void) -> i32 {
    hit_kind(18);
    REPLAY.with(|r| {
        let r = r.borrow();
        let team = |id: u32| {
            if id == r.pawn_pri {
                r.pawn_team
            } else if id == r.other_pri {
                r.other_team
            } else {
                0
            }
        };
        i32::from(team(pid(a)) != team(pid(b)))
    })
}
unsafe extern "C" fn gravity(a: *mut c_void) -> f32 {
    hit_kind(19);
    assert_eq!(pid(a), 3);
    REPLAY.with(|r| f32::from_bits(r.borrow().zone_gravity))
}
unsafe extern "C" fn reachable(a: *mut c_void, b: *mut c_void, k: i32) -> i32 {
    out(20, &[pid(a), pid(b), k as u32])[0] as i32
}
unsafe extern "C" fn find_path(
    a: *mut c_void,
    b: *mut c_void,
    s: i32,
    p: *mut *mut c_void,
    c: i32,
) -> i32 {
    let x = out(
        21,
        &[pid(a), pid(b), s as u32, pid(unsafe { *p }), c as u32],
    );
    unsafe {
        *p = REPLAY.with(|r| {
            if x[1] == 0 {
                std::ptr::null_mut()
            } else {
                r.borrow().actors[x[1] as usize] as *mut c_void
            }
        })
    };
    x[0] as i32
}
unsafe extern "C" fn special(a: *mut c_void, b: *mut c_void) -> *mut c_void {
    let args = [pid(a), pid(b)];
    let x = out(26, &args)[0] as usize;
    REPLAY.with(|r| {
        if x == 0 {
            std::ptr::null_mut()
        } else {
            r.borrow().actors[x] as *mut c_void
        }
    })
}
fn services() -> Hp2PawnSenseServices {
    Hp2PawnSenseServices {
        fast_line_check: fast_line,
        single_line_check: single_line,
        app_frand,
        set_physics,
        get_cylinder_extent: cylinder,
        is_probing: probing,
        event_hear_noise: hear,
        event_enemy_not_visible: not_visible,
        event_touch: touch,
        safe_normal,
        vector_rotation,
        rotation_vector,
        is_a,
        is_a_class_of: is_a_class,
        level_pawn_head: pawn_head,
        level_team_game: team_game,
        level_time,
        teams_differ: teams,
        zone_default_gravity_z: gravity,
        actor_reachable: reachable,
        find_path_toward: find_path,
        event_special_handling: special,
    }
}

struct Words {
    w: Vec<u32>,
    p: usize,
}
impl Words {
    fn u(&mut self) -> u32 {
        let x = self.w[self.p];
        self.p += 1;
        x
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
    fn v(&mut self) -> FVector {
        FVector {
            x: self.f(),
            y: self.f(),
            z: self.f(),
        }
    }
}
const CASE_IDS: [&[u32]; 10] = [
    &[0, 1, 2, 3, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    &[0, 1, 2, 3, 4, 5],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    &[0, 1, 2, 3, 4, 5, 6, 7],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    &[0, 1, 2],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    &[0, 1, 2, 3, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
];
fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).expect("hex word")
}
fn actor(p: &mut APawn, actors: &[usize]) {
    p.base.x_level = actors[4] as *mut c_void;
}

#[test]
fn pawn_sense_fixture_replay_exact() {
    let mut fn_counts = [0usize; 10];
    let mut case_sets: Vec<Vec<u32>> = (0..10).map(|_| Vec::new()).collect();
    let mut class_counts = [[0usize; 2]; 2];
    let mut return_counts = [0usize; 10];
    let mut nonzero_returns = [0usize; 10];
    for line in FIXTURE.lines().filter(|l| !l.trim().is_empty()) {
        let raw: Vec<u32> = line
            .split_whitespace()
            .map(|token| if token == "S" { 0x53 } else { hex(token) })
            .collect();
        assert!(raw.len() >= 4);
        assert_eq!(raw[0], 0x43, "record marker");
        let fnid = raw[1] as usize;
        assert!(fnid < 10);
        let case = raw[2];
        fn_counts[fnid] += 1;
        case_sets[fnid].push(case);
        let n = raw[3] as usize;
        assert!(n >= INPUT_WORDS);
        let start = 4;
        let end = start + n;
        let mut sc = Words {
            w: raw[start..end].to_vec(),
            p: 0,
        };
        let phys = sc.u() as u8;
        let flags = sc.u();
        let loc = sc.v();
        let old = sc.v();
        let vel = sc.v();
        let accel = sc.v();
        let rot = FRotator {
            pitch: sc.u() as i32,
            yaw: sc.u() as i32,
            roll: sc.u() as i32,
        };
        let drot = FRotator {
            pitch: sc.u() as i32,
            yaw: sc.u() as i32,
            roll: sc.u() as i32,
        };
        let cr = sc.f();
        let ch = sc.f();
        let eye = sc.f();
        let ar = sc.f();
        let melee = sc.f();
        let avg = sc.f();
        let ds = sc.f();
        let gs = sc.f();
        let air = sc.f();
        let water = sc.f();
        let timer = sc.f();
        let sight = sc.f();
        let pv = sc.f();
        let skill = sc.f();
        let hearing = sc.f();
        let alert = sc.f();
        let stimulus = sc.f();
        let zg = sc.f();
        let enemy = sc.u() as usize;
        let target = sc.u() as usize;
        let inst = sc.u() as usize;
        let level = sc.u() as usize;
        let pri = sc.u() as usize;
        let n1 = sc.v();
        let n1t = sc.f();
        let n1l = sc.f();
        let n2 = sc.v();
        let n2t = sc.f();
        let n2l = sc.f();
        let oloc = sc.v();
        let ocr = sc.f();
        let och = sc.f();
        let vis = sc.u() as u8;
        let ophys = sc.u() as u8;
        let opri = sc.u() as usize;
        let weapon = sc.u() as usize;
        let oflags = sc.u();
        let oweap = sc.u() as usize;
        let pawnclass = sc.u();
        let otherclass = sc.u();
        let pnext = sc.u() as usize;
        let onext = sc.u() as usize;
        let team = sc.u();
        let time = sc.u();
        let pawn_team = sc.u();
        let other_team = sc.u();
        let zone_gravity = sc.u();
        let destination = sc.v();
        let jump_z = sc.f();
        let net_priority = sc.f();
        assert_eq!(sc.p, INPUT_WORDS);
        let mut objects: Vec<APawn> = (0..12).map(|_| unsafe { std::mem::zeroed() }).collect();
        let mut ids: Vec<usize> = (0..12)
            .map(|i| {
                if i == 0 {
                    0
                } else if i == 4 {
                    LEVEL
                } else {
                    (&mut objects[i] as *mut APawn) as usize
                }
            })
            .collect();
        ids[5] = ids[2];
        ids[6] = ids[2];
        ids[7] = ids[2];
        REPLAY.with(|r| {
            let mut r = r.borrow_mut();
            r.actors = ids.clone();
            r.classes = vec![0; 12];
            r.classes[1] = pawnclass;
            r.classes[2] = otherclass;
            r.at = 0;
            r.scripts.clear();
            r.team_game = team;
            r.time = time;
            r.pawn_team = pawn_team;
            r.other_team = other_team;
            r.zone_gravity = zone_gravity;
            r.pawn_pri = pri as u32;
            r.other_pri = opri as u32;
        });
        unsafe {
            (*((ids[3]) as *mut AZoneInfo)).zone_gravity = FVector {
                x: 0.0,
                y: 0.0,
                z: zg,
            };
        }
        objects[2].base.location = oloc;
        objects[2].base.collision_radius = ocr;
        objects[2].base.collision_height = och;
        objects[2].visibility = vis;
        objects[2].base.physics = ophys;
        objects[2].base.x_level = ids[level] as *mut c_void;
        objects[2].player_replication_info = ids[opri] as *mut c_void;
        objects[2].base.set_b_hidden(oflags & 1 != 0);
        objects[2].base.set_b_net_owner(oflags & 2 != 0);
        objects[2].set_b_is_player(oflags & 4 != 0);
        objects[2].weapon = ids[oweap] as *mut c_void;
        objects[2].next_pawn = ids[onext] as *mut c_void;
        objects[2].base.instigator = ids[inst] as *mut c_void;
        let p = &mut objects[1];
        actor(p, &ids);
        p.base.physics = phys;
        p.base.location = loc;
        p.base.old_location = old;
        p.base.velocity = vel;
        p.base.acceleration = accel;
        p.base.rotation = rot;
        p.base.desired_rotation = drot;
        p.base.collision_radius = cr;
        p.base.collision_height = ch;
        p.base.net_priority = net_priority;
        p.destination = destination;
        p.jump_z = jump_z;
        p.base.region.zone = ids[3] as *mut c_void;
        p.base_eye_height = eye;
        p.accel_rate = ar;
        p.melee_range = melee;
        p.avg_physics_time = avg;
        p.desired_speed = ds;
        p.ground_speed = gs;
        p.air_speed = air;
        p.water_speed = water;
        p.move_timer = timer;
        p.sight_radius = sight;
        p.peripheral_vision = pv;
        p.skill = skill;
        p.hearing_threshold = hearing;
        p.alertness = alert;
        p.stimulus = stimulus;
        p.enemy = ids[enemy] as *mut c_void;
        p.move_target = ids[target] as *mut c_void;
        p.base.instigator = ids[inst] as *mut c_void;
        p.base.x_level = ids[level] as *mut c_void;
        p.player_replication_info = ids[pri] as *mut c_void;
        p.noise1spot = n1;
        p.noise1time = n1t;
        p.noise1loudness = n1l;
        p.noise2spot = n2;
        p.noise2time = n2t;
        p.noise2loudness = n2l;
        p.weapon = ids[weapon] as *mut c_void;
        p.next_pawn = ids[pnext] as *mut c_void;
        p.set_b_is_player(flags & 1 != 0);
        p.set_b_can_strafe(flags & 2 != 0);
        p.set_b_reduced_speed(flags & 4 != 0);
        p.set_b_lo_sflag(flags & 8 != 0);
        p.set_b_can_jump(flags & 16 != 0);
        p.set_b_can_do_special(flags & 32 != 0);
        p.base.set_b_hidden(flags & 64 != 0);
        p.set_b_jump_off_pawn(flags & 128 != 0);
        let retpos = end;
        let ret = raw[retpos];
        let snapn = raw[retpos + 1] as usize;
        assert_eq!(snapn, SNAPSHOT_WORDS);
        let snap = &raw[retpos + 2..retpos + 2 + snapn];
        let mut q = retpos + 2 + snapn;
        let logn = raw[q] as usize;
        q += 1;
        let mut scripts = Vec::new();
        for _ in 0..logn {
            assert_eq!(raw[q], 0x53, "script marker");
            let kind = raw[q + 1];
            let ac = raw[q + 2] as usize;
            q += 3;
            let args = raw[q..q + ac].to_vec();
            q += ac;
            let oc = raw[q] as usize;
            q += 1;
            let outs = raw[q..q + oc].to_vec();
            q += oc;
            scripts.push(Script { kind, args, outs });
        }
        assert_eq!(q, raw.len(), "trailing record data");
        REPLAY.with(|r| r.borrow_mut().scripts = scripts);
        HITS.with(|h| *h.borrow_mut() = [0; KINDS]);
        CLASS_RESULTS.with(|h| *h.borrow_mut() = [[0; 2]; 2]);
        let svc = services();
        let mut best = ids[5] as *mut AActor;
        let args = &raw[start + INPUT_WORDS..end];
        assert_eq!(
            args.len(),
            match fnid {
                0 | 5 | 8 =>
                    if fnid == 0 {
                        1
                    } else if fnid == 5 {
                        2
                    } else {
                        3
                    },
                1 | 2 => 3,
                3 => 2,
                4 => 5,
                6 | 7 => 0,
                _ => 1,
            },
            "function argument width"
        );
        let result = unsafe {
            match fnid {
                0 => {
                    set_move_timer(p, f32::from_bits(args[0]));
                    0
                }
                1 => move_toward(&svc, p, read_vec(args, &mut 0)) as u32,
                2 => rotate_toward(&svc, p, read_vec(args, &mut 0)) as u32,
                3 => line_of_sight_to(
                    &svc,
                    p,
                    ids[args[0] as usize] as *mut AActor,
                    args[1] as i32,
                ) as u32,
                4 => can_hear(
                    &svc,
                    p,
                    read_vec(args, &mut 0),
                    f32::from_bits(args[3]),
                    ids[args[4] as usize] as *mut APawn,
                ) as u32,
                5 => {
                    check_noise_hearing(
                        &svc,
                        ids[args[1] as usize] as *mut AActor,
                        f32::from_bits(args[0]),
                    );
                    0
                }
                6 => {
                    check_enemy_visible(&svc, p);
                    0
                }
                7 => pick_wall_adjust(&svc, p) as u32,
                8 => get_net_priority(
                    p,
                    ids[args[0] as usize] as *mut AActor,
                    f32::from_bits(args[1]),
                    f32::from_bits(args[2]),
                )
                .to_bits(),
                _ => {
                    best = ids[args[0] as usize] as *mut AActor;
                    handle_special(&svc, p, &mut best);
                    0
                }
            }
        };
        return_counts[fnid] += 1;
        if ret != 0 {
            nonzero_returns[fnid] += 1;
        }
        assert_eq!(result, ret, "return fn={fnid} case={case}");
        REPLAY.with(|r| {
            assert_eq!(
                r.borrow().at,
                r.borrow().scripts.len(),
                "unconsumed callbacks"
            )
        });
        let mut got = Vec::new();
        got.extend(vwords(&p.base.location));
        got.extend(vwords(&p.base.velocity));
        got.extend(vwords(&p.base.acceleration));
        got.extend([
            p.base.rotation.pitch as u32,
            p.base.rotation.yaw as u32,
            p.base.rotation.roll as u32,
        ]);
        got.extend([
            p.base.desired_rotation.pitch as u32,
            p.base.desired_rotation.yaw as u32,
            p.base.desired_rotation.roll as u32,
        ]);
        got.extend([
            fw(p.move_timer),
            fw(p.desired_speed),
            fw(p.stimulus),
            fw(p.last_seen_time),
        ]);
        got.extend(vwords(&p.last_seen_pos));
        got.extend(vwords(&p.last_seeing_pos));
        got.extend(vwords(&p.destination));
        got.extend([p.base.physics as u32, flags_out(p)]);
        got.extend(vwords(&p.noise1spot));
        got.extend([fw(p.noise1time), fw(p.noise1loudness)]);
        got.extend(vwords(&p.noise2spot));
        got.extend([fw(p.noise2time), fw(p.noise2loudness)]);
        got.extend([pid(p.special_goal), pid(best.cast())]);
        assert_eq!(got, snap, "snapshot fn={fnid} case={case}");
        CLASS_RESULTS.with(|h| {
            for (i, row) in h.borrow().iter().enumerate() {
                for (j, v) in row.iter().enumerate() {
                    class_counts[i][j] += v;
                }
            }
        });
    }
    let expected = [7usize, 16, 6, 16, 8, 11, 3, 17, 7, 11];
    assert_eq!(fn_counts, expected, "exact semantic records per function");
    for (fnid, required) in CASE_IDS.iter().enumerate() {
        assert_eq!(
            case_sets[fnid].len(),
            required.len(),
            "unique semantic IDs fn {fnid}"
        );
        for case in *required {
            assert!(
                case_sets[fnid].contains(case),
                "missing fn {fnid} semantic case {case}"
            );
        }
    }
    assert!(
        class_counts[0][0] > 0 && class_counts[0][1] > 0,
        "IsA true/false outcomes"
    );
    for fnid in [1usize, 2, 3, 4, 7] {
        assert!(
            nonzero_returns[fnid] > 0 && nonzero_returns[fnid] < return_counts[fnid],
            "true/false returns for fn {fnid}"
        );
    }
    for fnid in [0usize, 5, 6, 9] {
        assert_eq!(nonzero_returns[fnid], 0, "void fn {fnid} return");
    }
    assert_eq!(return_counts[8], expected[8], "GetNetPriority return count");
}

fn flags_out(p: &APawn) -> u32 {
    (p.b_is_player() as u32)
        | ((p.b_can_strafe() as u32) << 1)
        | ((p.b_reduced_speed() as u32) << 2)
        | ((p.b_lo_sflag() as u32) << 3)
        | ((p.b_can_jump() as u32) << 4)
        | ((p.b_can_do_special() as u32) << 5)
        | ((p.base.b_hidden() as u32) << 6)
        | ((p.b_jump_off_pawn() as u32) << 7)
}
