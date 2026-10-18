//! Replays the original-C++ oracle fixtures (`fixtures/primtrace_model.txt`,
//! `fixtures/primtrace_box.txt`) against `hp2_engine::trace` and
//! `hp2_engine::prim`.
//!
//! The fixtures were produced by linking the pristine 40bde73 `UnMath.cpp`,
//! `UnPrim.cpp` and `UnTrace.cpp` with a harness that builds the same convex
//! cube BSP (and the same box primitive) rebuilt below, then drives the
//! original `UModel::PointCheck/LineCheck/FastLineCheck` and the file-scope
//! `UBox` helpers with NULL owners. Floats are IEEE-754 bit patterns in hex,
//! so every comparison is bit-exact.
//!
//! Model line: `<variant> <P|L|F> <flags> | <params hex> => <ret> | <loc> <normal> <time> <item>`
//! Box lines:  `B <min> <max> <IsValid> <extent>` followed by `P`/`L` lines in the same shape.

use hp2_abi::engine::generated::FCheckResult;
use hp2_abi::render_scene::FBspNode;
use hp2_abi::{FPlane, FVector};
use hp2_engine::prim::{self, Aabb, BspArrays, FCheck};
use hp2_engine::trace::{self, ModelView};

const INDEX_NONE: i32 = -1;
const HULL_FLIP: i32 = 0x4000_0000;

fn hexf(tok: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(tok, 16).expect("hex float"))
}

fn vec_at(toks: &[&str], i: usize) -> FVector {
    FVector {
        x: hexf(toks[i]),
        y: hexf(toks[i + 1]),
        z: hexf(toks[i + 2]),
    }
}

fn assert_vec(got: FVector, want: FVector, line: &str) {
    assert_eq!(got.x.to_bits(), want.x.to_bits(), "x mismatch: {line}");
    assert_eq!(got.y.to_bits(), want.y.to_bits(), "y mismatch: {line}");
    assert_eq!(got.z.to_bits(), want.z.to_bits(), "z mismatch: {line}");
}

/// Mirrors `setupNodes(variant)` in the C++ harness.
fn cube_nodes(variant: u32) -> Vec<FBspNode> {
    let planes = [
        (1.0, 0.0, 0.0),
        (-1.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
        (0.0, -1.0, 0.0),
        (0.0, 0.0, 1.0),
        (0.0, 0.0, -1.0),
    ];
    (0..6usize)
        .map(|k| {
            let (x, y, z) = planes[k];
            // SAFETY: FBspNode is plain old data (floats, ints, byte arrays).
            let mut n: FBspNode = unsafe { std::mem::zeroed() };
            n.plane = FPlane { x, y, z, w: 1.0 };
            n.num_vertices = 4;
            n.node_flags = 0;
            n.i_front = INDEX_NONE;
            n.i_back = if k < 5 { k as i32 + 1 } else { INDEX_NONE };
            n.i_collision_bound = INDEX_NONE;
            n.i_leaf = [INDEX_NONE, INDEX_NONE];
            n
        })
        .enumerate()
        .map(|(k, mut n)| {
            if variant == 1 && k == 2 {
                n.node_flags = 0x01; // NF_NotCsg
            }
            if variant == 1 && k == 4 {
                n.num_vertices = 0;
            }
            if k == 5 {
                n.i_collision_bound = 0;
            }
            n
        })
        .collect()
}

/// Mirrors the `gHulls` layout of the C++ harness for one variant.
fn cube_hulls(variant: u32) -> Vec<i32> {
    let mut h = vec![0, 1, 2, 3, 4, 5, INDEX_NONE];
    if variant == 1 {
        h[3] = 3 | HULL_FLIP;
    }
    let mut b = [-1.0f32, -1.0, -1.0, 1.0, 1.0, 1.0];
    if variant == 1 {
        b[0] = -1.5;
        b[4] = 1.25;
    }
    h.extend(b.iter().map(|f| f.to_bits() as i32));
    h
}

#[test]
fn model_fixtures_bit_exact() {
    let text = include_str!("fixtures/primtrace_model.txt");
    let mut checked = 0usize;
    // Per-variant BSP arrays, built once and reused by every line of that variant.
    let mut owned: Vec<(Vec<FBspNode>, Vec<i32>, bool)> = Vec::new();
    for variant in 0..4u32 {
        let (nodes, hulls, root_outside) = match variant {
            0 => (cube_nodes(0), cube_hulls(0), true),
            1 => (cube_nodes(1), cube_hulls(1), false),
            2 => (Vec::new(), Vec::new(), true),
            _ => (Vec::new(), Vec::new(), false),
        };
        owned.push((nodes, hulls, root_outside));
    }
    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let variant: usize = toks[0].parse().unwrap();
        let op = toks[1];
        let flags: u32 = toks[2].parse().unwrap();
        // toks[3] == "|"
        let (nodes, hulls, root_outside) = &owned[variant];
        let mv = ModelView {
            bsp: BspArrays {
                nodes,
                points: &[],
                verts: &[],
                surfs: &[],
            },
            leaf_hulls: hulls,
            bounding_box: Aabb::empty(),
            root_outside: *root_outside,
            num_zones: 0,
            primitive: std::ptr::null_mut(),
        };
        let arrow = toks.iter().position(|t| *t == "=>").unwrap();
        let params = &toks[4..arrow];
        let ret: i32 = toks[arrow + 1].parse().unwrap();
        // Result block after the second "|".
        let rest = &toks[arrow + 3..];
        let mut hit: FCheckResult = prim::empty_result();
        match op {
            "P" => {
                let loc = vec_at(params, 0);
                let ext = vec_at(params, 3);
                let got = trace::model_point_check(&mut hit, &mv, None, loc, ext, flags);
                assert_eq!(got as i32, ret, "P ret: {line}");
            }
            "L" => {
                let end = vec_at(params, 0);
                let start = vec_at(params, 3);
                let ext = vec_at(params, 6);
                let got = trace::model_line_check(&mut hit, &mv, None, end, start, ext, flags);
                assert_eq!(got as i32, ret, "L ret: {line}");
            }
            "F" => {
                let end = vec_at(params, 0);
                let start = vec_at(params, 3);
                let got = trace::fast_line_check(&mv, end, start);
                assert_eq!(got as i32, ret, "F ret: {line}");
                checked += 1;
                continue;
            }
            other => panic!("unknown op {other}: {line}"),
        }
        assert_vec(hit.location, vec_at(rest, 0), line);
        assert_vec(hit.normal, vec_at(rest, 3), line);
        assert_eq!(hit.time.to_bits(), hexf(rest[6]).to_bits(), "time: {line}");
        assert_eq!(hit.item, rest[7].parse::<i32>().unwrap(), "item: {line}");
        checked += 1;
    }
    assert_eq!(checked, 16000);
}

#[test]
fn box_fixtures_bit_exact() {
    let text = include_str!("fixtures/primtrace_box.txt");
    let mut checked = 0usize;
    let mut bounds: Option<Aabb> = None;
    // NULL owner, as in the C++ oracle (`FCheck(..., NULL)`).
    let owner_none: Option<prim::Owner> = None;
    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        match toks[0] {
            "B" => {
                let min = vec_at(&toks, 1);
                let max = vec_at(&toks, 4);
                let valid: u32 = toks[7].parse().unwrap();
                let ext = vec_at(&toks, 8);
                let bb = Aabb::from_min_max(min, max);
                assert_eq!(valid, 1, "IsValid: {line}");
                assert_vec(prim::boxprim_extent(&bb), ext, line);
                // World-less bounds never dereference the owner.
                let zeroed_owner: prim::Owner = unsafe { std::mem::zeroed() };
                let got = prim::boxprim_bounds(&bb, &zeroed_owner, false);
                assert_vec(got.min, min, line);
                assert_vec(got.max, max, line);
                bounds = Some(bb);
            }
            "P" | "L" => {
                let bb = bounds.as_ref().expect("B line before P/L");
                let arrow = toks.iter().position(|t| *t == "=>").unwrap();
                let params = &toks[1..arrow];
                let ret: i32 = toks[arrow + 1].parse().unwrap();
                let rest = &toks[arrow + 3..];
                let mut hit: FCheckResult = prim::empty_result();
                let got = if toks[0] == "P" {
                    let start = vec_at(params, 0);
                    let ext = vec_at(params, 3);
                    let check = FCheck::point(start, ext, owner_none);
                    prim::box_point_check(&mut hit, &check, bb)
                } else {
                    let end = vec_at(params, 0);
                    let start = vec_at(params, 3);
                    let ext = vec_at(params, 6);
                    let check = FCheck {
                        end,
                        start,
                        extent: ext,
                        owner: owner_none,
                    };
                    prim::box_line_check(&mut hit, &check, bb)
                };
                assert_eq!(got as i32, ret, "ret: {line}");
                assert_vec(hit.location, vec_at(rest, 0), line);
                assert_vec(hit.normal, vec_at(rest, 3), line);
                assert_eq!(hit.time.to_bits(), hexf(rest[6]).to_bits(), "time: {line}");
                assert_eq!(hit.item, rest[7].parse::<i32>().unwrap(), "item: {line}");
                checked += 1;
            }
            other => panic!("unknown record {other}: {line}"),
        }
    }
    assert_eq!(checked, 1500);
}

// ---- Full oracle replay: fixtures/primtrace_model2.txt ----------------------

use hp2_abi::FCoords;
use hp2_abi::render_scene::{FBspSurf, FVert};
use hp2_engine::prim::Owner;
use std::ptr;

struct Model2 {
    nodes: Vec<FBspNode>,
    points: Vec<FVector>,
    verts: Vec<FVert>,
    surfs: Vec<FBspSurf>,
    hulls: Vec<i32>,
    bbox: Aabb,
    root_outside: bool,
}

fn tok_i32(t: &str) -> i32 {
    t.parse().unwrap()
}

fn coords_at(toks: &[&str], i: usize) -> FCoords {
    FCoords {
        origin: vec_at(toks, i),
        x_axis: vec_at(toks, i + 3),
        y_axis: vec_at(toks, i + 6),
        z_axis: vec_at(toks, i + 9),
    }
}

fn assert_hit(hit: &FCheckResult, rest: &[&str], line: &str) {
    assert_vec(hit.location, vec_at(rest, 0), line);
    assert_vec(hit.normal, vec_at(rest, 3), line);
    assert_eq!(hit.time.to_bits(), hexf(rest[6]).to_bits(), "time: {line}");
    assert_eq!(hit.item, tok_i32(rest[7]), "item: {line}");
}

/// Mirrors `FCheckResult(1.0f)` plus `Item = NONE` as the C++ harness sets them.
fn fresh_hit() -> FCheckResult {
    let mut hit = prim::empty_result();
    hit.time = 1.0;
    hit.item = INDEX_NONE;
    hit
}

#[test]
fn full_oracle_replay_bit_exact() {
    let text = include_str!("fixtures/primtrace_model2.txt");
    let mut models: Vec<Model2> = Vec::new();
    let mut owners: Vec<Owner> = Vec::new();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();

    for line in text.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let kind = toks[0];
        match kind {
            "M" => {
                assert_eq!(tok_i32(toks[1]) as usize, models.len());
                assert_eq!(tok_i32(toks[3]), 0, "NumZones: {line}");
                models.push(Model2 {
                    nodes: Vec::new(),
                    points: Vec::new(),
                    verts: Vec::new(),
                    surfs: Vec::new(),
                    hulls: Vec::new(),
                    bbox: Aabb {
                        min: vec_at(&toks, 4),
                        max: vec_at(&toks, 7),
                        valid: true,
                    },
                    root_outside: tok_i32(toks[2]) != 0,
                });
            }
            "N" => {
                let m = &mut models[tok_i32(toks[1]) as usize];
                let mut n: FBspNode = unsafe { std::mem::zeroed() };
                n.plane = FPlane {
                    x: hexf(toks[3]),
                    y: hexf(toks[4]),
                    z: hexf(toks[5]),
                    w: hexf(toks[6]),
                };
                n.i_vert_pool = tok_i32(toks[7]);
                n.i_surf = tok_i32(toks[8]);
                n.i_back = tok_i32(toks[9]);
                n.i_front = tok_i32(toks[10]);
                n.i_plane = tok_i32(toks[11]);
                n.i_collision_bound = tok_i32(toks[12]);
                n.i_render_bound = tok_i32(toks[13]);
                n.i_zone = [tok_i32(toks[14]) as u8, tok_i32(toks[15]) as u8];
                n.num_vertices = tok_i32(toks[16]) as u8;
                n.node_flags = tok_i32(toks[17]) as u8;
                n.i_leaf = [tok_i32(toks[18]), tok_i32(toks[19])];
                n.zone_mask = tok_i32(toks[20]) as u64;
                m.nodes.push(n);
            }
            "PT" => {
                let m = &mut models[tok_i32(toks[1]) as usize];
                m.points.push(vec_at(&toks, 3));
            }
            "V" => {
                let m = &mut models[tok_i32(toks[1]) as usize];
                m.verts.push(FVert {
                    p_vertex: tok_i32(toks[3]),
                    i_side: tok_i32(toks[4]),
                });
            }
            "S" => {
                let m = &mut models[tok_i32(toks[1]) as usize];
                // SAFETY: FBspSurf is plain old data; only the two indices are read here.
                let mut s: FBspSurf = unsafe { std::mem::zeroed() };
                s.p_base = tok_i32(toks[3]);
                s.v_normal = tok_i32(toks[4]);
                s.v_texture_u = tok_i32(toks[5]);
                s.v_texture_v = tok_i32(toks[6]);
                s.i_light_map = tok_i32(toks[7]);
                s.i_brush_poly = tok_i32(toks[8]);
                m.surfs.push(s);
            }
            "H" => {
                let m = &mut models[tok_i32(toks[1]) as usize];
                m.hulls = toks[2..].iter().map(|t| tok_i32(t)).collect();
            }
            "O" => {
                // O id loc(3) rad wid hei | tl(12) | tw(12) | ext(3) ext0(3)
                let bars: Vec<usize> = toks
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| **t == "|")
                    .map(|(i, _)| i)
                    .collect();
                owners.push(Owner {
                    actor: ptr::null_mut(),
                    location: vec_at(&toks, 2),
                    collision_radius: hexf(toks[5]),
                    collision_width: hexf(toks[6]),
                    collision_height: hexf(toks[7]),
                    to_local: coords_at(&toks, bars[0] + 1),
                    to_world: coords_at(&toks, bars[1] + 1),
                    cyl_extent: vec_at(&toks, bars[2] + 1),
                    cyl_extent_zero_width: vec_at(&toks, bars[2] + 4),
                });
            }
            "CP" | "CL" | "CF" | "CN" | "CR" | "CS" => {
                let mi = tok_i32(toks[1]) as usize;
                let m = &models[mi];
                let mv = ModelView {
                    bsp: BspArrays {
                        nodes: &m.nodes,
                        points: &m.points,
                        verts: &m.verts,
                        surfs: &m.surfs,
                    },
                    leaf_hulls: &m.hulls,
                    bounding_box: m.bbox,
                    root_outside: m.root_outside,
                    num_zones: 0,
                    primitive: ptr::null_mut(),
                };
                let arrow = toks.iter().position(|t| *t == "=>").unwrap();
                let params = &toks[2..arrow];
                let rest = toks.get(arrow + 3..).unwrap_or(&[]);
                *counts.entry(kind).or_default() += 1;
                match kind {
                    "CP" => {
                        let mut hit = fresh_hit();
                        let got = trace::model_point_check(
                            &mut hit,
                            &mv,
                            None,
                            vec_at(params, 0),
                            vec_at(params, 3),
                            0,
                        );
                        assert_eq!(got as i32, tok_i32(toks[arrow + 1]), "CP: {line}");
                        assert_hit(&hit, rest, line);
                    }
                    "CL" => {
                        let mut hit = fresh_hit();
                        let got = trace::model_line_check(
                            &mut hit,
                            &mv,
                            None,
                            vec_at(params, 0),
                            vec_at(params, 3),
                            vec_at(params, 6),
                            0,
                        );
                        assert_eq!(got as i32, tok_i32(toks[arrow + 1]), "CL: {line}");
                        assert_hit(&hit, rest, line);
                    }
                    "CF" => {
                        let got = trace::fast_line_check(&mv, vec_at(params, 0), vec_at(params, 3));
                        assert_eq!(got as i32, tok_i32(toks[arrow + 1]), "CF: {line}");
                    }
                    "CN" => {
                        let mut dest = FVector {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        };
                        let mut pv: i32 = INDEX_NONE;
                        let dist = trace::find_nearest_vertex(
                            &mv,
                            vec_at(params, 0),
                            &mut dest,
                            hexf(params[3]),
                            &mut pv,
                        );
                        assert_eq!(
                            dist.to_bits(),
                            hexf(toks[arrow + 1]).to_bits(),
                            "CN dist: {line}"
                        );
                        assert_vec(dest, vec_at(&toks, arrow + 2), line);
                        assert_eq!(pv, tok_i32(toks[arrow + 5]), "CN pv: {line}");
                    }
                    "CR" => {
                        let r = trace::point_region(&mv, vec_at(params, 0));
                        assert_eq!(r.i_leaf, tok_i32(toks[arrow + 1]), "CR leaf: {line}");
                        assert_eq!(
                            r.zone_number as i32,
                            tok_i32(toks[arrow + 2]),
                            "CR zone: {line}"
                        );
                    }
                    "CS" => {
                        let mut nodes = m.nodes.clone();
                        let sphere = FPlane {
                            x: hexf(params[0]),
                            y: hexf(params[1]),
                            z: hexf(params[2]),
                            w: hexf(params[3]),
                        };
                        trace::precompute_sphere_filter(&mut nodes, sphere);
                        for (i, want) in toks[arrow + 1..].iter().enumerate() {
                            assert_eq!(
                                nodes[i].node_flags as i32,
                                tok_i32(want),
                                "CS flags[{i}]: {line}"
                            );
                        }
                    }
                    _ => unreachable!(),
                }
            }
            "OP" | "OL" => {
                let oi = tok_i32(toks[1]) as usize;
                let kind_tag = toks[2];
                *counts.entry(kind).or_default() += 1;
                let o = owners[oi];
                let arrow = toks.iter().position(|t| *t == "=>").unwrap();
                let params = &toks[3..arrow];
                let rest = &toks[arrow + 3..];
                let ret = tok_i32(toks[arrow + 1]);
                let mut hit = fresh_hit();
                let got = if kind == "OP" {
                    let loc = vec_at(params, 0);
                    let ext = vec_at(params, 3);
                    match kind_tag {
                        "C" => prim::cyl_point_check(&mut hit, Some(&o), loc, ext),
                        "OC" => prim::oriented_cyl_point_check(&mut hit, &o, loc, ext),
                        "AO" => prim::aligned_oval_point_check(&mut hit, Some(&o), loc, ext),
                        "OO" => prim::oriented_oval_point_check(&mut hit, &o, loc, ext),
                        other => panic!("owner kind {other}: {line}"),
                    }
                } else {
                    let end = vec_at(params, 0);
                    let start = vec_at(params, 3);
                    let ext = vec_at(params, 6);
                    match kind_tag {
                        "C" => prim::cyl_line_check(&mut hit, Some(&o), end, start, ext),
                        "OC" => prim::oriented_cyl_line_check(&mut hit, &o, end, start, ext),
                        "AO" => prim::aligned_oval_line_check(&mut hit, Some(&o), end, start, ext),
                        "OO" => prim::oriented_oval_line_check(&mut hit, &o, end, start, ext),
                        other => panic!("owner kind {other}: {line}"),
                    }
                };
                assert_eq!(got as i32, ret, "{kind} ret: {line}");
                assert_hit(&hit, rest, line);
            }
            other => panic!("unknown record {other}: {line}"),
        }
    }

    assert_eq!(models.len(), 3);
    assert_eq!(owners.len(), 4);
    assert_eq!(counts["CP"], 4500);
    assert_eq!(counts["CL"], 4500);
    assert_eq!(counts["CF"], 1500);
    assert_eq!(counts["CN"], 900);
    assert_eq!(counts["CR"], 600);
    assert_eq!(counts["CS"], 12);
    assert_eq!(counts["OP"], 3200);
    assert_eq!(counts["OL"], 3200);
}
