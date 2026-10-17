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
// Real, deliberate simplification from the GLSL source, not an
// oversight: the GLSL vertex shader threads a per-draw `DrawID` through
// to the fragment shader (`vDrawID`), which `GetDrawColor(vDrawID)`
// then uses to index into a shared parameter array/buffer -- a scheme
// that exists to let one shader *program* serve many draw calls without
// rebinding state between them (needed for the SSBO/bindless multi-draw
// path). The non-SSBO fallback this file ports from only ever has one
// `DrawColor` bound at a time per draw call in the first place (CPU
// rebinds a small UBO before each draw), so the index is never actually
// used for anything there -- WGSL's own idiom for "one small uniform
// value per draw call" is a uniform buffer bound via a *dynamic offset*
// into a larger buffer, which needs no shader-side index parameter at
// all. `draw_id` is dropped entirely here, not carried through unused.
//
// Not yet ported, left honestly absent rather than stubbed: the
// `OPT_ClipDistance` custom clip-plane path (WGSL has no
// `gl_ClipDistance` equivalent; this crate's own module doc already
// scopes the real replacement as a fragment-shader `discard`, needed
// only for the plan's later "Editor hit testing" bullet, not core
// gameplay rendering) and the `OPT_SimulateMultiPass` dual-render-target
// output (depends on PolyFlags-driven blend-state selection not yet
// examined). Both are real, used GLSL features, not invented ones --
// deferred, not forgotten.

struct FrameUniforms {
    modelviewproj_mat: mat4x4<f32>,
}

struct DrawUniforms {
    draw_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: FrameUniforms;
@group(1) @binding(0) var<uniform> draw: DrawUniforms;

struct VertexInput {
    // == GLSL's `layout(location = 0) in vec3 Coords; // == gl_Vertex`
    @location(0) coords: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = frame.modelviewproj_mat * vec4<f32>(in.coords, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return draw.draw_color;
}
