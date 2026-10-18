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
    /// Depth target. The caller clears it to 1.0 before the first draw
    /// of a frame; the pass only loads and stores it.
    pub depth_target: &'a wgpu::TextureView,
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

/// Depth format every pipeline in this module attaches.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Creates a `DEPTH_FORMAT` target of `width` x `height` for the color
/// target. The caller clears it; pipelines only load and store it.
pub fn create_depth_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("draw.rs depth target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

// PolyFlags and LINE_* bits from Engine/Inc/UnObj.h and UnRender.h.
const PF_INVISIBLE: u32 = 0x0000_0001;
const PF_TRANSLUCENT: u32 = 0x0000_0004;
const PF_MODULATED: u32 = 0x0000_0040;
const PF_LUMOS_AFFECTED: u32 = 0x0080_0040;
const PF_HIGHLIGHTED: u32 = 0x1000_0000;
const PF_OCCLUDE: u32 = 0x8000_0000;
const LINE_DEPTH_CUED: u32 = 0x0000_0002;

/// Per-draw blend, color-mask and depth state (XOpenGL `SetBlend` and
/// `SetDepth`). Each distinct value needs its own pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderState {
    /// `None` is opaque replace.
    pub blend: Option<wgpu::BlendState>,
    pub color_write: bool,
    pub depth_write: bool,
    pub depth_compare: wgpu::CompareFunction,
}

impl RenderState {
    /// Opaque replace, depth tested and written.
    pub const OPAQUE: Self = Self {
        blend: None,
        color_write: true,
        depth_write: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
    };

    /// State for a surface or tile from its `PolyFlags`.
    /// `remove_occlude_if_solid` is set by tile draws only.
    pub fn for_poly_flags(poly_flags: u32, remove_occlude_if_solid: bool) -> Self {
        let mut flags = poly_flags;
        if flags & (PF_TRANSLUCENT | PF_MODULATED | PF_HIGHLIGHTED) == 0 {
            flags |= PF_OCCLUDE;
        } else if remove_occlude_if_solid {
            flags &= !PF_OCCLUDE;
        }
        Self {
            blend: blend_for_poly_flags(flags),
            color_write: flags & PF_INVISIBLE == 0,
            depth_write: flags & PF_OCCLUDE != 0,
            depth_compare: wgpu::CompareFunction::LessEqual,
        }
    }

    /// State for a line from its `LINE_*` flags. Lines always blend as
    /// `PF_Highlighted`; only `LINE_DepthCued` lines test and write depth.
    pub fn for_line_flags(line_flags: u32) -> Self {
        let depth_cued = line_flags & LINE_DEPTH_CUED != 0;
        Self {
            blend: blend_for_poly_flags(PF_HIGHLIGHTED),
            color_write: true,
            depth_write: depth_cued,
            depth_compare: if depth_cued {
                wgpu::CompareFunction::LessEqual
            } else {
                wgpu::CompareFunction::Always
            },
        }
    }

    /// Color target for a pipeline that renders into `format`.
    pub fn color_target(self, format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
        wgpu::ColorTargetState {
            format,
            blend: self.blend,
            write_mask: if self.color_write {
                wgpu::ColorWrites::ALL
            } else {
                wgpu::ColorWrites::empty()
            },
        }
    }

    /// Depth state over `DEPTH_FORMAT`, with default stencil and bias.
    pub fn depth_stencil(self) -> wgpu::DepthStencilState {
        wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(self.depth_write),
            depth_compare: Some(self.depth_compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }
    }
}

/// Depth load op paired with a pass's color `load`: a pass that clears
/// its color target starts a fresh frame, so it clears depth to 1.0 too
/// (XOpenGL clears both together, XOpenGL.cpp:2544-2546); any other
/// pass keeps the depth already drawn.
fn depth_load_for(load: wgpu::LoadOp<wgpu::Color>) -> wgpu::LoadOp<f32> {
    match load {
        wgpu::LoadOp::Clear(_) => wgpu::LoadOp::Clear(1.0),
        _ => wgpu::LoadOp::Load,
    }
}

/// Blend state for `poly_flags`, first match wins (XOpenGL `SetBlend`).
/// `None` is opaque replace.
fn blend_for_poly_flags(poly_flags: u32) -> Option<wgpu::BlendState> {
    use wgpu::BlendFactor as F;
    let (src, dst) = if poly_flags & PF_LUMOS_AFFECTED == PF_LUMOS_AFFECTED {
        (F::OneMinusSrcAlpha, F::SrcAlpha)
    } else if poly_flags & PF_INVISIBLE != 0 {
        (F::Zero, F::One)
    } else if poly_flags & PF_HIGHLIGHTED != 0 {
        let src = if poly_flags & PF_TRANSLUCENT != 0 {
            F::SrcAlpha
        } else {
            F::One
        };
        (src, F::OneMinusSrcAlpha)
    } else if poly_flags & PF_TRANSLUCENT != 0 {
        (F::One, F::OneMinusSrc)
    } else if poly_flags & PF_MODULATED != 0 {
        (F::Dst, F::Src)
    } else {
        return None;
    };
    Some(wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: src,
            dst_factor: dst,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: alpha_factor(src),
            dst_factor: alpha_factor(dst),
            operation: wgpu::BlendOperation::Add,
        },
    })
}

/// Alpha-channel factor matching a color factor. GL applies the same
/// blend function to alpha; wgpu needs the alpha factor spelled out.
fn alpha_factor(factor: wgpu::BlendFactor) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as F;
    match factor {
        F::Src => F::SrcAlpha,
        F::OneMinusSrc => F::OneMinusSrcAlpha,
        F::Dst => F::DstAlpha,
        F::OneMinusDst => F::OneMinusDstAlpha,
        other => other,
    }
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
    /// Builds the pipeline for `color_format`, the real target this
    /// pipeline renders into. `state` supplies the blend, color-mask and
    /// depth state; each distinct `RenderState` needs its own pipeline.
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        state: RenderState,
    ) -> Self {
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
            depth_stencil: Some(state.depth_stencil()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(state.color_target(color_format))],
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
            depth_target,
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
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load_for(load),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
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

/// Real `DrawSimpleTriangleProgram`'s own Rust counterpart --
/// confirmed (`XOpenGL.h:1900-1913`) to be a second, distinct real
/// instantiation of the exact same `ShaderProgramImpl<DrawSimpleVertex,
/// DrawSimpleParameters>` template `DrawSimpleLineProgram`
/// (`SimpleLinePipeline`'s own real counterpart) is, bound as `Simple_
/// Triangle_Prog` instead of `Simple_Line_Prog` -- the real vertex/
/// fragment GLSL source is identical between the two, only the real
/// `DrawMode`/topology differs (`GL_TRIANGLES` vs `GL_LINES`,
/// confirmed by reading both `ActivateShader`/`CreateInputLayout`
/// overrides, `DrawSimple.cpp:290-348`). This port keeps that same
/// real one-template-two-instances shape as two separate Rust
/// structs (not one parameterized by topology) rather than collapsing
/// them -- `simple.wgsl`/`LineVertex`/`DrawParams`/`LineDrawCall` are
/// all directly reused unchanged, only `new`'s own `primitive.topology`
/// and `record`'s own vertex-count assert differ from
/// `SimpleLinePipeline`'s. `Draw2DPoint` is this struct's own real
/// production caller (`DrawSimple.cpp:154-205`, `EndFlash`'s own
/// flash-overlay quad, `DrawSimple.cpp:207-256`, is not ported --
/// no real caller/FFI wiring exists for it yet, tracked separately).
pub struct SimpleTrianglePipeline {
    pipeline: wgpu::RenderPipeline,
    frame_uniforms_layout: wgpu::BindGroupLayout,
    draw_params_layout: wgpu::BindGroupLayout,
}

impl SimpleTrianglePipeline {
    /// Same contract as `SimpleLinePipeline::new`: `state` supplies the
    /// blend, color-mask and depth state. `primitive.topology` is
    /// `TriangleList`, matching `DrawSimpleTriangleProgram`'s
    /// `GL_TRIANGLES` `DrawMode`.
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        state: RenderState,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("simple.wgsl (draw.rs triangle production)"),
            source: wgpu::ShaderSource::Wgsl(SIMPLE_WGSL_SOURCE.into()),
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
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("simple.wgsl triangle pipeline"),
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
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: Some(state.depth_stencil()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(state.color_target(color_format))],
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

    /// Same real contract as `SimpleLinePipeline::record` (that
    /// method's own doc has the full correctness reasoning for
    /// `encoder`/`call.load`) -- `vertices.len().is_multiple_of(3)`
    /// (`TriangleList`, not `LineList`'s own `is_multiple_of(2)`) is
    /// the one real difference.
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
            depth_target,
            load,
        } = call;
        assert!(
            draw_params.len() <= MAX_DRAWS,
            "draw_params.len() ({}) exceeds simple.wgsl's real MAX_DRAWS ({MAX_DRAWS})",
            draw_params.len()
        );
        assert!(
            vertices.len().is_multiple_of(3),
            "TriangleList topology needs a vertex count that is a multiple of 3, got {}",
            vertices.len()
        );

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs triangle vertex buffer"),
            size: (std::mem::size_of_val(vertices)) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(vertices));

        let mut padded_params = vec![
            DrawParams {
                draw_color: [0.0; 4]
            };
            MAX_DRAWS
        ];
        padded_params[..draw_params.len()].copy_from_slice(draw_params);
        let draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs triangle draw-params buffer"),
            size: (MAX_DRAWS * std::mem::size_of::<DrawParams>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&draw_params_buffer, 0, bytemuck::cast_slice(&padded_params));

        let frame_uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs triangle frame uniforms bind group"),
            layout: &self.frame_uniforms_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms_buffer.as_entire_binding(),
            }],
        });
        let draw_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs triangle draw params bind group"),
            layout: &self.draw_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: draw_params_buffer.as_entire_binding(),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("draw.rs triangle render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load_for(load),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &frame_uniforms_bind_group, &[]);
        pass.set_bind_group(1, &draw_params_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }

    /// Same real contract as `SimpleLinePipeline::render`.
    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, call: LineDrawCall<'_>) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("draw.rs triangle render encoder"),
        });
        self.record(device, queue, &mut encoder, call);
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after a real triangle render");
    }
}

/// Groups `point_quad_corners`' real inputs, same clippy reasoning as
/// `draw::TileQuadRequest`. `rfx2`/`rfy2`/`fx2`/`fy2`/`z` are the same
/// real per-segment/per-call values `tile_quad_corners`/`draw_2d_line_
/// screen_to_coords` already take; `x1`/`y1`/`x2`/`y2` are `Draw2DPoint`'s
/// own real `FLOAT` parameters, unchanged (`z` is passed *after*
/// `resolve_point_z`'s own real adjustment has already been applied --
/// that function's own doc in `lib.rs` has the real citation for why,
/// not duplicated here).
pub struct PointQuadRequest {
    pub rfx2: f32,
    pub rfy2: f32,
    pub fx2: f32,
    pub fy2: f32,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub z: f32,
}

/// The four real corners `Draw2DPoint`'s own real six-vertex quad
/// (`DrawSimple.cpp:187-198`) is built from, deduplicated the same
/// way `draw::TileQuadCorners` already is (`Out[0]`/`Out[3]` share
/// `top_left`, `Out[2]`/`Out[4]` share `bottom_right`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointQuadCorners {
    pub top_left: [f32; 3],
    pub top_right: [f32; 3],
    pub bottom_right: [f32; 3],
    pub bottom_left: [f32; 3],
}

/// Real `Draw2DPoint`'s own per-corner formula (`DrawSimple.cpp:
/// 187-198`): `Coords = (RFX2 * Z * (X1 - Frame->FX2 - 0.5f), RFY2 *
/// Z * (Y1 - Frame->FY2 - 0.5f), Z)` for the top-left corner, `X2 +
/// 0.5f`/`Y2 + 0.5f` for the right/bottom edges -- the real `±0.5`
/// pixel inset/expansion past `X1`/`X2`/`Y1`/`Y2` themselves (not
/// present in `Draw2DLine`'s own formula at all), confirmed by direct
/// re-read, not assumed from `tile_quad_corners`'s own superficially
/// similar shape: a degenerate `X1==X2, Y1==Y2` point still produces
/// a real, visible 1x1-pixel-ish quad because of this offset, not a
/// zero-area one.
pub fn point_quad_corners(request: PointQuadRequest) -> PointQuadCorners {
    let PointQuadRequest {
        rfx2,
        rfy2,
        fx2,
        fy2,
        x1,
        y1,
        x2,
        y2,
        z,
    } = request;
    let coord = |px: f32, py: f32| [rfx2 * z * (px - fx2), rfy2 * z * (py - fy2), z];
    PointQuadCorners {
        top_left: coord(x1 - 0.5, y1 - 0.5),
        top_right: coord(x2 + 0.5, y1 - 0.5),
        bottom_right: coord(x2 + 0.5, y2 + 0.5),
        bottom_left: coord(x1 - 0.5, y2 + 0.5),
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
/// AndDrawFlags` sets -- a `queue_tile`/`queue_triangles` wiring-time
/// responsibility, not this pure function's. The real function's own
/// `RemoveOccludeIfSolid` third parameter (`SetTexture.cpp:578`) only
/// ever affects the *returned* `PolyFlags`' own `PF_Occlude` bit (fed
/// to `SetBlend`'s own real blend-state selection, the same separate,
/// still-unbuilt axis above) -- it has no effect on `DrawFlags`
/// itself (confirmed: `PF_Occlude` is not part of `RelevantPolyFlags`,
/// the mask `DrawFlags`' own if-chain is built from), so this one
/// function is the real, byte-identical translation for *both* real
/// call sites regardless of their own different third-argument value
/// -- `SubmitTileBatch`'s `GetPolyFlagsAndDrawFlags(PolyFlags,
/// DrawFlags, 1)` and `PrepareGouraudCall`'s `GetPolyFlagsAndDrawFlags
/// (PolyFlags, DrawFlags, 0)` (`DrawGouraud.cpp:74`) alike -- not a
/// coincidence, confirmed by direct re-read of the real function
/// before reusing it for a second caller, not assumed from the name.
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

/// The real `Info.Texture && Info.Texture->Alpha > 0.f ? Info.Texture
/// ->Alpha : 1.0f` resolution, factored out so `compute_tile_draw_
/// color`'s own `PF_Modulated`-then-unconditional-override sequence
/// and `queue_triangles`' own real `PrepareGouraudCall:166` (`const
/// FLOAT TextureAlpha = (Info.Texture && Info.Texture->Alpha > 0.f) ?
/// Info.Texture->Alpha : 1.f;` -- the identical real formula, a
/// second real call site, not a coincidence) both call the one real
/// formula instead of each re-deriving it.
pub fn resolve_texture_alpha(texture_alpha: Option<f32>) -> f32 {
    match texture_alpha {
        Some(alpha) if alpha > 0.0 => alpha,
        _ => 1.0,
    }
}

/// `DrawTile`'s own real pre-`SubmitTileBatch` color adjustment
/// (`DrawTile.cpp:33-43`, read in full, not just the `PF_Modulated`
/// branch alone): `PF_Modulated` forces the whole color -- including
/// alpha -- to opaque white first; then, unconditionally (a separate
/// `if`/`else` immediately after, not an `else if` chained onto the
/// `PF_Modulated` check above it), `Info.Texture->Alpha` overwrites
/// alpha again when the real texture object exists and that alpha is
/// strictly `> 0.f`, falling back to `1.0` otherwise (`resolve_
/// texture_alpha`'s own doc has the real shared-formula citation).
/// The two checks are independent, not mutually exclusive: a
/// `PF_Modulated` tile whose texture has a real positive `Alpha` ends
/// up with RGB forced to `1.0` but alpha taken from the texture, not
/// `1.0` -- confirmed by reading both branches in sequence, this
/// function's own tests exercise exactly that combination, not just
/// each flag alone. `texture_alpha` is `None` for the real `!Info.
/// Texture` case (a real `FTextureInfo` can genuinely have a null
/// `Texture` pointer -- the real source's own `&&` short-circuit),
/// `Some(Info.Texture->Alpha)` otherwise.
pub fn compute_tile_draw_color(
    color: [f32; 4],
    poly_flags: u32,
    texture_alpha: Option<f32>,
) -> [f32; 4] {
    const PF_MODULATED: u32 = 0x0000_0040;
    let mut color = color;
    if poly_flags & PF_MODULATED != 0 {
        color = [1.0, 1.0, 1.0, 1.0];
    }
    color[3] = resolve_texture_alpha(texture_alpha);
    color
}

/// Real `DrawComplexSurface`/`SetTextureHelper`'s own `DiffuseInfo`
/// computation (`DrawComplex.cpp:39,168-172`, both read together: the
/// helper's own `TextureInfo` write only happens when the caller
/// passes a non-null pointer, which `DrawComplexSurface` itself only
/// does when `Surface.Texture->Texture` is non-null, confirmed by
/// direct re-read of the real ternary at the call site, not assumed
/// from the helper's own body in isolation). Real shape, confirmed
/// genuinely different from `compute_tile_draw_color`'s/`PrepareGouraudCall`'s
/// own `TextureAlpha` formula in two ways, not assumed parallel:
/// `.x`(diffuse)/`.z`(alpha) each use a real `> 0.f` *value* check
/// (`resolve_texture_alpha`'s own formula, reused twice here) even
/// when the real texture object is non-null, and the real null-
/// texture fallback is `(1.0, 0.0, 0.0, cAlpha/255.0)` -- diffuse
/// defaults to `1.0` but alpha(`.z`) defaults to `0.0`, not a uniform
/// `1.0` fallback the way `compute_tile_draw_color`'s own `Option`
/// shape might suggest by surface analogy alone. `.y` (Specular) is
/// always `Info.Texture->Specular` directly, no `>0.f` check at all
/// (confirmed: the real source has no ternary for it) -- `0.0` only
/// in the real null-texture fallback. `.w` is always `cAlpha as f32 /
/// 255.0` regardless of which branch -- the real source's own
/// `DrawComplex.cpp:172`'s unconditional post-helper overwrite
/// (replacing whatever `TEXTURE_SCALE_NAME` the helper itself might
/// have written for the macro-texture-only case, not read anywhere in
/// this crate's own ported `complex.wgsl` fragment shader scope
/// either way). `texture` is `Some((Diffuse, Specular, Alpha))` when
/// `Surface.Texture->Texture` is real and non-null, `None` otherwise
/// -- one combined real null check drives all three real fields'
/// fallback together, not three independent `Option`s the way
/// `TileDrawRequest`'s own unrelated fields are.
pub fn compute_complex_diffuse_info(texture: Option<(f32, f32, f32)>, c_alpha: u8) -> [f32; 4] {
    let alpha_field = c_alpha as f32 / 255.0;
    match texture {
        Some((diffuse, specular, alpha)) => [
            resolve_texture_alpha(Some(diffuse)),
            specular,
            resolve_texture_alpha(Some(alpha)),
            alpha_field,
        ],
        None => [1.0, 0.0, 0.0, alpha_field],
    }
}

/// Groups `tile_quad_corners`' real inputs (clippy's
/// `too_many_arguments` -- fifteen real `f32` values, same reasoning
/// as `LineDrawCall`/`TileDrawCall` above). `rfx2`/`rfy2`/`fx2`/`fy2`
/// are the same real per-segment/per-call values `draw_2d_line_
/// screen_to_coords` takes in `hp2-wgpu`'s own `lib.rs` (that
/// function's own doc has the full real citation for both); `x`/`y`/
/// `xl`/`yl`/`u`/`v`/`ul`/`vl`/`z` are `DrawTile`'s own real `FLOAT`
/// parameters, unchanged; `u_mult`/`v_mult` are `texture_uv_mult`'s
/// own real per-bind output (`texture.rs`'s own doc has the full
/// reasoning for why this function takes them as already-computed,
/// rather than recomputing them itself from `UScale`/`VScale`/
/// `USize`/`VSize` -- those four belong to the real `FTextureInfo`/
/// `CachedTexture`, not to this pure screen-space quad geometry
/// function).
pub struct TileQuadRequest {
    pub rfx2: f32,
    pub rfy2: f32,
    pub fx2: f32,
    pub fy2: f32,
    pub x: f32,
    pub y: f32,
    pub xl: f32,
    pub yl: f32,
    pub u: f32,
    pub v: f32,
    pub ul: f32,
    pub vl: f32,
    pub z: f32,
    pub u_mult: f32,
    pub v_mult: f32,
}

/// The four distinct real corners `SubmitTileBatch`'s own ES-path
/// six-vertex quad (`DrawTile.cpp:213-245`) is built from -- `Out[0]`/
/// `Out[3]` share `top_left`, `Out[2]`/`Out[4]` share `bottom_right`,
/// so only four, not six, real `(Coords, TexCoords)` pairs are
/// distinct. `unlock_frame`'s own flush (`hp2-wgpu`'s `lib.rs`)
/// expands these into the real six-vertex, two-triangle
/// `TriangleList` layout with a real per-chunk `DrawID` stamped in at
/// that point -- not here, matching `QueuedLine`'s own established
/// pattern of carrying no `DrawID` until flush time (this struct's
/// own doc intentionally does not duplicate that reasoning, see
/// `QueuedLine`'s doc in `lib.rs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileQuadCorners {
    pub top_left: ([f32; 3], [f32; 2]),
    pub top_right: ([f32; 3], [f32; 2]),
    pub bottom_right: ([f32; 3], [f32; 2]),
    pub bottom_left: ([f32; 3], [f32; 2]),
}

/// Real `SubmitTileBatch`'s own ES-path per-corner formula
/// (`DrawTile.cpp:219-241`): `Coords = (RFX2 * Z * (X - Frame->FX2),
/// RFY2 * Z * (Y - Frame->FY2), Z)` for the left edge (`X`) / top
/// edge (`Y`), `X + XL` / `Y + YL` for the right/bottom edges;
/// `TexCoords = (U * UMult, V * VMult)` for the top/left UV, `(U +
/// UL) * UMult` / `(V + VL) * VMult` for the right/bottom UV --
/// exactly `draw_2d_line_screen_to_coords`'s own `RFX2 * Z * (coord -
/// FX2)` transform (`DrawSimple.cpp:80,82`), applied to a quad's four
/// corners instead of a line's two endpoints; the real source applies
/// the identical screen-to-clip formula to both call shapes.
pub fn tile_quad_corners(request: TileQuadRequest) -> TileQuadCorners {
    let TileQuadRequest {
        rfx2,
        rfy2,
        fx2,
        fy2,
        x,
        y,
        xl,
        yl,
        u,
        v,
        ul,
        vl,
        z,
        u_mult,
        v_mult,
    } = request;
    let coord = |px: f32, py: f32| [rfx2 * z * (px - fx2), rfy2 * z * (py - fy2), z];
    let tex = |pu: f32, pv: f32| [pu * u_mult, pv * v_mult];
    TileQuadCorners {
        top_left: (coord(x, y), tex(u, v)),
        top_right: (coord(x + xl, y), tex(u + ul, v)),
        bottom_right: (coord(x + xl, y + yl), tex(u + ul, v + vl)),
        bottom_left: (coord(x, y + yl), tex(u, v + vl)),
    }
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
    /// Depth target; see `LineDrawCall::depth_target`.
    pub depth_target: &'a wgpu::TextureView,
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
/// Built for one `RenderState`; the caller keys its cache on it.
pub struct TilePipeline {
    pipeline: wgpu::RenderPipeline,
    frame_uniforms_layout: wgpu::BindGroupLayout,
    draw_params_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
}

impl TilePipeline {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        state: RenderState,
    ) -> Self {
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
            depth_stencil: Some(state.depth_stencil()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(state.color_target(color_format))],
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
            depth_target,
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
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load_for(load),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
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

/// `gouraud.wgsl`'s real WGSL source, embedded the same way `simple.
/// wgsl`/`tile.wgsl` are (`SIMPLE_WGSL_SOURCE`'s own doc has the full
/// reasoning).
const GOURAUD_WGSL_SOURCE: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/gouraud.wgsl")
);

/// Byte-for-byte mirror of `gouraud.wgsl`'s own real `VertexInput`
/// (`@location(0) coords: vec3<f32>, @location(1) draw_id: u32,
/// @location(2) tex_coords: vec2<f32>, @location(3) light_color:
/// vec4<f32>, @location(4) fog_color: vec4<f32>`), matching the
/// already-proven vertex layout from this crate's own `gouraud_wgsl_
/// builds_a_render_pipeline_on_a_real_adapter` test (`array_stride:
/// 56`, offsets `0/12/16/24/40`) -- no `#[repr(C)]` padding needed:
/// each field's own natural `f32`/`u32` alignment (4 bytes) already
/// divides every one of those offsets evenly. `coords` are *not*
/// screen-to-clip-transformed the way `LineVertex`/`TileVertex`'s own
/// coords are -- real `BufferVert` (`DrawGouraud.cpp:35-43`) copies
/// `FTransTexture::Point` straight through (`glm::vec3(P->Point.X,
/// P->Point.Y, P->Point.Z)`), and `gouraud.wgsl`'s own `vs_main`
/// feeds that directly into `modelviewproj_mat` with no other
/// transform -- confirmed by direct re-read of both, not assumed
/// from `TileVertex`'s superficially similar shape. No `normals`
/// field: `gouraud.wgsl`'s own `VertexInput` already deliberately
/// omits it (bump-mapping only, not read anywhere in this crate's
/// own ported fragment shader scope, that shader's own module doc
/// has the full citation).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GouraudVertex {
    pub coords: [f32; 3],
    pub draw_id: u32,
    pub tex_coords: [f32; 2],
    pub light_color: [f32; 4],
    pub fog_color: [f32; 4],
}

/// `GouraudVertex` minus `draw_id` -- the real per-vertex shape a
/// caller (a future `RenderLoop::queue_triangles`/`hp2_rs_wgpu_
/// queue_triangles` FFI array parameter) provides, matching real
/// `BufferVert`'s own four real `FTransTexture` fields exactly
/// (`Point`/`TexCoords`/`Light`/`Fog` -- `Normal` excluded, same
/// reasoning `GouraudVertex`'s own doc already has). `draw_id` is
/// not known until a real `unlock_frame` flush actually assigns a
/// chunk-local one (same established pattern `QueuedLine`/
/// `QueuedTile`'s own corner structs already use -- geometry carries
/// no `draw_id` of its own until flush time), so this is a distinct,
/// smaller type rather than `GouraudVertex` itself with a throwaway
/// placeholder value a caller would have to remember never matters.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GouraudVertexInput {
    pub coords: [f32; 3],
    pub tex_coords: [f32; 2],
    pub light_color: [f32; 4],
    pub fog_color: [f32; 4],
}

/// Byte-for-byte mirror of `gouraud.wgsl`'s own real `DrawParameters`
/// (`diffuse_info: vec4<f32>, draw_color: vec4<f32>, draw_flags:
/// u32`) -- same `_padding`-to-32-bytes reasoning as `TileDrawParams`
/// (two `vec4<f32>` fields, already 16-byte aligned with nothing
/// trailing them, plus one `u32` that needs rounding the whole
/// struct's own stride up to the next 16-byte boundary). `draw_color`
/// is written here (`[0.0, 0.0, 0.0, texture_alpha]`, matching the
/// real non-hit-testing branch of `PrepareGouraudCall:168`) even
/// though `gouraud.wgsl`'s own `fs_main` never actually reads it in
/// this crate's current ported scope (`TextureAlpha` already reaches
/// the shader via `diffuse_info.w` too, confirmed real and not a
/// double-write bug: outside hit-testing, the real `DrawColor.w` is
/// genuinely redundant with `DiffuseInfo.w`) -- keeping the real GPU
/// buffer layout honest/forward-compatible with a future real hit-
/// testing wiring that *would* need `DrawColor`, not silently
/// dropping a real struct field this port happens not to read yet.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GouraudDrawParams {
    pub diffuse_info: [f32; 4],
    pub draw_color: [f32; 4],
    pub draw_flags: u32,
    pub _padding: [u32; 3],
}

/// Groups `GouraudPipeline::record`/`::render`'s "what to draw"
/// arguments, same reasoning as `TileDrawCall`.
pub struct GouraudDrawCall<'a> {
    pub frame_uniforms_buffer: &'a wgpu::Buffer,
    pub vertices: &'a [GouraudVertex],
    pub draw_params: &'a [GouraudDrawParams],
    pub texture_view: &'a wgpu::TextureView,
    pub sampler: &'a wgpu::Sampler,
    pub target: &'a wgpu::TextureView,
    /// Depth target; see `LineDrawCall::depth_target`.
    pub depth_target: &'a wgpu::TextureView,
    pub load: wgpu::LoadOp<wgpu::Color>,
}

/// One real, lazily-built `gouraud.wgsl` pipeline plus its three
/// bind-group layouts (`@group(0)` frame uniforms, `@group(1)` draw
/// params, `@group(2)` diffuse texture/sampler). Built for one
/// `RenderState`; the caller keys its cache on it.
pub struct GouraudPipeline {
    pipeline: wgpu::RenderPipeline,
    frame_uniforms_layout: wgpu::BindGroupLayout,
    draw_params_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
}

impl GouraudPipeline {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        state: RenderState,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gouraud.wgsl (draw.rs production)"),
            source: wgpu::ShaderSource::Wgsl(GOURAUD_WGSL_SOURCE.into()),
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gouraud.wgsl pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GouraudVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(state.depth_stencil()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(state.color_target(color_format))],
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

    /// Same real contract as `TilePipeline::record` (that method's
    /// own doc has the full correctness reasoning for `encoder`/
    /// `call.load`) -- `TriangleList` topology, matching real
    /// `DrawTriangles`' own flat (indexed-or-fan-expanded) triangle
    /// list (`DrawGouraud.cpp:337-350`, both the indexed and fan
    /// branches already expand to a flat triangle-list shape before
    /// this point, this crate's own `queue_triangles` orchestration
    /// does the same).
    pub fn record(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        call: GouraudDrawCall<'_>,
    ) {
        let GouraudDrawCall {
            frame_uniforms_buffer,
            vertices,
            draw_params,
            texture_view,
            sampler,
            target,
            depth_target,
            load,
        } = call;
        assert!(
            draw_params.len() <= MAX_DRAWS,
            "draw_params.len() ({}) exceeds gouraud.wgsl's real MAX_DRAWS ({MAX_DRAWS})",
            draw_params.len()
        );
        assert!(
            vertices.len().is_multiple_of(3),
            "TriangleList topology needs a vertex count that is a multiple of 3, got {}",
            vertices.len()
        );

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs gouraud vertex buffer"),
            size: (std::mem::size_of_val(vertices)) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(vertices));

        let mut padded_params = vec![
            GouraudDrawParams {
                diffuse_info: [0.0; 4],
                draw_color: [0.0; 4],
                draw_flags: 0,
                _padding: [0; 3],
            };
            MAX_DRAWS
        ];
        padded_params[..draw_params.len()].copy_from_slice(draw_params);
        let draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs gouraud draw-params buffer"),
            size: (MAX_DRAWS * std::mem::size_of::<GouraudDrawParams>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&draw_params_buffer, 0, bytemuck::cast_slice(&padded_params));

        let frame_uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs gouraud frame uniforms bind group"),
            layout: &self.frame_uniforms_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms_buffer.as_entire_binding(),
            }],
        });
        let draw_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs gouraud draw params bind group"),
            layout: &self.draw_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: draw_params_buffer.as_entire_binding(),
            }],
        });
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs gouraud texture bind group"),
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
            label: Some("draw.rs gouraud render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load_for(load),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &frame_uniforms_bind_group, &[]);
        pass.set_bind_group(1, &draw_params_bind_group, &[]);
        pass.set_bind_group(2, &texture_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }

    /// Same real contract as `TilePipeline::render`.
    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, call: GouraudDrawCall<'_>) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("draw.rs gouraud render encoder"),
        });
        self.record(device, queue, &mut encoder, call);
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after a real gouraud render");
    }
}

/// `complex.wgsl`'s real WGSL source, embedded the same way `simple.
/// wgsl`/`tile.wgsl`/`gouraud.wgsl` are.
const COMPLEX_WGSL_SOURCE: &str = concat!(
    include_str!("shaders/common.wgsl"),
    include_str!("shaders/complex.wgsl")
);

/// Byte-for-byte mirror of `complex.wgsl`'s own real `VertexInput`
/// (`@location(0) coords: vec3<f32>, @location(1) draw_id: u32`),
/// matching the already-proven layout from this crate's own `complex_
/// wgsl_builds_a_render_pipeline_on_a_real_adapter` test (`array_
/// stride: 16`). No `tex_coords`/`light_color`/`fog_color` the way
/// `GouraudVertex` has: real BSP surfaces have no per-vertex UVs at
/// all -- every texture layer's UV is computed in the vertex shader
/// from the surface's own shared planar `XAxis`/`YAxis` mapping axes
/// (`complex.wgsl`'s own module doc has the full real citation), so
/// per-vertex data here really is just `Coords`/`DrawID`, matching
/// the real source's own comment on why `Normal` is kept in the real
/// struct but never read (32-byte alignment padding, not a real input
/// this base path uses -- already excluded here, same reasoning
/// `GouraudVertex`'s own doc already has for its own unused `Normal`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ComplexVertex {
    pub coords: [f32; 3],
    pub draw_id: u32,
}

/// `ComplexVertex` minus `draw_id` -- the real per-vertex shape a
/// caller (a future `RenderLoop::queue_complex_surface`/`hp2_rs_wgpu_
/// queue_complex_surface` FFI array parameter) provides, same real
/// reasoning as `GouraudVertexInput`'s own doc (no `draw_id` until a
/// real `unlock_frame` flush assigns a chunk-local one). Real BSP
/// surfaces have no other real per-vertex fields at all in this base
/// path (`ComplexVertex`'s own doc has the full real citation for
/// why) -- just the triangulated `Coords` a future `WgpuDrv.cpp`
/// override builds from the real `FSavedPoly`/`FTransform` linked
/// list (`DrawComplex.cpp:206-248`'s own real fan-triangulation,
/// mirroring `queue_triangles`' own already-established non-indexed
/// fan-expansion shape).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ComplexVertexInput {
    pub coords: [f32; 3],
}

/// Byte-for-byte mirror of `complex.wgsl`'s own real `DrawParameters`
/// (`diffuse_uv`/`light_map_uv`/`fog_map_uv`: vec4<f32>, `diffuse_
/// info`: vec4<f32>, `x_axis`/`y_axis`: vec4<f32>, `draw_flags`: u32)
/// -- six real `vec4<f32>` fields (already 16-byte aligned) plus one
/// trailing `u32`, same `_padding`-to-the-next-16-byte-boundary
/// reasoning as `TileDrawParams`/`GouraudDrawParams`. Each `*_uv`
/// field is `(UMult, VMult, UPan, VPan)` for that real texture layer
/// (`texture_uv_mult`/`texture_uv_pan`'s own real per-bind formulas,
/// confirmed against `SetTextureHelper`'s own real `vec4(UMult, VMult,
/// UPan, VPan)` construction, `DrawComplex.cpp:37`) -- `.xy`=mult,
/// `.zw`=pan, matching `complex.wgsl`'s own vertex shader reads
/// exactly (`(map_dot - params.diffuse_uv.zw) * params.diffuse_uv.xy`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ComplexDrawParams {
    pub diffuse_uv: [f32; 4],
    pub light_map_uv: [f32; 4],
    pub fog_map_uv: [f32; 4],
    pub diffuse_info: [f32; 4],
    pub x_axis: [f32; 4],
    pub y_axis: [f32; 4],
    pub draw_flags: u32,
    pub _padding: [u32; 3],
}

/// Groups `ComplexPipeline::record`/`::render`'s "what to draw"
/// arguments, same reasoning as `TileDrawCall`/`GouraudDrawCall` --
/// now with *three* real texture/sampler pairs (diffuse/light map/fog
/// map, `complex.wgsl`'s own `@group(2)` bindings 0-5) instead of one,
/// matching the real source's own multi-layer `DrawComplexSurface`.
/// Real surfaces without a lightmap/fogmap still need *some* valid
/// bound texture here (WGSL cannot conditionally declare a binding
/// the way the real GLSL's own `#if OPT_HasLightMap` can) --
/// `complex.wgsl`'s own module doc already has the full real
/// reasoning for why the sampled value is simply discarded via
/// `select()` in that case, not read from this struct's own fields at
/// all; this crate's own future `queue_complex_surface` wiring is
/// responsible for choosing *which* real or dummy texture view to
/// pass here, not this struct.
pub struct ComplexDrawCall<'a> {
    pub frame_uniforms_buffer: &'a wgpu::Buffer,
    pub vertices: &'a [ComplexVertex],
    pub draw_params: &'a [ComplexDrawParams],
    pub diffuse_texture_view: &'a wgpu::TextureView,
    pub diffuse_sampler: &'a wgpu::Sampler,
    pub light_map_texture_view: &'a wgpu::TextureView,
    pub light_map_sampler: &'a wgpu::Sampler,
    pub fog_map_texture_view: &'a wgpu::TextureView,
    pub fog_map_sampler: &'a wgpu::Sampler,
    pub target: &'a wgpu::TextureView,
    /// Depth target. The caller clears it to 1.0 before the first draw
    /// of a frame; this pass only loads and stores it.
    pub depth_target: &'a wgpu::TextureView,
    pub load: wgpu::LoadOp<wgpu::Color>,
}

/// One real, lazily-built `complex.wgsl` pipeline plus its three
/// bind-group layouts (`@group(0)` frame uniforms, `@group(1)` draw
/// params, `@group(2)` the six real texture/sampler bindings). Built
/// for one `RenderState`; the caller keys its cache on it.
pub struct ComplexPipeline {
    pipeline: wgpu::RenderPipeline,
    frame_uniforms_layout: wgpu::BindGroupLayout,
    draw_params_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
}

impl ComplexPipeline {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        state: RenderState,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("complex.wgsl (draw.rs production)"),
            source: wgpu::ShaderSource::Wgsl(COMPLEX_WGSL_SOURCE.into()),
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("complex.wgsl pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ComplexVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(state.depth_stencil()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(state.color_target(color_format))],
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

    /// Same real contract as `TilePipeline::record`/`GouraudPipeline::
    /// record` (`TilePipeline::record`'s own doc has the full
    /// correctness reasoning for `encoder`/`call.load`) --
    /// `TriangleList` topology, matching real `DrawComplexSurface`'s
    /// own flat fan-triangulated vertex list (`DrawComplex.cpp:
    /// 234-244`).
    pub fn record(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        call: ComplexDrawCall<'_>,
    ) {
        let ComplexDrawCall {
            frame_uniforms_buffer,
            vertices,
            draw_params,
            diffuse_texture_view,
            diffuse_sampler,
            light_map_texture_view,
            light_map_sampler,
            fog_map_texture_view,
            fog_map_sampler,
            target,
            depth_target,
            load,
        } = call;
        assert!(
            draw_params.len() <= MAX_DRAWS,
            "draw_params.len() ({}) exceeds complex.wgsl's real MAX_DRAWS ({MAX_DRAWS})",
            draw_params.len()
        );
        assert!(
            vertices.len().is_multiple_of(3),
            "TriangleList topology needs a vertex count that is a multiple of 3, got {}",
            vertices.len()
        );

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs complex vertex buffer"),
            size: (std::mem::size_of_val(vertices)) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(vertices));

        let mut padded_params = vec![
            ComplexDrawParams {
                diffuse_uv: [0.0; 4],
                light_map_uv: [0.0; 4],
                fog_map_uv: [0.0; 4],
                diffuse_info: [0.0; 4],
                x_axis: [0.0; 4],
                y_axis: [0.0; 4],
                draw_flags: 0,
                _padding: [0; 3],
            };
            MAX_DRAWS
        ];
        padded_params[..draw_params.len()].copy_from_slice(draw_params);
        let draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw.rs complex draw-params buffer"),
            size: (MAX_DRAWS * std::mem::size_of::<ComplexDrawParams>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&draw_params_buffer, 0, bytemuck::cast_slice(&padded_params));

        let frame_uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs complex frame uniforms bind group"),
            layout: &self.frame_uniforms_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms_buffer.as_entire_binding(),
            }],
        });
        let draw_params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs complex draw params bind group"),
            layout: &self.draw_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: draw_params_buffer.as_entire_binding(),
            }],
        });
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw.rs complex texture bind group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(diffuse_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(diffuse_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(light_map_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(light_map_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(fog_map_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(fog_map_sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("draw.rs complex render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_target,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load_for(load),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &frame_uniforms_bind_group, &[]);
        pass.set_bind_group(1, &draw_params_bind_group, &[]);
        pass.set_bind_group(2, &texture_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }

    /// Same real contract as `TilePipeline::render`.
    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, call: ComplexDrawCall<'_>) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("draw.rs complex render encoder"),
        });
        self.record(device, queue, &mut encoder, call);
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after a real complex render");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendChoice, HeadlessSurfaceCreator, backend_name, create_device_sync};

    /// Depth view sized like `color_target`, cleared to 1.0 in its own
    /// submitted encoder. The pipeline pass under test only loads it.
    fn cleared_depth_view(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target: &wgpu::Texture,
    ) -> wgpu::TextureView {
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("draw.rs test depth target"),
            size: color_target.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("draw.rs test depth clear"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the depth clear");
        view
    }

    #[test]
    fn render_state_for_poly_flags_follows_set_blend_precedence() {
        // Plain surface: no blend bits, so PF_Occlude is forced on.
        let plain = RenderState::for_poly_flags(0, false);
        assert_eq!(plain.blend, None);
        assert!(plain.color_write);
        assert!(plain.depth_write);
        assert_eq!(plain.depth_compare, wgpu::CompareFunction::LessEqual);

        // Translucent surfaces leave PF_Occlude off unless it is set.
        let translucent = RenderState::for_poly_flags(PF_TRANSLUCENT, false);
        let blend = translucent.blend.expect("translucent blends");
        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::One);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::OneMinusSrc);
        assert_eq!(blend.alpha.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);
        assert!(!translucent.depth_write);

        // Tile draws (remove=true) clear PF_Occlude on a blended surface.
        let occluding = PF_TRANSLUCENT | PF_OCCLUDE;
        assert!(RenderState::for_poly_flags(occluding, false).depth_write);
        assert!(!RenderState::for_poly_flags(occluding, true).depth_write);

        // Highlighted + translucent: SrcAlpha / OneMinusSrcAlpha.
        let highlighted = RenderState::for_poly_flags(PF_HIGHLIGHTED | PF_TRANSLUCENT, false);
        let blend = highlighted.blend.expect("highlighted blends");
        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::SrcAlpha);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);

        // Modulated: Dst / Src, alpha as DstAlpha / SrcAlpha.
        let modulated = RenderState::for_poly_flags(PF_MODULATED, false);
        let blend = modulated.blend.expect("modulated blends");
        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::Dst);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::Src);
        assert_eq!(blend.alpha.src_factor, wgpu::BlendFactor::DstAlpha);
        assert_eq!(blend.alpha.dst_factor, wgpu::BlendFactor::SrcAlpha);

        // Invisible: no color writes, Zero/One blend.
        let invisible = RenderState::for_poly_flags(PF_INVISIBLE, false);
        assert!(!invisible.color_write);
        let blend = invisible.blend.expect("invisible blends");
        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::Zero);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::One);

        // LumosAffected wins over the other blend bits.
        let lumos = RenderState::for_poly_flags(PF_LUMOS_AFFECTED, false);
        let blend = lumos.blend.expect("lumos blends");
        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::OneMinusSrcAlpha);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::SrcAlpha);
    }

    #[test]
    fn render_state_for_line_flags_depth_cues_only_when_asked() {
        let plain = RenderState::for_line_flags(0);
        assert_eq!(plain.depth_compare, wgpu::CompareFunction::Always);
        assert!(!plain.depth_write);
        assert!(plain.color_write);
        let blend = plain.blend.expect("lines blend as PF_Highlighted");
        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::One);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);

        let depth_cued = RenderState::for_line_flags(LINE_DEPTH_CUED);
        assert_eq!(depth_cued.depth_compare, wgpu::CompareFunction::LessEqual);
        assert!(depth_cued.depth_write);
    }

    #[test]
    fn render_state_maps_to_wgpu_target_and_depth_state() {
        let state = RenderState {
            blend: None,
            color_write: false,
            depth_write: true,
            depth_compare: wgpu::CompareFunction::Less,
        };
        let target = state.color_target(wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(target.format, wgpu::TextureFormat::Rgba8Unorm);
        assert!(target.write_mask.is_empty());
        let opaque = RenderState::OPAQUE.color_target(wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(opaque.write_mask, wgpu::ColorWrites::ALL);

        let depth = state.depth_stencil();
        assert_eq!(depth.format, DEPTH_FORMAT);
        assert_eq!(depth.depth_write_enabled, Some(true));
        assert_eq!(depth.depth_compare, Some(wgpu::CompareFunction::Less));
    }

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

        let pipeline =
            SimpleLinePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm, RenderState::OPAQUE);

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
        let depth_view = cleared_depth_view(device, queue, &render_target);

        pipeline.render(
            device,
            queue,
            LineDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                target: &render_target_view,
                depth_target: &depth_view,
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

        let pipeline =
            SimpleLinePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm, RenderState::OPAQUE);
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
        let depth_view = cleared_depth_view(device, queue, &render_target);

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
                depth_target: &depth_view,
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

    #[test]
    fn compute_tile_draw_color_uses_texture_alpha_when_positive() {
        let out = compute_tile_draw_color([0.5, 0.5, 0.5, 0.5], 0, Some(0.3));
        assert_eq!(
            out,
            [0.5, 0.5, 0.5, 0.3],
            "RGB must pass through unchanged, alpha must come from texture_alpha"
        );
    }

    #[test]
    fn compute_tile_draw_color_defaults_alpha_to_one_when_absent_or_non_positive() {
        // real condition is `Info.Texture->Alpha > 0.f`, strictly
        // greater than, not `>=` -- 0.0 itself does not count,
        // hand-verified against DrawTile.cpp:40-43's own text before
        // relying on it, not assumed from the flag's mere presence.
        assert_eq!(
            compute_tile_draw_color([0.1, 0.2, 0.3, 0.4], 0, None),
            [0.1, 0.2, 0.3, 1.0],
            "no real Texture object (None) must default alpha to 1.0"
        );
        assert_eq!(
            compute_tile_draw_color([0.1, 0.2, 0.3, 0.4], 0, Some(0.0)),
            [0.1, 0.2, 0.3, 1.0],
            "texture_alpha of exactly 0.0 is not > 0.0, must still default to 1.0"
        );
    }

    #[test]
    fn compute_tile_draw_color_pf_modulated_forces_white_but_texture_alpha_still_overrides() {
        const PF_MODULATED: u32 = 0x0000_0040;
        // The subtle real case (DrawTile.cpp:33-43 read in full, not
        // just the PF_Modulated branch alone): PF_Modulated forces
        // RGBA to (1,1,1,1) first, but the texture-alpha check right
        // after is unconditional, not an `else` -- it overrides alpha
        // again. An earlier, wrong reading of this function (before
        // the real source was re-read in full) would treat PF_
        // Modulated's own W=1.0 as final, producing [1,1,1,1.0] here
        // instead of the real [1,1,1,0.7].
        let out = compute_tile_draw_color([0.1, 0.2, 0.3, 0.4], PF_MODULATED, Some(0.7));
        assert_eq!(
            out,
            [1.0, 1.0, 1.0, 0.7],
            "PF_Modulated forces RGB (and alpha, provisionally) to 1.0, but the texture's own \
             positive Alpha must still override alpha again afterward, got {out:?}"
        );
    }

    #[test]
    fn compute_complex_diffuse_info_uses_real_values_when_texture_present_and_positive() {
        let out = compute_complex_diffuse_info(Some((0.5, 0.3, 0.7)), 255);
        let close = |a: [f32; 4], b: [f32; 4]| (0..4).all(|i| (a[i] - b[i]).abs() < 1e-4);
        assert!(
            close(out, [0.5, 0.3, 0.7, 1.0]),
            "expected [diffuse, specular, alpha, c_alpha/255] unchanged when all real values \
             are positive, got {out:?}"
        );
    }

    #[test]
    fn compute_complex_diffuse_info_defaults_diffuse_and_alpha_to_one_when_non_positive() {
        // Specular has no real >0.f check at all (unlike diffuse/
        // alpha) -- 0.3 must pass through unchanged even though
        // diffuse/alpha (0.0/-1.0, both non-positive) get defaulted.
        let out = compute_complex_diffuse_info(Some((0.0, 0.3, -1.0)), 128);
        let close = |a: [f32; 4], b: [f32; 4]| (0..4).all(|i| (a[i] - b[i]).abs() < 1e-4);
        assert!(
            close(out, [1.0, 0.3, 1.0, 128.0 / 255.0]),
            "expected diffuse/alpha each defaulted to 1.0 independently (non-positive value \
             check, same real formula as resolve_texture_alpha), specular passed through \
             unchanged (no real >0.f check on it at all), got {out:?}"
        );
    }

    #[test]
    fn compute_complex_diffuse_info_null_texture_fallback_is_asymmetric() {
        // The real null-texture fallback (DrawComplex.cpp:170) is
        // (1.0, 0.0, 0.0, cAlpha/255.0) -- diffuse defaults to 1.0 but
        // alpha(.z) defaults to 0.0, not a uniform 1.0 the way
        // compute_tile_draw_color's own Option<f32> shape might
        // suggest by surface analogy alone. This is the real case
        // most likely to be gotten wrong by assuming symmetry with
        // Gouraud's own different null-fallback shape instead of
        // reading DrawComplex.cpp's own real line directly.
        let out = compute_complex_diffuse_info(None, 64);
        let close = |a: [f32; 4], b: [f32; 4]| (0..4).all(|i| (a[i] - b[i]).abs() < 1e-4);
        assert!(
            close(out, [1.0, 0.0, 0.0, 64.0 / 255.0]),
            "expected the real asymmetric null fallback (1.0, 0.0, 0.0, c_alpha/255.0), got \
             {out:?} -- a uniform (1,1,1,_) or (0,0,0,_) fallback here would be wrong"
        );
    }

    #[test]
    fn tile_quad_corners_matches_the_real_submit_tile_batch_formula() {
        // rfx2=rfy2=0.02, fx2=fy2=50.0: same real values reference_
        // perspective_scene() + draw_2d_line_screen_to_coords's own
        // test above derives (fx=fy=100, effective_fov_angle=90).
        // x=10,y=20,xl=30,yl=40 (tile rect left=10,top=20,right=40,
        // bottom=60); u=0.1,v=0.2,ul=0.3,vl=0.4; z=1.0; u_mult=2.0,
        // v_mult=0.5 (deliberately distinct from each other, so a
        // mixed-up U/V multiplier bug would be caught). Hand-computed
        // independently (scratch Python, not read off the
        // implementation) before relying on it:
        //   top_left:     coords=(-0.8,-0.6,1), tex=(0.2,0.1)
        //   top_right:    coords=(-0.2,-0.6,1), tex=(0.8,0.1)
        //   bottom_right: coords=(-0.2, 0.2,1), tex=(0.8,0.3)
        //   bottom_left:  coords=(-0.8, 0.2,1), tex=(0.2,0.3)
        let out = tile_quad_corners(TileQuadRequest {
            rfx2: 0.02,
            rfy2: 0.02,
            fx2: 50.0,
            fy2: 50.0,
            x: 10.0,
            y: 20.0,
            xl: 30.0,
            yl: 40.0,
            u: 0.1,
            v: 0.2,
            ul: 0.3,
            vl: 0.4,
            z: 1.0,
            u_mult: 2.0,
            v_mult: 0.5,
        });
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5);
        let close2 = |a: [f32; 2], b: [f32; 2]| (0..2).all(|i| (a[i] - b[i]).abs() < 1e-5);
        assert!(
            close(out.top_left.0, [-0.8, -0.6, 1.0]) && close2(out.top_left.1, [0.2, 0.1]),
            "top_left mismatch: {out:?}"
        );
        assert!(
            close(out.top_right.0, [-0.2, -0.6, 1.0]) && close2(out.top_right.1, [0.8, 0.1]),
            "top_right mismatch: {out:?}"
        );
        assert!(
            close(out.bottom_right.0, [-0.2, 0.2, 1.0]) && close2(out.bottom_right.1, [0.8, 0.3]),
            "bottom_right mismatch: {out:?}"
        );
        assert!(
            close(out.bottom_left.0, [-0.8, 0.2, 1.0]) && close2(out.bottom_left.1, [0.2, 0.3]),
            "bottom_left mismatch: {out:?}"
        );
    }

    #[test]
    fn point_quad_corners_matches_the_real_draw_2d_point_formula() {
        // Same real rfx2=rfy2=0.02/fx2=fy2=50.0 derivation as tile_
        // quad_corners's own test above. x1=10,y1=20,x2=15,y2=25
        // (a small, non-degenerate point rect, so all four corners
        // are distinguishable), z=1.0. Hand-computed independently
        // (scratch Python) before relying on it:
        //   top_left:     (-0.81, -0.61, 1.0)   [coord(x1-0.5,y1-0.5)]
        //   top_right:    (-0.69, -0.61, 1.0)   [coord(x2+0.5,y1-0.5)]
        //   bottom_right: (-0.69, -0.49, 1.0)   [coord(x2+0.5,y2+0.5)]
        //   bottom_left:  (-0.81, -0.49, 1.0)   [coord(x1-0.5,y2+0.5)]
        // The real +-0.5 inset/expansion (DrawSimple.cpp:187-198) is
        // the one thing distinguishing this from tile_quad_corners's
        // own formula -- without it, top_left would be (-0.8,-0.6,1)
        // instead (coord(x1,y1), no offset), a real, hand-verified
        // different value this test's own negative control confirms
        // it actually discriminates (see commit message).
        let out = point_quad_corners(PointQuadRequest {
            rfx2: 0.02,
            rfy2: 0.02,
            fx2: 50.0,
            fy2: 50.0,
            x1: 10.0,
            y1: 20.0,
            x2: 15.0,
            y2: 25.0,
            z: 1.0,
        });
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5);
        assert!(
            close(out.top_left, [-0.81, -0.61, 1.0]),
            "top_left mismatch: {out:?}"
        );
        assert!(
            close(out.top_right, [-0.69, -0.61, 1.0]),
            "top_right mismatch: {out:?}"
        );
        assert!(
            close(out.bottom_right, [-0.69, -0.49, 1.0]),
            "bottom_right mismatch: {out:?}"
        );
        assert!(
            close(out.bottom_left, [-0.81, -0.49, 1.0]),
            "bottom_left mismatch: {out:?}"
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

        let pipeline =
            TilePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm, RenderState::OPAQUE);

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
            1,
            1,
            &[255, 0, 0, 255],
            crate::texture::TextureOptions::default(),
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
        let depth_view = cleared_depth_view(device, queue, &render_target);

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
                depth_target: &depth_view,
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

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn tile_pipeline_renders_a_real_dxt1_compressed_texture_into_an_offscreen_texture() {
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
            label: Some("tile dxt1 render test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let pipeline =
            TilePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm, RenderState::OPAQUE);

        // A real, hand-built single-block BC1/DXT1 4x4 texture, solid
        // opaque red -- color0 == color1 == pure red in RGB565
        // (0xF800: R=11111,G=000000,B=00000 from the MSB), little-
        // endian u16 each, then 4 bytes of 2-bit-per-texel index data
        // all zero (every one of the 16 texels selects color0). BC1's
        // own real decode rule when color0 <= color1 (true here, they
        // are equal) produces color2 = lerp(color0,color1,0.5) = the
        // same red (not a 4-color ramp) and reserves index 3 for
        // transparent black -- irrelevant here since every index is 0,
        // selecting color0/red directly, not derived from the ramp at
        // all. A real BC1 hardware/driver decoder is what proves this
        // byte layout right, not an assumption re-derived from the
        // format's own written spec alone.
        let dxt1_block: [u8; 8] = [0x00, 0xF8, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00];
        let mut texture_cache = crate::texture::TextureCache::new();
        let Some(cached) = texture_cache.upload_compressed(
            &created,
            1,
            4,
            4,
            &[crate::texture::CompressedMipLevel {
                width: 4,
                height: 4,
                data: &dxt1_block,
            }],
            crate::texture::TextureOptions::default(),
        ) else {
            println!(
                "SKIPPED: adapter {:?} backend {} does not actually grant \
                 Features::TEXTURE_COMPRESSION_BC -- the real engine's own SupportsS3TC-gated \
                 graceful degradation (compute_tile_draw_flags's doc / create_compressed_gpu_\
                 texture's own doc both already cover why this is a real, not a bug), nothing \
                 to assert here on this adapter",
                created.adapter_name,
                backend_name(created.backend)
            );
            return;
        };
        let cached_view = cached.view.clone();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

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
        let draw_params = [TileDrawParams {
            draw_color: [1.0, 1.0, 1.0, 1.0],
            draw_flags: 0,
            _padding: [0; 3],
        }];

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tile dxt1 render test target"),
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
        let depth_view = cleared_depth_view(device, queue, &render_target);

        pipeline.render(
            device,
            queue,
            TileDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                texture_view: &cached_view,
                sampler: &sampler,
                target: &render_target_view,
                depth_target: &depth_view,
                load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
            },
        );

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tile dxt1 render test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tile dxt1 render test readback encoder"),
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
            "expected a real GPU BC1 hardware decoder to decode this hand-built block as solid \
             opaque red, got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected a pixel outside the drawn quad's span to still read back the real clear \
             color, blue -- got {right_pixel:?}"
        );
        println!(
            "TextureCache::upload_compressed + TilePipeline::render confirmed empirically: a \
             real hand-built DXT1/BC1 block, uploaded through create_compressed_gpu_texture and \
             sampled through a real GPU hardware decoder, produced the right decoded color at \
             the right position in a real offscreen texture on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn gouraud_pipeline_renders_a_real_textured_triangle_mesh_into_an_offscreen_texture() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let device = &created.device;
        let queue = &created.queue;

        // Identity modelviewproj_mat (bytes [256..320)), same real
        // technique as the other pipeline tests above. light_color_
        // intensity at byte 520 (FrameUniforms' own real field order,
        // re-read directly from its own struct definition rather than
        // assumed from a truncated earlier view of it, which would
        // have given the wrong offset -- light_space_mat/frame_coords/
        // frame_uncoords are three more real Mat4 fields between
        // modelviewproj_mat and the trailing scalars, not adjacent to
        // them) set to 1.0 (neutral): gouraud.wgsl's own vs_main
        // computes `out.light_color = in.light_color *
        // light_color_intensity`, so a non-1.0 value here would scale
        // this test's own light_color input unexpectedly.
        let mut frame_bytes = [0u8; 528];
        for i in 0..4 {
            let offset = 256 + i * 20;
            frame_bytes[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        frame_bytes[520..524].copy_from_slice(&1.0f32.to_le_bytes());
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gouraud render test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let pipeline =
            GouraudPipeline::new(device, wgpu::TextureFormat::Rgba8Unorm, RenderState::OPAQUE);

        // Same real solid-red 1x1 texture technique as the tile
        // pipeline test above.
        let mut texture_cache = crate::texture::TextureCache::new();
        let cached = texture_cache.upload_rgba8(
            &created,
            1,
            1,
            1,
            &[255, 0, 0, 255],
            crate::texture::TextureOptions::default(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // Same real six-vertex, two-triangle quad shape as the tile
        // pipeline test above (left half of the 64px-wide target) --
        // light_color=[1,1,1,1] (neutral: the real non-modulated,
        // non-fog fragment branch computes `color * vec4(light_color.
        // rgb, 1.0)`, so white keeps the sampled red texel unchanged),
        // fog_color=[0,0,0,0] (draw_flags=0 means DF_RenderFog is
        // unset, so this is never read in that branch -- zeroed for
        // cleanliness only, not because its value matters here).
        let light_color = [1.0, 1.0, 1.0, 1.0];
        let fog_color = [0.0, 0.0, 0.0, 0.0];
        let vertices = [
            GouraudVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 1.0],
                light_color,
                fog_color,
            },
            GouraudVertex {
                coords: [0.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 1.0],
                light_color,
                fog_color,
            },
            GouraudVertex {
                coords: [0.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 0.0],
                light_color,
                fog_color,
            },
            GouraudVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 1.0],
                light_color,
                fog_color,
            },
            GouraudVertex {
                coords: [0.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [1.0, 0.0],
                light_color,
                fog_color,
            },
            GouraudVertex {
                coords: [-1.0, 1.0, 0.0],
                draw_id: 0,
                tex_coords: [0.0, 0.0],
                light_color,
                fog_color,
            },
        ];
        // diffuse_info=[u_mult=1,v_mult=1,diffuse=1,texture_alpha=1]
        // (all neutral -- this test is not exercising texture_uv_mult
        // or the real diffuse/alpha scaling, which have their own
        // separate, dedicated unit tests), draw_flags=0 (no masking/
        // alpha-blend/fog/modulated -- compute_tile_draw_flags's own
        // tests cover that translation separately).
        let draw_params = [GouraudDrawParams {
            diffuse_info: [1.0, 1.0, 1.0, 1.0],
            draw_color: [0.0, 0.0, 0.0, 1.0],
            draw_flags: 0,
            _padding: [0; 3],
        }];

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gouraud render test target"),
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
        let depth_view = cleared_depth_view(device, queue, &render_target);

        pipeline.render(
            device,
            queue,
            GouraudDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                texture_view: &cached.view,
                sampler: &sampler,
                target: &render_target_view,
                depth_target: &depth_view,
                load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
            },
        );

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gouraud render test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gouraud render test readback encoder"),
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
            "expected a pixel inside the drawn mesh's span to read back red (the real sampled \
             texel times the real neutral light_color/diffuse), got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected a pixel outside the drawn mesh's span to still read back the real clear \
             color, blue -- got {right_pixel:?}"
        );
        println!(
            "GouraudPipeline::render confirmed empirically: a real DrawTriangles-shaped \
             textured mesh, uploaded through a real TextureCache entry and real buffers/bind \
             groups, produced the right pixels at the right positions in a real offscreen \
             texture on adapter {:?} backend {}",
            created.adapter_name,
            backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn complex_pipeline_renders_a_real_textured_surface_into_an_offscreen_texture() {
        let created = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>)
            .expect("expected a real device to be created");
        let device = &created.device;
        let queue = &created.queue;

        // Same real identity-modelviewproj_mat/light_color_intensity=1.0
        // technique as the gouraud pipeline test above.
        let mut frame_bytes = [0u8; 528];
        for i in 0..4 {
            let offset = 256 + i * 20;
            frame_bytes[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        frame_bytes[520..524].copy_from_slice(&1.0f32.to_le_bytes());
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("complex render test frame uniforms"),
            size: 528,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&frame_buffer, 0, &frame_bytes);

        let pipeline =
            ComplexPipeline::new(device, wgpu::TextureFormat::Rgba8Unorm, RenderState::OPAQUE);

        // Same real solid-red 1x1 texture technique as the tile/
        // gouraud pipeline tests -- bound for all three real layers
        // (diffuse/light map/fog map): draw_flags=0 below means
        // neither DF_LightMap nor DF_FogMap is set, so complex.wgsl's
        // own real select() calls discard whatever is sampled from
        // the light-map/fog-map slots entirely (that shader's own
        // module doc: "any valid texture works, its color is never
        // read") -- reusing the diffuse texture there is simplest,
        // not a claim this is what a real two-layer surface would do.
        let mut texture_cache = crate::texture::TextureCache::new();
        let cached = texture_cache.upload_rgba8(
            &created,
            1,
            1,
            1,
            &[255, 0, 0, 255],
            crate::texture::TextureOptions::default(),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // x_axis=(1,0,0,0), y_axis=(0,1,0,0): the real per-vertex UV
        // mapping (`map_dot = (dot(x_axis.xyz,coords)-x_axis.w, dot(
        // y_axis.xyz,coords)-y_axis.w)`) collapses to `(coords.x,
        // coords.y)` directly -- a deliberately trivial real mapping,
        // not a claim this is what any actual real BSP surface uses.
        // With a 1x1 texture the exact UV value sampled never matters
        // (there is only one texel regardless of address mode or
        // exact coordinate, including negative ones) -- this test
        // exists to prove the pipeline/bind-group/compositing
        // machinery works, not to exercise a specific real UV-mapping
        // scenario (texture_uv_mult/texture_uv_pan's own tests already
        // cover the real formula in isolation).
        let diffuse_uv = [1.0, 1.0, 0.0, 0.0];
        let x_axis = [1.0, 0.0, 0.0, 0.0];
        let y_axis = [0.0, 1.0, 0.0, 0.0];
        // Same real six-vertex, two-triangle quad shape as the tile/
        // gouraud pipeline tests (left half of the 64px-wide target).
        let vertices = [
            ComplexVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
            },
            ComplexVertex {
                coords: [0.0, -1.0, 0.0],
                draw_id: 0,
            },
            ComplexVertex {
                coords: [0.0, 1.0, 0.0],
                draw_id: 0,
            },
            ComplexVertex {
                coords: [-1.0, -1.0, 0.0],
                draw_id: 0,
            },
            ComplexVertex {
                coords: [0.0, 1.0, 0.0],
                draw_id: 0,
            },
            ComplexVertex {
                coords: [-1.0, 1.0, 0.0],
                draw_id: 0,
            },
        ];
        // diffuse_info=[1,0,1,1] (compute_complex_diffuse_info(Some((
        // 1.0,0.0,1.0)), 255), hand-computed: diffuse/alpha both
        // already positive so pass through, c_alpha=255 -> w=1.0),
        // draw_flags=0 (no masking/alpha-blend/modulated/lightmap/
        // fogmap -- compute_tile_draw_flags's own tests cover the
        // real PolyFlags->DrawFlags translation separately).
        let draw_params = [ComplexDrawParams {
            diffuse_uv,
            light_map_uv: [0.0; 4],
            fog_map_uv: [0.0; 4],
            diffuse_info: [1.0, 0.0, 1.0, 1.0],
            x_axis,
            y_axis,
            draw_flags: 0,
            _padding: [0; 3],
        }];

        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("complex render test target"),
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
        let depth_view = cleared_depth_view(device, queue, &render_target);

        pipeline.render(
            device,
            queue,
            ComplexDrawCall {
                frame_uniforms_buffer: &frame_buffer,
                vertices: &vertices,
                draw_params: &draw_params,
                diffuse_texture_view: &cached.view,
                diffuse_sampler: &sampler,
                light_map_texture_view: &cached.view,
                light_map_sampler: &sampler,
                fog_map_texture_view: &cached.view,
                fog_map_sampler: &sampler,
                target: &render_target_view,
                depth_target: &depth_view,
                load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
            },
        );

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("complex render test readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("complex render test readback encoder"),
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
            "expected a pixel inside the drawn surface's span to read back red (the real \
             sampled diffuse texel times the real neutral diffuse_info, with no lightmap/fogmap \
             flags set), got {left_pixel:?}"
        );
        let right_pixel = &mapped[56 * 4..56 * 4 + 4];
        assert_eq!(
            right_pixel,
            &[0u8, 0, 255, 255],
            "expected a pixel outside the drawn surface's span to still read back the real \
             clear color, blue -- got {right_pixel:?}"
        );
        println!(
            "ComplexPipeline::render confirmed empirically: a real DrawComplexSurface-shaped \
             textured surface, uploaded through a real TextureCache entry and real buffers/bind \
             groups (three texture/sampler pairs in one bind group), produced the right pixels \
             at the right positions in a real offscreen texture on adapter {:?} backend {}",
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

        let pipeline =
            TilePipeline::new(device, wgpu::TextureFormat::Rgba8Unorm, RenderState::OPAQUE);
        let mut texture_cache = crate::texture::TextureCache::new();
        let cached = texture_cache.upload_rgba8(
            &created,
            1,
            1,
            1,
            &[255, 0, 0, 255],
            crate::texture::TextureOptions::default(),
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
        let depth_view = cleared_depth_view(device, queue, &render_target);

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
                depth_target: &depth_view,
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
