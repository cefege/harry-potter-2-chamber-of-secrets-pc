//! Offscreen render smoke: exercises the full M3 pipeline set headlessly and
//! captures one frame via the M6 capture hook. Prints the PNG artifact path.
//!
//! Run: cargo run -p hp-render --example offscreen_smoke -- <out-dir>

use hp_render::capture::FrameCapture;
use hp_render::device::{GpuContext, OffscreenTarget};
use hp_render::mesh_gpu::{MeshGpu, MeshVertex, billboard_quad};
use hp_render::pipelines::{DrawItem, PassUniforms, PipelineSet, SurfaceKind, encode_frame};
use hp_render::textures::TextureManager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "out/render-smoke".to_string());

    let ctx = GpuContext::headless()?;
    let width = 256;
    let height = 192;
    let target = OffscreenTarget::new(&ctx, width, height);
    let set = PipelineSet::new(
        &ctx,
        OffscreenTarget::COLOR_FORMAT,
        OffscreenTarget::DEPTH_FORMAT,
    );
    let mut mgr = TextureManager::new();
    set.update_uniforms(
        &ctx,
        &PassUniforms {
            fog_color: [0.05, 0.05, 0.1, 1.0],
            ..PassUniforms::identity()
        },
    );

    // Checkerboard base texture (BGRA), lightmap half-intensity.
    let mut checker = vec![0u8; 64 * 64 * 4];
    for y in 0..64u32 {
        for x in 0..64u32 {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            let px = ((y * 64 + x) * 4) as usize;
            checker[px..px + 4].copy_from_slice(if on {
                &[255, 220, 180, 255]
            } else {
                &[40, 40, 60, 255]
            });
        }
    }
    let base = mgr.create_bgra8(&ctx, "checker", 64, 64, &checker);
    let lm = mgr.create_bgra8(&ctx, "lightmap", 1, 1, &[128, 140, 160, 255]);
    let base_view = mgr.view(&ctx, base).clone();
    let lm_view = mgr.view(&ctx, lm).clone();

    // Full-screen lightmapped ground quad.
    let ground_verts: Vec<MeshVertex> = [
        [-1.0f32, -1.0, 0.5],
        [1.0, -1.0, 0.5],
        [1.0, 1.0, 0.5],
        [-1.0, 1.0, 0.5],
    ]
    .iter()
    .zip([[0.0f32, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]])
    .map(|(p, uv)| MeshVertex {
        position: *p,
        uv0: uv,
        uv1: [0.5, 0.5],
        color: [1.0; 4],
        unfogged: 1.0,
    })
    .collect();
    let indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];
    let ground = MeshGpu {
        vertex_buffer: upload(&ctx, by_bytes(&ground_verts), wgpu::BufferUsages::VERTEX),
        index_buffer: upload(&ctx, by_bytes(&indices), wgpu::BufferUsages::INDEX),
        num_indices: 6,
    };

    // Green sprite billboard centered over the ground.
    let sprite = mgr.create_bgra8(&ctx, "sprite", 1, 1, &[0, 200, 80, 255]);
    let sprite_view = mgr.view(&ctx, sprite).clone();
    let (verts, idx) = billboard_quad(
        [0.2, -0.2, 0.4],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.7, 0.7],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        [1.0; 4],
        1.0,
    );
    let sprite_mesh = MeshGpu {
        vertex_buffer: upload(&ctx, by_bytes(&verts), wgpu::BufferUsages::VERTEX),
        index_buffer: upload(&ctx, by_bytes(&idx), wgpu::BufferUsages::INDEX),
        num_indices: 6,
    };

    // P8-indexed strip through the palette LUT (right edge).
    let indices_tex = mgr.create_r8(&ctx, "p8", 8, 1, &[10, 10, 10, 10, 20, 20, 20, 20]);
    let mut lut = [30u8; 256 * 4];
    lut[10 * 4..10 * 4 + 4].copy_from_slice(&[200, 60, 60, 255]); // RGBA red
    lut[20 * 4..20 * 4 + 4].copy_from_slice(&[60, 60, 220, 255]); // RGBA blue-ish
    let palette = mgr.create_palette_lut(&ctx, "lut", &lut);
    let p8_view = mgr.view(&ctx, indices_tex).clone();
    let lut_view = mgr.view(&ctx, palette).clone();
    let strip_verts: Vec<MeshVertex> = [
        [0.5f32, -0.25, 0.3],
        [0.9, -0.25, 0.3],
        [0.9, 0.25, 0.3],
        [0.5, 0.25, 0.3],
    ]
    .iter()
    .zip([[0.0f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])
    .map(|(p, uv)| MeshVertex {
        position: *p,
        uv0: uv,
        uv1: [0.0; 2],
        color: [1.0; 4],
        unfogged: 1.0,
    })
    .collect();
    let strip_indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];
    let strip = MeshGpu {
        vertex_buffer: upload(&ctx, by_bytes(&strip_verts), wgpu::BufferUsages::VERTEX),
        index_buffer: upload(&ctx, by_bytes(&strip_indices), wgpu::BufferUsages::INDEX),
        num_indices: 6,
    };

    let draws = [
        DrawItem {
            kind: SurfaceKind::Lightmap,
            mesh: &ground,
            base_view: &base_view,
            second_view: Some(&lm_view),
        },
        DrawItem {
            kind: SurfaceKind::Translucent,
            mesh: &sprite_mesh,
            base_view: &sprite_view,
            second_view: None,
        },
        DrawItem {
            kind: SurfaceKind::Palette,
            mesh: &strip,
            base_view: &p8_view,
            second_view: Some(&lut_view),
        },
    ];
    encode_frame(&ctx, &target, &set, &draws, wgpu::Color::BLACK);

    let mut capture = FrameCapture::new(&out_dir)?;
    let path = capture.capture_frame(&ctx, &target, 1.0, 120, "offscreen-smoke")?;
    let meta = capture.finish()?;

    // Teardown: destroy every texture, then emit balanced protocol markers.
    mgr.shutdown();
    for line in mgr.resource_marker_lines() {
        println!("{line}");
    }
    mgr.assert_balanced()?;
    println!("smoke artifact: {}", path.display());
    println!("meta: {}", meta.display());
    Ok(())
}

fn upload(ctx: &GpuContext, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    let padded = bytes.len().div_ceil(4) * 4;
    let mut buf = vec![0u8; padded];
    buf[..bytes.len()].copy_from_slice(bytes);
    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: padded as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue.write_buffer(&buffer, 0, &buf);
    buffer
}

fn by_bytes<T: Copy>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), std::mem::size_of_val(data)) }
}
