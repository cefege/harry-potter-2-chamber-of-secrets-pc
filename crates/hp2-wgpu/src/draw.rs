//! First real slice of "wire WgpuDrv.cpp's draw-call stubs to real
//! Rust draw-call batching" (Phase 3's still-open critical-path item
//! -- nothing renders a pixel in production without this). Scope
//! today: a direct, immediate-mode render of `Draw2DLine`-shaped
//! input through the already-ported `simple.wgsl`, into a caller-
//! supplied offscreen `wgpu::Texture`. Deliberately NOT yet the full
//! `DrawBatcher`/ring-buffer/append-only-command-recording
//! architecture those other open todo items describe -- this is the
//! first proof the real chain (vertex buffer, per-DrawID parameters
//! buffer, `FrameUniforms` bind group, pipeline creation, render
//! pass, draw, readback) works end to end on real hardware at all,
//! which that larger architecture needs as its own foundation. Not
//! wired to any FFI call or `WgpuDrv.cpp` override yet.
//!
//! Chosen deliberately independent of the real screen-lock confound
//! this session hit (`WgpuDrv.h`'s own addendum): offscreen render-to-
//! texture-plus-readback needs no live window surface or visible
//! display at all, the same technique this crate's own
//! `postprocess_wgsl_samples_the_top_row_at_ndc_y_plus_one` test
//! already proved works regardless of session-lock state.
//!
//! `simple.wgsl`'s real WGSL source, embedded into the binary via
//! `include_str!` (not the `#[cfg(test)]`-only `shader_source()` disk-
//! read helper elsewhere in this crate, which only resolves on a dev
//! machine with the source tree present) -- the mechanism that
//! helper's own doc comment named as "not written yet". `simple.wgsl`
//! needs no per-build specialization (no `#if`-gated branches at all
//! in its real base path, confirmed by its own header), so a plain
//! `concat!` is enough here; shaders that do need specialization will
//! need a real generation step, not this.
const SIMPLE_WGSL_SOURCE: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/simple.wgsl")
);

/// Byte-for-byte mirror of `simple.wgsl`'s real `VertexInput`
/// (`@location(0) coords: vec3<f32>, @location(1) draw_id: u32`).
/// `bytemuck::Pod`/`Zeroable`, not a hand-rolled `unsafe` byte
/// reinterpretation (this crate's own `FrameUniforms` doc has the
/// full reasoning for why, caught on review there first).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub coords: [f32; 3],
    pub draw_id: u32,
}

/// Byte-for-byte mirror of `simple.wgsl`'s real `DrawParameters`
/// (`draw_color: vec4<f32>`) -- `vec4<f32>` is already 16-byte
/// aligned, so no explicit padding is needed to satisfy WGSL's
/// uniform-address-space array-element alignment rule (confirmed
/// against the WGSL spec before relying on it, not assumed).
/// `bytemuck::Pod`/`Zeroable`, same reasoning as `LineVertex` above.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawParams {
    pub draw_color: [f32; 4],
}

/// Real `simple.wgsl`'s own provisional capacity (`MAX_DRAWS`, that
/// shader's own doc comment on why it is not yet the real per-build
/// value).
pub(crate) const MAX_DRAWS: usize = 256;

/// Groups `SimpleLinePipeline::render`'s "what to draw" arguments
/// (the actual rendering contract is documented on `render` itself,
/// not repeated here): `device`/`queue`/`&self` stay direct `render`
/// parameters, matching this crate's own established `RenderLoop::
/// set_scene_node(&mut self, device, queue, scene)` shape, while
/// these five are grouped into one struct to keep `render` itself
/// under clippy's `too_many_arguments` limit (an earlier version with
/// all eight as direct parameters tripped it) -- not an arbitrary
/// grouping.
pub struct LineDrawCall<'a> {
    pub frame_uniforms_buffer: &'a wgpu::Buffer,
    pub vertices: &'a [LineVertex],
    pub draw_params: &'a [DrawParams],
    pub target: &'a wgpu::TextureView,
    /// The real `wgpu::LoadOp<wgpu::Color>` -- `Clear(color)` to
    /// start a fresh target (this crate's own first offscreen-
    /// readback test still uses this), or `Load` to draw on top of
    /// whatever `target` already contains without erasing it, the
    /// real requirement for drawing a line into the live frame.
    /// Critical correctness note (caught on review before this was
    /// ever wired to a real `RenderLoop`, not after): `Load` alone is
    /// not enough to composite correctly onto `lock_frame`'s own
    /// clear. `lock_frame` only *records* its clear pass into
    /// `InProgressFrame`'s own `CommandEncoder` -- it does not submit
    /// it; only `unlock_frame` does, later. A `Load`-mode draw that
    /// did its own separate `queue.submit` between `lock_frame` and
    /// `unlock_frame` would therefore run on the GPU *before* the
    /// still-unsubmitted clear, which would then wipe it out when
    /// `unlock_frame` finally submits -- every frame would end up
    /// clear-only, a real, serious bug an earlier version of this
    /// doc comment wrongly reasoned around ("wgpu queue submits
    /// execute in submission order... without needing to share the
    /// same `CommandEncoder`" -- false for this specific case, since
    /// the clear is not actually submitted at the point a same-frame
    /// draw call would run). The real fix: a `Load`-mode draw into
    /// the live frame must record into the *same*, still-open
    /// `CommandEncoder` `lock_frame`'s clear pass used, not a fresh
    /// one of its own -- see `SimpleLinePipeline::record` (not
    /// `::render`, which still creates and submits its own encoder,
    /// correct only for a caller -- like this crate's own tests --
    /// that owns the whole encoder lifecycle itself).
    pub load: wgpu::LoadOp<wgpu::Color>,
}

/// One real, lazily-built `simple.wgsl` pipeline plus the two bind-
/// group layouts `layout: None` derives for it from the shader's own
/// `@group(0)`/`@group(1)` declarations -- kept so a real caller can
/// build matching bind groups without re-deriving the layout by hand
/// (wgpu's own `get_bind_group_layout` is the only way to retrieve a
/// derived layout after the fact).
pub struct SimpleLinePipeline {
    pipeline: wgpu::RenderPipeline,
    frame_uniforms_layout: wgpu::BindGroupLayout,
    draw_params_layout: wgpu::BindGroupLayout,
}

impl SimpleLinePipeline {
    /// Builds the real pipeline against `color_format` (the real
    /// target this pipeline will later render into -- `layout: None`
    /// plus a fixed `ColorTargetState::blend: None` opaque-replace
    /// blend state; `None` not an oversight, but this slice's own
    /// honest scope: the real per-draw blend-state axis
    /// (`SetBlend`'s seven real combinations, this crate's own module
    /// doc already has the full read) is not wired here yet, only a
    /// fixed-opaque first proof).
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("simple.wgsl (draw.rs production)"),
            source: wgpu::ShaderSource::Wgsl(SIMPLE_WGSL_SOURCE.into()),
        });
        // Matches VertexInput: @location(0) coords: vec3<f32>,
        // @location(1) draw_id: u32 -- same shape already proven
        // against a real pipeline in this crate's own
        // simple_wgsl_builds_a_render_pipeline_on_a_real_adapter test.
        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: std::mem::size_of::<[f32; 3]>() as u64,
                shader_location: 1,
            },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple.wgsl line pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let frame_uniforms_layout = pipeline.get_bind_group_layout(0);
        let draw_params_layout = pipeline.get_bind_group_layout(1);
        Self {
            pipeline,
            frame_uniforms_layout,
            draw_params_layout,
        }
    }

    /// Uploads `call`'s buffers and records (does **not** submit) a
    /// real render pass drawing `call.vertices` (a flat `LineList`:
    /// every consecutive pair is one line, matching `Draw2DLine`'s
    /// own real `GL_LINES` `DrawMode`) into `call.target`, using
    /// `call.frame_uniforms_buffer` (this crate's own `RenderLoop::
    /// set_scene_node` produces one of the right size/layout) and
    /// `call.draw_params` (one entry per real `DrawID`, indexed by
    /// each vertex's own `draw_id` field) -- into `encoder`, whichever
    /// one the caller passes. This is the real integration point for
    /// drawing into a live `RenderLoop` frame: `call.load` must be
    /// `wgpu::LoadOp::Load` and `encoder` must be the *same*,
    /// still-open `CommandEncoder` `lock_frame`'s own clear pass was
    /// recorded into (not a fresh one) -- `LineDrawCall`'s own `load`
    /// field doc has the full correctness reasoning for why a fresh
    /// encoder plus a separate `queue.submit` would silently lose the
    /// draw. `device`/`queue` are still needed here (buffer creation/
    /// upload), even though this does not submit anything itself.
    pub fn record(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        call: LineDrawCall<'_>,
    ) {
        let LineDrawCall {
            frame_uniforms_buffer,
            vertices,
            draw_params,
            target,
            load,
        } = call;
        assert!(
            draw_params.len() <= MAX_DRAWS,
            "draw_params.len() ({}) exceeds simple.wgsl's real MAX_DRAWS ({MAX_DRAWS})",
            draw_params.len()
        );
        assert!(
            vertices.len().is_multiple_of(2),
            "LineList topology needs an even number of vertices, got {}",
            vertices.len()
        );

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs line vertex buffer"),
            size: (std::mem::size_of_val(vertices)) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(vertices));

        // draw_params is a fixed-size MAX_DRAWS array in the real
        // shader (WGSL uniform arrays need a const-expression length,
        // this crate's own module doc already has the full reasoning
        // for why), so the uniform buffer backing it is always the
        // full MAX_DRAWS size regardless of how many entries the
        // caller actually supplied -- padded with zeroed entries
        // rather than a buffer sized to just `draw_params.len()`.
        let mut padded_params = vec![
            DrawParams {
                draw_color: [0.0; 4]
            };
            MAX_DRAWS
        ];
        padded_params[..draw_params.len()].copy_from_slice(draw_params);
        let draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs line draw-params buffer"),
            size: (MAX_DRAWS * std::mem::size_of::<DrawParams>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&draw_params_buffer, 0, bytemuck::cast_slice(&padded_params));

        let frame_uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs frame uniforms bind group"),
            layout: &self.frame_uniforms_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms_buffer.as_entire_binding(),
            }],
        });
        let draw_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs draw params bind group"),
            layout: &self.draw_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: draw_params_buffer.as_entire_binding(),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("draw.rs line render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &frame_uniforms_bind_group, &[]);
        pass.set_bind_group(1, &draw_params_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }

    /// Standalone wrapper around `record`: creates its own fresh
    /// `CommandEncoder`, records into it, submits, and blocks until
    /// GPU work completes before returning, so a caller can safely
    /// read `call.target` back immediately afterward. Correct only
    /// for a caller that owns the whole encoder lifecycle itself and
    /// is not drawing into a live `RenderLoop` frame -- this crate's
    /// own offscreen tests, or any other fully self-contained render.
    /// Drawing into a real in-progress `RenderLoop` frame must use
    /// `record` directly against that frame's own `CommandEncoder`
    /// instead (`LineDrawCall::load`'s own doc has the full
    /// correctness reasoning for why this standalone version cannot
    /// be used for that).
    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, call: LineDrawCall<'_>) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("draw.rs line render encoder"),
        });
        self.record(device, queue, &mut encoder, call);
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after a real line render");
    }
}

/// Real `GetPolyFlagsAndDrawFlags`'s own core `PolyFlags` ->
/// `DrawFlags` translation (`SetTexture.cpp:578-617`), the part that
/// determines actual shader behavior -- not its `CachedPolyFlags`
/// memoization (a GL-call-avoidance optimization, not a behavior
/// difference: this function is pure and cheap, nothing here needs
/// caching) nor its `PolyFlags` mutation for `PF_Occlude` (that feeds
/// `SetBlend`'s own real blend-state selection, a separate, still-
/// unbuilt axis -- see `TilePipeline`'s own doc). Also skips `DF_
/// DiffuseTexture`, which `SubmitTileBatch` ORs in itself right after
/// calling the real function this ports, not something `GetPolyFlags
/// AndDrawFlags` sets -- a future `queue_tile` wiring's own
/// responsibility, not this pure function's.
///
/// The one real behavior difference from a naive flag-for-flag
/// mapping: `PF_RenderFog` is first cleared from a local copy of
/// `poly_flags` whenever `(PolyFlags & (PF_RenderFog |
/// PF_Translucent)) != PF_RenderFog` -- true in every case except
/// "`PF_RenderFog` set, `PF_Translucent` not set" (verified by hand
/// truth table over all four combinations before relying on it, not
/// just read off the condition's own English-sounding shape, which
/// is easy to misread backwards -- an earlier version of this exact
/// doc comment did). So `DF_RenderFog` only ever comes out set when
/// `PF_RenderFog` is present *without* `PF_Translucent`; it is
/// cleared (not set) when neither is present, when only
/// `PF_Translucent` is present, and -- the least obvious case --
/// even when *both* are present together.
pub fn compute_tile_draw_flags(poly_flags: u32) -> u32 {
    const PF_MASKED: u32 = 0x0000_0002;
    const PF_TRANSLUCENT: u32 = 0x0000_0004;
    const PF_ENVIRONMENT: u32 = 0x0000_0010;
    const PF_MODULATED: u32 = 0x0000_0040;
    const PF_HIGHLIGHTED: u32 = 0x1000_0000;
    const PF_RENDER_FOG: u32 = 0x4000_0000;

    const DF_MASKED: u32 = 0x0100;
    const DF_MODULATED: u32 = 0x0400;
    const DF_TRANSLUCENT: u32 = 0x0800;
    const DF_ENVIRONMENT: u32 = 0x1000;
    const DF_RENDER_FOG: u32 = 0x2000;
    const DF_ALPHA_BLENDED: u32 = 0x4000;

    let mut poly_flags = poly_flags;
    if (poly_flags & (PF_RENDER_FOG | PF_TRANSLUCENT)) != PF_RENDER_FOG {
        poly_flags &= !PF_RENDER_FOG;
    }

    let mut draw_flags = 0u32;
    if poly_flags & PF_MODULATED != 0 {
        draw_flags |= DF_MODULATED;
    }
    if poly_flags & PF_RENDER_FOG != 0 {
        draw_flags |= DF_RENDER_FOG;
    }
    if poly_flags & PF_MASKED != 0 {
        draw_flags |= DF_MASKED;
    }
    if poly_flags & PF_HIGHLIGHTED != 0 {
        draw_flags |= DF_ALPHA_BLENDED;
    }
    if poly_flags & PF_TRANSLUCENT != 0 {
        draw_flags |= DF_TRANSLUCENT;
    }
    if poly_flags & PF_ENVIRONMENT != 0 {
        draw_flags |= DF_ENVIRONMENT;
    }
    draw_flags
}

/// `tile.wgsl`'s real WGSL source, embedded the same way `simple.wgsl`
/// is (`SIMPLE_WGSL_SOURCE`'s own doc has the full reasoning).
const TILE_WGSL_SOURCE: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/tile.wgsl")
);

/// Byte-for-byte mirror of `tile.wgsl`'s real `VertexInput`
/// (`@location(0) coords: vec3<f32>, @location(1) draw_id: u32,
/// @location(2) tex_coords: vec2<f32>`), matching the already-proven
/// vertex layout from this crate's own `tile_wgsl_builds_a_render_
/// pipeline_on_a_real_adapter` test (`array_stride: 24`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileVertex {
    pub coords: [f32; 3],
    pub draw_id: u32,
    pub tex_coords: [f32; 2],
}

/// Byte-for-byte mirror of `tile.wgsl`'s real `DrawParameters`
/// (`draw_color: vec4<f32>, draw_flags: u32`). Unlike `simple.wgsl`'s
/// `DrawParams` (a bare `vec4<f32>`, already 16-byte aligned with
/// nothing trailing it), this struct's raw size (16 + 4 = 20 bytes)
/// is *not* a multiple of 16 -- WGSL's uniform-address-space layout
/// rules round a struct's own size up to its own alignment (16, from
/// the `vec4<f32>` field) before using it as this array's real
/// per-element stride, giving a real stride of 32 bytes, not 20.
/// `_padding` makes this struct's own `size_of` match that exactly;
/// without it, writing `[TileDrawParams; N]` as a flat byte buffer
/// would silently misalign every entry after the first against the
/// real shader's own expected layout. `bytemuck::Pod`/`Zeroable`,
/// same reasoning as `LineVertex`/`DrawParams` above -- `_padding`
/// stays `pub` (not a private field defeating `Pod`'s own
/// requirements) and is always zeroed by `Zeroable`/by construction
/// in this module's own code, never read.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileDrawParams {
    pub draw_color: [f32; 4],
    pub draw_flags: u32,
    pub _padding: [u32; 3],
}

/// Groups `TilePipeline::record`/`::render`'s "what to draw"
/// arguments, same reasoning as `LineDrawCall` (clippy's
/// `too_many_arguments`, now with two more real fields this pipeline
/// needs that `SimpleLinePipeline` does not: the bound texture and
/// its sampler, `tile.wgsl`'s real `@group(2)`).
pub struct TileDrawCall<'a> {
    pub frame_uniforms_buffer: &'a wgpu::Buffer,
    pub vertices: &'a [TileVertex],
    pub draw_params: &'a [TileDrawParams],
    pub texture_view: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
    pub target: &'a wgpu::TextureView,
    /// Same real correctness contract as `LineDrawCall::load` --
    /// `Load` plus recording into the live frame's own still-
    /// unsubmitted `CommandEncoder` (via `record`, never `render`)
    /// for any draw composited into a real `RenderLoop` frame.
    pub load: wgpu::LoadOp<wgpu::Color>,
}

/// One real, lazily-built `tile.wgsl` pipeline plus the three bind-
/// group layouts `layout: None` derives for it (`@group(0)` frame
/// uniforms, `@group(1)` draw params, `@group(2)` diffuse texture/
/// sampler), same reasoning as `SimpleLinePipeline`.
///
/// Known, explicitly scoped limitation, not silently pretended away:
/// a single fixed `blend: None` (opaque-replace) state, not the real
/// `SetBlend`'s own seven `PolyFlags`-driven combinations (this
/// crate's own module doc already has the full read of those).
/// `None`, not a fixed alpha-blend state -- an earlier version of
/// this pipeline used `BlendState::ALPHA_BLENDING` unconditionally,
/// which is wrong for the common case, not just imprecise (caught on
/// review): real `SetBlend` only enables blending for `PF_Translucent`/
/// `PF_Modulated`/`PF_Highlighted` tiles -- an ordinary opaque tile
/// (the overwhelming majority of real `DrawTile` calls, e.g. most HUD
/// elements) gets no blending in the real source at all, and
/// `DrawTile.cpp`'s own real `Color.W` default is `1.0f` only when
/// `Info.Texture->Alpha <= 0.f` -- otherwise it is read straight from
/// the real texture's `Alpha` field, with no guarantee it is `1.0`
/// (`DrawSimple.cpp:64`'s own comment on `Draw2DLine`'s unrelated but
/// analogous `Color.W` field: "Unfortunately this is usually set to
/// 0"). Under a fixed alpha-blend pipeline, an ordinary opaque tile
/// whose real `Color.W` is not `1.0` would blend `texel.a * Color.W`
/// against the background instead of replacing it outright -- a real,
/// visible transparency bug for ordinary, non-translucent tiles, not
/// just a missing effect on the translucent ones. `blend: None` is
/// correct for every tile that does not carry `PF_Translucent`/
/// `PF_Modulated`/`PF_Highlighted`; those three remain visibly wrong
/// (rendered fully opaque) until the real blend-state pipeline cache
/// lands -- a real, known, explicitly scoped gap, not silently
/// pretended away, and the right trade for right now: wrong-but-
/// opaque on the minority (translucent effects) rather than wrong-
/// and-invisible on the majority (ordinary tiles).
pub struct TilePipeline {
    pipeline: wgpu::RenderPipeline,
    frame_uniforms_layout: wgpu::BindGroupLayout,
    draw_params_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
}

impl TilePipeline {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tile.wgsl (draw.rs production)"),
            source: wgpu::ShaderSource::Wgsl(TILE_WGSL_SOURCE.into()),
        });
        let vertex_attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: std::mem::size_of::<[f32; 3]>() as u64,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: std::mem::size_of::<[f32; 3]>() as u64 + std::mem::size_of::<u32>() as u64,
                shader_location: 2,
            },
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tile.wgsl pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TileVertex>() as u64,
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
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let frame_uniforms_layout = pipeline.get_bind_group_layout(0);
        let draw_params_layout = pipeline.get_bind_group_layout(1);
        let texture_layout = pipeline.get_bind_group_layout(2);
        Self {
            pipeline,
            frame_uniforms_layout,
            draw_params_layout,
            texture_layout,
        }
    }

    /// Same real contract as `SimpleLinePipeline::record` (its own
    /// doc has the full correctness reasoning for `encoder`/`call.
    /// load`): uploads `call`'s buffers and records (does **not**
    /// submit) a real render pass drawing `call.vertices` (a flat
    /// `TriangleList`: matching `DrawTileESProgram`'s own real
    /// `GL_TRIANGLES` `DrawMode` -- every consecutive 3 vertices is
    /// one triangle, two triangles per quad, matching `SubmitTileBatch`'s
    /// own real 6-vertices-per-tile layout) into `call.target`.
    pub fn record(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        call: TileDrawCall<'_>,
    ) {
        let TileDrawCall {
            frame_uniforms_buffer,
            vertices,
            draw_params,
            texture_view,
            sampler,
            target,
            load,
        } = call;
        assert!(
            draw_params.len() <= MAX_DRAWS,
            "draw_params.len() ({}) exceeds tile.wgsl's real MAX_DRAWS ({MAX_DRAWS})",
            draw_params.len()
        );
        assert!(
            vertices.len().is_multiple_of(3),
            "TriangleList topology needs a vertex count that is a multiple of 3, got {}",
            vertices.len()
        );

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs tile vertex buffer"),
            size: (std::mem::size_of_val(vertices)) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(vertices));

        let mut padded_params = vec![
            TileDrawParams {
                draw_color: [0.0; 4],
                draw_flags: 0,
                _padding: [0; 3],
            };
            MAX_DRAWS
        ];
        padded_params[..draw_params.len()].copy_from_slice(draw_params);
        let draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs tile draw-params buffer"),
            size: (MAX_DRAWS * std::mem::size_of::<TileDrawParams>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&draw_params_buffer, 0, bytemuck::cast_slice(&padded_params));

        let frame_uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs tile frame uniforms bind group"),
            layout: &self.frame_uniforms_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms_buffer.as_entire_binding(),
            }],
        });
        let draw_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs tile draw params bind group"),
            layout: &self.draw_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: draw_params_buffer.as_entire_binding(),
            }],
        });
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs tile texture bind group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("draw.rs tile render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &frame_uniforms_bind_group, &[]);
        pass.set_bind_group(1, &draw_params_bind_group, &[]);
        pass.set_bind_group(2, &texture_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }

    /// Standalone wrapper around `record`, same reasoning and same
    /// correctness scope as `SimpleLinePipeline::render`.
    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, call: TileDrawCall<'_>) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("draw.rs tile render encoder"),
        });
        self.record(device, queue, &mut encoder, call);
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after a real tile render");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendChoice, HeadlessSurfaceCreator, backend_name, create_device_sync};

    // Needs a real GPU adapter. CI runners have none. Run with
    // `cargo test -p hp2-wgpu -- --ignored`. Not confounded by this
    // session's real screen-lock finding (see this module's own
    // header doc): offscreen render-to-texture-plus-readback needs
    // no live window surface.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn simple_line_pipeline_renders_a_real_line_into_an_offscreen_texture() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let device = &created.device;
        let queue = &created.queue;

        // FrameUniforms with modelviewproj_mat = IDENTITY, so the
        // vertex buffer's own NDC-space coords pass straight through
        // to clip space unmodified -- isolates this test from the
        // real matrix math compute_frame_uniforms already has its own
        // dedicated, separately-verified tests for. 8 mat4x4<f32> (64
        // bytes each) + 4 f32 = 528 bytes, matching common.wgsl's
        // real FrameUniforms layout. modelviewproj_mat is the 5th
        // matrix (byte offset 4*64=256): identity is already an
        // all-zero buffer with 1.0 on the diagonal at offsets
        // 256+0, 256+20, 256+40, 256+60.
        let mut frame_bytes = [0u8; 528];
        for i in 0..4 {
            let offset = 256 + i * 20;
            frame_bytes[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line render test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let pipeline = SimpleLinePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm);

        // One line, NDC (-1, 0) to (0, 0): covers only the left half
        // of the render target's width. DrawColor is bright green,
        // distinct from the clear color (blue) -- a genuinely
        // partial-width line is the real negative control here: a
        // shader/pipeline bug that filled the whole target regardless
        // of actual vertex geometry (e.g. a hardcoded fragment-shader
        // output, or a vertex shader that collapsed every vertex to
        // the same clip position) would make the right half green
        // too, not blue.
        let vertices = [
            LineVertex {
                coords: [-1.0, 0.0, 0.0],
                draw_id: 0,
            },
            LineVertex {
                coords: [0.0, 0.0, 0.0],
                draw_id: 0,
            },
        ];
        let draw_params = [DrawParams {
            draw_color: [0.0, 1.0, 0.0, 1.0],
        }];

        // 64x1 Rgba8Unorm: real row bytes (64*4=256) are already a
        // multiple of 256, wgpu's own copy_texture_to_buffer
        // requirement (unlike write_texture), so no row padding is
        // needed in the readback below.
        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("line render test target"),
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
        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        pipeline.render(
            device,
            queue,
            LineDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                target: &render_target_view,
                load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
            },
        );

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line render test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("line render test readback encoder"),
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

        // Pixel 8 (well inside the drawn line's left-half span) must
        // be the real draw_color, green.
        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[0u8, 255, 0, 255],
            "expected a pixel inside the drawn line's span to read back green (the real \
             DrawParameters.draw_color), got {left_pixel:?}"
        );
        // Pixel 56 (well inside the right half, outside the line's
        // span) must still be the real clear color, blue -- proves
        // the line's actual vertex-buffer geometry was respected, not
        // a degenerate "whole target is green" result.
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected a pixel outside the drawn line's span to still read back the real clear \
             color, blue -- got {right_pixel:?} (a non-blue value here would mean the line's \
             own vertex geometry was not actually respected)"
        );
        println!(
            "SimpleLinePipeline::render confirmed empirically: a real Draw2DLine-shaped vertex \
             buffer, uploaded through real buffers and bind groups, produced the right pixels \
             at the right positions in a real offscreen texture on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // The real test the review that caught the clear-vs-draw-ordering
    // bug asked for: lock_frame records its clear pass into
    // InProgressFrame's own CommandEncoder without submitting it;
    // only unlock_frame submits, later. A literal lock_frame ->
    // record -> unlock_frame test needs a real OS window (wgpu
    // surfaces are always backed by one, there is no "offscreen
    // surface" concept) -- not available to a Rust #[test] on macOS
    // (this session's own established constraint: SDL_Init(VIDEO)
    // needs the real process main thread, which cargo test's harness
    // never runs on). This test instead manually reproduces the exact
    // mechanism lock_frame/unlock_frame rely on -- one CommandEncoder,
    // a clear pass recorded into it without submitting, then a second
    // pass recorded into that *same* still-open encoder (via
    // SimpleLinePipeline::record, Load mode, not ::render's own
    // separate-encoder-and-submit path), submitted exactly once at
    // the end -- against a real offscreen texture instead of a real
    // surface. This is the actual bug class being guarded against,
    // proven end to end even though it does not literally call
    // lock_frame/unlock_frame themselves.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn record_composites_correctly_into_a_shared_unsubmitted_encoder() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let device = &created.device;
        let queue = &created.queue;

        let mut frame_bytes = [0u8; 528];
        for i in 0..4 {
            let offset = 256 + i * 20;
            frame_bytes[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shared encoder test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let pipeline = SimpleLinePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm);
        let vertices = [
            LineVertex {
                coords: [-1.0, 0.0, 0.0],
                draw_id: 0,
            },
            LineVertex {
                coords: [0.0, 0.0, 0.0],
                draw_id: 0,
            },
        ];
        let draw_params = [DrawParams {
            draw_color: [0.0, 1.0, 0.0, 1.0],
        }];

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shared encoder test target"),
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
        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        // Step 1, matching lock_frame exactly: record a clear pass
        // into a fresh encoder, do NOT submit it yet.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shared encoder test encoder"),
        });
        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shared encoder test clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        // Step 2, matching a real Draw2DLine call between Lock and
        // Unlock: record the line into the SAME still-unsubmitted
        // encoder, Load mode (not Clear -- must not erase step 1).
        // Negative control actually run (caught on review): swapped
        // this `record` call for `pipeline.render(...)` (its own
        // encoder, submits immediately) with the clear recorded and
        // submitted *afterwards* -- the real historical bug shape,
        // not just a missing-draw check. Result: the real printed
        // failure was `left: [0, 0, 255, 255]` (blue) where `[0, 255,
        // 0, 255]` (green) was expected -- the late clear wiped the
        // already-submitted draw out entirely, exactly reproducing
        // what a same-frame Draw2DLine with its own separate submit
        // would have done to every real frame. Reverted to the real
        // `record`-based version below before committing either state.
        pipeline.record(
            device,
            queue,
            &mut encoder,
            LineDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                target: &render_target_view,
                load: wgpu::LoadOp::Load,
            },
        );
        // Step 3, matching unlock_frame exactly: submit the whole
        // accumulated encoder exactly once, now that both passes are
        // recorded.
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real shared-encoder submit");

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shared encoder test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut readback_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shared encoder test readback encoder"),
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
        queue.submit(std::iter::once(readback_encoder.finish()));
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

        // Left half (inside the drawn line's span): must be green --
        // proves the Load-mode draw, recorded into the clear pass's
        // own still-unsubmitted encoder, actually reached the GPU at
        // all once the single combined submit ran.
        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[0u8, 255, 0, 255],
            "expected the line drawn via record() into the shared encoder to actually render, \
             got {left_pixel:?}"
        );
        // Right half (outside the line's span): must still be blue,
        // the real clear color -- proves the clear pass, recorded
        // first into the same encoder but submitted together with the
        // draw, was not wiped out or reordered incorrectly relative
        // to the draw. This is the exact real bug an earlier version
        // of this crate's own design (a separate queue.submit inside
        // what is now record()) would have risked: if the draw had
        // run in its own earlier submit, the clear's own later submit
        // (matching unlock_frame's real timing) would have wiped it,
        // and this assertion would catch that by seeing blue only
        // where the draw itself did not touch, never by this pixel
        // going the wrong color -- the structural fix (record() now
        // only takes a shared `&mut CommandEncoder`, with no way to
        // create or submit its own) already prevents that class of
        // regression by construction, and this test proves the
        // resulting single-submit composite is actually correct.
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected the clear pass recorded into the shared encoder to survive the combined \
             submit outside the drawn line's span, got {right_pixel:?}"
        );
        println!(
            "record() confirmed empirically to composite correctly with lock_frame's own real \
             clear-recorded-not-submitted pattern: a clear pass and a Load-mode line draw, both \
             recorded into one shared CommandEncoder and submitted together exactly once, \
             produced the right pixels in both the clear-only and drawn regions on adapter \
             {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    fn compute_tile_draw_flags_matches_the_real_source_translation() {
        const PF_MASKED: u32 = 0x0000_0002;
        const PF_TRANSLUCENT: u32 = 0x0000_0004;
        const PF_MODULATED: u32 = 0x0000_0040;
        const PF_HIGHLIGHTED: u32 = 0x1000_0000;
        const PF_RENDER_FOG: u32 = 0x4000_0000;
        const DF_MASKED: u32 = 0x0100;
        const DF_MODULATED: u32 = 0x0400;
        const DF_TRANSLUCENT: u32 = 0x0800;
        const DF_RENDER_FOG: u32 = 0x2000;
        const DF_ALPHA_BLENDED: u32 = 0x4000;

        assert_eq!(compute_tile_draw_flags(0), 0, "no flags in, no flags out");
        assert_eq!(compute_tile_draw_flags(PF_MASKED), DF_MASKED);
        assert_eq!(compute_tile_draw_flags(PF_MODULATED), DF_MODULATED);
        assert_eq!(compute_tile_draw_flags(PF_TRANSLUCENT), DF_TRANSLUCENT);
        // PF_Highlighted maps to DF_AlphaBlended, not a same-named bit
        // -- a real, confirmed (not assumed-symmetric) mapping.
        assert_eq!(compute_tile_draw_flags(PF_HIGHLIGHTED), DF_ALPHA_BLENDED);
        // PF_RenderFog alone (without PF_Translucent) must produce
        // DF_RenderFog -- the condition that clears it,
        // `(PolyFlags & (PF_RenderFog|PF_Translucent)) != PF_RenderFog`,
        // is false in exactly this one case (verified by hand truth
        // table on compute_tile_draw_flags's own doc, not re-derived
        // here -- an earlier version of this test got this backwards
        // by misreading the condition's own English-sounding shape).
        assert_eq!(
            compute_tile_draw_flags(PF_RENDER_FOG),
            DF_RENDER_FOG,
            "PF_RenderFog alone must survive translation into DF_RenderFog, matching the real \
             source"
        );
        // PF_RenderFog together with PF_Translucent clears PF_RenderFog
        // again (the least obvious case) -- so DF_RenderFog must NOT
        // be set here, only DF_Translucent.
        assert_eq!(
            compute_tile_draw_flags(PF_RENDER_FOG | PF_TRANSLUCENT),
            DF_TRANSLUCENT,
            "PF_RenderFog + PF_Translucent together must clear PF_RenderFog before \
             translation, leaving only DF_Translucent set, matching the real source"
        );
    }

    // Needs a real GPU adapter. CI runners have none. Run with
    // `cargo test -p hp2-wgpu -- --ignored`. Not confounded by this
    // session's real screen-lock finding (see this module's own
    // header doc): offscreen render-to-texture-plus-readback needs
    // no live window surface.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn tile_pipeline_renders_a_real_textured_quad_into_an_offscreen_texture() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let device = &created.device;
        let queue = &created.queue;

        // Identity modelviewproj_mat, same real technique and byte
        // layout as simple_line_pipeline_renders_a_real_line_into_an_
        // offscreen_texture above.
        let mut frame_bytes = [0u8; 528];
        for i in 0..4 {
            let offset = 256 + i * 20;
            frame_bytes[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tile render test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let pipeline = TilePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm);

        // A real 1x1 solid-red texture via TextureCache::upload_rgba8
        // (the real crate this pipeline is meant to be fed by, not a
        // bespoke texture built only for this test) -- a solid color
        // makes the result independent of UV/V-flip orientation,
        // which is not what this test exists to check (the real
        // vertex geometry/NDC placement is).
        let mut texture_cache = crate::texture::TextureCache::new();
        let cached = texture_cache.upload_rgba8(
            &created,
            1,
            crate::texture::TextureDimensions {
                width: 1,
                height: 1,
                u_scale: 1.0,
                v_scale: 1.0,
            },
            &[255, 0, 0, 255],
            false,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // One quad (2 triangles, 6 vertices, matching SubmitTileBatch's
        // own real ES-path layout), covering only NDC x in [-1.0, 0.0]
        // (the left half of the 64px-wide target) -- a genuinely
        // partial-width quad is the real negative control here, same
        // reasoning as the line test above: a bug that filled the
        // whole target regardless of actual vertex geometry would
        // make the right half red too, not the clear color.
        let vertices = [
            TileVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 1.0],
            },
            TileVertex {
                coords: [0.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 1.0],
            },
            TileVertex {
                coords: [0.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 0.0],
            },
            TileVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 1.0],
            },
            TileVertex {
                coords: [0.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 0.0],
            },
            TileVertex {
                coords: [-1.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 0.0],
            },
        ];
        // White draw_color (multiplies with the sampled red texel
        // unchanged) and draw_flags=0 (no masking/alpha-blend discard
        // -- compute_tile_draw_flags's own tests cover that
        // translation separately, in isolation).
        let draw_params = [TileDrawParams {
            draw_color: [1.0, 1.0, 1.0, 1.0],
            draw_flags: 0,
            _padding: [0; 3],
        }];

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tile render test target"),
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
        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        pipeline.render(
            device,
            queue,
            TileDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                texture_view: &cached.view,
                sampler: &sampler,
                target: &render_target_view,
                load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
            },
        );

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tile render test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tile render test readback encoder"),
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

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[255u8, 0, 0, 255],
            "expected a pixel inside the drawn quad's span to read back red (the real sampled \
             texel times the real white draw_color), got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected a pixel outside the drawn quad's span to still read back the real clear \
             color, blue -- got {right_pixel:?} (a non-blue value here would mean the quad's own \
             vertex geometry was not actually respected)"
        );
        println!(
            "TilePipeline::render confirmed empirically: a real DrawTile-shaped textured quad, \
             uploaded through a real TextureCache entry and real buffers/bind groups, produced \
             the right pixels at the right positions in a real offscreen texture on adapter \
             {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    // The real test the blend-state review finding asked for: proves
    // blend: None (not the earlier, wrong ALPHA_BLENDING default)
    // actually matters for the common case. draw_color.W = 0.0 here
    // -- a real, reachable value (DrawTile.cpp's own real Color.W
    // comes from Info.Texture->Alpha when that is > 0.f, with no
    // guarantee it is ever 1.0, this struct's own doc has the full
    // real-source citation) -- must still render fully opaque red,
    // not blend away to the clear color the way a fixed alpha-blend
    // pipeline would (texel.a * 0.0 blended over blue stays blue,
    // never red).
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn tile_pipeline_stays_opaque_with_a_zero_alpha_draw_color() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let device = &created.device;
        let queue = &created.queue;

        let mut frame_bytes = [0u8; 528];
        for i in 0..4 {
            let offset = 256 + i * 20;
            frame_bytes[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zero-alpha tile test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let pipeline = TilePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm);
        let mut texture_cache = crate::texture::TextureCache::new();
        let cached = texture_cache.upload_rgba8(
            &created,
            1,
            crate::texture::TextureDimensions {
                width: 1,
                height: 1,
                u_scale: 1.0,
                v_scale: 1.0,
            },
            &[255, 0, 0, 255],
            false,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // Full-width quad this time (simpler: only the blend result
        // matters here, not vertex geometry, which the other tile
        // test already covers).
        let vertices = [
            TileVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 1.0],
            },
            TileVertex {
                coords: [1.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 1.0],
            },
            TileVertex {
                coords: [1.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 0.0],
            },
            TileVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 1.0],
            },
            TileVertex {
                coords: [1.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 0.0],
            },
            TileVertex {
                coords: [-1.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 0.0],
            },
        ];
        // The one real variable under test: draw_color.W = 0.0.
        let draw_params = [TileDrawParams {
            draw_color: [1.0, 1.0, 1.0, 0.0],
            draw_flags: 0,
            _padding: [0; 3],
        }];

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("zero-alpha tile test target"),
            size: wgpu::Extent3d {
                width: 4,
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
        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        pipeline.render(
            device,
            queue,
            TileDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                texture_view: &cached.view,
                sampler: &sampler,
                target: &render_target_view,
                load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
            },
        );

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zero-alpha tile test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("zero-alpha tile test readback encoder"),
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
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 4,
                height: 1,
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

        // RGB only, not the full RGBA: write_mask::ALL means the
        // fragment shader's own output alpha (texel.a * draw_color.W
        // = 1.0 * 0.0 = 0.0) is still written straight to the
        // texture's alpha channel -- blend: None disables blending
        // math, not which channels get written. That stored 0 alpha
        // is an expected, real consequence of this test's own W=0.0
        // input, not itself evidence of (or against) the real bug
        // this test exists to catch, which is specifically about
        // whether the *color* channels replaced the clear color
        // outright or blended with it.
        let pixel_rgb = &mapped[0..3];
        assert_eq!(
            pixel_rgb,
            &[255u8, 0, 0],
            "expected the tile to render fully opaque red (RGB) despite draw_color.W = 0.0 \
             (blend: None must replace, not blend, the clear color), got {pixel_rgb:?} -- a \
             blue-leaning result here would mean the pipeline is still alpha-blending"
        );
        println!(
            "TilePipeline::render confirmed empirically to stay fully opaque with a zero-alpha \
             draw_color (blend: None, not the earlier wrong ALPHA_BLENDING default) on adapter \
             {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }
}
