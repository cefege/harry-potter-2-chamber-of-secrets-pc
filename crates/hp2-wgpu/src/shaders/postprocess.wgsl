// postprocess.wgsl: WGSL port of
// ThirdParty/XOpenGLDrv/Src/DrawPostProcess.cpp's `PostProcessProgram`
// (vertex and fragment shaders read in full, 115-line file read in full)
// -- the final present/blit shader, genuinely the simplest of the five
// named in the plan: no vertex buffer, no `DrawID`-indexed batching
// (`DrawMode = GL_TRIANGLE_STRIP`, `VertexBufferSize = 0`,
// `ParametersInfo = nullptr`, the real vertex shader hardcodes
// `vDrawID = 0u`), no geometry shader, drawn exactly once per frame via
// `glDrawArrays(GL_TRIANGLE_STRIP, 0, 4)` with no bound vertex buffer at
// all -- a classic fullscreen-triangle-strip blit, computed purely from
// the vertex index.
//
// `x`/`y` come from the low two bits of the vertex index
// (`(gl_VertexID & 1) != 0 ? 1.0 : -1.0` / `(gl_VertexID & 2) != 0 ? 1.0
// : -1.0`), producing the four corners of a clip-space quad in
// triangle-strip order -- ported to WGSL's `@builtin(vertex_index)`
// directly, same bit tricks, no vertex buffer bound here either (a
// well-established WebGPU idiom for exactly this kind of draw: `draw(4,
// 1, 0, 0)` with zero vertex buffers, deriving everything from the
// builtin index).
//
// `YScale` (the vertical flip -- "-1.f when presenting through the DXGI
// interop swapchain (ReduceMouseLag), 1.f otherwise", confirmed against
// `XOpenGL.h`'s own comment on the field) and `Gamma` are both already
// in `common.wgsl`'s shared `FrameUniforms` (`y_scale`/`gamma`), not
// redeclared here -- this shader needs no `DrawParameters` struct or
// array at all, the first of the five named shaders that doesn't.
// `GammaCorrect`/`GetTexel` come from `common.wgsl` too, same as every
// other shader in this crate. `y_scale`'s own DXGI-interop use case is
// Windows/GL-specific and has no wgpu equivalent (wgpu presents through
// its own native swapchain on every backend, no interop step to work
// around) -- real Rust-side code populating `FrameUniforms` will likely
// always set it to `1.0` in practice, but the field itself is kept and
// used here anyway, for the same byte/field-fidelity reason the whole
// shared `FrameUniforms` struct mirrors the real `FrameState` in full
// rather than only the subset each shader happens to need.
//
// Real porting bug caught by review after this file's first commit, not
// before it -- a literal line-by-line transcription of
// `BlitUV = vec2(x * 0.5 + 0.5, y * 0.5 + 0.5)` is wrong in WGSL --
// confirmed against the WebGPU spec itself (`w3.org/TR/webgpu`'s own
// Coordinate Systems section, "the top-left corner is at (0.0, 0.0)...
// y increases down") and the well-documented GL-vs-WebGPU convention
// difference: both APIs agree NDC is +Y-up, but OpenGL's framebuffer
// *and* texture origin are bottom-left (so `uv.y = 0.5 + 0.5*ndc.y` is
// GL's own correct, self-consistent formula for exactly this kind of
// render-target-to-texture blit), while WebGPU's framebuffer *and*
// texture origin are both top-left instead -- self-consistent with
// each other within WebGPU too, but via the *opposite* sign: `uv.y =
// 0.5 - 0.5*ndc.y`. Ported with the corrected sign below, not the
// literal GLSL expression.
//
// Ported from the always-bound-TMU-0 path the real source's `Draw`
// function uses directly (`glActiveTexture(GL_TEXTURE0 +
// DiffuseTextureIndex); glBindTexture(GL_TEXTURE_2D, Texture);`) --
// there is no bindless/non-bindless split to choose between for this
// shader at all, since it never goes through `SetTexture`/`GetTexHandle`
// the way the other four do; it binds whatever source texture the C++
// side hands it (the just-rendered scene color target) directly.

@group(1) @binding(0) var source_texture: texture_2d<f32>;
@group(1) @binding(1) var source_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // == GLSL's `out vec2 BlitUV;`
    @location(0) blit_uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // == GLSL's `float x = (gl_VertexID & 1) != 0 ? 1.0 : -1.0;` /
    // `float y = (gl_VertexID & 2) != 0 ? 1.0 : -1.0;`
    let x = select(-1.0, 1.0, (vertex_index & 1u) != 0u);
    let y = select(-1.0, 1.0, (vertex_index & 2u) != 0u);
    // GL's `BlitUV = vec2(x * 0.5 + 0.5, y * 0.5 + 0.5)`, Y-flipped for
    // WebGPU's opposite texture/framebuffer origin -- see this file's
    // own header for why.
    out.blit_uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    // == GLSL's `gl_Position = vec4(x, y * YScale, 0.0, 1.0);`
    out.clip_position = vec4<f32>(x, y * frame.y_scale, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // == GLSL's `FragColor = GammaCorrect(Gamma, texture(Texture0, BlitUV));`
    return gamma_correct(frame.gamma, get_texel(source_texture, source_sampler, in.blit_uv));
}
