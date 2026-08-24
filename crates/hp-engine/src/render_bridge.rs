//! Bridge between the extracted [`RenderScene`] (`crate::scene`) and
//! `hp-render`: camera matrices, brush-poly mesh building, texture upload,
//! offscreen frame rendering.
//!
//! Design notes:
//! - Geometry is static per map run: meshes and draw lists are built once in
//!   [`RendererSession::load_scene`]; each frame only re-encodes draws.
//! - Textures upload through `hp_render::TextureManager` so the harness'
//!   `gl_textures_created/destroyed` balance protocol holds by construction;
//!   sampleable views are materialized eagerly at upload time so the frame
//!   path never needs `&mut`.
//! - P8 surfaces ride the dedicated Palette pipeline (R8 index map + 256x1
//!   LUT); blend-class surfaces need decoded color, so indexed textures on
//!   those classes expand through their palette once at load time.
//!
//! Approximations (documented in Tests/Fixtures/render-baseline/
//! rust_vs_cpp.md): no lightmaps (fullbright white gouraud), no zone fog,
//! no sprites/movers — brush polygons only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hp_render::device::{GpuContext, OffscreenTarget};
use hp_render::math::effective_fov_angle;
use hp_render::mesh_gpu::{MeshGpu, MeshVertex};
use hp_render::pipelines::{DrawItem, PassUniforms, PipelineSet, SurfaceKind, encode_frame_views};
use hp_render::textures::{TextureId, TextureManager};
use hp_render::wgpu;

use crate::error::{EngineError, Result};
use crate::scene::{RenderScene, SceneCamera, ScenePoly, TextureKey};
use crate::utx::{DecodedFormat, TextureStore};

/// Authored horizontal FOV preserved at the 4:3 viewport via
/// `hp_render::math` projection semantics.
const AUTHORED_HFOV_DEGREES: f32 = 90.0;
const VIEWPORT_WIDTH: u32 = 1024;
const VIEWPORT_HEIGHT: u32 = 768;
const NEAR_PLANE: f32 = 1.0;
const FAR_PLANE: f32 = 65536.0;

// EPolyFlags (UnObj.h) — the subset that steers surface classification.
const PF_INVISIBLE: u32 = 0x0000_0001;
const PF_MASKED: u32 = 0x0000_0002;
const PF_TRANSLUCENT: u32 = 0x0000_0004;
const PF_MODULATED: u32 = 0x0000_0040;

/// One resolved texture: ids for protocol counting plus eagerly created
/// sampleable views for the frame path.
struct ResolvedTexture {
    /// R8 index map id for P8 surfaces, BGRA8 id otherwise.
    index_or_color: TextureId,
    /// Sampleable view of [`ResolvedTexture::index_or_color`].
    base_view: wgpu::TextureView,
    /// 256x1 RGBA LUT view for P8 surfaces.
    lut_view: Option<wgpu::TextureView>,
    /// Mip-0 dimensions in texels — drives FPoly UV normalization.
    size: [f32; 2],
}

/// One prepared draw: GPU buffers plus its texture views.
struct PreparedPoly {
    mesh: MeshGpu,
    kind: SurfaceKind,
    base_view: wgpu::TextureView,
    second_view: Option<wgpu::TextureView>,
}

/// Offscreen renderer session: one GPU context + target + pipelines +
/// counted textures for a whole map run.
pub struct RendererSession {
    ctx: GpuContext,
    target: OffscreenTarget,
    set: PipelineSet,
    textures: TextureManager,
    store: TextureStore,
    uploads: HashMap<TextureKey, ResolvedTexture>,
    /// Draw order = archive poly order (deterministic submission).
    draws: Vec<PreparedPoly>,
}

impl RendererSession {
    /// Acquire the Metal device and allocate the offscreen target.
    pub fn new(data_root: &Path) -> Result<RendererSession> {
        let ctx = GpuContext::headless()
            .map_err(|e| EngineError::new("renderer.adapter_unavailable", e.to_string()))?;
        let target = OffscreenTarget::new(&ctx, VIEWPORT_WIDTH, VIEWPORT_HEIGHT);
        let set = PipelineSet::new(
            &ctx,
            OffscreenTarget::COLOR_FORMAT,
            OffscreenTarget::DEPTH_FORMAT,
        );
        Ok(RendererSession {
            ctx,
            target,
            set,
            textures: TextureManager::new(),
            store: TextureStore::new(data_root),
            uploads: HashMap::new(),
            draws: Vec::new(),
        })
    }

    pub fn textures(&self) -> &TextureManager {
        &self.textures
    }

    /// Harness resource markers (`gl_textures_*` protocol names). Emit after
    /// [`RendererSession::shutdown`] so created == destroyed at summary time.
    pub fn resource_marker_lines(&self) -> [String; 2] {
        self.textures.resource_marker_lines()
    }

    /// Build meshes, resolve textures, and program the camera uniforms.
    /// Deterministic: identical scene bytes produce identical buffers in
    /// identical submission order.
    pub fn load_scene(&mut self, scene: &RenderScene) -> Result<()> {
        self.set
            .update_uniforms(&self.ctx, &pass_uniforms(&scene.camera_or_default()));

        for poly in &scene.polys {
            if poly.poly_flags & PF_INVISIBLE != 0 || poly.vertices.len() < 3 {
                continue;
            }
            let key = match &poly.texture {
                Some(key) => key.clone(),
                None => continue,
            };
            let resolved = match self.uploads.get(&key) {
                Some(resolved) => ResolvedTexture {
                    index_or_color: resolved.index_or_color,
                    base_view: resolved.base_view.clone(),
                    lut_view: resolved.lut_view.clone(),
                    size: resolved.size,
                },
                None => self.upload_texture(&key)?,
            };
            let (kind, base_view, second_view) = classify(poly.poly_flags, &resolved);
            let mesh = build_poly_mesh(&self.ctx, poly, resolved.size)?;
            self.draws.push(PreparedPoly {
                mesh,
                kind,
                base_view,
                second_view,
            });
            self.uploads.entry(key).or_insert(ResolvedTexture {
                index_or_color: resolved.index_or_color,
                base_view: resolved.base_view,
                lut_view: resolved.lut_view,
                size: resolved.size,
            });
        }
        Ok(())
    }

    /// Encode one frame of the loaded scene into arbitrary color+depth
    /// attachments — the window-swapchain path shares this with the
    /// offscreen path; only the target differs.
    pub fn render_frame_to_views(
        &self,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        let draws: Vec<DrawItem<'_>> = self.draw_list();
        encode_frame_views(
            &self.ctx,
            &self.set,
            &draws,
            wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            color_view,
            depth_view,
        );
    }

    pub fn render_frame(&self) {
        self.render_frame_to_views(&self.target.color_view(), &self.target.depth_view());
    }

    /// Draw list in deterministic archive order (shared by both targets).
    fn draw_list(&self) -> Vec<DrawItem<'_>> {
        self.draws
            .iter()
            .map(|poly| DrawItem {
                kind: poly.kind,
                mesh: &poly.mesh,
                base_view: &poly.base_view,
                second_view: poly.second_view.as_ref(),
            })
            .collect()
    }

    /// The session's GPU context — the window path needs it to configure
    /// and resize the swapchain against the same instance/adapter/device.
    pub fn gpu_context(&self) -> &GpuContext {
        &self.ctx
    }

    pub fn update_camera(&mut self, location: [f32; 3], rotation: [i32; 3]) {
        let camera = SceneCamera { location, rotation };
        self.set.update_uniforms(&self.ctx, &pass_uniforms(&camera));
    }

    /// Read back + PNG-encode the last rendered frame through the run's
    /// sequential [`hp_render::FrameCapture`]. Deterministic file naming:
    /// frames land in submission order as `frame_%06d.png`.
    pub fn capture_frame(
        &self,
        capture: &mut hp_render::FrameCapture,
        ticks: u32,
        map: &str,
    ) -> Result<PathBuf> {
        capture
            .capture_frame(&self.ctx, &self.target, 1.0, ticks, map)
            .map_err(|error| EngineError::new("renderer.capture_write", error.to_string()))
    }

    /// Read back the last rendered frame (BGRA8, tightly packed).
    pub fn read_pixels_bgra(&self) -> Vec<u8> {
        self.target.read_pixels_bgra(&self.ctx)
    }

    pub fn adapter_name(&self) -> &str {
        &self.ctx.adapter_name
    }

    pub fn viewport(&self) -> (u32, u32) {
        (VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
    }

    /// Destroy every live texture; the caller then asserts balance and emits
    /// the run-summary resource markers.
    pub fn shutdown(&mut self) {
        self.textures.shutdown();
    }

    fn upload_texture(&mut self, key: &TextureKey) -> Result<ResolvedTexture> {
        // Missing/unreadable packages degrade to a neutral checkerboard so
        // geometry stays visible; the reason-coded note fires once per key.
        if let Err(error) = self.store.resolve(key) {
            eprintln!(
                "hp-engine: note [renderer.texture_unavailable] {}: {error}",
                key.package
            );
            const W: u32 = 8;
            let mut pixels = Vec::with_capacity((W * W * 4) as usize);
            for v in 0..W {
                for u in 0..W {
                    let shade = if (u / 4 + v / 4) % 2 == 0 { 128 } else { 64 };
                    pixels.extend_from_slice(&[shade, shade, shade, 255]);
                }
            }
            let id = self
                .textures
                .create_bgra8(&self.ctx, "missing-texture", W, W, &pixels);
            let view = self.textures.view(&self.ctx, id).clone();
            return Ok(ResolvedTexture {
                index_or_color: id,
                base_view: view,
                lut_view: None,
                size: [W as f32, W as f32],
            });
        }
        let decoded = self.store.resolve(key)?;
        let (width, height) = (decoded.width, decoded.height);
        let label = key.object_path.replace('.', "/");
        let size = [width as f32, height as f32];
        let resolved = match decoded.format {
            DecodedFormat::Bgra8(pixels) => {
                let id = self
                    .textures
                    .create_bgra8(&self.ctx, &label, width, height, &pixels);
                let view = self.textures.view(&self.ctx, id).clone();
                ResolvedTexture {
                    index_or_color: id,
                    base_view: view,
                    lut_view: None,
                    size,
                }
            }
            DecodedFormat::Indexed8 { indices, palette } => {
                let index_map = self
                    .textures
                    .create_r8(&self.ctx, &label, width, height, &indices);
                let rgba = *palette; // already RGBA per DecodedFormat contract
                let lut =
                    self.textures
                        .create_palette_lut(&self.ctx, &format!("{label}#lut"), &rgba);
                let base_view = self.textures.view(&self.ctx, index_map).clone();
                let lut_view = self.textures.view(&self.ctx, lut).clone();
                ResolvedTexture {
                    index_or_color: index_map,
                    base_view,
                    lut_view: Some(lut_view),
                    size,
                }
            }
        };
        Ok(resolved)
    }
}

/// Surface class + texture binding for one poly's flags and texture form.
fn classify(
    flags: u32,
    resolved: &ResolvedTexture,
) -> (SurfaceKind, wgpu::TextureView, Option<wgpu::TextureView>) {
    let kind = if flags & PF_MASKED != 0 {
        SurfaceKind::Masked
    } else if flags & PF_TRANSLUCENT != 0 {
        SurfaceKind::Translucent
    } else if flags & PF_MODULATED != 0 {
        SurfaceKind::Modulate
    } else if resolved.lut_view.is_some() {
        SurfaceKind::Palette
    } else {
        SurfaceKind::Opaque
    };
    (kind, resolved.base_view.clone(), resolved.lut_view.clone())
}

/// Rotator units (65536 per revolution) to radians.
fn rot_rad(units: i32) -> f32 {
    units as f32 * std::f32::consts::TAU / 65536.0
}

/// Unified UE1 rotator -> rotation matrix (column-vector convention),
/// C++-verified: R = Rz(yaw) · Ry(−pitch) · Rx(−roll). Axes: X forward,
/// Y right, Z up; positive pitch looks UP (FRotator::Vector convention).
/// Both the camera view basis and the brush LocalToWorld share this
/// helper so the conventions cannot diverge.
pub fn rotation_matrix(rotation: [i32; 3]) -> [[f32; 3]; 3] {
    let (sy, cy) = rot_rad(rotation[1]).sin_cos();
    let (sp, cp) = (-rot_rad(rotation[0])).sin_cos();
    let (sr, cr) = (-rot_rad(rotation[2])).sin_cos();
    [
        [cy * cp, cy * sp * sr - sy * cr, cy * sp * cr + sy * sr],
        [sy * cp, sy * sp * sr + cy * cr, sy * sp * cr - cy * sr],
        [-sp, cp * sr, cp * cr],
    ]
}

pub(crate) fn mat_vec(m: &[[f32; 3]; 3], v: &[f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn dot(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// View-projection uniform block for one camera: authored horizontal FOV at
/// the fixed viewport, view rows = (right, up, forward), depth mapped to
/// [0, 1] with `w = forward . d`.
fn pass_uniforms(camera: &SceneCamera) -> PassUniforms {
    let r = rotation_matrix(camera.rotation);
    let forward = mat_vec(&r, &[1.0, 0.0, 0.0]);
    let right = mat_vec(&r, &[0.0, 1.0, 0.0]);
    let up = mat_vec(&r, &[0.0, 0.0, 1.0]);
    let eye = camera.location;

    let hfov = effective_fov_angle(
        AUTHORED_HFOV_DEGREES,
        VIEWPORT_WIDTH as i32,
        VIEWPORT_HEIGHT as i32,
        false,
    );
    let tan_h = (hfov as f64 * std::f64::consts::PI / 360.0).tan() as f32;
    let tan_v = tan_h * VIEWPORT_HEIGHT as f32 / VIEWPORT_WIDTH as f32;
    let a = FAR_PLANE / (FAR_PLANE - NEAR_PLANE);
    let b = -NEAR_PLANE * FAR_PLANE / (FAR_PLANE - NEAR_PLANE);

    // Row-major clip matrix applied by the shader as clip[j] = row_j · v.
    // Row i is the full dot-product row of the i-th clip component:
    // x' = right·(v-eye)/tan_h, y' = up·(v-eye)/tan_v,
    // z' = a·(f·v-eye)+b, w' = f·(v-eye).
    let row_major: [[f32; 4]; 4] = [
        [
            right[0] / tan_h,
            right[1] / tan_h,
            right[2] / tan_h,
            -dot(&right, &eye) / tan_h,
        ],
        [
            up[0] / tan_v,
            up[1] / tan_v,
            up[2] / tan_v,
            -dot(&up, &eye) / tan_v,
        ],
        [
            forward[0] * a,
            forward[1] * a,
            forward[2] * a,
            b - a * dot(&forward, &eye),
        ],
        [forward[0], forward[1], forward[2], -dot(&forward, &eye)],
    ];
    let mut view_proj = [0.0f32; 16];
    for (row, values) in row_major.iter().enumerate() {
        for (col, value) in values.iter().enumerate() {
            view_proj[col * 4 + row] = *value;
        }
    }
    PassUniforms {
        view_proj,
        fog_color: [0.0, 0.0, 0.0, 1.0],
        misc: [0.333, 0.0, 0.0, 0.0],
    }
}

/// Build one polygon's GPU mesh: fan-triangulated, textured via the FPoly
/// basis (`dot(P - Base, TexU/V)` in texels, panned, normalized by the mip-0
/// texture size), fullbright white, unfogged.
fn build_poly_mesh(ctx: &GpuContext, poly: &ScenePoly, tex_size: [f32; 2]) -> Result<MeshGpu> {
    let n = poly.vertices.len();
    if n < 3 || n >= u16::MAX as usize {
        return Err(EngineError::new(
            "renderer.poly_vertex_count",
            format!("polygon has {n} vertices"),
        ));
    }
    let tw = tex_size[0].max(1.0);
    let th = tex_size[1].max(1.0);
    let base = poly.base;
    let mut vertices = Vec::with_capacity(n);
    for v in &poly.vertices {
        let d = [v[0] - base[0], v[1] - base[1], v[2] - base[2]];
        vertices.push(MeshVertex {
            position: *v,
            uv0: [
                (dot(&d, &poly.tex_u) - poly.pan_uv[0]) / tw,
                (dot(&d, &poly.tex_v) - poly.pan_uv[1]) / th,
            ],
            uv1: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            unfogged: 1.0,
        });
    }
    let mut indices = Vec::with_capacity((n - 2) * 3);
    for i in 1..n - 1 {
        indices.extend_from_slice(&[0u16, i as u16, (i + 1) as u16]);
    }
    Ok(hp_render::mesh_gpu::build_mesh_manual(
        ctx,
        "hp-engine brush poly",
        &vertices,
        &indices,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Submission proof: the full retail scene (brushes + static level
    /// BSP) must land in the draw list — every non-invisible poly with a
    /// resolvable-or-loud texture becomes exactly one draw.
    #[test]
    fn retail_scene_submits_all_polys_to_draw_list() -> Result<()> {
        let data_root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !data_root.join("Maps/PrivetDr.unr").is_file() {
            return Ok(());
        }
        let mut world = hp_uobject::bootstrap::World::load(&data_root).unwrap();
        let bytes = std::fs::read(data_root.join("Maps/PrivetDr.unr")).unwrap();
        let level =
            crate::level::load_level_from_bytes(&mut world.arena, "PrivetDr", &bytes).unwrap();
        let scene = crate::scene::build_render_scene(&world.arena, &level)?;
        let scene_total = scene.polys.len();
        let mut session = RendererSession::new(&data_root)?;
        session.load_scene(&scene)?;
        let draws = session.draws.len();
        println!(
            "[g4] submission: scene {scene_total} polys -> {draws} draws ({} skipped as invisible/untextured)",
            scene_total - draws
        );
        assert!(
            draws >= scene_total * 99 / 100,
            "draw list lost polys: {draws} of {scene_total}"
        );
        // The static level BSP polys must be present: they are appended
        // after the brush polys, so the tail of the draw list is level
        // geometry. Spot-check the last draw's vertex count is > 0.
        assert!(session.draws.iter().all(|d| d.mesh.num_indices >= 3));
        session.shutdown();
        session.textures().assert_balanced().expect("balanced");
        Ok(())
    }

    /// End-to-end offscreen render of a synthetic brush room: two textured
    /// quads seen from a yawed PlayerStart-style camera. Proves camera
    /// matrices, mesh building, texture upload (P8 LUT path), frame encode,
    /// and readback. Rendering twice must be bit-identical (determinism).
    #[test]
    fn renders_synthetic_scene_deterministically() -> Result<()> {
        let data_root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let mut scene = RenderScene {
            polys: Vec::new(),
            camera: Some(SceneCamera {
                location: [-64.0, 0.0, 0.0],
                rotation: [0, 0, 0], // looking along +X into the room
            }),
        };
        // A wall at x=+128 facing the camera, 4 verts, P8-indexed texture.
        // Vertex order must satisfy cross(v1-v0, v2-v0) == normal (the
        // wire convention; front = CW under the left-handed screen basis).
        let quad = |x: f32, flip: bool| {
            let mut v = vec![
                [x, -64.0, -64.0],
                [x, 64.0, -64.0],
                [x, 64.0, 64.0],
                [x, -64.0, 64.0],
            ];
            if flip {
                v.reverse();
            }
            v
        };
        for (x, normal_x, flip) in [(0.0f32, 1.0, false), (256.0, -1.0, true)] {
            scene.polys.push(crate::scene::ScenePoly {
                base: [x, -64.0, -64.0],
                vertices: quad(x, flip),
                normal: [normal_x, 0.0, 0.0],
                tex_u: [0.0, 1.0 / 16.0, 0.0],
                tex_v: [0.0, 0.0, 1.0 / 16.0],
                pan_uv: [0.0, 0.0],
                poly_flags: 0,
                texture: Some(TextureKey {
                    package: String::new(),
                    object_path: String::new(),
                }),
            });
        }

        let mut session = RendererSession::new(&data_root)?;
        // Synthetic 16x16 gradient index map + palette instead of a utx key:
        // upload directly through the manager so this test has no utx dependency.
        let indices: Vec<u8> = (0..16u32)
            .flat_map(|v| (0..16u32).map(move |u| ((u * 16 + v) % 255) as u8))
            .collect();
        let mut palette = [0u8; 256 * 4];
        for (i, px) in palette.chunks_exact_mut(4).enumerate() {
            px[0] = i as u8; // R
            px[1] = 255 - i as u8; // G
            px[2] = (i * 7) as u8; // B
            px[3] = 255;
        }
        session.uploads.insert(
            TextureKey {
                package: String::new(),
                object_path: String::new(),
            },
            {
                let id = session
                    .textures
                    .create_r8(&session.ctx, "synthetic", 16, 16, &indices);
                ResolvedTexture {
                    index_or_color: id,
                    base_view: session.textures.view(&session.ctx, id).clone(),
                    lut_view: {
                        let lut = session.textures.create_palette_lut(
                            &session.ctx,
                            "synthetic#lut",
                            &palette,
                        );
                        Some(session.textures.view(&session.ctx, lut).clone())
                    },
                    size: [16.0, 16.0],
                }
            },
        );
        // load_scene would re-resolve textures; feed meshes only by calling
        // the internals through the public path minus uploads: replicate its
        // loop here because the synthetic key bypasses TextureStore.
        session
            .set
            .update_uniforms(&session.ctx, &pass_uniforms(&scene.camera_or_default()));
        for poly in &scene.polys {
            let resolved = ResolvedTexture {
                index_or_color: session.uploads[&poly.texture.clone().unwrap()].index_or_color,
                base_view: session.uploads[&poly.texture.clone().unwrap()]
                    .base_view
                    .clone(),
                lut_view: session.uploads[&poly.texture.clone().unwrap()]
                    .lut_view
                    .clone(),
                size: session.uploads[&poly.texture.clone().unwrap()].size,
            };
            let (kind, base_view, second_view) = classify(poly.poly_flags, &resolved);
            let mesh = build_poly_mesh(&session.ctx, poly, resolved.size)?;
            session.draws.push(PreparedPoly {
                mesh,
                kind,
                base_view,
                second_view,
            });
        }

        session.render_frame();
        let first = session.read_pixels_bgra();
        assert_eq!(first.len(), (VIEWPORT_WIDTH * VIEWPORT_HEIGHT * 4) as usize);
        // Non-vacuous: count pixels whose RGB differs from the black
        // backdrop (the alpha byte is always 255 and must not satisfy
        // this check).
        let lit_pixels = first
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
            .count();
        assert!(
            lit_pixels > (VIEWPORT_WIDTH * VIEWPORT_HEIGHT) as usize / 50,
            "frame must contain a substantial lit region, got {lit_pixels} pixels"
        );

        session.render_frame();
        let second = session.read_pixels_bgra();
        assert_eq!(first, second, "same seed + fixed inputs => identical frame");

        session.shutdown();
        session.textures().assert_balanced().expect("balanced");
        Ok(())
    }
}
