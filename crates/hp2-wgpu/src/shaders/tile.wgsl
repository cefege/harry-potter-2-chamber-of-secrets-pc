// tile.wgsl: WGSL port of ThirdParty/XOpenGLDrv/Src/DrawTile_GLSL.cpp's
// `DrawTileESProgram` (`BuildVertexShader`/`BuildFragmentShader`, read in
// full before writing this file), not `DrawTileCoreProgram` -- per this
// crate's own module doc, the Core program's geometry shader genuinely
// amplifies one submitted point into a quad's six vertices (1-to-6
// expansion), which has no WGSL equivalent (WGSL has no geometry stage),
// while the ES program already does the quad expansion CPU-side, in
// `DrawTile.cpp`'s `SubmitTileBatch` itself (confirmed by reading that
// function in full: it writes six full vertices with real per-corner
// `Coords`/`TexCoords` into its vertex buffer directly, no geometry
// shader involved) -- a direct, line-by-line transcription target, not a
// design problem.
//
// Covers UE1's `DrawTile`/`DrawNativeTextTile` (both funnel through the
// same `SubmitTileBatch`, confirmed by reading `DrawTile.cpp` in full --
// `DrawNativeTextTile` is a thin wrapper, not a separate rendering path).
//
// Ported from the non-bindless fallback, same as every other shader in
// this crate (see this crate's own module doc, "Shader transcription
// scoping"): `GetTexel`'s real GLSL signature takes a bindless texture
// handle *and* a TMU-bound sampler, and the non-bindless branch ignores
// the handle entirely (`texture(BoundSampler, TexCoords)`) -- this file's
// `get_texel` call (from common.wgsl) only ever needs the bound
// texture/sampler, never a handle.
//
// `EyeSpacePos` (the GLSL vertex shader's `Out.EyeSpacePos = modelviewMat
// * vec4(Coords, 1.0)`) is computed but never read anywhere in
// `DrawTileESProgram`'s own fragment shader -- confirmed by reading both
// shader bodies in full, not assumed from the varying's mere presence --
// so it is not carried as a varying here at all. (XOpenGL's own shader
// compiler would have dropped the unused varying too; GLSL interface
// blocks don't force that the way a naive first read of the struct might
// suggest.)
//
// DrawID-indexed parameters: a fixed-size uniform array, not a storage
// buffer, for the same portability reason documented at length in this
// crate's module doc and in simple.wgsl's own header (WebGL2/GLES3.0 has
// zero storage buffers available, confirmed against
// `wgpu_types::Limits::downlevel_webgl2_defaults()`). `DrawParameters`
// here carries only `draw_color`/`draw_flags` -- the GLSL source's
// `TexHandle` (bindless-only, unused in the non-bindless path this file
// transcribes from) and `Dummy0`/`Dummy1`/`Dummy2` (GLSL std140 manual
// padding, which WGSL's own uniform-address-space layout rules handle
// automatically -- no manual padding fields needed here) are both
// dropped, not carried through unused.
//
// Not yet ported, left honestly absent rather than stubbed:
// `OPT_SimulateMultiPass`'s second color output (same real dual-source-
// blending constraint documented in simple.wgsl's header: wgpu's optional
// `Features::DUAL_SOURCE_BLENDING`, not guaranteed on every backend or
// web, deferred as a feature-gated follow-up).

struct FrameUniforms {
    modelviewproj_mat: mat4x4<f32>,
}

struct DrawParameters {
    draw_color: vec4<f32>,
    draw_flags: u32,
}

// See the correction note in simple.wgsl: a concrete, conservative,
// *provisional* capacity, not yet the real per-build-generated value.
const MAX_DRAWS: u32 = 256u;

@group(0) @binding(0) var<uniform> frame: FrameUniforms;
@group(1) @binding(0) var<uniform> draw_params: array<DrawParameters, MAX_DRAWS>;
// Bound once per flushed batch, not per draw call -- DrawTile.cpp's
// SubmitTileBatch only ever has one TMU-bound diffuse texture active at
// a time (SetTexture before the batch starts), and a texture change is
// one of the state changes that forces a flush of any already-buffered
// tiles first (confirmed by reading DrawComplex.cpp's/DrawGouraud.cpp's/
// DrawTile.cpp's shared flush-on-state-change batching, surveyed in this
// crate's module doc), so unlike draw_params this never needs per-draw
// indexing.
@group(2) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(2) @binding(1) var diffuse_sampler: sampler;

struct VertexInput {
    // == GLSL's `layout(location = 0) in vec3 Coords; // ==gl_Vertex`
    @location(0) coords: vec3<f32>,
    // == GLSL's `layout(location = 1) in uint DrawID; // emulated gl_DrawID`
    @location(1) draw_id: u32,
    // == GLSL's `layout(location = 2) in vec2 TexCoords;`
    @location(2) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // == GLSL's `flat out uint vDrawID;`
    @location(0) @interpolate(flat) draw_id: u32,
    // == GLSL's `Out.TexCoords.xy` (the varying's .zw are always 0 in
    // the GLSL source and never read; dropped here, not carried unused).
    @location(1) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = frame.modelviewproj_mat * vec4<f32>(in.coords, 1.0);
    out.draw_id = in.draw_id;
    out.tex_coords = in.tex_coords;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = get_texel(diffuse_texture, diffuse_sampler, in.tex_coords);
    let draw_flags = draw_params[in.draw_id].draw_flags;
    // == GLSL's `ApplyPolyFlags(Color, DrawFlags) * GetDrawColor(vDrawID)`
    return apply_poly_flags(color, draw_flags) * draw_params[in.draw_id].draw_color;
}
