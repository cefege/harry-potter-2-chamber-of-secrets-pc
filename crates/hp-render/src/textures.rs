//! Counted texture manager (Milestone M4).
//!
//! The smoke harness parses `<HP2_RES> gl_textures_created=N` /
//! `gl_textures_destroyed=M` log lines and fails on imbalance
//! (`resources.leak`). The `gl_` names are kept for frozen-protocol
//! compatibility even though the backend is wgpu.

use crate::device::GpuContext;
use crate::error::ResourceLeakError;
use hp_format::dxt1;
use std::collections::HashMap;

pub type TextureId = u64;

/// Owns every render-relevant texture; creation/destruction are strictly
/// counted so a shutdown-time balance check is possible headlessly.
pub struct TextureManager {
    next_id: TextureId,
    created: u64,
    destroyed: u64,
    live: HashMap<TextureId, wgpu::Texture>,
    /// Sizes kept for diagnostics without touching GPU state.
    sizes: HashMap<TextureId, (u32, u32)>,
    views: HashMap<TextureId, wgpu::TextureView>,
}

impl TextureManager {
    pub fn new() -> TextureManager {
        TextureManager {
            next_id: 1,
            created: 0,
            destroyed: 0,
            live: HashMap::new(),
            sizes: HashMap::new(),
            views: HashMap::new(),
        }
    }

    pub fn created(&self) -> u64 {
        self.created
    }

    pub fn destroyed(&self) -> u64 {
        self.destroyed
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    fn register(&mut self, texture: wgpu::Texture, width: u32, height: u32) -> TextureId {
        let id = self.next_id;
        self.next_id += 1;
        self.created += 1;
        self.live.insert(id, texture);
        self.sizes.insert(id, (width, height));
        id
    }

    /// Sampleable view of one managed texture; created lazily, dropped with
    /// the texture.
    pub fn view(&mut self, _ctx: &GpuContext, id: TextureId) -> &wgpu::TextureView {
        if !self.views.contains_key(&id) {
            let texture = self
                .live
                .get(&id)
                .unwrap_or_else(|| panic!("[renderer.texture_unknown] no texture id {id}"));
            self.views
                .insert(id, texture.create_view(&Default::default()));
        }
        self.views.get(&id).expect("just inserted")
    }

    /// Upload a BGRA8888 surface byte-for-byte (no channel swizzle).
    pub fn create_bgra8(
        &mut self,
        ctx: &GpuContext,
        label: &str,
        width: u32,
        height: u32,
        bgra_pixels: &[u8],
    ) -> TextureId {
        assert_eq!(
            bgra_pixels.len(),
            (width * height * 4) as usize,
            "BGRA8 upload size mismatch"
        );
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        ctx.queue.write_texture(
            texture.as_image_copy(),
            bgra_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.register(texture, width, height)
    }

    /// Single-channel R8 surface (P8 index map before palette lookup).
    pub fn create_r8(
        &mut self,
        ctx: &GpuContext,
        label: &str,
        width: u32,
        height: u32,
        indices: &[u8],
    ) -> TextureId {
        assert_eq!(
            indices.len(),
            (width * height) as usize,
            "R8 upload size mismatch"
        );
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        ctx.queue.write_texture(
            texture.as_image_copy(),
            indices,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.register(texture, width, height)
    }

    /// Decode BC1/DXT1 blocks through `hp_format` and upload as BGRA8.
    ///
    /// Software decode is the deliberate default (plan contingency): it
    /// sidesteps any Metal BC1 sampling quirk at zero visual cost offscreen.
    pub fn create_from_dxt1(
        &mut self,
        ctx: &GpuContext,
        label: &str,
        width: u32,
        height: u32,
        blocks: &[u8],
    ) -> Result<TextureId, dxt1::DxtError> {
        let bgra = dxt1::decode_dxt1_to_bgra(blocks, width, height)?;
        Ok(self.create_bgra8(ctx, label, width, height, &bgra))
    }

    /// 256x1 palette LUT for P8-indexed surfaces.
    pub fn create_palette_lut(
        &mut self,
        ctx: &GpuContext,
        label: &str,
        rgba_palette: &[u8; 256 * 4],
    ) -> TextureId {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 256,
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
            texture.as_image_copy(),
            rgba_palette,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        self.register(texture, 256, 1)
    }

    /// Destroy one texture; destroying an unknown id is loud, not ignored.
    pub fn destroy(&mut self, id: TextureId) {
        if self.live.remove(&id).is_none() {
            panic!("[renderer.texture_double_destroy] destroy of unknown texture id {id}");
        }
        self.views.remove(&id);
        self.destroyed += 1;
        // Dropping the wgpu::Texture here releases the GPU allocation.
    }

    /// Protocol markers in harness order; emit each as its own
    /// `<HP2_RES> …` line.
    pub fn resource_marker_lines(&self) -> [String; 2] {
        [
            format!("<HP2_RES> gl_textures_created={}", self.created),
            format!("<HP2_RES> gl_textures_destroyed={}", self.destroyed),
        ]
    }

    /// Renderer teardown: destroy every live texture. Call before
    /// `assert_balanced` at shutdown.
    pub fn shutdown(&mut self) {
        let ids: Vec<TextureId> = self.live.keys().copied().collect();
        for id in ids {
            self.destroy(id);
        }
    }

    /// Shutdown invariant: everything created was destroyed.
    pub fn assert_balanced(&self) -> Result<(), ResourceLeakError> {
        if self.created == self.destroyed && self.live.is_empty() {
            Ok(())
        } else {
            Err(ResourceLeakError {
                created: self.created,
                destroyed: self.destroyed,
                live: self.live.len(),
            })
        }
    }
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_destroy_counters_balance_at_shutdown() {
        let ctx = GpuContext::headless().expect("headless Metal device");
        let mut mgr = TextureManager::new();
        let a = mgr.create_bgra8(&ctx, "a", 4, 4, &[7u8; 64]);
        let b = mgr
            .create_from_dxt1(&ctx, "b", 4, 4, &dxt1_block_fixture())
            .expect("bc1 decode");
        assert_eq!(mgr.created(), 2);
        assert_eq!(mgr.live_count(), 2);

        // Mid-frame imbalance must be detectable…
        mgr.destroy(a);
        assert!(mgr.assert_balanced().is_err());

        // …and resolve once every creation is destroyed.
        mgr.destroy(b);
        assert_eq!(mgr.destroyed(), 2);
        assert_eq!(
            mgr.resource_marker_lines(),
            [
                "<HP2_RES> gl_textures_created=2".to_string(),
                "<HP2_RES> gl_textures_destroyed=2".to_string()
            ]
        );
        mgr.assert_balanced().expect("no leaks at shutdown");
    }

    #[test]
    #[should_panic(expected = "renderer.texture_double_destroy")]
    fn double_destroy_is_loud() {
        let ctx = GpuContext::headless().expect("headless Metal device");
        let mut mgr = TextureManager::new();
        let id = mgr.create_bgra8(&ctx, "a", 2, 2, &[0u8; 16]);
        mgr.destroy(id);
        mgr.destroy(id);
    }

    /// One 4x4 BC1 block: endpoints 0x0000/0xFFFF, all indices selecting
    /// endpoint interpolation extremes — enough to exercise the decoder.
    fn dxt1_block_fixture() -> Vec<u8> {
        let mut block = [0u8; 8];
        block[0] = 0xFF; // color0 little-endian
        block[1] = 0xFF;
        block[2] = 0x00; // color1
        block[3] = 0x00;
        block[4..8].copy_from_slice(&[0b11100100, 0b11100100, 0b11100100, 0b11100100]);
        block.to_vec()
    }
}
