//! Bit-exact check of `hp2_render::sprite_filter` against the original
//! `FDynamicChunk::Filter` split/filter-down and `FDynamicLight::Filter`
//! decision arithmetic (`tests/fixtures/sprite_filter_reference.txt`: verbatim
//! copies of the original functions compiled with clang++ -ffp-contract=on on
//! arm64, deterministic inputs, raster buffers pre-filled with 0xEE).

use hp2_render::sprite_filter::{
    Action, Buf, FPlane, FRasterPolyHeader, FRasterSpan, FTransform, FVector, INDEX_NONE,
    LIGHT_BACK_ENTER, LIGHT_BACK_VOLUMETRIC, LIGHT_FRONT_ENTER, LIGHT_FRONT_VOLUMETRIC, Raster,
    Source, app_floor, app_round, crossings, filter_chunk, light_flags, plane_dot,
    raster_alloc_bytes, split_allocates,
};

const FIXTURE: &str = include_str!("fixtures/sprite_filter_reference.txt");

/// Fill word the C harness writes into every raster buffer before the call.
const FILL: u32 = 0xEEEE_EEEE;
const FILL_SPAN: FRasterSpan = FRasterSpan {
    x: [FILL as i32; 2],
};

fn w(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}
fn f(s: &str) -> f32 {
    f32::from_bits(w(s))
}
fn i(s: &str) -> i32 {
    w(s) as i32
}

/// Records with the given tag as (input words, output words). Output is empty
/// for records without the `|` separator.
fn records(tag: &str) -> Vec<(Vec<&'static str>, Vec<&'static str>)> {
    FIXTURE
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            if parts.next()? != tag {
                return None;
            }
            let rest: Vec<&'static str> = parts.collect();
            Some(match rest.iter().position(|t| *t == "|") {
                Some(bar) => (rest[..bar].to_vec(), rest[bar + 1..].to_vec()),
                None => (rest, Vec::new()),
            })
        })
        .collect()
}

fn plane_at(p: &[&str]) -> FPlane {
    FPlane {
        x: f(p[0]),
        y: f(p[1]),
        z: f(p[2]),
        w: f(p[3]),
    }
}

fn vec_at(p: &[&str]) -> FVector {
    FVector {
        x: f(p[0]),
        y: f(p[1]),
        z: f(p[2]),
    }
}

fn vert(x: f32, y: f32, z: f32) -> FTransform {
    FTransform {
        point: FVector { x, y, z },
        flags: 0,
        screen_x: 0.0,
        screen_y: 0.0,
        int_y: 0,
        rz: 0.0,
    }
}

fn pick(r: Raster) -> u32 {
    match r {
        Raster::Null => 0,
        Raster::Front => 1,
        Raster::Back => 2,
        Raster::Source => 3,
    }
}

fn action(a: Action) -> u32 {
    match a {
        Action::None => 0,
        Action::Chunk => 1,
        Action::Final => 2,
    }
}

/// Header then spans, as the original `FRasterPoly` lays them out in memory.
fn buf_words(h: &FRasterPolyHeader, lines: &[FRasterSpan]) -> Vec<u32> {
    let mut v = vec![h.start_y as u32, h.end_y as u32];
    for s in lines {
        v.push(s.x[0] as u32);
        v.push(s.x[1] as u32);
    }
    v
}

struct ChunkCase {
    verts: [FTransform; 4],
    plane: FPlane,
    start: i32,
    end: i32,
    spans: Vec<FRasterSpan>,
    i_front: i32,
    i_back: i32,
    outside: bool,
    csg: bool,
}

/// Input layout: plane XYZW, then 4 × (point XYZ, screen X, screen Y), then
/// StartY, EndY, `EndY - StartY` spans of (X0, X1), iFront, iBack, Outside, CSG.
fn chunk_case(inp: &[&str]) -> ChunkCase {
    let start = i(inp[24]);
    let end = i(inp[25]);
    let n = (end - start) as usize;
    assert_eq!(inp.len(), 30 + 2 * n, "input layout {inp:?}");

    let plane = plane_at(&inp[0..4]);
    let mut verts = [vert(0.0, 0.0, 0.0); 4];
    for (v, slot) in verts.iter_mut().enumerate() {
        let b = 4 + 5 * v;
        *slot = FTransform {
            point: vec_at(&inp[b..b + 3]),
            flags: 0,
            screen_x: f(inp[b + 3]),
            screen_y: f(inp[b + 4]),
            int_y: 0,
            rz: 0.0,
        };
    }
    let spans = inp[26..26 + 2 * n]
        .chunks(2)
        .map(|p| FRasterSpan {
            x: [i(p[0]), i(p[1])],
        })
        .collect();
    let t = 26 + 2 * n;
    ChunkCase {
        verts,
        plane,
        start,
        end,
        spans,
        i_front: i(inp[t]),
        i_back: i(inp[t + 1]),
        outside: i(inp[t + 2]) != 0,
        csg: i(inp[t + 3]) != 0,
    }
}

#[test]
fn app_floor_and_round_truncate_toward_zero() {
    let recs = records("A");
    assert_eq!(recs.len(), 208);
    for (inp, _) in &recs {
        // Record: A <float bits> <appFloor> <appRound>
        let v = f(inp[0]);
        assert_eq!(app_floor(v), i(inp[1]), "appFloor({v:?})");
        assert_eq!(app_round(v), i(inp[2]), "appRound({v:?})");
    }
}

#[test]
fn app_floor_and_round_do_not_floor_negative_fractions() {
    // appFloor/appRound are static_cast<INT> (truncation toward zero) on this build.
    assert_eq!(app_floor(-0.5), 0);
    assert_eq!(app_round(-0.5), 0);
    assert_eq!(app_floor(-1.5), -1);
    assert_eq!(app_round(-1.5), -1);
    assert_eq!(app_floor(2.9), 2);
}

#[test]
fn plane_dot_and_light_flags_match_original() {
    let recs = records("L");
    assert_eq!(recs.len(), 1000);
    for (inp, out) in &recs {
        let plane = plane_at(&inp[0..4]);
        let loc = vec_at(&inp[4..7]);
        let (radius, vol_radius, is_vol) = (f(inp[7]), f(inp[8]), w(inp[9]) != 0);
        assert_eq!(plane_dot(&plane, &loc).to_bits(), w(out[0]), "dist {inp:?}");
        assert_eq!(
            light_flags(&plane, &loc, radius, vol_radius, is_vol) as u32,
            w(out[1]),
            "flags {inp:?}"
        );
    }
}

#[test]
fn light_flags_thresholds_are_strict() {
    let plane = FPlane {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 0.0,
    };
    // dist == 0 sits exactly on both -radius and +radius: neither side enters.
    let origin = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    assert_eq!(light_flags(&plane, &origin, 0.0, 0.0, true), 0);

    let above = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.5,
    };
    assert_eq!(
        light_flags(&plane, &above, 1.0, 2.0, true),
        LIGHT_FRONT_ENTER | LIGHT_FRONT_VOLUMETRIC | LIGHT_BACK_ENTER | LIGHT_BACK_VOLUMETRIC
    );
    // Non-volumetric lights never set the volumetric bits.
    assert_eq!(
        light_flags(&plane, &above, 1.0, 2.0, false),
        LIGHT_FRONT_ENTER | LIGHT_BACK_ENTER
    );
}

#[test]
fn crossings_classify_and_split_criterion() {
    let plane = FPlane {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 0.0,
    };
    // Two sign changes (edges 0→1 and 1→2): front 3, back 1, split allowed.
    let verts = [
        vert(0.0, 0.0, 1.0),
        vert(0.0, 0.0, -1.0),
        vert(0.0, 0.0, 1.0),
        vert(0.0, 0.0, 1.0),
    ];
    let c = crossings(&verts, &plane);
    assert_eq!(c.dist, [1.0, -1.0, 1.0, 1.0]);
    assert_eq!((c.front, c.back, c.num), (3, 1, 2));
    assert!(split_allocates(&c));

    // All verts in front: no back side, so no split buffers are allocated.
    let all_front = [vert(0.0, 0.0, 1.0); 4];
    assert!(!split_allocates(&crossings(&all_front, &plane)));
}

#[test]
fn raster_alloc_bytes_is_header_plus_spans() {
    assert_eq!(raster_alloc_bytes(0, 0), 8);
    assert_eq!(raster_alloc_bytes(3, 5), 8 + 2 * 8);
    assert_eq!(raster_alloc_bytes(-2, 1), 8 + 3 * 8);
}

#[test]
fn filter_chunk_matches_original_for_all_fixture_cases() {
    let recs = records("C");
    assert_eq!(recs.len(), 1200);

    let (mut front_bufs, mut back_bufs, mut finals, mut chunks) = (0, 0, 0, 0);
    for (inp, out) in &recs {
        let case = chunk_case(inp);
        let n = case.spans.len();
        let alloc = w(out[0]) != 0;

        let split = split_allocates(&crossings(&case.verts, &case.plane));
        assert_eq!(split, alloc, "split_allocates {inp:?}");

        let src = Source {
            start_y: case.start,
            end_y: case.end,
            lines: &case.spans,
        };
        let mut h0 = FRasterPolyHeader {
            start_y: FILL as i32,
            end_y: FILL as i32,
        };
        let mut h1 = FRasterPolyHeader {
            start_y: FILL as i32,
            end_y: FILL as i32,
        };
        let mut l0 = vec![FILL_SPAN; n];
        let mut l1 = vec![FILL_SPAN; n];
        let bufs = if alloc {
            Some([
                Buf {
                    header: &mut h0,
                    lines: &mut l0[..],
                },
                Buf {
                    header: &mut h1,
                    lines: &mut l1[..],
                },
            ])
        } else {
            None
        };

        let r = filter_chunk(
            &case.verts,
            &case.plane,
            &src,
            bufs,
            case.i_front,
            case.i_back,
            case.outside,
            case.csg,
        );

        assert_eq!(pick(r.front), w(out[1]), "front raster {inp:?}");
        assert_eq!(pick(r.back), w(out[2]), "back raster {inp:?}");
        assert_eq!(action(r.front_action), w(out[3]), "front action {inp:?}");
        assert_eq!(action(r.back_action), w(out[4]), "back action {inp:?}");

        if w(out[1]) == 1 {
            front_bufs += 1;
        }
        if w(out[2]) == 2 {
            back_bufs += 1;
        }
        match w(out[3]) {
            2 => finals += 1,
            1 => chunks += 1,
            _ => {}
        }

        if alloc {
            let words = 2 + 2 * n;
            let want: Vec<u32> = out[5..].iter().map(|s| w(s)).collect();
            assert_eq!(want.len(), 2 * words, "output layout {out:?}");
            assert_eq!(buf_words(&h0, &l0), want[..words], "front buffer {inp:?}");
            assert_eq!(buf_words(&h1, &l1), want[words..], "back buffer {inp:?}");
        } else {
            assert_eq!(out.len(), 5, "output layout {out:?}");
        }
    }

    // The fixture must exercise both split buffers and both filter-down actions.
    assert!(front_bufs > 0 && back_bufs > 0, "buffer coverage");
    assert!(finals > 0 && chunks > 0, "action coverage");
}

#[test]
fn filter_chunk_without_split_keeps_source_and_picks_actions() {
    let plane = FPlane {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 0.0,
    };
    // All verts in front, so the back side is empty and no buffers are used.
    let verts = [vert(0.0, 0.0, 1.0); 4];
    let src = Source {
        start_y: 0,
        end_y: 0,
        lines: &[],
    };

    let r = filter_chunk(&verts, &plane, &src, None, 0, INDEX_NONE, false, false);
    assert_eq!(r.front, Raster::Source);
    assert_eq!(r.back, Raster::Source);
    assert_eq!(r.front_action, Action::Chunk);
    assert_eq!(r.back_action, Action::None);

    let r = filter_chunk(
        &verts, &plane, &src, None, INDEX_NONE, INDEX_NONE, true, false,
    );
    assert_eq!(r.front_action, Action::Final);
    assert_eq!(r.back_action, Action::None);

    let r = filter_chunk(
        &verts, &plane, &src, None, INDEX_NONE, INDEX_NONE, false, false,
    );
    assert_eq!(r.front_action, Action::None);
}
