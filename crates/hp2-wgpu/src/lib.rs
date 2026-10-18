//! Rust port of the renderer, replacing `ThirdParty/XOpenGLDrv`/
//! `ThirdParty/UT99VulkanDrv` with one `wgpu`-backed render device.
//! Phase 3.2 of `local://rust-inplace-port-plan.md`.
//!
//! **Scope of this crate today: adapter/device creation, macOS
//! window-surface creation from a raw `NSView*`, and a real per-tick
//! configure/acquire/clear/present loop (`lock_frame`/`unlock_frame`),
//! all wired into the real `UWgpuRenderDevice::Init`/`Lock`/`Unlock`/
//! `Exit`.** Wired to `hp2-ffi` behind the `renderer-wgpu` feature,
//! consumed by the real `HarryPotter2/Unreal/WgpuDrv` C++ package
//! (`UWgpuRenderDevice : URenderDevice`, registered as
//! `WgpuDrv.WgpuRenderDevice`, gated behind the
//! `HP2_ENABLE_WGPU_DRIVER` CMake option, default OFF).
//!
//! `create_device_sync` shares one `Instance` between surface and
//! adapter creation, as wgpu's `compatible_surface` check requires (an
//! earlier version built two separate, unrelated `Instance`s for the
//! headless device and the standalone surface-creation test, which
//! could never have been wired together correctly -- caught and
//! redesigned before anything downstream depended on it, see
//! `CreatedDevice`/`create_device_sync`'s own doc for the detail), and
//! validates `surface.get_capabilities(&adapter).formats` is
//! non-empty before returning a device at all -- an empty list is
//! wgpu's own documented signal that the surface is incompatible with
//! the adapter, so this is a real failure, not a cosmetic check (an
//! earlier version only checked `surface.is_some()` in one caller's
//! own test, which was vacuous -- caught on review, fixed in the
//! production function itself, not just the test).
//!
//! `lock_frame`/`unlock_frame` prove the stronger claim the capability
//! check alone could not: `surface.configure` plus `get_current_
//! texture`/`Queue::present` actually succeeding on a real window, not
//! just the surface being reported compatible. `Tests/
//! WgpuSurfaceTests.cpp` exercises the real `hp2_rs_wgpu_lock`/
//! `hp2_rs_wgpu_unlock` pair against a real SDL window and passes
//! reliably on this project's own dev machine, confirmed over several
//! repeated runs; getting there needed several precautions
//! (`SDL_HINT_MAC_BACKGROUND_APP`, `SDL_RaiseWindow`,
//! `SDL_WINDOW_ALWAYS_ON_TOP`, and polling the lock call itself for up
//! to ~2 seconds) documented in detail in that file. Which single
//! element of that combination is actually necessary was not isolated
//! (no ablation study was run); a *persistent* `Occluded` outcome
//! after that polling window is treated as an environment gap (ctest
//! `SKIP_RETURN_CODE 77` on that one check) rather than a hard
//! failure, but every run so far in this project's own dev environment
//! has passed for real, not skipped.
//!
//! That same test-harness pass does **not** mean the real game
//! presents frames, and this doc does not claim otherwise: a real
//! `hp2_game -window -testticks=60` launch (`Build/run_wgpu_smoke.py`)
//! on the same machine got `FrameOutcome::Occluded` on all 60 of 60
//! real per-tick `Lock` calls -- zero frames ever actually reached the
//! screen in that run, confirmed via the real cumulative tally
//! `Lock` itself logs every call (`WgpuDrv: Lock outcome=... tally
//! presented=... occluded=...`), not inferred from anything indirect.
//! The most likely difference from the passing test-harness case is
//! that a real `hp2_game -window` launch applies none of the
//! precautions above and makes only one attempt per tick (deliberately
//! -- blocking the real game loop for up to ~2 seconds on every single
//! frame to work around this would be a bad trade even if it helped,
//! since a frame failing to acquire is already an expected, transient
//! condition in real usage that the next tick's own `Lock` call
//! retries for free), but this was not isolated either.
//! `run_wgpu_smoke.py` reports this run as `blocked`, not `pass` --
//! deliberately not suppressed via ctest's `SKIP_RETURN_CODE` either,
//! since that would hide a real, current gap behind a green summary
//! (an earlier version of this change did exactly that to keep a
//! build directory's ctest count looking stable, caught on review as
//! the wrong tradeoff before it was committed).
//!
//! `request_adapter_sync`/`create_device_sync`'s own `#[ignore]`d
//! tests (CI runners have no GPU, see their own doc) cover the
//! headless half at the Rust level directly.
//!
//! Still open, not started: resize handling beyond what `lock_frame`'s
//! own per-call drawable-size query already gets for free, all WGSL
//! shader transcription, real draw-call forwarding for any of
//! `URenderDevice`'s pure virtuals (`lock_frame` presents an empty,
//! cleared frame, nothing drawn yet), and -- the open question this
//! doc's own "zero frames presented in a real launch" paragraph above
//! leaves unresolved -- why a real, non-test-harness launch behaves
//! differently from the dedicated ctest on the same machine.
//!
//! Shader transcription scoping, from reading the real GLSL sources
//! (not assumed): the plan's own "hand-transcribe each GLSL source to
//! WGSL" undersells the actual work. `ThirdParty/XOpenGLDrv/Src/
//! ShaderProgram.cpp`'s `#extension GL_ARB_bindless_texture : require`
//! and `DrawTile_GLSL.cpp`'s `DrawTileCoreProgram::BuildGeometryShader`
//! (130 lines) both use real GL features with no WGSL/WebGPU
//! equivalent at all (WebGPU has no bindless-handle model, and
//! deliberately excludes geometry shaders from its pipeline stages).
//! A literal line-by-line transcription of the "Core" GL 4.x shader
//! variants cannot work. Two findings change the actual plan:
//!
//! 1. Bindless textures are not load-bearing -- `CheckExtensions.cpp`
//!    runtime-detects support and disables `UseBindlessTextures`
//!    whenever the extension is missing, *and* unconditionally on AMD
//!    GPUs regardless of support ("AMD drivers don't like how we use
//!    bindless textures and perform very poorly"). The fallback this
//!    already exercises in production is a complete, working, fixed-
//!    texture-unit path (`ShaderProgram.cpp`'s `#define TMUDiffuse
//!    Texture<N>` aliases) that maps directly onto WebGPU's ordinary
//!    bind-group model -- one texture binding per slot, no handle
//!    indirection. Port from this path, not the bindless one.
//!    `UseSSBOParametersBuffer` (`DrawTile.cpp`) has the identical
//!    shape: defaults `false`, a plain per-draw UBO in the fallback,
//!    mapping directly onto a WebGPU uniform-buffer bind group with a
//!    dynamic offset per draw call instead of `gl_DrawID`-indexed SSBO
//!    lookups.
//! 2. The two geometry shaders are two genuinely different patterns,
//!    not one -- `DrawComplex_GLSL.cpp`/`DrawSimple_GLSL.cpp` have
//!    neither a geometry shader nor an ES/Core split, confirmed by
//!    grepping all four files, not assumed.
//!    `DrawGouraudProgram::BuildGeometryShader` does *not* amplify
//!    topology -- `layout(triangles) in; layout(triangle_strip,
//!    max_vertices = 3) out;`, one triangle in, one out, read in full.
//!    It does exactly three things, each with a direct vertex/
//!    fragment-only replacement: (a) broadcast the first vertex's
//!    `DrawID` to all three vertices as a flat-interpolated value --
//!    WGSL's `@interpolate(flat)` on a vertex output does this with no
//!    geometry stage; (b) under `OPT_BumpMaps`, compute a per-triangle
//!    tangent/bitangent basis from all three vertices' positions and
//!    UVs -- the standard modern replacement is either precomputing
//!    tangents per-vertex at mesh-load time (most engines' actual
//!    practice) or computing them in the fragment shader from screen-
//!    space derivatives (WGSL has `dpdx`/`dpdy`, the same primitive);
//!    (c) under `OPT_ClipDistance`, set a per-vertex `gl_ClipDistance`
//!    against a custom clip plane -- WGSL/WebGPU has no clip-distance
//!    equivalent at all; the standard replacement is an explicit
//!    `discard` in the fragment shader against the same plane
//!    equation.
//!
//!    `DrawTileCoreProgram::BuildGeometryShader` is the opposite
//!    pattern -- confirmed by reading its full body, not inferred from
//!    `DrawTileESProgram` lacking one, which turned out to be the
//!    wrong inference to make without checking (an earlier version of
//!    this doc guessed `DrawTileCoreProgram` matched `DrawGouraud`'s
//!    shape; it does not). It genuinely amplifies topology:
//!    `layout(triangles) in; layout(triangle_strip, max_vertices = 6)
//!    out;`, and only ever reads vertex 0 of the input "triangle"
//!    (`In[0].TexCoords`/`gl_in[0].gl_Position`/etc. -- vertices 1 and
//!    2 are never referenced). This is GPU-side quad expansion from a
//!    single point: the C++ side uploads one vertex per tile carrying
//!    the whole quad's parameters (screen-space scale/offset, UV rect,
//!    width/height), and the geometry shader expands it into six
//!    vertices (two triangles) using that one vertex's data. The
//!    direct, idiomatic WebGPU replacement is vertex-shader quad
//!    expansion from `@builtin(vertex_index)`: draw 6 vertices per
//!    tile instance (one instance per tile, instance-rate attributes
//!    or a fixed-size uniform array carrying today's per-tile
//!    parameters -- not a storage buffer, per the DrawID-indexing
//!    correction elsewhere in this doc: WebGL2/GLES3.0 has zero
//!    storage buffers available),
//!    and branch on `vertex_index % 6` in the vertex shader to compute
//!    which of the quad's six corners this invocation is -- the exact
//!    same per-corner math already in this geometry shader, just moved
//!    up one stage. A common, well-established technique for sprite/
//!    particle/UI rendering on hardware with no geometry shaders at
//!    all (mobile GPUs, WebGPU itself), not a novel design needed for
//!    this port specifically.
//!
//! Net effect, not uniform across all five shaders: `DrawTile` has a
//! real, working, geometry-shader-free alternative already running in
//! production (`DrawTileESProgram`) to copy the vertex-submission shape
//! from (per-vertex `Coords`/`TexCoords` inputs, not a single
//! point-per-tile). This crate's own `tile.wgsl` (second real shader
//! port) already resolved the per-corner-expansion question this
//! paragraph originally framed as still open between two options --
//! not a live "either/or" anymore: it copies the ES path's shape
//! directly, i.e. the CPU-side expansion `DrawTile.cpp`'s
//! `SubmitTileBatch` already does for that path today, not the
//! `vertex_index`-based GPU-side alternative sketched above (which
//! remains a real, valid *technique* in general, just not the one this
//! crate ended up using, since a working CPU-side alternative already
//! existed to copy instead of designing a new GPU-side one).
//!
//! `DrawGouraud`'s geometry shader is not a single-path thing to design
//! a replacement for from scratch, the way an earlier version of this
//! paragraph wrongly claimed (caught on review, corrected after
//! confirming `DrawGouraud.cpp:376`'s real `GeoShaderFunc =
//! RenDev->UsingGeometryShaders ? &BuildGeometryShader : nullptr; //
//! optional`): it is genuinely optional, gated by a runtime flag, and
//! `DrawGouraudProgram`'s own vertex shader already has a complete,
//! already-in-production `#else` branch for when it is off
//! (`DrawGouraud_GLSL.cpp` lines 100-118) that does the final
//! `modelviewprojMat` transform directly, no geometry stage involved
//! -- `gouraud.wgsl` (this crate's third real shader port) already
//! transcribes exactly that branch, not a freshly-designed
//! replacement. The geometry shader's `OPT_BumpMaps && OPT_HasBumpMap`
//! per-triangle tangent/bitangent computation is its one genuinely
//! undesigned piece when it does run -- but that is not the same thing
//! as bump mapping overall being undesigned: the same
//! `!OPT_GeometryShaders` branch already carries its own placeholder
//! vertex-shader-only TBN basis (`T = B = vec3(1,1,1)`, both literally
//! commented "Arbitrary" in the real source, `N =
//! normalize(Normals.xyz)`), a direct, already-in-production
//! transcription target `gouraud.wgsl`'s own header now scopes
//! correctly -- lower fidelity than the geometry shader's real
//! per-triangle tangents, but not undesigned. `gouraud.wgsl`'s base
//! path still correctly defers implementing it at all (see that file's
//! own header) rather than needing to solve either version to port the
//! base, non-bump-mapped path.
//! For bindless textures and SSBO draw parameters specifically, the
//! non-bindless/non-SSBO fallback branches (not a separate `*ESProgram`
//! class, just an `#if`/runtime-flag branch within the same shader
//! source) are real alternatives already exercised in production
//! (disabled on AMD GPUs today) and are the right transcription
//! source regardless of which shader.
//!
//! `DrawSimple_GLSL.cpp` has been read in full and ported:
//! `shaders/simple.wgsl`. `DrawComplex_GLSL.cpp` (BSP surface
//! rendering, 732 lines, by far the largest of the five) has now
//! actually been read in full -- an earlier version of this doc
//! claimed that after reading only its first 300 lines (vertex shader
//! plus parallax helpers), caught on review before the fragment
//! shader (301-732) was ever read. One load-bearing finding holds up: it has
//! no geometry shader at all, and its bump-map tangent space
//! (`vTBNMat`) is computed directly in the vertex shader from the
//! surface's own known `XAxis`/`YAxis`/`ZAxis` UV mapping axes -- not
//! derived from adjacent-triangle geometry the way a typical mesh
//! normal/tangent would be -- so it needs no geometry-shader
//! replacement design at all, unlike `DrawGouraud`. One real WGSL
//! translation detail for that same tangent space: `flat in mat3
//! vTBNMat` has no direct WGSL equivalent -- inter-stage IO values
//! must be scalars or vectors, not matrices -- so the port needs to
//! split it into three separate `@location(n) @interpolate(flat)
//! vec3<f32>` varyings (the matrix's three columns) and reassemble
//! `mat3x3<f32>` from them in `fs_main`; those three extra locations
//! count against WGSL's 16-location inter-stage-variable limit
//! alongside every optional UV varying this shader has, so the real
//! per-specialization location budget needs counting once all of
//! `DrawComplex`'s optional outputs are actually laid out, not just
//! assumed to fit. Its real complexity is breadth, not architecture:
//! lightmaps, fogmaps, detail/macro textures (both with real HSV
//! recoloring, not plain multiply), environment maps, height maps,
//! three alternate parallax-mapping techniques (basic/occlusion/relief,
//! the latter two using real texture-sampling loops), and -- only
//! visible once the fragment body itself was actually read -- a real
//! per-pixel hardware point-light loop used twice (`OPT_HWLighting`
//! base lighting, and a second pass inside `OPT_BumpMaps` for
//! tangent-space diffuse+specular), distance fog mixing, and an
//! `OPT_Editor`-gated block of debug render modes (zone/poly-cut/
//! walkable-surface visualization, hit-test color override) that is
//! editor-only, not needed for core gameplay rendering. All `#if`-gated
//! optional branches of one vertex+fragment pair, each individually
//! portable with the same non-bindless-fallback strategy as every other
//! shader here -- except the occlusion/relief parallax loops
//! specifically need one more real change: both call `GetTexel`
//! (implicit-derivative texture sampling) from inside a loop whose exit
//! condition depends on a per-fragment height value read from that same
//! texture, which is non-uniform control flow -- WGSL's spec restricts
//! implicit-derivative texture sampling (`textureSample`) to uniform
//! control flow in exactly this situation, so the port should use
//! `textureSampleGrad` with derivatives (`dpdx`/`dpdy` of the base UV)
//! computed once before entering the loop, or `textureSampleLevel`,
//! instead. This crate's own `naga_accepts_texture_sample_in_non_
//! uniform_control_flow` test settles, empirically, a real surprise
//! here: naga's validator does *not* reject the minimal repro of this
//! exact shape -- it does not implement (or does not apply) WGSL's
//! full uniformity-analysis restriction. `every_wgsl_shader_parses_
//! and_validates`'s naga-based CI check therefore cannot be relied on
//! to catch a parallax loop ported with plain `textureSample` left in
//! by mistake; a real backend might still reject it or silently
//! produce wrong derivatives, but naga accepting it is not evidence of
//! correctness either way. The `textureSampleGrad`/`textureSampleLevel`
//! requirement above still stands on the WGSL spec's own written text,
//! independent of naga's enforcement gap -- it changes mipmap
//! filtering behavior slightly versus the GLSL original, a real, not
//! cosmetic, porting change for those two techniques
//! specifically, not the basic parallax technique (no loop) or any
//! other `#if`-gated branch in this file. None of them have been
//! transcribed yet. The file also has a second genuinely-dead `#if 0`
//! block (an unused parallax soft-shadow function, lines 673-732) --
//! confirmed dead the same way `DrawSimple_GLSL.cpp`'s stipple block
//! is, and likewise not a transcription target.
//!
//! `SetTexture.cpp` (747 lines, texture cache/upload) has been read in
//! enough depth to scope its shape, though not line-for-line the way
//! the shader sources above were: textures are keyed by `CacheID`
//! (`FTextureInfo::CacheID`, with a reused low tag bit distinguishing
//! the alpha-premultiplied-for-`PF_Masked` copy of a `TEXF_P8` texture
//! from its unmasked copy -- two real cache entries can exist for one
//! source texture). Upload is one of: a full upload
//! (`glTexImage2D`/`glCompressedTexImage2D` for the one compressed
//! format in use, `TEXF_DXT1`, followed by `glGenerateMipmap`) or an
//! incremental/partial mip upload (`glTexSubImage2D`/
//! `glCompressedTexImage2D` at an explicit, manually incremented mip
//! level, no mipmap generation). `TEXF_DXT1` maps to wgpu's
//! `Features::TEXTURE_COMPRESSION_BC` (not asserted anywhere yet in
//! this crate); the other formats (`P8`/`RGBA7`/`RGB16`/`RGB8`/
//! `RGBA8`) upload uncompressed and would map to `Queue::write_texture`
//! with a wgpu-side format chosen per source format -- `P8` in
//! particular is an indexed-palette format needing CPU-side expansion
//! to RGBA before upload (the real palette-to-RGBA conversion call
//! site itself has not been read yet). `glGenerateMipmap` itself has
//! no wgpu equivalent at all -- wgpu never generates mipmaps for you,
//! full stop -- so a real Rust port needs its own mip-generation step:
//! a render/blit downsample pass per level for uncompressed formats,
//! but that path does not exist for `TEXF_DXT1`/BC specifically, since
//! BC-compressed textures cannot be render or storage targets in
//! wgpu -- the options there are a CPU decode-downsample-re-encode
//! pass, uploading only the mips the asset already ships
//! (`Info.NumMips`, skipping GPU mip generation for compressed
//! textures entirely), or expanding `TEXF_DXT1` to RGBA8 on upload.
//! Not yet decided which.
//!
//! `DrawComplex.cpp` (304 lines), `DrawGouraud.cpp` (406 lines, also
//! home to `DrawTriangles`), and `DrawTile.cpp` (379 lines) -- the C++
//! orchestration that actually issues draw calls, all currently no-op
//! stubs in `WgpuDrv.cpp` -- have now been read for their `RenDev`
//! interface entry points, shared private helpers (`SetTextureHelper`
//! in both `DrawComplex.cpp` and `DrawGouraud.cpp`, `BufferVert`,
//! `DrawNativeTextTile`, `FlushNativeTextTileBatch`), and
//! `SubmitTileBatch` in full; an earlier version of this doc claimed
//! all three "read in full", which overclaimed past what was actually
//! read at the time (those helper bodies, caught on review) -- each
//! shader class's own constructor/`CreateInputLayout`/
//! `Activate`/`DeactivateShader` bodies in these three files remain
//! genuinely unread, stated here rather than implied. All three share
//! one real batching architecture that is good news for porting:
//! CPU-side ring buffers (`VertBuffer`/`DrawBuffer`/
//! `ParametersBuffer`) accumulate vertices and one small per-draw-call
//! parameter struct across multiple draw calls, flushed only when
//! blend state or shader specialization actually changes or a buffer
//! fills up -- not one GL draw call per engine draw call. Each
//! buffered draw call gets a `DrawID`, written per-vertex and used
//! shader-side to index back into that draw call's own parameter slot
//! (`GetXAxis(DrawID)` etc. in `DrawComplex_GLSL.cpp`). `simple.wgsl`
//! originally dropped this indexing on an unverified claim that the
//! non-SSBO fallback only ever binds one `DrawColor` at a time --
//! caught on review, then disproven by reading
//! `ShaderProgram.cpp`'s `EmitDrawCallParametersHeader`: the UBO path
//! emits a bounded *array* (`DrawCallParameters
//! Draw<Name>Params[ParametersBufferSize]`) and generates
//! `Get<Field>(uint DrawID)` accessors indexing it by `DrawID`
//! identically to the SSBO path. `simple.wgsl` now carries `draw_id`
//! through and indexes a fixed-size `var<uniform>
//! array<DrawParameters, MAX_DRAWS>` -- not a storage buffer: an
//! earlier version of this paragraph said storage buffers are "core
//! and unconditionally available on every backend and web tier",
//! which is what `simple.wgsl` itself originally shipped with too;
//! both wrong, caught on review and disproven by reading
//! `wgpu-types`'s own `Limits::downlevel_webgl2_defaults()`
//! (`max_storage_buffers_per_shader_stage: 0`,
//! `max_storage_buffer_binding_size: 0`) -- genuinely zero storage
//! buffers on WebGL2/GLES3.0, matching this crate's own
//! `BackendChoice::Gl` and the plan's later Phase 8 web target. A
//! fixed-size uniform array is the real portable equivalent of GLSL's
//! UBO fallback this shader already transcribes from.
//! `DrawComplex`/`DrawGouraud`/`DrawTile`'s eventual WGSL ports need
//! this uniform-array shape too, not a storage buffer and not the
//! version this file originally shipped with. Triangulation is
//! already CPU-side for both `DrawComplex`
//! (`FSavedPoly` n-gon fan triangulated into `(NumPts-2)*3` vertices)
//! and `DrawGouraud`/`DrawTriangles` (same fan triangulation, plus
//! real index-buffer support in `DrawTriangles`) -- no GPU-side
//! amplification to design around for either. `DrawGouraud`'s
//! `BufferVert` helper also confirms a real architectural distinction
//! from `DrawComplex`: per-vertex `LightColor`/`FogColor` are carried
//! directly on each vertex (true Gouraud shading, interpolated by the
//! rasterizer) rather than sampled from a lightmap/fogmap texture in
//! the fragment shader the way `DrawComplex` does. `DrawTile.cpp`'s
//! `SubmitTileBatch` confirms and sharpens the earlier
//! `DrawTileCoreProgram`-vs-`DrawTileESProgram` finding: the Core path
//! packs an entire tile's rect and UVs into a single vertex's
//! attributes and advances the ring buffer by 3 with only that one
//! vertex actually written, relying on its geometry shader for the
//! real 1-to-6 expansion before rasterization; the ES path's C++
//! computes all 6 corner vertices directly, by hand, right there in
//! `SubmitTileBatch` (`Out[0..5].Coords`/`TexCoords`, using
//! `RFX2`/`RFY2`/`Frame->FX2`/`Frame->FY2` the same way the vertex
//! shader itself would) -- for `DrawTile` this is no longer a
//! geometry-shader-replacement design problem, it is a direct
//! transcription target. `DrawNativeTextTile` -- the native-text
//! rendering path, `default_enabled: true` per AGENTS.md #4 and
//! distinct from the `NativeText=False` bitmap-font path AGENTS.md #5
//! requires stay reachable -- is not a separate rendering path at all
//! -- it is a thin wrapper that binds an externally-supplied GL
//! texture/sampler (from the CoreText/FreeType backend, bypassing the
//! normal `FTextureInfo` cache) and then calls the exact same
//! `SubmitTileBatch` regular `DrawTile` uses; `FlushNativeTextTileBatch`
//! just forwards to the same `Shader->Flush`. A real Rust port's
//! native-text path needs no separate design from `DrawTile`'s, only a
//! way to bind an already-created wgpu texture/sampler instead of
//! going through the texture cache.
//!
//! One real architectural gap all three orchestration files surface,
//! now actually designed (not left open): GLSL's `#if`-gated shader
//! specialization (`ShaderCompilationOptions`, lazily compiled and
//! cached per unique combination of texture layers and blend mode via
//! `SelectSpecialization`) has no direct WGSL/wgpu equivalent -- wgpu
//! has no API-level preprocessor. An earlier version of this doc
//! concluded the port needs to replicate that whole mechanism, either
//! as a `HashMap<ShaderCompilationOptions, wgpu::RenderPipeline>` keyed
//! by the full options bitmask, or (after rejecting pipeline-
//! overridable constants as unable to avoid populating every optional
//! binding anyway) by generating N actual WGSL source-text variants.
//! Both wrong, caught on review after actually reading why GLSL needs
//! this at all, not just that it exists: `DrawComplex.cpp` calls
//! `SetBlend(NextPolyFlags)` as a plain, separate GL state change
//! *before* `SelectSpecialization` runs, not something the shader
//! specialization encodes -- confirming the one genuinely pipeline-
//! level axis (opaque/alpha-blended/masked blend state, which wgpu
//! bakes into `ColorTargetState::blend` with no runtime-settable
//! equivalent) is already structurally separate from everything else
//! `ShaderCompilationOptions` varies. Of what remains -- optional
//! texture layers (lightmap/fogmap/detail/macro/bump/environment/
//! height) and simple per-draw boolean flags (masked/alpha-blended/
//! modulated/translucent/unlit/render-fog) -- none require a different
//! bind-group or vertex-buffer layout: `DrawComplex_GLSL.cpp`'s bump-
//! mapping tangent basis (`vTBNMat`) is computed in the vertex shader
//! from the surface's own known `XAxis`/`YAxis`/`ZAxis`, not read from
//! a dedicated bound vertex attribute, so no shader needs a per-option
//! vertex layout; and this crate's own `complex.wgsl` already proves,
//! tested on real hardware, that an optional texture layer (lightmap,
//! fogmap) can instead always be bound (a 1x1 dummy when the real
//! layer is absent for a given draw) and conditionally sampled at
//! runtime with `select()`, keyed off the same per-DrawID `DrawFlags`
//! bits GLSL's per-draw-call signature computation already reads. The
//! real design, then: one WGSL "uber-shader" module per draw-call type
//! (already this crate's shape for `complex.wgsl`/`gouraud.wgsl`/
//! `tile.wgsl`), extending that same proven dummy-texture-plus-
//! `select()` pattern to the remaining optional texture layers instead
//! of introducing permutation-cache machinery to avoid building at
//! all; plus a small, separate `HashMap<BlendMode, wgpu::
//! RenderPipeline>` per draw-call-type shader for the actual pipeline-
//! level blend-state axis. `BlendMode` is not a simple 3-4-value enum
//! (`Opaque`/`AlphaBlended`/`Masked`) as an earlier version of this
//! paragraph claimed, caught on review after actually reading
//! `SetTexture.cpp`'s real `SetBlend` in full rather than guessing
//! its shape from `PolyFlags`' own bit names: it is seven distinct
//! `glBlendFunc` combinations (Lumos -- `PF_LumosAffected`'s full
//! mask, HP2-specific, driven by the renderer's own computed `cAlpha`
//! distance fade; Invisible; Highlighted, two sub-variants gated by
//! `PF_Translucent`; Translucent, two sub-variants gated by
//! `SimulateMultiPass` -- the dual-source one needs `wgpu::Features::
//! DUAL_SOURCE_BLENDING` plus WGSL's `enable dual_source_blending`/
//! `@blend_src(1)`, unavailable on WebGL2, so not the base path;
//! Modulated; Opaque, the fallback), plus two more axes `SetBlend`
//! itself also changes in the same real function (`glColorMask` for
//! `PF_Invisible`, `glDepthMask` for `PF_Occlude`) and `SetZTestMode`
//! changes separately (7 depth-compare modes), plus a `HitTesting()`
//! override that forces `(One, Zero)` unconditionally. The real
//! pipeline-cache key is this derived blend/depth-write/depth-compare
//! state, not raw `PolyFlags` or `ShaderCompilationOptions`' `OPT_Is*`
//! bits directly -- `Masked` itself is not a blend mode at all in the
//! real source (it is a fragment-shader `discard` plus ordinary
//! opaque blending, already correctly modeled as a per-pixel branch
//! in WGSL, not a pipeline-level axis). This also means the earlier
//! "per-specialization" WGSL inter-stage-location budget (`DrawComplex`
//! paragraph above) is moot under this design: there is only one real
//! varying layout per shader module, not one per specialization, so
//! every optional varying just needs to fit WGSL's 16-location limit
//! once, globally, for that shader. Not yet implemented: this crate
//! currently has no `BlendMode` enum, no per-draw-call-type pipeline
//! cache, and `complex.wgsl`/`gouraud.wgsl` do not yet carry
//! detail/macro/bump/environment/height textures at all (both still
//! base-path only, as their own headers already say). Confirmed in
//! passing: `DrawGouraud.cpp`'s real `MaxVertices()` also returns a
//! hardcoded `256`, matching `WgpuDrv.cpp`'s current stub value -- not
//! a shortcut, already correct. Still unexamined: the ring-buffer/
//! `ShaderProgram::Flush` machinery itself (`ShaderProgram.cpp`, read
//! only for its bindless/SSBO-fallback logic so far, not its buffering
//! internals), the shader class constructors/`CreateInputLayout`
//! bodies named above, and how any of this would connect to
//! `WgpuDrv.cpp`'s current no-op `DrawComplexSurface`/`DrawTriangles`/
//! `DrawTile`/`Draw2DLine`/`Draw2DPoint` stubs.
//!
//! Depth-range convention (GLSL/OpenGL clip-space Z is -1..1; WebGPU's
//! is 0..1): resolved in `compute_frame_uniforms` (`glam::camera::
//! rh::proj::directx`, 0..1, right-handed, Y-up -- see that
//! function's own doc for the full derivation) -- an earlier version
//! of this paragraph said "not yet resolved in code, since no Rust
//! code builds a real projection matrix yet", true when written, now
//! stale. `simple.wgsl`'s own header comment still states the
//! per-shader usage contract this satisfies (whatever populates
//! `modelviewproj_mat` must build it directly in WebGPU's 0..1
//! convention), not duplicated here.

pub mod capture;
pub mod draw;
pub mod texture;

/// Backend selection, matching the plan's `[WgpuDrv.WgpuRenderDevice]
/// Backend=Auto|Metal|Vulkan|Dx12|Gl` ini key -- `from_ffi_discriminant`
/// is this enum's C++ side: `WgpuDrv.cpp`'s `StaticConstructor`
/// registers a `UEnum` with members in exactly this order (`Auto`,
/// `Metal`, `Vulkan`, `Dx12`, `Gl`), and UE1's `UByteProperty` stores an
/// enum-backed config property as the member's plain index -- `Backend`
/// (the C++ `BYTE` field) is 0 for `Auto`, 1 for `Metal`, and so on by
/// construction, not a value this crate invents a mapping for
/// independently of what the UE1 side actually does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    Auto,
    Metal,
    Vulkan,
    Dx12,
    Gl,
}

impl BackendChoice {
    /// Matches the plan's own "Device creation" bullet exactly:
    /// `wgpu::Instance with Backends::from_env().unwrap_or(Backends::
    /// PRIMARY | Backends::GL)`. Only the `Auto` case consults the
    /// `WGPU_BACKEND` environment variable (confirmed real, reads that
    /// exact variable: `wgpu-types-30.0.1/src/backend.rs`'s own
    /// `Backends::from_env`) -- the explicit `Metal`/`Vulkan`/`Dx12`/
    /// `Gl` choices are themselves the plan's ini-key-driven way to
    /// pin one backend ("the ini key ... restricts the backend set",
    /// the plan's own next sentence), and letting an environment
    /// variable silently override an explicit ini choice would defeat
    /// that restriction's whole purpose -- `WGPU_BACKEND` is a
    /// developer/CI override for the *default* case only, not a
    /// universal one.
    fn to_wgpu(self) -> wgpu::Backends {
        match self {
            BackendChoice::Auto => {
                wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY | wgpu::Backends::GL)
            }
            BackendChoice::Metal => wgpu::Backends::METAL,
            BackendChoice::Vulkan => wgpu::Backends::VULKAN,
            BackendChoice::Dx12 => wgpu::Backends::DX12,
            BackendChoice::Gl => wgpu::Backends::GL,
        }
    }

    /// `WgpuDrv.cpp`'s real `UEnum` member order (see this type's own
    /// doc). Values outside 0..=4 (a config file hand-edited to an
    /// invalid value, or a stale ini from a future version with more
    /// members) fall back to `Auto` rather than panicking across FFI --
    /// logged by the caller (`hp2_rs_wgpu_create`), not silently here,
    /// so the real invalid value reaches the log, not just "it fell
    /// back to Auto".
    pub fn from_ffi_discriminant(value: u32) -> Option<Self> {
        match value {
            0 => Some(BackendChoice::Auto),
            1 => Some(BackendChoice::Metal),
            2 => Some(BackendChoice::Vulkan),
            3 => Some(BackendChoice::Dx12),
            4 => Some(BackendChoice::Gl),
            _ => None,
        }
    }
}

/// Human-readable backend name for the `WgpuDrv: adapter "<name>"
/// backend <X>` log line the plan specifies (`Phase 3.2`'s "Device
/// creation" bullet), matching the plan's own `Metal|Vulkan|Dx12|Gl|
/// BrowserWebGpu` vocabulary exactly.
pub fn backend_name(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::Metal => "Metal",
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Dx12 => "Dx12",
        wgpu::Backend::Gl => "Gl",
        wgpu::Backend::BrowserWebGpu => "BrowserWebGpu",
        wgpu::Backend::Noop => "Noop",
    }
}

/// Shared `Instance` construction: the chosen backend set. Used both
/// for headless adapter/device creation (`request_adapter_sync`/
/// `create_device_sync` below) and, on macOS, for window-surface
/// creation (`macos::create_surface_from_ns_view`) -- the same
/// `Instance` must be shared between the two for `request_adapter`'s
/// `compatible_surface` check to mean anything once that integration
/// lands (not yet; see this module's own doc for what's still open).
/// Public so callers needing that integration can construct one
/// instance and pass it to both halves themselves.
pub fn build_instance(choice: BackendChoice) -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: choice.to_wgpu(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

/// Function-pointer shorthand for the "no surface" case when calling
/// `create_device_sync` headlessly -- `None::<HeadlessSurfaceCreator>`
/// instead of spelling out the full closure bound every call site
/// needs (clippy's `type_complexity` lint flagged the inline version).
pub type HeadlessSurfaceCreator =
    fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>;

/// Shared adapter request: `PowerPreference::HighPerformance`.
/// `compatible_surface` must come from the *same* `Instance` passed
/// here -- wgpu's adapter/surface compatibility is instance-scoped,
/// so a surface built from a different `Instance` would make this
/// check meaningless (a real defect an earlier version of this
/// module had: `create_device_sync` built and dropped its own
/// throwaway `Instance` internally, and the standalone surface-
/// creation test function built a second, unrelated one -- neither
/// could ever have been wired together correctly without this
/// rework, caught on review before anything downstream depended on
/// it).
fn request_adapter_raw(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Option<wgpu::Adapter> {
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface,
        ..Default::default()
    }))
    .ok()
}

/// Replaces the adapter-request half of `WgpuDrv`'s future `Init`
/// (plan's "Device creation" bullet). Returns the adapter's name and
/// backend on success, or `None` on total failure (the real C++
/// forwarding function will turn that into the plan's `appErrorf(
/// TEXT("WgpuDrv: no compatible GPU adapter (tried %s)"), ...)` once it
/// exists). Headless (no surface) -- `create_device_sync` is the one
/// that supports an optional surface.
pub fn request_adapter_sync(choice: BackendChoice) -> Option<(String, wgpu::Backend)> {
    let adapter = request_adapter_raw(&build_instance(choice), None)?;
    let info = adapter.get_info();
    Some((info.name, info.backend))
}

/// Result of `create_device_sync`. A plain struct instead of a growing
/// tuple: positional fields were getting hard to read correctly at
/// call sites. Holds the `Instance`/`Adapter` (and `Surface`, if one
/// was requested) alongside the `Device`/`Queue` so the whole group
/// stays alive and drops together -- dropping the `Instance` early
/// while the `Device`/`Surface` are still in use would be a use-
/// after-free at the API boundary wgpu cannot catch for us. Field
/// order is drop order (Rust drops struct fields in declaration
/// order): `surface` first, `instance` last, so the root owner of
/// everything outlives every resource built from it.
pub struct CreatedDevice {
    /// `Some` only if `create_device_sync` was given a
    /// `create_surface` closure that succeeded. Caller-owned window
    /// resources behind the surface (e.g. the `SDL_MetalView` on
    /// macOS) must outlive this field -- drop `CreatedDevice` (or at
    /// least take and drop `surface` specifically) before destroying
    /// them.
    pub surface: Option<wgpu::Surface<'static>>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Kept (not dropped once `request_device` returns -- it takes
    /// `&self`, not `self`) so callers can query real
    /// surface/adapter compatibility themselves, e.g.
    /// `surface.get_capabilities(&adapter)`, rather than only trusting
    /// that `surface.is_some()` means something (it doesn't by
    /// itself: a failed `create_surface` closure already makes
    /// `create_device_sync` return `None` before this struct exists
    /// at all, so `surface.is_some()` alone is tautological once you
    /// have a `CreatedDevice` -- caught on review, see this crate's
    /// own tests for the real check).
    pub adapter: wgpu::Adapter,
    pub adapter_name: String,
    pub backend: wgpu::Backend,
    /// The adapter's own limits, which `required_limits` was set to --
    /// `device.limits()` is guaranteed equal to this (not re-queried
    /// here; returned so callers, including this module's own tests,
    /// can check that guarantee against the real API instead of taking
    /// it on faith).
    pub adapter_limits: wgpu::Limits,
    pub instance: wgpu::Instance,
}

/// Replaces the device/queue-request half of `WgpuDrv`'s future `Init`
/// (plan's "Device creation" bullet). `create_surface`, if given, is
/// called with the freshly-built `Instance` *before* the adapter is
/// requested, and the resulting `Surface` (if `Ok`) is passed as
/// `compatible_surface` to `request_adapter` -- wgpu's own
/// compatibility check is instance-scoped, so sharing one `Instance`
/// between surface and adapter creation (not two separate ones, an
/// earlier version of this function's own mistake, caught on review)
/// is what makes that check mean anything. `None` skips surface
/// creation entirely (headless, today's only real caller via
/// `hp2_rs_wgpu_create` until window-surface wiring into
/// `UWgpuRenderDevice::Init` lands).
///
/// `required_limits: adapter.limits()`, not `Limits::default()`:
/// `request_device` *panics* (not `Err`, confirmed in `wgpu-30.0.1/
/// src/api/adapter.rs`'s own doc comment) if the requested limits
/// exceed what the adapter actually supports, and `Limits::default()`
/// is a fixed WebGPU-spec baseline that a GL/GLES backend -- the
/// plan's own Raspberry Pi fallback -- can fall short of. Asking for
/// exactly `adapter.limits()` can never exceed what the adapter
/// reports, by construction, so this can never hit that panic.
/// Placeholder, not the final policy: `adapter.limits()` means
/// renderer code validated only on a high-end adapter (this machine's
/// M5 reports `max_texture_dimension_2d=32768`) can end up silently
/// depending on capability a lower-end backend doesn't have -- nothing
/// here catches that until it's actually run on one. Once the
/// renderer's real per-feature requirements are known, replace this
/// with a fixed, deliberately-chosen baseline (e.g.
/// `Limits::downlevel_webgl2_defaults()` raised to exactly the texture/
/// buffer sizes HP2 needs, via `.using_resolution(adapter.limits())`)
/// and fail with a clear error when an adapter can't meet it, rather
/// than silently taking whatever the adapter happens to offer. Returns
/// `None` on total failure (same real-C++-forwarding note as
/// `request_adapter_sync`).
/// Matches the plan's "on `None`, retry with `force_fallback_adapter:
/// false` and `Backends::GL` only" exactly: if `choice`'s own backend
/// set finds no adapter at all, and `choice` was not itself already
/// GL-only, this tries once more with a fresh `Instance` built from
/// `Backends::GL` alone before giving up for real. `force_fallback_
/// adapter` is already `false` by default on `RequestAdapterOptions`
/// (confirmed in `wgpu-types-30.0.1/src/adapter.rs`'s own struct
/// definition -- a plain `bool` field, `..Default::default()` already
/// used by `request_adapter_raw`), so nothing extra is needed there.
/// `create_surface` takes `Fn`, not `FnOnce`: the retry needs to call
/// it a second time against the second, GL-only `Instance` -- every
/// real caller's closure (e.g. `hp2_rs_wgpu_create`'s, capturing a
/// `Copy` raw `ns_view` pointer and forwarding it to `macos::
/// create_surface_from_ns_view`) is trivially re-callable, so this is
/// not a real constraint on callers, just a tighter bound than the
/// single-attempt version needed.
pub fn create_device_sync(
    choice: BackendChoice,
    create_surface: Option<
        impl Fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>,
    >,
) -> Option<CreatedDevice> {
    if let Some(created) = try_create_device_once(choice, create_surface.as_ref()) {
        return Some(created);
    }
    if choice == BackendChoice::Gl {
        return None;
    }
    try_create_device_once(BackendChoice::Gl, create_surface.as_ref())
}

fn try_create_device_once(
    choice: BackendChoice,
    create_surface: Option<
        &impl Fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>,
    >,
) -> Option<CreatedDevice> {
    let instance = build_instance(choice);
    let surface = match create_surface {
        Some(f) => Some(f(&instance).ok()?),
        None => None,
    };
    let adapter = request_adapter_raw(&instance, surface.as_ref())?;
    // A non-null surface that is actually incompatible with the
    // adapter (empty format list -- wgpu-types-30.0.1/src/surface.rs's
    // own doc comment on `SurfaceCapabilities::formats`: "Returns an
    // empty vector if the surface is incompatible with the adapter")
    // is as much a real failure as no adapter at all: a surface that
    // can never be configured is useless to the caller. Checked here,
    // not left for callers to discover later -- an earlier version of
    // this crate only checked `surface.is_some()` in one caller's own
    // test, which was vacuous (caught on review): this validates the
    // actual production path every caller goes through, not just a
    // test-only assertion.
    if let Some(surface) = &surface
        && surface.get_capabilities(&adapter).formats.is_empty()
    {
        return None;
    }
    let info = adapter.get_info();
    let adapter_limits = adapter.limits();
    // Real `SupportsS3TC`'s own graceful degradation (SetTexture.cpp:
    // 233: `if (!SupportsS3TC && IsHP2CompressedTexture(Info.Format))
    // UnsupportedTexture = true;`, not a hard failure) -- requesting
    // an unsupported feature would make request_device itself fail
    // outright, so this intersects with what the real adapter actually
    // reports rather than requiring it unconditionally. Checked again
    // at actual DXT1 upload time via `device.features()` (what was
    // actually granted, not just requested) -- `texture::texture_bytes`'s
    // own doc already has the real per-format byte-size formula this
    // compressed-format path depends on.
    let required_features = adapter
        .features()
        .intersection(wgpu::Features::TEXTURE_COMPRESSION_BC);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("hp2-wgpu device"),
        required_limits: adapter_limits.clone(),
        required_features,
        ..Default::default()
    }))
    .ok()?;
    Some(CreatedDevice {
        surface,
        device,
        queue,
        adapter,
        adapter_name: info.name,
        backend: info.backend,
        adapter_limits,
        instance,
    })
}

/// Specific result of `lock_frame`, not a bare bool: a bool would
/// hide exactly which step failed and which real
/// `wgpu::CurrentSurfaceTexture` variant was observed -- needed both
/// to diagnose failures (a bare bool hid the real cause of this
/// enum's own predecessor's first failing run, caught on review
/// before a comment that guessed at the cause, wrongly claiming it
/// was "confirmed empirically", ever landed) and so this crate's own
/// callers never have to guess either.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOutcome {
    /// Configured, acquired, and presented a frame.
    Presented = 0,
    /// `created.surface` was `None` (headless device).
    NoSurface = 1,
    /// `Surface::get_default_config` returned `None`: the surface has
    /// zero supported formats or present modes for this adapter.
    NoDefaultConfig = 2,
    /// `get_current_texture` returned `Timeout`.
    Timeout = 3,
    /// `get_current_texture` returned `Occluded` (window minimized or
    /// behind another window -- or, on macOS, possibly never actually
    /// mapped to the screen). Nothing is presented; with
    /// `HP2_CAPTURE_FRAMES` on, the frame is still rendered offscreen
    /// and captured (`FramePresentable::CaptureOnly`).
    Occluded = 4,
    /// `get_current_texture` returned `Outdated`.
    Outdated = 5,
    /// `get_current_texture` returned `Lost`.
    Lost = 6,
    /// `get_current_texture` returned `Validation`.
    Validation = 7,
    /// Not returned by `lock_frame` itself -- reserved for its FFI
    /// wrapper (`hp2-ffi`'s `hp2_rs_wgpu_lock`) to report a null
    /// `handle`, without reusing any of the above codes to mean
    /// something they do not.
    InternalError = 8,
    /// Not returned by `lock_frame` itself -- reserved for its FFI
    /// wrapper to pass as `guard`'s `on_panic` value, distinct from
    /// `InternalError`, so a caught panic (`hp2-ffi-support::guard`
    /// logs it via `log::error!`, which a standalone ctest binary may
    /// have no logger installed to show) is still distinguishable
    /// from a plain null-handle call in the returned code itself, not
    /// only in a log that might not be visible.
    Panicked = 9,
}

/// What `lock_frame`/`begin_offscreen_frame` actually acquired:
/// either a real `wgpu::SurfaceTexture` (the only variant any real
/// production `WgpuDrv.cpp` call site ever produces), or a plain
/// `()` marker for an offscreen target with nothing to present.
/// Split out specifically so this crate's own tests can exercise the
/// real `lock_frame`/`unlock_frame` encoder-and-draw-queue machinery
/// end to end against an offscreen texture, not a hand-rolled replica
/// of it (caught on review: wgpu surfaces are always backed by a real
/// OS window -- there is no "offscreen surface" -- so without this
/// split, the real bug class this whole refactor exists to catch
/// could only ever be tested by manually reproducing lock_frame's own
/// logic by hand, which cannot prove the *actual* `RenderLoop` code
/// is correct, only that a hand-written copy of its reasoning is).
enum FramePresentable {
    Surface(wgpu::SurfaceTexture),
    Offscreen,
    /// `lock_frame` got `Occluded` while `HP2_CAPTURE_FRAMES` is on:
    /// the frame is rendered into `RenderLoop::capture_target` instead
    /// (never presented) so it can still be captured -- XOpenGL renders
    /// and captures every frame regardless of window visibility, and
    /// capture consumers (`Tests/DeterminismDoubleRunTests.py`,
    /// `Build/smoke_maps.py`) depend on one capture per frame.
    CaptureOnly(wgpu::Texture),
}

/// One in-flight frame between `lock_frame`/`begin_offscreen_frame`
/// and `unlock_frame`: what was acquired (`FramePresentable`), the
/// `TextureView` built from it (`encoder.begin_render_pass` needs a
/// `&TextureView`, not the texture itself), and the `CommandEncoder`
/// the clear render pass -- and now, any queued draws -- are recorded
/// into. `presentable`/`view` are both read in `unlock_frame` (`view`
/// only transitively, via the encoder already holding everything it
/// needs -- kept here anyway so this struct, not a borrow of it, is
/// what crosses the `lock_frame`/`unlock_frame` boundary;
/// `SurfaceTexture`'s own `Drop` impl discards it safely if
/// `unlock_frame` is never actually called with `present: true`,
/// matching wgpu's own documented behavior).
struct InProgressFrame {
    presentable: FramePresentable,
    #[allow(dead_code)] // kept alive for presentable's sake; never read directly
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
    /// The real color target format, needed to lazily build/reuse a
    /// `draw::SimpleLinePipeline` matching it when flushing queued
    /// line draws in `unlock_frame` -- read directly from the real
    /// acquired texture (`lock_frame`) or the caller-given offscreen
    /// one (`begin_offscreen_frame`), not assumed or re-derived from
    /// surface config each time.
    color_format: wgpu::TextureFormat,
    /// `true` if `get_current_texture` returned `Suboptimal`, not
    /// `Success` -- `unlock_frame` clears `configured_size` for this
    /// case the same way `lock_frame` already does for `Outdated`,
    /// since `Suboptimal`'s own meaning is "usable now, but
    /// reconfigure soon" (`Surface::get_current_texture`'s own doc).
    /// Deferred to `unlock_frame`, not done here: the frame is still
    /// in flight at this point, and reconfiguring before it is
    /// submitted/presented would be premature. Always `false` for
    /// `FramePresentable::Offscreen` -- no surface, nothing to
    /// reconfigure.
    suboptimal: bool,
}

/// Per-live-`CreatedDevice` tally of every `lock_frame` outcome, not
/// just the latest one: a single real/production run can mix
/// `Presented` and `Occluded` calls across many ticks (confirmed on
/// this project's own real `hp2_game -window -testticks=60` run,
/// which got `Occluded` on all 60 of 60 ticks -- see `WgpuDrv.h`'s
/// own doc), so a single last-outcome value would hide that a run
/// consisting entirely of failures still "completed" without ever
/// actually drawing anything. `UWgpuRenderDevice::Exit` logs these
/// counts; `Build/run_wgpu_smoke.py` uses that log line to tell a run
/// that really presented frames apart from one that merely didn't
/// crash while trying to.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCounts {
    pub presented: u32,
    pub occluded: u32,
    pub outdated: u32,
    pub other_failure: u32,
}

/// One real queued `Draw2DLine` call (`RenderLoop::queue_line`'s own
/// doc has the full contract): `p1`/`p2` already screen-to-clip-
/// transformed (`draw_2d_line_screen_to_coords`'s own real output,
/// not raw screen pixels), `color` the real `DrawColor`. Carries no
/// segment index of its own -- that now lives on the owning `Batch`
/// (every line in one `BatchPayload::Lines` group shares one real
/// `SetSceneNode` call's segment, by construction of `queue_line`
/// itself), not duplicated per entry the way an earlier version of
/// this struct did.
struct QueuedLine {
    p1: [f32; 3],
    p2: [f32; 3],
    color: [f32; 4],
}

/// One real queued `DrawTile` call's own real quad geometry
/// (`tile_quad_corners`'s own real output, already screen-to-clip-
/// transformed the same way `QueuedLine::p1`/`p2` are -- both apply
/// the identical real `RFX2 * Z * (coord - FX2)` formula, just to a
/// quad's four corners instead of a line's two endpoints) plus the
/// real per-draw `DrawColor`/`DrawFlags` (`compute_tile_draw_color`/
/// `compute_tile_draw_flags`'s own real output). Carries no segment
/// or texture `cache_id` of its own -- both live on the owning
/// `Batch`/`BatchPayload::Tiles`, same reasoning as `QueuedLine`.
struct QueuedTile {
    corners: draw::TileQuadCorners,
    draw_color: [f32; 4],
    draw_flags: u32,
}

/// One real queued `Draw2DPoint` call's own real quad geometry
/// (`point_quad_corners`'s own real output) plus the real per-draw
/// `DrawColor` (`Color.W` already forced to `1.0` -- `queue_point`'s
/// own doc has the full real reasoning, same as `QueuedLine::color`).
/// Carries no segment of its own, same reasoning as `QueuedLine`/
/// `QueuedTile`.
struct QueuedPoint {
    corners: draw::PointQuadCorners,
    color: [f32; 4],
}

/// One real queued `DrawTriangles` call's own real mesh: its own
/// already-index/fan-resolved flat triangle-list vertices (`draw::
/// GouraudVertexInput`, the real `BufferVert`-extracted per-vertex
/// data, C++'s own responsibility to resolve from the real `Pts`/
/// `Indices` arrays before calling in -- `RenderLoop::queue_triangles`'
/// own doc has the full real reasoning for why that resolution lives
/// on the caller side, not here) plus the real per-draw `DiffuseInfo`/
/// `DrawColor`/`DrawFlags` (`PrepareGouraudCall`'s own scoped-down
/// output this crate's own `queue_triangles` computes). Carries no
/// segment or texture `cache_id` of its own, same reasoning as
/// `QueuedTile`.
struct QueuedTriangleMesh {
    vertices: Vec<draw::GouraudVertexInput>,
    diffuse_info: [f32; 4],
    draw_color: [f32; 4],
    draw_flags: u32,
}

/// One real queued `DrawComplexSurface` call's own real surface: its
/// own flat fan-triangulated vertex coords (real `FSavedPoly`
/// linked-list polygons, already triangulated -- `RenderLoop::queue_
/// complex_surface`'s own doc has the full real reasoning for why
/// this crate's own CPU-buffer-then-upload-once architecture needs
/// no equivalent of the real source's own mid-call vertex-buffer-
/// capacity flush-restart, DrawComplex.cpp:213-229) plus the real
/// per-draw `DiffuseUV`/`LightMapUV`/`FogMapUV`/`DiffuseInfo`/`XAxis`/
/// `YAxis`/`DrawFlags` (`DrawComplexSurface`/`SetTextureHelper`'s own
/// scoped-down real output this crate's own `queue_complex_surface`
/// computes). Carries no segment or texture `cache_id`s of its own,
/// same reasoning as `QueuedTile`.
struct QueuedComplexSurface {
    vertices: Vec<draw::ComplexVertexInput>,
    diffuse_uv: [f32; 4],
    light_map_uv: [f32; 4],
    fog_map_uv: [f32; 4],
    diffuse_info: [f32; 4],
    x_axis: [f32; 4],
    y_axis: [f32; 4],
    draw_flags: u32,
}

/// One ordered entry in `RenderLoop::batches`. Either queued lines or
/// queued tiles for one real segment (`Batch::segment`, an index into
/// `RenderLoop::frame_uniforms_segments` -- same real meaning as
/// `QueuedLine`'s own former `segment` field) -- and, for tiles only,
/// one bound texture (`cache_id`).
///
/// Matches the real source's own flush-on-state-change behavior,
/// confirmed by direct re-read of three separate real call sites, not
/// assumed from any one of them alone: `Draw2DLine`/`Draw2DPoint`/
/// `DrawTile` each call `SetProgram` with their own distinct shader
/// program at their own entry point (`DrawSimple.cpp:60,100,162,214`,
/// `DrawTile.cpp:31`), and `SetProgram` flushes whatever the
/// previously active program had buffered before switching
/// (`XOpenGL.cpp:2978-2995`, `ShaderProgramImpl::DeactivateShader`'s
/// own `Flush(false)` call, `XOpenGL.h:1574-1577`) -- a primitive-
/// kind change is a real flush boundary. Separately, binding a
/// *different* texture than whatever a shader's own TMU already has
/// bound is *also* a real flush boundary, for tiles specifically:
/// `SetTexture.cpp:502` bails out early (no flush) only when the
/// requested texture is already bound and not stale; otherwise
/// `SetTexture.cpp:515` flushes `Shaders[ActiveProgram]` before
/// rebinding -- confirmed by direct re-read, not inferred from
/// `SubmitTileBatch`'s own flush conditions alone (`DrawTile.cpp:
/// 188-197`), which do *not* check the bound texture at all (the
/// real per-draw `DrawCallParams` entry carries its own bindless
/// texture handle, so multiple distinct textures actually can
/// coexist within one already-*flushed*-and-newly-started real GL
/// batch -- but only until the *next* `SetTexture` call changes the
/// binding again, which is exactly the real flush this models). This
/// Rust port's own `TilePipeline::record` binds one `texture_view`/
/// `sampler` pair per call (no bindless equivalent), so here a
/// texture change is unconditionally a new batch, matching the real
/// `SetTexture.cpp:502/515` behavior exactly rather than diverging
/// from it for portability reasons (an earlier version of this doc
/// wrongly attributed the need to this port's own lack of bindless
/// textures alone -- corrected on review after re-reading `SetTexture.
/// cpp:499-515` directly: the real engine flushes on texture change
/// too, just from a different call site than `SubmitTileBatch`'s own
/// buffer-full/blend-state checks).
///
/// An earlier version of this crate's own design (before `queue_tile`
/// existed) used a single flat `queued_lines: Vec<QueuedLine>`, with
/// no equivalent for tiles even contemplated yet -- a hypothetical
/// separate `queued_tiles: Vec<QueuedTile>`, flushed in its own fixed-
/// order loop after all lines, would have been wrong for any real
/// frame that interleaves `Draw2DLine` and `DrawTile` calls within one
/// segment (editor grid-plus-icon overlays are a real, reachable
/// example): final compositing (both pipelines use `blend: None`, so
/// whichever draw lands on a given pixel *last* wins outright) must
/// follow real call order, not a per-primitive-kind fixed order.
/// `batches` is one single ordered `Vec`; `queue_line`/`queue_tile`/
/// `queue_point`/`queue_triangles`/`queue_complex_surface` each
/// either extend the last batch (same segment, same kind, same
/// `cache_id`(s) for tiles/triangles/complex surfaces) or push a
/// brand new one -- `unlock_frame` then replays `batches` in push
/// order, which is real call order.
enum BatchPayload {
    Lines(Vec<QueuedLine>),
    Tiles {
        cache_id: u64,
        entries: Vec<QueuedTile>,
    },
    Points(Vec<QueuedPoint>),
    Triangles {
        cache_id: u64,
        meshes: Vec<QueuedTriangleMesh>,
    },
    Complex {
        diffuse_cache_id: u64,
        /// `None` when the real `Surface.LightMap`/`FogMap` was null
        /// for every surface in this batch (the real dummy-texture
        /// case, `RenderLoop::dummy_texture`'s own doc has the full
        /// real reasoning) -- a batch boundary on this field changing
        /// matches the real per-layer `SetTexture` TMU-staleness
        /// check the same way `Tiles`'/`Triangles`' own `cache_id`
        /// already does, not a simplification specific to this
        /// variant.
        light_map_cache_id: Option<u64>,
        fog_map_cache_id: Option<u64>,
        surfaces: Vec<QueuedComplexSurface>,
    },
}

struct Batch {
    segment: usize,
    payload: BatchPayload,
}

/// Per-`CreatedDevice` state that must persist across the
/// `lock_frame`/`unlock_frame` pair (`UWgpuRenderDevice::Lock`/
/// `Unlock`'s future Rust half) and across repeated Lock/Unlock
/// cycles: `configured_size` so every `lock_frame` call does not
/// reconfigure the surface (`Surface::configure`'s own doc: "If the
/// surface is already configured, this will wait for the GPU to come
/// idle before recreating the swapchain" -- real per-frame cost, not
/// just wasted work, if done when the size has not actually changed).
/// One `RenderLoop` per live `CreatedDevice`; `WgpuHandle` (`hp2-ffi`)
/// owns both together.
pub struct RenderLoop {
    configured_size: Option<(u32, u32)>,
    /// Tracks the real `PresentMode` `lock_frame` last configured the
    /// surface with -- alongside `configured_size`, so a `UseVSync`
    /// ini-key change (without a matching resolution change) still
    /// triggers a real `surface.configure` on the next `lock_frame`
    /// call instead of being silently ignored until the next resize.
    configured_present_mode: Option<wgpu::PresentMode>,
    in_progress: Option<InProgressFrame>,
    counts: FrameCounts,
    /// One real GPU buffer per `set_scene_node` call since the last
    /// `lock_frame`/`begin_offscreen_frame` -- the real per-segment
    /// fix for a real bug an earlier version of this crate's own
    /// design had (caught on review before any draw call ever shipped
    /// against it): `SetSceneNode` runs many times per real frame
    /// (confirmed: exactly 3 per tick in a real wgpu-smoke.log run --
    /// the 3D world, editor orthographic viewports, HUD/Canvas, plus
    /// any mirrors/portals/sub-viewports), and `queue.write_buffer`
    /// does not apply at the call site -- it stages the write,
    /// applied at the next `queue.submit`, before any command in that
    /// submit runs. A single shared buffer overwritten in place on
    /// every call would mean every draw in one submit saw only the
    /// *last* `SetSceneNode` call's matrices, not its own segment's.
    /// Each segment is `528` bytes (`FrameUniforms`' own real size --
    /// checked by a test below rather than assumed to stay in sync if
    /// the struct's own field list ever changes); a small, separate
    /// `wgpu::Buffer` per segment, not one larger buffer with dynamic-
    /// offset slots -- real per-frame segment counts are small
    /// (single digits, per the same real log evidence above), so N
    /// tiny allocations is simpler and no less correct than dynamic
    /// offsets, and avoids rewriting `SimpleLinePipeline`/
    /// `TilePipeline`'s own already-verified `layout: None`-derived
    /// bind group layouts (switching to dynamic offsets needs an
    /// explicit, hand-written layout for every group in every
    /// pipeline, not just group 0 -- a real, separate, higher-risk
    /// change this fix does not need). Cleared at the start of every
    /// real `lock_frame`/`begin_offscreen_frame` call (regardless of
    /// that call's own outcome -- a stale segment from a previous,
    /// already-flushed frame must never leak into the next one).
    frame_uniforms_segments: Vec<wgpu::Buffer>,
    /// Appended by `queue_line`/`queue_tile`, drained and recorded
    /// into the real in-progress frame's own `CommandEncoder` by
    /// `unlock_frame`, *after* the clear pass that encoder already
    /// holds -- not drawn immediately per call (this crate's own todo
    /// list already tracked that as a real, serious bug: `lock_frame`
    /// only records its clear pass without submitting it, so an
    /// immediate per-call submit would run before the still-
    /// unsubmitted clear and get wiped out by it; see `draw::
    /// LineDrawCall`'s own doc for the full reasoning). One single
    /// ordered `Vec<Batch>`, not a separate queue per primitive kind
    /// -- `Batch`'s own doc has the full reasoning (real call-order
    /// compositing) and the real flush-boundary citations (primitive-
    /// kind change, segment change, texture change for tiles).
    /// `unlock_frame` replays `batches` in push order, chunking each
    /// batch's own contents into `draw::MAX_DRAWS`-sized `record()`
    /// calls, re-indexing local `draw_id`s fresh per chunk -- not one
    /// call for the whole frame sharing a single (wrong, per the
    /// `frame_uniforms_segments` field above) `FrameUniforms` buffer.
    batches: Vec<Batch>,
    /// Lazily built on first use against whichever `color_format` the
    /// first real in-progress frame actually has -- cached and
    /// *checked* against that format on every later use (a `RenderLoop`
    /// is only ever used against one `CreatedDevice`/surface for its
    /// whole real lifetime, so a mismatch here would mean a real
    /// caller bug, not a condition to silently handle).
    line_pipeline: Option<(wgpu::TextureFormat, draw::SimpleLinePipeline)>,
    /// Same lazy-build-and-check contract as `line_pipeline`, for
    /// `queue_tile`'s own real draws.
    tile_pipeline: Option<(wgpu::TextureFormat, draw::TilePipeline)>,
    /// Same lazy-build-and-check contract as `line_pipeline`/`tile_
    /// pipeline`, for `queue_point`'s own real draws.
    point_pipeline: Option<(wgpu::TextureFormat, draw::SimpleTrianglePipeline)>,
    /// Same lazy-build-and-check contract as `line_pipeline`/`tile_
    /// pipeline`/`point_pipeline`, for `queue_triangles`' own real
    /// draws.
    gouraud_pipeline: Option<(wgpu::TextureFormat, draw::GouraudPipeline)>,
    /// Same lazy-build-and-check contract as `line_pipeline`/`tile_
    /// pipeline`/`point_pipeline`/`gouraud_pipeline`, for `queue_
    /// complex_surface`'s own real draws.
    complex_pipeline: Option<(wgpu::TextureFormat, draw::ComplexPipeline)>,
    /// Real `BindMap`'s own role (`texture.rs`'s own `TextureCache`
    /// doc has the full real citation) -- persists across frames
    /// (unlike `batches`/`frame_uniforms_segments`, never cleared by
    /// `lock_frame`/`begin_offscreen_frame`/`unlock_frame`), matching
    /// the real `BindMap`'s own lifetime: uploaded textures stay
    /// resident until the real engine itself invalidates/replaces
    /// them, not just for one frame.
    texture_cache: texture::TextureCache,
    /// Lazily created on first real tile draw (`unlock_frame`'s own
    /// lazy-init, same point `tile_pipeline` itself is ensured) --
    /// one shared `wgpu::Sampler` for every `queue_tile` draw this
    /// `RenderLoop`'s whole lifetime, `wgpu::SamplerDescriptor::
    /// default()` (bilinear-equivalent default filtering, no
    /// anisotropy). A real, known, explicitly scoped gap, not
    /// silently pretended away: the real source's own sampler state
    /// varies per real `FTextureInfo` (wrap mode, `MaxAnisotropy`
    /// ini key) -- this crate's own todo list already tracks real
    /// per-texture sampler state and `MaxAnisotropy` wiring as a
    /// separate, not-yet-done item; one fixed default sampler is the
    /// right scope for the first real textured-quad draw to land,
    /// not a blocker on wiring real per-texture sampler state first.
    sampler: Option<wgpu::Sampler>,
    /// Lazily created once this `RenderLoop`'s whole lifetime (unlike
    /// `texture_cache`'s own real per-`CacheID` entries, never
    /// invalidated/replaced): a real 1x1 opaque white GPU texture,
    /// bound as the real light-map/fog-map texture slot whenever a
    /// real `DrawComplexSurface` call's own `Surface.LightMap`/
    /// `FogMap` is null -- `complex.wgsl`'s own module doc has the
    /// full real reasoning for why WGSL needs *some* valid texture
    /// bound there regardless (it cannot conditionally declare a
    /// binding the way the real GLSL's own `#if OPT_HasLightMap`/
    /// `#if OPT_HasFogMap` can), and why its own color is never
    /// actually read in that case (`select()` discards the sampled
    /// value entirely whenever the corresponding `DrawFlags` bit is
    /// clear) -- deliberately *not* stored inside `texture_cache`
    /// itself (no risk of ever colliding with a real `CacheID`, and
    /// keeps "this is a real uploaded texture" and "this is an inert
    /// always-bound placeholder" two clearly separate kinds of state).
    dummy_texture: Option<texture::CachedTexture>,
    /// `HP2_CAPTURE_FRAMES` end-of-frame capture (`capture::FrameCapture`),
    /// read once from the environment on the first `lock_frame`, exactly
    /// like XOpenGL.cpp's own lazily-initialized `GFrameCapture`
    /// (`HPInitFrameCapture`, XOpenGL.cpp:2686-2705). `None` when capture
    /// is off -- one `getenv` per process, nothing else.
    capture: Option<capture::FrameCapture>,
    capture_initialized: bool,
    /// HiDPI backing scale (drawable pixels per logical window pixel),
    /// set by the C++ side before each `lock_frame` (`set_backing_scale`)
    /// and recorded per captured frame as `backing_scale_factor`, the
    /// same value XOpenGL.cpp:2728-2731 computes from SDL.
    backing_scale: f64,
    /// Offscreen render target for `FramePresentable::CaptureOnly`
    /// frames, reused while its size and format still match.
    capture_target: Option<wgpu::Texture>,
    /// Set by every `set_scene_node` call -- the real `RFX2`/`RFY2`/
    /// `Frame->FX2`/`FY2` screen-to-clip transform `Draw2DLine`'s own
    /// real source applies (`DrawSimple.cpp:80,82`) is derived
    /// entirely from the most recent `SceneFrame`'s `fx`/`fy`/
    /// `is_ortho`/`effective_fov_angle`/`viewport_fov_angle` -- not
    /// from any separate `FSceneNode` parameter `Draw2DLine` itself
    /// receives beyond what `SetSceneNode` already captured, matching
    /// `UXOpenGLRenderDevice`'s own real `RFX2`/`RFY2` member fields
    /// (computed once per `SetSceneNode`, reused by every `Draw2DLine`
    /// call that frame -- `queue_line`'s own doc has the full
    /// reasoning). `None` until the first `set_scene_node` call --
    /// `queue_line` drops the draw cleanly in that case (a real,
    /// reachable edge case, not a caller bug: `Draw2DLine` called
    /// before any `SetSceneNode` this session).
    last_scene: Option<SceneFrame>,
}

impl Default for RenderLoop {
    fn default() -> Self {
        Self::new()
    }
}

/// Groups `RenderLoop::queue_tile`'s real inputs (clippy's
/// `too_many_arguments`, same reasoning as `draw::TileQuadRequest`
/// above -- this struct is one level higher: `DrawTile`'s own real
/// `FTextureInfo`-sourced fields (`cache_id`, `u_scale`, `v_scale`,
/// `texture_alpha`) plus its own direct real parameters, not yet
/// `rfx2`/`rfy2` which `queue_tile` itself derives from `self.
/// last_scene`). `cache_id` must already be `fix_cache_id`'s own
/// output (`texture.rs`'s own doc), matching `self.texture_cache`'s
/// own real lookup key. `u_scale`/`v_scale` are the real `FTextureInfo::
/// UScale`/`VScale` fields `texture_uv_mult` needs, read fresh from
/// whichever real `FTextureInfo` this specific `DrawTile` call
/// received -- not cached, same real per-bind-freshness reasoning as
/// `fx2`/`fy2` below. `texture_alpha` is `compute_tile_draw_color`'s
/// own real `Option<f32>` input (`None` for a real null `Info.
/// Texture`, confirmed real-reachable, `Some(Info.Texture->Alpha)`
/// otherwise). `fx2`/`fy2` are the real `Frame->FX2`/`FY2` the actual
/// `DrawTile` call site itself received, same real per-call-not-
/// cached reasoning `queue_line`'s own `fx2`/`fy2` parameters already
/// establish (`draw_2d_line_screen_to_coords`'s own doc has the full
/// citation) -- `DrawTile.cpp:219` etc. read `Frame->FX2`/`FY2` the
/// identical way `DrawSimple.cpp:80,82` does.
pub struct TileDrawRequest {
    pub cache_id: u64,
    pub x: f32,
    pub y: f32,
    pub xl: f32,
    pub yl: f32,
    pub u: f32,
    pub v: f32,
    pub ul: f32,
    pub vl: f32,
    pub z: f32,
    pub color: [f32; 4],
    pub poly_flags: u32,
    pub u_scale: f32,
    pub v_scale: f32,
    pub texture_alpha: Option<f32>,
    pub fx2: f32,
    pub fy2: f32,
}

/// Groups `RenderLoop::queue_triangles`' real scalar inputs (clippy's
/// `too_many_arguments`, same reasoning as `TileDrawRequest` -- the
/// real per-vertex data is a separate slice parameter, not part of
/// this struct, since it has its own lifetime and variable length).
/// `cache_id` must already be `fix_cache_id`'s own output, matching
/// `TileDrawRequest`'s own contract. `u_scale`/`v_scale` are the real
/// `FTextureInfo::UScale`/`VScale` fields, read fresh from whichever
/// real `FTextureInfo` this specific `DrawTriangles` call received --
/// same real per-bind-freshness reasoning as `TileDrawRequest`'s own.
/// `texture_alpha` is the real `Info.Texture->Alpha` (`None` for a
/// real null `Info.Texture`), same real `Option<f32>` shape as
/// `TileDrawRequest`'s own. `diffuse` is the real `Info.Texture ?
/// Info.Texture->Diffuse : 1.f` (`PrepareGouraudCall:166,171`) --
/// `UTexture::Diffuse`, a real field `TileDrawRequest` has no
/// equivalent of at all (`DrawTile`'s own real color path never reads
/// it). `Option<f32>` for the same real reason `texture_alpha` is:
/// `None` for a real null `Info.Texture`, resolved to `1.0` by
/// `queue_triangles` itself (`.unwrap_or(1.0)`, the real formula's
/// own simpler shape -- no `> 0.f` comparison the way `TextureAlpha`'s
/// own resolution needs, so no separate named pure function the way
/// `resolve_texture_alpha` was worth factoring out for that one).
pub struct TrianglesDrawRequest {
    pub cache_id: u64,
    pub poly_flags: u32,
    pub u_scale: f32,
    pub v_scale: f32,
    pub texture_alpha: Option<f32>,
    pub diffuse: Option<f32>,
}

/// One real texture layer's own per-bind inputs for `ComplexSurface
/// DrawRequest` (the real `SetTexture`'s own per-call fields:
/// `CacheID`/`UScale`/`VScale`/`Pan.X`/`Pan.Y`) -- `cache_id` must
/// already be `fix_cache_id`'s own output, same contract as every
/// other `*DrawRequest`'s own `cache_id` field. `u_scale`/`v_scale`
/// feed `texture_uv_mult`; `pan_x`/`pan_y` feed `texture_uv_pan`
/// (that function's own doc has the real `PanBias` reasoning --
/// `queue_complex_surface` itself supplies the real per-layer
/// constant, `0.0` for diffuse/`-0.5` for light-map/fog-map, not this
/// struct, since it is a property of *which* layer is being bound,
/// not something a real caller chooses per call).
pub struct ComplexTextureLayer {
    pub cache_id: u64,
    pub u_scale: f32,
    pub v_scale: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

/// Groups `RenderLoop::queue_complex_surface`'s real scalar inputs
/// (clippy's `too_many_arguments`, same reasoning as `TileDrawRequest`/
/// `TrianglesDrawRequest` -- the real per-vertex coords are a separate
/// slice parameter). `diffuse` is always present (the real source's
/// own `check(Surface.Texture)` assertion, `DrawComplex.cpp:60` --
/// every real `DrawComplexSurface` call has a real diffuse `Surface.
/// Texture`, confirmed, not assumed). `light_map`/`fog_map` are
/// `None` for the real `!Surface.LightMap`/`!Surface.FogMap` case
/// (real, common -- most real surfaces have neither or only one).
/// `texture_fields` is `Some((Diffuse, Specular, Alpha))` when the
/// real `Surface.Texture->Texture` is non-null, same real combined-
/// null-check shape `compute_complex_diffuse_info`'s own doc already
/// has the full citation for (not three independent `Option`s).
/// `c_alpha` is the real `BYTE cAlpha` `DrawComplexSurface` itself
/// receives. `x_axis`/`y_axis` are the real `Facet.MapCoords.XAxis`/
/// `YAxis`, already packed as `(axis.X, axis.Y, axis.Z, axis |
/// MapCoords.Origin)` (the real dot-product bias baked into `.w`,
/// `DrawComplex.cpp:193-194`) -- a future `WgpuDrv.cpp` override's own
/// responsibility to compute from the real `FCoords`, not this
/// function's.
pub struct ComplexSurfaceDrawRequest {
    pub diffuse: ComplexTextureLayer,
    pub light_map: Option<ComplexTextureLayer>,
    pub fog_map: Option<ComplexTextureLayer>,
    pub poly_flags: u32,
    pub texture_fields: Option<(f32, f32, f32)>,
    pub c_alpha: u8,
    pub x_axis: [f32; 4],
    pub y_axis: [f32; 4],
}

impl RenderLoop {
    pub fn new() -> Self {
        Self {
            configured_size: None,
            configured_present_mode: None,
            in_progress: None,
            counts: FrameCounts::default(),
            frame_uniforms_segments: Vec::new(),
            batches: Vec::new(),
            line_pipeline: None,
            tile_pipeline: None,
            point_pipeline: None,
            gouraud_pipeline: None,
            complex_pipeline: None,
            texture_cache: texture::TextureCache::new(),
            sampler: None,
            dummy_texture: None,
            last_scene: None,
            capture: None,
            capture_initialized: false,
            backing_scale: 1.0,
            capture_target: None,
        }
    }

    /// Lazily reads `HP2_CAPTURE_FRAMES`/`MAP`/`TICKS` once, matching
    /// XOpenGL.cpp's own `if (!GFrameCapture.Initialized)
    /// HPInitFrameCapture();`.
    fn ensure_capture_initialized(&mut self) {
        if self.capture_initialized {
            return;
        }
        self.capture_initialized = true;
        self.capture = capture::FrameCapture::from_env();
        if self.capture.is_some() {
            log::info!("WgpuDrv: frame capture enabled");
        }
    }

    /// Records the real HiDPI backing scale for the next captured frame
    /// (`backing_scale`'s own doc).
    pub fn set_backing_scale(&mut self, scale: f64) {
        self.backing_scale = scale;
    }

    pub fn counts(&self) -> FrameCounts {
        self.counts
    }

    /// Real per-tick entry point `UWgpuRenderDevice::Draw2DLine`
    /// calls through `hp2_rs_wgpu_draw_2d_line` (wired, not a future
    /// plan -- an earlier version of this doc said "not wired yet",
    /// true when written, now stale). `p1`/`p2` are raw screen-space
    /// `FVector`-shaped points -- `(pixel X, pixel Y, view-space Z)`,
    /// exactly `Draw2DLine`'s own real `P1`/`P2` parameters
    /// (`DrawSimple.cpp:52`), not already transformed. `fx2`/`fy2` are
    /// the real `Frame->FX2`/`FY2` the actual `Draw2DLine` call site
    /// itself received (`WgpuDrv.cpp`'s own override reads them
    /// straight off its own `Frame` parameter) -- not derived from
    /// `self.last_scene` (the real `rfx2`/`rfy2` computed below
    /// *are* correctly cached that way, matching `UXOpenGLRenderDevice`'s
    /// own real `RFX2`/`RFY2` member fields -- `draw_2d_line_screen_
    /// to_coords`'s own doc has the full real distinction and why an
    /// earlier version of this function wrongly conflated the two).
    /// This function applies the real `Draw2DLine` screen-to-clip
    /// transform itself (`DrawSimple.cpp:80,82`: `Coords = (RFX2 *
    /// P.Z * (P.X - Frame->FX2), RFY2 * P.Z * (P.Y - Frame->FY2),
    /// P.Z)`).
    ///
    /// Drops the draw cleanly (not an error, not a panic) when there
    /// is no frame in progress -- a real condition, not a caller bug:
    /// the engine keeps calling `SetSceneNode`/`Draw*` after a
    /// failed/`Occluded` `Lock` (confirmed by reading this project's
    /// own real `wgpu-smoke.log`, `WgpuDrv: Lock did not begin a
    /// frame` immediately followed by a real `SetSceneNode` call on
    /// the very next line), so silently dropping here is the correct,
    /// already-established behavior, not a gap. Also drops cleanly
    /// when no `set_scene_node` call has happened yet this frame
    /// (`self.frame_uniforms_segments` empty -- implies `self.
    /// last_scene` is also `None`, checked explicitly too since the
    /// transform itself needs a real `SceneFrame`, not just a segment
    /// to bind to): `Draw2DLine` called before any real `SetSceneNode`
    /// this session -- `RFX2`/`RFY2` would be undefined, so there is
    /// nothing correct to draw. Tagged with `self.
    /// frame_uniforms_segments.len() - 1` (the *current*, most
    /// recently created segment -- `RenderLoop::frame_uniforms_
    /// segments`'s own doc has the full real bug this fixes), not
    /// drawn with whichever segment happens to be bound when
    /// `unlock_frame` later flushes the queue. `unlock_frame`'s own
    /// flush replays `batches` in real call order (`Batch`'s own doc
    /// has the full reasoning), chunking each batch's own contents
    /// into one `record()` call per `simple.wgsl`'s real `MAX_DRAWS`
    /// capacity worth of lines (matching the real GLSL batcher's own
    /// `Flush(true)`-when-full behavior, `DrawSimple.cpp:68-69` --
    /// not checked here, since how many *other* lines will end up
    /// sharing this call's own batch is not yet known at queue time;
    /// no line is ever lost, unlike an earlier version of this doc
    /// and the flush logic it described, caught on review). Extends
    /// `self.batches`' own last entry when it is already a `Lines`
    /// batch for this same segment, otherwise pushes a new one --
    /// `Batch`'s own doc has the full reasoning for why a segment
    /// change (not just a primitive-kind change) is also a real new-
    /// batch boundary in this port's own design.
    pub fn queue_line(&mut self, p1: [f32; 3], p2: [f32; 3], fx2: f32, fy2: f32, color: [f32; 4]) {
        if self.in_progress.is_none() {
            return;
        }
        let Some(scene) = self.last_scene.as_ref() else {
            return;
        };
        let Some(segment) = self.frame_uniforms_segments.len().checked_sub(1) else {
            return;
        };
        let line = QueuedLine {
            p1: draw_2d_line_screen_to_coords(scene, fx2, fy2, p1),
            p2: draw_2d_line_screen_to_coords(scene, fx2, fy2, p2),
            color,
        };
        match self.batches.last_mut() {
            Some(Batch {
                segment: last_segment,
                payload: BatchPayload::Lines(lines),
            }) if *last_segment == segment => {
                lines.push(line);
            }
            _ => {
                self.batches.push(Batch {
                    segment,
                    payload: BatchPayload::Lines(vec![line]),
                });
            }
        }
    }

    /// Real per-tick entry point a future `UWgpuRenderDevice::
    /// DrawTile` override will call through a future `hp2_rs_wgpu_
    /// queue_tile` FFI function -- not wired yet (this crate's own
    /// todo list tracks the remaining FFI/`WgpuDrv.cpp`/real
    /// `FTextureInfo` CacheID-and-palette-extraction wiring as a
    /// separate, following change; this function itself is real,
    /// tested Rust, not a stub). Mirrors `DrawTile`'s own real
    /// pipeline in order (`DrawTile.cpp:24-56`, `SubmitTileBatch`,
    /// `DrawTile.cpp:117-265`, both read in full): looks up `request.
    /// cache_id` in `self.texture_cache` (dropped cleanly, matching
    /// `queue_line`'s own no-frame/no-scene drop convention, if the
    /// real engine ever calls this before the matching real
    /// `SetTexture`-equivalent upload landed -- a real, reachable
    /// transient condition, not a caller bug), computes `UMult`/
    /// `VMult` fresh via `texture::texture_uv_mult` from the cached
    /// texture's own real `width`/`height` and `request`'s own
    /// `u_scale`/`v_scale` (never cached -- `texture_uv_mult`'s own
    /// doc has the full real per-bind-freshness citation), builds the
    /// real quad geometry via `draw::tile_quad_corners`, and the real
    /// `DrawColor`/`DrawFlags` via `draw::compute_tile_draw_color`/
    /// `compute_tile_draw_flags`.
    ///
    /// Known, explicitly scoped gap, not silently pretended away:
    /// does not apply `DrawTile.cpp:206-211`'s real editor-only `Z =
    /// 1.0f` override (`GIsEditor && Frame->Viewport->Actor &&
    /// (Frame->Viewport->IsOrtho() || Abs(Z) <= SMALL_NUMBER)`) --
    /// none of `GIsEditor`/`Viewport->Actor`/`Viewport->IsOrtho()` are
    /// real inputs this function (or any real caller up to this
    /// point in the port) has yet; a future FFI wrapper that has
    /// real access to them would need to apply this itself before
    /// calling in, or this function would need a new explicit
    /// request field for it -- not invented here without a real
    /// caller to thread it from.
    ///
    /// Extends `self.batches`' own last entry when it is already a
    /// `Tiles` batch for this same segment *and* the same `cache_id`,
    /// otherwise pushes a new one -- `Batch`'s own doc has the full
    /// real citation for why a texture change is also a real new-
    /// batch boundary here, matching `SetTexture.cpp:502/515`
    /// directly, not a port-specific simplification.
    pub fn queue_tile(&mut self, request: TileDrawRequest) {
        if self.in_progress.is_none() {
            return;
        }
        let Some(scene) = self.last_scene.as_ref() else {
            return;
        };
        let Some(segment) = self.frame_uniforms_segments.len().checked_sub(1) else {
            return;
        };
        let Some(cached) = self.texture_cache.get(request.cache_id) else {
            return;
        };
        let (rfx2, rfy2) = scene_rfx2_rfy2(scene);
        let (u_mult, v_mult) = texture::texture_uv_mult(
            request.u_scale,
            request.v_scale,
            cached.width,
            cached.height,
        );
        let corners = draw::tile_quad_corners(draw::TileQuadRequest {
            rfx2,
            rfy2,
            fx2: request.fx2,
            fy2: request.fy2,
            x: request.x,
            y: request.y,
            xl: request.xl,
            yl: request.yl,
            u: request.u,
            v: request.v,
            ul: request.ul,
            vl: request.vl,
            z: request.z,
            u_mult,
            v_mult,
        });
        let tile = QueuedTile {
            corners,
            draw_color: draw::compute_tile_draw_color(
                request.color,
                request.poly_flags,
                request.texture_alpha,
            ),
            draw_flags: draw::compute_tile_draw_flags(request.poly_flags),
        };
        match self.batches.last_mut() {
            Some(Batch {
                segment: last_segment,
                payload: BatchPayload::Tiles { cache_id, entries },
            }) if *last_segment == segment && *cache_id == request.cache_id => {
                entries.push(tile);
            }
            _ => {
                self.batches.push(Batch {
                    segment,
                    payload: BatchPayload::Tiles {
                        cache_id: request.cache_id,
                        entries: vec![tile],
                    },
                });
            }
        }
    }

    /// Real per-tick entry point a future `UWgpuRenderDevice::
    /// Draw2DPoint` override will call through a future `hp2_rs_wgpu_
    /// queue_point` FFI function -- not wired yet (same staged-rollout
    /// reasoning `queue_tile`'s own doc already established: this is
    /// real, tested Rust, not a stub, the FFI/`WgpuDrv.cpp` wiring is
    /// a separate, following change). Mirrors `Draw2DPoint`'s own real
    /// pipeline in order (`DrawSimple.cpp:154-205`, read in full):
    /// `Color.W` forced to `1.0` (`color` here is `[f32; 3]`, same
    /// convention `queue_line`'s own FFI wrapper already established
    /// for `Draw2DLine` -- the real source always overwrites alpha,
    /// so a caller-supplied one could never mean anything), `Z`
    /// resolved via `resolve_point_z` *before* being passed into
    /// `draw::point_quad_corners` (not after -- the real source
    /// applies its own `Z` adjustment before computing any vertex
    /// coordinates, `DrawSimple.cpp:170-173` runs before `:187-198`).
    ///
    /// `fx2`/`fy2` are the real `Frame->FX2`/`FY2` the actual
    /// `Draw2DPoint` call received -- `DrawSimple.cpp:187` reads
    /// `Frame->FX2` directly off its own `Frame` parameter, the exact
    /// same real pattern `Draw2DLine`/`DrawTile` already established
    /// (`draw_2d_line_screen_to_coords`'s own doc has the full real
    /// citation and why this must never be derived from `self.
    /// last_scene` instead). An earlier version of this function did
    /// exactly that wrong derivation (`fx2: scene.fx * 0.5`) -- caught
    /// by re-reading `DrawSimple.cpp:187` against this function's own
    /// first draft before it was ever built, the same real mistake
    /// `6de550c` already fixed once for `Draw2DLine` specifically;
    /// fixed here before landing, not shipped and fixed later.
    ///
    /// Known, explicitly scoped gaps, not silently pretended away,
    /// same category `hp2_rs_wgpu_draw_2d_line`'s own doc already
    /// names for lines: the real `HitTesting() ? HitColor : Color`
    /// substitution and the real fixed `BlendMode = PF_Highlighted`
    /// (`DrawSimple.cpp:166-168`) are not applied -- `SimpleTriangle
    /// Pipeline`'s own fixed `blend: None` is the same known first-
    /// proof limitation `SimpleLinePipeline`'s own doc already states.
    ///
    /// Extends `self.batches`' own last entry when it is already a
    /// `Points` batch for this same segment, otherwise pushes a new
    /// one -- same real flush-boundary reasoning `Batch`'s own doc
    /// already has (primitive-kind change, confirmed via `Draw2DPoint`'s
    /// own real `SetProgram(Simple_Triangle_Prog)` call site,
    /// `DrawSimple.cpp:162`).
    #[allow(clippy::too_many_arguments)]
    pub fn queue_point(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        z: f32,
        fx2: f32,
        fy2: f32,
        color: [f32; 3],
    ) {
        if self.in_progress.is_none() {
            return;
        }
        let Some(scene) = self.last_scene.as_ref() else {
            return;
        };
        let Some(segment) = self.frame_uniforms_segments.len().checked_sub(1) else {
            return;
        };
        let (rfx2, rfy2) = scene_rfx2_rfy2(scene);
        let z = resolve_point_z(scene.is_ortho, z);
        let corners = draw::point_quad_corners(draw::PointQuadRequest {
            rfx2,
            rfy2,
            fx2,
            fy2,
            x1,
            y1,
            x2,
            y2,
            z,
        });
        let point = QueuedPoint {
            corners,
            color: [color[0], color[1], color[2], 1.0],
        };
        match self.batches.last_mut() {
            Some(Batch {
                segment: last_segment,
                payload: BatchPayload::Points(points),
            }) if *last_segment == segment => {
                points.push(point);
            }
            _ => {
                self.batches.push(Batch {
                    segment,
                    payload: BatchPayload::Points(vec![point]),
                });
            }
        }
    }

    /// Real per-tick entry point a future `UWgpuRenderDevice::
    /// DrawTriangles` override will call through a future `hp2_rs_
    /// wgpu_queue_triangles` FFI function -- not wired yet (same
    /// staged-rollout pattern `queue_tile`/`queue_point`'s own docs
    /// already established). Mirrors `DrawTriangles`/`PrepareGouraud
    /// Call`'s own real pipeline (`DrawGouraud.cpp:68-205`/`285-359`,
    /// both read in full), *after* the real `Indices`/fan validation
    /// and expansion into a flat triangle list -- that part is this
    /// function's own caller's responsibility, not this function's:
    /// the real `Pts: FTransTexture**`/`Indices: _WORD*` indirection
    /// (nullable individual `Pts[i]` pointers, an optional index
    /// buffer selecting between an indexed list and a triangle fan
    /// over `Pts[0..NumPts)`, real out-of-range/null-pointer
    /// validation that silently drops the *whole* call, not just the
    /// bad vertex) is real `FTransTexture*` pointer-dereferencing
    /// work this crate has no access to -- a future `WgpuDrv.cpp`
    /// override does that real resolution itself (mirroring `DrawGouraud.
    /// cpp:303-350`'s own real logic exactly) and calls in with an
    /// already-flat `vertices` slice, the real `BufferVert`-extracted
    /// per-vertex data (`draw::GouraudVertexInput`'s own doc has the
    /// full real field-by-field citation).
    ///
    /// Computes the real, scoped-down `DiffuseInfo`/`DrawColor`/
    /// `DrawFlags` (`PrepareGouraudCall:166-173`): `texture_uv_mult`
    /// for `UMult`/`VMult` (never cached, same real per-bind-
    /// freshness reasoning `queue_tile`'s own doc already
    /// establishes), `resolve_texture_alpha` for the real shared
    /// `TextureAlpha` formula, `compute_tile_draw_flags` for
    /// `DrawFlags` (the same real `GetPolyFlagsAndDrawFlags` function
    /// `SubmitTileBatch` itself calls -- confirmed by direct re-read
    /// that function's own doc already has the full citation for, not
    /// a coincidentally-similar second function).
    ///
    /// Known, explicitly scoped gaps, not silently pretended away,
    /// all confirmed by reading `PrepareGouraudCall` in full, not
    /// assumed from `DrawTriangles`' own simpler entry point alone:
    /// detail textures, macro textures, bump maps, the real `OPT_
    /// HWLighting`/shader-specialization system, `GIsEditor &&
    /// PF_Selected -> DF_Selected`, `NoNearZ`/`SetProjection`
    /// re-invocation, and the real `HitTesting() ? HitColor : (0,0,0,
    /// TextureAlpha)` substitution are not applied -- `gouraud.wgsl`'s
    /// own module doc already names every one of these as a real,
    /// separately scoped, not-yet-ported `#if`-gated GLSL branch, not
    /// an invented scope cut made here.
    ///
    /// Drops cleanly (not a panic), same established conditions as
    /// `queue_tile`: no frame in progress, no `set_scene_node` call
    /// yet this frame, or `request.cache_id` not yet uploaded. Also
    /// drops cleanly on an empty `vertices` slice -- a real
    /// `DrawTriangles` call already guarantees `OutVertexCount >= 3`
    /// before ever reaching this point (`DrawGouraud.cpp:305-306`),
    /// so an empty slice here would mean a caller bug upstream of
    /// this function, not a condition to panic on.
    ///
    /// Extends `self.batches`' own last entry when it is already a
    /// `Triangles` batch for this same segment *and* the same
    /// `cache_id`, otherwise pushes a new one -- same real flush-
    /// boundary reasoning `Batch`'s own doc already has (primitive-
    /// kind change confirmed via `DrawTriangles`' own real
    /// `SetProgram(Gouraud_Prog)` call site, `DrawGouraud.cpp:326`;
    /// texture change confirmed via the same real `SetTexture.cpp:
    /// 502/515` citation already established for tiles -- `Prepare
    /// GouraudCall:170`'s own `SetTexture(DiffuseTextureIndex, ...)`
    /// call is the identical real function, not a different one).
    pub fn queue_triangles(
        &mut self,
        request: TrianglesDrawRequest,
        vertices: &[draw::GouraudVertexInput],
    ) {
        if self.in_progress.is_none() {
            return;
        }
        let Some(segment) = self.frame_uniforms_segments.len().checked_sub(1) else {
            return;
        };
        let Some(cached) = self.texture_cache.get(request.cache_id) else {
            return;
        };
        if vertices.is_empty() {
            return;
        }
        let (u_mult, v_mult) = texture::texture_uv_mult(
            request.u_scale,
            request.v_scale,
            cached.width,
            cached.height,
        );
        let texture_alpha = draw::resolve_texture_alpha(request.texture_alpha);
        let mesh = QueuedTriangleMesh {
            vertices: vertices.to_vec(),
            diffuse_info: [
                u_mult,
                v_mult,
                request.diffuse.unwrap_or(1.0),
                texture_alpha,
            ],
            draw_color: [0.0, 0.0, 0.0, texture_alpha],
            draw_flags: draw::compute_tile_draw_flags(request.poly_flags),
        };
        match self.batches.last_mut() {
            Some(Batch {
                segment: last_segment,
                payload: BatchPayload::Triangles { cache_id, meshes },
            }) if *last_segment == segment && *cache_id == request.cache_id => {
                meshes.push(mesh);
            }
            _ => {
                self.batches.push(Batch {
                    segment,
                    payload: BatchPayload::Triangles {
                        cache_id: request.cache_id,
                        meshes: vec![mesh],
                    },
                });
            }
        }
    }

    /// Real per-tick entry point a future `UWgpuRenderDevice::
    /// DrawComplexSurface` override will call through a future
    /// `hp2_rs_wgpu_queue_complex_surface` FFI function -- not wired
    /// yet (same staged-rollout pattern every other `queue_*`
    /// function's own doc already established). Mirrors
    /// `DrawComplexSurface`/`SetTextureHelper`'s own real pipeline
    /// (`DrawComplex.cpp:21-259`, read in full) for the real base
    /// path only (diffuse + light map + fog map -- `complex.wgsl`'s
    /// own module doc already scopes detail/macro/bump/environment/
    /// parallax/distance-fog/editor/multi-pass as real, separately
    /// tracked, not-yet-ported branches, not an invented cut made
    /// here), *after* the real per-polygon fan-triangulation -- that
    /// part is this function's own caller's responsibility, same real
    /// reasoning `queue_triangles`' own doc already establishes for
    /// its own fan/indexed expansion (real `FSavedPoly`/`FTransform*`
    /// pointer-dereferencing work this crate has no access to).
    ///
    /// Computes each present real texture layer's own `UMult`/`VMult`/
    /// `UPan`/`VPan` fresh (`texture_uv_mult`/`texture_uv_pan`, never
    /// cached, same real per-bind-freshness reasoning established for
    /// every other `queue_*` function) with the real per-layer
    /// `PanBias` constant baked in here (`0.0` diffuse, `-0.5` light-
    /// map/fog-map, confirmed against the real five `SetTextureHelper`
    /// call sites, `DrawComplex.cpp:168,175,178`). A real `light_map`/
    /// `fog_map` layer whose own `cache_id` is not yet actually
    /// uploaded (a real, reachable transient condition, same category
    /// `queue_tile`'s own doc already covers for the single-layer
    /// case) degrades to the same real dummy-texture/absent-flag
    /// treatment as a real null `Surface.LightMap`/`FogMap` -- a
    /// deliberate choice, not silently pretended away: the real
    /// source has no equivalent transient-upload-ordering condition
    /// to begin with (a real `FCachedTexture` either exists or the
    /// real engine is already uploading it synchronously), so there
    /// is no single real behavior to match exactly here; failing the
    /// *whole* surface (like a missing diffuse texture does) would
    /// lose strictly more than necessary for what is, in this port,
    /// only a one-frame-transient condition -- rendering the surface
    /// without that one optional layer for this one frame is the
    /// safer, more conservative choice.
    ///
    /// Drops the whole call cleanly (not a panic), same established
    /// conditions as every other `queue_*` function: no frame in
    /// progress, no `set_scene_node` call yet this frame, the real
    /// diffuse texture not yet uploaded (unlike light-map/fog-map,
    /// *not* degradable -- every real `DrawComplexSurface` call has a
    /// real diffuse `Surface.Texture`, confirmed by the real source's
    /// own `check(Surface.Texture)` assertion, so a missing diffuse
    /// cache entry here means the real engine's own `SetTexture`
    /// upload has not reached this `cache_id` yet, the same real
    /// condition `queue_tile`'s own doc already covers), or an empty
    /// `vertices` slice.
    ///
    /// Extends `self.batches`' own last entry when it is already a
    /// `Complex` batch for this same segment *and* all three real
    /// `cache_id`s match (diffuse, and light-map/fog-map including
    /// their own `None`-for-dummy state), otherwise pushes a new one
    /// -- `BatchPayload::Complex`'s own doc has the full real
    /// citation for why all three, not just diffuse, are real batch
    /// boundaries here.
    pub fn queue_complex_surface(
        &mut self,
        request: ComplexSurfaceDrawRequest,
        vertices: &[draw::ComplexVertexInput],
    ) {
        if self.in_progress.is_none() {
            return;
        }
        let Some(segment) = self.frame_uniforms_segments.len().checked_sub(1) else {
            return;
        };
        let Some(diffuse_cached) = self.texture_cache.get(request.diffuse.cache_id) else {
            return;
        };
        if vertices.is_empty() {
            return;
        }
        let (diffuse_u_mult, diffuse_v_mult) = texture::texture_uv_mult(
            request.diffuse.u_scale,
            request.diffuse.v_scale,
            diffuse_cached.width,
            diffuse_cached.height,
        );
        let (diffuse_u_pan, diffuse_v_pan) = texture::texture_uv_pan(
            request.diffuse.pan_x,
            request.diffuse.pan_y,
            0.0,
            request.diffuse.u_scale,
            request.diffuse.v_scale,
        );
        let diffuse_uv = [diffuse_u_mult, diffuse_v_mult, diffuse_u_pan, diffuse_v_pan];

        const LIGHT_FOG_PAN_BIAS: f32 = -0.5;
        const DF_LIGHT_MAP: u32 = 0x0002;
        const DF_FOG_MAP: u32 = 0x0004;
        let mut draw_flags = draw::compute_tile_draw_flags(request.poly_flags);

        let (light_map_uv, light_map_cache_id) = match request.light_map.as_ref() {
            Some(layer) => match self.texture_cache.get(layer.cache_id) {
                Some(cached) => {
                    let (u_mult, v_mult) = texture::texture_uv_mult(
                        layer.u_scale,
                        layer.v_scale,
                        cached.width,
                        cached.height,
                    );
                    let (u_pan, v_pan) = texture::texture_uv_pan(
                        layer.pan_x,
                        layer.pan_y,
                        LIGHT_FOG_PAN_BIAS,
                        layer.u_scale,
                        layer.v_scale,
                    );
                    draw_flags |= DF_LIGHT_MAP;
                    ([u_mult, v_mult, u_pan, v_pan], Some(layer.cache_id))
                }
                None => ([0.0; 4], None),
            },
            None => ([0.0; 4], None),
        };
        let (fog_map_uv, fog_map_cache_id) = match request.fog_map.as_ref() {
            Some(layer) => match self.texture_cache.get(layer.cache_id) {
                Some(cached) => {
                    let (u_mult, v_mult) = texture::texture_uv_mult(
                        layer.u_scale,
                        layer.v_scale,
                        cached.width,
                        cached.height,
                    );
                    let (u_pan, v_pan) = texture::texture_uv_pan(
                        layer.pan_x,
                        layer.pan_y,
                        LIGHT_FOG_PAN_BIAS,
                        layer.u_scale,
                        layer.v_scale,
                    );
                    draw_flags |= DF_FOG_MAP;
                    ([u_mult, v_mult, u_pan, v_pan], Some(layer.cache_id))
                }
                None => ([0.0; 4], None),
            },
            None => ([0.0; 4], None),
        };

        let surface = QueuedComplexSurface {
            vertices: vertices.to_vec(),
            diffuse_uv,
            light_map_uv,
            fog_map_uv,
            diffuse_info: draw::compute_complex_diffuse_info(
                request.texture_fields,
                request.c_alpha,
            ),
            x_axis: request.x_axis,
            y_axis: request.y_axis,
            draw_flags,
        };
        match self.batches.last_mut() {
            Some(Batch {
                segment: last_segment,
                payload:
                    BatchPayload::Complex {
                        diffuse_cache_id,
                        light_map_cache_id: last_light_map_cache_id,
                        fog_map_cache_id: last_fog_map_cache_id,
                        surfaces,
                    },
            }) if *last_segment == segment
                && *diffuse_cache_id == request.diffuse.cache_id
                && *last_light_map_cache_id == light_map_cache_id
                && *last_fog_map_cache_id == fog_map_cache_id =>
            {
                surfaces.push(surface);
            }
            _ => {
                self.batches.push(Batch {
                    segment,
                    payload: BatchPayload::Complex {
                        diffuse_cache_id: request.diffuse.cache_id,
                        light_map_cache_id,
                        fog_map_cache_id,
                        surfaces: vec![surface],
                    },
                });
            }
        }
    }

    /// Real `SetTexture.cpp:508-515`'s own forced flush, read
    /// directly before writing this (`Shaders[ActiveProgram]->
    /// Flush(false);`, issued whenever a real `SetTexture` call is
    /// about to rebind or re-upload a texture, specifically *because*
    /// "any draw calls we've already batched up but not yet issued
    /// were built expecting the OLD binding/contents" -- that
    /// function's own comment, quoted exactly). This crate's own
    /// `BatchPayload` stores a `cache_id`, resolved against `self.
    /// texture_cache` only later, at actual dispatch time inside
    /// `dispatch_batches` (`unlock_frame`'s own doc has the full real
    /// reasoning for why that deferred-resolution design exists at
    /// all: real call-order compositing across primitive kinds).
    /// Without this function, a real texture re-upload mid-frame
    /// (`Info.bRealtimeChanged`, `upload_texture`/`upload_compressed_
    /// texture`'s own call sites below) would silently let an
    /// EARLIER-queued batch retroactively pick up the LATER re-
    /// upload's new content merely because both share a `cache_id` --
    /// caught on review before `Info.bRealtimeChanged`-triggered re-
    /// upload was ever wired through any real caller, not discovered
    /// after it shipped. Dispatches (and submits) every currently-
    /// queued batch using whatever `self.texture_cache` holds *right
    /// now*, using the SAME real call-order-preserving `dispatch_
    /// batches` every other real flush point uses, then replaces
    /// `self.in_progress`'s own `CommandEncoder` with a fresh one so
    /// the frame stays open for more draws after -- unlike `unlock_
    /// frame`, never finishes/presents the frame itself, and never
    /// touches `self.in_progress` at all beyond that swap (still
    /// `Some` afterward, exactly as it was before this call, matching
    /// the real engine's own `Flush` not ending the real frame
    /// either). A no-op when no frame is currently in progress
    /// (nothing could have been queued without one -- every real
    /// `queue_*` method already bails on `self.in_progress.is_none()`)
    /// or nothing is actually queued (`self.batches.is_empty()`).
    fn flush_pending_batches(&mut self, created: &CreatedDevice) {
        if self.batches.is_empty() {
            return;
        }
        let Some(mut frame) = self.in_progress.take() else {
            return;
        };
        let batches = std::mem::take(&mut self.batches);
        dispatch_batches(created, self, &mut frame, batches);
        let fresh_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("hp2-wgpu mid-frame flush encoder"),
                });
        let finished_encoder = std::mem::replace(&mut frame.encoder, fresh_encoder);
        created
            .queue
            .submit(std::iter::once(finished_encoder.finish()));
        self.in_progress = Some(frame);
    }

    /// Uploads `rgba8_bytes` into `self.texture_cache` under
    /// `cache_id` -- thin passthrough to `texture::TextureCache::
    /// upload_rgba8` (that method's own doc has the full real
    /// contract and panics). A real future `SetTexture`-equivalent
    /// FFI wrapper calls this before any `queue_tile` call that
    /// references the same `cache_id` can find it. Flushes `self.
    /// batches` first (`flush_pending_batches`'s own doc has the full
    /// real reasoning) whenever `cache_id` is already resident -- a
    /// real re-upload (`Info.bRealtimeChanged`), not a first-ever
    /// upload, the one real case that actually needs it: a brand-new
    /// `cache_id` cannot retroactively affect any earlier-queued
    /// batch, since no earlier batch could possibly already reference
    /// a `cache_id` that did not exist in `self.texture_cache` yet.
    pub fn upload_texture(
        &mut self,
        created: &CreatedDevice,
        cache_id: u64,
        width: u32,
        height: u32,
        rgba8_bytes: &[u8],
        srgb: bool,
    ) {
        if self.texture_cache.get(cache_id).is_some() {
            self.flush_pending_batches(created);
        }
        self.texture_cache
            .upload_rgba8(created, cache_id, width, height, rgba8_bytes, srgb);
    }

    /// Uploads `mips` (a real full `Info.Mips[0..NumMips)` chain) into
    /// `self.texture_cache` under `cache_id` -- thin passthrough to
    /// `texture::TextureCache::upload_compressed` (that method's own
    /// doc, and `texture::create_compressed_gpu_texture`'s own doc,
    /// have the full real contract, including the real `None`-on-
    /// unsupported-hardware case). A real future FFI wrapper calls
    /// this before any `queue_tile`/`queue_complex_surface` call that
    /// references the same `cache_id` can find it, same real ordering
    /// requirement `upload_texture`'s own doc already establishes.
    /// Flushes `self.batches` first under the same real condition
    /// `upload_texture`'s own doc already establishes (`cache_id`
    /// already resident). Returns whether the upload actually
    /// happened -- unlike `upload_texture`, this one has a real
    /// failure mode (`upload_compressed` can return `None`), so unlike
    /// that method this one does not discard its own callee's result
    /// (caught on review before landing: an earlier version of this
    /// method returned `()` the same way `upload_texture` does, which
    /// would have silently told a real FFI caller "uploaded" even when
    /// the real device lacks `Features::TEXTURE_COMPRESSION_BC` and
    /// nothing was actually cached).
    pub fn upload_compressed_texture(
        &mut self,
        created: &CreatedDevice,
        cache_id: u64,
        width: u32,
        height: u32,
        mips: &[texture::CompressedMipLevel<'_>],
        srgb: bool,
    ) -> bool {
        if self.texture_cache.get(cache_id).is_some() {
            self.flush_pending_batches(created);
        }
        self.texture_cache
            .upload_compressed(created, cache_id, width, height, mips, srgb)
            .is_some()
    }

    /// Real `SetTexture.cpp:502`'s own fast-path bailout check, the
    /// caller-facing half: lets a future FFI wrapper skip the real
    /// (potentially expensive -- P8-palette expansion, format
    /// conversion) texture-data extraction work entirely when
    /// `cache_id` is already resident, the same real condition that
    /// lets `SetTexture` itself return immediately without reaching
    /// its own `UploadTexture` call. Does not detect staleness
    /// (`IsTextureDataStale`'s own real counterpart) -- a resident
    /// `cache_id` is always treated as still fresh; re-upload of an
    /// already-cached, realtime-changing texture is `TextureCache`'s
    /// own separately tracked, not-yet-done item (that struct's own
    /// doc has the full reasoning).
    pub fn is_texture_cached(&self, cache_id: u64) -> bool {
        self.texture_cache.get(cache_id).is_some()
    }

    /// Real per-tick entry point `UWgpuRenderDevice::SetSceneNode`
    /// calls through `hp2_rs_wgpu_set_scene_node` (wired, not a
    /// future plan -- an earlier version of this doc said "not wired
    /// to any FFI function or C++ call site yet", true when written,
    /// now stale). Computes `FrameUniforms` from `scene` and appends
    /// a *new* real segment buffer (`RenderLoop::frame_uniforms_
    /// segments`'s own doc has the full real bug this fixes and why a
    /// small buffer per segment, not dynamic offsets) -- every call
    /// gets its own buffer, never reusing a previous one, so later
    /// `queue_line`/`queue_tile` calls this same real frame correctly
    /// bind the matrices that were actually current when each was
    /// queued, not whatever `SetSceneNode` call happened to run last.
    /// Unlike the real GLSL source's own change-detection
    /// (`StoredGamma != ...`, `StoredOrthoFX != ...`, etc.,
    /// `SetSceneNode`'s own early-out when nothing relevant changed),
    /// this always recomputes and always writes -- a known, non-
    /// urgent performance gap (one extra small buffer allocation per
    /// call even when nothing relevant changed), not a correctness
    /// one.
    pub fn set_scene_node(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &SceneFrame,
    ) {
        let uniforms = compute_frame_uniforms(scene);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hp2-wgpu frame uniforms segment"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            // COPY_SRC: lets this crate's own
            // set_scene_node_creates_and_writes_a_real_uniform_buffer
            // test (and any future debug-readback tooling) copy this
            // buffer's real contents out for verification --
            // negligible cost on a 528-byte buffer, and an earlier
            // version without it made that test's own
            // copy_buffer_to_buffer call fail wgpu's real validation
            // (confirmed by actually running it, not assumed: "Usage
            // flags ... do not contain required usage flags
            // BufferUsages(COPY_SRC)").
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        // FrameUniforms now derives bytemuck::Pod/Zeroable (this
        // struct's own doc has the real reasoning) -- bytemuck::
        // bytes_of is the compile-time-checked replacement for an
        // earlier version's hand-rolled unsafe raw-pointer
        // reinterpretation (caught on review).
        let bytes = bytemuck::bytes_of(&uniforms);
        queue.write_buffer(&buffer, 0, bytes);
        self.frame_uniforms_segments.push(buffer);
        self.last_scene = Some(*scene);
    }

    /// How many real `set_scene_node` segments exist for the current
    /// (or most recently cleared) frame -- `0` before the first call
    /// since the last `lock_frame`/`begin_offscreen_frame`.
    pub fn frame_uniforms_segment_count(&self) -> usize {
        self.frame_uniforms_segments.len()
    }

    /// The real segment buffer `set_scene_node` created on its
    /// `index`-th call since the last `lock_frame`/
    /// `begin_offscreen_frame` (`0`-based). `None` if `index` is out
    /// of range -- real callers (tests, future debug-readback
    /// tooling) need to handle that, not assume a given segment
    /// exists.
    pub fn frame_uniforms_segment(&self, index: usize) -> Option<&wgpu::Buffer> {
        self.frame_uniforms_segments.get(index)
    }
}

/// Exact byte-for-byte mirror of `common.wgsl`'s `FrameUniforms` struct
/// (`@group(0) @binding(0)`, shared by every shader) -- 8 `mat4x4<f32>`
/// fields (each `glam::Mat4`: `#[repr(C)]`, 4 consecutive `Vec4`
/// columns, 16-byte aligned, confirmed against glam's own source
/// before relying on it, not assumed) followed by 4 `f32` fields.
/// `#[repr(C)]` with no explicit padding needed: 8*64 = 512 bytes
/// already a multiple of 16, so the four trailing `f32`s need no
/// alignment padding either before or after themselves or the struct
/// as a whole (528 bytes total, still a multiple of 16) -- checked
/// against WGSL's own uniform-address-space layout rules, not
/// assumed identical to Rust's own default `#[repr(C)]` rules by
/// coincidence.
///
/// Derives `bytemuck::Pod`/`Zeroable` (glam's own `bytemuck` feature
/// flag, verified real and exactly named that before enabling it, not
/// assumed from the dependency name alone -- `glam::Mat4: bytemuck::
/// Pod` compiles under it) rather than hand-rolling an `unsafe {
/// std::slice::from_raw_parts(...) }` byte reinterpretation the way
/// an earlier version of `RenderLoop::set_scene_node` did (caught on
/// review): `Copy` alone, which that version relied on, does not rule
/// out padding bytes -- reading them through a `&[u8]` would be real
/// UB if they ever existed, and nothing in a plain `Copy` bound
/// enforces that they don't. `bytemuck`'s `Pod` derive rejects a
/// padded layout at compile time instead of relying on a doc comment
/// asserting there is none.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniforms {
    pub proj_mat: glam::Mat4,
    pub view_mat: glam::Mat4,
    pub model_mat: glam::Mat4,
    pub modelview_mat: glam::Mat4,
    pub modelviewproj_mat: glam::Mat4,
    /// Not yet populated by `compute_frame_uniforms` -- shadow mapping
    /// is a separate, unexamined subsystem (no real GLSL source read
    /// yet for where/how `LightSpaceMat` -- if that is even its real
    /// name, not assumed -- gets built). Always `Mat4::IDENTITY` here
    /// until that subsystem is actually read and ported; stated
    /// honestly rather than silently wrong.
    pub light_space_mat: glam::Mat4,
    pub frame_coords: glam::Mat4,
    pub frame_uncoords: glam::Mat4,
    pub gamma: f32,
    pub light_map_intensity: f32,
    pub light_color_intensity: f32,
    pub y_scale: f32,
}

/// FFI input for `compute_frame_uniforms`, carrying exactly the real
/// `FSceneNode`/engine-state fields `XOpenGL.cpp`'s own
/// `SetSceneNode`/`SetOrthoProjection`/`SetProjection`/`UpdateCoords`
/// read (`HarryPotter2/Unreal/Engine/Inc/UnRender.h`'s real
/// `FSceneNode` struct, confirmed field-by-field, not guessed).
/// `gamma`/`light_color_intensity`/`light_map_intensity` are plain
/// `f32` inputs, not computed here: the real values come from
/// `UXOpenGLRenderDevice::GetViewportGamma`/`ActorXBlending`/
/// `OneXBlending`, which are config/engine-state reads with no
/// graphics-math content -- C++'s own job to read and pass in, not a
/// reason to cross the FFI boundary with a `UViewport*`/`AActor*`
/// pointer. `y_scale` is not a field here at all: the real source's
/// own branch for it (`XOPENGL_WGL && !OLDUNREAL_WINXP`, the DXGI
/// interop swapchain row-order fix) is Windows-GL-specific and
/// unreachable on this macOS-first wgpu driver, so
/// `compute_frame_uniforms` hardcodes the real source's own `#else`
/// value, `1.0`, unconditionally.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneFrame {
    pub coords_origin: [f32; 3],
    pub coords_x_axis: [f32; 3],
    pub coords_y_axis: [f32; 3],
    pub coords_z_axis: [f32; 3],
    pub uncoords_origin: [f32; 3],
    pub uncoords_x_axis: [f32; 3],
    pub uncoords_y_axis: [f32; 3],
    pub uncoords_z_axis: [f32; 3],
    /// `Frame->FX`/`Frame->FY` -- floating-point frame width/height,
    /// not `Viewport->SizeX`/`SizeY` (a real, already-established
    /// distinction elsewhere in this crate's own `Lock`/HiDPI fix).
    pub fx: f32,
    pub fy: f32,
    /// `Frame->EffectiveFovAngle`, read only on the perspective path.
    pub effective_fov_angle: f32,
    /// `Viewport->Actor->FovAngle`, read only on the ortho path
    /// (`SetOrthoProjection`'s own real source reads the raw actor
    /// FOV, not `EffectiveFovAngle`, for its `RProjZ` -- a real,
    /// deliberate difference between the two paths, not an oversight
    /// to unify).
    pub viewport_fov_angle: f32,
    /// `Frame->Viewport->IsOrtho()`.
    pub is_ortho: bool,
    /// `GIsEditor && Frame->Viewport->Actor->RendMap == REN_Wire` --
    /// selects the real source's wireframe-editor `zFar` (131072)
    /// over the normal one (65336).
    pub is_wireframe_zfar: bool,
    pub gamma: f32,
    pub light_color_intensity: f32,
    pub light_map_intensity: f32,
}

/// `zNear` for the perspective (non-ortho) path: the real source's
/// `SetProjection(Frame, bNearZ)` is only ever called as
/// `SetProjection(Frame, 0)` from `SetSceneNode` (the only call site
/// this function replaces), and with neither `UNREAL_TOURNAMENT_
/// OLDUNREAL` nor `bNearZ` set, `zNear` is unconditionally `1.0f` --
/// confirmed by reading `ENGINE_VERSION` (`UnObjVer.h`: `433`) and
/// grepping the whole tree for `UNREAL_TOURNAMENT_OLDUNREAL`, which
/// does not exist anywhere in this codebase (a leftover portability
/// macro from the upstream project this file was pinned from, never
/// defined here), not assumed from the macro's mere presence in the
/// source text.
const PERSPECTIVE_Z_NEAR: f32 = 1.0;

/// Real source's two `zFar` values (`XOpenGL.cpp`'s own `SetOrthoProjection`/
/// `SetProjection`, identical ternary in both).
const Z_FAR_NORMAL: f32 = 65336.0;
const Z_FAR_WIREFRAME: f32 = 131072.0;

/// Replaces the matrix-math half of `UXOpenGLRenderDevice::
/// SetSceneNode`/`SetOrthoProjection`/`SetProjection`/`UpdateCoords`
/// (not yet wired into any FFI call or `WgpuDrv.cpp` override -- see
/// this crate's own module doc, "real draw-call forwarding" item).
/// Pure function, no GPU/device access: `scene`'s values in, a fully
/// populated `FrameUniforms` out, independently testable without an
/// adapter.
///
/// Uses `glam::camera::rh::proj::directx` for both the ortho and
/// frustum matrices, not `vulkan` or `opengl`: `directx` is the
/// module glam itself documents with `#[doc(alias = "webgpu")]` --
/// right-handed,
/// Z range `[0, 1]`, Y-up -- matching the real source's own GLM
/// convention exactly except for the Z range (confirmed no
/// `GLM_FORCE_LEFT_HANDED`/`GLM_FORCE_DEPTH_ZERO_TO_ONE` define
/// anywhere in `ThirdParty/XOpenGLDrv/glm/`, so the real `glm::
/// frustum`/`glm::ortho` calls are GLM's default right-handed, Z
/// range `[-1, 1]` -- the one difference this function's whole job is
/// to correct, per `simple.wgsl`'s own depth-range-contract note).
/// `vulkan` was rejected: it additionally flips Y in clip space, a
/// Vulkan-specific quirk WebGPU does not share at the API level.
///
/// View matrix is the real source's own fixed `glm::scale(mat4(1),
/// vec3(1,-1,-1))` (`XOpenGL.cpp`'s `Init`, set once, never
/// recomputed per frame); model matrix is always identity, matching
/// the real source exactly (never set to anything else anywhere it
/// was read).
pub fn compute_frame_uniforms(scene: &SceneFrame) -> FrameUniforms {
    let view_mat = glam::Mat4::from_scale(glam::Vec3::new(1.0, -1.0, -1.0));
    let model_mat = glam::Mat4::IDENTITY;

    let z_far = if scene.is_wireframe_zfar {
        Z_FAR_WIREFRAME
    } else {
        Z_FAR_NORMAL
    };
    let aspect = scene.fy / scene.fx;

    let proj_mat = if scene.is_ortho {
        let r_proj_z = (scene.viewport_fov_angle * std::f32::consts::PI / 360.0).tan();
        glam::camera::rh::proj::directx::orthographic(
            -r_proj_z,
            r_proj_z,
            -aspect * r_proj_z,
            aspect * r_proj_z,
            -z_far,
            z_far,
        )
    } else {
        let r_proj_z = (scene.effective_fov_angle * std::f32::consts::PI / 360.0).tan();
        let z_near = PERSPECTIVE_Z_NEAR;
        glam::camera::rh::proj::directx::frustum(
            -r_proj_z * z_near,
            r_proj_z * z_near,
            -aspect * r_proj_z * z_near,
            aspect * r_proj_z * z_near,
            z_near,
            z_far,
        )
    };

    let modelview_mat = view_mat * model_mat;
    let modelviewproj_mat = proj_mat * modelview_mat;

    // UpdateCoords: FrameCoords[i]/FrameUncoords[i] are glm columns,
    // i.e. mat4::from_cols in glam's own column-major convention --
    // not rows, matching `glm::mat4`'s own real layout.
    let frame_coords = glam::Mat4::from_cols(
        glam::Vec3::from(scene.coords_origin).extend(0.0),
        glam::Vec3::from(scene.coords_x_axis).extend(0.0),
        glam::Vec3::from(scene.coords_y_axis).extend(0.0),
        glam::Vec3::from(scene.coords_z_axis).extend(0.0),
    );
    let frame_uncoords = glam::Mat4::from_cols(
        glam::Vec3::from(scene.uncoords_origin).extend(0.0),
        glam::Vec3::from(scene.uncoords_x_axis).extend(0.0),
        glam::Vec3::from(scene.uncoords_y_axis).extend(0.0),
        glam::Vec3::from(scene.uncoords_z_axis).extend(0.0),
    );

    FrameUniforms {
        proj_mat,
        view_mat,
        model_mat,
        modelview_mat,
        modelviewproj_mat,
        light_space_mat: glam::Mat4::IDENTITY,
        frame_coords,
        frame_uncoords,
        gamma: scene.gamma,
        light_map_intensity: scene.light_map_intensity,
        light_color_intensity: scene.light_color_intensity,
        y_scale: 1.0,
    }
}

/// Shared real `RFX2`/`RFY2` derivation (`XOpenGL.cpp:1821-1823`
/// ortho / `1868-1870` perspective -- both branches compute them the
/// same way from their own `RProjZ`/`Aspect`, matching
/// `compute_frame_uniforms`'s own `r_proj_z`/`aspect` derivation
/// exactly), factored out of `draw_2d_line_screen_to_coords` so
/// `queue_tile`'s own real quad-corner geometry can reuse the exact
/// same real per-segment values rather than duplicating the formula
/// -- both real call shapes (`Draw2DLine`'s two endpoints, `DrawTile`'s
/// four corners) apply the identical `RFX2 * Z * (coord - FX2)`
/// transform using the same cached-per-`SetSceneNode` `RFX2`/`RFY2`.
fn scene_rfx2_rfy2(scene: &SceneFrame) -> (f32, f32) {
    let aspect = scene.fy / scene.fx;
    let r_proj_z = if scene.is_ortho {
        (scene.viewport_fov_angle * std::f32::consts::PI / 360.0).tan()
    } else {
        (scene.effective_fov_angle * std::f32::consts::PI / 360.0).tan()
    };
    (
        2.0 * r_proj_z / scene.fx,
        2.0 * r_proj_z * aspect / scene.fy,
    )
}

/// Real `Draw2DLine`/`Draw2DPoint`/`Draw2DClippedLine`'s own shared
/// screen-to-clip transform (`DrawSimple.cpp:80,82` for `Draw2DLine`
/// itself; `187-197` for `Draw2DClippedLine`, same formula with a
/// constant 0.5px inset rather than a per-call parameter -- that
/// variant is not implemented by this function, only the plain
/// `Draw2DLine` shape). `p` is `(pixel X, pixel Y, view-space Z)`,
/// exactly `Draw2DLine`'s own real `P1`/`P2` `FVector` parameters.
///
/// `RFX2`/`RFY2` (`XOpenGL.cpp:1821-1823` ortho / `1868-1870`
/// perspective -- both branches compute them the same way from their
/// own `RProjZ`/`Aspect`, matching `compute_frame_uniforms`'s own
/// `r_proj_z`/`aspect` derivation exactly, not duplicated with a
/// different formula, now shared via `scene_rfx2_rfy2` above) are
/// real `UXOpenGLRenderDevice` *member* fields, cached once per
/// `SetSceneNode` and reused by every `Draw2DLine` call that frame --
/// `scene` (the caller's own cached `self.last_scene`) is the right
/// source for those two specifically.
///
/// `fx2`/`fy2` are different: the real source reads them fresh off
/// `Frame->FX2`/`FY2` (`UnCamera.cpp:351-352`: `FX2 = FX * 0.5f`, `FY2
/// = FY * 0.5f`) on the *`FSceneNode` `Draw2DLine` itself received*
/// (`DrawSimple.cpp:80,82`), not from any `UXOpenGLRenderDevice`
/// member field -- nothing in the real source guarantees that frame
/// is the same one the last `SetSceneNode` call used. An earlier
/// version of this function derived `fx2`/`fy2` from `scene.fx`/`fy`
/// internally (the same cached scene `rfx2`/`rfy2` come from),
/// silently assuming the two always coincide -- a real, unstated
/// assumption, caught on review, not something the real source
/// itself relies on or guarantees. Callers must pass the real `Frame->
/// FX2`/`FY2` from the actual `Draw2DLine` call site (`WgpuDrv.cpp`'s
/// own override reads them directly off its own `Frame` parameter),
/// not derive them from whatever scene happens to be cached.
pub fn draw_2d_line_screen_to_coords(
    scene: &SceneFrame,
    fx2: f32,
    fy2: f32,
    p: [f32; 3],
) -> [f32; 3] {
    let (rfx2, rfy2) = scene_rfx2_rfy2(scene);
    [rfx2 * p[2] * (p[0] - fx2), rfy2 * p[2] * (p[1] - fy2), p[2]]
}

/// Real `Draw2DPoint`'s own Z adjustment (`DrawSimple.cpp:170-173`:
/// `if (Frame->Viewport->IsOrtho()) Z = 1.f; else if (Z < 0.f) Z =
/// -Z;`), applied *before* `draw::point_quad_corners`'s own per-
/// corner transform -- a real, separate step `Draw2DLine`'s own
/// formula has no equivalent of at all (confirmed by direct re-read
/// of both functions, not assumed symmetric). `is_ortho` is `scene.
/// is_ortho` (the real `Frame->Viewport->IsOrtho()` value `SetSceneNode`
/// already cached for this segment, same source `draw_2d_line_screen_
/// to_coords`'s own ortho/perspective branch uses).
fn resolve_point_z(is_ortho: bool, z: f32) -> f32 {
    if is_ortho {
        1.0
    } else if z < 0.0 {
        -z
    } else {
        z
    }
}

/// Replaces the per-frame-begin half of `UWgpuRenderDevice::Lock`
/// (plan's still-open "real draw-call forwarding" item's first piece:
/// nothing is drawn by this function itself, but it is the real
/// per-tick entry point any future draw-call forwarding will record
/// into). Reconfigures the surface only when `width`/`height` differ
/// from the last configured size (`loop_state.configured_size`) --
/// cheap on every other call, including every call at a steady
/// resolution, which is the common case. Begins a render pass that
/// clears to `clear_color` and records nothing else; the pass is
/// ended (not `unlock_frame`'s concern -- `begin_render_pass` returning
/// a pass borrowing `encoder` would make storing both in
/// `InProgressFrame` a self-referential-struct problem) before this
/// function returns, so a draw-call forwarder landing later will need
/// its own render pass, not this clear pass still open.
///
/// `Outdated` additionally clears `loop_state.configured_size`, so the
/// *next* `lock_frame` call reconfigures rather than repeating the
/// same stale configuration -- `Surface::get_current_texture`'s own
/// doc says exactly that ("Call Surface::configure() and try again")
/// for this code.
///
/// Fails (and leaves `loop_state.in_progress` untouched, i.e. `None`
/// or whatever a previous successful `lock_frame` left it as) on a
/// non-`None` surface whose format/present-mode list is empty, or on
/// `get_current_texture` returning anything other than `Success`/
/// `Suboptimal` -- see `FrameOutcome`'s own doc for what each code
/// means. Every outcome is tallied into `loop_state.counts()`.
pub fn lock_frame(
    created: &CreatedDevice,
    loop_state: &mut RenderLoop,
    width: u32,
    height: u32,
    clear_color: wgpu::Color,
    present_mode: wgpu::PresentMode,
) -> FrameOutcome {
    // Cleared here, regardless of this call's own outcome below --
    // segments/queued draws are strictly per-frame (RenderLoop::
    // frame_uniforms_segments's own doc has the full real reasoning),
    // so a stale leftover from a frame whose own unlock_frame was
    // never reached (e.g. a skipped Unlock) must never leak into the
    // next one.
    loop_state.frame_uniforms_segments.clear();
    loop_state.batches.clear();
    let Some(surface) = created.surface.as_ref() else {
        loop_state.counts.other_failure += 1;
        return FrameOutcome::NoSurface;
    };
    // Real UseVSync ini key support: resolve_present_mode's own doc
    // has the full reasoning.
    let present_mode = resolve_present_mode(
        present_mode,
        &surface.get_capabilities(&created.adapter).present_modes,
    );
    if loop_state.configured_size != Some((width, height))
        || loop_state.configured_present_mode != Some(present_mode)
    {
        let Some(mut config) = surface.get_default_config(&created.adapter, width, height) else {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::NoDefaultConfig;
        };
        config.present_mode = present_mode;
        // Capture reads the presented surface texture back, which needs
        // COPY_SRC on the swapchain textures -- requested only when
        // capture is actually on and the surface really supports it
        // (not every backend does), so the normal path is unchanged.
        loop_state.ensure_capture_initialized();
        if loop_state
            .capture
            .as_ref()
            .is_some_and(capture::FrameCapture::is_enabled)
            && surface
                .get_capabilities(&created.adapter)
                .usages
                .contains(wgpu::TextureUsages::COPY_SRC)
        {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        surface.configure(&created.device, &config);
        loop_state.configured_size = Some((width, height));
        loop_state.configured_present_mode = Some(present_mode);
    }
    let (texture, is_suboptimal) = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture) => (texture, false),
        wgpu::CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
        wgpu::CurrentSurfaceTexture::Timeout => {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::Timeout;
        }
        wgpu::CurrentSurfaceTexture::Occluded => {
            loop_state.counts.occluded += 1;
            // Same format `get_default_config` picks (`formats[0]`), so a
            // captured occluded frame matches a captured visible one.
            if loop_state
                .capture
                .as_ref()
                .is_some_and(capture::FrameCapture::is_enabled)
                && let Some(&format) = surface.get_capabilities(&created.adapter).formats.first()
            {
                begin_capture_only_frame(created, loop_state, width, height, format, clear_color);
            }
            return FrameOutcome::Occluded;
        }
        wgpu::CurrentSurfaceTexture::Outdated => {
            loop_state.configured_size = None;
            loop_state.counts.outdated += 1;
            return FrameOutcome::Outdated;
        }
        wgpu::CurrentSurfaceTexture::Lost => {
            // Same reconfigure-on-next-call treatment as Outdated:
            // without this, a later Lock at the same (width, height)
            // would skip surface.configure (since the cached size
            // still matches) and keep getting Lost forever instead of
            // ever recovering -- e.g. after a display or Space change,
            // or a fullscreen toggle. Missing here was a real bug, not
            // just an inconsistency with Outdated's own handling
            // (caught on review).
            loop_state.configured_size = None;
            loop_state.counts.other_failure += 1;
            return FrameOutcome::Lost;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::Validation;
        }
    };
    let color_format = texture.texture.format();
    let view = texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = created
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("hp2-wgpu frame encoder"),
        });
    {
        let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hp2-wgpu clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    loop_state.in_progress = Some(InProgressFrame {
        presentable: FramePresentable::Surface(texture),
        view,
        encoder,
        color_format,
        suboptimal: is_suboptimal,
    });
    // Not counted as `presented` here: a successful lock_frame only
    // means a frame was *acquired*, not that it will actually reach
    // the screen -- unlock_frame(present: false) (Blit=0, e.g. a
    // hit-testing pass) never presents it at all. The real `presented`
    // tally belongs in unlock_frame, counting only calls that actually
    // present (caught on review: counting here would have
    // over-counted relative to what Build/run_wgpu_smoke.py's
    // presented>0 check is meant to mean).
    FrameOutcome::Presented
}

/// Real `UseVSync` ini key support (`lock_frame`'s own real caller).
/// `wgpu::PresentMode::Fifo` is the only mode wgpu itself guarantees
/// is supported on every surface/adapter (its own doc: "Supported
/// on: All platforms") -- `Immediate`/`FifoRelaxed`/`Mailbox` are
/// not, so blindly requesting a caller's chosen mode would be a real
/// `wgpu::Surface::configure` validation error on a surface whose own
/// real capabilities do not list it. Falls back to `Fifo` (VSync On)
/// when `requested` is not in `supported` -- an unsupported present
/// mode is a real, reachable hardware/driver condition, not a caller
/// bug, so this never panics. A separate, pure function (not inlined
/// into `lock_frame` itself) specifically so this real decision is
/// unit-testable without a live `wgpu::Surface` -- `lock_frame`
/// itself needs a real OS window, unavailable to a Rust `#[test]` on
/// macOS (this session's own established constraint), but this
/// function's own `supported` parameter can be any real or
/// deliberately-constructed slice.
pub fn resolve_present_mode(
    requested: wgpu::PresentMode,
    supported: &[wgpu::PresentMode],
) -> wgpu::PresentMode {
    if supported.contains(&requested) {
        requested
    } else {
        wgpu::PresentMode::Fifo
    }
}

/// Real `lock_frame`/`unlock_frame` encoder-and-draw-queue machinery,
/// exercised against a caller-owned offscreen `&wgpu::Texture`
/// instead of acquiring one from a real surface -- the real
/// production entry point for every actual `WgpuDrv.cpp` call site is
/// still `lock_frame` alone; this exists only so this crate's own
/// tests can drive the *real* `RenderLoop` code (not a hand-written
/// replica of its reasoning) while no real OS window is available
/// (this session's own established constraint: a Rust `#[test]`
/// cannot create one on macOS). Always succeeds (no `Occluded`/
/// `Timeout`/etc. to report -- those are real `Surface::
/// get_current_texture` outcomes an offscreen `wgpu::Texture` has no
/// equivalent of) and always marks the frame non-suboptimal (no
/// surface to reconfigure). Panics (`debug_assert!`-free, a real
/// `assert!`, since a caller passing a non-`RENDER_ATTACHMENT`
/// texture is a real programming error, not a runtime condition) if
/// `target` was not created with `TextureUsages::RENDER_ATTACHMENT`.
pub fn begin_offscreen_frame(
    created: &CreatedDevice,
    loop_state: &mut RenderLoop,
    target: &wgpu::Texture,
    clear_color: wgpu::Color,
) {
    assert!(
        target
            .usage()
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
        "begin_offscreen_frame's target must be created with TextureUsages::RENDER_ATTACHMENT"
    );
    // Same real per-frame reset lock_frame itself does -- see its own
    // comment at the equivalent point for the full reasoning.
    loop_state.frame_uniforms_segments.clear();
    loop_state.batches.clear();
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = created
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("hp2-wgpu offscreen frame encoder"),
        });
    {
        let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hp2-wgpu offscreen clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    loop_state.in_progress = Some(InProgressFrame {
        presentable: FramePresentable::Offscreen,
        view,
        encoder,
        color_format: target.format(),
        suboptimal: false,
    });
}

/// `FramePresentable::CaptureOnly`'s frame start: (re)uses
/// `loop_state.capture_target` at `width`x`height`/`format`, then
/// begins the frame into it exactly like `begin_offscreen_frame`.
fn begin_capture_only_frame(
    created: &CreatedDevice,
    loop_state: &mut RenderLoop,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    clear_color: wgpu::Color,
) {
    let target = match loop_state.capture_target.take() {
        Some(texture)
            if texture.width() == width
                && texture.height() == height
                && texture.format() == format =>
        {
            texture
        }
        _ => created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hp2-wgpu occluded capture target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        }),
    };
    begin_offscreen_frame(created, loop_state, &target, clear_color);
    if let Some(frame) = loop_state.in_progress.as_mut() {
        frame.presentable = FramePresentable::CaptureOnly(target.clone());
    }
    loop_state.capture_target = Some(target);
}

/// Dispatches `batches` into `frame.encoder`, lazily building
/// whatever pipelines/sampler/`dummy_texture` they need, in real push
/// order (`Batch`'s own doc has the full real citation for why real
/// call order, not a per-primitive-kind fixed order, is required).
/// Does not submit `frame.encoder` or touch `loop_state.in_progress`
/// at all -- shared by two real callers with two different real
/// submit/frame-lifetime needs of their own: `unlock_frame` (frame-
/// end, submits once itself afterward) and `RenderLoop::
/// flush_pending_batches` (mid-frame, submits and replaces `frame.
/// encoder` with a fresh one itself afterward, keeping the frame
/// open for more draws). A `Tiles`/`Triangles`/`Complex` batch
/// referencing a `cache_id` that was never (or not yet) uploaded into
/// `loop_state.texture_cache` is skipped cleanly (not a panic) -- a
/// real, reachable transient condition, matching `queue_tile`'s own
/// doc. Each chunk's own real `DrawID`s are re-indexed to start at
/// `0`, chunked at `draw::MAX_DRAWS` (matching the real GLSL
/// batcher's own `Flush(true)`-when-full behavior, `DrawSimple.cpp:
/// 68-69`/`DrawTile.cpp:188-197`, no line or tile is ever lost).
fn dispatch_batches(
    created: &CreatedDevice,
    loop_state: &mut RenderLoop,
    frame: &mut InProgressFrame,
    batches: Vec<Batch>,
) {
    let needs_line_pipeline = batches
        .iter()
        .any(|b| matches!(b.payload, BatchPayload::Lines(_)))
        && !matches!(
            &loop_state.line_pipeline,
            Some((format, _)) if *format == frame.color_format
        );
    if needs_line_pipeline {
        loop_state.line_pipeline = Some((
            frame.color_format,
            draw::SimpleLinePipeline::new(&created.device, frame.color_format),
        ));
    }
    let needs_tile_pipeline = batches
        .iter()
        .any(|b| matches!(b.payload, BatchPayload::Tiles { .. }))
        && !matches!(
            &loop_state.tile_pipeline,
            Some((format, _)) if *format == frame.color_format
        );
    if needs_tile_pipeline {
        loop_state.tile_pipeline = Some((
            frame.color_format,
            draw::TilePipeline::new(&created.device, frame.color_format),
        ));
    }
    if loop_state.sampler.is_none()
        && batches.iter().any(|b| {
            matches!(
                b.payload,
                BatchPayload::Tiles { .. }
                    | BatchPayload::Triangles { .. }
                    | BatchPayload::Complex { .. }
            )
        })
    {
        loop_state.sampler = Some(
            created
                .device
                .create_sampler(&wgpu::SamplerDescriptor::default()),
        );
    }
    // Real 1x1 opaque white dummy texture (RenderLoop::dummy_texture's
    // own doc has the full real reasoning) -- only actually needed
    // when some real Complex batch is missing a real light-map or
    // fog-map binding (light_map_cache_id/fog_map_cache_id is None
    // for whichever of the two is absent -- a batch can have either,
    // both, or neither None).
    if loop_state.dummy_texture.is_none()
        && batches.iter().any(|b| {
            matches!(
                &b.payload,
                BatchPayload::Complex {
                    light_map_cache_id: None,
                    ..
                } | BatchPayload::Complex {
                    fog_map_cache_id: None,
                    ..
                }
            )
        })
    {
        loop_state.dummy_texture = Some(texture::create_gpu_texture(
            created,
            1,
            1,
            &[255, 255, 255, 255],
            false,
        ));
    }
    let needs_point_pipeline = batches
        .iter()
        .any(|b| matches!(b.payload, BatchPayload::Points(_)))
        && !matches!(
            &loop_state.point_pipeline,
            Some((format, _)) if *format == frame.color_format
        );
    if needs_point_pipeline {
        loop_state.point_pipeline = Some((
            frame.color_format,
            draw::SimpleTrianglePipeline::new(&created.device, frame.color_format),
        ));
    }
    let needs_gouraud_pipeline = batches
        .iter()
        .any(|b| matches!(b.payload, BatchPayload::Triangles { .. }))
        && !matches!(
            &loop_state.gouraud_pipeline,
            Some((format, _)) if *format == frame.color_format
        );
    if needs_gouraud_pipeline {
        loop_state.gouraud_pipeline = Some((
            frame.color_format,
            draw::GouraudPipeline::new(&created.device, frame.color_format),
        ));
    }
    let needs_complex_pipeline = batches
        .iter()
        .any(|b| matches!(b.payload, BatchPayload::Complex { .. }))
        && !matches!(
            &loop_state.complex_pipeline,
            Some((format, _)) if *format == frame.color_format
        );
    if needs_complex_pipeline {
        loop_state.complex_pipeline = Some((
            frame.color_format,
            draw::ComplexPipeline::new(&created.device, frame.color_format),
        ));
    }

    for batch in &batches {
        let buffer = &loop_state.frame_uniforms_segments[batch.segment];
        match &batch.payload {
            BatchPayload::Lines(lines) => {
                let (_, pipeline) = loop_state
                    .line_pipeline
                    .as_ref()
                    .expect("line_pipeline was just ensured above");
                let mut vertices = Vec::new();
                let mut draw_params: Vec<draw::DrawParams> = Vec::new();
                for line in lines {
                    // Real GLSL batcher's own Flush(true)-when-full
                    // behavior (DrawSimple.cpp:68-69) -- chunk, never
                    // drop; queue_line's own doc has the full
                    // reasoning.
                    if draw_params.len() >= draw::MAX_DRAWS {
                        pipeline.record(
                            &created.device,
                            &created.queue,
                            &mut frame.encoder,
                            draw::LineDrawCall {
                                frame_uniforms_buffer: buffer,
                                vertices: &vertices,
                                draw_params: &draw_params,
                                target: &frame.view,
                                load: wgpu::LoadOp::Load,
                            },
                        );
                        vertices.clear();
                        draw_params.clear();
                    }
                    let draw_id = draw_params.len() as u32;
                    vertices.push(draw::LineVertex {
                        coords: line.p1,
                        draw_id,
                    });
                    vertices.push(draw::LineVertex {
                        coords: line.p2,
                        draw_id,
                    });
                    draw_params.push(draw::DrawParams {
                        draw_color: line.color,
                    });
                }
                if !draw_params.is_empty() {
                    pipeline.record(
                        &created.device,
                        &created.queue,
                        &mut frame.encoder,
                        draw::LineDrawCall {
                            frame_uniforms_buffer: buffer,
                            vertices: &vertices,
                            draw_params: &draw_params,
                            target: &frame.view,
                            load: wgpu::LoadOp::Load,
                        },
                    );
                }
            }
            BatchPayload::Tiles { cache_id, entries } => {
                // A cache_id never (or not yet) uploaded is a real,
                // reachable transient condition -- queue_tile's own
                // doc has the full reasoning -- dropped cleanly, not
                // a panic.
                let Some(cached) = loop_state.texture_cache.get(*cache_id) else {
                    continue;
                };
                let (_, pipeline) = loop_state
                    .tile_pipeline
                    .as_ref()
                    .expect("tile_pipeline was just ensured above");
                let sampler = loop_state
                    .sampler
                    .as_ref()
                    .expect("sampler was just ensured above");
                let mut vertices: Vec<draw::TileVertex> = Vec::new();
                let mut draw_params: Vec<draw::TileDrawParams> = Vec::new();
                for tile in entries {
                    // Same real chunk-not-drop reasoning as the
                    // Lines arm above, this time matching DrawTile.
                    // cpp:188-197's own CanBuffer/Flush check.
                    if draw_params.len() >= draw::MAX_DRAWS {
                        pipeline.record(
                            &created.device,
                            &created.queue,
                            &mut frame.encoder,
                            draw::TileDrawCall {
                                frame_uniforms_buffer: buffer,
                                vertices: &vertices,
                                draw_params: &draw_params,
                                texture_view: &cached.view,
                                sampler,
                                target: &frame.view,
                                load: wgpu::LoadOp::Load,
                            },
                        );
                        vertices.clear();
                        draw_params.clear();
                    }
                    let draw_id = draw_params.len() as u32;
                    let draw::TileQuadCorners {
                        top_left,
                        top_right,
                        bottom_right,
                        bottom_left,
                    } = tile.corners;
                    // Real DrawTile.cpp:219-241's own two-triangle,
                    // shared-diagonal layout: top_left/bottom_right
                    // each appear in both triangles.
                    vertices.push(draw::TileVertex {
                        coords: top_left.0,
                        draw_id,
                        tex_coords: top_left.1,
                    });
                    vertices.push(draw::TileVertex {
                        coords: top_right.0,
                        draw_id,
                        tex_coords: top_right.1,
                    });
                    vertices.push(draw::TileVertex {
                        coords: bottom_right.0,
                        draw_id,
                        tex_coords: bottom_right.1,
                    });
                    vertices.push(draw::TileVertex {
                        coords: top_left.0,
                        draw_id,
                        tex_coords: top_left.1,
                    });
                    vertices.push(draw::TileVertex {
                        coords: bottom_right.0,
                        draw_id,
                        tex_coords: bottom_right.1,
                    });
                    vertices.push(draw::TileVertex {
                        coords: bottom_left.0,
                        draw_id,
                        tex_coords: bottom_left.1,
                    });
                    draw_params.push(draw::TileDrawParams {
                        draw_color: tile.draw_color,
                        draw_flags: tile.draw_flags,
                        _padding: [0; 3],
                    });
                }
                if !draw_params.is_empty() {
                    pipeline.record(
                        &created.device,
                        &created.queue,
                        &mut frame.encoder,
                        draw::TileDrawCall {
                            frame_uniforms_buffer: buffer,
                            vertices: &vertices,
                            draw_params: &draw_params,
                            texture_view: &cached.view,
                            sampler,
                            target: &frame.view,
                            load: wgpu::LoadOp::Load,
                        },
                    );
                }
            }
            BatchPayload::Points(points) => {
                let (_, pipeline) = loop_state
                    .point_pipeline
                    .as_ref()
                    .expect("point_pipeline was just ensured above");
                let mut vertices: Vec<draw::LineVertex> = Vec::new();
                let mut draw_params: Vec<draw::DrawParams> = Vec::new();
                for point in points {
                    // Same real chunk-not-drop reasoning as the
                    // Lines/Tiles arms above.
                    if draw_params.len() >= draw::MAX_DRAWS {
                        pipeline.record(
                            &created.device,
                            &created.queue,
                            &mut frame.encoder,
                            draw::LineDrawCall {
                                frame_uniforms_buffer: buffer,
                                vertices: &vertices,
                                draw_params: &draw_params,
                                target: &frame.view,
                                load: wgpu::LoadOp::Load,
                            },
                        );
                        vertices.clear();
                        draw_params.clear();
                    }
                    let draw_id = draw_params.len() as u32;
                    let draw::PointQuadCorners {
                        top_left,
                        top_right,
                        bottom_right,
                        bottom_left,
                    } = point.corners;
                    // Same real two-triangle, shared-diagonal layout
                    // as the Tiles arm's own TileQuadCorners (no
                    // tex_coords here -- LineVertex has none).
                    vertices.push(draw::LineVertex {
                        coords: top_left,
                        draw_id,
                    });
                    vertices.push(draw::LineVertex {
                        coords: top_right,
                        draw_id,
                    });
                    vertices.push(draw::LineVertex {
                        coords: bottom_right,
                        draw_id,
                    });
                    vertices.push(draw::LineVertex {
                        coords: top_left,
                        draw_id,
                    });
                    vertices.push(draw::LineVertex {
                        coords: bottom_right,
                        draw_id,
                    });
                    vertices.push(draw::LineVertex {
                        coords: bottom_left,
                        draw_id,
                    });
                    draw_params.push(draw::DrawParams {
                        draw_color: point.color,
                    });
                }
                if !draw_params.is_empty() {
                    pipeline.record(
                        &created.device,
                        &created.queue,
                        &mut frame.encoder,
                        draw::LineDrawCall {
                            frame_uniforms_buffer: buffer,
                            vertices: &vertices,
                            draw_params: &draw_params,
                            target: &frame.view,
                            load: wgpu::LoadOp::Load,
                        },
                    );
                }
            }
            BatchPayload::Triangles { cache_id, meshes } => {
                // Same real reachable-transient-condition reasoning
                // as the Tiles arm above.
                let Some(cached) = loop_state.texture_cache.get(*cache_id) else {
                    continue;
                };
                let (_, pipeline) = loop_state
                    .gouraud_pipeline
                    .as_ref()
                    .expect("gouraud_pipeline was just ensured above");
                let sampler = loop_state
                    .sampler
                    .as_ref()
                    .expect("sampler was just ensured above");
                let mut vertices: Vec<draw::GouraudVertex> = Vec::new();
                let mut draw_params: Vec<draw::GouraudDrawParams> = Vec::new();
                for mesh in meshes {
                    // Same real chunk-not-drop reasoning as the
                    // Lines/Tiles/Points arms above -- one real mesh
                    // is one draw_id/one draw_params entry here (like
                    // one tile is one draw_id), regardless of how
                    // many actual GPU vertices that one mesh itself
                    // contributes (variable, unlike Tile's fixed 6).
                    if draw_params.len() >= draw::MAX_DRAWS {
                        pipeline.record(
                            &created.device,
                            &created.queue,
                            &mut frame.encoder,
                            draw::GouraudDrawCall {
                                frame_uniforms_buffer: buffer,
                                vertices: &vertices,
                                draw_params: &draw_params,
                                texture_view: &cached.view,
                                sampler,
                                target: &frame.view,
                                load: wgpu::LoadOp::Load,
                            },
                        );
                        vertices.clear();
                        draw_params.clear();
                    }
                    let draw_id = draw_params.len() as u32;
                    vertices.extend(mesh.vertices.iter().map(|v| draw::GouraudVertex {
                        coords: v.coords,
                        draw_id,
                        tex_coords: v.tex_coords,
                        light_color: v.light_color,
                        fog_color: v.fog_color,
                    }));
                    draw_params.push(draw::GouraudDrawParams {
                        diffuse_info: mesh.diffuse_info,
                        draw_color: mesh.draw_color,
                        draw_flags: mesh.draw_flags,
                        _padding: [0; 3],
                    });
                }
                if !draw_params.is_empty() {
                    pipeline.record(
                        &created.device,
                        &created.queue,
                        &mut frame.encoder,
                        draw::GouraudDrawCall {
                            frame_uniforms_buffer: buffer,
                            vertices: &vertices,
                            draw_params: &draw_params,
                            texture_view: &cached.view,
                            sampler,
                            target: &frame.view,
                            load: wgpu::LoadOp::Load,
                        },
                    );
                }
            }
            BatchPayload::Complex {
                diffuse_cache_id,
                light_map_cache_id,
                fog_map_cache_id,
                surfaces,
            } => {
                // Same real reachable-transient-condition reasoning
                // as the Tiles/Triangles arms above -- the real
                // diffuse texture specifically, not degradable the
                // way light-map/fog-map are (queue_complex_surface's
                // own doc has the full real reasoning for both).
                let Some(diffuse_cached) = loop_state.texture_cache.get(*diffuse_cache_id) else {
                    continue;
                };
                let (_, pipeline) = loop_state
                    .complex_pipeline
                    .as_ref()
                    .expect("complex_pipeline was just ensured above");
                let sampler = loop_state
                    .sampler
                    .as_ref()
                    .expect("sampler was just ensured above");
                // A real light-map/fog-map cache_id here is always
                // already resident (queue_complex_surface only ever
                // records Some(cache_id) after a successful real
                // texture_cache.get) -- the dummy texture (RenderLoop::
                // dummy_texture's own doc) covers the real None case,
                // just ensured above whenever any batch needs it.
                let light_map_view = match light_map_cache_id {
                    Some(id) => {
                        &loop_state
                            .texture_cache
                            .get(*id)
                            .expect("queue_complex_surface only records an already-cached id")
                            .view
                    }
                    None => {
                        &loop_state
                            .dummy_texture
                            .as_ref()
                            .expect("dummy_texture was just ensured above")
                            .view
                    }
                };
                let fog_map_view = match fog_map_cache_id {
                    Some(id) => {
                        &loop_state
                            .texture_cache
                            .get(*id)
                            .expect("queue_complex_surface only records an already-cached id")
                            .view
                    }
                    None => {
                        &loop_state
                            .dummy_texture
                            .as_ref()
                            .expect("dummy_texture was just ensured above")
                            .view
                    }
                };
                let mut vertices: Vec<draw::ComplexVertex> = Vec::new();
                let mut draw_params: Vec<draw::ComplexDrawParams> = Vec::new();
                for surface in surfaces {
                    // Same real chunk-not-drop reasoning as the
                    // Tiles/Triangles arms above -- one real surface
                    // is one draw_id/one draw_params entry, regardless
                    // of its own variable vertex count.
                    if draw_params.len() >= draw::MAX_DRAWS {
                        pipeline.record(
                            &created.device,
                            &created.queue,
                            &mut frame.encoder,
                            draw::ComplexDrawCall {
                                frame_uniforms_buffer: buffer,
                                vertices: &vertices,
                                draw_params: &draw_params,
                                diffuse_texture_view: &diffuse_cached.view,
                                diffuse_sampler: sampler,
                                light_map_texture_view: light_map_view,
                                light_map_sampler: sampler,
                                fog_map_texture_view: fog_map_view,
                                fog_map_sampler: sampler,
                                target: &frame.view,
                                load: wgpu::LoadOp::Load,
                            },
                        );
                        vertices.clear();
                        draw_params.clear();
                    }
                    let draw_id = draw_params.len() as u32;
                    vertices.extend(surface.vertices.iter().map(|v| draw::ComplexVertex {
                        coords: v.coords,
                        draw_id,
                    }));
                    draw_params.push(draw::ComplexDrawParams {
                        diffuse_uv: surface.diffuse_uv,
                        light_map_uv: surface.light_map_uv,
                        fog_map_uv: surface.fog_map_uv,
                        diffuse_info: surface.diffuse_info,
                        x_axis: surface.x_axis,
                        y_axis: surface.y_axis,
                        draw_flags: surface.draw_flags,
                        _padding: [0; 3],
                    });
                }
                if !draw_params.is_empty() {
                    pipeline.record(
                        &created.device,
                        &created.queue,
                        &mut frame.encoder,
                        draw::ComplexDrawCall {
                            frame_uniforms_buffer: buffer,
                            vertices: &vertices,
                            draw_params: &draw_params,
                            diffuse_texture_view: &diffuse_cached.view,
                            diffuse_sampler: sampler,
                            light_map_texture_view: light_map_view,
                            light_map_sampler: sampler,
                            fog_map_texture_view: fog_map_view,
                            fog_map_sampler: sampler,
                            target: &frame.view,
                            load: wgpu::LoadOp::Load,
                        },
                    );
                }
            }
        }
    }
}

/// Replaces the per-frame-end half of `UWgpuRenderDevice::Unlock`.
/// `present`, matching the real `Unlock(UBOOL Blit)`'s own parameter
/// (`ThirdParty/XOpenGLDrv/Src/XOpenGL.cpp`'s own `Unlock` only blits
/// when `Blit` is set -- used for render passes that should not
/// actually reach the screen, e.g. hit-testing passes): `true` submits
/// and presents the frame `lock_frame` began; `false` submits (so the
/// GPU work still completes, matching XOpenGL's own real behavior of
/// still finishing the frame) but never calls `queue.present`, letting
/// `SurfaceTexture::drop` discard it instead (wgpu's own documented,
/// safe behavior for an un-presented texture). Never calls
/// `queue.present` at all for a `FramePresentable::Offscreen` frame
/// (`begin_offscreen_frame`'s own doc), regardless of `present` --
/// there is no real surface to present to.
///
/// Before submitting, flushes `loop_state.batches` (`RenderLoop::
/// queue_line`/`queue_tile`) into the frame's own `CommandEncoder`,
/// *after* the clear pass already recorded into it, in real push
/// order (`Batch`'s own doc has the full real citation for why real
/// call order, not a per-primitive-kind fixed order, is required):
/// one combined `Load`-mode `record()` call per batch (or per
/// `draw::MAX_DRAWS`-sized chunk of a batch, for any batch larger
/// than that -- matching the real GLSL batcher's own `Flush(true)`-
/// when-full behavior, `DrawSimple.cpp:68-69`/`DrawTile.cpp:188-197`,
/// no line or tile is ever lost), each bound to its own batch's real
/// segment (`RenderLoop::frame_uniforms_segments`'s own doc has the
/// full real bug this fixes) rather than one pass for the whole frame
/// sharing a single `FrameUniforms` buffer, and not one GPU operation
/// per engine draw call either (matching this crate's own real GLSL-
/// batching precedent: one real GPU draw call batches many logical
/// ones via `DrawID`). Each chunk's own real `DrawID`s are re-indexed
/// to start at `0`. A `Tiles` batch referencing a `cache_id` that was
/// never (or not yet) uploaded into `loop_state.texture_cache` is
/// skipped cleanly (not a panic) -- a real, reachable transient
/// condition, matching `queue_tile`'s own doc. Skipped entirely
/// (queue cleared either way) if nothing was queued.
///
/// `loop_state.counts.presented` is incremented here, not in
/// `lock_frame`, and only when `present` is `true` -- a successful
/// `lock_frame` only acquired a frame; `unlock_frame(present: false)`
/// never actually presents it, and counting it as `presented` would
/// have over-counted relative to what callers like
/// `Build/run_wgpu_smoke.py`'s `presented > 0` check mean by the name
/// (caught on review, not self-caught). A `Suboptimal` frame
/// (`lock_frame`'s own `InProgressFrame::suboptimal`) clears
/// `loop_state.configured_size` regardless of `present` -- it is a
/// property of the surface itself (`Surface::get_current_texture`'s
/// own doc: "usable now, but reconfigure soon"), not of what this one
/// call chose to do with the frame it got, so the next `lock_frame`
/// reconfigures either way.
///
/// Returns `false` if `loop_state.in_progress` is `None` -- `unlock_
/// frame` called without a matching successful `lock_frame` first is a
/// caller bug, not a real runtime condition to report via
/// `FrameOutcome`.
pub fn unlock_frame(created: &CreatedDevice, loop_state: &mut RenderLoop, present: bool) -> bool {
    let Some(mut frame) = loop_state.in_progress.take() else {
        return false;
    };
    // Owned locally (not borrowed from loop_state) specifically so
    // the loop below can freely take immutable borrows of loop_
    // state's other fields (frame_uniforms_segments, texture_cache,
    // line_pipeline, tile_pipeline, sampler) without fighting the
    // borrow checker over two different fields of the same struct --
    // already empty in loop_state.batches after this, equivalent to
    // the explicit clear() a lone-queue design would still need.
    let batches = std::mem::take(&mut loop_state.batches);
    dispatch_batches(created, loop_state, &mut frame, batches);
    created
        .queue
        .submit(std::iter::once(frame.encoder.finish()));
    match frame.presentable {
        FramePresentable::Surface(texture) if present => {
            // XOpenGL.cpp:2829-2831: capture runs after the frame is fully
            // rendered and immediately before the swap -- here, after the
            // submit above and before present.
            capture_presented_frame(created, loop_state, &texture.texture);
            created.queue.present(texture);
            loop_state.counts.presented += 1;
        }
        // Same Blit-only capture point, minus the present (`CaptureOnly`'s
        // own doc); not counted as presented.
        FramePresentable::CaptureOnly(texture) if present => {
            capture_presented_frame(created, loop_state, &texture);
        }
        _ => {}
    }
    if frame.suboptimal {
        loop_state.configured_size = None;
    }
    true
}

/// Real `HPCapturePresentedFrame` (XOpenGL.cpp:2707-2767) for a wgpu
/// surface texture: no-op unless `HP2_CAPTURE_FRAMES` is on. Reads the
/// finished frame back (`capture::read_texture_rgba8`, top row first --
/// wgpu needs no bottom-up flip, unlike `glReadPixels`) and appends it via
/// `capture::FrameCapture::write_frame`, recording the real adapter name
/// as `renderer` (XOpenGL's `GL_RENDERER` equivalent). A swapchain that
/// could not get `COPY_SRC` (`lock_frame`'s own capability check), or an
/// unsupported surface format, disables capture with one warning instead
/// of failing every frame -- same as XOpenGL's own write-failure path.
fn capture_presented_frame(
    created: &CreatedDevice,
    loop_state: &mut RenderLoop,
    texture: &wgpu::Texture,
) {
    let Some(frame_capture) = loop_state.capture.as_mut() else {
        return;
    };
    if !frame_capture.is_enabled() {
        return;
    }
    if !texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
        log::warn!("WgpuDrv: surface does not support COPY_SRC; disabling frame capture");
        loop_state.capture = None;
        return;
    }
    let Some(rgba) = capture::read_texture_rgba8(created, texture) else {
        log::warn!(
            "WgpuDrv: surface format {:?} unsupported for frame capture; disabling it",
            texture.format()
        );
        loop_state.capture = None;
        return;
    };
    if let Err(error) = frame_capture.write_frame(
        &rgba,
        texture.width(),
        texture.height(),
        loop_state.backing_scale,
        &created.adapter_name,
    ) {
        log::warn!("WgpuDrv: frame capture write failed ({error}); disabling capture");
    }
}

/// `src/shaders/common.wgsl`'s source, concatenated in front of every
/// named shader's own source. WGSL has no `#include`/import mechanism
/// (unlike the GLSL source this crate ports from, which assembles its
/// own shared prelude plus each shader's body into one compilation
/// unit via plain C++ string concatenation) -- this is the direct
/// WGSL-side equivalent of that, one real mechanism used by this
/// crate's own tests (`every_wgsl_shader_parses_and_validates`'s naga
/// validation and the real-GPU pipeline tests) rather than each
/// inventing its own ad hoc concatenation. `common.wgsl` itself has no
/// `@vertex`/`@fragment` entry points and is never passed to this
/// function as `name` -- it is always the prefix, never a standalone
/// shader.
///
/// Test/dev-only, deliberately `#[cfg(test)]`, not a real production
/// API despite reading real shader sources -- this reads from disk via
/// `CARGO_MANIFEST_DIR` at runtime (`include_str!`'s path argument
/// must be a string literal, not a runtime-formatted one, so it can't
/// be used with a parameterized `name` the way this function takes
/// one), which only resolves on a dev machine with the source tree
/// present, not in a shipped game binary. Real production pipeline
/// creation -- not written yet -- needs shader source compiled into
/// the binary (e.g. `concat!(include_str!("shaders/common.wgsl"),
/// include_str!("shaders/simple.wgsl"))` per shader, or a `build.rs`
/// step), a separate mechanism from this one, not yet designed.
#[cfg(test)]
fn shader_source(name: &str) -> String {
    let shader_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders");
    let common = std::fs::read_to_string(format!("{shader_dir}/common.wgsl"))
        .unwrap_or_else(|e| panic!("failed to read {shader_dir}/common.wgsl: {e}"));
    let shader = std::fs::read_to_string(format!("{shader_dir}/{name}.wgsl"))
        .unwrap_or_else(|e| panic!("failed to read {shader_dir}/{name}.wgsl: {e}"));
    format!("{common}\n{shader}")
}

/// Window-surface creation from a C++-owned `SDL_Window*`'s raw view
/// pointer. Only macOS today -- the plan's "Approved deviations" SDL2
/// entry's reasoning applies here too: no `sdl2` crate, so no
/// `raw-window-handle` feature on a crate dependency either; C++ gets
/// the raw `NSView*` itself via `SDL_Metal_CreateView`/
/// `SDL_Metal_GetLayer` (SDL functions, not an `sdl2` crate call) and
/// passes the pointer across FFI, same pattern as every other
/// SDL-touching slice this session.
#[cfg(target_os = "macos")]
pub mod macos {
    use std::ffi::c_void;
    use std::ptr::NonNull;

    /// Replaces the window-surface half of `WgpuDrv`'s future `Init`
    /// (plan's "Window surface" bullet). `ns_view` is the pointer
    /// `SDL_Metal_CreateView` returns (itself "can be cast directly to
    /// an NSView", per `SDL_metal.h`'s own doc comment) -- C++ calls
    /// `SDL_Metal_CreateView`, not this function, since it owns the
    /// `SDL_Window*` the view is attached to; this function only turns
    /// the resulting raw pointer into a `wgpu::Surface`.
    ///
    /// # Safety
    /// `ns_view` must be a valid, non-null pointer to an `NSView`
    /// (typically a `CAMetalLayer`-backed one from
    /// `SDL_Metal_CreateView`) that outlives the returned `Surface`.
    /// Must be called from the application's main thread: AppKit views
    /// are only safely accessed from there (see
    /// `raw_window_handle::AppKitWindowHandle`'s own doc comment).
    pub unsafe fn create_surface_from_ns_view(
        instance: &wgpu::Instance,
        ns_view: *mut c_void,
    ) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError> {
        let ns_view = NonNull::new(ns_view).expect("ns_view must not be null");
        let raw_window_handle = raw_window_handle::RawWindowHandle::AppKit(
            raw_window_handle::AppKitWindowHandle::new(ns_view),
        );
        let raw_display_handle = raw_window_handle::RawDisplayHandle::AppKit(
            raw_window_handle::AppKitDisplayHandle::new(),
        );
        unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle),
                raw_window_handle,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real reference scene, loosely matching a typical in-game FSceneNode:
    // identity Coords/Uncoords, a square viewport, a 90-degree FOV --
    // chosen for round, easy-to-verify numbers, not copied from any
    // particular real capture.
    fn reference_perspective_scene() -> SceneFrame {
        SceneFrame {
            coords_origin: [0.0, 0.0, 0.0],
            coords_x_axis: [1.0, 0.0, 0.0],
            coords_y_axis: [0.0, 1.0, 0.0],
            coords_z_axis: [0.0, 0.0, 1.0],
            uncoords_origin: [0.0, 0.0, 0.0],
            uncoords_x_axis: [1.0, 0.0, 0.0],
            uncoords_y_axis: [0.0, 1.0, 0.0],
            uncoords_z_axis: [0.0, 0.0, 1.0],
            fx: 100.0,
            fy: 100.0,
            effective_fov_angle: 90.0,
            viewport_fov_angle: 90.0,
            is_ortho: false,
            is_wireframe_zfar: false,
            gamma: 1.0,
            light_color_intensity: 1.5,
            light_map_intensity: 4.0,
        }
    }

    #[test]
    fn draw_2d_line_screen_to_coords_matches_the_real_source_formula() {
        // reference_perspective_scene(): fx=fy=100 -> aspect=1.0,
        // effective_fov_angle=90 -> r_proj_z=tan(45 deg)=1.0 ->
        // rfx2=rfy2=2.0*1.0/100.0=0.02, fx2=fy2=50.0. P=(0, 50, 1):
        // x = 0.02*1*(0-50) = -1.0, y = 0.02*1*(50-50) = 0.0, z = 1.0
        // -- DrawSimple.cpp:80's own real formula, hand-computed
        // independently of draw_2d_line_screen_to_coords's own
        // implementation.
        let out = draw_2d_line_screen_to_coords(
            &reference_perspective_scene(),
            50.0,
            50.0,
            [0.0, 50.0, 1.0],
        );
        assert!(
            (out[0] - (-1.0)).abs() < 1e-5,
            "expected x ~= -1.0, got {}",
            out[0]
        );
        assert!(
            (out[1] - 0.0).abs() < 1e-5,
            "expected y ~= 0.0, got {}",
            out[1]
        );
        assert_eq!(out[2], 1.0, "expected z to pass through P.Z unchanged");
    }

    #[test]
    fn draw_2d_line_screen_to_coords_uses_viewport_fov_angle_only_on_the_ortho_path() {
        // SetOrthoProjection (XOpenGL.cpp:1821) reads Viewport->Actor
        // ->FovAngle (viewport_fov_angle); SetProjection (1868) reads
        // Frame->EffectiveFovAngle (effective_fov_angle) -- a real,
        // deliberate difference between the two paths (SceneFrame's
        // own doc already notes this), not interchangeable. Exact
        // expected values, not just `assert_ne!` (an earlier version
        // of this test used `assert_ne!`, which would still pass if
        // the two branches were swapped -- caught on review): with
        // fx=fy=100, viewport_fov_angle=60, effective_fov_angle=90,
        // P=(0,50,1), fx2=fy2=50 -- ortho: r_proj_z=tan(30deg),
        // rfx2=2*tan(30deg)/100, x=rfx2*(0-50)=-tan(30deg)=-0.57735;
        // perspective: r_proj_z=tan(45deg)=1.0, rfx2=0.02,
        // x=0.02*(-50)=-1.0. Hand-verified independently before
        // relying on it, not just read off the implementation.
        let mut scene = reference_perspective_scene();
        scene.viewport_fov_angle = 60.0;
        scene.effective_fov_angle = 90.0;
        scene.is_ortho = true;
        let ortho_out = draw_2d_line_screen_to_coords(&scene, 50.0, 50.0, [0.0, 50.0, 1.0]);
        scene.is_ortho = false;
        let perspective_out = draw_2d_line_screen_to_coords(&scene, 50.0, 50.0, [0.0, 50.0, 1.0]);
        assert!(
            (ortho_out[0] - (-0.57735)).abs() < 1e-4,
            "expected ortho x ~= -tan(30deg) ~= -0.57735, got {}",
            ortho_out[0]
        );
        assert!(
            (perspective_out[0] - (-1.0)).abs() < 1e-5,
            "expected perspective x ~= -1.0, got {}",
            perspective_out[0]
        );
    }

    #[test]
    fn draw_2d_line_screen_to_coords_uses_the_passed_fx2_fy2_not_scene_fx_fy_times_half() {
        // Every other test in this file passes fx2/fy2 ==
        // scene.fx*0.5/scene.fy*0.5 (the value they'd coincidentally
        // equal if the real Draw2DLine call's own Frame happens to be
        // the same one the last SetSceneNode call cached) -- none of
        // them would catch a regression back to the earlier, wrong
        // version of this function that derived fx2/fy2 internally
        // from scene.fx/fy instead of using its own parameters
        // (6de550c's real bug). This test deliberately picks fx2/fy2
        // that differ from scene.fx*0.5/scene.fy*0.5 so the two
        // interpretations produce different, distinguishable outputs.
        //
        // reference_perspective_scene(): fx=fy=100 (so scene.fx*0.5 =
        // scene.fy*0.5 = 50.0, deliberately NOT passed here),
        // effective_fov_angle=90 -> r_proj_z=tan(45deg)=1.0 ->
        // rfx2=rfy2=2.0*1.0/100.0=0.02. P=(0, 50, 1), fx2=30.0,
        // fy2=70.0 (both != 50.0): x = 0.02*1*(0-30) = -0.6,
        // y = 0.02*1*(50-70) = -0.4 -- hand-computed independently,
        // not read off the implementation. The old, wrong, scene-
        // derived behavior would instead ignore these and compute
        // x = -1.0, y = 0.0 (the same result the first test above
        // gets with fx2=fy2=50.0), a clearly different value.
        let out = draw_2d_line_screen_to_coords(
            &reference_perspective_scene(),
            30.0,
            70.0,
            [0.0, 50.0, 1.0],
        );
        assert!(
            (out[0] - (-0.6)).abs() < 1e-5,
            "expected x ~= -0.6 (from the passed fx2=30.0, not scene.fx*0.5=50.0), got {}",
            out[0]
        );
        assert!(
            (out[1] - (-0.4)).abs() < 1e-5,
            "expected y ~= -0.4 (from the passed fy2=70.0, not scene.fy*0.5=50.0), got {}",
            out[1]
        );
    }

    #[test]
    fn resolve_point_z_matches_the_real_draw_2d_point_z_adjustment() {
        // Real DrawSimple.cpp:170-173: ortho always forces Z=1.0
        // regardless of the input Z's own sign or magnitude; non-
        // ortho negates a negative Z, leaves a non-negative Z alone.
        assert_eq!(
            resolve_point_z(true, -7.0),
            1.0,
            "ortho must force Z=1.0 even when the input Z is negative"
        );
        assert_eq!(
            resolve_point_z(true, 7.0),
            1.0,
            "ortho must force Z=1.0 even when the input Z is already positive"
        );
        assert_eq!(
            resolve_point_z(false, -7.0),
            7.0,
            "non-ortho must negate a negative Z"
        );
        assert_eq!(
            resolve_point_z(false, 7.0),
            7.0,
            "non-ortho must leave a non-negative Z unchanged"
        );
    }

    #[test]
    fn frame_uniforms_view_and_model_mat_match_the_real_fixed_source_values() {
        let out = compute_frame_uniforms(&reference_perspective_scene());
        // XOpenGL.cpp's own Init: glm::scale(mat4(1), vec3(1,-1,-1)),
        // set once and never recomputed per frame -- not derived from
        // any SceneFrame field, so this must hold regardless of input.
        assert_eq!(
            out.view_mat,
            glam::Mat4::from_scale(glam::Vec3::new(1.0, -1.0, -1.0))
        );
        assert_eq!(out.model_mat, glam::Mat4::IDENTITY);
        // modelview_mat = view_mat * model_mat, modelviewproj_mat =
        // proj_mat * modelview_mat -- SetOrthoProjection/SetProjection's
        // own real composition order ("yes, this is right" in the
        // source's own comment), checked here by reconstructing it
        // independently from the three returned factors rather than
        // trusting the function got its own multiplication right.
        assert_eq!(out.modelview_mat, out.view_mat * out.model_mat);
        assert_eq!(out.modelviewproj_mat, out.proj_mat * out.modelview_mat);
    }

    #[test]
    fn frame_uniforms_frame_coords_use_the_real_column_order() {
        let mut scene = reference_perspective_scene();
        scene.coords_origin = [1.0, 2.0, 3.0];
        scene.coords_x_axis = [4.0, 5.0, 6.0];
        scene.coords_y_axis = [7.0, 8.0, 9.0];
        scene.coords_z_axis = [10.0, 11.0, 12.0];
        let out = compute_frame_uniforms(&scene);
        // UpdateCoords: FrameCoords[0..3] = Origin, XAxis, YAxis, ZAxis,
        // each a glm column (glam::Mat4::from_cols matches glm's own
        // column-major convention) with w=0, not w=1 -- these are
        // direction-like basis vectors, not a translated point, in the
        // real source.
        assert_eq!(out.frame_coords.col(0), glam::Vec4::new(1.0, 2.0, 3.0, 0.0));
        assert_eq!(out.frame_coords.col(1), glam::Vec4::new(4.0, 5.0, 6.0, 0.0));
        assert_eq!(out.frame_coords.col(2), glam::Vec4::new(7.0, 8.0, 9.0, 0.0));
        assert_eq!(
            out.frame_coords.col(3),
            glam::Vec4::new(10.0, 11.0, 12.0, 0.0)
        );
    }

    #[test]
    fn frame_uniforms_perspective_z_range_is_zero_to_one_not_negative_one_to_one() {
        let out = compute_frame_uniforms(&reference_perspective_scene());
        // A point straight down the view axis at the real near/far
        // planes (PERSPECTIVE_Z_NEAR=1.0, Z_FAR_NORMAL=65336.0) must
        // land at clip-space z/w == 0.0 / 1.0 respectively under
        // WebGPU's convention -- -1.0/1.0 under GL's, the exact bug
        // this function's whole job is to avoid (simple.wgsl's own
        // depth-range-contract note). glam's RH view-space convention
        // (`camera::rh`'s own doc: "the negative Z axis pointing
        // forward") puts visible points at *negative* view-space Z;
        // view_mat flips Z (`vec3(1,-1,-1)`), so a world point needs
        // *positive* Z to land in front of the camera after that flip
        // (verified by hand before relying on it, not assumed from
        // the sign alone -- an earlier version of this test got this
        // backwards, caught before committing by deriving it
        // numerically rather than trusting intuition).
        let near_world = glam::Vec4::new(0.0, 0.0, 1.0, 1.0);
        let far_world = glam::Vec4::new(0.0, 0.0, 65336.0, 1.0);
        let near_clip = out.modelviewproj_mat * near_world;
        let far_clip = out.modelviewproj_mat * far_world;
        assert!(
            (near_clip.z / near_clip.w).abs() < 1e-4,
            "expected near-plane NDC z ~= 0.0 (WebGPU convention), got {}",
            near_clip.z / near_clip.w
        );
        assert!(
            ((far_clip.z / far_clip.w) - 1.0).abs() < 1e-4,
            "expected far-plane NDC z ~= 1.0 (WebGPU convention), got {}",
            far_clip.z / far_clip.w
        );
    }

    #[test]
    fn frame_uniforms_perspective_fov_matches_the_real_frustum_edges() {
        // 90-degree FOV, square viewport (fx==fy, aspect==1): RProjZ =
        // tan(45 deg) == 1.0, so the horizontal AND vertical frustum
        // half-angles at any given depth both equal that depth --
        // view-space (depth, depth, -depth) should land exactly on
        // the clip-space x==w / y==w edge (NDC x==1.0, y==1.0).
        // view_mat (`vec3(1,-1,-1)`) leaves X unchanged and flips
        // Y/Z, so the matching world-space point is (depth, -depth,
        // +depth) -- positive world Z, same forward-direction fix as
        // the z-range test above, verified numerically, not assumed.
        let out = compute_frame_uniforms(&reference_perspective_scene());
        let depth = 10.0;
        let edge_world = glam::Vec4::new(depth, -depth, depth, 1.0);
        let edge_clip = out.modelviewproj_mat * edge_world;
        assert!(
            (edge_clip.x / edge_clip.w - 1.0).abs() < 1e-4,
            "expected the 90-degree-FOV horizontal edge at NDC x ~= 1.0, got {}",
            edge_clip.x / edge_clip.w
        );
        assert!(
            (edge_clip.y / edge_clip.w - 1.0).abs() < 1e-4,
            "expected the 90-degree-FOV vertical edge at NDC y ~= 1.0, got {}",
            edge_clip.y / edge_clip.w
        );
    }

    #[test]
    fn frame_uniforms_ortho_path_is_selected_and_z_range_is_zero_to_one() {
        let mut scene = reference_perspective_scene();
        scene.is_ortho = true;
        let out = compute_frame_uniforms(&scene);
        // Same WebGPU-convention check as the perspective test, but
        // for SetOrthoProjection's own real glm::ortho(-RProjZ,
        // +RProjZ, ..., -zFar, +zFar) call -- note the real source
        // passes a *negative* near (-zFar), confirmed to not panic
        // glam's own `directx::orthographic` (unlike `frustum`, its
        // documented panic conditions never require a positive near).
        //
        // Asserts a specific point-to-extreme pairing, not just that
        // the two extremes land somewhere in {0.0, 1.0} in either
        // order (caught on review: an earlier version of this test
        // only checked the sorted range, which is genuinely order-
        // independent but for the wrong reason -- it would still pass
        // if ortho depth came out reversed, mapping the forward
        // direction toward 0 instead of 1; that is not a cosmetic
        // difference, it would make the editor's orthographic views
        // depth-test backwards, back geometry winning under a `Less`
        // compare). `forward_world` (world Z = +65336) is forward
        // under `view_mat`'s real `vec3(1,-1,-1)` Z-flip -- matching
        // the perspective test's own same derivation above -- and
        // must land at NDC z ~= 1.0 (far), not 0.0; `backward_world`
        // must land at ~= 0.0 (near). This project's own real run
        // already produces exactly that pairing (verified empirically
        // before writing this assertion, not assumed from the
        // formula) -- an earlier version of this same test got this
        // backwards once already (caught on a real run, see this
        // test's own prior history), so the fix here is a real,
        // checked assertion, not a guess repeated a second time.
        let forward_world = glam::Vec4::new(0.0, 0.0, 65336.0, 1.0);
        let backward_world = glam::Vec4::new(0.0, 0.0, -65336.0, 1.0);
        let forward_ndc_z = (out.modelviewproj_mat * forward_world).z;
        let backward_ndc_z = (out.modelviewproj_mat * backward_world).z;
        assert!(
            (forward_ndc_z - 1.0).abs() < 1e-3,
            "expected the forward ortho direction (UE Z=+65336) to land at NDC z ~= 1.0 (far), \
             got {forward_ndc_z}"
        );
        assert!(
            backward_ndc_z.abs() < 1e-3,
            "expected the backward ortho direction (UE Z=-65336) to land at NDC z ~= 0.0 \
             (near), got {backward_ndc_z}"
        );
    }

    #[test]
    fn frame_uniforms_wireframe_zfar_changes_the_far_plane() {
        // Real bug, caught on review: an earlier version of this test
        // checked a world point at the real wireframe zFar (131072)
        // against a 1e-3 NDC-z tolerance -- that passes even when
        // is_wireframe_zfar is silently ignored and the matrix always
        // uses 65336, because perspective depth is hyperbolic: with
        // near=1, far=65336, a point at z=131072 already projects to
        // NDC z ~= 1.0000077 (empirically computed before relying on
        // it, not assumed), well inside that tolerance regardless of
        // which real zFar the matrix was built with. Comparing
        // proj_mat directly between the two flag settings sidesteps
        // the whole precision trap: the matrices must differ at all
        // if is_wireframe_zfar has any effect, with no NDC-precision
        // question involved.
        let normal = compute_frame_uniforms(&reference_perspective_scene());
        let mut wireframe_scene = reference_perspective_scene();
        wireframe_scene.is_wireframe_zfar = true;
        let wireframe = compute_frame_uniforms(&wireframe_scene);
        assert_ne!(
            normal.proj_mat, wireframe.proj_mat,
            "expected is_wireframe_zfar to produce a genuinely different projection matrix \
             (different zFar), not an identical one"
        );
    }

    #[test]
    fn frame_uniforms_passes_through_scalar_fields_unmodified() {
        let mut scene = reference_perspective_scene();
        scene.gamma = 2.2;
        scene.light_color_intensity = 1.5;
        scene.light_map_intensity = 4.0;
        let out = compute_frame_uniforms(&scene);
        assert_eq!(out.gamma, 2.2);
        assert_eq!(out.light_color_intensity, 1.5);
        assert_eq!(out.light_map_intensity, 4.0);
        // Hardcoded 1.0 unconditionally -- the real source's DXGI
        // interop branch is Windows-GL-only, unreachable here.
        assert_eq!(out.y_scale, 1.0);
        // Not yet populated; see this function's own doc.
        assert_eq!(out.light_space_mat, glam::Mat4::IDENTITY);
    }

    #[test]
    fn frame_uniforms_byte_size_matches_common_wgsl_and_the_buffer_size_comment() {
        // 8 mat4x4<f32> (64 bytes each) + 4 f32 (4 bytes each) = 528,
        // matching common.wgsl's FrameUniforms exactly -- checked here
        // so RenderLoop::set_scene_node's buffer-size doc comment
        // cannot silently drift from the real struct size if a field
        // is ever added or removed.
        assert_eq!(std::mem::size_of::<FrameUniforms>(), 8 * 64 + 4 * 4);
        assert_eq!(std::mem::size_of::<FrameUniforms>(), 528);
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn set_scene_node_creates_and_writes_a_real_uniform_buffer() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device on this machine");
        let mut loop_state = RenderLoop::new();
        assert_eq!(
            loop_state.frame_uniforms_segment_count(),
            0,
            "no segment should exist before the first set_scene_node call"
        );
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        assert_eq!(loop_state.frame_uniforms_segment_count(), 1);
        let buffer_size = loop_state
            .frame_uniforms_segment(0)
            .expect("expected segment 0 to exist after one set_scene_node call")
            .size();
        assert_eq!(buffer_size, std::mem::size_of::<FrameUniforms>() as u64);
        // Real proof the write actually reached the GPU with the
        // right bytes, not just that create_buffer/write_buffer/
        // submit didn't error (caught on review: an earlier version
        // of this test only submitted an empty encoder and polled,
        // which only proves the device didn't error -- it says
        // nothing about whether the uploaded bytes are what
        // compute_frame_uniforms actually produced, and would not
        // have caught a wrong offset/size in queue.write_buffer's own
        // call, or a bug in the unsafe FrameUniforms-to-bytes
        // reinterpretation that call depends on). Copies the real
        // buffer into a MAP_READ staging buffer and compares the
        // mapped bytes against compute_frame_uniforms's own output
        // computed independently here, byte for byte.
        let expected = compute_frame_uniforms(&reference_perspective_scene());
        let expected_bytes = bytemuck::bytes_of(&expected);
        let readback = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = created
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            0,
            &readback,
            0,
            buffer_size,
        );
        created.queue.submit(std::iter::once(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed while mapping the readback buffer");
        rx.recv()
            .expect("map_async callback should have fired")
            .expect("mapping the readback buffer should succeed");
        let mapped_bytes: Vec<u8> = readback
            .slice(..)
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async")
            .to_vec();
        assert_eq!(
            mapped_bytes, expected_bytes,
            "expected the bytes actually uploaded to the GPU buffer to match \
             compute_frame_uniforms's own real output exactly"
        );
        readback.unmap();
        // A second call must append a *new* segment, not reuse the
        // first one -- the real fix this per-segment redesign exists
        // for (RenderLoop::frame_uniforms_segments's own doc has the
        // full real bug reusing a single shared buffer caused once
        // real draw calls existed). `Buffer: Clone + PartialEq`
        // (confirmed against wgpu's own source,
        // `impl_eq_ord_hash_proxy!(Buffer => .inner)`, before relying
        // on it -- an earlier version of this test tried a
        // `global_id()` method that does not exist on wgpu 30.0.1's
        // real `Buffer` at all, caught by actually building it, not
        // assumed from memory of a different wgpu version's API).
        let first_buffer = loop_state.frame_uniforms_segment(0).unwrap().clone();
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        assert_eq!(loop_state.frame_uniforms_segment_count(), 2);
        assert_eq!(
            loop_state.frame_uniforms_segment(0).unwrap(),
            &first_buffer,
            "expected the first segment to still exist, unchanged, after a second \
             set_scene_node call"
        );
        assert_ne!(
            loop_state.frame_uniforms_segment(1).unwrap(),
            &first_buffer,
            "expected the second set_scene_node call to create a genuinely new segment buffer, \
             not reuse the first one -- the real bug this redesign fixes"
        );
    }

    // The real test the review that caught the clear-vs-draw-ordering
    // bug asked for (Finding B): draw.rs's own
    // record_composites_correctly_into_a_shared_unsubmitted_encoder
    // only manually *replicates* lock_frame/unlock_frame's own
    // reasoning -- it proves SimpleLinePipeline::record composites
    // correctly into a hand-built encoder, not that RenderLoop's own
    // real lock/unlock/queue code actually does the right thing. This
    // test instead drives the real production functions end to end:
    // begin_offscreen_frame (the real lock_frame logic, against a
    // caller-owned offscreen texture instead of a real OS-backed
    // surface -- no literal "offscreen surface" exists in wgpu, and a
    // real Surface-backed lock_frame needs a real window, unavailable
    // to a Rust #[test] on macOS), RenderLoop::queue_line (the real
    // Draw2DLine accumulation path), and unlock_frame (the real flush
    // + single combined submit). Only the SceneFrame -> identity
    // matrix substitution below is not the literal real path end to
    // end -- matching the same known-good technique
    // record_composites_correctly_into_a_shared_unsubmitted_encoder's
    // own manual replica already uses, for the same reason: a real
    // perspective SceneFrame's own modelviewproj_mat is not a simple
    // number to predict screen position from by hand, and that is not
    // what this test exists to check (compute_frame_uniforms's own
    // tests already cover that math).
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_flushes_queued_line_draws_through_the_real_lock_unlock_path() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_loop offscreen test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Real lock_frame logic (clear recorded, not submitted).
        // Must come *before* set_scene_node: begin_offscreen_frame
        // clears frame_uniforms_segments at its own start, matching
        // the real engine's own real call order (SetSceneNode always
        // runs between Lock and Unlock, never before Lock) -- an
        // earlier version of this test called set_scene_node first,
        // which begin_offscreen_frame's own clear then silently wiped
        // out, making queue_line drop the line entirely (no segment
        // to tag it with) and the final assertion below fail with the
        // real clear color instead of the real drawn line -- a real
        // bug in this test's own ordering, not in the production
        // code, caught by this test's own failure.
        begin_offscreen_frame(&created, &mut loop_state, &render_target, wgpu::Color::BLUE);
        // Real production call: creates frame_uniforms_segments[0]
        // via the same real path a real SetSceneNode call uses.
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        // Overwrite modelviewproj_mat (bytes [256..320), the last of
        // FrameUniforms' five leading Mat4 fields -- proj_mat/
        // view_mat/model_mat/modelview_mat/modelviewproj_mat, each 64
        // bytes) with an identity matrix: diagonal floats at relative
        // offsets 0, 20, 40, 60 (column-major Mat4: col0 row0, col1
        // row1, col2 row2, col3 row3), matching the exact technique
        // draw.rs's own manual replica test already uses, for the
        // same reason -- predictable screen-space geometry instead of
        // a real lens projection's own math.
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );

        // Real Draw2DLine accumulation path, raw screen-space points
        // (not pre-transformed -- queue_line's own doc has the real
        // transform). With reference_perspective_scene()'s fx=fy=100,
        // effective_fov_angle=90 (so aspect=1.0, r_proj_z=tan(45)=1.0,
        // rfx2=rfy2=0.02, fx2=fy2=50.0): P1=(0, 50, 1) transforms to
        // (0.02*1*(0-50), 0.02*1*(50-50), 1) = (-1.0, 0.0, 1.0); P2=
        // (50, 50, 1) transforms to (0.0, 0.0, 1.0) -- a green line
        // spanning NDC x in [-1.0, 0.0] once the identity-matrix-
        // substituted modelviewproj_mat above passes those coords
        // straight through to clip space, the left half of the
        // 64px-wide target (Z is irrelevant: the pipeline has no
        // depth attachment).
        loop_state.queue_line(
            [0.0, 50.0, 1.0],
            [50.0, 50.0, 1.0],
            50.0,
            50.0,
            [0.0, 1.0, 0.0, 1.0],
        );
        // Real unlock_frame: flushes the queued line into the frame's
        // own still-unsubmitted encoder (Load mode, after the clear),
        // then submits exactly once. present=false: no real surface
        // to present to anyway (FramePresentable::Offscreen), and
        // this matches a real hit-testing-style Unlock(Blit=0) call.
        let did_unlock = unlock_frame(&created, &mut loop_state, false);
        assert!(
            did_unlock,
            "expected unlock_frame to find the frame begin_offscreen_frame began"
        );
        assert_eq!(
            loop_state.counts().presented,
            0,
            "present=false must not count as presented, and Offscreen frames never call \
             queue.present regardless"
        );
        // A second unlock_frame call with no matching lock must report
        // no frame in progress -- proves the first call actually took
        // loop_state.in_progress, not left it behind.
        assert!(!unlock_frame(&created, &mut loop_state, false));

        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_loop offscreen test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_loop offscreen test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        // Left half (inside the queued line's span): must be green --
        // proves RenderLoop::queue_line's real accumulation plus
        // unlock_frame's real flush-then-submit actually reached the
        // GPU, through the real production RenderLoop API, not a
        // manual replica of it.
        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[0u8, 255, 0, 255],
            "expected the line queued via RenderLoop::queue_line and flushed by the real \
             unlock_frame to actually render, got {left_pixel:?}"
        );
        // Right half: must still be the real begin_offscreen_frame
        // clear color -- proves the clear (recorded first, same as a
        // real lock_frame) survived the combined submit alongside the
        // later-recorded draw, through the real production code path.
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected begin_offscreen_frame's own clear pass to survive the real unlock_frame's \
             combined submit outside the drawn line's span, got {right_pixel:?}"
        );
        println!(
            "RenderLoop's real begin_offscreen_frame/queue_line/unlock_frame path confirmed \
             empirically to composite a queued line draw correctly on top of its own clear, in \
             one combined submit, on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_queue_tile_renders_a_real_tile_through_the_real_lock_unlock_path() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_loop tile offscreen test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Same real call order and identity-modelviewproj substitution
        // as render_loop_flushes_queued_line_draws_through_the_real_
        // lock_unlock_path above -- that test's own doc has the full
        // reasoning for both.
        begin_offscreen_frame(&created, &mut loop_state, &render_target, wgpu::Color::BLUE);
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );

        // A real 1x1 opaque white texture: with draw_flags=0 (no
        // PF_Masked/PF_Highlighted), tile.wgsl's own fs_main computes
        // apply_poly_flags(texel, 0) * draw_color == texel * draw_color
        // unchanged (common.wgsl's own apply_poly_flags is a no-op
        // passthrough when draw_flags has neither bit set, confirmed
        // by reading it directly) -- a white texel makes the final
        // pixel exactly draw_color, isolating this test from needing
        // to also reason about texture-color multiplication.
        loop_state.upload_texture(&created, 99, 1, 1, &[255, 255, 255, 255], false);

        // Same real rfx2=rfy2=0.02/fx2=fy2=50.0 derivation as the
        // line test above: X=0,XL=50 -> NDC x in [-1.0, 0.0] (left
        // half of the 64px target), matching that test's own P1/P2
        // exactly so both tests share one hand-verified geometry
        // derivation. Y=49.5,YL=1.0 -> NDC y in [-0.01, 0.01],
        // straddling the single render row's own center symmetrically
        // regardless of wgpu's NDC-to-viewport Y-flip convention
        // (unlike the line test, a *filled* quad needs its own Y-span
        // to actually cover that row's center, not just a degenerate
        // zero-height line at it).
        loop_state.queue_tile(TileDrawRequest {
            cache_id: 99,
            x: 0.0,
            y: 49.5,
            xl: 50.0,
            yl: 1.0,
            u: 0.0,
            v: 0.0,
            ul: 1.0,
            vl: 1.0,
            z: 1.0,
            color: [0.0, 1.0, 0.0, 1.0],
            poly_flags: 0,
            u_scale: 1.0,
            v_scale: 1.0,
            texture_alpha: None,
            fx2: 50.0,
            fy2: 50.0,
        });
        let did_unlock = unlock_frame(&created, &mut loop_state, false);
        assert!(
            did_unlock,
            "expected unlock_frame to find the frame begin_offscreen_frame began"
        );
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_loop tile offscreen test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_loop tile offscreen test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[0u8, 255, 0, 255],
            "expected the tile queued via RenderLoop::queue_tile and flushed by the real \
             unlock_frame to actually render, got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected begin_offscreen_frame's own clear pass to survive outside the drawn \
             tile's span, got {right_pixel:?}"
        );
        println!(
            "RenderLoop's real upload_texture/queue_tile/unlock_frame path confirmed \
             empirically to composite a queued tile draw correctly on top of its own clear, on \
             adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_mid_frame_texture_reupload_does_not_retroactively_change_earlier_queued_draws() {
        // The real regression test flush_pending_batches exists for
        // (this crate's own todo list: "re-upload/bRealtimeChanged
        // in-place update path"). Caught on review before Info.
        // bRealtimeChanged-triggered re-upload was ever wired through
        // any real caller, not discovered after it shipped: this
        // crate's own BatchPayload stores a cache_id, resolved against
        // self.texture_cache only later, at actual dispatch time
        // (dispatch_batches's own doc). Without a real flush-before-
        // replace safeguard (SetTexture.cpp:508-515's own real
        // "Shaders[ActiveProgram]->Flush(false);", quoted in flush_
        // pending_batches's own doc), re-uploading a texture under the
        // SAME cache_id mid-frame would let an EARLIER-queued draw
        // retroactively pick up the LATER re-upload's new content.
        // This test queues a real tile against cache_id 99 while it is
        // still red, re-uploads cache_id 99 as blue, then queues a
        // SECOND real tile against the same cache_id 99 -- the
        // earlier tile must still read back red, the later one blue.
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_loop mid-frame reupload offscreen test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        begin_offscreen_frame(
            &created,
            &mut loop_state,
            &render_target,
            wgpu::Color::GREEN,
        );
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );

        // Same real white draw_color convention as the plain tile test
        // above -- isolates this test from needing to also reason
        // about texture-color multiplication, white * texel == texel.
        loop_state.upload_texture(&created, 99, 1, 1, &[255, 0, 0, 255], false);
        // Same real X=0,XL=50 -> NDC x in [-1.0, 0.0] (left half)
        // geometry as render_loop_queue_tile_renders_a_real_tile_
        // through_the_real_lock_unlock_path's own hand-verified
        // derivation above.
        loop_state.queue_tile(TileDrawRequest {
            cache_id: 99,
            x: 0.0,
            y: 49.5,
            xl: 50.0,
            yl: 1.0,
            u: 0.0,
            v: 0.0,
            ul: 1.0,
            vl: 1.0,
            z: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            poly_flags: 0,
            u_scale: 1.0,
            v_scale: 1.0,
            texture_alpha: None,
            fx2: 50.0,
            fy2: 50.0,
        });

        // Real mid-frame re-upload under the SAME cache_id -- the one
        // call this whole test exists to prove is safe. Must trigger
        // RenderLoop::flush_pending_batches internally (cache_id 99 is
        // already resident), dispatching the tile queued above using
        // the still-red texture before this call's own blue bytes ever
        // reach self.texture_cache.
        loop_state.upload_texture(&created, 99, 1, 1, &[0, 0, 255, 255], false);

        // Same real symmetric formula as the left half, X=50,XL=50 ->
        // NDC x in [0.0, 1.0] (right half): rfx2*(px-fx2) with
        // rfx2=0.02, fx2=50.0 gives 0.02*(50-50)=0 at the left edge
        // and 0.02*(100-50)=1.0 at the right edge.
        loop_state.queue_tile(TileDrawRequest {
            cache_id: 99,
            x: 50.0,
            y: 49.5,
            xl: 50.0,
            yl: 1.0,
            u: 0.0,
            v: 0.0,
            ul: 1.0,
            vl: 1.0,
            z: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            poly_flags: 0,
            u_scale: 1.0,
            v_scale: 1.0,
            texture_alpha: None,
            fx2: 50.0,
            fy2: 50.0,
        });

        let did_unlock = unlock_frame(&created, &mut loop_state, false);
        assert!(
            did_unlock,
            "expected unlock_frame to find the frame begin_offscreen_frame began"
        );
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_loop mid-frame reupload offscreen test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_loop mid-frame reupload offscreen test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[255u8, 0, 0, 255],
            "expected the EARLIER-queued tile (left half) to still read back red -- the texture \
             it referenced at queue time, not the blue it was later re-uploaded as mid-frame, \
             got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected the LATER-queued tile (right half), queued after the mid-frame re-upload, \
             to read back blue, got {right_pixel:?}"
        );
        println!(
            "RenderLoop::flush_pending_batches confirmed empirically: a real mid-frame texture \
             re-upload under the same cache_id does not retroactively change an earlier-queued \
             draw's own already-resolved texture content -- earlier geometry stayed red, later \
             geometry became blue, on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_queue_point_renders_a_real_point_through_the_real_lock_unlock_path() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_loop point offscreen test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Same real call order and identity-modelviewproj substitution
        // as render_loop_flushes_queued_line_draws_through_the_real_
        // lock_unlock_path above.
        begin_offscreen_frame(&created, &mut loop_state, &render_target, wgpu::Color::BLUE);
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );

        // Same real rfx2=rfy2=0.02/fx2=fy2=50.0 derivation as the
        // line/tile tests above. X1=0.5,X2=49.5 -> (after the real
        // +-0.5 inset/expansion point_quad_corners applies) NDC x in
        // [-1.0, 0.0], the same left-half span every other offscreen
        // test in this file uses, not a coincidence: X1-0.5=0,
        // X2+0.5=50, matching the line test's own raw P1.X=0/P2.X=50
        // exactly. Y1=Y2=50 -> Y1-0.5=49.5/Y2+0.5=50.5, straddling the
        // single render row's own center the same way the tile test's
        // own Y=49.5/YL=1.0 does. Hand-computed independently (scratch
        // Python) before relying on it: top_left=(-1.0,-0.01,1),
        // top_right=(0.0,-0.01,1), bottom_right=(0.0,0.01,1),
        // bottom_left=(-1.0,0.01,1).
        loop_state.queue_point(0.5, 50.0, 49.5, 50.0, 1.0, 50.0, 50.0, [0.0, 1.0, 0.0]);
        let did_unlock = unlock_frame(&created, &mut loop_state, false);
        assert!(
            did_unlock,
            "expected unlock_frame to find the frame begin_offscreen_frame began"
        );
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_loop point offscreen test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_loop point offscreen test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[0u8, 255, 0, 255],
            "expected the point queued via RenderLoop::queue_point and flushed by the real \
             unlock_frame to actually render, got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected begin_offscreen_frame's own clear pass to survive outside the drawn \
             point's span, got {right_pixel:?}"
        );
        println!(
            "RenderLoop's real queue_point/unlock_frame path confirmed empirically to \
             composite a queued point draw correctly on top of its own clear, on adapter {:?} \
             backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_queue_triangles_renders_a_real_mesh_through_the_real_lock_unlock_path() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_loop triangles offscreen test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Same real call order and identity-modelviewproj substitution
        // as the tile/point tests above -- light_color_intensity at
        // byte 520 set to 1.0 (neutral), same real reasoning as
        // gouraud_pipeline_renders_a_real_textured_triangle_mesh_
        // into_an_offscreen_texture's own doc (draw.rs).
        begin_offscreen_frame(&created, &mut loop_state, &render_target, wgpu::Color::BLUE);
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            520,
            &1.0f32.to_le_bytes(),
        );
        loop_state.upload_texture(&created, 99, 1, 1, &[255, 0, 0, 255], false);

        // Real GouraudVertex coords pass straight through to
        // modelviewproj_mat with no RFX2/FX2 screen-to-clip transform
        // at all (unlike Tile/Point/Line) -- queue_triangles' own doc
        // and GouraudVertex's own doc both have the full real
        // citation for why. Direct NDC quad (two triangles, matching
        // the real BufferVert's own per-vertex extraction shape):
        // left half of the 64px target, same established visual
        // convention as every other RenderLoop end-to-end test here.
        let light_color = [1.0, 1.0, 1.0, 1.0];
        let fog_color = [0.0, 0.0, 0.0, 0.0];
        let vertices = [
            draw::GouraudVertexInput {
                coords: [-1.0, -1.0, 0.0],
                tex_coords: [0.0, 1.0],
                light_color,
                fog_color,
            },
            draw::GouraudVertexInput {
                coords: [0.0, -1.0, 0.0],
                tex_coords: [1.0, 1.0],
                light_color,
                fog_color,
            },
            draw::GouraudVertexInput {
                coords: [0.0, 1.0, 0.0],
                tex_coords: [1.0, 0.0],
                light_color,
                fog_color,
            },
            draw::GouraudVertexInput {
                coords: [-1.0, -1.0, 0.0],
                tex_coords: [0.0, 1.0],
                light_color,
                fog_color,
            },
            draw::GouraudVertexInput {
                coords: [0.0, 1.0, 0.0],
                tex_coords: [1.0, 0.0],
                light_color,
                fog_color,
            },
            draw::GouraudVertexInput {
                coords: [-1.0, 1.0, 0.0],
                tex_coords: [0.0, 0.0],
                light_color,
                fog_color,
            },
        ];
        loop_state.queue_triangles(
            TrianglesDrawRequest {
                cache_id: 99,
                poly_flags: 0,
                u_scale: 1.0,
                v_scale: 1.0,
                texture_alpha: None,
                diffuse: None,
            },
            &vertices,
        );
        let did_unlock = unlock_frame(&created, &mut loop_state, false);
        assert!(
            did_unlock,
            "expected unlock_frame to find the frame begin_offscreen_frame began"
        );
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_loop triangles offscreen test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_loop triangles offscreen test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[255u8, 0, 0, 255],
            "expected the mesh queued via RenderLoop::queue_triangles and flushed by the real \
             unlock_frame to actually render, got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected begin_offscreen_frame's own clear pass to survive outside the drawn \
             mesh's span, got {right_pixel:?}"
        );
        println!(
            "RenderLoop's real upload_texture/queue_triangles/unlock_frame path confirmed \
             empirically to composite a queued triangle mesh draw correctly on top of its own \
             clear, on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_queue_complex_surface_renders_a_real_surface_through_the_real_lock_unlock_path()
    {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_loop complex offscreen test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        begin_offscreen_frame(&created, &mut loop_state, &render_target, wgpu::Color::BLUE);
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            520,
            &1.0f32.to_le_bytes(),
        );
        loop_state.upload_texture(&created, 99, 1, 1, &[255, 0, 0, 255], false);

        // No real light-map/fog-map (request.light_map/fog_map both
        // None below) -- deliberately exercises RenderLoop::dummy_
        // texture's own real lazy-creation path, not just the
        // already-proven-elsewhere real-texture path. Same trivial
        // identity x_axis/y_axis UV mapping as the draw.rs-level
        // ComplexPipeline test above (a 1x1 texture makes the exact
        // UV value irrelevant to the sampled color).
        let vertices = [
            draw::ComplexVertexInput {
                coords: [-1.0, -1.0, 0.0],
            },
            draw::ComplexVertexInput {
                coords: [0.0, -1.0, 0.0],
            },
            draw::ComplexVertexInput {
                coords: [0.0, 1.0, 0.0],
            },
            draw::ComplexVertexInput {
                coords: [-1.0, -1.0, 0.0],
            },
            draw::ComplexVertexInput {
                coords: [0.0, 1.0, 0.0],
            },
            draw::ComplexVertexInput {
                coords: [-1.0, 1.0, 0.0],
            },
        ];
        loop_state.queue_complex_surface(
            ComplexSurfaceDrawRequest {
                diffuse: ComplexTextureLayer {
                    cache_id: 99,
                    u_scale: 1.0,
                    v_scale: 1.0,
                    pan_x: 0.0,
                    pan_y: 0.0,
                },
                light_map: None,
                fog_map: None,
                poly_flags: 0,
                texture_fields: Some((1.0, 0.0, 1.0)),
                c_alpha: 255,
                x_axis: [1.0, 0.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0, 0.0],
            },
            &vertices,
        );
        let did_unlock = unlock_frame(&created, &mut loop_state, false);
        assert!(
            did_unlock,
            "expected unlock_frame to find the frame begin_offscreen_frame began"
        );
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_loop complex offscreen test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_loop complex offscreen test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[255u8, 0, 0, 255],
            "expected the surface queued via RenderLoop::queue_complex_surface and flushed by \
             the real unlock_frame to actually render (including the real dummy-texture path \
             for the absent light-map/fog-map), got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected begin_offscreen_frame's own clear pass to survive outside the drawn \
             surface's span, got {right_pixel:?}"
        );
        println!(
            "RenderLoop's real upload_texture/queue_complex_surface/unlock_frame path (no real \
             light-map/fog-map -- exercising the real dummy-texture path) confirmed \
             empirically to composite a queued complex surface draw correctly on top of its \
             own clear, on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // The real regression test the ordered Vec<Batch> design exists
    // for (this crate's own todo list: "use one ordered Vec<Batch...>
    // ... not separate per-primitive-type queues flushed in a fixed
    // order -- interleaved real draw calls must composite in real
    // call order"). Queues a tile FIRST, then a line SECOND, both
    // opaque, both covering the identical screen-space region --
    // real call order means the line (queued later) must win the
    // overlap, painting over the tile, not the other way around. A
    // hypothetical fixed-kind-order flush (all tiles, then all lines,
    // or vice versa, regardless of real queue order) would get this
    // backwards for exactly this input. Negative control: temporarily
    // sorted RenderLoop::batches by kind (Lines before Tiles) before
    // the flush loop, confirmed this test then fails (red, not
    // green), reverted.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_composites_interleaved_line_and_tile_draws_in_real_call_order() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render_loop interleave offscreen test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        begin_offscreen_frame(&created, &mut loop_state, &render_target, wgpu::Color::BLUE);
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );
        loop_state.upload_texture(&created, 99, 1, 1, &[255, 255, 255, 255], false);

        // Queued first: an opaque RED tile, same geometry as render_
        // loop_queue_tile_renders_a_real_tile_through_the_real_lock_
        // unlock_path above (NDC x in [-1.0, 0.0], y straddling 0).
        loop_state.queue_tile(TileDrawRequest {
            cache_id: 99,
            x: 0.0,
            y: 49.5,
            xl: 50.0,
            yl: 1.0,
            u: 0.0,
            v: 0.0,
            ul: 1.0,
            vl: 1.0,
            z: 1.0,
            color: [1.0, 0.0, 0.0, 1.0],
            poly_flags: 0,
            u_scale: 1.0,
            v_scale: 1.0,
            texture_alpha: None,
            fx2: 50.0,
            fy2: 50.0,
        });
        // Queued second: an opaque GREEN line covering the identical
        // NDC x in [-1.0, 0.0] region (same P1/P2 as the plain line
        // test above). Both SimpleLinePipeline and TilePipeline use
        // blend: None (replace, not blend) -- whichever actually
        // renders *last* wins the overlap outright, an unambiguous
        // test oracle for real call-order compositing.
        loop_state.queue_line(
            [0.0, 50.0, 1.0],
            [50.0, 50.0, 1.0],
            50.0,
            50.0,
            [0.0, 1.0, 0.0, 1.0],
        );
        let did_unlock = unlock_frame(&created, &mut loop_state, false);
        assert!(did_unlock);
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_loop interleave offscreen test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render_loop interleave offscreen test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let overlap_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            overlap_pixel,
            &[0u8, 255, 0, 255],
            "expected the line (queued SECOND) to win the overlap with the tile (queued \
             FIRST), proving real call order, not primitive kind, decides final compositing; \
             got {overlap_pixel:?} -- red here would mean tiles are being flushed after lines \
             regardless of real queue order"
        );
        println!(
            "RenderLoop confirmed empirically to composite interleaved tile-then-line draws in \
             real call order (the line queued second wins the overlap), on adapter {:?} \
             backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // The real regression test the per-segment redesign exists for:
    // two SetSceneNode calls in one frame, each with a genuinely
    // different modelviewproj_mat, with one line queued under each --
    // proving each line is drawn with *its own* segment's matrices,
    // not whichever SetSceneNode call happened to run last (the real
    // bug this redesign fixes: RenderLoop::frame_uniforms_segments's
    // own doc has the full reasoning).
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_draws_each_queued_line_with_its_own_segments_matrices() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("multi-segment test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        // Real lock_frame logic, clear BLACK (distinct from both
        // lines' own real colors below).
        begin_offscreen_frame(
            &created,
            &mut loop_state,
            &render_target,
            wgpu::Color::BLACK,
        );

        // Segment 0: identity modelviewproj_mat (same real technique
        // as render_loop_flushes_queued_line_draws_through_the_real_
        // lock_unlock_path above).
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );
        // Green line queued under segment 0: P1=(0,50,1), P2=(50,50,1)
        // -> pre-matrix coords (-1,0,1)-(0,0,1) (same real derivation
        // as the single-segment test above) -> under identity, spans
        // NDC x in [-1.0, 0.0] (left half, pixels 0-31).
        loop_state.queue_line(
            [0.0, 50.0, 1.0],
            [50.0, 50.0, 1.0],
            50.0,
            50.0,
            [0.0, 1.0, 0.0, 1.0],
        );

        // Segment 1: a genuinely different modelviewproj_mat -- same
        // identity diagonal except m00 = -1.0 instead of 1.0 (flips X
        // only; m33 stays 1.0, so the homogeneous divide is still by
        // w=1, not also flipped -- confirmed by hand before relying
        // on it: a point at pre-matrix x=-1 maps to x=-1*-1=1, a point
        // at x=0 maps to x=0*-1=0, so a line spanning [-1,0] under
        // identity spans [0,1] under this matrix instead).
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut x_flip_diagonal = identity_diagonal;
        x_flip_diagonal[0..4].copy_from_slice(&(-1.0f32).to_le_bytes());
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(1).unwrap(),
            256,
            &x_flip_diagonal,
        );
        // Red line queued under segment 1: the exact same raw screen
        // coords as the green line above -- only the segment's own
        // matrix differs, isolating this test to that one real
        // variable. Under the X-flip matrix, spans NDC x in [0.0,
        // 1.0] (right half, pixels 32-63).
        loop_state.queue_line(
            [0.0, 50.0, 1.0],
            [50.0, 50.0, 1.0],
            50.0,
            50.0,
            [1.0, 0.0, 0.0, 1.0],
        );

        unlock_frame(&created, &mut loop_state, false);
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("multi-segment test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("multi-segment test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[0u8, 255, 0, 255],
            "expected the segment-0 (identity matrix) line to render green in the left half, \
             got {left_pixel:?} -- a wrong value here means it was drawn with the wrong \
             segment's matrix"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[255u8, 0, 0, 255],
            "expected the segment-1 (X-flip matrix) line to render red in the right half, got \
             {right_pixel:?} -- a wrong value here means it was drawn with the wrong segment's \
             matrix"
        );
        println!(
            "RenderLoop confirmed empirically to draw each queued line with its own real \
             segment's matrices, not whichever SetSceneNode call happened to run last, on \
             adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // The real regression test the MAX_DRAWS chunking fix exists for:
    // queues draw::MAX_DRAWS (256) "filler" lines, all landing on the
    // left half of the target, forcing unlock_frame's own flush to
    // chunk -- then one more line (the 257th, the real first line of
    // the second chunk) at a distinct, non-overlapping position.
    // Proves the 257th line actually renders (an earlier version of
    // this crate's own flush logic silently dropped every line past
    // MAX_DRAWS in a given segment -- caught on review, fixed, this
    // is the real regression test for that fix, not present before).
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn render_loop_chunks_more_than_max_draws_lines_instead_of_dropping_them() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let mut loop_state = RenderLoop::new();
        let render_target = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("max-draws chunking test target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        begin_offscreen_frame(
            &created,
            &mut loop_state,
            &render_target,
            wgpu::Color::BLACK,
        );
        loop_state.set_scene_node(
            &created.device,
            &created.queue,
            &reference_perspective_scene(),
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        created.queue.write_buffer(
            loop_state.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );
        // draw::MAX_DRAWS (256, private to the draw module) filler
        // lines, all P1=(0,50,1)-P2=(50,50,1) (left half, NDC x in
        // [-1.0, 0.0], same real derivation as the other RenderLoop
        // tests above) -- forces exactly one chunk flush inside
        // unlock_frame.
        for _ in 0..draw::MAX_DRAWS {
            loop_state.queue_line(
                [0.0, 50.0, 1.0],
                [50.0, 50.0, 1.0],
                50.0,
                50.0,
                [0.0, 0.0, 1.0, 1.0],
            );
        }
        // The 257th line: P1=(50,50,1)-P2=(100,50,1) -> NDC x in
        // [0.0, 1.0] (right half, hand-verified independently before
        // relying on it, not assumed from the left-half case alone),
        // a genuinely different, non-overlapping position from every
        // filler line above.
        loop_state.queue_line(
            [50.0, 50.0, 1.0],
            [100.0, 50.0, 1.0],
            50.0,
            50.0,
            [0.0, 1.0, 0.0, 1.0],
        );
        unlock_frame(&created, &mut loop_state, false);
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("max-draws chunking test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder =
            created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("max-draws chunking test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let filler_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            filler_pixel,
            &[0u8, 0, 255, 255],
            "expected a filler-line pixel (left half) to read back blue, got {filler_pixel:?}"
        );
        let chunked_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            chunked_pixel,
            &[0u8, 255, 0, 255],
            "expected the 257th line (the real first line of the second chunk) to actually \
             render green, got {chunked_pixel:?} -- a black (clear-color) result here would \
             mean lines past MAX_DRAWS are still being silently dropped"
        );
        println!(
            "RenderLoop confirmed empirically to chunk more than MAX_DRAWS queued lines into \
             multiple record() calls instead of dropping the excess, on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // Needs a real GPU adapter. CI runners (ci.yml) have none. Run with
    // `cargo test -p hp2-wgpu -- --ignored`. Don't make it pass when no
    // adapter is found: that would hide real failures on machines that
    // do have one.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn request_adapter_sync_finds_a_real_adapter_on_this_machine() {
        let result = request_adapter_sync(BackendChoice::Auto);
        assert!(
            result.is_some(),
            "expected a real GPU adapter to be available"
        );
        let (name, backend) = result.unwrap();
        assert!(!name.is_empty(), "adapter name should not be empty");
        println!("found adapter {name:?} backend {}", backend_name(backend));
    }

    // Same #[ignore] reasoning as above. Proves genuine device
    // communication, not just that a handle type-checks: device.limits()
    // is asserted equal to the adapter's own limits (not merely nonzero
    // -- a nonzero check alone would pass for the fixed Limits::default()
    // baseline too, proving nothing hardware-specific), and a real (if
    // empty) command buffer is submitted and polled for actual
    // completion.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn create_device_sync_produces_a_working_device() {
        let result = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>);
        assert!(result.is_some(), "expected a real device to be created");
        let created = result.unwrap();
        assert!(!created.adapter_name.is_empty());
        assert_eq!(
            created.device.limits(),
            created.adapter_limits,
            "the device must report exactly the limits it was created with"
        );
        let encoder = created
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        created.queue.submit(std::iter::once(encoder.finish()));
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        println!(
            "created device on adapter {:?} backend {}, max_texture_dimension_2d={}, \
             TEXTURE_COMPRESSION_BC granted={}",
            created.adapter_name,
            backend_name(created.backend),
            created.adapter_limits.max_texture_dimension_2d,
            created
                .device
                .features()
                .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        );
    }

    // Proves an explicit BackendChoice is genuinely honored end to
    // end, not silently treated as Auto -- discriminates correctly
    // on this machine regardless of whether a real GL adapter exists
    // here or not (caught on review: an earlier version of this test
    // used `BackendChoice::Metal`, which this machine's `Auto` would
    // also pick, so it could not tell "the choice was honored" apart
    // from "Auto happened to land on the same backend anyway"; a
    // second version then tried `enumerate_adapters` to work around
    // that, more complex than necessary). The real, simpler proof:
    // `BackendChoice::Gl` is a backend `Auto` (`Backends::PRIMARY |
    // Backends::GL`) would only ever fall back to, never prefer over
    // Metal on this machine -- so if the explicit choice were
    // silently ignored and treated as Auto, this call would return
    // `Some(Metal)`. `None` (genuinely no GL adapter) or
    // `Some(Gl)` (a GL adapter that really exists) both correctly
    // prove the choice reached `to_wgpu`/`build_instance`; only
    // `Some(Metal)` (or any other backend) would mean it did not.
    // This machine genuinely has no GL adapter (confirmed by
    // actually running this test and reading the real `None` result,
    // not assumed): not because Apple removed OpenGL -- the
    // framework still ships on macOS -- but because wgpu's own GL
    // backend goes through EGL/ANGLE on every platform, and macOS has
    // neither present by default (an earlier version of this comment
    // said "Apple deprecated and removed OpenGL support", which
    // overstated the real cause -- corrected on review). This also
    // means the real `Backends::GL`-only retry inside
    // `create_device_sync` genuinely runs and genuinely fails here
    // too (`choice` is `Gl` already, so the retry is skipped per its
    // own doc -- this call exercises the no-retry-when-already-Gl
    // branch specifically, not the retry path itself).
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn create_device_sync_honors_an_explicit_backend_choice() {
        let result = create_device_sync(BackendChoice::Gl, None::<HeadlessSurfaceCreator>);
        match result {
            None => println!(
                "BackendChoice::Gl genuinely found no adapter on this machine -- correct \
                 discriminating result: Auto would have returned Some(Metal) instead if the \
                 explicit choice had been silently ignored"
            ),
            Some(created) => {
                assert_eq!(
                    created.backend,
                    wgpu::Backend::Gl,
                    "expected BackendChoice::Gl to either find no adapter at all, or a real Gl \
                     one -- Some(Metal) specifically would mean the explicit choice was \
                     silently ignored and treated as Auto instead"
                );
                println!(
                    "BackendChoice::Gl honored: created device on adapter {:?} backend {}",
                    created.adapter_name,
                    backend_name(created.backend)
                );
            }
        }
    }

    #[test]
    fn backend_name_covers_every_plan_named_backend() {
        assert_eq!(backend_name(wgpu::Backend::Metal), "Metal");
        assert_eq!(backend_name(wgpu::Backend::Vulkan), "Vulkan");
        assert_eq!(backend_name(wgpu::Backend::Dx12), "Dx12");
        assert_eq!(backend_name(wgpu::Backend::Gl), "Gl");
        assert_eq!(backend_name(wgpu::Backend::BrowserWebGpu), "BrowserWebGpu");
    }

    #[test]
    fn resolve_present_mode_uses_the_requested_mode_when_supported() {
        let supported = [
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::FifoRelaxed,
        ];
        assert_eq!(
            resolve_present_mode(wgpu::PresentMode::Immediate, &supported),
            wgpu::PresentMode::Immediate
        );
        assert_eq!(
            resolve_present_mode(wgpu::PresentMode::FifoRelaxed, &supported),
            wgpu::PresentMode::FifoRelaxed
        );
        assert_eq!(
            resolve_present_mode(wgpu::PresentMode::Fifo, &supported),
            wgpu::PresentMode::Fifo
        );
    }

    #[test]
    fn resolve_present_mode_falls_back_to_fifo_when_unsupported() {
        // A real, not hypothetical, case: Fifo is the only mode wgpu
        // guarantees everywhere, so a surface whose own real
        // capabilities list only Fifo is a real condition, not a
        // contrived empty slice.
        let fifo_only = [wgpu::PresentMode::Fifo];
        assert_eq!(
            resolve_present_mode(wgpu::PresentMode::Immediate, &fifo_only),
            wgpu::PresentMode::Fifo,
            "expected an unsupported Immediate request to fall back to Fifo"
        );
        assert_eq!(
            resolve_present_mode(wgpu::PresentMode::FifoRelaxed, &fifo_only),
            wgpu::PresentMode::Fifo,
            "expected an unsupported FifoRelaxed request to fall back to Fifo"
        );
        // Even an empty supported list (degenerate, but not a panic
        // case) must fall back, not index out of bounds or crash.
        assert_eq!(
            resolve_present_mode(wgpu::PresentMode::Fifo, &[]),
            wgpu::PresentMode::Fifo
        );
    }

    // Pins the Rust half of the real C++/Rust FFI contract
    // (`BackendChoice::from_ffi_discriminant`'s own doc: `WgpuDrv.cpp`'s
    // `StaticConstructor` registers a UEnum with members in exactly
    // this order, 0=Auto..4=Gl). This test only exercises the Rust
    // side -- it cannot by itself catch a future reorder of
    // `WgpuDrv.cpp`'s own `Backends->Names` array, which would still
    // pass this test while silently selecting the wrong backend from
    // a correctly-set ini value (an earlier version of this comment
    // overclaimed "either side"; corrected on review). Keep the two
    // orders in sync by hand; no automated cross-language check exists
    // for this. No GPU needed.
    #[test]
    fn from_ffi_discriminant_matches_the_real_ue1_enum_order() {
        assert_eq!(
            BackendChoice::from_ffi_discriminant(0),
            Some(BackendChoice::Auto)
        );
        assert_eq!(
            BackendChoice::from_ffi_discriminant(1),
            Some(BackendChoice::Metal)
        );
        assert_eq!(
            BackendChoice::from_ffi_discriminant(2),
            Some(BackendChoice::Vulkan)
        );
        assert_eq!(
            BackendChoice::from_ffi_discriminant(3),
            Some(BackendChoice::Dx12)
        );
        assert_eq!(
            BackendChoice::from_ffi_discriminant(4),
            Some(BackendChoice::Gl)
        );
        assert_eq!(
            BackendChoice::from_ffi_discriminant(5),
            None,
            "expected an out-of-range discriminant (one past the real UEnum's last member) to \
             return None, not silently wrap to a valid variant"
        );
        assert_eq!(BackendChoice::from_ffi_discriminant(u32::MAX), None);
    }

    // Plan's own V2 verification: "cargo test -p hp2-wgpu validates
    // every .wgsl via naga". No GPU adapter needed -- parsing and
    // validating WGSL is pure CPU-side work naga does without touching
    // any backend, so this runs in CI (ci.yml's runners have no GPU)
    // unlike the two #[ignore]d tests above. Iterates
    // `src/shaders/*.wgsl` by directory listing rather than naming
    // `simple.wgsl` specifically, so a future shader file can't skip
    // validation just by existing without a matching test update; a
    // non-empty-directory assertion guards against an empty glob
    // silently "passing".
    //
    // Capabilities::empty(), not ::default(): checked naga's own
    // source (naga-30.0.1/src/valid/mod.rs's `impl Default for
    // Capabilities`) before relying on either -- `default()` is
    // `MULTISAMPLED_SHADING | CUBE_ARRAY_TEXTURES`, not an
    // empty/no-extensions baseline as an earlier version of this
    // comment wrongly claimed (caught on review). `empty()` is the
    // real conservative choice, and the right one for this plan: it
    // matches baseline WebGPU with no optional capabilities assumed,
    // so a shader that accidentally relies on a native-only capability
    // fails here instead of surfacing later as a Phase 8 web-target
    // regression.
    #[test]
    fn every_wgsl_shader_parses_and_validates() {
        fn validate(label: &str, source: &str) {
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{label} failed to parse: {e}"));
            let mut validator = naga::valid::Validator::new(
                naga::valid::ValidationFlags::default(),
                naga::valid::Capabilities::empty(),
            );
            validator
                .validate(&module)
                .unwrap_or_else(|e| panic!("{label} failed naga validation: {e}"));
        }

        let shader_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders");
        let mut checked = 0usize;
        for entry in std::fs::read_dir(shader_dir)
            .unwrap_or_else(|e| panic!("failed to list {shader_dir}: {e}"))
        {
            let path = entry
                .unwrap_or_else(|e| panic!("failed to list {shader_dir}: {e}"))
                .path();
            if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_else(|| panic!("non-UTF-8 shader filename: {}", path.display()));
            if stem == "common" {
                // common.wgsl has no @vertex/@fragment entry points and
                // is never a standalone shader -- validated here on its
                // own (naga accepts a module of only unused helper
                // functions) rather than via shader_source(), which
                // would concatenate it with itself.
                let source = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
                validate("common.wgsl", &source);
            } else {
                // Every other shader is validated the same way real
                // pipeline creation will eventually build it --
                // common.wgsl concatenated in front, via the same
                // shader_source() helper, not a second ad hoc
                // concatenation that could drift out of sync with it.
                validate(
                    &format!("{stem}.wgsl (+ common.wgsl)"),
                    &shader_source(stem),
                );
            }
            checked += 1;
        }
        assert!(
            checked > 0,
            "expected at least one .wgsl file under {shader_dir}, found none -- an empty \
             directory must not make this test vacuously pass"
        );
    }

    // Settles, empirically, what the DrawComplex_GLSL.cpp scoping
    // paragraph above said about `textureSample` inside non-uniform
    // control flow (a loop whose exit depends on a per-fragment
    // texture-read value) -- rather than leaving it an unverified
    // expectation, or (worse) restating it as a "fact" without ever
    // actually trying it, which is what happened before this was
    // caught on review. Real, surprising result: naga's validator
    // accepts this. It does not implement (or does not apply here)
    // WGSL's full uniformity-analysis restriction on implicit-
    // derivative texture sampling the way the spec describes it --
    // confirmed by running this exact repro, not assumed from how
    // strict WGSL/Tint's rules are described in general. This means
    // `every_wgsl_shader_parses_and_validates`'s naga-based CI check
    // cannot be relied on to catch a parallax loop ported with plain
    // `textureSample` left in -- a real backend (Metal/Tint, or
    // whatever a web build's WebGPU implementation uses) may still
    // reject it, or worse, silently produce wrong derivatives, but
    // naga validation passing is not evidence either way. The
    // `textureSampleGrad`/`textureSampleLevel` porting note in that
    // scoping paragraph still stands on its own correctness grounds
    // (matching the WGSL spec's own written restriction), just not on
    // "naga will catch it if we get it wrong". Not real shader code
    // from this crate -- a minimal, self-contained repro of the exact
    // shape the parallax loops have, small enough to keep inline
    // rather than as a separate `.wgsl` fixture file.
    #[test]
    fn naga_accepts_texture_sample_in_non_uniform_control_flow() {
        let source = r#"
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;

@fragment
fn f(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    var c = vec4<f32>(0.0);
    var h = 1.0;
    loop {
        if h <= 0.0 {
            break;
        }
        c = textureSample(t, s, uv);
        h -= c.r;
    }
    return c;
}
"#;
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("repro shader failed to parse: {e}"));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::default(),
            naga::valid::Capabilities::empty(),
        );
        let result = validator.validate(&module);
        assert!(
            result.is_ok(),
            "expected naga to accept textureSample inside non-uniform control flow (this test \
             exists to document and pin that real, surprising behavior -- if naga now rejects \
             it, a naga upgrade changed this and every_wgsl_shader_parses_and_validates can \
             newly be relied on to catch it; update the doc paragraph this test references, \
             don't just loosen this assertion): {result:?}"
        );
    }

    // `every_wgsl_shader_parses_and_validates`'s naga Capabilities::empty()
    // check validated `simple.wgsl` even while it still used
    // `var<storage, read>` for its draw-parameters array -- naga
    // capabilities gate *language features* (e.g. a binding-array
    // extension), not per-limit numeric caps like "zero storage
    // buffers", so that check alone cannot catch a limit regression
    // like the one this test exists to catch (caught on review).
    // This creates a real device with `required_limits:
    // wgpu::Limits::downlevel_webgl2_defaults()` -- not the adapter's
    // own limits the way `create_device_sync` does for its normal
    // callers -- and builds the exact same pipeline
    // `simple_wgsl_builds_a_render_pipeline_on_a_real_adapter` does.
    // wgpu validates a created pipeline against the *device's* limits,
    // not the adapter's (confirmed by this test actually running,
    // same Metal backend as every other real-GPU test here -- there is
    // no WebGL2/Gl backend available to run this on in this
    // environment, so this proves the shader survives WebGL2's binding
    // limits on whatever backend is actually present, not that it
    // runs correctly under a real WebGL2/Gl backend, which remains
    // unverified), so a storage binding (or any other
    // downlevel-incompatible requirement) fails pipeline creation here
    // even on a desktop GPU that would otherwise happily support it.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn simple_wgsl_pipeline_builds_under_webgl2_downlevel_limits() {
        let instance = build_instance(BackendChoice::Auto);
        let adapter =
            request_adapter_raw(&instance, None).expect("expected a real GPU adapter to exist");
        let downlevel_limits = wgpu::Limits::downlevel_webgl2_defaults();
        let (device, _queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("hp2-wgpu device (WebGL2 downlevel limits)"),
                required_limits: downlevel_limits,
                ..Default::default()
            }))
            .unwrap_or_else(|e| {
                panic!(
                    "expected this adapter to support WebGL2's downlevel limits (it supports \
                     strictly more than a real WebGL2 backend would): {e}"
                )
            });
        // Proves the constraint is actually in force on this device,
        // not merely requested -- wgpu's own docs describe
        // required_limits as a *minimum* the device must support, not
        // a hard ceiling every backend enforces downward, so without
        // this check a backend that silently granted more than asked
        // could make the rest of this test pass for the wrong reason
        // (plenty of real storage buffers available) rather than the
        // one it claims to prove (the shader has none to begin with).
        assert_eq!(
            device.limits().max_storage_buffers_per_shader_stage,
            0,
            "expected the device to actually report zero storage buffers under \
             downlevel_webgl2_defaults -- if this fails, the device granted more than \
             requested and the rest of this test no longer proves what its name claims"
        );
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal_scope = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("simple.wgsl (WebGL2 downlevel limits)"),
            source: wgpu::ShaderSource::Wgsl(shader_source("simple").into()),
        });
        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 12,
                shader_location: 1,
            },
        ];
        let _pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple.wgsl pipeline (WebGL2 downlevel limits)"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let internal_err = pollster::block_on(internal_scope.pop());
        let validation_err = pollster::block_on(validation_scope.pop());
        if let Some(err) = validation_err.or(internal_err) {
            panic!(
                "simple.wgsl's pipeline failed to build under WebGL2 downlevel limits -- this \
                 is exactly the bug class the storage-buffer-vs-uniform-array fix exists to \
                 prevent: {err}"
            );
        }
        println!(
            "simple.wgsl's pipeline built successfully under wgpu::Limits::downlevel_webgl2_defaults()"
        );
    }

    // Negative control for the two real-GPU pipeline tests below: proves
    // the push_error_scope/pop_error_scope mechanism they both rely on
    // actually catches a real failure, rather than only ever observing
    // `None` regardless of whether anything went wrong (which an error
    // scope that silently never captures anything would also produce).
    // Deliberately creates a pipeline whose vertex buffer layout
    // declares only one Float32x3 attribute at location 0 while
    // simple.wgsl's vs_main also requires a location-1 `u32` --
    // asserts the resulting pipeline creation reports a real error
    // through the Validation-filtered scope.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn mismatched_vertex_layout_is_caught_by_the_error_scope() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let scope = created
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let module = created
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("simple.wgsl (negative control)"),
                source: wgpu::ShaderSource::Wgsl(shader_source("simple").into()),
            });
        // Only location 0 (coords) -- vs_main's location 1 (draw_id) is
        // left undeclared on purpose.
        let incomplete_attributes = [wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }];
        let _pipeline = created
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("simple.wgsl pipeline (negative control)"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 12,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &incomplete_attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                // Real fragment stage + color target, identical to the
                // passing pipeline test below -- the *only* difference
                // from that test is the incomplete vertex attributes
                // above, so a caught error here can only be attributed
                // to that mismatch, not (as an earlier draft of this
                // test did, caught by actually reading its real error
                // message instead of trusting `is_some()`) to an
                // unrelated "no render target specified" error from
                // `fragment: None`.
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let err = pollster::block_on(scope.pop());
        let err = err.unwrap_or_else(|| {
            panic!(
                "expected a real validation error for a vertex buffer layout missing the \
                 shader's location-1 input -- if this is None, the error-scope mechanism the \
                 other real-GPU pipeline tests rely on is not actually catching failures"
            )
        });
        let message = format!("{err:?}");
        assert!(
            message.contains("location: 1") && message.contains("Missing"),
            "expected the caught error to specifically mention the missing location-1 \
             input, not some unrelated validation failure -- got: {message}"
        );
        println!("negative control confirmed: error scope caught {err:?}");
    }

    // Stronger than naga validation alone -- and stronger than only
    // calling `create_shader_module`, which an earlier version of this
    // test did and wrongly described as proving a Metal compile
    // (caught on review): reading wgpu-hal's own Metal backend
    // (wgpu-hal-30.0.1/src/metal/device.rs's `create_shader_module`)
    // shows the `ShaderInput::Naga` case -- what `ShaderSource::Wgsl`
    // actually produces -- only wraps the naga IR
    // (`ShaderModuleSource::Naga(naga)`), no MSL text, no
    // `device.newLibraryWithSource_options_error` call (the real
    // Metal shader compiler) at all. That real call only happens for
    // the separate `ShaderInput::Msl` case. The real MSL translation
    // and compiler invocation for a WGSL-sourced module happens later,
    // inside `create_render_pipeline` -- so this test creates a real
    // (if minimal) pipeline, not just a shader module, to actually
    // exercise that path. `layout: None` lets wgpu derive the bind
    // group layout from `simple.wgsl`'s own `@group`/`@binding`
    // declarations instead of this test hand-constructing one that
    // could itself drift out of sync with the shader and mask a real
    // mismatch.
    //
    // Two nested error scopes, not one: a failed MSL translation or
    // Metal compile inside create_render_pipeline could in principle
    // surface as an Internal error rather than a Validation error, and
    // a Validation-only scope would silently miss it -- so this pops
    // both and fails on either.
    //
    // push_error_scope(Validation)/pop_error_scope: wgpu's
    // create_shader_module/create_render_pipeline return their handles
    // unconditionally (not a Result), with any real compile error
    // surfacing later through this out-of-band error-scope mechanism
    // instead -- confirmed by reading their real signatures before
    // relying on this, not assumed from create_device_sync's own
    // device.poll pattern. The negative-control test above proves this
    // mechanism actually catches a real failure rather than only ever
    // observing `None`. Same #[ignore] reasoning as the other
    // real-GPU tests above: needs a real adapter, CI runners have
    // none.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn simple_wgsl_builds_a_render_pipeline_on_a_real_adapter() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let validation_scope = created
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let internal_scope = created.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = created
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("simple.wgsl"),
                source: wgpu::ShaderSource::Wgsl(shader_source("simple").into()),
            });
        // Matches VertexInput: @location(0) coords: vec3<f32>,
        // @location(1) draw_id: u32.
        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 12, // size_of::<[f32; 3]>()
                shader_location: 1,
            },
        ];
        let _pipeline = created
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("simple.wgsl pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 16, // 3 * size_of::<f32>() + size_of::<u32>()
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &vertex_attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let internal_err = pollster::block_on(internal_scope.pop());
        let validation_err = pollster::block_on(validation_scope.pop());
        if let Some(err) = validation_err.or(internal_err) {
            panic!(
                "simple.wgsl's pipeline failed to compile on real adapter {:?} backend {}: {err}",
                created.adapter_name,
                backend_name(created.backend)
            );
        }
        println!(
            "simple.wgsl's pipeline compiled successfully (real MSL translation + Metal \
             compiler invocation exercised) on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // Same reasoning as simple_wgsl_builds_a_render_pipeline_on_a_real_adapter
    // (real MSL translation + Metal compiler invocation, not just naga
    // validation) applied to tile.wgsl, which actually exercises a
    // texture/sampler bind group (group 2) that simple.wgsl has no
    // equivalent of -- this is the first test in this crate to prove a
    // texture+sampler binding declared in WGSL, with `layout: None`
    // deriving the group-2 bind group layout from the shader itself,
    // actually survives a real Metal pipeline creation.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn tile_wgsl_builds_a_render_pipeline_on_a_real_adapter() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let validation_scope = created
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let internal_scope = created.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = created
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("tile.wgsl"),
                source: wgpu::ShaderSource::Wgsl(shader_source("tile").into()),
            });
        // Matches VertexInput: @location(0) coords: vec3<f32>,
        // @location(1) draw_id: u32, @location(2) tex_coords: vec2<f32>.
        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 16,
                shader_location: 2,
            },
        ];
        let _pipeline = created
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tile.wgsl pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 24, // 3*f32 + u32 + 2*f32 = 12 + 4 + 8
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &vertex_attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let internal_err = pollster::block_on(internal_scope.pop());
        let validation_err = pollster::block_on(validation_scope.pop());
        if let Some(err) = validation_err.or(internal_err) {
            panic!(
                "tile.wgsl's pipeline failed to compile on real adapter {:?} backend {}: {err}",
                created.adapter_name,
                backend_name(created.backend)
            );
        }
        println!(
            "tile.wgsl's pipeline compiled successfully (real MSL translation + Metal \
             compiler invocation exercised, including a real texture/sampler bind group) \
             on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // Same reasoning as the other real-GPU pipeline tests (real MSL
    // translation + Metal compiler invocation, not just naga
    // validation) applied to gouraud.wgsl, which is the first shader
    // in this crate with real per-vertex interpolated (not flat)
    // varyings (light_color/fog_color/tex_coords) alongside the one
    // flat draw_id -- proving a real pipeline with a mix of
    // interpolation modes on the same vertex output survives real
    // Metal pipeline creation.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn gouraud_wgsl_builds_a_render_pipeline_on_a_real_adapter() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let validation_scope = created
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let internal_scope = created.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = created
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gouraud.wgsl"),
                source: wgpu::ShaderSource::Wgsl(shader_source("gouraud").into()),
            });
        // Matches VertexInput: @location(0) coords: vec3<f32>,
        // @location(1) draw_id: u32, @location(2) tex_coords: vec2<f32>,
        // @location(3) light_color: vec4<f32>, @location(4) fog_color: vec4<f32>.
        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 24,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 40,
                shader_location: 4,
            },
        ];
        let _pipeline = created
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("gouraud.wgsl pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 56, // 12 + 4 + 8 + 16 + 16
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &vertex_attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let internal_err = pollster::block_on(internal_scope.pop());
        let validation_err = pollster::block_on(validation_scope.pop());
        if let Some(err) = validation_err.or(internal_err) {
            panic!(
                "gouraud.wgsl's pipeline failed to compile on real adapter {:?} backend {}: {err}",
                created.adapter_name,
                backend_name(created.backend)
            );
        }
        println!(
            "gouraud.wgsl's pipeline compiled successfully (real MSL translation + Metal \
             compiler invocation exercised, including mixed flat/interpolated varyings) \
             on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // Same reasoning as the other real-GPU pipeline tests applied to
    // complex.wgsl, the fourth and most complex shader in this crate so
    // far: the first test here to prove a pipeline with *three* texture/
    // sampler pairs (diffuse/lightmap/fogmap, all in one bind group)
    // alongside the shared frame uniform and the draw-parameters array.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn complex_wgsl_builds_a_render_pipeline_on_a_real_adapter() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let validation_scope = created
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let internal_scope = created.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = created
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("complex.wgsl"),
                source: wgpu::ShaderSource::Wgsl(shader_source("complex").into()),
            });
        // Matches VertexInput: @location(0) coords: vec3<f32>,
        // @location(1) draw_id: u32.
        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 12,
                shader_location: 1,
            },
        ];
        let _pipeline = created
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("complex.wgsl pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 16, // 3*f32 + u32
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &vertex_attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let internal_err = pollster::block_on(internal_scope.pop());
        let validation_err = pollster::block_on(validation_scope.pop());
        if let Some(err) = validation_err.or(internal_err) {
            panic!(
                "complex.wgsl's pipeline failed to compile on real adapter {:?} backend {}: {err}",
                created.adapter_name,
                backend_name(created.backend)
            );
        }
        println!(
            "complex.wgsl's pipeline compiled successfully (real MSL translation + Metal \
             compiler invocation exercised, including three texture/sampler pairs in one \
             bind group) on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // Same reasoning as the other real-GPU pipeline tests, applied to
    // postprocess.wgsl -- the first and only pipeline test in this
    // crate with zero vertex buffers: the real shader (and this port of
    // it) derives everything from @builtin(vertex_index) alone, a
    // well-established fullscreen-triangle-strip idiom, so `vertex.
    // buffers` is an empty slice here, not a placeholder.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn postprocess_wgsl_builds_a_render_pipeline_on_a_real_adapter() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let validation_scope = created
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let internal_scope = created.device.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = created
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("postprocess.wgsl"),
                source: wgpu::ShaderSource::Wgsl(shader_source("postprocess").into()),
            });
        let _pipeline = created
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("postprocess.wgsl pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..wgpu::PrimitiveState::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let internal_err = pollster::block_on(internal_scope.pop());
        let validation_err = pollster::block_on(validation_scope.pop());
        if let Some(err) = validation_err.or(internal_err) {
            panic!(
                "postprocess.wgsl's pipeline failed to compile on real adapter {:?} backend {}: {err}",
                created.adapter_name,
                backend_name(created.backend)
            );
        }
        println!(
            "postprocess.wgsl's pipeline compiled successfully (real MSL translation + Metal \
             compiler invocation exercised, zero vertex buffers) on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // Proves the UV-flip fix empirically, not just reasoned from the
    // WebGPU spec (the other postprocess.wgsl test only proves the
    // pipeline compiles, not that it samples the right texel). Uploads
    // a real 2x2 source texture, red on top row (y=0 in upload order,
    // which is WebGPU's top-left texel per the spec facts documented in
    // postprocess.wgsl's own header), blue on bottom row; renders
    // through the real postprocess pipeline into a 2x2 render target
    // with gamma=1/y_scale=1 (so GammaCorrect and the YScale multiply
    // are both no-ops, isolating the UV computation as the only thing
    // under test); reads the render target back and asserts its own
    // top row is red -- if the UV flip were wrong (the literal GLSL
    // formula), the top row would read back blue instead. Also doubles
    // as this crate's first real render-to-texture + readback path,
    // the same mechanism frame capture and editor hit-testing will
    // need later.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn postprocess_wgsl_samples_the_top_row_at_ndc_y_plus_one() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let device = &created.device;
        let queue = &created.queue;

        // 2x2 RGBA8 source texture: row 0 (first in upload order, i.e.
        // WebGPU's top texel row) is red, row 1 is blue.
        let source_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uv flip test source"),
            size: wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        #[rustfmt::skip]
        let source_pixels: [u8; 16] = [
            255, 0, 0, 255,   255, 0, 0, 255, // row 0: red, red
            0, 0, 255, 255,   0, 0, 255, 255, // row 1: blue, blue
        ];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &source_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let source_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // frame: FrameUniforms -- only gamma (byte offset 512) and
        // y_scale (byte offset 524) matter to postprocess.wgsl; both
        // set to 1.0 (a no-op gamma correct, no vertical flip from
        // YScale) so the UV computation is the only thing under test.
        // 8 mat4x4<f32> (64 bytes each) + 4 f32 = 528 bytes total,
        // matching common.wgsl's real FrameUniforms layout.
        let mut frame_bytes = [0u8; 528];
        frame_bytes[512..516].copy_from_slice(&1.0f32.to_le_bytes()); // gamma
        frame_bytes[524..528].copy_from_slice(&1.0f32.to_le_bytes()); // y_scale
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uv flip test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("postprocess.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_source("postprocess").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uv flip test pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uv flip test frame bind group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        let source_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uv flip test source bind group"),
            layout: &pipeline.get_bind_group_layout(1),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&source_sampler),
                },
            ],
        });

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uv flip test render target"),
            size: wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uv flip test render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &frame_bind_group, &[]);
            pass.set_bind_group(1, &source_bind_group, &[]);
            pass.draw(0..4, 0..1);
        }

        // Readback buffer: copy_texture_to_buffer requires bytes_per_row
        // to be a multiple of 256 (unlike write_texture), so this pads
        // each 8-byte real row out to 256 bytes.
        let padded_bytes_per_row = 256u32;
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uv flip test readback buffer"),
            size: (padded_bytes_per_row as u64) * 2,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(2),
                },
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");
        let top_row_red_pixel = &mapped[0..4];
        assert_eq!(
            top_row_red_pixel,
            &[255u8, 0, 0, 255],
            "expected the render target's top row to read back red (sampling the source \
             texture's top row at NDC y=+1) -- if this reads blue instead, the UV flip fix \
             regressed back to the literal GLSL formula; got {top_row_red_pixel:?}"
        );
        // Row 1 starts at the padded offset 256 (padded_bytes_per_row
        // above), not immediately after the 4-byte top-left pixel --
        // checking only the top row would let a degenerate result pass
        // (e.g. a UV that is constant 0 everywhere would sample red for
        // every pixel, top and bottom, and still pass the assertion
        // above); checking that the bottom row is genuinely blue is
        // what actually proves the orientation, not just "some pixel
        // somewhere is red".
        let bottom_row_blue_pixel = &mapped[256..260];
        assert_eq!(
            bottom_row_blue_pixel,
            &[0u8, 0, 255, 255],
            "expected the render target's bottom row to read back blue (sampling the source \
             texture's bottom row at NDC y=-1) -- got {bottom_row_blue_pixel:?}"
        );
        println!(
            "postprocess.wgsl's UV flip confirmed empirically: top row of the render target \
             sampled red, bottom row sampled blue, matching the source texture's real \
             orientation, not a degenerate constant UV, on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn capture_presented_frame_writes_the_rendered_frame_and_meta() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let dir = std::env::temp_dir().join(format!(
            "hp2_capture_presented_frame_writes_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("expected a writable temp directory");
        let mut loop_state = RenderLoop::new();
        loop_state.capture = Some(capture::FrameCapture::new(
            dir.clone(),
            "capture_test_map".into(),
            Some(7),
        ));
        let texture = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture_presented_frame writes test texture"),
            size: wgpu::Extent3d {
                width: 3,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut rgba = Vec::with_capacity(24);
        for i in 0u8..6 {
            rgba.extend_from_slice(&[i * 10, i * 10 + 1, i * 10 + 2, 255]);
        }
        created.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(12),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 3,
                height: 2,
                depth_or_array_layers: 1,
            },
        );

        capture_presented_frame(&created, &mut loop_state, &texture);

        assert!(
            loop_state
                .capture
                .as_ref()
                .is_some_and(|capture| capture.is_enabled()),
            "expected capture to stay enabled after a successful write"
        );
        let expected_rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        let mut reader = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(dir.join("frame_000000.png"))
                .expect("expected frame_000000.png to be written"),
        ))
        .read_info()
        .expect("expected a valid PNG header");
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
        assert_eq!((reader.info().width, reader.info().height), (3, 2));
        let mut buffer = vec![0; reader.output_buffer_size().expect("expected a frame size")];
        reader
            .next_frame(&mut buffer)
            .expect("expected the PNG pixel data to decode");
        assert_eq!(
            buffer, expected_rgb,
            "expected the PNG to hold the texture's own RGB pixels with alpha dropped"
        );

        let meta = std::fs::read_to_string(dir.join("frame_meta.json"))
            .expect("expected frame_meta.json to be written");
        assert_eq!(
            meta,
            format!(
                "{{\"captures\":[{{\"width_px\":3,\"height_px\":2,\"backing_scale_factor\":1.000000,\"renderer\":\"{}\",\"ticks\":7,\"map\":\"capture_test_map\"}}]}}\n",
                created.adapter_name
            )
        );
        std::fs::remove_dir_all(&dir).expect("expected the temp capture dir to be removable");
        println!(
            "capture_presented_frame wrote a real 3x2 frame and meta on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn capture_presented_frame_disables_capture_when_texture_lacks_copy_src() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let dir = std::env::temp_dir().join(format!(
            "hp2_capture_presented_frame_no_copy_src_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("expected a writable temp directory");
        let mut loop_state = RenderLoop::new();
        loop_state.capture = Some(capture::FrameCapture::new(
            dir.clone(),
            "capture_test_map".into(),
            None,
        ));
        let texture = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture_presented_frame no COPY_SRC test texture"),
            size: wgpu::Extent3d {
                width: 3,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        capture_presented_frame(&created, &mut loop_state, &texture);

        assert!(
            loop_state.capture.is_none(),
            "expected capture to be disabled when the presented texture lacks COPY_SRC"
        );
        assert!(
            !dir.join("frame_000000.png").exists(),
            "expected no frame to be written when capture is disabled"
        );
        assert!(
            !dir.join("frame_meta.json").exists(),
            "expected no meta to be written when capture is disabled"
        );
        std::fs::remove_dir_all(&dir).expect("expected the temp capture dir to be removable");
        println!(
            "capture_presented_frame disabled capture for a non-COPY_SRC texture on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn capture_only_frame_is_captured_but_never_presented() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let dir = std::env::temp_dir().join(format!(
            "hp2_capture_only_frame_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("expected a writable temp directory");
        let mut loop_state = RenderLoop::new();
        loop_state.capture = Some(capture::FrameCapture::new(
            dir.clone(),
            "Entry".into(),
            None,
        ));
        loop_state.set_backing_scale(2.0);

        // Bgra8Unorm: the real macOS swapchain format family, so the
        // capture's BGRA->RGBA swap is exercised too.
        begin_capture_only_frame(
            &created,
            &mut loop_state,
            4,
            2,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::Color::RED,
        );
        assert!(unlock_frame(&created, &mut loop_state, true));
        let first_target = loop_state
            .capture_target
            .clone()
            .expect("expected the capture target to be kept for reuse");
        // Blit=0: no capture point, same as a surface frame.
        begin_capture_only_frame(
            &created,
            &mut loop_state,
            4,
            2,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::Color::RED,
        );
        assert!(unlock_frame(&created, &mut loop_state, false));
        assert_eq!(
            loop_state.capture_target.as_ref(),
            Some(&first_target),
            "expected a same-shape capture-only frame to reuse its target"
        );

        assert_eq!(
            loop_state.counts.presented, 0,
            "a capture-only frame is never presented"
        );
        assert!(
            !dir.join("frame_000001.png").exists(),
            "Blit=0 must not capture"
        );
        let mut reader = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(dir.join("frame_000000.png"))
                .expect("expected frame_000000.png to be written"),
        ))
        .read_info()
        .expect("expected a valid PNG header");
        assert_eq!((reader.info().width, reader.info().height), (4, 2));
        let mut buffer = vec![0; reader.output_buffer_size().expect("expected a frame size")];
        reader
            .next_frame(&mut buffer)
            .expect("expected the PNG pixel data to decode");
        assert_eq!(
            buffer,
            [255, 0, 0].repeat(8),
            "expected the red clear color, RGB order"
        );
        let meta = std::fs::read_to_string(dir.join("frame_meta.json"))
            .expect("expected frame_meta.json to be written");
        assert_eq!(
            meta,
            format!(
                "{{\"captures\":[{{\"width_px\":4,\"height_px\":2,\"backing_scale_factor\":2.000000,\"renderer\":\"{}\",\"ticks\":null,\"map\":\"Entry\"}}]}}\n",
                created.adapter_name
            )
        );
        std::fs::remove_dir_all(&dir).expect("expected the temp capture dir to be removable");
    }
}
