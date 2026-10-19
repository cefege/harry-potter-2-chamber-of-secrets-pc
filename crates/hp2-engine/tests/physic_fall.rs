//! Replays `fixtures/physic_fall.txt` against the Rust `physFalling` port.
//! The fixture uses 66 input words and 56 output words per record; fall-only
//! actor/zone properties follow the shared scripted-physics input layout.

include!("physic_scripted.rs");

const FALL_INPUT_WORDS: usize = 66;
const FALL_OUTPUT_WORDS: usize = 56;

fn run_fall(seed: u64, w: &[u32]) -> [u32; FALL_OUTPUT_WORDS] {
    assert_eq!(w.len(), FALL_INPUT_WORDS);
    let mut subj = vec![0u64; 1024];
    let mut zone = vec![0u64; 1024];
    let mut foot_zone = vec![0u64; 1024];
    let foot_zone_p = foot_zone.as_mut_ptr() as usize;
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
        let fz = foot_zone_p as *mut AZoneInfo;
        (*z).set_b_bounce_velocity(w[38] != 0);
        (*z).zone_gravity = v3(&w[32..35]);
        (*z).zone_velocity = v3(&w[35..38]);
        (*z).set_b_water_zone(w[61] != 0);
        (*z).zone_fluid_friction = f(w[62]);
        (*z).zone_terminal_velocity = f(w[63]);
        (*fz).set_b_water_zone(w[64] != 0);
        (*fz).zone_fluid_friction = f(w[62]);
        for k in 0..3 {
            let d = pool_p[k] as *mut ADecoration;
            (*d).base.mass = f(w[40 + 3 * k]);
            (*d).set_b_pushable(w[41 + 3 * k] != 0);
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
        (*p).base.physics = w[12] as u8;
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
        (*p).air_control = f(w[60]);
        (*p).base.collision_radius = f(w[25]);
        (*p).base.collision_height = f(w[26]);
        (*p).base.mass = f(w[27]);
        (*p).base.buoyancy = f(w[28]);
        (*(subj_p as *mut ADecoration)).num_landings = w[29] as i32;
        (*(subj_p as *mut ACarcass)).set_b_sliding_carcass(w[30] != 0);
        (*p).base.region.zone = zone_p as *mut c_void;
        (*p).base.region.zone_number = w[58] as u8;
        (*p).base.role = w[59] as u8;
        (*p).foot_region.zone = foot_zone_p as *mut c_void;
        (*p).base.set_b_bounce(w[65] != 0);
    }
    let hit = FCheckResult {
        next: null_mut(),
        actor: null_mut(),
        location: zero(),
        normal: zero(),
        primitive: null_mut(),
        time: 1.0,
        item: -1,
    };
    unsafe {
        hp2_engine::physic::fall::phys_falling(
            subj_p as *mut AActor,
            &SERVICES,
            f(w[56]),
            w[57] as i32,
        );
    }
    let mut out = [0u32; FALL_OUTPUT_WORDS];
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
        out[31] = with(|s| s.subj_class);
        let z = zone_p as *const AZoneInfo;
        put3(&mut out[32..35], (*z).zone_gravity);
        put3(&mut out[35..38], (*z).zone_velocity);
        out[38] = u32::from((*z).b_bounce_velocity());
    }
    out[39] = bits(hit.time);
    put3(&mut out[40..43], hit.normal);
    put3(&mut out[43..46], hit.location);
    out[46] = with(|s| pidx(s, hit.actor as usize)) as u32;
    let log = with(|s| s.log.clone());
    out[47] = u32::from(log.contains("slc "))
        | (u32::from(log.contains("move ")) * 2)
        | (u32::from(log.contains("event_hit_wall ")) * 4)
        | (u32::from(log.contains("event_landed ")) * 8)
        | (u32::from(log.contains("event_mount ")) * 16);
    if w[65] != 0 && log.contains("event_hit_wall ") {
        out[47] |= 32;
    }
    if log.contains("event_fell_out_of_world\n") {
        out[47] |= 64;
    }
    let digest = with(|s| fnv1a(s.log.as_bytes()));
    out[48] = (digest >> 32) as u32;
    out[49] = digest as u32;
    out[50] = with(|s| s.log.len()) as u32;
    out[51] = w[58];
    out[52] = w[59];
    out[53] = w[61];
    out[54] = bits(f(w[62]));
    out[55] = bits(f(w[63]));
    drop((subj, zone, foot_zone, pool));
    out
}

#[test]
fn physic_fall_matches_original() {
    let text = include_str!("fixtures/physic_fall.txt");
    let mut total = 0usize;
    let mut seen = [false; 6];
    let mut branch_mask = 0u32;
    for (rec, line) in text.lines().enumerate() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let sep = toks.iter().position(|t| *t == "|").expect("separator");
        assert_eq!(toks[0], "phys_falling", "record {rec}");
        let seed = u64::from_str_radix(toks[1], 16).expect("seed");
        let input: Vec<u32> = toks[2..sep]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("input word"))
            .collect();
        let expected: Vec<u32> = toks[sep + 1..]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("output word"))
            .collect();
        assert_eq!(input.len(), FALL_INPUT_WORDS, "record {rec} input width");
        assert_eq!(
            expected.len(),
            FALL_OUTPUT_WORDS,
            "record {rec} output width"
        );
        let got = run_fall(seed, &input);
        if let Some(i) = (0..FALL_OUTPUT_WORDS).find(|&i| got[i] != expected[i]) {
            panic!(
                "record {rec} seed {seed:016x}: word {i} expected {:08x} got {:08x}",
                expected[i], got[i]
            );
        }
        seen[0] |= input[58] == 0;
        seen[1] |= input[13] != 0;
        seen[2] |= input[31] == 3;
        seen[3] |= input[61] != 0;
        seen[4] |= input[64] != 0;
        seen[5] |= input[65] != 0;
        branch_mask |= expected[47];
        total += 1;
    }
    assert_eq!(total, 1200);
    assert_eq!(branch_mask & 0x03, 0x03, "trace/move classes");
    assert_ne!(branch_mask & 0x04, 0, "hit-wall class");
    assert_ne!(branch_mask & 0x08, 0, "landed class");
    assert_ne!(branch_mask & 0x20, 0, "bounce class");
    assert_ne!(branch_mask & 0x40, 0, "fell-out event class");
    assert!(seen.into_iter().all(|x| x), "fall branch classes: {seen:?}");
}
