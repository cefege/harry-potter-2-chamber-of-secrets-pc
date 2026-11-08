use hp2_abi::{FVector, engine::generated::FRotator};
use hp2_engine::decal::*;
use std::{ffi::c_void, ptr, sync::LazyLock};
fn v(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}
fn token(n: usize) -> *mut c_void {
    n as *mut c_void
}
#[derive(Clone)]
struct Stored {
    owner: bool,
    texture: usize,
    vertices: [FVector; 4],
    nodes: Vec<i32>,
}
#[derive(Clone)]
struct Surface {
    data: Hp2DecalSurface,
    polygon: Vec<FVector>,
    decals: Vec<Stored>,
    nodes: Vec<i32>,
}
#[derive(Clone)]
struct World {
    enabled: i32,
    state: Hp2DecalState,
    surfaces: Vec<Surface>,
    list: Vec<i32>,
    calls: usize,
    fail_at: usize,
    rng: usize,
    logs: usize,
    traces: usize,
    secondary: i32,
    next_plane: i32,
    removals: Vec<(i32, i32)>,
}
impl World {
    fn new() -> Self {
        let surface = |x| Surface {
            data: Hp2DecalSurface {
                base: v(x, -8., -8.),
                normal: v(1., 0., 0.),
                texture: token(9),
                flags: 0,
                node_count: 1,
            },
            polygon: vec![v(x, -8., -8.), v(x, 8., -8.), v(x, 8., 8.), v(x, -8., 8.)],
            decals: vec![],
            nodes: vec![if x == 10. { 0 } else { 1 }],
        };
        Self {
            enabled: 1,
            state: Hp2DecalState {
                texture: token(1),
                location: v(20., 0., 0.),
                rotation: FRotator {
                    pitch: 0,
                    yaw: 0,
                    roll: 0,
                },
                draw_scale: 1.,
                texture_u_size: 32,
                multi: 0,
            },
            surfaces: vec![surface(10.), surface(12.)],
            list: vec![],
            calls: 0,
            fail_at: usize::MAX,
            rng: 0,
            logs: 0,
            traces: 0,
            secondary: 0,
            next_plane: -1,
            removals: vec![],
        }
    }
    fn context(&mut self) -> *mut c_void {
        (self as *mut Self).cast()
    }
    fn call(&mut self) -> i32 {
        self.calls += 1;
        if self.calls == self.fail_at { 77 } else { 0 }
    }
    fn attach(&mut self, dir: FVector) -> DecalResult<*mut c_void> {
        unsafe { attach(&SERVICES, self.context(), 20., dir) }
    }
    fn detach(&mut self) -> DecalResult<()> {
        unsafe { detach(&SERVICES, self.context()) }
    }
}
macro_rules! world {
    ($c:expr) => {{
        let w = unsafe { &mut *($c as *mut World) };
        let status = w.call();
        if status != 0 {
            return status;
        }
        w
    }};
}
unsafe extern "C" fn enabled(c: *mut c_void, p: *mut i32) -> i32 {
    let w = world!(c);
    unsafe { *p = w.enabled };
    0
}
unsafe extern "C" fn state(c: *mut c_void, p: *mut Hp2DecalState) -> i32 {
    let w = world!(c);
    unsafe { *p = w.state };
    0
}
unsafe extern "C" fn multi(c: *mut c_void, n: i32) -> i32 {
    world!(c).state.multi = n;
    0
}
unsafe extern "C" fn log(c: *mut c_void) -> i32 {
    world!(c).logs += 1;
    0
}
unsafe extern "C" fn random(c: *mut c_void, p: *mut FVector) -> i32 {
    world!(c).rng += 1;
    unsafe { *p = v(0., 1., 0.) };
    0
}
unsafe extern "C" fn trig() -> *const f32 {
    static TRIG: LazyLock<[f32; 16384]> = LazyLock::new(|| {
        std::array::from_fn(|i| ((i as f64) * std::f64::consts::TAU / 16384.).sin() as f32)
    });
    TRIG.as_ptr()
}
unsafe extern "C" fn line(
    c: *mut c_void,
    _: *const FVector,
    _: *const FVector,
    p: *mut i32,
) -> i32 {
    let w = world!(c);
    unsafe { *p = if w.traces == 0 { 0 } else { w.secondary } };
    w.traces += 1;
    0
}
unsafe extern "C" fn surface(c: *mut c_void, n: i32, p: *mut Hp2DecalSurface) -> i32 {
    let w = world!(c);
    unsafe { *p = w.surfaces[n as usize].data };
    0
}
unsafe extern "C" fn node(c: *mut c_void, n: i32, p: *mut Hp2DecalNode) -> i32 {
    let w = world!(c);
    unsafe {
        *p = Hp2DecalNode {
            surface: n,
            next_plane: if n == 0 { w.next_plane } else { -1 },
            vertex_count: w.surfaces[n as usize].polygon.len() as i32,
        }
    };
    0
}
unsafe extern "C" fn vertex(c: *mut c_void, n: i32, j: i32, p: *mut FVector) -> i32 {
    let w = world!(c);
    unsafe { *p = w.surfaces[n as usize].polygon[j as usize] };
    0
}
unsafe extern "C" fn surface_node(c: *mut c_void, n: i32, j: i32, p: *mut i32) -> i32 {
    let w = world!(c);
    unsafe { *p = w.surfaces[n as usize].nodes[j as usize] };
    0
}
unsafe extern "C" fn count(c: *mut c_void, n: i32, p: *mut i32) -> i32 {
    let w = world!(c);
    unsafe { *p = w.surfaces[n as usize].decals.len() as i32 };
    0
}
unsafe extern "C" fn texture(c: *mut c_void, n: i32, j: i32, p: *mut *mut c_void) -> i32 {
    let w = world!(c);
    unsafe { *p = token(w.surfaces[n as usize].decals[j as usize].texture) };
    0
}
unsafe extern "C" fn insert(c: *mut c_void, n: i32, j: i32) -> i32 {
    let w = world!(c);
    // Force relocation of every surface's backing storage on each insertion.
    // A Rust borrow/pointer retained into any native decal would become stale.
    for surf in &mut w.surfaces {
        let moved = surf.decals.clone();
        surf.decals = moved;
    }
    w.surfaces[n as usize].decals.insert(
        j as usize,
        Stored {
            owner: true,
            texture: w.state.texture as usize,
            vertices: [v(0., 0., 0.); 4],
            nodes: vec![],
        },
    );
    0
}
unsafe extern "C" fn set_vertex(c: *mut c_void, n: i32, j: i32, k: i32, p: *const FVector) -> i32 {
    let w = world!(c);
    w.surfaces[n as usize].decals[j as usize].vertices[k as usize] = unsafe { *p };
    0
}
unsafe extern "C" fn add_node(c: *mut c_void, n: i32, j: i32, k: i32) -> i32 {
    let w = world!(c);
    w.surfaces[n as usize].decals[j as usize].nodes.push(k);
    0
}
unsafe extern "C" fn list_count(c: *mut c_void, p: *mut i32) -> i32 {
    let w = world!(c);
    unsafe { *p = w.list.len() as i32 };
    0
}
unsafe extern "C" fn list_item(c: *mut c_void, j: i32, p: *mut i32) -> i32 {
    let w = world!(c);
    unsafe { *p = w.list[j as usize] };
    0
}
unsafe extern "C" fn list_add(c: *mut c_void, n: i32) -> i32 {
    world!(c).list.push(n);
    0
}
unsafe extern "C" fn list_remove(c: *mut c_void, n: i32) -> i32 {
    world!(c).list.remove(n as usize);
    0
}
unsafe extern "C" fn owner(c: *mut c_void, n: i32, j: i32, p: *mut i32) -> i32 {
    let w = world!(c);
    unsafe { *p = w.surfaces[n as usize].decals[j as usize].owner as i32 };
    0
}
unsafe extern "C" fn remove(c: *mut c_void, n: i32, j: i32) -> i32 {
    let w = world!(c);
    w.removals.push((n, j));
    w.surfaces[n as usize].decals.remove(j as usize);
    0
}
static SERVICES: Hp2DecalServices = Hp2DecalServices {
    enabled,
    state,
    set_multi: multi,
    no_texture: log,
    random,
    trig,
    line,
    surface,
    node,
    vertex,
    surface_node,
    decal_count: count,
    decal_texture: texture,
    insert,
    set_vertex,
    add_node,
    list_count,
    list_item,
    list_add,
    list_remove,
    is_owner: owner,
    remove,
};

#[test]
fn attach_clips_in_world_space_then_stores_relative_square_and_returns_surface_texture() {
    let mut w = World::new();
    assert_eq!(w.attach(v(0., 1., 0.)), Ok(token(9)));
    assert_eq!(w.list, vec![0]);
    let d = &w.surfaces[0].decals[0];
    assert_eq!(d.nodes, vec![0]);
    for p in d.vertices {
        assert_eq!(p.x, 0.);
        assert!(((p.y - 8.).abs() - 16.).abs() < 0.001);
        assert!(((p.z - 8.).abs() - 16.).abs() < 0.001);
    }
    assert_eq!(w.rng, 0);
    w.detach().unwrap();
    assert!(w.list.is_empty());
    assert!(w.surfaces[0].decals.is_empty());
}
#[test]
fn gates_rng_and_upper_only_clamp_preserve_order() {
    let mut w = World::new();
    w.enabled = 0;
    w.state.multi = 99;
    assert_eq!(w.attach(v(0., 0., 0.)), Ok(ptr::null_mut()));
    assert_eq!(w.calls, 1);
    assert_eq!(w.state.multi, 99);
    assert_eq!(w.rng, 0);
    let mut w = World::new();
    w.state.texture = ptr::null_mut();
    w.state.multi = 99;
    assert_eq!(w.attach(v(0., 0., 0.)), Ok(ptr::null_mut()));
    assert_eq!(w.logs, 1);
    assert_eq!(w.rng, 0);
    assert_eq!(w.state.multi, 99);
    let mut w = World::new();
    w.state.multi = -3;
    w.attach(v(0., 0., 0.)).unwrap();
    assert_eq!(w.state.multi, -3);
    assert_eq!(w.traces, 1);
    assert_eq!(w.rng, 1);
    let mut w = World::new();
    w.state.multi = 99;
    w.attach(v(0., 1., 0.)).unwrap();
    assert_eq!(w.state.multi, 4);
    assert_eq!(w.traces, 17);
    assert_eq!(w.list, vec![0]);
}
#[test]
fn coplanar_chain_zeros_winding_and_edges_preserve_legacy_signs() {
    let mut w = World::new();
    let c = w.context();
    unsafe {
        assert_eq!(find_coplanar(&SERVICES, c, 0, v(10., 8., 8.)), Ok(0));
        assert_eq!(find_coplanar(&SERVICES, c, 0, v(10., 9., 0.)), Ok(-1));
    }
    w.surfaces[0].polygon.reverse();
    assert_eq!(
        unsafe { find_coplanar(&SERVICES, w.context(), 0, v(10., 0., 0.)) },
        Ok(0)
    );
    // Bow-tie signs are [zero, positive, zero, negative]. Zero resets the
    // previous sign, and the original does not compare the last to the first.
    w.surfaces[0].polygon = vec![
        v(10., 1., 1.),
        v(10., -1., 1.),
        v(10., 1., -1.),
        v(10., -1., -1.),
    ];
    assert_eq!(
        unsafe { find_coplanar(&SERVICES, w.context(), 0, v(10., 0., 0.)) },
        Ok(0)
    );
    w.surfaces[0].polygon.clear();
    w.next_plane = 1;
    assert_eq!(
        unsafe { find_coplanar(&SERVICES, w.context(), 0, v(12., 0., 0.)) },
        Ok(1)
    );
}
#[test]
fn clipping_keeps_boundary_points_and_rejects_negative_halfspace() {
    let mut p = [v(0., 0., 0.); MAX_FINAL_VERTICES];
    p[..4].copy_from_slice(&[
        v(-1., -1., 0.),
        v(1., -1., 0.),
        v(1., 1., 0.),
        v(-1., 1., 0.),
    ]);
    let mut n = 4;
    clip_edge(&mut p, &mut n, v(0., 0., 0.), v(1., 0., 0.));
    assert_eq!(n, 4);
    assert!(p[..n].iter().all(|p| p.x >= 0.));
    assert_eq!(p[..n].iter().filter(|p| p.x == 0.).count(), 2);
    clip_edge(&mut p, &mut n, v(2., 0., 0.), v(1., 0., 0.));
    assert_eq!(n, 0);
}
#[test]
#[should_panic(expected = "NumPts < FBspNode::MAX_FINAL_VERTICES")]
fn clipping_capacity_is_checked_not_silently_truncated() {
    let mut p = [v(1., 0., 0.); MAX_FINAL_VERTICES];
    p[0].x = -1.;
    let mut n = 23;
    clip_edge(&mut p, &mut n, v(0., 0., 0.), v(1., 0., 0.));
}
#[test]
fn normalization_threshold_and_degenerate_axis_do_not_consume_randomness() {
    let center = v(10., 0., 0.);
    let normal = v(1., 0., 0.);
    let p = square_vertices(center, center, normal, v(1., 0., 0.), false, 1., 32);
    assert_eq!(p, [center; 4]);
    let tiny = square_vertices(center, center, normal, v(0., 0.00001, 0.), true, 1., 32);
    assert!(tiny[0].y > 0. && tiny[0].y < 0.001);
    let normal_length = square_vertices(center, center, normal, v(0., 0.001, 0.), true, 1., 32);
    assert!(normal_length[0].y > 22.);
    let mut w = World::new();
    w.attach(v(1., 0., 0.)).unwrap();
    assert_eq!(w.rng, 0);
}
#[test]
fn secondary_projection_and_relocation_preserve_main_handle_and_deduplicate() {
    let mut w = World::new();
    w.state.multi = 2;
    w.secondary = 1;
    w.attach(v(0., 1., 0.)).unwrap();
    assert_eq!(w.list, vec![0, 1]);
    for s in &w.surfaces {
        assert_eq!(s.decals.len(), 1);
        assert_eq!(s.decals[0].nodes, s.nodes);
        for p in s.decals[0].vertices {
            assert_eq!(p.x, 0.);
            assert!(((p.y - 8.).abs() - 16.).abs() < 0.001);
        }
    }
    w.detach().unwrap();
    assert_eq!(w.removals, vec![(1, 0), (0, 0)]);
}
#[test]
fn autopan_and_secondary_cosine_rejections_happen_before_insertion() {
    for flags in [0x200, 0x400] {
        let mut w = World::new();
        w.surfaces[0].data.flags = flags;
        assert_eq!(w.attach(v(0., 1., 0.)), Ok(ptr::null_mut()));
        assert!(w.list.is_empty());
    }
    let mut w = World::new();
    w.state.multi = 2;
    w.secondary = 1;
    w.surfaces[1].data.flags = 0x400;
    w.attach(v(0., 1., 0.)).unwrap();
    assert_eq!(w.list, vec![0]);
    for (cosine, accepted) in [(0.6_f32, false), (0.7, false), (0.7001, true), (-0.8, true)] {
        let mut w = World::new();
        w.state.multi = 2;
        w.secondary = 1;
        // A genuine tilted polygon; multi=2 probes its center first.
        let z = (1. - cosine * cosine).sqrt();
        w.surfaces[1].data.normal = v(cosine, 0., z);
        w.surfaces[1].data.base = v(10., 0., 0.);
        w.surfaces[1].polygon = vec![
            v(10. + 8. * z, -8., -8. * cosine),
            v(10. + 8. * z, 8., -8. * cosine),
            v(10. - 8. * z, 8., 8. * cosine),
            v(10. - 8. * z, -8., 8. * cosine),
        ];
        assert_eq!(
            unsafe { find_coplanar(&SERVICES, w.context(), 1, v(10., 0., 0.)) },
            Ok(1)
        );
        w.attach(v(0., 1., 0.)).unwrap();
        assert_eq!(
            w.list,
            if accepted { vec![0, 1] } else { vec![0] },
            "cosine {cosine}"
        );
    }
}
#[test]
fn insertion_precedes_first_matching_texture_and_detach_removes_only_first_owner() {
    let mut w = World::new();
    let stored = |owner, texture| Stored {
        owner,
        texture,
        vertices: [v(0., 0., 0.); 4],
        nodes: vec![],
    };
    w.surfaces[0].decals = vec![stored(false, 2), stored(false, 1), stored(false, 1)];
    w.attach(v(0., 1., 0.)).unwrap();
    assert!(!w.surfaces[0].decals[0].owner);
    assert!(w.surfaces[0].decals[1].owner);
    assert!(!w.surfaces[0].decals[2].owner);
    w.surfaces[0].decals.push(stored(true, 3));
    w.list.push(0);
    w.detach().unwrap();
    assert_eq!(w.removals, vec![(0, 1), (0, 3)]);
    assert_eq!(w.surfaces[0].decals.len(), 3);
    w.list.push(0);
    w.detach().unwrap();
    assert_eq!(w.removals.len(), 2);
    assert!(w.list.is_empty());
}
#[test]
fn every_throwing_leaf_preserves_first_status_and_stops_callbacks() {
    let mut original = World::new();
    original.state.multi = 2;
    original.secondary = 1;
    let mut success = original.clone();
    success.attach(v(0., 0., 0.)).unwrap();
    for at in 1..=success.calls {
        let mut w = original.clone();
        w.fail_at = at;
        assert_eq!(w.attach(v(0., 0., 0.)), Err(77), "leaf {at}");
        assert_eq!(w.calls, at);
    }
    let mut baseline = success.clone();
    baseline.calls = 0;
    baseline.detach().unwrap();
    for at in 1..=baseline.calls {
        let mut w = success.clone();
        w.calls = 0;
        w.fail_at = at;
        assert_eq!(w.detach(), Err(77), "detach leaf {at}");
        assert_eq!(w.calls, at);
    }
}

#[test]
fn clipping_precedes_relative_conversion_even_with_distant_plane_base() {
    let mut w = World::new();
    w.surfaces[0].data.base = v(10., 100., 200.);
    w.attach(v(0., 1., 0.)).unwrap();
    let d = &w.surfaces[0].decals[0];
    assert_eq!(d.nodes, vec![0]);
    for p in d.vertices {
        assert_eq!(p.x, 0.);
        assert!(((p.y + 100.).abs() - 16.).abs() < 0.001);
        assert!(((p.z + 200.).abs() - 16.).abs() < 0.001);
    }
}

#[test]
fn missing_surface_texture_still_attaches_and_node_order_keeps_duplicates() {
    let mut w = World::new();
    w.surfaces[0].data.texture = ptr::null_mut();
    w.surfaces[0].data.node_count = 2;
    w.surfaces[0].nodes = vec![0, 0];
    assert_eq!(w.attach(v(0., 1., 0.)), Ok(ptr::null_mut()));
    assert_eq!(w.list, vec![0]);
    assert_eq!(w.surfaces[0].decals[0].nodes, vec![0, 0]);
    assert_eq!(w.logs, 0);
}
