//! Decal projection and live-container policy. C++ owns UModel/FDecal storage;
//! indices, never borrowed FDecal references, survive mutating callbacks.
use crate::{
    fpoly::{cross, dot, line_plane_intersection, normalize, plane_dot, plane_from},
    math::app_sqrt,
    vec::{add, div, scale, size, sub, zero},
};
use hp2_abi::{FVector, engine::generated::FRotator};
use std::{ffi::c_void, mem::MaybeUninit, ptr};

pub const MAX_FINAL_VERTICES: usize = 24;
const AUTO_PAN: u32 = 0x600;
pub type DecalResult<T> = Result<T, i32>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2DecalState {
    pub texture: *mut c_void,
    pub location: FVector,
    pub rotation: FRotator,
    pub draw_scale: f32,
    pub texture_u_size: i32,
    pub multi: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2DecalSurface {
    pub base: FVector,
    pub normal: FVector,
    pub texture: *mut c_void,
    pub flags: u32,
    pub node_count: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2DecalNode {
    pub surface: i32,
    pub next_plane: i32,
    pub vertex_count: i32,
}

/// Every fallible leaf captures native exceptions and returns their status.
/// Geometry inputs are scalar records, not snapshots of native containers.
#[repr(C)]
pub struct Hp2DecalServices {
    pub enabled: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub state: unsafe extern "C" fn(*mut c_void, *mut Hp2DecalState) -> i32,
    pub set_multi: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub no_texture: unsafe extern "C" fn(*mut c_void) -> i32,
    pub random: unsafe extern "C" fn(*mut c_void, *mut FVector) -> i32,
    pub trig: unsafe extern "C" fn() -> *const f32,
    pub line: unsafe extern "C" fn(*mut c_void, *const FVector, *const FVector, *mut i32) -> i32,
    pub surface: unsafe extern "C" fn(*mut c_void, i32, *mut Hp2DecalSurface) -> i32,
    pub node: unsafe extern "C" fn(*mut c_void, i32, *mut Hp2DecalNode) -> i32,
    pub vertex: unsafe extern "C" fn(*mut c_void, i32, i32, *mut FVector) -> i32,
    pub surface_node: unsafe extern "C" fn(*mut c_void, i32, i32, *mut i32) -> i32,
    pub decal_count: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub decal_texture: unsafe extern "C" fn(*mut c_void, i32, i32, *mut *mut c_void) -> i32,
    pub insert: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    pub set_vertex: unsafe extern "C" fn(*mut c_void, i32, i32, i32, *const FVector) -> i32,
    pub add_node: unsafe extern "C" fn(*mut c_void, i32, i32, i32) -> i32,
    pub list_count: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub list_item: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub list_add: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub list_remove: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub is_owner: unsafe extern "C" fn(*mut c_void, i32, i32, *mut i32) -> i32,
    pub remove: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
}
fn status(s: i32) -> DecalResult<()> {
    if s == 0 { Ok(()) } else { Err(s) }
}
unsafe fn out<T>(f: impl FnOnce(*mut T) -> i32) -> DecalResult<T> {
    let mut value = MaybeUninit::uninit();
    status(f(value.as_mut_ptr()))?;
    Ok(unsafe { value.assume_init() })
}

fn opposite(a: f32, b: f32) -> bool {
    (a < 0.0 && b > 0.0) || (a > 0.0 && b < 0.0)
}

/// One clipping edge, with the original strict post-insertion capacity invariant.
/// Failure is surfaced as a guarded Rust panic, never truncation or fallback.
pub fn clip_edge(
    points: &mut [FVector; MAX_FINAL_VERTICES],
    count: &mut usize,
    vertex: FVector,
    normal: FVector,
) {
    let plane = plane_from(vertex, normal);
    let mut dots = [0.0; MAX_FINAL_VERTICES];
    for i in 0..*count {
        dots[i] = plane_dot(plane, points[i]);
    }
    let mut j = 0;
    while j < *count {
        let next = (j + 1) % *count;
        if opposite(dots[j], dots[next]) {
            assert!(
                *count + 1 < MAX_FINAL_VERTICES,
                "NumPts < FBspNode::MAX_FINAL_VERTICES"
            );
            // Plane overload uses W - dot(P1, N), not dot(Base-P1, N).
            let delta = sub(points[next], points[j]);
            let p = add(
                points[j],
                scale(
                    delta,
                    (plane.w - dot(points[j], normal)) / dot(delta, normal),
                ),
            );
            points.copy_within(j + 1..*count, j + 2);
            dots.copy_within(j + 1..*count, j + 2);
            points[j + 1] = p;
            dots[j + 1] = 0.0;
            *count += 1;
            j += 1;
        }
        j += 1;
    }
    let mut j = 0;
    while j < *count {
        if dots[j] < 0.0 {
            points.copy_within(j + 1..*count, j);
            dots.copy_within(j + 1..*count, j);
            *count -= 1;
        } else {
            j += 1;
        }
    }
}

/// # Safety
/// Services and context must address live native storage throughout the call.
pub unsafe fn find_coplanar(
    s: &Hp2DecalServices,
    c: *mut c_void,
    mut node: i32,
    point: FVector,
) -> DecalResult<i32> {
    unsafe {
        while node != -1 {
            let n = out(|p| (s.node)(c, node, p))?;
            if n.vertex_count > 0 {
                let surf = out(|p| (s.surface)(c, n.surface, p))?;
                let mut prev = out(|p| (s.vertex)(c, node, n.vertex_count - 1, p))?;
                let mut prev_dot = 0.0;
                let mut success = true;
                for i in 0..n.vertex_count {
                    let v = out(|p| (s.vertex)(c, node, i, p))?;
                    let d = plane_dot(plane_from(v, cross(surf.normal, sub(v, prev))), point);
                    if opposite(d, prev_dot) {
                        success = false;
                        break;
                    }
                    prev_dot = d;
                    prev = v;
                }
                if success {
                    return Ok(n.surface);
                }
            }
            node = n.next_plane;
        }
        Ok(-1)
    }
}
unsafe fn clipped_nodes(
    s: &Hp2DecalServices,
    c: *mut c_void,
    surface: i32,
    decal: i32,
    surf: Hp2DecalSurface,
    vertices: &[FVector; 4],
) -> DecalResult<()> {
    unsafe {
        for i in 0..surf.node_count {
            let node = out(|p| (s.surface_node)(c, surface, i, p))?;
            let n = out(|p| (s.node)(c, node, p))?;
            if n.vertex_count == 0 {
                continue;
            }
            let mut points = [zero(); MAX_FINAL_VERTICES];
            points[..4].copy_from_slice(vertices);
            let mut count = 4;
            let mut prev = out(|p| (s.vertex)(c, node, n.vertex_count - 1, p))?;
            for j in 0..n.vertex_count {
                let v = out(|p| (s.vertex)(c, node, j, p))?;
                clip_edge(&mut points, &mut count, v, cross(surf.normal, sub(v, prev)));
                if count == 0 {
                    break;
                }
                prev = v;
            }
            if count != 0 {
                status((s.add_node)(c, surface, decal, node))?;
            }
        }
        Ok(())
    }
}

/// Square-decal coordinates, including the legacy degenerate explicit-axis fallback.
pub fn square_vertices(
    center: FVector,
    base: FVector,
    normal: FVector,
    mut direction: FVector,
    mut random: bool,
    draw_scale: f32,
    u_size: i32,
) -> [FVector; 4] {
    let diag = app_sqrt(f64::from(
        draw_scale * draw_scale * u_size as f32 * u_size as f32 / 2.0,
    )) as f32;
    let mut a = zero();
    let mut b = zero();
    if !random {
        let mut axis = sub(direction, scale(normal, dot(direction, normal)));
        if axis.x.abs() < 0.0001 && axis.y.abs() < 0.0001 && axis.z.abs() < 0.0001 {
            direction = sub(base, center);
            random = true;
        } else {
            let mut other = cross(axis, normal);
            normalize(&mut axis);
            normalize(&mut other);
            a = add(axis, other);
            b = sub(axis, other);
        }
    }
    if random {
        a = sub(direction, scale(normal, dot(direction, normal)));
        b = cross(a, normal);
    }
    normalize(&mut a);
    normalize(&mut b);
    a = scale(a, diag);
    b = scale(b, diag);
    [
        add(a, center),
        add(b, center),
        add(scale(a, -1.0), center),
        add(scale(b, -1.0), center),
    ]
}
unsafe fn insert(
    s: &Hp2DecalServices,
    c: *mut c_void,
    surface: i32,
    texture: *mut c_void,
) -> DecalResult<i32> {
    unsafe {
        let count = out(|p| (s.decal_count)(c, surface, p))?;
        let mut at = count;
        for i in 0..count {
            if out(|p| (s.decal_texture)(c, surface, i, p))? == texture {
                at = i;
                break;
            }
        }
        status((s.insert)(c, surface, at))?;
        status((s.list_add)(c, surface))?;
        Ok(at)
    }
}
unsafe fn write_vertices(
    s: &Hp2DecalServices,
    c: *mut c_void,
    surface: i32,
    decal: i32,
    vertices: &[FVector; 4],
    base: FVector,
) -> DecalResult<()> {
    unsafe {
        for (j, v) in vertices.iter().enumerate() {
            status((s.set_vertex)(c, surface, decal, j as i32, &sub(*v, base)))?;
        }
        Ok(())
    }
}

/// # Safety
/// Same as `find_coplanar`; callbacks must not retain transient vector pointers.
pub unsafe fn attach(
    s: &Hp2DecalServices,
    c: *mut c_void,
    distance: f32,
    mut direction: FVector,
) -> DecalResult<*mut c_void> {
    unsafe {
        if out(|p| (s.enabled)(c, p))? == 0 {
            return Ok(ptr::null_mut());
        }
        let state = out(|p| (s.state)(c, p))?;
        if state.texture.is_null() {
            status((s.no_texture)(c))?;
            return Ok(ptr::null_mut());
        }
        let multi = state.multi.min(4);
        status((s.set_multi)(c, multi))?;
        let rot = hp2_core::math::FRotator {
            pitch: state.rotation.pitch,
            yaw: state.rotation.yaw,
            roll: state.rotation.roll,
        };
        let rotation = hp2_core::math::rotator_vector(rot, &*(s.trig)().cast::<[f32; 16384]>());
        let end = add(state.location, scale(scale(rotation, -1.0), distance));
        let random = direction.x == 0.0 && direction.y == 0.0 && direction.z == 0.0;
        if random {
            direction = out(|p| (s.random)(c, p))?;
        }
        let hit = out(|p| (s.line)(c, &end, &state.location, p))?;
        if hit == -1 {
            return Ok(ptr::null_mut());
        }
        let n = out(|p| (s.node)(c, hit, p))?;
        let surf = out(|p| (s.surface)(c, n.surface, p))?;
        let point = line_plane_intersection(state.location, end, surf.base, surf.normal);
        let main = find_coplanar(s, c, hit, point)?;
        if main == -1 {
            return Ok(ptr::null_mut());
        }
        let surf = out(|p| (s.surface)(c, main, p))?;
        let center = line_plane_intersection(state.location, end, surf.base, surf.normal);
        if dot(rotation, surf.normal) == 0.0
            || dot(sub(surf.base, center), surf.normal).abs() > 0.001
            || surf.flags & AUTO_PAN != 0
        {
            return Ok(ptr::null_mut());
        }
        let main_decal = insert(s, c, main, state.texture)?;
        let vertices = square_vertices(
            center,
            surf.base,
            surf.normal,
            direction,
            random,
            state.draw_scale,
            state.texture_u_size,
        );
        write_vertices(s, c, main, main_decal, &vertices, zero())?;
        clipped_nodes(s, c, main, main_decal, surf, &vertices)?;
        let trace = scale(div(surf.normal, size(surf.normal)), -50.0);
        let xv = sub(vertices[1], vertices[0]);
        let yv = sub(vertices[3], vertices[0]);
        for x in 0..multi {
            for y in 0..multi {
                let point = add(
                    add(vertices[0], scale(xv, (x + 1) as f32 / multi as f32)),
                    scale(yv, (y + 1) as f32 / multi as f32),
                );
                let start = sub(point, trace);
                let end = add(point, trace);
                let hit = out(|p| (s.line)(c, &end, &start, p))?;
                if hit == -1 {
                    continue;
                }
                let n = out(|p| (s.node)(c, hit, p))?;
                let sec = out(|p| (s.surface)(c, n.surface, p))?;
                let index = find_coplanar(
                    s,
                    c,
                    hit,
                    line_plane_intersection(start, end, sec.base, sec.normal),
                )?;
                if index == -1 {
                    continue;
                }
                let sec = out(|p| (s.surface)(c, index, p))?;
                let mut found = false;
                for i in 0..out(|p| (s.list_count)(c, p))? {
                    if out(|p| (s.list_item)(c, i, p))? == index {
                        found = true;
                        break;
                    }
                }
                if found || sec.flags & AUTO_PAN != 0 {
                    continue;
                }
                let cosine = dot(surf.normal, sec.normal) / (size(surf.normal) * size(sec.normal));
                if cosine.abs() <= 0.7 {
                    continue;
                }
                let decal = insert(s, c, index, state.texture)?;
                let mut secondary = [zero(); 4];
                for j in 0..4 {
                    let a = line_plane_intersection(
                        sub(vertices[j], surf.normal),
                        vertices[j],
                        sec.base,
                        sec.normal,
                    );
                    let b = line_plane_intersection(
                        sub(vertices[j], sec.normal),
                        vertices[j],
                        sec.base,
                        sec.normal,
                    );
                    let x = size(sub(vertices[j], b)) / cosine;
                    let h = size(sub(vertices[j], a)) / cosine;
                    let mut ab = sub(b, a);
                    normalize(&mut ab);
                    secondary[j] = sub(b, scale(ab, h - x));
                    status((s.set_vertex)(c, index, decal, j as i32, &secondary[j]))?;
                }
                clipped_nodes(s, c, index, decal, sec, &secondary)?;
                write_vertices(s, c, index, decal, &secondary, sec.base)?;
            }
        }
        write_vertices(s, c, main, main_decal, &vertices, surf.base)?;
        Ok(surf.texture)
    }
}

/// # Safety
/// Services must address this actor's live SurfList and model containers.
pub unsafe fn detach(s: &Hp2DecalServices, c: *mut c_void) -> DecalResult<()> {
    unsafe {
        loop {
            let count = out(|p| (s.list_count)(c, p))?;
            if count == 0 {
                return Ok(());
            }
            let surface = out(|p| (s.list_item)(c, count - 1, p))?;
            for i in 0..out(|p| (s.decal_count)(c, surface, p))? {
                if out(|p| (s.is_owner)(c, surface, i, p))? != 0 {
                    status((s.remove)(c, surface, i))?;
                    break;
                }
            }
            status((s.list_remove)(c, count - 1))?;
        }
    }
}

/*-----------------------------------------------------------------------------
    Publish-once storage for UnScript.cpp's `GHp2DecalServices` table.
-----------------------------------------------------------------------------*/

static mut DECAL_SERVICES: Option<Hp2DecalServices> = None;

/// # Safety
/// `value` is the real, immutable `GHp2DecalServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2DecalServices) {
    unsafe { (&raw mut DECAL_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Result<Hp2DecalServices, i32> {
    unsafe { (&raw const DECAL_SERVICES).read().ok_or(1) }
}
