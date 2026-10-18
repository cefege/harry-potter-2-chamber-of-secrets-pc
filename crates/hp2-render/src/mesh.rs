//! Port of the pure math in `Render/Src/UnMeshRnLOD.h` (`DrawLodMesh`) and
//! `Render/Src/UnMeshRn.cpp`: LOD vertex subset, morph setup and point lerp,
//! unlit colour, fatness, sort-depth keys, and triangle normals.
//!
//! Float fidelity: the reference build is clang with `-ffp-contract=on`.
//! Fusion happens only where the C++ writes a multiply and an add in one
//! expression. Operator calls (`FVector::operator*`, `operator+=`,
//! `FDistSquared`'s `Square()` calls) are separate functions, so they are not
//! fused. Each fused site is marked `// contract:`.

pub use hp2_abi::FVector;

/// `appRound` (`UnGnuG.h`, non-PSX): `static_cast<INT>(f)`. Rust `as`
/// saturates and maps NaN to 0, the same as arm64 `fcvtzs`.
pub fn app_round(f: f32) -> i32 {
    f as i32
}

/// `Min(A,B)` (`UnTemplate.h`): `(A<=B) ? A : B`.
fn min_f(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// `Max(A,B)` (`UnTemplate.h`): `(A>=B) ? A : B`.
fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `Clamp(X,Min,Max)` (`UnTemplate.h`): `X<Min ? Min : X<Max ? X : Max`.
fn clamp_f(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x < hi {
        x
    } else {
        hi
    }
}

/// `LODPix * VERTS_PER_PIX2` constants from `DrawLodMesh`.
const VERTS_PER_PIX2: f32 = 0.04;
const LOD_PER_FRAC: f32 = 4.0;

/// Inputs of the LOD vertex-subset block (`UnMeshRnLOD.h` lines 63-92).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodInputs {
    /// `Mesh->LODStrength`.
    pub lod_strength: f32,
    /// `Mesh->CollapsePointThus.Num()`.
    pub collapse_count: i32,
    /// `Mesh->ModelVerts`.
    pub model_verts: i32,
    /// `Mesh->LODMinVerts`.
    pub lod_min_verts: i32,
    /// `Owner->LODBias`.
    pub lod_bias: f32,
    /// `GlobalShapeLOD`.
    pub global_shape_lod: f32,
    /// `GlobalShapeLODAdjust`.
    pub global_shape_lod_adjust: f32,
    /// `Sprite->X1`, `Sprite->Y1`, `Sprite->X2`, `Sprite->Y2`.
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    /// `Frame->FX`.
    pub fx: f32,
    /// `ShapeLODMode`.
    pub shape_lod_mode: i32,
    /// `ShapeLODFix`.
    pub shape_lod_fix: f32,
}

/// Outputs of the LOD vertex-subset block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodSubset {
    /// `DoLOD` after the block.
    pub do_lod: bool,
    /// `TargetSubset`.
    pub target_subset: f32,
    /// `VertexSubset`.
    pub vertex_subset: i32,
}

/// `DrawLodMesh` lines 63-92: picks how many vertices to keep for a sprite of
/// the given on-screen size.
pub fn lod_subset(i: &LodInputs) -> LodSubset {
    let model_verts = i.model_verts;
    let mut target = model_verts as f32;
    let mut vertex = model_verts;
    let mut do_lod = i.lod_strength.to_bits() != 0 && i.collapse_count != 0;
    if do_lod {
        let bias = i.lod_bias / (i.lod_strength * i.global_shape_lod * i.global_shape_lod_adjust);
        let pix_i =
            i.y2.wrapping_add(1)
                .wrapping_sub(i.y1)
                .wrapping_mul(i.x2.wrapping_add(1).wrapping_sub(i.x1));
        let pix = pix_i as f32;
        let lod_pix = pix * VERTS_PER_PIX2 / (model_verts as f32);
        // appSqrt(DOUBLE) is sqrt() in double; the quotient is stored to FLOAT.
        let lod_frac = ((pix as f64).sqrt() / (i.fx as f64) * (LOD_PER_FRAC as f64)) as f32;
        let lod = bias * max_f(lod_pix, lod_frac);
        // contract: LODMinVerts + (ModelVerts - LODMinVerts) * LOD
        target =
            (model_verts.wrapping_sub(i.lod_min_verts) as f32).mul_add(lod, i.lod_min_verts as f32);
        if i.shape_lod_mode == 0 {
            target = if i.shape_lod_fix != 0.0 {
                i.shape_lod_fix * (model_verts as f32)
            } else {
                model_verts as f32
            };
        }
        vertex = min_i32(app_round(target), model_verts);
        if vertex >= model_verts {
            do_lod = false;
        }
    }
    LodSubset {
        do_lod,
        target_subset: target,
        vertex_subset: vertex,
    }
}

/// `Min(A,B)` for `INT`.
fn min_i32(a: i32, b: i32) -> i32 {
    if a <= b { a } else { b }
}

/// `appRound(TargetSubset*(1.f-LODMorph))` (`UnMeshRnLOD.h` line 127).
pub fn non_morph_subset(target: f32, lod_morph: f32) -> i32 {
    app_round(target * (1.0 - lod_morph))
}

/// `InvLODMorph = 1.f/(LODMorph*TargetSubset)` (`UnMeshRnLOD.h` line 130).
pub fn morph_inv(lod_morph: f32, target: f32) -> f32 {
    1.0 / (lod_morph * target)
}

/// `Alpha = Min(1.f, (FLOAT)(i-NonMorphSubset)*InvLODMorph)`
/// (`UnMeshRnLOD.h` line 139).
pub fn morph_alpha(i: i32, non_morph: i32, inv: f32) -> f32 {
    min_f(1.0, (i.wrapping_sub(non_morph) as f32) * inv)
}

/// `Samples[i].Point += (Samples[Morph2].Point - Samples[i].Point) * Alpha`
/// (`UnMeshRnLOD.h` line 146). The subtraction and the scale are operator
/// calls and `operator+=` is a separate function, so there is no contraction.
pub fn morph_point(p: FVector, target: FVector, alpha: f32) -> FVector {
    let d = FVector {
        x: target.x - p.x,
        y: target.y - p.y,
        z: target.z - p.z,
    };
    let s = FVector {
        x: d.x * alpha,
        y: d.y * alpha,
        z: d.z * alpha,
    };
    FVector {
        x: p.x + s.x,
        y: p.y + s.y,
        z: p.z + s.z,
    }
}

/// `Unlit = Clamp(ScaleGlow*0.5f + AmbientGlow/256.f, 0.f, 1.f)`
/// (`UnMeshRnLOD.h` line 366, `UnMeshRn.cpp` line 556).
pub fn unlit(scale_glow: f32, ambient_glow: u8) -> f32 {
    // contract: ScaleGlow*0.5f + AmbientGlow/256.f
    clamp_f(
        scale_glow.mul_add(0.5, ambient_glow as f32 / 256.0),
        0.0,
        1.0,
    )
}

/// `Fatness = (Owner->Fatness/16.0f)-8.0f` (`UnMeshRnLOD.h` line 538).
pub fn fatness_offset(fatness: u8) -> f32 {
    (fatness as f32) / 16.0 - 8.0
}

/// Sort depth key for the height heuristic:
/// `appRound(1000*(V0.Z+V1.Z+V2.Z+SortBias))` (`UnMeshRnLOD.h` NotWeapon
/// branch).
pub fn depth_key_height(z0: f32, z1: f32, z2: f32, sort_bias: f32) -> i32 {
    app_round(1000.0 * (z0 + z1 + z2 + sort_bias))
}

/// `FDistSquared(V1,V2)` (`UnMath.h`): `Square(V2.X-V1.X) + ...`. `Square` is
/// a separate inline function, so the products are not fused.
pub fn fdist_squared(v1: FVector, v2: FVector) -> f32 {
    let dx = v2.x - v1.x;
    let dy = v2.y - v1.y;
    let dz = v2.z - v1.z;
    dx * dx + dy * dy + dz * dz
}

/// Sort depth key for the distance heuristic:
/// `appRound(FDistSquared(V0,Hack)*FDistSquared(V1,Hack)*FDistSquared(V2,Hack))`
/// (`UnMeshRnLOD.h`, `Hack = FVector(0,-8,0)`).
pub fn depth_key_distance(p0: FVector, p1: FVector, p2: FVector, hack: FVector) -> i32 {
    app_round(fdist_squared(p0, hack) * fdist_squared(p1, hack) * fdist_squared(p2, hack))
}

/// `FVector::SizeSquared`: `X*X + Y*Y + Z*Z` (`UnMath.h`), one expression, so
/// the left-hand products are fused.
pub fn size_squared(v: FVector) -> f32 {
    // contract: X*X + Y*Y + Z*Z
    v.z.mul_add(v.z, v.x.mul_add(v.x, v.y * v.y))
}

/// `FVector::operator^` (`UnMath.h`): `Y*V.Z - Z*V.Y`, `Z*V.X - X*V.Z`,
/// `X*V.Y - Y*V.X`, one return statement, so each left product is fused.
pub fn cross(a: FVector, b: FVector) -> FVector {
    FVector {
        // contract: a.y*b.z - a.z*b.y
        x: a.y.mul_add(b.z, -(a.z * b.y)),
        // contract: a.z*b.x - a.x*b.z
        y: a.z.mul_add(b.x, -(a.x * b.z)),
        // contract: a.x*b.y - a.y*b.x
        z: a.x.mul_add(b.y, -(a.y * b.x)),
    }
}

/// Triangle normal (`UnMeshRn.cpp` lines 585-586):
/// `TriNormals[i] = (V1-V2) ^ (V3-V1)`, then
/// `*= DivSqrtApprox(SizeSquared()+0.001f)`.
pub fn tri_normal(v1: FVector, v2: FVector, v3: FVector) -> FVector {
    let a = FVector {
        x: v1.x - v2.x,
        y: v1.y - v2.y,
        z: v1.z - v2.z,
    };
    let b = FVector {
        x: v3.x - v1.x,
        y: v3.y - v1.y,
        z: v3.z - v1.z,
    };
    let n = cross(a, b);
    let s = div_sqrt_approx(size_squared(n) + 0.001);
    FVector {
        x: n.x * s,
        y: n.y * s,
        z: n.z * s,
    }
}

/// The bare `DivSqrtApprox` that `UnMeshRn.cpp` and `UnMeshRnLOD.h` call.
/// `Render.h` selects the table version only under `#if ASM && defined(_M_IX86)`.
/// On this target the `#else` branch is `1.0/appSqrt(F)`, where `appSqrt`
/// takes a `DOUBLE` and the quotient is narrowed to `FLOAT` on return.
pub fn div_sqrt_approx(f: f32) -> f32 {
    (1.0 / f64::from(f).sqrt()) as f32
}
