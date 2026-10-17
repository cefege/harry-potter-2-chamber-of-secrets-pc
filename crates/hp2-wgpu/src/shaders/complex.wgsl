// complex.wgsl: WGSL port of the *base* path (diffuse + lightmap + fog,
// no parallax/bump/detail/macro/environment/hardware-lighting/editor) of
// ThirdParty/XOpenGLDrv/Src/DrawComplex_GLSL.cpp's `DrawComplexProgram`
// (vertex shader read in full, fragment shader read in full earlier this
// session -- this file ports only its base-path lines, per this crate's
// module doc's own scoping and the plan's own explicit guidance: "port
// the base path first (diffuse, lightmap, fog) and gate parallax, bump,
// detail and macro behind their OPT_* flags as follow-ups"). Covers
// UE1's `DrawComplexSurface` (BSP surface rendering).
//
// No geometry shader at all -- confirmed by this crate's own earlier
// full read of this file, the one shader of the five with no
// geometry-stage question to resolve. Its bump-map tangent space
// (`vTBNMat`, not ported here -- base path only) is computed directly in
// the vertex shader from the surface's own known `XAxis`/`YAxis`/`ZAxis`
// UV mapping axes, not derived from adjacent-triangle geometry, so even
// that deferred feature needs no geometry-shader-replacement design
// either, unlike `DrawGouraud`'s.
//
// Real UV mapping math, not a simple pass-through: BSP surfaces don't
// carry their own per-vertex UVs the way a regular mesh does -- each
// surface has its own planar UV-mapping axes (`XAxis`/`YAxis`, each a
// `vec4` with the axis direction in `.xyz` and a dot-product bias in
// `.w`), and every texture layer's UV coordinate is `(dot(axis, Coords)
// - bias - pan) * mult`, computed once per layer from the shared
// `MapDot` intermediate. Ported exactly, not simplified.
//
// Ported from the non-bindless fallback, same as every other shader in
// this crate: `GetTexel`'s non-bindless branch (`common.wgsl`'s
// `get_texel`) needs only the bound texture/sampler, never a handle
// (the real source's `GetTexHandleHelper`, which combines bindless
// handle pairs, is not needed here at all).
//
// `LightMapIntensity` (`LightColor.rgb *= LightMapIntensity * 255.0 /
// 127.0` in the real fragment shader -- lightmap texel data is stored in
// a 0..127/255 range and needs rescaling to 0..2) is a real frame-global
// value, not a per-draw one, confirmed the same way `gouraud.wgsl`'s
// `LightColorIntensity` was -- already carried in `common.wgsl`'s shared
// `FrameUniforms.light_map_intensity`, not redeclared here.
//
// Not yet ported, left honestly absent rather than stubbed, each a real
// `#if`-gated GLSL branch, not an invented scope cut (this crate's own
// module doc already scopes each of these at length from reading the
// full fragment shader body; restated briefly here, not re-derived):
// - `OPT_HWLighting`'s real per-pixel point-light loop -- this base path
//   always uses the real lightmap-texture-sampled `LightColor` instead,
//   which is the correct, already-production behavior when
//   `OPT_HWLighting` is off.
// - `OPT_DetailTextures`/`OPT_MacroTextures`/`OPT_EnvironmentMaps`/
//   `OPT_HeightMaps` (parallax) -- all real optional texture layers, none
//   transcribed yet; the parallax loops specifically need
//   `textureSampleGrad`/`textureSampleLevel`, not plain `textureSample`
//   (non-uniform control flow, per this crate's module doc).
// - `OPT_BumpMaps` -- needs splitting `vTBNMat` into three flat
//   `vec3<f32>` varyings (WGSL has no `mat3x3<f32>` inter-stage IO),
//   same constraint already noted for `DrawGouraud`'s bump path.
// - `OPT_DistanceFog`'s eye-space-depth-driven fog mix (distinct from
//   the real-time fogmap-texture blend this file *does* port).
// - `OPT_Editor`'s debug render modes and hit-testing color override.
// - `OPT_SimulateMultiPass`'s second color output (same real
//   dual-source-blending constraint documented in simple.wgsl's header).
//
// Lightmap and fogmap textures/samplers are always bound (WGSL can't
// conditionally declare a binding the way GLSL's `#if OPT_HasLightMap`/
// `#if OPT_HasFogMap` compile-time specialization can), but the real
// per-draw `DF_LightMap`/`DF_FogMap` flags (confirmed against
// `DrawComplex.cpp`'s `SetTextureHelper`, which only ORs them into
// `DrawFlags` when `Surface.LightMap`/`Surface.FogMap` is actually
// non-null) gate whether the sampled value is used at all, via
// `select()` rather than an `if` around the sample itself -- see the
// real comments at each call site below for why. This shader's own
// logic now handles a surface with no lightmap or fogmap correctly
// (an earlier version of this file applied both unconditionally
// instead, overbright-multiplying every such surface by whatever was
// bound to the unused texture slot -- caught on review, fixed) -- not
// the same claim as a surface actually rendering correctly today,
// since nothing calls this shader yet; the Rust side still has to
// bind *some* real 1x1 texture into each of these always-declared
// bindings whenever a real surface has neither layer -- any valid
// texture works, its color is never read: `select()` discards the
// sampled value entirely whenever the corresponding flag is clear, so
// nothing about this binding is load-bearing except that it exists and
// is a valid texture -- which is not written yet either. What remains
// a real, not yet resolved
// simplification is the broader shader-specialization question this
// crate's module doc already scopes: the real GLSL still compiles a
// genuinely different shader per combination of optional texture
// layers for other reasons (fewer bindings, fewer instructions, no
// always-bound dummy textures to manage Rust-side), not just to
// decide presence/absence the way this file now does at runtime.

struct DrawParameters {
    diffuse_uv: vec4<f32>,
    light_map_uv: vec4<f32>,
    fog_map_uv: vec4<f32>,
    // == GLSL's DiffuseInfo.x (diffuse factor), .z (alpha), .w
    // (separate per-surface alpha, applied after the lightmap/fog
    // blend). .y is the only component not read anywhere in the base
    // path, confirmed by reading the full fragment shader body.
    diffuse_info: vec4<f32>,
    x_axis: vec4<f32>,
    y_axis: vec4<f32>,
    draw_flags: u32,
}

// See the correction note in simple.wgsl: a concrete, conservative,
// *provisional* capacity, not yet the real per-build-generated value.
const MAX_DRAWS: u32 = 256u;

// frame: FrameUniforms (group 0, binding 0) comes from common.wgsl --
// shared by every shader, not redeclared here.
@group(1) @binding(0) var<uniform> draw_params: array<DrawParameters, MAX_DRAWS>;
// Same reasoning as tile.wgsl/gouraud.wgsl: bound once per flushed
// batch, not per draw call -- SetTexture is one of the state changes
// that forces a flush of any already-buffered facets first.
@group(2) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(2) @binding(1) var diffuse_sampler: sampler;
@group(2) @binding(2) var light_map_texture: texture_2d<f32>;
@group(2) @binding(3) var light_map_sampler: sampler;
@group(2) @binding(4) var fog_map_texture: texture_2d<f32>;
@group(2) @binding(5) var fog_map_sampler: sampler;

struct VertexInput {
    // == GLSL's `layout(location = 0) in vec3 Coords; // == gl_Vertex`
    @location(0) coords: vec3<f32>,
    // == GLSL's `layout(location = 1) in uint DrawID; // emulated gl_DrawID`
    @location(1) draw_id: u32,
    // Normal (location 2 in the real source) is skipped here -- unused
    // anywhere in the base path (only OPT_Editor/OPT_BumpMaps/
    // OPT_HWLighting read GetZAxis/MapCoordsZAxis, all deferred).
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // == GLSL's `flat out uint vDrawID;`
    @location(0) @interpolate(flat) draw_id: u32,
    // == GLSL's `out vec2 vTexCoords;`
    @location(1) tex_coords: vec2<f32>,
    // == GLSL's `out vec2 vLightMapCoords;`
    @location(2) light_map_coords: vec2<f32>,
    // == GLSL's `out vec2 vFogMapCoords;`
    @location(3) fog_map_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let params = draw_params[in.draw_id];

    // == GLSL's `vec3 MapCoordsXAxis = GetXAxis(DrawID).xyz;` /
    // `vec3 MapCoordsYAxis = GetYAxis(DrawID).xyz;` / `float UDot =
    // GetXAxis(DrawID).w;` / `float VDot = GetYAxis(DrawID).w;`
    let map_coords_x_axis = params.x_axis.xyz;
    let map_coords_y_axis = params.y_axis.xyz;
    let u_dot = params.x_axis.w;
    let v_dot = params.y_axis.w;

    // == GLSL's `float MapDotU = dot(MapCoordsXAxis, Coords.xyz) - UDot;`
    // / `float MapDotV = dot(MapCoordsYAxis, Coords.xyz) - VDot;`
    let map_dot = vec2<f32>(
        dot(map_coords_x_axis, in.coords) - u_dot,
        dot(map_coords_y_axis, in.coords) - v_dot,
    );

    // == GLSL's `vTexCoords = (MapDot - TexMapPan) * TexMapMult;`
    out.tex_coords = (map_dot - params.diffuse_uv.zw) * params.diffuse_uv.xy;
    // == GLSL's `vLightMapCoords = (MapDot - LightMapPan) * LightMapMult;`
    out.light_map_coords = (map_dot - params.light_map_uv.zw) * params.light_map_uv.xy;
    // == GLSL's `vFogMapCoords = (MapDot - FogMapPan) * FogMapMult;`
    out.fog_map_coords = (map_dot - params.fog_map_uv.zw) * params.fog_map_uv.xy;

    out.clip_position = frame.modelviewproj_mat * vec4<f32>(in.coords, 1.0);
    out.draw_id = in.draw_id;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let params = draw_params[in.draw_id];
    let draw_flags = params.draw_flags;

    var color = get_texel(diffuse_texture, diffuse_sampler, in.tex_coords);
    // == GLSL's `Color *= GetDiffuseInfo(DrawID).x; // Diffuse factor.`
    color = color * params.diffuse_info.x;
    // == GLSL's `Color.a *= GetDiffuseInfo(DrawID).z; // Alpha.`
    color.a = color.a * params.diffuse_info.z;

    var total_color = apply_poly_flags(color, draw_flags);

    // == GLSL's `OPT_HasLightMap` branch (`OPT_HWLighting` off):
    // `LightColor = GetTexel(...LightMapIndex..., vLightMapCoords);
    // LightColor.rgb *= LightMapIntensity * 255.0 / 127.0; LightColor.a
    // = 1.0;` -- GLSL defaults `LightColor = vec4(1.0)` (line 405) and
    // only overwrites it under `#if OPT_HasLightMap`; an earlier version
    // of this file applied the lightmap unconditionally instead, which
    // would have overbright-multiplied every surface without a lightmap
    // by whatever garbage happened to be in the (always-bound, since
    // WGSL can't conditionally declare a binding) light_map_texture slot
    // -- caught on review, confirmed against the real source's default
    // and its real per-draw gate: `DrawComplex.cpp`'s `SetTextureHelper`
    // only ORs `ShaderDrawFlags::DF_LightMap` (0x0002, confirmed against
    // `XOpenGL.h:1212`) into `DrawFlags` when `Surface.LightMap` is
    // non-null. Sampled unconditionally here (not inside an `if`) and
    // selected with `select()`: `draw_flags` is a per-draw, not a
    // uniform-across-the-whole-pipeline, value -- different fragments in
    // the *same* real GPU draw call can have different `DrawID`s (many
    // batched UE1 draw calls share one flushed draw call), so branching
    // on it around a texture sample would be exactly the kind of
    // non-uniform control flow this crate's own module doc already
    // flags as a real hazard for the parallax loops; `select()` avoids
    // the question entirely rather than relying on naga's (confirmed
    // elsewhere in this crate) non-enforcement of it.
    let light_map_sample = get_texel(light_map_texture, light_map_sampler, in.light_map_coords);
    let light_color = select(
        vec4<f32>(1.0, 1.0, 1.0, 1.0),
        vec4<f32>(light_map_sample.rgb * (frame.light_map_intensity * 255.0 / 127.0), 1.0),
        (draw_flags & DRAW_FLAG_LIGHT_MAP) != 0u,
    );

    // == GLSL's `FogColor = GetTexel(...FogMapIndex..., vFogMapCoords) *
    // 2.0;` -- same real default-then-gated shape as LightColor above:
    // GLSL defaults `FogColor = vec4(0.0)` (line 558), only overwritten
    // under `#if OPT_HasFogMap`, gated the same way by the real
    // `ShaderDrawFlags::DF_FogMap` (0x0004, `XOpenGL.h:1213`) `SetTexture
    // Helper` ORs in only when `Surface.FogMap` is non-null.
    let fog_map_sample = get_texel(fog_map_texture, fog_map_sampler, in.fog_map_coords) * 2.0;
    let fog_color = select(
        vec4<f32>(0.0, 0.0, 0.0, 0.0),
        fog_map_sample,
        (draw_flags & DRAW_FLAG_FOG_MAP) != 0u,
    );

    // == GLSL's `#if !OPT_IsModulated TotalColor = TotalColor *
    // LightColor; #endif`
    if (draw_flags & DRAW_FLAG_MODULATED) == 0u {
        total_color = total_color * light_color;
    }

    // == GLSL's `TotalColor += FogColor;`
    total_color = total_color + fog_color;

    // == GLSL's `TotalColor.a *= GetDiffuseInfo(DrawID).w;` (HP2 passes
    // per-surface alpha separately from the polygon flags).
    total_color.a = total_color.a * params.diffuse_info.w;

    return total_color;
}
