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
//!    or a storage-buffer read carrying today's per-tile parameters),
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
//! production (`DrawTileESProgram`) to copy the vertex-submission
//! shape from (per-vertex `Coords`/`TexCoords` inputs, not a single
//! point-per-tile) even though the actual per-corner expansion
//! arithmetic still has to move from the deleted geometry shader into
//! either the vertex shader (`vertex_index`-based) or C++/Rust-side
//! CPU quad expansion. `DrawGouraud` has no such alternative to copy
//! -- there is only one `DrawGouraudProgram` class, no ES/Core split,
//! and its geometry shader runs unconditionally for every Gouraud
//! (polygon) draw; the vertex/fragment-only replacement described
//! above has to be designed fresh from reading what that geometry
//! shader does, not copied from an existing alternate implementation.
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
//! replacement design at all, unlike `DrawGouraud`. Its real
//! complexity is breadth, not architecture: lightmaps, fogmaps,
//! detail/macro textures (both with real HSV recoloring, not plain
//! multiply), environment maps, height maps, three alternate
//! parallax-mapping techniques (basic/occlusion/relief, the latter two
//! using real texture-sampling loops), and -- only visible once the
//! fragment body itself was actually read -- a real per-pixel
//! hardware point-light loop used twice (`OPT_HWLighting` base
//! lighting, and a second pass inside `OPT_BumpMaps` for tangent-space
//! diffuse+specular), distance fog mixing, and an `OPT_Editor`-gated
//! block of debug render modes (zone/poly-cut/walkable-surface
//! visualization, hit-test color override) that is editor-only, not
//! needed for core gameplay rendering. All `#if`-gated optional
//! branches of one vertex+fragment pair, each individually portable
//! with the same non-bindless-fallback strategy as every other shader
//! here; none of them have been transcribed yet. The file also has a
//! second genuinely-dead `#if 0` block (an unused parallax
//! soft-shadow function, lines 673-732) -- confirmed dead the same way
//! `DrawSimple_GLSL.cpp`'s stipple block is, and likewise not a
//! transcription target.
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
//! through and indexes a `var<storage, read> array<DrawParameters>` --
//! a single storage buffer rather than replicating GLSL's
//! UBO-size-limited/SSBO-unlimited split, since WebGPU storage buffers
//! are core and unconditionally available on every backend and web
//! tier. `DrawComplex`/`DrawGouraud`/`DrawTile`'s eventual WGSL ports
//! need the same shape, not the version this file originally shipped
//! with. Triangulation is already CPU-side for both `DrawComplex`
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
//! One real, not-yet-decided architectural gap surfaced by all three
//! orchestration files: GLSL's `#if`-gated shader specialization
//! (`ShaderCompilationOptions`, lazily compiled and cached per unique
//! combination of texture layers and blend mode via
//! `SelectSpecialization`) has no direct WGSL/wgpu equivalent -- wgpu
//! has no API-level preprocessor. The likely shape, not yet
//! implemented or further scoped: a Rust-side
//! `HashMap<ShaderCompilationOptions, wgpu::RenderPipeline>` cache. An
//! earlier version of this doc suggested WGSL pipeline-overridable
//! constants as the preferred mechanism for this -- wrong, caught on
//! review: an override constant can disable a branch's *logic*, but
//! WGSL still counts a `@group`/`@binding` as statically used if any
//! code references it at all, even code a constant switches off, so a
//! single WGSL source with every optional texture layer behind
//! override-disabled branches would still require every one of those
//! bindings to be populated (dummy textures for absent lightmap/
//! fog/detail/etc. layers) in every specialization, which the GLSL
//! `#if`-compiled-out approach never needs. Generating N actual WGSL
//! source variants (string substitution, or naga's own preprocessing
//! if it offers one) avoids that and is the better fit, not pipeline
//! constants. Confirmed in passing: `DrawGouraud.cpp`'s real
//! `MaxVertices()` also returns a hardcoded `256`, matching
//! `WgpuDrv.cpp`'s current stub value -- not a shortcut, already
//! correct. Still unexamined: the ring-buffer/`ShaderProgram::Flush`
//! machinery itself (`ShaderProgram.cpp`, read only for its
//! bindless/SSBO-fallback logic so far, not its buffering internals),
//! the shader class constructors/`CreateInputLayout` bodies named
//! above, and how any of this would connect to `WgpuDrv.cpp`'s current
//! no-op `DrawComplexSurface`/`DrawTriangles`/`DrawTile`/`Draw2DLine`/
//! `Draw2DPoint` stubs.
//!
//! Depth-range convention (GLSL/OpenGL clip-space Z is -1..1; WebGPU's
//! is 0..1): not yet resolved in code, since no Rust code builds a
//! real projection matrix yet. Decision recorded in `simple.wgsl`'s
//! own header comment rather than here, since it is a per-shader
//! usage contract (whatever populates `modelviewproj_mat` must build
//! it directly in WebGPU's 0..1 convention), not a crate-wide fact.

/// Backend selection, matching the plan's `[WgpuDrv.WgpuRenderDevice]
/// Backend=Auto|Metal|Vulkan|Dx12|Gl` ini key (not yet read from an ini
/// file by this crate -- the ini key's C++ side doesn't exist yet
/// either; this enum is the Rust-side vocabulary that key will
/// eventually map onto).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    Auto,
    Metal,
    Vulkan,
    Dx12,
    Gl,
}

impl BackendChoice {
    fn to_wgpu(self) -> wgpu::Backends {
        match self {
            BackendChoice::Auto => wgpu::Backends::PRIMARY | wgpu::Backends::GL,
            BackendChoice::Metal => wgpu::Backends::METAL,
            BackendChoice::Vulkan => wgpu::Backends::VULKAN,
            BackendChoice::Dx12 => wgpu::Backends::DX12,
            BackendChoice::Gl => wgpu::Backends::GL,
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
pub fn create_device_sync(
    choice: BackendChoice,
    create_surface: Option<
        impl FnOnce(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>,
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
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("hp2-wgpu device"),
        required_limits: adapter_limits.clone(),
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
    /// mapped to the screen).
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

/// One in-flight frame between `lock_frame` and `unlock_frame`:
/// `surface.get_current_texture()`'s own `SurfaceTexture`, the
/// `TextureView` built from it (`encoder.begin_render_pass` needs a
/// `&TextureView`, not the texture itself), and the `CommandEncoder`
/// the clear render pass was recorded into. `texture`/`view` are both
/// read in `unlock_frame` (`view` only transitively, via the encoder
/// already holding everything it needs -- kept here anyway so this
/// struct, not a borrow of it, is what crosses the `lock_frame`/
/// `unlock_frame` boundary; `SurfaceTexture`'s own `Drop` impl
/// discards it safely if `unlock_frame` is never actually called with
/// `present: true`, matching wgpu's own documented behavior).
struct InProgressFrame {
    texture: wgpu::SurfaceTexture,
    #[allow(dead_code)] // kept alive for texture's sake; never read directly
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
    /// `true` if `get_current_texture` returned `Suboptimal`, not
    /// `Success` -- `unlock_frame` clears `configured_size` for this
    /// case the same way `lock_frame` already does for `Outdated`,
    /// since `Suboptimal`'s own meaning is "usable now, but
    /// reconfigure soon" (`Surface::get_current_texture`'s own doc).
    /// Deferred to `unlock_frame`, not done here: the frame is still
    /// in flight at this point, and reconfiguring before it is
    /// submitted/presented would be premature.
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
    in_progress: Option<InProgressFrame>,
    counts: FrameCounts,
}

impl Default for RenderLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderLoop {
    pub fn new() -> Self {
        Self {
            configured_size: None,
            in_progress: None,
            counts: FrameCounts::default(),
        }
    }

    pub fn counts(&self) -> FrameCounts {
        self.counts
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
) -> FrameOutcome {
    let Some(surface) = created.surface.as_ref() else {
        loop_state.counts.other_failure += 1;
        return FrameOutcome::NoSurface;
    };
    if loop_state.configured_size != Some((width, height)) {
        let Some(config) = surface.get_default_config(&created.adapter, width, height) else {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::NoDefaultConfig;
        };
        surface.configure(&created.device, &config);
        loop_state.configured_size = Some((width, height));
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
        texture,
        view,
        encoder,
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

/// Replaces the per-frame-end half of `UWgpuRenderDevice::Unlock`.
/// `present`, matching the real `Unlock(UBOOL Blit)`'s own parameter
/// (`ThirdParty/XOpenGLDrv/Src/XOpenGL.cpp`'s own `Unlock` only blits
/// when `Blit` is set -- used for render passes that should not
/// actually reach the screen, e.g. hit-testing passes): `true` submits
/// and presents the frame `lock_frame` began; `false` submits (so the
/// GPU work still completes, matching XOpenGL's own real behavior of
/// still finishing the frame) but never calls `queue.present`, letting
/// `SurfaceTexture::drop` discard it instead (wgpu's own documented,
/// safe behavior for an un-presented texture).
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
    let Some(frame) = loop_state.in_progress.take() else {
        return false;
    };
    created
        .queue
        .submit(std::iter::once(frame.encoder.finish()));
    if present {
        created.queue.present(frame.texture);
        loop_state.counts.presented += 1;
    }
    if frame.suboptimal {
        loop_state.configured_size = None;
    }
    true
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
            "created device on adapter {:?} backend {}, max_texture_dimension_2d={}",
            created.adapter_name,
            backend_name(created.backend),
            created.adapter_limits.max_texture_dimension_2d
        );
    }

    #[test]
    fn backend_name_covers_every_plan_named_backend() {
        assert_eq!(backend_name(wgpu::Backend::Metal), "Metal");
        assert_eq!(backend_name(wgpu::Backend::Vulkan), "Vulkan");
        assert_eq!(backend_name(wgpu::Backend::Dx12), "Dx12");
        assert_eq!(backend_name(wgpu::Backend::Gl), "Gl");
        assert_eq!(backend_name(wgpu::Backend::BrowserWebGpu), "BrowserWebGpu");
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
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
            let mut validator = naga::valid::Validator::new(
                naga::valid::ValidationFlags::default(),
                naga::valid::Capabilities::empty(),
            );
            validator
                .validate(&module)
                .unwrap_or_else(|e| panic!("{} failed naga validation: {e}", path.display()));
            checked += 1;
        }
        assert!(
            checked > 0,
            "expected at least one .wgsl file under {shader_dir}, found none -- an empty \
             directory must not make this test vacuously pass"
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
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/simple.wgsl").into()),
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
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/simple.wgsl").into()),
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
}
