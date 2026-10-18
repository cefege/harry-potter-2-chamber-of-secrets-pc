//! Mesh LOD, morph, shading and sort-key exports (`hp2-render::mesh`). The
//! C++ side (`UnMeshRnLOD.h`, `UnMeshRn.cpp`) keeps the FTransTexture loops
//! and the FSceneNode/RenDev plumbing and calls these per-element helpers.
//! `FVector` crosses as 3 contiguous floats.

use hp2_ffi_support::guard;
use hp2_render::mesh::{
    FVector, LodInputs, depth_key_distance, depth_key_height, fatness_offset, lod_subset,
    morph_alpha, morph_inv, morph_point, non_morph_subset, tri_normal, unlit,
};

/// `LODStrength`, `CollapseCount`, `ModelVerts`, `LODMinVerts`, `LODBias`,
/// `GlobalShapeLOD`, `GlobalShapeLODAdjust`, the viewport rect, `FX`,
/// `ShapeLODMode`, `ShapeLODFix`, in the order `DrawLodMesh` reads them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2RsLodInputs {
    pub lod_strength: f32,
    pub collapse_count: i32,
    pub model_verts: i32,
    pub lod_min_verts: i32,
    pub lod_bias: f32,
    pub global_shape_lod: f32,
    pub global_shape_lod_adjust: f32,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub fx: f32,
    pub shape_lod_mode: i32,
    pub shape_lod_fix: f32,
}

/// `DoLOD`, `TargetSubset`, `VertexSubset` for `DrawLodMesh`. Returns
/// `DoLOD` as `0`/`1`; writes `out_target` and `out_vertex`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_mesh_lod_subset(
    inp: *const Hp2RsLodInputs,
    out_target: *mut f32,
    out_vertex: *mut i32,
) -> i32 {
    guard("hp2_rs_render_mesh_lod_subset", 0, move || unsafe {
        if inp.is_null() || out_target.is_null() || out_vertex.is_null() {
            return 0;
        }
        let i = *inp;
        let r = lod_subset(&LodInputs {
            lod_strength: i.lod_strength,
            collapse_count: i.collapse_count,
            model_verts: i.model_verts,
            lod_min_verts: i.lod_min_verts,
            lod_bias: i.lod_bias,
            global_shape_lod: i.global_shape_lod,
            global_shape_lod_adjust: i.global_shape_lod_adjust,
            x1: i.x1,
            y1: i.y1,
            x2: i.x2,
            y2: i.y2,
            fx: i.fx,
            shape_lod_mode: i.shape_lod_mode,
            shape_lod_fix: i.shape_lod_fix,
        });
        *out_target = r.target_subset;
        *out_vertex = r.vertex_subset;
        r.do_lod as i32
    })
}

/// `appRound(TargetSubset*(1.f-LODMorph))`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_render_mesh_non_morph_subset(target: f32, lod_morph: f32) -> i32 {
    guard("hp2_rs_render_mesh_non_morph_subset", 0, move || {
        non_morph_subset(target, lod_morph)
    })
}

/// `InvLODMorph = 1.f/(LODMorph*TargetSubset)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_render_mesh_morph_inv(lod_morph: f32, target: f32) -> f32 {
    guard("hp2_rs_render_mesh_morph_inv", 0.0, move || {
        morph_inv(lod_morph, target)
    })
}

/// `Alpha = Min(1.f, (FLOAT)(i-NonMorphSubset)*InvLODMorph)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_render_mesh_morph_alpha(i: i32, non_morph: i32, inv: f32) -> f32 {
    guard("hp2_rs_render_mesh_morph_alpha", 0.0, move || {
        morph_alpha(i, non_morph, inv)
    })
}

/// `Point += (Target - Point) * Alpha` on the 3 floats at `p`, in place.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_mesh_morph_point(
    p: *mut f32,
    target: *const f32,
    alpha: f32,
) -> i32 {
    guard("hp2_rs_render_mesh_morph_point", 0, move || unsafe {
        if p.is_null() || target.is_null() {
            return 0;
        }
        let pv = FVector {
            x: *p,
            y: *p.add(1),
            z: *p.add(2),
        };
        let tv = FVector {
            x: *target,
            y: *target.add(1),
            z: *target.add(2),
        };
        let r = morph_point(pv, tv, alpha);
        *p = r.x;
        *p.add(1) = r.y;
        *p.add(2) = r.z;
        1
    })
}

/// `Clamp(ScaleGlow*0.5f + AmbientGlow/256.f, 0.f, 1.f)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_render_mesh_unlit(scale_glow: f32, ambient_glow: u8) -> f32 {
    guard("hp2_rs_render_mesh_unlit", 0.0, move || {
        unlit(scale_glow, ambient_glow)
    })
}

/// `Fatness = (Owner->Fatness/16.0f)-8.0f`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_render_mesh_fatness_offset(fatness: u8) -> f32 {
    guard("hp2_rs_render_mesh_fatness_offset", 0.0, move || {
        fatness_offset(fatness)
    })
}

/// `appRound(1000*(V0.Z+V1.Z+V2.Z+SortBias))`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_render_mesh_depth_key_height(
    z0: f32,
    z1: f32,
    z2: f32,
    sort_bias: f32,
) -> i32 {
    guard("hp2_rs_render_mesh_depth_key_height", 0, move || {
        depth_key_height(z0, z1, z2, sort_bias)
    })
}

/// `appRound(FDistSquared(V0,Hack)*FDistSquared(V1,Hack)*FDistSquared(V2,Hack))`.
/// `p0`, `p1`, `p2`, `hack` each point at 3 floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_mesh_depth_key_distance(
    p0: *const f32,
    p1: *const f32,
    p2: *const f32,
    hack: *const f32,
) -> i32 {
    guard("hp2_rs_render_mesh_depth_key_distance", 0, move || unsafe {
        if p0.is_null() || p1.is_null() || p2.is_null() || hack.is_null() {
            return 0;
        }
        depth_key_distance(read_vec(p0), read_vec(p1), read_vec(p2), read_vec(hack))
    })
}

/// `TriNormals[i] = (V1-V2) ^ (V3-V1)` scaled by `DivSqrtApprox(SizeSquared()+0.001f)`,
/// where `DivSqrtApprox` is the bare `Render.h` function (`hp2_render::mesh::div_sqrt_approx`).
/// Writes the normal to `out` (3 floats).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_render_mesh_tri_normal(
    v1: *const f32,
    v2: *const f32,
    v3: *const f32,
    out: *mut f32,
) -> i32 {
    guard("hp2_rs_render_mesh_tri_normal", 0, move || unsafe {
        if v1.is_null() || v2.is_null() || v3.is_null() || out.is_null() {
            return 0;
        }
        let n = tri_normal(read_vec(v1), read_vec(v2), read_vec(v3));
        *out = n.x;
        *out.add(1) = n.y;
        *out.add(2) = n.z;
        1
    })
}

/// Reads 3 contiguous floats.
unsafe fn read_vec(p: *const f32) -> FVector {
    unsafe {
        FVector {
            x: *p,
            y: *p.add(1),
            z: *p.add(2),
        }
    }
}
