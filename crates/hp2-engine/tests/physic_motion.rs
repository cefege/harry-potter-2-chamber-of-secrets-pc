//! Replays `fixtures/physic_motion.txt` against the Rust motion physics ports.
include!("physic_scripted.rs");

const MOTION_INPUT_WORDS: usize = 67;
const MOTION_OUTPUT_WORDS: usize = 57;
const FLOOR_OFFSET: usize = 1184;

fn motion_log_mask(log: &str) -> u32 {
    [
        ("move ", 1),
        ("slc ", 2),
        ("event_hit_wall ", 4),
        ("event_falling", 8),
        ("set_base ", 16),
        ("slc_source -1 1\n", 32),
    ]
    .into_iter()
    .filter(|(needle, _)| log.contains(needle))
    .fold(0, |mask, (_, bit)| mask | bit)
}

fn run_motion(seed: u64, w: &[u32]) -> [u32; MOTION_OUTPUT_WORDS] {
    assert_eq!(w.len(), MOTION_INPUT_WORDS);
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
    let action = w[58];
    with(|s| {
        s.svc = seed;
        s.log.clear();
        s.subj = subj_p;
        s.subj_class = if action <= 1 { 0 } else { 1 };
        s.zone = zone_p;
        s.pool = pool_p;
        s.pool_class = [w[39], w[42], w[45]];
        s.projectile_max_speed = f(w[60]);
        s.motion_action = action;
        s.motion_mover_pool = [w[64] != 0, false, false];
    });
    unsafe {
        let z = zone_p as *mut AZoneInfo;
        (*z).zone_gravity = v3(&w[32..35]);
        (*z).zone_velocity = v3(&w[35..38]);
        (*z).set_b_bounce_velocity(w[38] != 0);
        (*z).zone_ground_friction = f(w[65]);
        (*z).zone_fluid_friction = f(w[66]);
        for k in 0..3 {
            let p = pool_p[k] as *mut ADecoration;
            (*p).base.mass = f(w[40 + 3 * k]);
            (*p).set_b_pushable(w[41 + 3 * k] != 0);
        }
        let p = subj_p as *mut APawn;
        (*p).base.location = v3(&w[0..3]);
        (*p).base.rotation = FRotator {
            pitch: w[3] as i32,
            yaw: w[4] as i32,
            roll: w[5] as i32,
        };
        (*p).base.velocity = v3(&w[6..9]);
        (*p).base.acceleration = v3(&w[9..12]);
        (*p).base.physics = match action {
            0 => 6,
            1 => 7,
            _ => 10,
        };
        (*p).base.set_b_is_pawn(w[13] != 0);
        (*p).base.set_b_just_teleported(w[14] != 0);
        (*p).set_b_is_walking(w[15] != 0);
        (*p).set_b_from_wall(w[16] != 0);
        (*p).destination = v3(&w[17..20]);
        (*p).accel_rate = f(w[20]);
        (*p).desired_speed = f(w[21]);
        (*p).max_step_height = f(w[22]);
        (*p).max_mount_height = f(w[23]);
        (*p).min_hit_wall = f(w[24]);
        (*p).base.collision_radius = f(w[25]);
        (*p).base.collision_height = f(w[26]);
        (*p).base.mass = f(w[27]);
        (*p).base.buoyancy = f(w[28]);
        (*(subj_p as *mut ADecoration)).num_landings = w[29] as i32;
        (*(subj_p as *mut ACarcass)).set_b_sliding_carcass(w[30] != 0);
        (*p).base.region.zone = zone_p as *mut c_void;
        (*p).base.region.zone_number = w[59] as u8;
        (*p).base.set_b_bounce(w[64] != 0);
        let floor = (subj_p as *mut u8).add(FLOOR_OFFSET).cast::<FVector>();
        *floor = v3(&w[61..64]);
    }
    let mut hit = blank_hit(1.0);
    let mut ret = 0;
    unsafe {
        let actor = subj_p as *mut AActor;
        let pawn = subj_p as *mut APawn;
        match action {
            0 => physic::motion::phys_projectile(actor, &SERVICES, f(w[56]), w[57] as i32),
            1 => physic::motion::phys_rolling(actor, &SERVICES, f(w[56]), w[57] as i32),
            2 => physic::motion::phys_spider(pawn, &SERVICES, f(w[56]), w[57] as i32),
            3 => ret = physic::motion::check_floor(pawn, &SERVICES, v3(&w[61..64]), &mut hit),
            _ => {
                ret = physic::motion::find_new_floor(
                    pawn,
                    &SERVICES,
                    v3(&w[0..3]),
                    f(w[56]),
                    f(w[56]),
                    w[57] as i32,
                )
            }
        }
    }
    let mut out = [0u32; MOTION_OUTPUT_WORDS];
    unsafe {
        let p = subj_p as *const APawn;
        let a = &(*p).base;
        put3(&mut out[0..3], a.location);
        out[3] = a.rotation.pitch as u32;
        out[4] = a.rotation.yaw as u32;
        out[5] = a.rotation.roll as u32;
        put3(&mut out[6..9], a.velocity);
        put3(&mut out[9..12], a.acceleration);
        out[12] = u32::from(a.physics);
        out[13] = u32::from(a.b_is_pawn());
        out[14] = u32::from(a.b_just_teleported());
        out[15] = u32::from((*p).b_is_walking());
        out[16] = u32::from((*p).b_from_wall());
        put3(&mut out[17..20], (*p).destination);
        out[20] = bits((*p).accel_rate);
        out[21] = bits((*p).desired_speed);
        out[22] = bits((*p).max_step_height);
        out[23] = bits((*p).max_mount_height);
        out[24] = bits((*p).min_hit_wall);
        out[25] = bits(a.collision_radius);
        out[26] = bits(a.collision_height);
        out[27] = bits(a.mass);
        out[28] = bits(a.buoyancy);
        out[29] = (*(subj_p as *const ADecoration)).num_landings as u32;
        out[30] = u32::from((*(subj_p as *const ACarcass)).b_sliding_carcass());
        out[31] = match action {
            0 => u32::MAX,
            1 => 0,
            _ => 1,
        };
        let z = zone_p as *const AZoneInfo;
        put3(&mut out[32..35], (*z).zone_gravity);
        put3(&mut out[35..38], (*z).zone_velocity);
        out[38] = u32::from((*z).b_bounce_velocity());
        put3(
            &mut out[47..50],
            *((subj_p as *const u8).add(FLOOR_OFFSET).cast::<FVector>()),
        );
    }
    out[39] = bits(hit.time);
    put3(&mut out[40..43], hit.normal);
    put3(&mut out[43..46], hit.location);
    out[46] = with(|s| pidx(s, hit.actor as usize)) as u32;
    let log = with(|s| s.log.clone());
    let digest = fnv1a(log.as_bytes());
    out[50] = (digest >> 32) as u32;
    out[51] = digest as u32;
    out[52] = log.len() as u32;
    out[53] = action;
    out[54] = ret as u32;
    out[55] = w[60];
    out[56] = motion_log_mask(&log);
    out
}

#[test]
fn physic_motion_matches_original() {
    let text = include_str!("fixtures/physic_motion.txt");
    let mut total = 0;
    let mut seen = [false; 5];
    let mut branches = 0u32;
    for (rec, line) in text.lines().enumerate() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let sep = toks.iter().position(|t| *t == "|").expect("separator");
        assert_eq!(toks[0], "phys_motion", "record {rec}");
        let seed = u64::from_str_radix(toks[1], 16).expect("seed");
        let input: Vec<u32> = toks[2..sep]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("input"))
            .collect();
        let expected: Vec<u32> = toks[sep + 1..]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("output"))
            .collect();
        assert_eq!(input.len(), MOTION_INPUT_WORDS, "record {rec} input width");
        assert_eq!(
            expected.len(),
            MOTION_OUTPUT_WORDS,
            "record {rec} output width"
        );
        let got = run_motion(seed, &input);
        if let Some(i) = (0..MOTION_OUTPUT_WORDS).find(|&i| got[i] != expected[i]) {
            panic!(
                "record {rec} seed {seed:016x}: word {i} expected {:08x} got {:08x}",
                expected[i], got[i]
            );
        }
        let action = input[58] as usize;
        seen[action] = true;
        branches |= expected[56];
        total += 1;
    }
    assert_eq!(total, 1200);
    assert!(seen.into_iter().all(|x| x), "motion actions: {seen:?}");
    assert_eq!(
        branches & 0b11_1111,
        0b11_1111,
        "motion branch classes: {branches:06b}"
    );
}
