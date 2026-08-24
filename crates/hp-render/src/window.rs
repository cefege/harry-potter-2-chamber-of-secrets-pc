//! Window swapchain target (interactive mode).
//!
//! The on-window counterpart of [`crate::device::OffscreenTarget`]: a
//! `wgpu::Surface` configured against the same BGRA8Unorm color format and
//! Depth24Plus depth format the pipelines were built for, so
//! [`crate::pipelines::PipelineSet`] instances are reusable unchanged.
//!
//! Frame protocol: [`WindowTarget::begin`] acquires the next swapchain
//! texture (reconfiguring once on `Outdated`, e.g. right after a resize),
//! callers encode into [`WindowFrame::color_view`] /
//! [`WindowFrame::depth_view`], then [`WindowFrame::present`] queues the
//! frame. With the default `Fifo` present mode, present blocks at vertical
//! sync — the loop's natural frame pacing.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::device::{GpuContext, OffscreenTarget};
use crate::error::RenderError;

/// One acquired swapchain frame. Render into its views, then consume it
/// with [`WindowFrame::present`]. Dropping it without presenting discards
/// the frame.
pub struct WindowFrame<'a> {
    texture: wgpu::SurfaceTexture,
    color_view: wgpu::TextureView,
    depth_view: &'a wgpu::TextureView,
}

impl WindowFrame<'_> {
    pub fn color_view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    pub fn depth_view(&self) -> &wgpu::TextureView {
        self.depth_view
    }

    /// Queue the frame for display (vsync-paced under `Fifo`).
    pub fn present(self, queue: &wgpu::Queue) {
        queue.present(self.texture);
    }
}

/// A window-owned swapchain: surface configuration plus a matching depth
/// attachment, resized eagerly with the window.
pub struct WindowTarget {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    depth_view: Option<wgpu::TextureView>,
    width: u32,
    height: u32,
}

impl WindowTarget {
    /// Configure a surface over `window` at `width` x `height`. The color
    /// format is pinned to `Bgra8Unorm` — the format every pipeline set is
    /// built against — so sharing pipelines with the offscreen path is exact.
    pub fn new<W>(
        ctx: &GpuContext,
        window: &Arc<W>,
        width: u32,
        height: u32,
    ) -> Result<WindowTarget, RenderError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        // Arc<W> keeps the native window alive for as long as the surface,
        // so the surface is 'static by construction.
        let surface = ctx
            .instance
            .create_surface(Arc::clone(window))
            .map_err(|error| RenderError::InvalidOperation {
                reason_code: "renderer.surface_unavailable",
                detail: format!("surface creation failed: {error}"),
            })?;
        let capabilities = surface.get_capabilities(&ctx.adapter);
        let format = OffscreenTarget::COLOR_FORMAT;
        if !capabilities.formats.contains(&format) {
            return Err(RenderError::InvalidOperation {
                reason_code: "renderer.surface_format_unavailable",
                detail: format!("surface does not support {format:?}"),
            });
        }
        let alpha_mode =
            *capabilities
                .alpha_modes
                .first()
                .ok_or(RenderError::InvalidOperation {
                    reason_code: "renderer.surface_alpha_unavailable",
                    detail: "surface offers no alpha modes".to_string(),
                })?;
        let mut target = WindowTarget {
            surface,
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: 1,
                height: 1,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode,
                desired_maximum_frame_latency: 2,
                view_formats: Vec::new(),
                color_space: wgpu::SurfaceColorSpace::Auto,
            },
            depth_view: None,
            width: 0,
            height: 0,
        };
        target.resize(ctx, width.max(1), height.max(1));
        Ok(target)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Reconfigure for a new swapchain size (idempotent). A zero-sized
    /// request clamps to 1x1 — surfaces reject zero extents.
    pub fn resize(&mut self, ctx: &GpuContext, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if width == self.width && height == self.height && self.depth_view.is_some() {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.width = width;
        self.height = height;
        self.reconfigure(ctx);
    }

    /// Unconditional surface reconfigure + depth attachment rebuild.
    fn reconfigure(&mut self, ctx: &GpuContext) {
        self.surface.configure(&ctx.device, &self.config);
        let depth = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hp-render window depth"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OffscreenTarget::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.depth_view = Some(depth.create_view(&Default::default()));
    }

    /// Acquire the next swapchain frame. One transparent reconfigure-and-
    /// retry covers `Outdated`/`Lost` (typical right after a resize);
    /// skippable conditions (`Timeout`, `Occluded`, `Validation`) come back
    /// as `Ok(None)` — the caller skips rendering and presents nothing this
    /// frame; anything else fails loud.
    pub fn begin(&mut self, ctx: &GpuContext) -> Result<Option<WindowFrame<'_>>, RenderError> {
        use wgpu::CurrentSurfaceTexture as Current;

        let texture = match self.surface.get_current_texture() {
            Current::Success(texture) | Current::Suboptimal(texture) => Some(texture),
            Current::Outdated | Current::Lost => {
                // Force a full reconfigure (resize() early-returns when the
                // size is unchanged), then retry exactly once.
                self.reconfigure(ctx);
                match self.surface.get_current_texture() {
                    Current::Success(texture) | Current::Suboptimal(texture) => Some(texture),
                    _ => None,
                }
            }
            // Skippable this frame; retry on the next one.
            _ => None,
        };
        let Some(texture) = texture else {
            return Ok(None);
        };
        let color_view = texture.texture.create_view(&Default::default());
        let depth_view = self
            .depth_view
            .as_ref()
            .ok_or(RenderError::InvalidOperation {
                reason_code: "renderer.surface_acquire_failed",
                detail: "window target has no configured depth attachment".to_string(),
            })?;
        Ok(Some(WindowFrame {
            texture,
            color_view,
            depth_view,
        }))
    }
}
