//! Replays original-product `AActor::physTrailer` cases.
include!("physic_scripted.rs");

const INPUT_WORDS: usize = 24;
const OUTPUT_WORDS: usize = 6;

fn run_trailer(seed: u64, w: &[u32]) -> ([u32; OUTPUT_WORDS], String) {
    assert_eq!(w.len(), INPUT_WORDS);
    let mut subj = vec![0u64; 1024];
    let mut owner = vec![0u64; 1024];
    let mut middle = vec![0u64; 1024];
    let mut other = vec![0u64; 1024];
    let subject_ptr = subj.as_mut_ptr() as usize;
    let owner_ptr = owner.as_mut_ptr() as usize;
    let middle_ptr = middle.as_mut_ptr() as usize;
    let other_ptr = other.as_mut_ptr() as usize;
    let action = w[19];
    let bone_coords = [
        f(w[20]),
        f(w[21]),
        f(w[22]),
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ];
    with(|s| {
        s.svc = seed;
        s.log.clear();
        s.subj = subject_ptr;
        s.subj_class = 0;
        s.pool = [owner_ptr, middle_ptr, other_ptr];
        s.trailer_prepivot = [f(w[15]), f(w[16]), f(w[17])];
        s.trailer_mass = f(w[18]);
        s.trailer_draw_type = if action == 1 { 1 } else { 2 };
        s.trailer_anim_bone = i32::from(action == 2);
        s.trailer_flags = match action {
            1 => 1,
            4 => 4,
            _ => 0,
        };
        s.trailer_bone_coords = bone_coords;
    });
    unsafe {
        let actor = subject_ptr as *mut AActor;
        let owner = owner_ptr as *mut AActor;
        (*actor).location = v3(&w[0..3]);
        (*actor).rotation = FRotator {
            pitch: w[3] as i32,
            yaw: w[4] as i32,
            roll: w[5] as i32,
        };
        (*actor).mass = f(w[18]);
        (*actor).owner = if action == 0 {
            null_mut()
        } else {
            owner.cast()
        };
        (*owner).location = v3(&w[6..9]);
        (*owner).velocity = zero();
        (*owner).rotation = FRotator {
            pitch: w[12] as i32,
            yaw: w[13] as i32,
            roll: w[14] as i32,
        };
        hp2_engine::physic::dispatch::phys_trailer(actor, &SERVICES, 0.0);
    }
    let mut out = [0u32; OUTPUT_WORDS];
    unsafe {
        let actor = &*(subject_ptr as *const AActor);
        put3(&mut out[0..3], actor.location);
        out[3] = actor.rotation.pitch as u32;
        out[4] = actor.rotation.yaw as u32;
        out[5] = actor.rotation.roll as u32;
    }
    let log = with(|s| s.log.clone());
    (out, log)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[test]
fn original_trailer_cases_replay_state_and_ordered_move_trace() {
    let mut cases = 0usize;
    let mut branches = 0u32;
    for line in include_str!("fixtures/physic_move.txt").lines() {
        let (record, rest) = line.split_once('|').expect("missing output delimiter");
        let (expected_words, expected_hex) = rest.split_once('|').expect("missing trace delimiter");
        let mut fields = record.split_whitespace();
        assert_eq!(fields.next(), Some("phys_move"));
        let seed = u64::from_str_radix(fields.next().unwrap(), 16).unwrap();
        let w: Vec<u32> = fields
            .map(|word| u32::from_str_radix(word, 16).unwrap())
            .collect();
        assert_eq!(w.len(), INPUT_WORDS);
        let expected: Vec<u32> = expected_words
            .split_whitespace()
            .map(|word| u32::from_str_radix(word, 16).unwrap())
            .collect();
        assert_eq!(expected.len(), OUTPUT_WORDS);
        let (actual, log) = run_trailer(seed, &w);
        assert_eq!(actual.as_slice(), expected.as_slice(), "seed {seed:016x}");
        assert_eq!(
            hex(log.as_bytes()),
            expected_hex.trim(),
            "ordered move trace for seed {seed:016x}"
        );
        let action = w[19];
        if action == 0 {
            branches |= 1;
        } else {
            branches |= 1 << action;
        }
        cases += 1;
    }
    assert_eq!(cases, 1200);
    assert_eq!(branches, 0x1f);
}
