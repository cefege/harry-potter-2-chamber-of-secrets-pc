include!("physic_scripted.rs");

const SWIM_FUNCTIONS: [&str; 5] = [
    "start_swimming",
    "phys_flying",
    "swim",
    "find_water_line",
    "phys_swimming",
];

fn swim_arg_words(name: &str) -> usize {
    match name {
        "start_swimming" => 6,
        "phys_flying" | "phys_swimming" => 2,
        "swim" => 3,
        "find_water_line" => 6,
        other => panic!("unknown swim fixture function {other}"),
    }
}

fn swim_point_region_zone(s: &mut Scene, location: FVector) -> *mut c_void {
    let current = unsafe { &*(s.zone as *const AZoneInfo) };
    let current_water = current.b_water_zone();
    let water_zone = if location.z < 0.0 {
        if current_water { s.zone } else { s.pool[2] }
    } else if current_water {
        s.pool[2]
    } else {
        s.zone
    };
    let water = location.z < 0.0;
    lg(
        s,
        format!(
            "point_region {:08x} {}\n",
            bits(location.x),
            i32::from(water)
        ),
    );
    water_zone as *mut c_void
}

unsafe extern "C" fn m_swim_point_region_zone(
    _actor: *mut c_void,
    location: *const FVector,
) -> *mut c_void {
    let location = unsafe { *location };
    with(|s| swim_point_region_zone(s, location))
}

fn swim_services() -> Hp2PhysicServices {
    // The table contains only function pointers; cloning its entries gives this
    // replay the same deterministic engine mocks while replacing PointRegion.
    let mut services = unsafe { std::ptr::read(&SERVICES) };
    services.point_region_zone = m_swim_point_region_zone;
    services
}

fn run_swim_case(name: &str, seed: u64, w: &[u32]) -> [u32; OUT_WORDS] {
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
    let actor = subj_p as *mut APawn;
    with(|s| {
        s.svc = seed;
        s.log.clear();
        s.subj = subj_p;
        s.subj_class = w[31];
        s.zone = zone_p;
        s.pool = pool_p;
        s.pool_class = [w[39], w[42], w[45]];
    });
    let services = swim_services();
    unsafe {
        let z = zone_p as *mut AZoneInfo;
        (*z).zone_gravity = v3(&w[32..35]);
        (*z).zone_velocity = v3(&w[35..38]);
        (*z).set_b_bounce_velocity(w[38] != 0);
        (*z).set_b_water_zone(w[38] != 0);
        for k in 0..3 {
            let d = pool_p[k] as *mut ADecoration;
            (*d).base.mass = f(w[40 + 3 * k]);
            (*d).set_b_pushable(w[41 + 3 * k] != 0);
        }
        (*(pool_p[2] as *mut AZoneInfo)).set_b_water_zone(w[38] == 0);
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
        (*p).base.collision_radius = f(w[25]);
        (*p).base.collision_height = f(w[26]);
        (*p).base.mass = f(w[27]);
        (*p).base.buoyancy = f(w[28]);
        (*(subj_p as *mut ADecoration)).num_landings = w[29] as i32;
        (*(subj_p as *mut ACarcass)).set_b_sliding_carcass(w[30] != 0);
        (*p).base.region.zone = zone_p as *mut c_void;
        (*p).head_region.zone = zone_p as *mut c_void;
    }
    let mut hit = FCheckResult {
        next: null_mut(),
        actor: pool_or_null(&pool_p, w[55]),
        location: v3(&w[52..55]),
        normal: v3(&w[49..52]),
        primitive: null_mut(),
        time: f(w[48]),
        item: -1,
    };
    let arg = &w[ARG0..];
    let mut line_end = zero();
    let ret = unsafe {
        match name {
            "start_swimming" => {
                physic::swim::start_swimming(
                    actor,
                    &services,
                    v3(&arg[0..3]),
                    f(arg[3]),
                    f(arg[4]),
                    arg[5] as i32,
                );
                0
            }
            "phys_flying" => {
                physic::swim::phys_flying(actor, &services, f(arg[0]), arg[1] as i32);
                0
            }
            "swim" => bits(physic::swim::swim(
                actor,
                &services,
                v3(&arg[0..3]),
                &mut hit,
            )),
            "find_water_line" => {
                line_end = v3(&arg[3..6]);
                physic::swim::find_water_line(actor, &services, v3(&arg[0..3]), &mut line_end);
                0
            }
            "phys_swimming" => {
                physic::swim::phys_swimming(actor, &services, f(arg[0]), arg[1] as i32);
                0
            }
            other => panic!("unknown swim fixture function {other}"),
        }
    };
    let mut out = [0u32; OUT_WORDS];
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
    if name == "find_water_line" {
        put3(&mut out[43..46], line_end);
    }
    out[46] = with(|s| pidx(s, hit.actor as usize)) as u32;
    out[47] = ret;
    let digest = with(|s| fnv1a(s.log.as_bytes()));
    out[48] = (digest >> 32) as u32;
    out[49] = digest as u32;
    out[50] = with(|s| s.log.len()) as u32;
    drop((subj, zone, pool));
    out
}

#[test]
fn physic_swim_matches_original() {
    let text = include_str!("fixtures/physic_swim.txt");
    let mut per_fn = BTreeMap::<&str, usize>::new();
    let mut branch_values = BTreeMap::<&str, BTreeMap<u32, usize>>::new();
    for (record, line) in text.lines().enumerate() {
        let toks: Vec<_> = line.split_whitespace().collect();
        let sep = toks
            .iter()
            .position(|t| *t == "|")
            .unwrap_or_else(|| panic!("record {record}: missing separator"));
        let name = toks[0];
        let seed = u64::from_str_radix(toks[1], 16).expect("seed");
        let input: Vec<u32> = toks[2..sep]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("input word"))
            .collect();
        let expected: Vec<u32> = toks[sep + 1..]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("output word"))
            .collect();
        assert_eq!(
            input.len(),
            ARG0 + swim_arg_words(name),
            "record {record} ({name}): input width"
        );
        assert_eq!(
            expected.len(),
            OUT_WORDS,
            "record {record} ({name}): output width"
        );
        let got = run_swim_case(name, seed, &input);
        if let Some(i) = (0..OUT_WORDS).find(|&i| got[i] != expected[i]) {
            panic!(
                "record {record} {name} seed {seed:016x}: word {i} expected {:08x} got {:08x}",
                expected[i], got[i]
            );
        }
        *per_fn.entry(name).or_default() += 1;
        *branch_values
            .entry(name)
            .or_default()
            .entry(expected[12])
            .or_default() += 1;
    }
    assert_eq!(text.lines().count(), 5 * 1200);
    for name in SWIM_FUNCTIONS {
        assert_eq!(per_fn.get(name).copied(), Some(1200), "{name} case count");
        assert!(
            branch_values.get(name).unwrap().len() >= 2,
            "{name} physics branch coverage"
        );
    }
}
