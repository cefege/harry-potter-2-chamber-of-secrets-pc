//! Sprite setup math reused by the Rust-owned dynamic renderer.

/// Top-level `URender::SetupDynamics` admission. An empty BSP or hidden actor
/// display prevents all subsequent actor, iterator, shadow, and light work.
pub const fn dynamics_enabled(model_node_count: i32, show_actors: bool) -> bool {
    model_node_count != 0 && show_actors
}

/// First per-actor `SetupDynamics` filter. Later visibility, ownership, and
/// render-iterator filters run only after this admission succeeds.
pub const fn dynamics_outer_admitted(
    actor_present: bool,
    actor_high_detail: bool,
    high_detail_actors: bool,
    recursion: i32,
    behind_view: bool,
    actor_is_view_target: bool,
) -> bool {
    actor_present
        && (!actor_high_detail || high_detail_actors)
        && (recursion != 0 || behind_view || !actor_is_view_target)
}

/// Evaluates remaining `SetupDynamics` admission leaves in native
/// short-circuit order. A failed C++ leaf terminates evaluation.
pub fn dynamics_inner_admitted(mut query: impl FnMut(u32) -> Result<bool, ()>) -> Result<bool, ()> {
    if query(0)? || query(1)? || !query(2)? || !query(3)? || !query(4)? {
        return Ok(false);
    }
    if query(5)? && (!query(6)? || query(7)?) {
        return Ok(false);
    }
    if !query(6)? || !query(8)? {
        return Ok(true);
    }
    Ok(query(6)? && query(7)?)
}

/// Runs the native render-iterator cursor protocol. The iterator and draw
/// actions remain checked C++ leaves; this owns only their traversal order.
pub fn dynamics_iterate(mut query: impl FnMut(u32, &mut bool) -> Result<(), ()>) -> Result<(), ()> {
    let mut done = false;
    query(0, &mut done)?;
    loop {
        query(1, &mut done)?;
        if done {
            return Ok(());
        }
        query(2, &mut done)?;
        query(3, &mut done)?;
    }
}

/// Evaluates dynamic-light eligibility and four-plane admission. Query 4
/// computes the native maximum radius; queries 5–8 consume it. Queries 10–13
/// deliberately re-evaluate the native volumetric radius per plane.
pub fn dynamics_light_admitted(
    mut query: impl FnMut(u32) -> Result<bool, ()>,
) -> Result<Option<bool>, ()> {
    if !query(0)? || !query(1)? || !query(2)? || !query(3)? {
        return Ok(None);
    }
    query(4)?;
    for op in 5..9 {
        if !query(op)? {
            return Ok(None);
        }
    }
    let mut volumetric = query(9)?;
    for op in 10..14 {
        if volumetric && !query(op)? {
            volumetric = false;
        }
    }
    Ok(Some(volumetric))
}

/// Native shadow-update outcome. The status preserves the exact trace labels
/// emitted by `URender::SetupDynamics`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DynamicShadowAdmission {
    MissingShadow = 0,
    RejectedOrtho = 1,
    RejectedBehindProjection = 2,
    RejectedBounds = 3,
    Admitted = 4,
}

/// Evaluates dynamic-shadow admission in native short-circuit order. The C++
/// leaves retain SDK geometry and `BoundVisible`; Rust owns branch selection.
pub fn dynamics_shadow_admission(
    mut query: impl FnMut(u32) -> Result<bool, ()>,
) -> Result<DynamicShadowAdmission, ()> {
    if !query(0)? {
        return Ok(DynamicShadowAdmission::MissingShadow);
    }
    if !query(1)? {
        return Ok(DynamicShadowAdmission::RejectedOrtho);
    }
    if query(2)? {
        return Ok(DynamicShadowAdmission::RejectedBehindProjection);
    }
    if !query(3)? {
        return Ok(DynamicShadowAdmission::RejectedBounds);
    }
    Ok(DynamicShadowAdmission::Admitted)
}

/// Requests exactly one native shadow trace/update leaf for the Rust-selected
/// admission outcome. An unknown status is rejected before native work.
pub fn dynamics_shadow_dispatch(
    outcome: i32,
    mut dispatch: impl FnMut(DynamicShadowAdmission) -> Result<(), ()>,
) -> Result<(), ()> {
    let outcome = match outcome {
        0 => DynamicShadowAdmission::MissingShadow,
        1 => DynamicShadowAdmission::RejectedOrtho,
        2 => DynamicShadowAdmission::RejectedBehindProjection,
        3 => DynamicShadowAdmission::RejectedBounds,
        4 => DynamicShadowAdmission::Admitted,
        _ => return Err(()),
    };
    dispatch(outcome)
}

/// Native `FBox` coordinates and behind-projection decision for a dynamic
/// shadow. C++ retains `FBox` construction and `BoundVisible`; Rust owns the
/// arithmetic which produces this value.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct DynamicShadowProjection {
    pub min: FVector,
    pub max: FVector,
    pub behind_projection: i32,
}

/// Computes the dynamic-shadow projection box and its camera-space rejection.
/// The operation order mirrors `URender::SetupDynamics`.
pub fn dynamics_shadow_projection(
    location: FVector,
    collision_radius: f32,
    origin: FVector,
    z_axis: FVector,
) -> DynamicShadowProjection {
    const SHADOW_HEIGHT: f32 = 8192.0;
    let min = FVector {
        x: location.x - collision_radius,
        y: location.y - collision_radius,
        z: location.z - SHADOW_HEIGHT,
    };
    let max = FVector {
        x: location.x + collision_radius,
        y: location.y + collision_radius,
        z: location.z,
    };
    let center = FVector {
        x: (min.x + max.x) * 0.5,
        y: (min.y + max.y) * 0.5,
        z: (min.z + max.z) * 0.5,
    };
    let size = FVector {
        x: (max.x - min.x) * 0.5,
        y: (max.y - min.y) * 0.5,
        z: (max.z - min.z) * 0.5,
    };
    let extent = (z_axis.x * size.x).abs() + (z_axis.y * size.y).abs() + (z_axis.z * size.z).abs();
    let z = (center.x - origin.x) * z_axis.x
        + (center.y - origin.y) * z_axis.y
        + (center.z - origin.z) * z_axis.z;
    DynamicShadowProjection {
        min,
        max,
        behind_projection: (z < -extent) as i32,
    }
}

/// Chooses whether an admitted actor follows ordinary sprite processing or
/// the moving-brush tracker route. The native `IsMovingBrush` query and both
/// typed route implementations remain C++ leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DynamicSpriteRoute {
    Ordinary = 0,
    MovingBrush = 1,
}

pub const fn dynamics_sprite_route(is_moving_brush: bool) -> DynamicSpriteRoute {
    if is_moving_brush {
        DynamicSpriteRoute::MovingBrush
    } else {
        DynamicSpriteRoute::Ordinary
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DynamicSpriteDrawRoute {
    Ordinary = 0,
    Iterator = 1,
    FilteredIterator = 2,
}

pub const fn dynamics_sprite_draw_route(
    render_interface_present: bool,
    filter_by_volume: bool,
) -> DynamicSpriteDrawRoute {
    if !render_interface_present {
        DynamicSpriteDrawRoute::Ordinary
    } else if filter_by_volume {
        DynamicSpriteDrawRoute::FilteredIterator
    } else {
        DynamicSpriteDrawRoute::Iterator
    }
}

/// Action for synchronizing an actor's optional `RenderInterface` with its
/// optional `RenderIteratorClass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DynamicIteratorLifecycle {
    Construct = 0,
    Destroy = 1,
    Retain = 2,
}

/// Chooses the native render-iterator lifecycle action. Construction,
/// destruction, and validity probing remain typed C++ leaves.
pub const fn dynamics_iterator_lifecycle(
    object_class_present: bool,
    interface_present: bool,
    interface_valid: bool,
) -> DynamicIteratorLifecycle {
    if object_class_present && (!interface_present || !interface_valid) {
        DynamicIteratorLifecycle::Construct
    } else if !object_class_present && interface_present {
        DynamicIteratorLifecycle::Destroy
    } else {
        DynamicIteratorLifecycle::Retain
    }
}

#[cfg(test)]
mod dynamics_tests {
    use super::{
        DynamicIteratorLifecycle, DynamicShadowAdmission, DynamicSpriteDrawRoute,
        DynamicSpriteRoute, FVector, dynamics_enabled, dynamics_inner_admitted, dynamics_iterate,
        dynamics_iterator_lifecycle, dynamics_light_admitted, dynamics_outer_admitted,
        dynamics_shadow_admission, dynamics_shadow_dispatch, dynamics_shadow_projection,
        dynamics_sprite_draw_route, dynamics_sprite_route,
    };
    #[test]
    fn dynamics_requires_nodes_and_actor_visibility() {
        assert!(!dynamics_enabled(0, true));
        assert!(!dynamics_enabled(1, false));
        assert!(dynamics_enabled(1, true));
    }

    #[test]
    fn dynamics_outer_filter_preserves_high_detail_and_view_target_rules() {
        assert!(!dynamics_outer_admitted(
            false, false, true, 0, false, false
        ));
        assert!(!dynamics_outer_admitted(true, true, false, 0, false, false));
        assert!(!dynamics_outer_admitted(true, false, true, 0, false, true));
        assert!(dynamics_outer_admitted(true, false, true, 1, false, true));
        assert!(dynamics_outer_admitted(true, false, true, 0, true, true));
    }

    #[test]
    fn dynamics_inner_filter_stops_before_visibility_for_excluded_actor() {
        let mut calls = Vec::new();
        assert!(
            !dynamics_inner_admitted(|op| {
                calls.push(op);
                Ok(op == 0)
            })
            .unwrap()
        );
        assert_eq!(calls, [0]);
    }

    #[test]
    fn dynamics_inner_filter_preserves_repeated_owner_query_order() {
        let mut calls = Vec::new();
        assert!(
            dynamics_inner_admitted(|op| {
                calls.push(op);
                Ok(matches!(op, 2 | 3 | 4 | 6 | 8 | 7))
            })
            .unwrap()
        );
        assert_eq!(calls, [0, 1, 2, 3, 4, 5, 6, 8, 6, 7]);
    }

    #[test]
    fn dynamics_inner_filter_stops_after_failed_native_leaf() {
        let mut calls = Vec::new();
        assert_eq!(
            dynamics_inner_admitted(|op| {
                calls.push(op);
                if op == 2 { Err(()) } else { Ok(false) }
            }),
            Err(())
        );
        assert_eq!(calls, [0, 1, 2]);
    }

    #[test]
    fn dynamics_iterator_traversal_preserves_first_done_visit_next_order() {
        let mut calls = Vec::new();
        let mut done_checks = 0;
        dynamics_iterate(|op, done| {
            calls.push(op);
            if op == 1 {
                *done = done_checks == 2;
                done_checks += 1;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, [0, 1, 2, 3, 1, 2, 3, 1]);
    }

    #[test]
    fn dynamics_light_filter_preserves_plane_and_volumetric_query_order() {
        let mut calls = Vec::new();
        assert_eq!(
            dynamics_light_admitted(|op| {
                calls.push(op);
                Ok(true)
            }),
            Ok(Some(true))
        );
        assert_eq!(calls, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    }

    #[test]
    fn dynamics_light_filter_stops_volumetric_planes_after_rejection() {
        let mut calls = Vec::new();
        assert_eq!(
            dynamics_light_admitted(|op| {
                calls.push(op);
                Ok(op != 11)
            }),
            Ok(Some(false))
        );
        assert_eq!(calls, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    }

    #[test]
    fn dynamics_shadow_filter_stops_after_missing_shadow() {
        let mut calls = Vec::new();
        assert_eq!(
            dynamics_shadow_admission(|op| {
                calls.push(op);
                Ok(false)
            }),
            Ok(DynamicShadowAdmission::MissingShadow)
        );
        assert_eq!(calls, [0]);
    }

    #[test]
    fn dynamics_shadow_filter_preserves_projection_then_bounds_order() {
        let mut calls = Vec::new();
        assert_eq!(
            dynamics_shadow_admission(|op| {
                calls.push(op);
                Ok(op < 2)
            }),
            Ok(DynamicShadowAdmission::RejectedBounds)
        );
        assert_eq!(calls, [0, 1, 2, 3]);
    }

    #[test]
    fn dynamics_shadow_dispatches_exactly_the_rust_selected_outcome() {
        let mut calls = Vec::new();
        for outcome in 0..=4 {
            dynamics_shadow_dispatch(outcome, |action| {
                calls.push(action as i32);
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(calls, [0, 1, 2, 3, 4]);

        let mut called = false;
        assert_eq!(
            dynamics_shadow_dispatch(5, |_| {
                called = true;
                Ok(())
            }),
            Err(())
        );
        assert!(!called);
    }

    #[test]
    fn dynamics_shadow_projection_preserves_box_and_camera_rejection_math() {
        let projection = dynamics_shadow_projection(
            FVector {
                x: 100.0,
                y: -20.0,
                z: 9000.0,
            },
            50.0,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            FVector {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        );
        assert_eq!(
            projection.min,
            FVector {
                x: 50.0,
                y: -70.0,
                z: 808.0
            }
        );
        assert_eq!(
            projection.max,
            FVector {
                x: 150.0,
                y: 30.0,
                z: 9000.0
            }
        );
        assert_eq!(projection.behind_projection, 0);

        let behind = dynamics_shadow_projection(
            FVector {
                x: 0.0,
                y: 0.0,
                z: -9000.0,
            },
            10.0,
            FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            FVector {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        );
        assert_eq!(behind.behind_projection, 1);
    }

    #[test]
    fn dynamics_sprite_route_preserves_moving_brush_split() {
        assert_eq!(dynamics_sprite_route(false), DynamicSpriteRoute::Ordinary);
        assert_eq!(dynamics_sprite_route(true), DynamicSpriteRoute::MovingBrush);
    }

    #[test]
    fn dynamics_sprite_draw_route_preserves_default_iterator_and_volume_split() {
        assert_eq!(
            dynamics_sprite_draw_route(false, true),
            DynamicSpriteDrawRoute::Ordinary
        );
        assert_eq!(
            dynamics_sprite_draw_route(true, false),
            DynamicSpriteDrawRoute::Iterator
        );
        assert_eq!(
            dynamics_sprite_draw_route(true, true),
            DynamicSpriteDrawRoute::FilteredIterator
        );
    }

    #[test]
    fn dynamics_iterator_lifecycle_preserves_construct_destroy_and_retain_cases() {
        assert_eq!(
            dynamics_iterator_lifecycle(true, false, false),
            DynamicIteratorLifecycle::Construct
        );
        assert_eq!(
            dynamics_iterator_lifecycle(true, true, false),
            DynamicIteratorLifecycle::Construct
        );
        assert_eq!(
            dynamics_iterator_lifecycle(false, true, true),
            DynamicIteratorLifecycle::Destroy
        );
        assert_eq!(
            dynamics_iterator_lifecycle(true, true, true),
            DynamicIteratorLifecycle::Retain
        );
        assert_eq!(
            dynamics_iterator_lifecycle(false, false, false),
            DynamicIteratorLifecycle::Retain
        );
    }
}
// SDK gathering, Project and BoundVisible are native leaves; `dynamic_items`
// owns setup branch selection and node publication.

pub use hp2_abi::FVector;

/// Result of one axis: `[lo, hi]` after clamping to the frame, and whether the
/// sprite survives. Both fields are always written, matching the C++ members
/// `X1/X2` (`Y1/Y2`), which are assigned even when the sprite is rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct SpriteExtent {
    pub lo: i32,
    pub hi: i32,
    pub visible: bool,
}

/// `appRound(appCeil(v))`. `appCeil` is `(INT)ceil(v)`; `appRound` on this
/// build is `static_cast<INT>(f)` (UnGnuG.h). The INT is converted to FLOAT
/// before `appRound`, as the C++ call does.
#[inline]
fn round_ceil(v: f32) -> i32 {
    let c = v.ceil() as i32;
    (c as f32) as i32
}

/// X extent. `size` is `Persp * DrawScale * Texture->USize`, computed here in
/// the same order. There is no multiply-add here, so no fma.
pub fn extent_x(
    screen_x: f32,
    persp: f32,
    draw_scale: f32,
    tex_usize: i32,
    frame_x: i32,
) -> SpriteExtent {
    let x_size = persp * draw_scale * (tex_usize as f32);
    let mut x1 = round_ceil(screen_x - x_size / 2.0);
    let mut x2 = round_ceil(screen_x + x_size / 2.0);
    if x1 > x2 {
        std::mem::swap(&mut x1, &mut x2);
    }
    if x1 < 0 {
        x1 = 0;
        if x2 < 0 {
            x2 = 0;
        }
    }
    if x2 > frame_x {
        x2 = frame_x;
        if x1 > frame_x {
            x1 = frame_x;
        }
    }
    let visible = !(x2 <= 0 || x1 >= frame_x - 1);
    SpriteExtent {
        lo: x1,
        hi: x2,
        visible,
    }
}

/// Y extent, same as [`extent_x`] with the vertical rejection test.
pub fn extent_y(
    screen_y: f32,
    persp: f32,
    draw_scale: f32,
    tex_vsize: i32,
    frame_y: i32,
) -> SpriteExtent {
    let y_size = persp * draw_scale * (tex_vsize as f32);
    let mut y1 = round_ceil(screen_y - y_size / 2.0);
    let mut y2 = round_ceil(screen_y + y_size / 2.0);
    if y1 > y2 {
        std::mem::swap(&mut y1, &mut y2);
    }
    if y1 < 0 {
        y1 = 0;
        if y2 < 0 {
            y2 = 0;
        }
    }
    if y2 > frame_y {
        y2 = frame_y;
        if y1 > frame_y {
            y1 = frame_y;
        }
    }
    let visible = !(y2 <= 0 || y1 >= frame_y || y1 >= y2);
    SpriteExtent {
        lo: y1,
        hi: y2,
        visible,
    }
}

/// `a*b + c`, fused with `f32::mul_add` on aarch64 (the reference build uses
/// `-ffp-contract=on` there), plain `a*b + c` elsewhere. Same as particle.rs.
#[inline]
fn madd(a: f32, b: f32, c: f32) -> f32 {
    #[cfg(target_arch = "aarch64")]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        a * b + c
    }
}

/// `FVector::operator|`: `X*V.X + Y*V.Y + Z*V.Z`, contracted as
/// `fma(Z,V.Z, fma(X,V.X, Y*V.Y))` (same shape as `softln::dot`).
#[inline]
fn dot(a: FVector, b: FVector) -> f32 {
    madd(a.z, b.z, madd(a.x, b.x, a.y * b.y))
}

/// `FVector::SizeSquared`: `X*X + Y*Y + Z*Z` with the same contraction as [`dot`].
pub fn size_squared(v: FVector) -> f32 {
    dot(v, v)
}

/// Which branch of `FDynamicSprite::Setup` runs (the DrawType test).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SetupPath {
    /// Neither branch matches: `return 0`.
    Reject = 0,
    /// Sprite branch: projection plane, then X and Y extents.
    Sprite = 1,
    /// Mesh/particle branch: bounding box to screen bounds.
    Box = 2,
}

/// Branch selection of `FDynamicSprite::Setup`. The C++ supplies the
/// booleans it reads from the actor (DrawType tests, ShowFlags, viewport
/// ortho/realtime). `is_sprite_type` is `DT_Sprite || DT_SpriteAnimOnce`.
/// The C++ `Texture` and `DrawScale` fallbacks stay in C++ (engine pointers).
pub fn setup_path(
    is_sprite_type: bool,
    is_mesh: bool,
    is_particles: bool,
    show_icons: bool,
    ortho: bool,
    realtime: bool,
) -> SetupPath {
    if is_sprite_type || show_icons || (is_particles && (ortho || !realtime)) {
        SetupPath::Sprite
    } else if is_mesh || is_particles {
        SetupPath::Box
    } else {
        SetupPath::Reject
    }
}

/// Projection plane of the sprite branch: `MinZ = MaxZ = ((Loc - Origin) | ZAxis) - ProjForward`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct ProjectionPlane {
    /// `MinZ` (also `MaxZ`), always written.
    pub min_z: f32,
    /// True when `MinZ < -2*ProjForward && !ortho`: `return 0`.
    pub reject: bool,
}

/// Port of the projection-plane block of `FDynamicSprite::Setup`.
pub fn projection_plane(
    location: FVector,
    origin: FVector,
    z_axis: FVector,
    proj_forward: f32,
    ortho: bool,
) -> ProjectionPlane {
    let d = FVector {
        x: location.x - origin.x,
        y: location.y - origin.y,
        z: location.z - origin.z,
    };
    let min_z = dot(d, z_axis) - proj_forward;
    let reject = min_z < -2.0 * proj_forward && !ortho;
    ProjectionPlane { min_z, reject }
}

/// Result of the mesh/particle box branch of `FDynamicSprite::Setup`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct BoxExtent {
    pub x1: i32,
    pub x2: i32,
    pub y1: i32,
    pub y2: i32,
    pub min_z: f32,
    pub max_z: f32,
    /// True when `Y1 >= Y2`: `return 0`.
    pub reject: bool,
}

/// Port of the screen-bounds block of the box branch. `appRound` is
/// `static_cast<INT>` (truncation), per UnGnuG.h.
pub fn box_extent(
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    min_z: f32,
    max_z: f32,
) -> BoxExtent {
    let x1 = min_x as i32;
    let x2 = max_x as i32;
    let y1 = min_y as i32;
    let y2 = max_y as i32;
    BoxExtent {
        x1,
        x2,
        y1,
        y2,
        min_z,
        max_z,
        reject: y1 >= y2,
    }
}

/// `BoxCoords.XAxis.SizeSquared() == 0.f`: the box branch rejects.
pub fn box_axis_is_empty(x_axis: FVector) -> bool {
    size_squared(x_axis) == 0.0
}
