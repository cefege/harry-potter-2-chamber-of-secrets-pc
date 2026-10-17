// simple.wgsl: WGSL port of ThirdParty/XOpenGLDrv/Src/DrawSimple_GLSL.cpp's
// `SimpleVertexShader`/`SimpleFragmentShader`, shared by UE1's
// `UXOpenGLRenderDevice::Draw2DLine`/`Draw2DPoint`/`Draw3DLine` (via
// `DrawSimpleLineProgram`) and simple untextured triangle fills (via
// `DrawSimpleTriangleProgram`) -- both C++ classes build from the exact
// same two shader strings in the real source, confirmed by reading it,
// not assumed; this one file covers both the WGSL side too.
//
// Ported from the non-bindless, non-SSBO-draw-parameters fallback shape,
// not the GLSL source's default/bindless path -- see this crate's own
// module doc ("Shader transcription scoping") for why: that fallback is
// a complete, already-in-production code path (disabled on AMD GPUs
// today regardless of hardware support), and it is the one that maps
// directly onto WGSL's bind-group model with no bindless-handle
// equivalent needed.
//
// CORRECTED (an earlier version of this file wrongly dropped DrawID
// entirely, on an unverified claim that the non-SSBO fallback only ever
// binds one DrawColor per draw call -- caught on review, then actually
// disproven by reading ShaderProgram.cpp's EmitDrawCallParametersHeader:
// the UBO path emits `layout(std140) uniform ... { DrawCallParameters
// Draw<Name>Params[ParametersBufferSize]; }`, a bounded *array*, and the
// `Get<Field>(uint DrawID)` accessor functions it generates index that
// array by DrawID identically for both the SSBO and UBO paths -- there
// is no code path where only one DrawColor is ever bound. DrawID really
// is load-bearing: it is what lets the real batching architecture
// surveyed in this crate's module doc (DrawComplex.cpp/DrawGouraud.cpp/
// DrawTile.cpp's ring-buffer flush-on-state-change batching) pack many
// logical draw calls' worth of per-draw parameters into one real GPU
// draw call.
//
// This WGSL file threads `draw_id` through exactly as the GLSL does,
// but backs it with a *storage* buffer (`var<storage, read>`) rather
// than replicating GLSL's UBO-vs-SSBO split: WebGPU storage buffers are
// core, unconditionally available on every backend and web tier (no
// feature gate, unlike GLSL's SSBO extension), so there is no
// capability reason to keep two separate paths the way the GLSL source
// has to for GL driver compatibility -- one array-indexed storage
// buffer covers both GLSL paths' semantics.
//
// Not yet ported, left honestly absent rather than stubbed: the
// `OPT_ClipDistance` custom clip-plane path (WGSL has no
// `gl_ClipDistance` equivalent at the WGSL language level, but wgpu
// does expose an optional, backend-dependent `Features::CLIP_DISTANCES`
// surfacing WGSL's `clip_distances` extension where supported -- not
// relied on here; this crate's own module doc already scopes the
// portable replacement as a fragment-shader `discard` instead, needed
// only for the plan's later "Editor hit testing" bullet, not core
// gameplay rendering) and the `OPT_SimulateMultiPass` second color
// output (GLSL's Core-GL path emits it via
// `layout(location = 0, index = 1) out vec4 FragColor1` -- real
// dual-source blending, which in WebGPU is the optional
// `Features::DUAL_SOURCE_BLENDING` feature plus WGSL's
// `enable dual_source_blending;` and `@blend_src(1)`, not guaranteed on
// every backend or on web -- deferred as a feature-gated follow-up, not
// part of the base path). The GLSL fragment shader's stippled-line
// experiment (`#if 0 && OPT_Transparent`) is permanently dead code in
// the real source -- `0 && anything` can never be true -- and is
// correctly not transcribed here at all.
//
// Depth-range contract (not yet exercised by any real code -- no Rust
// code builds `frame.modelviewproj_mat` yet): GLSL/OpenGL's clip-space
// Z range is -1..1; WebGPU's is 0..1. This shader does not remap Z
// per-vertex. The contract is that whatever Rust code eventually
// populates `FrameUniforms::modelviewproj_mat` is responsible for
// building it directly in WebGPU's 0..1 convention (e.g. glam's
// `*_lh`/`*_rh` zero-to-one projection constructors), not reusing a
// GLSL-convention matrix unmodified. Recorded here so the decision is
// made once, not re-litigated per shader.

struct FrameUniforms {
    modelviewproj_mat: mat4x4<f32>,
}

struct DrawParameters {
    draw_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: FrameUniforms;
// Matches the GLSL array-indexed-by-DrawID shape (see the correction
// note above), backed by a storage buffer rather than replicating
// GLSL's separate bounded-UBO-array/unbounded-SSBO-array paths.
@group(1) @binding(0) var<storage, read> draw_params: array<DrawParameters>;

struct VertexInput {
    // == GLSL's `layout(location = 0) in vec3 Coords; // == gl_Vertex`
    @location(0) coords: vec3<f32>,
    // == GLSL's `layout(location = 1) in uint DrawID; // emulated gl_DrawID`
    @location(1) draw_id: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // == GLSL's `flat out uint vDrawID;`
    @location(0) @interpolate(flat) draw_id: u32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = frame.modelviewproj_mat * vec4<f32>(in.coords, 1.0);
    out.draw_id = in.draw_id;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // == GLSL's `GetDrawColor(vDrawID)`
    return draw_params[in.draw_id].draw_color;
}
