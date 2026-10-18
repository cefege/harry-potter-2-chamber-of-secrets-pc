//! Port of `Engine/Src/UnTrace.cpp`: BSP model traces (point, line, box
//! convolution, region lookup, nearest vertex, sphere filter precompute).
//! The C++ shells keep class identity checks and owner transforms; this
//! module sees only plain values and the `UModel` fields it reads.

use std::ffi::c_void;

use hp2_abi::engine::generated::{FCheckResult, FPoly};
use hp2_abi::render_scene::FBspNode;
use hp2_abi::{FCoords, FPlane, FVector};

use crate::math::app_sqrt;
use crate::prim::{
    Aabb, BspArrays, INDEX_NONE, Owner, add, box_push_out, clamp_f, cross, dot, neg, normalize,
    plane_dot, scale, sub, transform_plane_by_ortho, transform_point, transform_vector, v3,
};
use crate::vec;

/// `FBspNode` flags (`UnObj.h`).
pub const NF_NOT_CSG: u8 = 0x01;
pub const NF_IS_NEW: u8 = 0x20;
pub const NF_IS_FRONT: u8 = 0x40;
pub const NF_IS_BACK: u8 = 0x80;
/// Combined with `ExtraNodeFlags` as a `DWORD`.
pub const NF_BRIGHT_CORNERS: u32 = 0x10;

/// `FBoxCheckInfo` convolution flags.
const CV_XM: u32 = 0x01;
const CV_XP: u32 = 0x02;
const CV_YM: u32 = 0x04;
const CV_YP: u32 = 0x08;
const CV_ZM: u32 = 0x10;
const CV_ZP: u32 = 0x20;

/// Bit set on `LeafHulls` entries that flip the hull plane.
const HULL_FLIP: i32 = 0x4000_0000;
const MAX_HULLS: usize = 64;

/// The `UModel` fields the trace code reads. `primitive` is the model's own
/// object pointer, written to `Hit.Primitive`.
#[derive(Clone, Copy)]
pub struct ModelView<'a> {
    pub bsp: BspArrays<'a>,
    pub leaf_hulls: &'a [i32],
    pub bounding_box: Aabb,
    pub root_outside: bool,
    pub num_zones: i32,
    pub primitive: *mut c_void,
}

#[inline]
fn xyz(p: FPlane) -> FVector {
    v3(p.x, p.y, p.z)
}

#[inline]
fn zero_plane() -> FPlane {
    FPlane {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    }
}

fn unit_coords() -> FCoords {
    FCoords {
        origin: v3(0.0, 0.0, 0.0),
        x_axis: v3(1.0, 0.0, 0.0),
        y_axis: v3(0.0, 1.0, 0.0),
        z_axis: v3(0.0, 0.0, 1.0),
    }
}

fn plane_flip(p: FPlane) -> FPlane {
    FPlane {
        x: -p.x,
        y: -p.y,
        z: -p.z,
        w: -p.w,
    }
}

/// `FPlane(Base, Normal)`.
fn plane_from(base: FVector, normal: FVector) -> FPlane {
    FPlane {
        x: normal.x,
        y: normal.y,
        z: normal.z,
        w: dot(base, normal),
    }
}

fn actor_ptr(owner: Option<Owner>) -> *mut c_void {
    owner.map_or(std::ptr::null_mut(), |o| o.actor)
}

fn is_nonzero(v: FVector) -> bool {
    v.x != 0.0 || v.y != 0.0 || v.z != 0.0
}

fn child_of(node: &FBspNode, front: bool) -> i32 {
    if front { node.i_front } else { node.i_back }
}

/// `FBspNode::IsCsg`.
fn is_csg(node: &FBspNode, extra: u32) -> bool {
    node.num_vertices > 0
        && (u32::from(node.node_flags) & (u32::from(NF_IS_NEW) | u32::from(NF_NOT_CSG) | extra))
            == 0
}

/// `FBspNode::ChildOutside`.
fn child_outside(node: &FBspNode, front: bool, outside: bool, extra: u32) -> bool {
    if front {
        outside || is_csg(node, extra)
    } else {
        outside && !is_csg(node, extra)
    }
}

/// `FBoxPushOut`-style hull flags of `FBoxCheckInfo::SetupHulls`.
fn hull_flags(h: FPlane) -> u32 {
    let axis = |v: f32, neg_bit: u32, pos_bit: u32| {
        if v < 0.0 {
            neg_bit
        } else if v > 0.0 {
            pos_bit
        } else {
            0
        }
    };
    axis(h.x, CV_XM, CV_XP) | axis(h.y, CV_YM, CV_YP) | axis(h.z, CV_ZM, CV_ZP)
}

/// `UModel::GetRenderBoundingBox` (`UnTrace.cpp:15`). `polys` is `Some` when
/// the model has a polygon list; each entry holds one poly's vertices.
pub fn get_render_bounding_box(
    m: &ModelView,
    owner: Option<Owner>,
    exact: bool,
    polys: Option<&[FPoly]>,
) -> Aabb {
    let Some(o) = owner else {
        return m.bounding_box;
    };
    match polys {
        Some(polys) if exact => {
            let c = o.to_world;
            let mut b = Aabb::empty();
            for poly in polys {
                let n = poly.num_vertices.max(0) as usize;
                for v in &poly.vertex[..n] {
                    b.add_point(transform_point(*v, &c));
                }
            }
            b
        }
        _ => m.bounding_box.transform_by(&o.to_world),
    }
}

/// `FIntersectPlanes2` (`UnMath.h:2628`). The C++ bool result is ignored by
/// every caller, so only `(I, D)` is returned.
fn intersect_planes2(p1: FPlane, p2: FPlane) -> (FVector, FVector) {
    let a = xyz(p1);
    let b = xyz(p2);
    let mut d = cross(a, b);
    let dd = vec::size_squared(d);
    if dd < 0.001f32 * 0.001f32 {
        return (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0));
    }
    let num = add(scale(cross(b, d), p1.w), scale(cross(d, a), p2.w));
    let i = vec::div(num, dd);
    normalize(&mut d);
    (i, d)
}

/// `FBoxCheckInfo` hull state, shared by the point and line box traces.
pub struct Hulls<'a> {
    num: usize,
    hulls: [FPlane; MAX_HULLS],
    flags: [u32; MAX_HULLS],
    hull_nodes: &'a [i32],
    bbox: Aabb,
}

impl<'a> Hulls<'a> {
    fn new() -> Self {
        Hulls {
            num: 0,
            hulls: [zero_plane(); MAX_HULLS],
            flags: [0; MAX_HULLS],
            hull_nodes: &[],
            bbox: Aabb::empty(),
        }
    }

    /// `FBoxCheckInfo::SetupHulls`. `coords` is `Some` when the trace has an owner.
    fn setup(&mut self, m: &ModelView<'a>, coords: Option<&FCoords>, i_collision_bound: i32) {
        let leaf: &'a [i32] = m.leaf_hulls;
        let base = i_collision_bound as usize;
        self.hull_nodes = &leaf[base..];
        self.num = 0;
        while self.num < MAX_HULLS
            && self
                .hull_nodes
                .get(self.num)
                .is_some_and(|&n| n != INDEX_NONE)
        {
            let raw = self.hull_nodes[self.num];
            let mut hull = m.bsp.nodes[(raw & !HULL_FLIP) as usize].plane;
            if let Some(c) = coords {
                hull = transform_plane_by_ortho(hull, c);
            }
            if raw & HULL_FLIP != 0 {
                hull = plane_flip(hull);
            }
            self.hulls[self.num] = hull;
            self.flags[self.num] = hull_flags(hull);
            self.num += 1;
        }
        // Bounds follow the hull list as six raw floats.
        let t = base + self.num + 1;
        let f = |k: usize| f32::from_bits(leaf[t + k] as u32);
        self.bbox = Aabb::from_min_max(v3(f(0), f(1), f(2)), v3(f(3), f(4), f(5)));
    }
}

/// Clipping hooks used by `CLIP_COLLISION_PRIMITIVE` and `CONVOLVE_EDGE`.
trait HullClip {
    fn hulls(&self) -> &Hulls<'_>;
    fn has_owner(&self) -> bool;
    /// `ClipTo(Hull, Item)`; `false` means the C++ `goto NoBlock`.
    fn clip_to(&mut self, hull: FPlane, item: i32) -> bool;
}

/// `CONVOLVE_EDGE`. Returns `false` for the C++ `goto NoBlock`.
fn convolve_edge<C: HullClip>(
    c: &mut C,
    hi: FPlane,
    hj: FPlane,
    axis: FVector,
    or2: u32,
    mask: u32,
) -> bool {
    if or2 & mask != mask {
        return true;
    }
    let c1 = cross(axis, xyz(hi));
    let c2 = cross(axis, xyz(hj));
    if dot(c1, c2) > 0.001 {
        let (i, d) = intersect_planes2(hi, hj);
        let mut a = vec::unsafe_normal(cross(axis, d));
        if dot(xyz(hi), a) < 0.0 {
            a = scale(a, -1.0);
        }
        if !c.clip_to(plane_from(i, a), INDEX_NONE) {
            return false;
        }
    }
    true
}

/// `CLIP_COLLISION_PRIMITIVE`. Returns `false` for the C++ `goto NoBlock`.
fn clip_collision_primitive<C: HullClip>(c: &mut C) -> bool {
    let n = c.hulls().num;
    for i in 0..n {
        let (hull, item) = {
            let hs = c.hulls();
            (hs.hulls[i], hs.hull_nodes[i] & !HULL_FLIP)
        };
        if !c.clip_to(hull, item) {
            return false;
        }
    }
    if !c.has_owner() {
        let b = c.hulls().bbox;
        let planes = [
            FPlane {
                x: 0.0,
                y: 0.0,
                z: -1.0,
                w: 0.1 - b.min.z,
            },
            FPlane {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: b.max.z + 0.1,
            },
            FPlane {
                x: -1.0,
                y: 0.0,
                z: 0.0,
                w: 0.1 - b.min.x,
            },
            FPlane {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: b.max.x - 0.1,
            },
            FPlane {
                x: 0.0,
                y: -1.0,
                z: 0.0,
                w: 0.1 - b.min.y,
            },
            FPlane {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: b.max.y - 0.1,
            },
        ];
        for p in planes {
            if !c.clip_to(p, INDEX_NONE) {
                return false;
            }
        }
    }
    for i in 0..n {
        for j in 0..i {
            let (hi, hj, or2) = {
                let hs = c.hulls();
                (hs.hulls[i], hs.hulls[j], hs.flags[i] | hs.flags[j])
            };
            if !convolve_edge(c, hi, hj, v3(1.0, 0.0, 0.0), or2, CV_XM | CV_XP) {
                return false;
            }
            if !convolve_edge(c, hi, hj, v3(0.0, 1.0, 0.0), or2, CV_YM | CV_YP) {
                return false;
            }
            if !convolve_edge(c, hi, hj, v3(0.0, 0.0, 1.0), or2, CV_ZM | CV_ZP) {
                return false;
            }
        }
    }
    true
}

/// `FBoxPointCheckInfo`.
struct BoxPointTrace<'a, 'h> {
    hit: &'h mut FCheckResult,
    model: ModelView<'a>,
    owner: Option<Owner>,
    coords: Option<FCoords>,
    point: FVector,
    extent: FVector,
    extra: u32,
    best_dist: f32,
    hs: Hulls<'a>,
}

impl HullClip for BoxPointTrace<'_, '_> {
    fn hulls(&self) -> &Hulls<'_> {
        &self.hs
    }

    fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    fn clip_to(&mut self, hull: FPlane, item: i32) -> bool {
        let h = xyz(hull);
        let push_out = box_push_out(h, self.extent);
        let dist = plane_dot(hull, self.point);
        if dist > 0.0 && dist < self.best_dist && dist < push_out {
            self.best_dist = dist;
            self.hit.location = add(self.point, scale(scale(h, 1.02), push_out - dist));
            self.hit.normal = h;
            self.hit.actor = actor_ptr(self.owner);
            self.hit.primitive = self.model.primitive;
            self.hit.item = item;
            self.hit.time = 0.0;
        }
        dist < push_out
    }
}

impl BoxPointTrace<'_, '_> {
    /// `FBoxPointCheckInfo::BoxPointCheck`. Returns the C++ `Result` (`true` = clear).
    fn run(&mut self, mut i_parent: i32, mut i_node: i32, mut outside: bool) -> bool {
        let m = self.model;
        let nodes = m.bsp.nodes;
        let mut result = true;
        while i_node != INDEX_NONE {
            let node = &nodes[i_node as usize];
            let plane = match self.coords {
                Some(c) => transform_plane_by_ortho(node.plane, &c),
                None => node.plane,
            };
            let push_out = box_push_out(xyz(plane), scale(self.extent, 1.1));
            let dist = plane_dot(plane, self.point);
            if dist > -push_out
                && !self.run(i_node, node.i_front, outside || is_csg(node, self.extra))
            {
                result = false;
            }
            i_parent = i_node;
            i_node = node.i_back;
            outside = outside && !is_csg(node, self.extra);
            if dist > push_out {
                return result;
            }
        }
        let parent = &nodes[i_parent as usize];
        if !outside && parent.i_collision_bound != INDEX_NONE {
            self.hs
                .setup(&m, self.coords.as_ref(), parent.i_collision_bound);
            if clip_collision_primitive(self) {
                result = false;
            }
        }
        result
    }
}

/// `UModel::PointCheck` (`UnTrace.cpp:284`). Returns the C++ `Outside`.
pub fn model_point_check(
    hit: &mut FCheckResult,
    m: &ModelView,
    owner: Option<Owner>,
    location: FVector,
    extent: FVector,
    extra: u32,
) -> bool {
    let m = *m;
    hit.normal = v3(0.0, 0.0, 0.0);
    hit.location = location;
    hit.primitive = m.primitive;
    hit.actor = actor_ptr(owner);
    hit.time = 0.0;

    let mut outside = m.root_outside;
    let nodes = m.bsp.nodes;
    if !nodes.is_empty() {
        if is_nonzero(extent) {
            let mut trace = BoxPointTrace {
                hit,
                model: m,
                owner,
                coords: owner.map(|o| o.to_world),
                point: location,
                extent,
                extra,
                best_dist: 100000.0,
                hs: Hulls::new(),
            };
            outside = trace.run(0, 0, outside);
        } else {
            let coords = owner.map_or(unit_coords(), |o| o.to_world);
            let mut i_prev: i32;
            let mut i_node = 0i32;
            let mut is_front: bool;
            loop {
                i_prev = i_node;
                let node = &nodes[i_node as usize];
                is_front = plane_dot(transform_plane_by_ortho(node.plane, &coords), location) > 0.0;
                outside = child_outside(node, is_front, outside, 0);
                i_node = child_of(node, is_front);
                if i_node == INDEX_NONE {
                    break;
                }
            }
            hit.item = i_prev * 2 + i32::from(is_front);
        }
    }
    outside
}

/// `LineCheckInner` (`UnTrace.cpp:338`), the fast line check walker.
fn line_check_inner(
    nodes: &[FBspNode],
    mut i_node: i32,
    mut end: FVector,
    start: FVector,
    outside: bool,
) -> bool {
    let mut outside = u8::from(outside);
    while i_node != INDEX_NONE {
        let node = &nodes[i_node as usize];
        let d1 = plane_dot(node.plane, start);
        let d2 = plane_dot(node.plane, end);
        let not_csg = node.node_flags & NF_NOT_CSG;
        // Sign-bit test: `*(INT*)&Dist >= 0` (-0.0 counts as back).
        let g1 = u8::from(d1.to_bits() as i32 >= 0);
        let g2 = u8::from(d2.to_bits() as i32 >= 0);
        if g1 != g2 {
            let alpha = d1 / (d1 - d2);
            let middle = add(start, scale(sub(end, start), alpha));
            let child_out = g2 ^ ((g2 ^ outside) & not_csg);
            if !line_check_inner(nodes, child_of(node, g2 != 0), middle, end, child_out != 0) {
                return false;
            }
            end = middle;
        }
        outside = g1 ^ ((g1 ^ outside) & not_csg);
        i_node = child_of(node, g1 != 0);
    }
    outside != 0
}

/// `UModel::FastLineCheck`.
pub fn fast_line_check(m: &ModelView, end: FVector, start: FVector) -> bool {
    let nodes = m.bsp.nodes;
    if nodes.is_empty() {
        return m.root_outside;
    }
    line_check_inner(nodes, 0, end, start, m.root_outside)
}

/// Recursive `LineCheck` (`UnTrace.cpp:380`). `coords` is the owner's
/// `ToWorld` when present. `out_of_corner` is the C++ `GOutOfCorner`.
#[allow(clippy::too_many_arguments)]
fn line_check(
    hit: &mut FCheckResult,
    m: &ModelView,
    coords: Option<&FCoords>,
    mut i_hit: i32,
    mut i_node: i32,
    end: FVector,
    mut start: FVector,
    mut outside: bool,
    flags: u32,
    out_of_corner: &mut bool,
) -> bool {
    let nodes = m.bsp.nodes;
    let no_bright = flags & !NF_BRIGHT_CORNERS;
    while i_node != INDEX_NONE {
        let node = &nodes[i_node as usize];
        let plane = match coords {
            Some(c) => transform_plane_by_ortho(node.plane, c),
            None => node.plane,
        };
        let dist1 = plane_dot(plane, start);
        let dist2 = plane_dot(plane, end);
        if dist1 > -0.001 && dist2 > -0.001 {
            outside |= is_csg(node, no_bright);
            i_node = node.i_front;
        } else if dist1 < 0.001 && dist2 < 0.001 {
            outside &= !is_csg(node, no_bright);
            i_node = node.i_back;
        } else {
            let middle = add(start, scale(sub(start, end), dist1 / (dist2 - dist1)));
            let front_first = dist1 > 0.0;
            if !line_check(
                hit,
                m,
                coords,
                i_hit,
                child_of(node, front_first),
                middle,
                start,
                child_outside(node, front_first, outside, flags),
                flags,
                out_of_corner,
            ) {
                return false;
            }
            outside = child_outside(node, !front_first, outside, flags);
            i_hit = i_node;
            i_node = child_of(node, !front_first);
            start = middle;
        }
    }
    if !outside {
        if *out_of_corner || flags & NF_BRIGHT_CORNERS == 0 {
            let hit_node = &nodes[i_hit as usize];
            hit.location = start;
            hit.normal = xyz(hit_node.plane);
            hit.primitive = m.primitive;
            hit.item = i_hit;
        } else {
            outside = true;
        }
    } else {
        *out_of_corner = true;
    }
    outside
}

/// `FBoxLineCheckInfo`. `ExtraFlags` is stored but not consulted by the
/// box line walk (`ChildOutside` gets the default 0), as in the C++.
struct BoxLineTrace<'a, 'h> {
    hit: &'h mut FCheckResult,
    model: ModelView<'a>,
    owner: Option<Owner>,
    coords: Option<FCoords>,
    start: FVector,
    end: FVector,
    extent: FVector,
    dist: f32,
    did_hit: bool,
    t0: f32,
    t1: f32,
    local_hit: FVector,
    local_item: i32,
    hs: Hulls<'a>,
}

impl HullClip for BoxLineTrace<'_, '_> {
    fn hulls(&self) -> &Hulls<'_> {
        &self.hs
    }

    fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    fn clip_to(&mut self, hull: FPlane, item: i32) -> bool {
        let h = xyz(hull);
        let push_out = box_push_out(h, self.extent);
        let d0 = plane_dot(hull, self.start);
        let d1 = plane_dot(hull, self.end);
        let mut adj_d0 = d0 - push_out;
        if d0 > d1 && adj_d0 >= -push_out && adj_d0 < 0.0 {
            adj_d0 = 0.0;
        }
        let t = adj_d0 / (d0 - d1);
        if d0 - d1 < -0.00001 {
            if t < self.t1 {
                self.t1 = t;
            }
        } else if d0 - d1 > 0.00001 {
            if t > self.t0 {
                self.t0 = t;
                self.local_hit = h;
                if item >= 0 {
                    self.local_item = item;
                }
            }
        } else if d0 > push_out && d1 > push_out {
            return false;
        }
        self.t0 < self.t1
    }
}

impl BoxLineTrace<'_, '_> {
    /// `FBoxLineCheckInfo::BoxLineCheck`.
    fn run(&mut self, mut i_parent: i32, mut i_node: i32, mut outside: bool) {
        let m = self.model;
        let nodes = m.bsp.nodes;
        while i_node != INDEX_NONE {
            let node = &nodes[i_node as usize];
            let plane = match self.coords {
                Some(c) => transform_plane_by_ortho(node.plane, &c),
                None => node.plane,
            };
            let d0 = plane_dot(plane, self.start);
            let d1 = plane_dot(plane, self.end);
            let push_out = box_push_out(xyz(plane), scale(self.extent, 1.1));
            let use_ = [
                d0 <= push_out || d1 <= push_out,
                d0 >= -push_out || d1 >= -push_out,
            ];
            let front_first = d0 >= d1;
            if use_[usize::from(front_first)] {
                self.run(
                    i_node,
                    child_of(node, front_first),
                    child_outside(node, front_first, outside, 0),
                );
            }
            if !use_[usize::from(!front_first)] {
                return;
            }
            i_parent = i_node;
            i_node = child_of(node, !front_first);
            outside = child_outside(node, !front_first, outside, 0);
        }
        let parent = &nodes[i_parent as usize];
        if !outside && parent.i_collision_bound != INDEX_NONE {
            self.hs
                .setup(&m, self.coords.as_ref(), parent.i_collision_bound);
            self.t0 = -1.0;
            self.t1 = self.hit.time;
            self.local_hit = v3(0.0, 0.0, 0.0);
            self.local_item = i_parent;
            if clip_collision_primitive(self)
                && self.t0 > -1.0
                && self.t0 < self.t1
                && self.t1 > 0.0
            {
                self.hit.time = self.t0;
                self.hit.normal = self.local_hit;
                self.hit.actor = actor_ptr(self.owner);
                self.hit.primitive = m.primitive;
                self.hit.item = self.local_item;
                self.did_hit = true;
            }
        }
    }
}

/// `UModel::LineCheck` (`UnTrace.cpp:569`). Returns the C++ `UBOOL` (`true` = unblocked).
pub fn model_line_check(
    hit: &mut FCheckResult,
    m: &ModelView,
    owner: Option<Owner>,
    end: FVector,
    start: FVector,
    extent: FVector,
    flags: u32,
) -> bool {
    if m.bsp.nodes.is_empty() {
        if m.root_outside {
            return true;
        }
        hit.time = 0.0;
        hit.normal = v3(0.0, 0.0, 1.0);
        hit.location = start;
        return false;
    }
    if !is_nonzero(extent) {
        let mut out_of_corner = false;
        let coords = owner.map(|o| o.to_world);
        let outside = line_check(
            hit,
            m,
            coords.as_ref(),
            0,
            0,
            end,
            start,
            m.root_outside,
            flags,
            &mut out_of_corner,
        );
        if !outside {
            let v = sub(end, start);
            hit.time = dot(sub(hit.location, start), v) / dot(v, v);
            hit.time = clamp_f(hit.time - 0.5 / vec::size(v), 0.0, 1.0);
            hit.location = add(start, scale(v, hit.time));
            hit.actor = actor_ptr(owner);
            if let Some(o) = owner {
                hit.normal = transform_vector(hit.normal, &o.to_world);
            }
            // Reject "impossible" normals (more than 90 degrees from the move).
            if dot(neg(vec::unsafe_normal(v)), hit.normal) < 0.0 {
                hit.normal = neg(hit.normal);
            }
        }
        return outside;
    }

    hit.time = 2.0;
    let (did_hit, dist) = {
        let mut trace = BoxLineTrace {
            hit,
            model: *m,
            owner,
            coords: owner.map(|o| o.to_world),
            start,
            end,
            extent,
            dist: vec::size(sub(end, start)),
            did_hit: false,
            t0: -1.0,
            t1: 0.0,
            local_hit: v3(0.0, 0.0, 0.0),
            local_item: INDEX_NONE,
            hs: Hulls::new(),
        };
        trace.run(0, 0, m.root_outside);
        (trace.did_hit, trace.dist)
    };
    if did_hit {
        hit.time = clamp_f(hit.time - 0.5 / dist, 0.0, 1.0);
        hit.location = add(start, scale(sub(end, start), hit.time));
        hit.time == 1.0
    } else {
        true
    }
}

/// `FPointRegion` as far as the model knows it. The shell maps `zone_number`
/// through `Zones[]` to the zone actor.
pub struct PointRegion {
    pub i_leaf: i32,
    pub zone_number: u8,
}

/// `UModel::PointRegion` (`UnTrace.cpp:650`).
pub fn point_region(m: &ModelView, location: FVector) -> PointRegion {
    let mut result = PointRegion {
        i_leaf: INDEX_NONE,
        zone_number: 0,
    };
    let nodes = m.bsp.nodes;
    if nodes.is_empty() {
        return result;
    }
    let mut i_node = 0i32;
    let mut i_parent = 0i32;
    let mut is_front = false;
    while i_node != INDEX_NONE {
        let node = &nodes[i_node as usize];
        is_front = plane_dot(node.plane, location) >= 0.0;
        i_parent = i_node;
        i_node = child_of(node, is_front);
    }
    let parent = &nodes[i_parent as usize];
    let front = usize::from(is_front);
    result.i_leaf = parent.i_leaf[front];
    result.zone_number = if m.num_zones != 0 {
        parent.i_zone[front]
    } else {
        0
    };
    result
}

#[inline]
fn dist_sq(a: FVector, b: FVector) -> f32 {
    // `FDistSquared`: `Square(A.X-B.X) + Square(A.Y-B.Y) + Square(A.Z-B.Z)`, each
    // product rounded before the sums (oracle-verified; not the `dot` FMA pattern).
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

/// `FindNearestVertex` static worker (`UnTrace.cpp:684`).
fn find_nearest_vertex_inner(
    m: &ModelView,
    source: FVector,
    dest: &mut FVector,
    mut min_radius: f32,
    mut i_node: i32,
    p_vertex: &mut i32,
) -> f32 {
    let nodes = m.bsp.nodes;
    let mut result = -1.0f32;
    while i_node != INDEX_NONE {
        let node = &nodes[i_node as usize];
        let i_back = node.i_back;
        let plane_dist = plane_dot(node.plane, source);
        if plane_dist >= -min_radius && node.i_front != INDEX_NONE {
            let temp =
                find_nearest_vertex_inner(m, source, dest, min_radius, node.i_front, p_vertex);
            if temp >= 0.0 {
                result = temp;
                min_radius = temp;
            }
        }
        if plane_dist > -min_radius && plane_dist <= min_radius {
            // Loop through all coplanars.
            while i_node != INDEX_NONE {
                let node = &nodes[i_node as usize];
                let surf = &m.bsp.surfs[node.i_surf as usize];
                let base = m.bsp.points[surf.p_base as usize];
                let temp_sq = dist_sq(source, base);
                if temp_sq < min_radius * min_radius {
                    *p_vertex = surf.p_base;
                    result = app_sqrt(f64::from(temp_sq)) as f32;
                    min_radius = result;
                    *dest = base;
                }
                let vp0 = node.i_vert_pool as usize;
                for vp in vp0..vp0 + node.num_vertices as usize {
                    let vert = m.bsp.verts[vp];
                    let vertex = m.bsp.points[vert.p_vertex as usize];
                    let temp_sq = dist_sq(source, vertex);
                    if temp_sq < min_radius * min_radius {
                        *p_vertex = vert.p_vertex;
                        result = app_sqrt(f64::from(temp_sq)) as f32;
                        min_radius = result;
                        *dest = vertex;
                    }
                }
                i_node = node.i_plane;
            }
        }
        if plane_dist > min_radius {
            break;
        }
        i_node = i_back;
    }
    result
}

/// `UModel::FindNearestVertex` (`UnTrace.cpp:751`). Returns the distance, or -1.
pub fn find_nearest_vertex(
    m: &ModelView,
    source: FVector,
    dest: &mut FVector,
    min_radius: f32,
    p_vertex: &mut i32,
) -> f32 {
    if m.bsp.nodes.is_empty() {
        return -1.0;
    }
    find_nearest_vertex_inner(m, source, dest, min_radius, 0, p_vertex)
}

/// Recursive worker for `PrecomputeSphereFilter` (`UnTrace.cpp:771`).
fn precompute_sphere_filter_inner(nodes: &mut [FBspNode], mut i_node: i32, sphere: FPlane) {
    let sphere_n = xyz(sphere);
    loop {
        let idx = i_node as usize;
        nodes[idx].node_flags &= !(NF_IS_FRONT | NF_IS_BACK);
        let dist = plane_dot(nodes[idx].plane, sphere_n);
        if dist < -sphere.w {
            nodes[idx].node_flags |= NF_IS_BACK;
            i_node = nodes[idx].i_back;
        } else {
            if dist > sphere.w {
                nodes[idx].node_flags |= NF_IS_FRONT;
            } else if nodes[idx].i_back != INDEX_NONE {
                let back = nodes[idx].i_back;
                precompute_sphere_filter_inner(nodes, back, sphere);
            }
            i_node = nodes[idx].i_front;
        }
        if i_node == INDEX_NONE {
            break;
        }
    }
}

/// `UModel::PrecomputeSphereFilter` (`UnTrace.cpp:804`).
pub fn precompute_sphere_filter(nodes: &mut [FBspNode], sphere: FPlane) {
    if !nodes.is_empty() {
        precompute_sphere_filter_inner(nodes, 0, sphere);
    }
}
