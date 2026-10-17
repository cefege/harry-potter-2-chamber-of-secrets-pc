// gouraud.wgsl: WGSL port of ThirdParty/XOpenGLDrv/Src/DrawGouraud_GLSL.cpp's
// `DrawGouraudProgram` (vertex shader read in full, geometry shader read in
// full, fragment shader read through its main diffuse/light/fog/detail
// blend path and its OPT_Editor-gated tail -- `OPT_MacroTextures` and
// `OPT_BumpMaps`'s fragment-side specular loop not yet read in full).
// Covers UE1's `DrawGouraudPolygon`/`DrawTriangles` (both funnel through
// `PrepareGouraudCall`/`FinishGouraudCall` onto the same
// `DrawGouraudProgram`, confirmed by reading `DrawGouraud.cpp` in full
// per this crate's own module doc).
//
// Geometry shader: genuinely optional, not a mandatory part of this
// program the way an earlier version of this file's own header claimed
// (caught on review, corrected after confirming `DrawGouraud.cpp:376`'s
// real `GeoShaderFunc = RenDev->UsingGeometryShaders ?
// &BuildGeometryShader : nullptr; // optional`). When it does run:
// `layout(triangles) in; layout(triangle_strip, max_vertices = 3) out;`,
// with a `for (i in 0..3) { ...; EmitVertex(); }` body -- confirmed, by
// reading the full body, to never amplify topology: exactly one triangle
// out per triangle in. Its real purpose is `OPT_BumpMaps`-only: computing
// one tangent/bitangent *per triangle* (`GetTangent`/`GetBitangent`,
// needing all three vertices' positions and UVs at once, which a
// per-vertex-only vertex shader cannot do) and deferring the final
// `modelviewprojMat` transform to this stage for that reason. This file
// transcribes the already-in-production, already-complete `#else`
// (`!OPT_GeometryShaders`) branch of the same `DrawGouraudProgram`'s own
// vertex shader (`DrawGouraud_GLSL.cpp` lines 100-118) instead, which
// does the final MVP transform directly, no geometry stage involved, and
// is exactly what every non-bump-mapped draw already uses today
// regardless of whether geometry shaders happen to be enabled -- not a
// freshly-designed replacement, a direct transcription of an existing
// code path.
//
// Real per-vertex Gouraud shading, not texture-sampled lighting (the
// architectural distinction this crate's module doc already flagged from
// reading `DrawGouraud.cpp`'s `BufferVert`): `LightColor`/`FogColor` are
// real vertex inputs, interpolated by the rasterizer like any other
// varying, not read from a lightmap/fogmap texture in the fragment shader
// the way `DrawComplex` does.
//
// `LightColorIntensity` (`Out.LightColor = LightColor *
// LightColorIntensity` in the real vertex shader) is a real frame-global
// value, not a per-draw one -- confirmed in `XOpenGL.cpp`:
// `FrameState->LightColorIntensity = ActorXBlending ? 1.f : 1.5f;` -- so
// it lives in `FrameUniforms` here, alongside `modelviewproj_mat`, the
// same uniform a real Rust caller would need to update once per frame
// from the engine's `ActorXBlending` config, not per draw call.
//
// Ported from the non-bindless fallback, same as every other shader in
// this crate: `GetTexel`'s non-bindless branch (`common.wgsl`'s
// `get_texel`) needs only the bound texture/sampler, never a handle.
//
// The real GLSL fragment shader's diffuse/light/fog blend is a tree of
// four mutually-exclusive compile-time `#if OPT_IsRenderFog`/`#if
// OPT_IsModulated` branches (`DrawGouraud_GLSL.cpp` lines 290-351, read
// in full and ported in full here, not just the one branch an earlier,
// incomplete version of this file ported -- caught on review: "any actor
// in a fog zone and every modulated mesh renders incorrectly" without
// these). Ported as runtime branches on `draw_flags`, following the same
// pattern `apply_poly_flags` already established in `common.wgsl`, since
// WGSL has no compile-time per-specialization branch selection matching
// GLSL's `#if` here (the broader shader-specialization question is still
// open, scoped elsewhere in this crate's module doc; these four mutually
// exclusive branches don't need that resolved first, the same reasoning
// already applied to `apply_poly_flags`).
//
// Not yet ported, left honestly absent rather than stubbed, each a real
// `#if`-gated GLSL branch, not an invented scope cut:
// - `OPT_HWLighting`'s real per-pixel point-light loop (replaces
//   `In.LightColor` with a computed sum when on) -- this file always uses
//   the real vertex-interpolated `LightColor` instead, which is the
//   correct, already-production behavior when `OPT_HWLighting` is off.
// - `OPT_DetailTextures`/`OPT_MacroTextures` (detail/macro texture HSV
//   recoloring, same technique as `DrawComplex`'s, not yet transcribed --
//   and, per this crate's own module doc, the detail-texture loop samples
//   inside non-uniform control flow the same way the parallax loops do,
//   so it will need the same `textureSampleGrad`/`textureSampleLevel`
//   treatment when it is ported, not plain `textureSample`).
// - `OPT_BumpMaps` (needs the fragment-derivative tangent-space
//   reconstruction noted above, not yet designed).
// - `OPT_Editor`'s debug render modes and hit-testing color override
//   (editor-only, not needed for core gameplay rendering, same deferral
//   as `DrawComplex`'s).
// - `OPT_SimulateMultiPass`'s second color output (same real
//   dual-source-blending constraint documented in simple.wgsl's header).

struct DrawParameters {
    // == GLSL's DiffuseInfo.xy (TexCoords scale) and .z/.w (diffuse
    // factor / alpha) -- DetailMacroInfo/MiscInfo/TexHandles dropped,
    // not yet needed by this base path (detail/macro/bump are deferred).
    diffuse_info: vec4<f32>,
    draw_color: vec4<f32>,
    draw_flags: u32,
}

// See the correction note in simple.wgsl: a concrete, conservative,
// *provisional* capacity, not yet the real per-build-generated value.
const MAX_DRAWS: u32 = 256u;

// frame: FrameUniforms (group 0, binding 0) comes from common.wgsl --
// shared by every shader, not redeclared here (see common.wgsl's own
// header).
@group(1) @binding(0) var<uniform> draw_params: array<DrawParameters, MAX_DRAWS>;
// Same reasoning as tile.wgsl: bound once per flushed batch, not per
// draw call -- PrepareGouraudCall's SetTexture call is one of the state
// changes that forces a flush of any already-buffered triangles first.
@group(2) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(2) @binding(1) var diffuse_sampler: sampler;

struct VertexInput {
    // == GLSL's `layout(location = 0) in vec3 Coords; // == gl_Vertex`
    @location(0) coords: vec3<f32>,
    // == GLSL's `layout(location = 1) in uint DrawID; // emulated gl_DrawID`
    @location(1) draw_id: u32,
    // == GLSL's `layout(location = 3) in vec2 TexCoords; // TexCoords`
    // (location 2, Normals, is skipped here -- bump-mapping only, not
    // used anywhere in this base path's fragment shader).
    @location(2) tex_coords: vec2<f32>,
    // == GLSL's `layout(location = 4) in vec4 LightColor;`
    @location(3) light_color: vec4<f32>,
    // == GLSL's `layout(location = 5) in vec4 FogColor;`
    @location(4) fog_color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // == GLSL's `flat out uint vDrawID;`
    @location(0) @interpolate(flat) draw_id: u32,
    // == GLSL's `Out.TexCoords`
    @location(1) tex_coords: vec2<f32>,
    // == GLSL's `Out.LightColor` -- real per-vertex interpolation, not
    // flat, matching Gouraud shading.
    @location(2) light_color: vec4<f32>,
    // == GLSL's `Out.FogColor`
    @location(3) fog_color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let diffuse_info = draw_params[in.draw_id].diffuse_info;
    out.tex_coords = in.tex_coords * diffuse_info.xy;
    out.light_color = in.light_color * frame.light_color_intensity;
    out.fog_color = in.fog_color;
    out.clip_position = frame.modelviewproj_mat * vec4<f32>(in.coords, 1.0);
    out.draw_id = in.draw_id;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let diffuse_info = draw_params[in.draw_id].diffuse_info;
    let draw_flags = draw_params[in.draw_id].draw_flags;
    var color = get_texel(diffuse_texture, diffuse_sampler, in.tex_coords);
    // == GLSL's `Color *= GetDiffuseInfo(DrawID).z;` (diffuse factor)
    color = color * diffuse_info.z;
    // == GLSL's `Color.a *= GetDiffuseInfo(DrawID).w;` (alpha)
    color.a = color.a * diffuse_info.w;

    // == GLSL's `#if OPT_IsAlphaBlended Color.a *= In.LightColor.a; #endif`
    if (draw_flags & DRAW_FLAG_ALPHA_BLENDED) != 0u {
        color.a = color.a * in.light_color.a;
    }

    color = apply_poly_flags(color, draw_flags);
    // `OPT_HWLighting` off: real vertex-interpolated LightColor.
    let light_color = in.light_color;

    let is_modulated = (draw_flags & DRAW_FLAG_MODULATED) != 0u;
    var total_color: vec4<f32>;
    if (draw_flags & DRAW_FLAG_RENDER_FOG) != 0u {
        if is_modulated {
            // == GLSL's `#if OPT_IsRenderFog && OPT_IsModulated` branch.
            var delta = vec3<f32>(0.5) - color.xyz;
            delta = delta * (1.0 - in.fog_color.a);
            delta = delta * (vec3<f32>(1.0) - in.fog_color.rgb);
            total_color = vec4<f32>(vec3<f32>(0.5) - delta, color.a);
        } else {
            // == GLSL's `#if OPT_IsRenderFog && !OPT_IsModulated` branch.
            let fogged = color * light_color;
            total_color = vec4<f32>(
                fogged.rgb * (1.0 - in.fog_color.a) + in.fog_color.rgb,
                fogged.a,
            );
        }
    } else {
        if is_modulated {
            // == GLSL's `#if !OPT_IsRenderFog && OPT_IsModulated` branch.
            total_color = color;
        } else {
            // == GLSL's `#if !OPT_IsRenderFog && !OPT_IsModulated` branch.
            total_color = color * vec4<f32>(light_color.rgb, 1.0);
        }
    }
    return total_color;
}
