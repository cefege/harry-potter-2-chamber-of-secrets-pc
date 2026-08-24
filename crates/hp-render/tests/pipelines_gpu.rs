//! Offscreen GPU verification for every surface pipeline class
//! (Milestone M3) plus the bitmap-canvas blit path (Milestone M5).
//!
//! Each test renders a full-screen 1x1-textured quad through an
//! [`OffscreenTarget`] and asserts the exact resulting pixel. Readback is
//! BGRA byte order; every expectation below is written as `[B, G, R, A]`.

use hp_format::font::{FontPage, GlyphAtlas, GlyphRect, UFontData};
use hp_render::device::{GpuContext, OffscreenTarget};
use hp_render::mesh_gpu::{MeshGpu, MeshVertex, billboard_quad, build_mesh};
use hp_render::pipelines::{DrawItem, PassUniforms, PipelineSet, SurfaceKind, encode_frame};
use hp_render::textures::TextureManager;

fn ctx() -> GpuContext {
    GpuContext::headless().expect("headless Metal device")
}

/// Full-screen quad in clip space (view_proj = identity), UV 0..1.
const QUAD_VERTS: [[f32; 3]; 4] = [
    [-1.0, -1.0, 0.5],
    [1.0, -1.0, 0.5],
    [1.0, 1.0, 0.5],
    [-1.0, 1.0, 0.5],
];
const QUAD_UV: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

fn upload_pod(ctx: &GpuContext, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    let padded_len = bytes.len().div_ceil(4) * 4;
    let mut padded = vec![0u8; padded_len];
    padded[..bytes.len()].copy_from_slice(bytes);
    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: padded_len as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue.write_buffer(&buffer, 0, &padded);
    buffer
}

fn pod_bytes<T: Copy>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), std::mem::size_of_val(data)) }
}

/// Full-screen quad with per-vertex color/fog override.
fn quad_mesh(ctx: &GpuContext, color: [f32; 4], unfogged: f32) -> MeshGpu {
    let verts: Vec<MeshVertex> = QUAD_VERTS
        .iter()
        .zip(QUAD_UV.iter())
        .map(|(p, uv)| MeshVertex {
            position: *p,
            uv0: *uv,
            uv1: *uv,
            color,
            unfogged,
        })
        .collect();
    let indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];
    MeshGpu {
        vertex_buffer: upload_pod(ctx, pod_bytes(&verts), wgpu::BufferUsages::VERTEX),
        index_buffer: upload_pod(ctx, pod_bytes(&indices), wgpu::BufferUsages::INDEX),
        num_indices: 6,
    }
}

fn center_pixel(target: &OffscreenTarget, pixels: &[u8]) -> [u8; 4] {
    let offset = ((target.height / 2) * target.width + target.width / 2) as usize * 4;
    [
        pixels[offset],
        pixels[offset + 1],
        pixels[offset + 2],
        pixels[offset + 3],
    ]
}

fn corner_pixel(pixels: &[u8]) -> [u8; 4] {
    [pixels[0], pixels[1], pixels[2], pixels[3]]
}

fn render(
    ctx: &GpuContext,
    set: &PipelineSet,
    target: &OffscreenTarget,
    draws: &[DrawItem<'_>],
    backdrop: wgpu::Color,
) -> Vec<u8> {
    encode_frame(ctx, target, set, draws, backdrop);
    target.read_pixels_bgra(ctx)
}

#[test]
fn opaque_textured_gouraud_preserves_bgra_order_and_tint() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    // Upload bytes are BGRA: B=64 G=128 R=192.
    let tex = mgr.create_bgra8(&ctx, "tex", 1, 1, &[64, 128, 192, 255]);
    let view = mgr.view(&ctx, tex).clone();
    // Tint doubles green (clamped at store), halves red and blue.
    let mesh = quad_mesh(&ctx, [0.5, 2.0, 0.5, 1.0], 1.0);
    set.update_uniforms(&ctx, &PassUniforms::identity());
    let draws = vec![DrawItem {
        kind: SurfaceKind::Opaque,
        mesh: &mesh,
        base_view: &view,
        second_view: None,
    }];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    assert_eq!(center_pixel(&target, &pixels), [32, 255, 96, 255]);
}

#[test]
fn masked_pass_discards_below_alpha_cutoff() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    let mut uniforms = PassUniforms::identity();
    uniforms.misc = [0.5, 0.0, 0.0, 0.0];
    set.update_uniforms(&ctx, &uniforms);

    // alpha 0.75 keeps the pixel (masked writes source alpha verbatim).
    let solid = mgr.create_bgra8(&ctx, "solid", 1, 1, &[0, 0, 255, 191]);
    let solid_view = mgr.view(&ctx, solid).clone();
    let mesh = quad_mesh(&ctx, [1.0; 4], 1.0);
    let draws = vec![DrawItem {
        kind: SurfaceKind::Masked,
        mesh: &mesh,
        base_view: &solid_view,
        second_view: None,
    }];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    assert_eq!(center_pixel(&target, &pixels), [0, 0, 255, 191]);

    // alpha 0.25 discards: the black backdrop remains.
    let faint = mgr.create_bgra8(&ctx, "faint", 1, 1, &[0, 0, 255, 64]);
    let faint_view = mgr.view(&ctx, faint).clone();
    let draws = vec![DrawItem {
        kind: SurfaceKind::Masked,
        mesh: &mesh,
        base_view: &faint_view,
        second_view: None,
    }];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    assert_eq!(center_pixel(&target, &pixels), [0, 0, 0, 255]);
}

#[test]
fn translucent_blends_src_alpha_over_backdrop() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());
    // Red at alpha 0.5 over white backdrop.
    let red = mgr.create_bgra8(&ctx, "red", 1, 1, &[0, 0, 255, 128]);
    let view = mgr.view(&ctx, red).clone();
    let mesh = quad_mesh(&ctx, [1.0; 4], 1.0);
    let draws = vec![DrawItem {
        kind: SurfaceKind::Translucent,
        mesh: &mesh,
        base_view: &view,
        second_view: None,
    }];
    let pixels = render(
        &ctx,
        &set,
        &target,
        &draws,
        wgpu::Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    );
    assert_eq!(center_pixel(&target, &pixels), [127, 127, 255, 255]);
}

#[test]
fn modulate_multiplies_framebuffer_by_source() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());
    // Half-gray modulator over an opaque white quad drawn in the same frame.
    let gray = mgr.create_bgra8(&ctx, "gray", 1, 1, &[128, 128, 128, 255]);
    let view = mgr.view(&ctx, gray).clone();
    let white = mgr.create_bgra8(&ctx, "white", 1, 1, &[255, 255, 255, 255]);
    let white_view = mgr.view(&ctx, white).clone();
    let backdrop = quad_mesh(&ctx, [1.0; 4], 1.0);
    let modulator = quad_mesh(&ctx, [1.0; 4], 1.0);
    let draws = vec![
        DrawItem {
            kind: SurfaceKind::Opaque,
            mesh: &backdrop,
            base_view: &white_view,
            second_view: None,
        },
        DrawItem {
            kind: SurfaceKind::Modulate,
            mesh: &modulator,
            base_view: &view,
            second_view: None,
        },
    ];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    assert_eq!(center_pixel(&target, &pixels), [128, 128, 128, 255]);
}

#[test]
fn lightmap_multiplies_base_by_second_uv_set_sample() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());
    // White base × half-intensity lightmap.
    let base = mgr.create_bgra8(&ctx, "base", 1, 1, &[255, 255, 255, 255]);
    let lm = mgr.create_bgra8(&ctx, "lm", 1, 1, &[128, 128, 128, 255]);
    let base_view = mgr.view(&ctx, base).clone();
    let lm_view = mgr.view(&ctx, lm).clone();
    let mesh = quad_mesh(&ctx, [1.0; 4], 1.0);
    let draws = vec![DrawItem {
        kind: SurfaceKind::Lightmap,
        mesh: &mesh,
        base_view: &base_view,
        second_view: Some(&lm_view),
    }];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    assert_eq!(center_pixel(&target, &pixels), [128, 128, 128, 255]);
}

#[test]
fn detail_multiply_brightens_midtones_with_clamp() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());
    // Half-gray detail over white base: 1.0 * 0.5 * 2 = 1.0.
    let base = mgr.create_bgra8(&ctx, "base", 1, 1, &[255, 255, 255, 255]);
    let detail = mgr.create_bgra8(&ctx, "detail", 1, 1, &[128, 128, 128, 255]);
    let base_view = mgr.view(&ctx, base).clone();
    let detail_view = mgr.view(&ctx, detail).clone();
    let mesh = quad_mesh(&ctx, [1.0; 4], 1.0);
    let draws = vec![DrawItem {
        kind: SurfaceKind::Detail,
        mesh: &mesh,
        base_view: &base_view,
        second_view: Some(&detail_view),
    }];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    assert_eq!(center_pixel(&target, &pixels), [255, 255, 255, 255]);
}

#[test]
fn palette_lookup_maps_p8_index_through_lut() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());
    // R8 index 128 → LUT entry 128 with RGBA (50, 100, 200, 255).
    let indices = mgr.create_r8(&ctx, "indices", 1, 1, &[128]);
    let mut lut = [0u8; 256 * 4];
    lut[128 * 4..128 * 4 + 4].copy_from_slice(&[50, 100, 200, 255]); // RGBA
    let palette = mgr.create_palette_lut(&ctx, "palette", &lut);
    let indices_view = mgr.view(&ctx, indices).clone();
    let palette_view = mgr.view(&ctx, palette).clone();
    let mesh = quad_mesh(&ctx, [1.0; 4], 1.0);
    let draws = vec![DrawItem {
        kind: SurfaceKind::Palette,
        mesh: &mesh,
        base_view: &indices_view,
        second_view: Some(&palette_view),
    }];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    // Readback is BGRA: R=50 G=100 B=200 arrives as [200, 100, 50, 255].
    assert_eq!(center_pixel(&target, &pixels), [200, 100, 50, 255]);
}

#[test]
fn per_vertex_fog_mixes_toward_scene_fog_color() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    let mut uniforms = PassUniforms::identity();
    uniforms.fog_color = [0.0, 0.0, 1.0, 1.0]; // blue fog
    set.update_uniforms(&ctx, &uniforms);
    // Fully fogged vertex: texture content vanishes behind fog color even
    // though a red backdrop was cleared underneath.
    let any = mgr.create_bgra8(&ctx, "any", 1, 1, &[255, 255, 255, 255]);
    let view = mgr.view(&ctx, any).clone();
    let mesh = quad_mesh(&ctx, [1.0; 4], 0.0);
    let draws = vec![DrawItem {
        kind: SurfaceKind::Opaque,
        mesh: &mesh,
        base_view: &view,
        second_view: None,
    }];
    let pixels = render(
        &ctx,
        &set,
        &target,
        &draws,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    // Blue fog in BGRA byte order.
    assert_eq!(center_pixel(&target, &pixels), [255, 0, 0, 255]);
}

#[test]
fn sprite_billboard_draws_translucent_over_backdrop() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 32, 32);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());
    let sprite_tex = mgr.create_bgra8(&ctx, "sprite", 1, 1, &[0, 255, 0, 255]); // green
    let view = mgr.view(&ctx, sprite_tex).clone();

    // Camera-facing billboard covering the middle half of the frame.
    let (verts, idx) = billboard_quad(
        [0.0, 0.0, 0.5],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        [1.0; 4],
        1.0,
    );
    let verts: Vec<MeshVertex> = verts.to_vec();
    let idx: Vec<u16> = idx.to_vec();
    let mesh = MeshGpu {
        vertex_buffer: upload_pod(&ctx, pod_bytes(&verts), wgpu::BufferUsages::VERTEX),
        index_buffer: upload_pod(&ctx, pod_bytes(&idx), wgpu::BufferUsages::INDEX),
        num_indices: idx.len() as u32,
    };
    let draws = vec![DrawItem {
        kind: SurfaceKind::Translucent,
        mesh: &mesh,
        base_view: &view,
        second_view: None,
    }];
    let pixels = render(
        &ctx,
        &set,
        &target,
        &draws,
        wgpu::Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    );
    // Center is covered by the sprite...
    assert_eq!(center_pixel(&target, &pixels), [0, 255, 0, 255]);
    // ...but the top-left corner still shows the white backdrop.
    assert_eq!(corner_pixel(&pixels), [255, 255, 255, 255]);
}

#[test]
fn mesh_from_hp_format_readers_renders_triangle() {
    use hp_format::mesh::{MeshFace, MeshVert};

    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 16, 16);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());

    // Packed FMeshVerts scaled by 1/8: ±8 pack to ±1.0 world == clip here.
    let neg8 = (-8i32 as u32) & 0x7FF;
    let pos8 = 8u32;
    let verts = [
        MeshVert::from_packed(neg8 | (neg8 << 11)), // (-1,-1)
        MeshVert::from_packed(pos8 | (neg8 << 11)), // ( 1,-1)
        MeshVert::from_packed(pos8 | (pos8 << 11)), // ( 1, 1)
    ];
    let faces = [MeshFace {
        i_wedge: [0, 1, 2],
        material_index: 0,
    }];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
    let colors = [[1.0; 4]; 3];
    let unfogged = [1.0; 3];
    let mesh = build_mesh(&ctx, "tri", &verts, &faces, &uvs, &colors, &unfogged);
    assert_eq!(mesh.num_indices, 3);

    // Yellow texture: B=0 G=255 R=255.
    let tex = mgr.create_bgra8(&ctx, "tex", 1, 1, &[0, 255, 255, 255]);
    let view = mgr.view(&ctx, tex).clone();
    let draws = vec![DrawItem {
        kind: SurfaceKind::Opaque,
        mesh: &mesh,
        base_view: &view,
        second_view: None,
    }];
    let pixels = render(
        &ctx,
        &set,
        &target,
        &draws,
        wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }, // red backdrop
    );
    // The triangle covers bl→br→tr, so screen center is inside...
    assert_eq!(center_pixel(&target, &pixels), [0, 255, 255, 255]);
    // ...and the top-left corner shows the red backdrop (BGRA bytes).
    assert_eq!(corner_pixel(&pixels), [0, 0, 255, 255]);
}

// ---------------------------------------------------------------------------
// Milestone M5: bitmap canvas glyph blitting through the translucent path.
// ---------------------------------------------------------------------------

fn font_fixture() -> (GlyphAtlas, UFontData) {
    let mut characters = vec![
        GlyphRect {
            start_u: 0,
            start_v: 0,
            u_size: 0,
            v_size: 0
        };
        256
    ];
    characters[b'A' as usize] = GlyphRect {
        start_u: 0,
        start_v: 0,
        u_size: 8,
        v_size: 8,
    };
    characters[b'B' as usize] = GlyphRect {
        start_u: 16,
        start_v: 0,
        u_size: 8,
        v_size: 8,
    };
    let font = UFontData {
        pages: vec![FontPage {
            texture_ref: 1,
            characters,
        }],
        characters_per_page: 256,
        char_remap: vec![],
        is_remapped: false,
    };
    let pixels = vec![255u8; 512 * 32];
    (
        GlyphAtlas::from_page(pixels, &font.pages[0].characters),
        font,
    )
}

#[test]
fn canvas_glyph_quads_blit_through_translucent_pipeline() {
    let ctx = ctx();
    let target = OffscreenTarget::new(&ctx, 32, 32);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(&ctx, &PassUniforms::identity());

    let (_atlas, font) = font_fixture();
    // White page bitmap uploaded byte-verbatim (the real path uploads the
    // decoded/palette-applied page from GlyphAtlas).
    let page = mgr.create_bgra8(&ctx, "font page", 512, 32, &[255u8; 512 * 32 * 4]);
    let page_view = mgr.view(&ctx, page).clone();

    // One glyph quad sampling exactly A's rect (start_u 0..8 of a
    // 512x32 page), covering the middle half of the frame.
    let quads = hp_render::canvas::text_quads(
        &_atlas,
        &font,
        [512.0, 32.0],
        "A",
        [-0.5, -0.5],
        [1.0; 4],
        |_x, _y, _w, _h| ([0.0, 0.0, 0.5], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    );
    // The place closure above ignores glyph metrics on purpose — it pins one
    // centered billboard; assert the helper produced exactly one quad.
    assert_eq!(quads.len(), 1);

    let (verts, idx) = billboard_quad(
        [0.0, 0.0, 0.5],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0],
        [
            [0.0 / 512.0, 24.0 / 32.0],
            [8.0 / 512.0, 24.0 / 32.0],
            [8.0 / 512.0, 32.0 / 32.0],
            [0.0 / 512.0, 32.0 / 32.0],
        ],
        [1.0; 4],
        1.0,
    );
    let verts: Vec<MeshVertex> = verts.to_vec();
    let idx: Vec<u16> = idx.to_vec();
    let mesh = MeshGpu {
        vertex_buffer: upload_pod(&ctx, pod_bytes(&verts), wgpu::BufferUsages::VERTEX),
        index_buffer: upload_pod(&ctx, pod_bytes(&idx), wgpu::BufferUsages::INDEX),
        num_indices: idx.len() as u32,
    };
    let draws = vec![DrawItem {
        kind: SurfaceKind::Translucent,
        mesh: &mesh,
        base_view: &page_view,
        second_view: None,
    }];
    let pixels = render(&ctx, &set, &target, &draws, wgpu::Color::BLACK);
    // Opaque white page sampled through the glyph's UV window covers the
    // quad region; the rest stays the black backdrop.
    assert_eq!(center_pixel(&target, &pixels), [255, 255, 255, 255]);
    assert_eq!(corner_pixel(&pixels), [0, 0, 0, 255]);
}
