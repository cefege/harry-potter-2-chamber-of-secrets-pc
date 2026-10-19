//! Replay the dispatch-family oracle cases and cover callback-driven physics changes.
include!("physic_scripted.rs");

const DISPATCH_INPUT_WORDS: usize = 67;
const DISPATCH_OUTPUT_WORDS: usize = 57;

fn run_dispatch(seed: u64, w: &[u32]) -> [u32; DISPATCH_OUTPUT_WORDS] {
    assert_eq!(w.len(), DISPATCH_INPUT_WORDS);
    let action = w[58];
    let mut subj = vec![0u64; 1024];
    let mut zone = vec![0u64; 1024];
    let mut pool = [vec![0u64; 1024], vec![0u64; 1024], vec![0u64; 1024]];
    let subj_p = subj.as_mut_ptr() as usize;
    let zone_p = zone.as_mut_ptr() as usize;
    let pool_p = [
        pool[0].as_mut_ptr() as usize,
        pool[1].as_mut_ptr() as usize,
        pool[2].as_mut_ptr() as usize,
    ];
    with(|s| {
        s.svc = seed;
        s.log.clear();
        s.subj = subj_p;
        s.subj_class = w[31];
        s.zone = zone_p;
        s.pool = pool_p;
        s.pool_class = [w[39], w[42], w[45]];
    });
    unsafe {
        let z = zone_p as *mut AZoneInfo;
        (*z).zone_gravity = v3(&w[32..35]);
        (*z).zone_velocity = v3(&w[35..38]);
        (*z).set_b_bounce_velocity(w[38] != 0);
        for k in 0..3 {
            let p = pool_p[k] as *mut ADecoration;
            (*p).base.mass = f(w[40 + 3 * k]);
            (*p).set_b_pushable(w[41 + 3 * k] != 0);
        }
        let a = subj_p as *mut AActor;
        let p = subj_p as *mut APawn;
        (*a).location = v3(&w[0..3]);
        (*a).old_location = v3(&w[17..20]);
        (*a).rotation = FRotator {
            pitch: w[3] as i32,
            yaw: w[4] as i32,
            roll: w[5] as i32,
        };
        (*a).velocity = v3(&w[6..9]);
        (*a).acceleration = v3(&w[9..12]);
        (*a).physics = match action {
            2 | 3 => 8,
            _ => w[12] as u8,
        };
        (*a).set_b_is_pawn(w[13] != 0);
        (*a).set_b_just_teleported(w[14] != 0);
        (*a).rotation_rate = FRotator {
            pitch: (w[3] as i32) % 1200,
            yaw: (w[4] as i32) % 2400,
            roll: (w[5] as i32) % 800,
        };
        (*a).desired_rotation = FRotator {
            pitch: (w[3] as i32) + 500,
            yaw: (w[4] as i32) - 700,
            roll: (w[5] as i32) + 300,
        };
        (*a).set_b_rotate_to_desired(w[66] != 0);
        (*a).set_b_fixed_rotation_dir(w[57] & 1 != 0);
        (*a).collision_radius = f(w[25]);
        (*a).collision_height = f(w[26]);
        (*a).mass = f(w[27]);
        (*a).buoyancy = f(w[28]);
        (*p).destination = v3(&w[17..20]);
        (*p).desired_speed = f(w[21]);
        (*p).max_step_height = f(w[22]);
        (*p).max_mount_height = f(w[23]);
        (*p).min_hit_wall = f(w[24]);
        (*(subj_p as *mut ADecoration)).num_landings = w[29] as i32;
        (*(subj_p as *mut ACarcass)).set_b_sliding_carcass(w[30] != 0);
        (*a).region.zone = zone_p as *mut c_void;
        (*p).step_height = f(w[59]);
        (*p).step_width = f(w[60]);
        (*p).avg_physics_time = f(w[64]);
        (*p).move_timer = f(w[65]);
        (*p).accel_rate = f(w[20]);
        (*p).set_b_is_walking(w[15] != 0);
        (*p).set_b_from_wall(w[16] != 0);
    }
    let hit = blank_hit(1.0);
    let mut out = [0u32; DISPATCH_OUTPUT_WORDS];
    unsafe {
        let a = subj_p as *mut AActor;
        let p = subj_p as *mut APawn;
        match action {
            0 => physic::dispatch::find_base(a, &SERVICES),
            1 => physic::dispatch::set_physics(
                a,
                &SERVICES,
                (w[57] % 12) as u8,
                if w[66] != 0 {
                    pool_p[0] as *mut AActor
                } else {
                    std::ptr::null_mut()
                },
            ),
            2 => physic::dispatch::perform_physics_actor(a, &SERVICES, f(w[56])),
            3 => physic::dispatch::perform_physics_pawn(p, &SERVICES, f(w[56])),
            4 => physic::dispatch::physics_rotation_actor(a, &SERVICES, f(w[56])),
            _ => physic::dispatch::physics_rotation_pawn(p, &SERVICES, f(w[56]), v3(&w[61..64])),
        }
        let a = &*(subj_p as *const AActor);
        let p = &*(subj_p as *const APawn);
        put3(&mut out[0..3], a.location);
        out[3] = a.rotation.pitch as u32;
        out[4] = a.rotation.yaw as u32;
        out[5] = a.rotation.roll as u32;
        put3(&mut out[6..9], a.velocity);
        put3(&mut out[9..12], a.acceleration);
        out[12] = u32::from(a.physics);
        out[13] = u32::from(a.b_is_pawn());
        out[14] = u32::from(a.b_just_teleported());
        out[15] = u32::from(p.b_is_walking());
        out[16] = u32::from(p.b_from_wall());
        put3(&mut out[17..20], p.destination);
        out[20] = bits(p.accel_rate);
        out[21] = bits(p.desired_speed);
        out[22] = bits(p.max_step_height);
        out[23] = bits(p.max_mount_height);
        out[24] = bits(p.min_hit_wall);
        out[25] = bits(a.collision_radius);
        out[26] = bits(a.collision_height);
        out[27] = bits(a.mass);
        out[28] = bits(a.buoyancy);
        out[29] = (*(subj_p as *const ADecoration)).num_landings as u32;
        out[30] = u32::from((*(subj_p as *const ACarcass)).b_sliding_carcass());
        out[31] = w[31];
        let z = &*(zone_p as *const AZoneInfo);
        put3(&mut out[32..35], z.zone_gravity);
        put3(&mut out[35..38], z.zone_velocity);
        out[38] = u32::from(z.b_bounce_velocity());
        out[39] = bits(hit.time);
        put3(&mut out[40..43], hit.normal);
        put3(&mut out[43..46], hit.location);
        out[46] = with(|s| pidx(s, hit.actor as usize)) as u32;
        out[47] = bits(p.step_height);
        out[48] = bits(p.step_width);
        out[49] = bits(a.saved_pre_pivot_z);
        out[53] = action;
        out[54] = bits(p.avg_physics_time);
        out[55] = bits(p.move_timer);
        out[56] = u32::from(a.b_fixed_rotation_dir());
    }
    let log = with(|s| s.log.clone());
    let digest = fnv1a(log.as_bytes());
    out[50] = (digest >> 32) as u32;
    out[51] = digest as u32;
    out[52] = log.len() as u32;
    out
}

#[test]
fn physic_dispatch_matches_original() {
    let text = include_str!("fixtures/physic_dispatch.txt");
    let mut total = 0;
    let mut seen = [false; 6];
    let mut set_floor = [false; 2];
    let mut set_reset = false;
    let mut pawn_class = [false; 2];
    let mut actor_rotation_guard = [false; 2];
    let mut pawn_roll_rate = [false; 2];
    let mut find_trace = [false; 2];
    let mut find_rebase = false;
    for (rec, line) in text.lines().enumerate() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let sep = toks.iter().position(|t| *t == "|").expect("separator");
        assert_eq!(toks[0], "phys_dispatch", "record {rec}");
        let seed = u64::from_str_radix(toks[1], 16).expect("seed");
        let input: Vec<u32> = toks[2..sep]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("input"))
            .collect();
        let expected: Vec<u32> = toks[sep + 1..]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("output"))
            .collect();
        assert_eq!(
            input.len(),
            DISPATCH_INPUT_WORDS,
            "record {rec} input width"
        );
        assert_eq!(
            expected.len(),
            DISPATCH_OUTPUT_WORDS,
            "record {rec} output width"
        );
        let action = input[58] as usize;
        seen[action] = true;
        let actual = run_dispatch(seed, &input);
        assert_eq!(actual, expected.as_slice(), "record {rec}");
        if action == 0 {
            let log = with(|s| s.log.clone());
            let trace = log
                .lines()
                .find(|line| line.starts_with("slc "))
                .expect("FindBase trace");
            find_trace[usize::from(trace.ends_with(" 1"))] = true;
            find_rebase |= log.contains("set_base ");
        } else if action == 1 {
            let target = input[57] % 12;
            if input[12] as usize != target as usize {
                set_floor[(input[66] != 0) as usize] = true;
                set_reset |= target == 0 || target == 5;
            }
        } else if action == 3 {
            pawn_class[(input[31] == 2) as usize] = true;
        } else if action == 4 {
            let rotate = input[66] != 0;
            let fixed = input[57] & 1 != 0;
            actor_rotation_guard[(!rotate && !fixed) as usize] = true;
        } else if action == 5 {
            pawn_roll_rate[((input[5] as i32) % 800 > 0) as usize] = true;
        }
        total += 1;
    }
    assert_eq!(total, 1200);
    assert!(seen.into_iter().all(|x| x));
    assert!(set_floor.into_iter().all(|x| x));
    assert!(set_reset);
    assert!(pawn_class.into_iter().all(|x| x));
    assert!(actor_rotation_guard.into_iter().all(|x| x));
    assert!(pawn_roll_rate.into_iter().all(|x| x));
    assert!(find_trace.into_iter().all(|x| x));
    assert!(find_rebase);
}

unsafe extern "C" fn mutate_physics_on_base(actor: *mut c_void, _base: *mut c_void) {
    unsafe {
        (*(actor as *mut AActor)).physics = 2;
    }
}

#[test]
fn set_physics_preserves_motion_after_reentrant_base_callback() {
    let mut actor = unsafe { std::mem::zeroed::<AActor>() };
    actor.physics = 1;
    actor.velocity = FVector {
        x: 4.0,
        y: -3.0,
        z: 2.0,
    };
    actor.acceleration = FVector {
        x: 1.0,
        y: 2.0,
        z: -1.0,
    };
    let floor = Box::into_raw(Box::new(unsafe { std::mem::zeroed::<AActor>() }));
    let mut svc = unsafe { std::ptr::read(&SERVICES) };
    svc.set_base = mutate_physics_on_base;
    unsafe {
        physic::dispatch::set_physics(&mut actor, &svc, 0, floor);
    }
    assert_eq!(actor.physics, 2);
    assert_eq!(actor.velocity.x, 4.0);
    assert_eq!(actor.acceleration.z, -1.0);
    unsafe {
        drop(Box::from_raw(floor));
    }
}
