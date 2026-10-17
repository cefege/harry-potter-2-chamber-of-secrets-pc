// gouraud.wgsl: WGSL port of the *base* (non-bump-mapped,
// non-hardware-lit) path of ThirdParty/XOpenGLDrv/Src/DrawGouraud_GLSL.cpp's
// `DrawGouraudProgram` (vertex shader read in full, geometry shader read in
// full, fragment shader read through its main diffuse/light/fog/detail
// blend path and its OPT_Editor-gated tail -- `OPT_MacroTextures` and
// `OPT_BumpMaps`'s fragment-side specular loop not yet read in full).
// Covers UE1's `DrawGouraudPolygon`/`DrawTriangles` (both funnel through
// `PrepareGouraudCall`/`FinishGouraudCall` onto the same
// `DrawGouraudProgram`, confirmed by reading `DrawGouraud.cpp` in full
// per this crate's own module doc).
//
// Geometry shader: `layout(triangles) in; layout(triangle_strip,
// max_vertices = 3) out;`, with a `for (i in 0..3) { ...; EmitVertex(); }`
// body -- confirmed, by reading the full body, to never amplify topology:
// exactly one triangle out per triangle in. Its real purpose is
// `OPT_BumpMaps`-only: computing one tangent/bitangent *per triangle*
// (`GetTangent`/`GetBitangent`, needing all three vertices' positions and
// UVs at once, which a per-vertex-only vertex shader cannot do) and
// deferring the final `modelviewprojMat` transform to this stage (the
// vertex shader only emits object-space `gl_Position = vec4(Coords, 1.0)`
// when `OPT_GeometryShaders` is on, because the tangent/bitangent math
// needs object-space, not clip-space, input). Confirms this crate's own
// earlier scoping decision rather than changing it: no WGSL geometry-stage
// equivalent exists, and the portable replacement for the tangent/
// bitangent computation this geometry shader exists for is fragment-shader
// screen-space derivatives (`dpdx`/`dpdy`) -- not yet implemented, because
// it is purely a bump-mapping concern (`OPT_BumpMaps && OPT_HasBumpMap`),
// out of scope for this base-path port the same way DrawComplex's parallax
// and bump-mapping sections are. The MVP transform this file's vertex
// shader does is the plain, always-correct `OPT_GeometryShaders`-off GLSL
// branch (`gl_Position = modelviewprojMat * vec4(Coords, 1.0)`) -- the
// right one to port from regardless of bump-map status, since it is what
// every non-bump-mapped draw already uses today.
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
// Not yet ported, left honestly absent rather than stubbed, each a real
// `#if`-gated GLSL branch, not an invented scope cut:
// - `OPT_HWLighting`'s real per-pixel point-light loop (replaces
//   `In.LightColor` with a computed sum when on) -- this base path always
//   uses the real vertex-interpolated `LightColor` instead, which is the
//   correct, already-production behavior when `OPT_HWLighting` is off.
// - `OPT_IsRenderFog`'s real-time fog blend (both its `OPT_IsModulated`
//   and non-modulated sub-branches) -- this base path always takes the
//   `!OPT_IsRenderFog` branch (`TotalColor = Color * vec4(LightColor.rgb,
//   1.0)` for the non-modulated case ported here; `OPT_IsModulated`'s
//   `TotalColor = Color` deferred along with it, both real PolyFlags-
//   driven branches, not examined deeply enough yet to port correctly).
// - `OPT_DetailTextures`/`OPT_MacroTextures` (detail/macro texture HSV
//   recoloring, same technique as `DrawComplex`'s, not yet transcribed).
// - `OPT_BumpMaps` (needs the fragment-derivative tangent-space
//   reconstruction noted above, not yet designed).
// - `OPT_Editor`'s debug render modes and hit-testing color override
//   (editor-only, not needed for core gameplay rendering, same deferral
//   as `DrawComplex`'s).
// - `OPT_SimulateMultiPass`'s second color output (same real
//   dual-source-blending constraint documented in simple.wgsl's header).

struct FrameUniforms {
    modelviewproj_mat: mat4x4<f32>,
    light_color_intensity: f32,
}

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

@group(0) @binding(0) var<uniform> frame: FrameUniforms;
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
    color = apply_poly_flags(color, draw_flags);
    // == GLSL's `!OPT_IsRenderFog` / `!OPT_IsModulated` branch:
    // `TotalColor = Color * vec4(LightColor.rgb, 1.0)`.
    return color * vec4<f32>(in.light_color.rgb, 1.0);
}
