//! Headless GPU context and offscreen render target (Milestone M1).
//!
//! The renderer runs in three modes driven by `hp2rs` CLI flags:
//! - `--null-render`: no GPU work at all (handled by the caller, not here);
//! - windowed: swapchain owned by the app shell on top of [`GpuContext`];
//! - `--capture-frames` without a window: renders into an
//!   [`OffscreenTarget`] (BGRA8888 color + 24-bit depth) and reads pixels
//!   back for PNG capture — the CI path.

use crate::block_on::block_on;
use crate::error::RenderError;
use wgpu::{
    Backends, Extent3d, Instance, InstanceDescriptor, Texture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureView,
};

/// Shared instance + adapter + device + queue.
pub struct GpuContext {
    pub instance: Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Human-readable adapter name (goes into `frame_meta.json`'s
    /// `renderer` field).
    pub adapter_name: String,
}

impl GpuContext {
    /// Acquire a Metal-backed logical device. Loud
    /// `[renderer.adapter_unavailable]` / `[renderer.device_request_failed]`
    /// errors on failure — never a silent fallback.
    pub fn headless() -> Result<GpuContext, RenderError> {
        Self::with_backends(Backends::METAL)
    }

    /// Acquire a device restricted to the given backends.
    pub fn with_backends(backends: Backends) -> Result<GpuContext, RenderError> {
        let backend_name = if backends.contains(Backends::METAL) {
            "Metal"
        } else if backends.contains(Backends::VULKAN) {
            "Vulkan"
        } else {
            "primary"
        };
        let mut desc = InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        let instance = Instance::new(desc);

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: Default::default(),
        }))
        .map_err(|_| RenderError::AdapterUnavailable {
            backend: backend_name,
        })?;

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("hp-render"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .map_err(|e| RenderError::DeviceRequestFailed {
            detail: e.to_string(),
        })?;

        let adapter_name = adapter.get_info().name;
        Ok(GpuContext {
            instance,
            adapter,
            device,
            queue,
            adapter_name,
        })
    }
}

/// Offscreen BGRA8888 + 24-bit-depth render target usable without a window.
pub struct OffscreenTarget {
    color: Texture,
    depth: Texture,
    pub width: u32,
    pub height: u32,
}

impl OffscreenTarget {
    pub const COLOR_FORMAT: TextureFormat = TextureFormat::Bgra8Unorm;
    pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth24Plus;

    /// Allocate an offscreen surface: BGRA8888 color + 24-bit depth.
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> OffscreenTarget {
        let color = ctx.device.create_texture(&TextureDescriptor {
            label: Some("hp-render offscreen color"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: Self::COLOR_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = ctx.device.create_texture(&TextureDescriptor {
            label: Some("hp-render offscreen depth"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        OffscreenTarget {
            color,
            depth,
            width,
            height,
        }
    }

    pub fn color_view(&self) -> TextureView {
        self.color.create_view(&Default::default())
    }

    pub fn depth_view(&self) -> TextureView {
        self.depth.create_view(&Default::default())
    }

    /// Encode + submit a bare clear-to-color frame and block until done.
    pub fn clear_to(&self, ctx: &GpuContext, clear: [f64; 4]) {
        let mut encoder = ctx.device.create_command_encoder(&Default::default());
        {
            let color_view = self.color_view();
            let depth_view = self.depth_view();
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hp-render clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear[0],
                            g: clear[1],
                            b: clear[2],
                            a: clear[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: Default::default(),
            });
        }
        ctx.queue.submit(Some(encoder.finish()));
        let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Read back tightly packed BGRA8 rows. Row padding for the 256-byte
    /// copy alignment stays internal; the returned buffer does not.
    pub fn read_pixels_bgra(&self, ctx: &GpuContext) -> Vec<u8> {
        debug_assert_eq!(Self::COLOR_FORMAT, TextureFormat::Bgra8Unorm);
        let bytes_per_pixel = 4u32;
        let unpadded = self.width * bytes_per_pixel;
        let bytes_per_row = unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer_size = (bytes_per_row * self.height) as u64;
        let readback = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hp-render readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = ctx.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        ctx.queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel::<Result<(), wgpu::BufferAsyncError>>();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv()
            .expect("readback callback must fire after blocking poll")
            .expect("readback map must succeed");

        let data = slice
            .get_mapped_range()
            .expect("readback buffer must be mapped after blocking poll");
        let mut out = Vec::with_capacity((unpadded * self.height) as usize);
        for row in 0..self.height {
            let start = (row * bytes_per_row) as usize;
            out.extend_from_slice(&data[start..start + unpadded as usize]);
        }
        drop(data);
        readback.unmap();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_device_creates_and_clears_to_expected_pixels() {
        let ctx = GpuContext::headless().expect("headless Metal device");
        let target = OffscreenTarget::new(&ctx, 16, 16);
        // Clear to opaque magenta: B=255, G=0, R=255, A=255 in BGRA order.
        target.clear_to(&ctx, [1.0, 0.0, 1.0, 1.0]);
        let pixels = target.read_pixels_bgra(&ctx);
        assert_eq!(pixels.len(), 16 * 16 * 4);
        for px in pixels.chunks_exact(4) {
            assert_eq!(
                px,
                &[255, 0, 255, 255],
                "cleared pixel must be BGRA magenta"
            );
        }
    }

    #[test]
    fn readback_is_tightly_packed_across_256_byte_alignment() {
        let ctx = GpuContext::headless().expect("headless Metal device");
        // 70 px wide => 280 bytes per row, straddling the copy alignment;
        // padding must not leak into the packed result.
        let target = OffscreenTarget::new(&ctx, 70, 3);
        target.clear_to(&ctx, [0.0, 1.0, 0.0, 1.0]);
        let pixels = target.read_pixels_bgra(&ctx);
        assert_eq!(pixels.len(), 70 * 3 * 4);
        for px in pixels.chunks_exact(4) {
            assert_eq!(px, &[0, 255, 0, 255]);
        }
    }
}
