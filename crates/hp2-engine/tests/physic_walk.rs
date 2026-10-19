//! Replays walking branch scenarios against the original-engine fixture.

include!("physic_scripted.rs");
thread_local! {
    static WALK_SCRIPT: RefCell<(u32, usize, usize, bool)> = const { RefCell::new((0, 0, 0, false)) };
}
const PHYS_SWIMMING: u8 = 3;

unsafe extern "C" fn walk_move(
    actor: *mut c_void,
    delta: *const FVector,
    rotation: *const FRotator,
    hit: *mut FCheckResult,
) -> i32 {
    // Mirrors the original harness's `hook_move_actor`.
    let (mode, index) = WALK_SCRIPT.with(|s| {
        let mut s = s.borrow_mut();
        let index = s.2;
        s.2 += 1;
        (s.0, index)
    });
    let (delta, rotation) = unsafe { (*delta, *rotation) };
    let blank = |hit: *mut FCheckResult, actor: *mut c_void, time: f32, normal: FVector| unsafe {
        (*hit).time = time;
        (*hit).normal = normal;
        (*hit).location = zero();
        (*hit).actor = actor;
    };
    let ret = match mode {
        1 | 2 => i32::from(index < 2),
        3 | 6 => i32::from(index < 3),
        4 => {
            if index == 0 {
                blank(
                    hit,
                    with(|s| s.pool[0]) as *mut c_void,
                    0.5,
                    FVector {
                        x: 0.0,
                        y: 0.0,
                        z: 0.8,
                    },
                );
            } else if index <= 3 {
                blank(hit, null_mut(), 1.0, zero());
            }
            i32::from(index != 0)
        }
        5 => {
            unsafe {
                (*(actor as *mut AActor)).physics = PHYS_SWIMMING;
            }
            1
        }
        _ => with(|s| i32::from(sm_next(&mut s.svc).is_multiple_of(4))),
    };
    if ret != 0 && matches!(mode, 1 | 2 | 3 | 5 | 6) {
        blank(hit, null_mut(), 1.0, zero());
    }
    with(|s| {
        lg(
            s,
            format!(
                "move {:08x} {:08x} {:08x} {} {} {} {ret}\n",
                bits(delta.x),
                bits(delta.y),
                bits(delta.z),
                rotation.pitch,
                rotation.yaw,
                rotation.roll
            ),
        )
    });
    if ret == 0 && mode != 4 {
        with(|s| scripted_hit(s, unsafe { &mut *hit }));
    }
    ret
}

unsafe extern "C" fn walk_slc(
    _actor: *mut c_void,
    hit: *mut FCheckResult,
    end: *const FVector,
    start: *const FVector,
    trace: i32,
    extent: *const FVector,
) -> i32 {
    // Mirrors the original harness's `hook_slc`.
    let (mode, index, source_null) = WALK_SCRIPT.with(|s| {
        let mut s = s.borrow_mut();
        let index = s.1;
        s.1 += 1;
        (s.0, index, s.3)
    });
    let set = |time: f32, normal: FVector| unsafe {
        (*hit).time = time;
        (*hit).normal = normal;
        (*hit).location = zero();
        (*hit).actor = null_mut();
    };
    let floor = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.9,
    };
    let (ret, scripted) = match mode {
        1 if index == 0 => {
            set(2.0 / 3.0, floor);
            (1, true)
        }
        2 | 3 | 4 | 6 => {
            let hit_call = match mode {
                2 => Some(1),
                3 | 6 => Some(3),
                _ => None,
            };
            let floor_call = match mode {
                2 => 22,
                3 => 7,
                6 => 4,
                _ => 0,
            };
            if hit_call == Some(index) {
                set(
                    0.5,
                    if mode == 2 {
                        FVector {
                            x: 1.0,
                            y: 0.0,
                            z: 0.0,
                        }
                    } else {
                        FVector {
                            x: 0.8,
                            y: 0.6,
                            z: 0.0,
                        }
                    },
                );
                (0, true)
            } else if index == floor_call {
                set(2.0 / 3.0, floor);
                (1, true)
            } else {
                set(1.0, zero());
                (1, true)
            }
        }
        _ => (
            with(|s| i32::from(sm_next(&mut s.svc).is_multiple_of(3))),
            false,
        ),
    };
    let (end, start, extent) = unsafe { (*end, *start, *extent) };
    with(|s| {
        lg(
            s,
            format!(
                "slc {} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {ret}\n",
                i32::from(trace == TRACE_ALL_BLOCKING),
                bits(end.x),
                bits(end.y),
                bits(end.z),
                bits(start.x),
                bits(start.y),
                bits(start.z),
                bits(extent.x),
                bits(extent.y),
                bits(extent.z)
            ),
        )
    });
    with(|s| {
        lg(
            s,
            format!(
                "slc_source {} {}\n",
                pidx(s, _actor as usize),
                i32::from(source_null)
            ),
        )
    });
    if ret == 0 && !scripted {
        with(|s| scripted_hit(s, unsafe { &mut *hit }));
    }
    ret
}
unsafe extern "C" fn walk_slc_no_source(
    actor: *mut c_void,
    hit: *mut FCheckResult,
    end: *const FVector,
    start: *const FVector,
    trace: i32,
    extent: *const FVector,
) -> i32 {
    WALK_SCRIPT.with(|s| s.borrow_mut().3 = true);
    let ret = unsafe { walk_slc(actor, hit, end, start, trace, extent) };
    WALK_SCRIPT.with(|s| s.borrow_mut().3 = false);
    ret
}

// The original harness answers every `IsProbing` from the service RNG and logs each event.
unsafe extern "C" fn walk_is_probing_may_fall(_actor: *mut c_void) -> i32 {
    with(is_probing_impl)
}

unsafe extern "C" fn walk_event(_actor: *mut c_void) {
    with(|s| lg(s, "event_may_fall\n".to_string()));
}

unsafe extern "C" fn walk_debug_fell_out(_actor: *mut c_void) {
    panic!("walk fixture unexpectedly fell out of the world");
}

unsafe extern "C" fn walk_do_jump(_actor: *mut c_void, value: f32) {
    with(|s| lg(s, format!("event_do_jump {:08x}\n", bits(value))));
}

static WALK_SERVICES: Hp2PhysicServices = Hp2PhysicServices {
    is_a: m_is_a,
    is_probing_hit_wall: m_is_probing,
    app_frand: m_app_frand,
    cylinder_extent: m_cyl,
    bsp_surf_special: m_bsp,
    rotation_vector: m_rot,
    move_actor: walk_move,
    single_line_check: walk_slc,
    set_base: m_set_base,
    event_mount: m_event_mount,
    event_hit_wall: m_event_hit_wall,
    event_landed: m_event_landed,
    point_region_zone: m_point_region_zone,
    debugf_flew_out: m_debugf_flew_out,
    destroy_actor: m_destroy_actor,
    event_falling: m_event_falling,
    projectile_max_speed: m_projectile_max_speed,
    log_fell_out_of_world: m_log_fell_out_of_world,
    event_fell_out_of_world: m_event_fell_out_of_world,
    rotation_from_vector: m_rotation_from_vector,
    find_view_target: m_find_view_target,
    event_update_camera: m_event_update_camera,
    event_interpolate_end: m_event_interpolate_end,
    event_finished_interpolation: m_event_finished_interpolation,
    phys_trailer: m_phys_trailer,
    phys_moving_brush: m_phys_moving_brush,
    event_post_touch: m_event_post_touch,
    is_probing_ended_rotation: m_is_probing_ended_rotation,
    event_ended_rotation: m_event_ended_rotation,
    sin_tab: m_sin_tab,
    cos_tab: m_cos_tab,
    is_probing_may_fall: walk_is_probing_may_fall,
    event_may_fall: walk_event,
    event_do_jump: walk_do_jump,
    far_move_actor: m_far_move_actor,
    debugf_fell_out: walk_debug_fell_out,
    single_line_check_no_source: walk_slc_no_source,
    trailer_state: m_trailer_state,
    mesh_bone_coords: m_mesh_bone_coords,
    trailer_far_move_actor: m_trailer_far_move_actor,
};

const WALK_INPUT_WORDS: usize = 59;

fn run_walk(seed: u64, w: &[u32]) -> [u32; OUT_WORDS] {
    assert_eq!(w.len(), WALK_INPUT_WORDS);
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
        (*z).zone_ground_friction = 0.0;
        (*z).set_b_bounce_velocity(w[38] != 0);
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
        (*p).ground_speed = 100.0;
        let mode = w[58];
        (*p).set_b_can_fly(matches!(mode, 1 | 2 | 4 | 5));
        (*p).set_b_can_jump(mode == 6);
        (*p).set_b_avoid_ledges(mode == 3);
        (*p).jump_z = 0.0;
        if mode == 2 {
            (*(subj_p as *mut hp2_abi::engine::generated::APlayerPawn)).set_b_auto_jump(true);
        }
        (*p).base.collision_radius = f(w[25]);
        (*p).base.collision_height = f(w[26]);
        (*p).base.mass = f(w[27]);
        (*p).base.buoyancy = f(w[28]);
        (*(subj_p as *mut ADecoration)).num_landings = w[29] as i32;
        (*(subj_p as *mut ACarcass)).set_b_sliding_carcass(w[30] != 0);
        (*p).base.region.zone = zone_p as *mut c_void;
        (*p).base.region.zone_number = 1;
        (*p).base.role = 1;
    }
    let hit = FCheckResult {
        next: null_mut(),
        actor: pool_or_null(&pool_p, w[55]),
        location: v3(&w[52..55]),
        normal: v3(&w[49..52]),
        primitive: null_mut(),
        time: f(w[48]),
        item: -1,
    };
    WALK_SCRIPT.with(|s| *s.borrow_mut() = (w[58], 0, 0, false));
    unsafe {
        hp2_engine::physic::walk::phys_walking(
            subj_p as *mut APawn,
            &WALK_SERVICES,
            f(w[56]),
            w[57] as i32,
        );
    }
    // The ordered hook trace (digest words 48..50) pins the original's call sequence.
    let (_, _, _, source_null) = WALK_SCRIPT.with(|s| *s.borrow());
    assert!(!source_null, "walk callback left NULL-source state enabled");
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
    out[46] = with(|s| pidx(s, hit.actor as usize)) as u32;
    out[47] = 0;
    let digest = with(|s| fnv1a(s.log.as_bytes()));
    out[48] = (digest >> 32) as u32;
    out[49] = digest as u32;
    out[50] = with(|s| s.log.len()) as u32;
    drop((subj, zone, pool));
    out
}

#[test]
fn physic_walk_matches_original() {
    let text = include_str!("fixtures/physic_walk.txt");
    let mut total = 0usize;
    let mut seen_mode = [false; 7];
    let mut mode_counts = [0usize; 7];
    for (rec, line) in text.lines().enumerate() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let sep = toks.iter().position(|t| *t == "|").expect("separator");
        assert_eq!(toks[0], "phys_walking", "record {rec}");
        let seed = u64::from_str_radix(toks[1], 16).expect("seed");
        let input: Vec<u32> = toks[2..sep]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("input word"))
            .collect();
        let expected: Vec<u32> = toks[sep + 1..]
            .iter()
            .map(|t| u32::from_str_radix(t, 16).expect("output word"))
            .collect();
        assert_eq!(input.len(), WALK_INPUT_WORDS, "record {rec} input width");
        assert_eq!(expected.len(), OUT_WORDS, "record {rec} output width");
        let got = run_walk(seed, &input);
        if let Some(i) = (0..OUT_WORDS).find(|&i| got[i] != expected[i]) {
            panic!(
                "record {rec} seed {seed:016x}: word {i} expected {:08x} got {:08x}",
                expected[i], got[i]
            );
        }
        assert!(input[58] < 7, "record {rec} has unknown walk mode");
        seen_mode[input[58] as usize] = true;
        mode_counts[input[58] as usize] += 1;
        total += 1;
    }
    assert_eq!(total, 1200, "walk fixture case count");
    assert!(seen_mode[0], "iteration-limit branch is absent");
    assert!(seen_mode[1], "positive-time slope-slide branch is absent");
    assert!(seen_mode[2], "auto-jump Predict branch is absent");
    assert!(seen_mode[3], "ledge avoidance branch is absent");
    assert!(seen_mode[4], "step-up branch is absent");
    assert!(seen_mode[5], "water transition branch is absent");
    assert!(seen_mode[6], "may-fall event branch is absent");
    assert_eq!(
        mode_counts,
        [172, 172, 172, 171, 171, 171, 171],
        "walk mode coverage"
    );
}
