//! Replays interpolation pause-rotation records captured from the original engine.
include!("physic_scripted.rs");

use hp2_abi::FCoords;
use hp2_abi::engine::generated::{AInterpolationManager, AInterpolationPoint};
use hp2_engine::physic::interp::{get_desired_rotation_at_pause, get_desired_rotation_at_position};

#[test]
fn interpolation_pause_and_position_rotations_match_original() {
    let text = include_str!("fixtures/physic_interp.txt");
    let cases: [(i32, i32, i32); 4] = [
        (0, 0, 0),
        (16384, -8192, 7),
        (-32768, 65535, -1),
        (1073741824, -1073741824, 123456789),
    ];
    let mut count = 0;
    for line in text.lines() {
        if !line.starts_with("interp_pause ") && !line.starts_with("interp_position ") {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        assert_eq!(parts.len(), 2, "bad record: {line}");
        let input: Vec<&str> = parts[0].split_whitespace().collect();
        let output: Vec<&str> = parts[1].split_whitespace().collect();
        assert_eq!(input.len(), 5, "bad input record: {line}");
        assert_eq!(output.len(), 3, "bad output record: {line}");
        let case = count;
        assert!(case < cases.len() * 2, "unexpected extra case: {line}");
        let fn_name = if case < cases.len() {
            "interp_pause"
        } else {
            "interp_position"
        };
        assert_eq!(input[0], fn_name, "unexpected case: {line}");
        let rotation_case = case % cases.len();
        let words: [u32; 3] = [
            u32::from_str_radix(input[2], 16).unwrap(),
            u32::from_str_radix(input[3], 16).unwrap(),
            u32::from_str_radix(input[4], 16).unwrap(),
        ];
        assert_eq!(
            words,
            [
                cases[rotation_case].0 as u32,
                cases[rotation_case].1 as u32,
                cases[rotation_case].2 as u32
            ],
            "wrong input case: {line}"
        );
        let mut memory = vec![0u64; 1024];
        let point = memory.as_mut_ptr().cast::<AInterpolationPoint>();
        unsafe {
            core::ptr::addr_of_mut!((*point).base.base.rotation).write_unaligned(FRotator {
                pitch: words[0] as i32,
                yaw: words[1] as i32,
                roll: words[2] as i32,
            });
            let got = if case < cases.len() {
                get_desired_rotation_at_pause(
                    point,
                    &SERVICES,
                    0,
                    FVector {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                )
            } else {
                get_desired_rotation_at_position(
                    point,
                    &SERVICES,
                    0,
                    0,
                    FVector {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                )
            };
            assert_eq!(
                got.pitch as u32,
                u32::from_str_radix(output[0], 16).unwrap(),
                "pitch: {line}"
            );
            assert_eq!(
                got.yaw as u32,
                u32::from_str_radix(output[1], 16).unwrap(),
                "yaw: {line}"
            );
            assert_eq!(
                got.roll as u32,
                u32::from_str_radix(output[2], 16).unwrap(),
                "roll: {line}"
            );
        }
        count += 1;
    }
    assert_eq!(count, cases.len() * 2);
}

#[test]
fn interpolation_manager_idle_matches_original() {
    let text = include_str!("fixtures/physic_interp.txt");
    let mut count = 0;
    for line in text
        .lines()
        .filter(|line| line.starts_with("interp_manager_idle "))
    {
        let parts: Vec<&str> = line.split('|').collect();
        assert_eq!(parts.len(), 2, "bad record: {line}");
        let input: Vec<&str> = parts[0].split_whitespace().collect();
        let output: Vec<&str> = parts[1].split_whitespace().collect();
        assert_eq!(input.len(), 6, "bad input record: {line}");
        assert_eq!(output.len(), 6, "bad output record: {line}");
        let word = |s: &str| u32::from_str_radix(s, 16).unwrap();
        let mut manager_mem = vec![0u64; 1024];
        let mut owner_mem = vec![0u64; 1024];
        let manager = manager_mem.as_mut_ptr().cast::<AInterpolationManager>();
        let owner = owner_mem.as_mut_ptr().cast::<AActor>();
        unsafe {
            core::ptr::addr_of_mut!((*manager).base.base.owner).write_unaligned(owner.cast());
            core::ptr::addr_of_mut!((*manager).phys_rate).write_unaligned(0.0);
            core::ptr::addr_of_mut!((*owner).location).write_unaligned(FVector {
                x: f(word(input[3])),
                y: f(word(input[4])),
                z: f(word(input[5])),
            });
            core::ptr::addr_of_mut!((*owner).velocity).write_unaligned(FVector {
                x: 2.0,
                y: -4.0,
                z: 8.0,
            });
            hp2_engine::physic::interp::perform_physics(manager, &SERVICES, f(word(input[2])));
            let velocity = core::ptr::addr_of!((*owner).velocity).read_unaligned();
            let acceleration = core::ptr::addr_of!((*owner).acceleration).read_unaligned();
            let actual = [
                velocity.x,
                velocity.y,
                velocity.z,
                acceleration.x,
                acceleration.y,
                acceleration.z,
            ];
            for (index, value) in actual.into_iter().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    word(output[index]),
                    "output {index}: {line}"
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 1);
}

unsafe extern "C" fn interp_sin_zero(_index: i32) -> f32 {
    0.0
}

unsafe extern "C" fn interp_cos_zero(_index: i32) -> f32 {
    1.0
}

#[test]
fn interpolated_position_instant_path_matches_original() {
    let text = include_str!("fixtures/physic_interp.txt");
    let line = text
        .lines()
        .find(|line| line.starts_with("interp_position_instant "))
        .unwrap();
    let parts: Vec<&str> = line.split('|').collect();
    assert_eq!(parts.len(), 2, "bad record: {line}");
    let input: Vec<&str> = parts[0].split_whitespace().collect();
    let output: Vec<&str> = parts[1].split_whitespace().collect();
    assert_eq!(input.len(), 6, "bad input record: {line}");
    assert_eq!(output.len(), 12, "bad output record: {line}");
    let word = |s: &str| u32::from_str_radix(s, 16).unwrap();
    let mut memory = vec![0u64; 1024];
    let point = memory.as_mut_ptr().cast::<AInterpolationPoint>();
    let mut svc = unsafe { core::ptr::read(core::ptr::addr_of!(SERVICES)) };
    svc.sin_tab = interp_sin_zero;
    svc.cos_tab = interp_cos_zero;
    unsafe {
        (*point).set_b_instant_next_path(true);
        core::ptr::addr_of_mut!((*point).base.base.location).write_unaligned(FVector {
            x: f(word(input[3])),
            y: f(word(input[4])),
            z: f(word(input[5])),
        });
        let coords = FCoords {
            origin: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            x_axis: FVector {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            y_axis: FVector {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            z_axis: FVector {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        };
        let got = hp2_engine::physic::interp::get_interpolated_position(
            point,
            &svc,
            coords,
            FVector {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            f(word(input[2])),
            0,
        );
        let actual = [
            got.origin.x,
            got.origin.y,
            got.origin.z,
            got.x_axis.x,
            got.x_axis.y,
            got.x_axis.z,
            got.y_axis.x,
            got.y_axis.y,
            got.y_axis.z,
            got.z_axis.x,
            got.z_axis.y,
            got.z_axis.z,
        ];
        for (index, value) in actual.into_iter().enumerate() {
            assert_eq!(
                value.to_bits(),
                word(output[index]),
                "output {index}: {line}"
            );
        }
    }
}

#[test]
fn interpolation_manager_active_step_matches_original() {
    let text = include_str!("fixtures/physic_interp.txt");
    let line = text
        .lines()
        .find(|line| line.starts_with("interp_manager_active "))
        .unwrap();
    let parts: Vec<&str> = line.split('|').collect();
    assert_eq!(parts.len(), 2, "bad record: {line}");
    let input: Vec<&str> = parts[0].split_whitespace().collect();
    let output: Vec<&str> = parts[1].split_whitespace().collect();
    assert_eq!(input.len(), 3, "bad input record: {line}");
    assert_eq!(output.len(), 14, "bad output record: {line}");
    assert_eq!(input[1], "000000000000000b", "unexpected seed: {line}");
    let word = |s: &str| u32::from_str_radix(s, 16).unwrap();
    let mut manager_mem = vec![0u64; 1024];
    let mut owner_mem = vec![0u64; 1024];
    let mut point_mem = vec![0u64; 1024];
    let manager = manager_mem.as_mut_ptr().cast::<AInterpolationManager>();
    let owner = owner_mem.as_mut_ptr().cast::<AActor>();
    let point = point_mem.as_mut_ptr().cast::<AInterpolationPoint>();
    with(|s| {
        s.svc = 11;
        s.log.clear();
        s.subj = manager as usize;
        s.subj_class = 0;
        s.pool = [manager as usize, owner as usize, point as usize];
        s.pool_class = [0; 3];
    });
    let mut svc = unsafe { core::ptr::read(core::ptr::addr_of!(SERVICES)) };
    svc.sin_tab = interp_sin_zero;
    svc.cos_tab = interp_cos_zero;
    unsafe {
        core::ptr::addr_of_mut!((*manager).base.base.owner).write_unaligned(owner.cast());
        core::ptr::addr_of_mut!((*manager).dest).write_unaligned(point.cast());
        core::ptr::addr_of_mut!((*manager).phys_rate).write_unaligned(0.5);
        core::ptr::addr_of_mut!((*manager).phys_alpha).write_unaligned(0.0);
        core::ptr::addr_of_mut!((*manager).remaining_pause).write_unaligned(0.0);
        (*owner).set_b_interpolating(true);
        (*owner).set_b_interpolating_ignore_rot(true);
        core::ptr::addr_of_mut!((*owner).location).write_unaligned(FVector {
            x: 3.0,
            y: -2.0,
            z: 1.0,
        });
        core::ptr::addr_of_mut!((*owner).velocity).write_unaligned(FVector {
            x: 2.0,
            y: -4.0,
            z: 8.0,
        });
        core::ptr::addr_of_mut!((*point).base.base.location).write_unaligned(FVector {
            x: 5.0,
            y: 6.0,
            z: 7.0,
        });
        core::ptr::addr_of_mut!((*point).start_control_point).write_unaligned(FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        core::ptr::addr_of_mut!((*point).end_control_point).write_unaligned(FVector {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        });
        core::ptr::addr_of_mut!((*point).path_dist).write_unaligned(1.0);
        hp2_engine::physic::interp::perform_physics(manager, &svc, f(word(input[2])));
        let alpha = core::ptr::addr_of!((*manager).phys_alpha).read_unaligned();
        let remaining_pause = core::ptr::addr_of!((*manager).remaining_pause).read_unaligned();
        let location = core::ptr::addr_of!((*owner).location).read_unaligned();
        let velocity = core::ptr::addr_of!((*owner).velocity).read_unaligned();
        let acceleration = core::ptr::addr_of!((*owner).acceleration).read_unaligned();
        let actual = [
            alpha,
            remaining_pause,
            location.x,
            location.y,
            location.z,
            velocity.x,
            velocity.y,
            velocity.z,
            acceleration.x,
            acceleration.y,
            acceleration.z,
        ];
        for (index, value) in actual.into_iter().enumerate() {
            assert_eq!(
                value.to_bits(),
                word(output[index]),
                "output {index}: {line}"
            );
        }
    }
    let (log, digest, len) = with(|s| (s.log.clone(), fnv1a(s.log.as_bytes()), s.log.len()));
    assert!(
        log.contains("event_update_camera "),
        "missing camera callback: {line}"
    );
    assert!(
        log.contains("event_interpolate_end 2 0 1\n"),
        "missing completion callback: {line}"
    );
    assert_eq!((digest >> 32) as u32, word(output[11]), "log high: {line}");
    assert_eq!(digest as u32, word(output[12]), "log low: {line}");
    assert_eq!(len as u32, word(output[13]), "log length: {line}");
}

#[test]
fn interpolation_manager_finished_event_matches_original() {
    let text = include_str!("fixtures/physic_interp.txt");
    let line = text
        .lines()
        .find(|line| line.starts_with("interp_manager_finished "))
        .unwrap();
    let parts: Vec<&str> = line.split('|').collect();
    assert_eq!(parts.len(), 2, "bad record: {line}");
    let input: Vec<&str> = parts[0].split_whitespace().collect();
    let output: Vec<&str> = parts[1].split_whitespace().collect();
    assert_eq!(input.len(), 3, "bad input record: {line}");
    assert_eq!(output.len(), 14, "bad output record: {line}");
    assert_eq!(input[1], "000000000000000c", "unexpected seed: {line}");
    let word = |s: &str| u32::from_str_radix(s, 16).unwrap();
    let mut manager_mem = vec![0u64; 1024];
    let mut owner_mem = vec![0u64; 1024];
    let manager = manager_mem.as_mut_ptr().cast::<AInterpolationManager>();
    let owner = owner_mem.as_mut_ptr().cast::<AActor>();
    with(|s| {
        s.svc = 12;
        s.log.clear();
        s.subj = manager as usize;
        s.subj_class = 0;
        s.pool = [manager as usize, owner as usize, 0];
        s.pool_class = [0; 3];
    });
    unsafe {
        core::ptr::addr_of_mut!((*manager).base.base.owner).write_unaligned(owner.cast());
        core::ptr::addr_of_mut!((*manager).phys_rate).write_unaligned(0.5);
        core::ptr::addr_of_mut!((*manager).phys_alpha).write_unaligned(0.0);
        (*owner).set_b_interpolating(true);
        core::ptr::addr_of_mut!((*owner).location).write_unaligned(FVector {
            x: 7.0,
            y: -3.0,
            z: 11.0,
        });
        core::ptr::addr_of_mut!((*owner).velocity).write_unaligned(FVector {
            x: 2.0,
            y: -4.0,
            z: 8.0,
        });
        hp2_engine::physic::interp::perform_physics(manager, &SERVICES, f(word(input[2])));
        let alpha = core::ptr::addr_of!((*manager).phys_alpha).read_unaligned();
        let remaining_pause = core::ptr::addr_of!((*manager).remaining_pause).read_unaligned();
        let location = core::ptr::addr_of!((*owner).location).read_unaligned();
        let velocity = core::ptr::addr_of!((*owner).velocity).read_unaligned();
        let acceleration = core::ptr::addr_of!((*owner).acceleration).read_unaligned();
        let actual = [
            alpha,
            remaining_pause,
            location.x,
            location.y,
            location.z,
            velocity.x,
            velocity.y,
            velocity.z,
            acceleration.x,
            acceleration.y,
            acceleration.z,
        ];
        for (index, value) in actual.into_iter().enumerate() {
            assert_eq!(
                value.to_bits(),
                word(output[index]),
                "output {index}: {line}"
            );
        }
    }
    let (log, digest, len) = with(|s| (s.log.clone(), fnv1a(s.log.as_bytes()), s.log.len()));
    assert!(
        log.contains("event_finished_interpolation 0 2\n"),
        "missing finished callback: {line}"
    );
    assert_eq!((digest >> 32) as u32, word(output[11]), "log high: {line}");
    assert_eq!(digest as u32, word(output[12]), "log low: {line}");
    assert_eq!(len as u32, word(output[13]), "log length: {line}");
}

#[test]
fn interpolation_fixture_has_all_branch_classes() {
    let text = include_str!("fixtures/physic_interp.txt");
    for (name, expected) in [
        ("interp_pause ", 4),
        ("interp_position ", 4),
        ("interp_manager_idle ", 1),
        ("interp_position_instant ", 1),
        ("interp_manager_active ", 1),
        ("interp_manager_finished ", 1),
    ] {
        assert_eq!(
            text.lines().filter(|line| line.starts_with(name)).count(),
            expected,
            "wrong case count for {name}"
        );
    }
    assert_eq!(text.lines().count(), 12);
}
