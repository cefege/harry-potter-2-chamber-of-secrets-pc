//! Surface-class draw pipelines (Milestone M3).
//!
//! Boring two-pass design: one draw call per surface class, no bindless.
//! Every class shares one vertex layout ([`crate::mesh_gpu::MeshVertex`]),
//! one uniform block, and differs only in fragment shader + blend state:
//!
//! | class       | shader      | blend                | depth write |
//! |-------------|-------------|----------------------|-------------|
//! | Opaque      | textured gouraud | replace         | yes |
//! | Masked      | textured + alpha-test discard | replace | yes |
//! | Lightmap    | base × lightmap (UV set 1)    | replace | yes |
//! | Detail      | base × detail × 2             | replace | yes |
//! | Palette     | P8 index → LUT lookup         | replace | yes |
//! | Translucent | textured                      | src-alpha | no |
//! | Modulate    | textured                      | dst×src   | no |

use crate::device::{GpuContext, OffscreenTarget};
use crate::mesh_gpu::{MeshGpu, MeshVertex};
use std::num::NonZeroU64;

/// Draw-order classes; ordering into opaque-then-translucent passes happens
/// at scene level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Opaque,
    Masked,
    Lightmap,
    Detail,
    Palette,
    Translucent,
    Modulate,
    /// Fake-backdrop: samples the sky render (second bind group) by screen
    /// position — UE1 fake backdrop.
    Backdrop,
}

/// Matches the WGSL `Uniforms` struct (96 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PassUniforms {
    /// Clip-space transform (camera projection × view).
    pub view_proj: [f32; 16],
    /// Scene fog color (rgb) — alpha slot unused.
    pub fog_color: [f32; 4],
    /// x = alpha-test cutoff for the masked pass; rest reserved.
    pub misc: [f32; 4],
}

impl PassUniforms {
    pub const SIZE: u64 = std::mem::size_of::<Self>() as u64;

    pub fn identity() -> PassUniforms {
        PassUniforms {
            view_proj: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
            fog_color: [0.0, 0.0, 0.0, 1.0],
            misc: [0.333, 0.0, 0.0, 0.0],
        }
    }
}

const COMMON_WGSL: &str = r"
struct Uniforms {
    view_proj: mat4x4<f32>,
    fog_color: vec4<f32>,
    misc: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var base_sampler: sampler;
@group(0) @binding(2) var base_texture: texture_2d<f32>;
@group(1) @binding(0) var second_sampler: sampler;
@group(1) @binding(1) var second_texture: texture_2d<f32>;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) uv1: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) unfogged: f32,
};

struct FsIn {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv0: vec2<f32>,
    @location(1) uv1: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) unfogged: f32,
};

@vertex
fn vs_main(in: VsIn) -> FsIn {
    var out: FsIn;
    out.clip = u.view_proj * vec4<f32>(in.position, 1.0);
    out.uv0 = in.uv0;
    out.uv1 = in.uv1;
    out.color = in.color;
    out.unfogged = in.unfogged;
    return out;
}

// Per-vertex fog: mix toward the scene fog color by the vertex's unfogged
// factor (0 = fully fogged).
fn apply_fog(frag: FsIn, rgb: vec3<f32>) -> vec3<f32> {
    return mix(u.fog_color.rgb, rgb, clamp(frag.unfogged, 0.0, 1.0));
}

fn sample_base(frag: FsIn) -> vec4<f32> {
    return textureSample(base_texture, base_sampler, frag.uv0);
}

fn shade(frag: FsIn, sampled: vec4<f32>) -> vec4<f32> {
    let rgb = apply_fog(frag, sampled.rgb * frag.color.rgb);
    return vec4<f32>(rgb, sampled.a * frag.color.a);
}
";

const FRAG_OPAQUE: &str = r"
@fragment
fn fs_main(frag: FsIn) -> @location(0) vec4<f32> {
    return shade(frag, sample_base(frag));
}
";

const FRAG_MASKED: &str = r"
@fragment
fn fs_main(frag: FsIn) -> @location(0) vec4<f32> {
    let sampled = sample_base(frag);
    if (sampled.a * frag.color.a < u.misc.x) { discard; }
    return shade(frag, sampled);
}
";

const FRAG_LIGHTMAP: &str = r"
@fragment
fn fs_main(frag: FsIn) -> @location(0) vec4<f32> {
    // UE1 display-space overbright: base x lightmap x 2.
    let base = sample_base(frag);
    let lm = textureSample(second_texture, second_sampler, frag.uv1);
    return shade(frag, vec4<f32>(base.rgb * lm.rgb * 2.0, base.a));
}
";

const FRAG_BACKDROP: &str = r"
@fragment
fn fs_main(frag: FsIn) -> @location(0) vec4<f32> {
    // Screen-space sample of the sky render (u.misc.y/z = target dims).
    let uv = frag.clip.xy / vec2<f32>(u.misc.y, u.misc.z);
    return textureSample(second_texture, second_sampler, uv);
}
";

const FRAG_DETAIL: &str = r"
@fragment
fn fs_main(frag: FsIn) -> @location(0) vec4<f32> {
    // UE-style detail multiply brightens midtones: base * detail * 2.
    let base = sample_base(frag);
    let detail = textureSample(second_texture, second_sampler, frag.uv1);
    let rgb = min(base.rgb * detail.rgb * 2.0, vec3<f32>(1.0));
    return shade(frag, vec4<f32>(rgb, base.a));
}
";

const FRAG_PALETTE: &str = r"
@fragment
fn fs_main(frag: FsIn) -> @location(0) vec4<f32> {
    // Base texture holds P8 indices as R8Unorm; look up the 256x1 RGBA LUT.
    let indexed = sample_base(frag).r;
    let index = floor(indexed * 255.0 + 0.5);
    let lut_uv = vec2<f32>((index + 0.5) / 256.0, 0.5);
    let color = textureSampleLevel(second_texture, second_sampler, lut_uv, 0.0);
    return shade(frag, color);
}
";

const SHARED_TAIL: &str = r#"
@fragment
fn fs_main(frag: FsIn) -> @location(0) vec4<f32> {
    return shade(frag, sample_base(frag));
}
"#;

enum BlendClass {
    Replace,
    Alpha,
    Modulate,
}

impl BlendClass {
    fn state(self) -> wgpu::BlendState {
        match self {
            BlendClass::Replace => wgpu::BlendState::REPLACE,
            BlendClass::Alpha => wgpu::BlendState::ALPHA_BLENDING,
            BlendClass::Modulate => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            },
        }
    }
}

/// Nearest for world geometry compat; linear kept for downscaled sprites.
pub struct Samplers {
    pub nearest: wgpu::Sampler,
    pub linear: wgpu::Sampler,
}

/// All pipelines plus shared resources for one device.
pub struct PipelineSet {
    pub uniform_buffer: wgpu::Buffer,
    pub samplers: Samplers,
    bind_group_layout_base: wgpu::BindGroupLayout,
    bind_group_layout_secondary: wgpu::BindGroupLayout,
    /// Bound at group 1 by classes that don't use it (the layout still
    /// declares the group).
    null_secondary_view: wgpu::TextureView,
    pipelines: Vec<(SurfaceKind, wgpu::RenderPipeline)>,
}

impl PipelineSet {
    pub fn new(
        ctx: &GpuContext,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> PipelineSet {
        let module = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("hp-render surface shaders"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(COMMON_WGSL)),
            });

        let uniform_size = NonZeroU64::new(PassUniforms::SIZE).expect("nonzero uniform size");
        let bind_group_layout_base =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("hp-render base bindings"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: Some(uniform_size),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                    ],
                });
        let bind_group_layout_secondary =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("hp-render secondary bindings"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                    ],
                });

        let layout = ctx
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("hp-render pipeline layout"),
                bind_group_layouts: &[
                    Some(&bind_group_layout_base),
                    Some(&bind_group_layout_secondary),
                ],
                immediate_size: 0,
            });

        let uniform_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hp-render pass uniforms"),
            size: PassUniforms::SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let samplers = Samplers {
            nearest: ctx.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("hp-render nearest"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }),
            linear: ctx.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("hp-render linear"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        };

        // Constructed in draw-pass order: opaque classes first.
        let kinds = [
            (
                SurfaceKind::Opaque,
                "fs_opaque",
                FRAG_OPAQUE,
                BlendClass::Replace,
                true,
            ),
            (
                SurfaceKind::Masked,
                "fs_masked",
                FRAG_MASKED,
                BlendClass::Replace,
                true,
            ),
            (
                SurfaceKind::Lightmap,
                "fs_lightmap",
                FRAG_LIGHTMAP,
                BlendClass::Replace,
                true,
            ),
            (
                SurfaceKind::Detail,
                "fs_detail",
                FRAG_DETAIL,
                BlendClass::Replace,
                true,
            ),
            (
                SurfaceKind::Palette,
                "fs_palette",
                FRAG_PALETTE,
                BlendClass::Replace,
                true,
            ),
            (
                SurfaceKind::Translucent,
                "fs_translucent",
                SHARED_TAIL,
                BlendClass::Alpha,
                false,
            ),
            (
                SurfaceKind::Modulate,
                "fs_modulate",
                SHARED_TAIL,
                BlendClass::Modulate,
                false,
            ),
            (
                SurfaceKind::Backdrop,
                "fs_backdrop",
                FRAG_BACKDROP,
                BlendClass::Replace,
                true,
            ),
        ];
        let mut pipelines = Vec::with_capacity(kinds.len());
        for (kind, label, frag_src, blend, depth_write) in kinds {
            let frag_module = ctx
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(label),
                    source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(
                        COMMON_WGSL.to_string() + frag_src,
                    )),
                });
            pipelines.push((
                kind,
                ctx.device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(label),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState {
                            module: &module,
                            entry_point: Some("vs_main"),
                            compilation_options: Default::default(),
                            buffers: &[Some(MeshVertex::buffer_layout())],
                        },
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::TriangleList,
                            // Screen basis (right, up, forward) is left-handed
                            // (UE1/D3D style): a surface whose normal faces the
                            // camera projects CLOCKWISE, so front = CW.
                            front_face: wgpu::FrontFace::Cw,
                            cull_mode: Some(wgpu::Face::Back),
                            ..Default::default()
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: depth_format,
                            depth_write_enabled: Some(depth_write),
                            depth_compare: Some(wgpu::CompareFunction::LessEqual),
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: Default::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &frag_module,
                            entry_point: Some("fs_main"),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: color_format,
                                blend: Some(blend.state()),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    }),
            ));
        }

        // 1x1 opaque white stand-in for classes whose shader never reads
        // group 1 but whose layout still declares it.
        let null_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hp-render null secondary"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        ctx.queue.write_texture(
            null_texture.as_image_copy(),
            &[255u8; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let null_secondary_view = null_texture.create_view(&Default::default());

        PipelineSet {
            uniform_buffer,
            samplers,
            bind_group_layout_base,
            bind_group_layout_secondary,
            null_secondary_view,
            pipelines,
        }
    }

    pub fn pipeline(&self, kind: SurfaceKind) -> &wgpu::RenderPipeline {
        self.pipelines
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, p)| p)
            .unwrap_or_else(|| panic!("pipeline for {kind:?} not built"))
    }

    /// Group 0: pass uniforms + base texture (nearest-filtered).
    pub fn bind_group_base(
        &self,
        ctx: &GpuContext,
        base_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hp-render base bind group"),
            layout: &self.bind_group_layout_base,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.samplers.nearest),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(base_view),
                },
            ],
        })
    }

    /// Group 1: lightmap / detail / palette-LUT texture.
    pub fn bind_group_secondary(
        &self,
        ctx: &GpuContext,
        second_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hp-render secondary bind group"),
            layout: &self.bind_group_layout_secondary,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.samplers.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(second_view),
                },
            ],
        })
    }

    pub fn update_uniforms(&self, ctx: &GpuContext, uniforms: &PassUniforms) {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                std::ptr::from_ref(uniforms).cast::<u8>(),
                PassUniforms::SIZE as usize,
            )
        };
        ctx.queue.write_buffer(&self.uniform_buffer, 0, bytes);
    }
}

/// One recorded draw: mesh + which pipeline + its texture bindings.
pub struct DrawItem<'a> {
    pub kind: SurfaceKind,
    pub mesh: &'a MeshGpu,
    /// Group-0 texture (base / index map).
    pub base_view: &'a wgpu::TextureView,
    /// Group-1 texture (lightmap / detail / LUT); required by those classes.
    pub second_view: Option<&'a wgpu::TextureView>,
}

/// Record all draws into one pass over the given color+depth attachments:
/// one `Clear` to `backdrop`, then opaque classes first, then
/// translucent/modulate classes in list order. Deterministic submission
/// order; the window path presents after, the offscreen path reads pixels
/// back.
pub fn encode_frame_views(
    ctx: &GpuContext,
    set: &PipelineSet,
    draws: &[DrawItem<'_>],
    backdrop: wgpu::Color,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    // Sky-render override for Backdrop draws: (sky color view, [w, h]).
    sky: Option<(&wgpu::TextureView, [f32; 2])>,
) {
    let mut encoder = ctx.device.create_command_encoder(&Default::default());
    let opaque_first =
        |kind: SurfaceKind| !matches!(kind, SurfaceKind::Translucent | SurfaceKind::Modulate);

    // Bind groups must outlive the pass; keep them alive in these vectors.
    let mut base_groups: Vec<wgpu::BindGroup> = Vec::new();
    let mut second_groups: Vec<wgpu::BindGroup> = Vec::new();

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hp-render frame"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(backdrop),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            multiview_mask: Default::default(),
            timestamp_writes: None,
        });

        for draw in draws.iter().filter(|d| opaque_first(d.kind)) {
            record_draw(ctx, &mut pass, set, draw, sky, &mut base_groups, &mut second_groups);
        }
        for draw in draws.iter().filter(|d| !opaque_first(d.kind)) {
            record_draw(ctx, &mut pass, set, draw, sky, &mut base_groups, &mut second_groups);
        }
    }

    ctx.queue.submit(Some(encoder.finish()));
}

/// Record all draws into one pass over `target`: one `Clear` to `backdrop`,
/// then opaque classes first, then translucent/modulate classes in list
/// order. Deterministic submission order; caller reads pixels back after.
pub fn encode_frame<'a>(
    ctx: &'a GpuContext,
    target: &OffscreenTarget,
    set: &'a PipelineSet,
    draws: &'a [DrawItem<'a>],
    backdrop: wgpu::Color,
) {
    encode_frame_views(
        ctx,
        set,
        draws,
        backdrop,
        &target.color_view(),
        &target.depth_view(),
        None,
    );
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
}

fn record_draw<'a>(
    ctx: &'a GpuContext,
    pass: &mut wgpu::RenderPass<'a>,
    set: &'a PipelineSet,
    draw: &DrawItem<'a>,
    sky: Option<(&wgpu::TextureView, [f32; 2])>,
    base_groups: &mut Vec<wgpu::BindGroup>,
    second_groups: &mut Vec<wgpu::BindGroup>,
) {
    base_groups.push(set.bind_group_base(ctx, draw.base_view));
    pass.set_pipeline(set.pipeline(draw.kind));
    pass.set_bind_group(0, base_groups.last().unwrap(), &[]);
    // Backdrop draws sample the sky render, not their own second view.
    let second_view = match draw.kind {
        SurfaceKind::Backdrop => sky.map(|(view, _)| view),
        _ => draw.second_view,
    }
    .unwrap_or(&set.null_secondary_view);
    second_groups.push(set.bind_group_secondary(ctx, second_view));
    pass.set_bind_group(1, second_groups.last().unwrap(), &[]);
    pass.set_vertex_buffer(0, draw.mesh.vertex_buffer.slice(..));
    pass.set_index_buffer(draw.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
    pass.draw_indexed(0..draw.mesh.num_indices, 0, 0..1);
}
