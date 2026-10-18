//! Port of the screen-extent and setup math of `FDynamicSprite::Setup`
//! (`Render/Src/UnSprite.cpp`). `Setup` itself stays in C++: it reads engine
//! objects (Viewport, Actor, GRender->Project, GetRenderBoundingBox,
//! BoundVisible). These functions take the values it reads and return the
//! values it assigns.

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
