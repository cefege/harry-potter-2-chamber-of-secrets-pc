// common.wgsl: WGSL port of the shared GLSL prelude functions
// ThirdParty/XOpenGLDrv/Src/ShaderProgram.cpp emits into every shader program
// (`UXOpenGLRenderDevice::ShaderProgram::EmitHeader`, read in full before
// writing this file) -- `PlaneDot`, `GammaCorrect`, `rgb2hsv`/`hsv2rgb`,
// `GetTexel`, `ApplyPolyFlags`, `getFogFactor`.
//
// Not a shader module on its own -- no @vertex/@fragment entry points, and
// not meant to be compiled standalone by anything other than this crate's
// own naga validation test (every_wgsl_shader_parses_and_validates, which
// iterates src/shaders/*.wgsl and validates each file in isolation; this
// file validates on its own because naga accepts a module containing only
// unused helper functions). WGSL has no `#include`/module-import mechanism
// at the language level (unlike GLSL's string concatenation, which is how
// the real source assembles this prelude plus each shader's own body into
// one compilation unit) -- shaders that need these functions are expected
// to be combined with this file's source via plain Rust-side string
// concatenation before being handed to `wgpu::ShaderSource::Wgsl`, the
// direct WGSL-side equivalent of what the GLSL source already does. Not
// wired into any shader yet -- simple.wgsl doesn't need any of these
// (no texture sampling, no fog, no PolyFlags-driven discard), so this file
// exists ahead of the shader that will actually need it (DrawTile/
// DrawGouraud/DrawComplex), not retrofitted after.
//
// Real, deliberate simplifications from the GLSL source, each because WGSL
// functions can take parameters GLSL's free global functions relied on
// implicit shader-wide state for instead, keeping this file composable
// regardless of which @group/@binding indices a particular consuming
// shader ends up using (deferred entirely to each consumer, not assumed
// here):
// - `GetTexel` takes the texture and sampler as explicit parameters
//   (`texture_2d<f32>`, `sampler`) instead of relying on a module-level
//   binding. Ported from the GLSL source's non-bindless (`#else`) branch
//   only -- `texture(BoundSampler, TexCoords)` -- per this crate's own
//   module doc: the bindless branch has no WebGPU equivalent at all, and
//   the non-bindless fallback is a complete, already-in-production code
//   path this crate transcribes from throughout.
// - `getFogFactor` takes DistanceFogMode/DistanceFogDensity/
//   DistanceFogStart/DistanceFogEnd as explicit parameters instead of
//   reading them from a module-level `DistanceFogParams` uniform block,
//   for the same reason.
// - `ApplyPolyFlags` uses WGSL's fragment-stage-only `discard` statement,
//   the same restriction GLSL's `discard` already has -- this function can
//   only be called from a @fragment entry point, exactly as in the real
//   source. OPT_IsMasked and OPT_IsAlphaBlended are GLSL compile-time
//   `#if` branches selecting one of two discard conditions (never both);
//   ported as a runtime `if`/`else if` on bits of `draw_flags` instead,
//   since WGSL has no compile-time branch-selection matching GLSL's
//   per-specialization `#if` here -- this crate's own module doc already
//   scopes the open question of how GLSL's shader-specialization system as
//   a whole maps to WGSL/wgpu; this one function's two mutually exclusive
//   branches are simple enough not to need that resolved first.
//
// Not ported: `GammaCorrect` and `rgb2hsv`/`hsv2rgb` are ported in full
// despite no shader using them yet (DrawComplex's detail/macro texture
// HSV recoloring will, per this crate's own module doc) -- pure math
// functions with no binding/specialization questions attached, safe to
// port ahead of their first real caller. `PlaneDot` is ported too, for
// `OPT_ClipDistance` custom clip planes -- still not wired to anything,
// since the portable clip-plane replacement (fragment `discard`, per this
// crate's own module doc) hasn't been designed yet; the function itself is
// trivial and costs nothing to have ready.

// == `ShaderDrawFlags::DF_Masked`/`DF_AlphaBlended`, confirmed against
// ThirdParty/XOpenGLDrv/Inc/XOpenGL.h:1221,1227.
const DRAW_FLAG_MASKED: u32 = 0x0100u;
const DRAW_FLAG_ALPHA_BLENDED: u32 = 0x4000u;

fn plane_dot(plane: vec4<f32>, point: vec3<f32>) -> f32 {
    return dot(plane.xyz, point) - plane.w;
}

fn gamma_correct(gamma: f32, color: vec4<f32>) -> vec4<f32> {
    let inv_gamma = 1.0 / gamma;
    return vec4<f32>(
        pow(color.r, inv_gamma),
        pow(color.g, inv_gamma),
        pow(color.b, inv_gamma),
        color.a,
    );
}

// http://lolengine.net/blog/2013/07/27/rgb-to-hsv-in-glsl -- ported
// verbatim from the GLSL source's own comment-attributed algorithm.
fn rgb2hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = select(vec4<f32>(c.gb, k.xy), vec4<f32>(c.bg, k.wz), c.g < c.b);
    let q = select(vec4<f32>(c.r, p.yzx), vec4<f32>(p.xyw, c.r), c.r < p.x);
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + k.xyz) * 6.0 - k.www);
    return c.z * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

// == GLSL's non-bindless `GetTexel(uvec2 BindlessTexHandle, sampler2D
// BoundSampler, vec2 TexCoords)` branch, minus the always-ignored
// BindlessTexHandle parameter that branch never reads.
fn get_texel(tex: texture_2d<f32>, samp: sampler, tex_coords: vec2<f32>) -> vec4<f32> {
    return textureSample(tex, samp, tex_coords);
}

// == GLSL's `ApplyPolyFlags(vec4 Color, uint DrawFlags)`. Fragment-stage
// only: `discard` is only legal inside a @fragment entry point in WGSL,
// exactly as in the real source.
fn apply_poly_flags(color: vec4<f32>, draw_flags: u32) -> vec4<f32> {
    var result = color;
    if (draw_flags & DRAW_FLAG_MASKED) != 0u {
        if result.a < 0.5 {
            discard;
        } else {
            result = vec4<f32>(result.rgb / result.a, result.a);
        }
    } else if (draw_flags & DRAW_FLAG_ALPHA_BLENDED) != 0u {
        if result.a < 0.01 {
            discard;
        }
    }
    return result;
}

// == GLSL's `getFogFactor(float FogCoord)`. DistanceFogMode: -1 disabled,
// 0 linear, 1 exp, 2 exp2 -- matches the real source's own comment.
fn get_fog_factor(
    fog_coord: f32,
    distance_fog_mode: i32,
    distance_fog_density: f32,
    distance_fog_start: f32,
    distance_fog_end: f32,
) -> f32 {
    var fog_result = 1.0;
    if distance_fog_mode == 0 {
        fog_result = (distance_fog_end - fog_coord) / (distance_fog_end - distance_fog_start);
    } else if distance_fog_mode == 1 {
        fog_result = exp(-distance_fog_density * fog_coord);
    } else if distance_fog_mode == 2 {
        fog_result = exp(-pow(distance_fog_density * fog_coord, 2.0));
    }
    fog_result = 1.0 - clamp(fog_result, 0.0, 1.0);
    return fog_result;
}
