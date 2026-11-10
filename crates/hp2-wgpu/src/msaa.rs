//! Multisample attachment preparation. Publishing is the caller's transaction.
//!
//! A prepared pair is not connected to a frame until the caller commits it.
//! The resolve destination remains a separate, single-sample texture.

use crate::{CreatedDevice, draw};

/// The persisted UI values are 0 (off), 2 and 4; WGPU uses 1 for off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleCount(u32);

impl SampleCount {
    pub fn from_setting(value: i32) -> Result<Self, &'static str> {
        match value {
            0 => Ok(Self(1)),
            2 | 4 => Ok(Self(value as u32)),
            _ => Err("MSAA must be off, 2x or 4x"),
        }
    }
    pub fn get(self) -> u32 {
        self.0
    }
    pub fn setting(self) -> i32 {
        if self.0 == 1 { 0 } else { self.0 as i32 }
    }
}

/// GPU-owned attachments. `None` means the regular single-sample path.
pub struct PreparedMsaa {
    pub count: SampleCount,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub color: Option<(wgpu::Texture, wgpu::TextureView)>,
    pub depth: Option<(wgpu::Texture, wgpu::TextureView)>,
}

/// The active attachments can only change through an explicit commit.
#[derive(Default)]
pub struct MsaaTargets {
    active: Option<PreparedMsaa>,
}

impl MsaaTargets {
    pub fn active(&self) -> Option<&PreparedMsaa> {
        self.active.as_ref()
    }
    pub fn commit(&mut self, prepared: PreparedMsaa) {
        self.active = Some(prepared);
    }
}

impl PreparedMsaa {
    pub fn prepare(
        created: &CreatedDevice,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        count: SampleCount,
    ) -> Result<Self, String> {
        let device = &created.device;
        let limit = device.limits().max_texture_dimension_2d;
        if size.0 == 0 || size.1 == 0 || size.0 > limit || size.1 > limit {
            return Err("MSAA target dimensions exceed device limits".into());
        }
        if count.get() == 1 {
            return Ok(Self {
                count,
                size,
                format,
                color: None,
                depth: None,
            });
        }
        let color_features = created.adapter.get_texture_format_features(format);
        let depth_features = created
            .adapter
            .get_texture_format_features(draw::DEPTH_FORMAT);
        if !color_features
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            || !depth_features
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            || !color_features.flags.sample_count_supported(count.get())
            || !depth_features.flags.sample_count_supported(count.get())
            || !color_features
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
        {
            return Err(format!(
                "{}x MSAA is unsupported for the color/depth formats",
                count.get()
            ));
        }
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let make = |label, format| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: count.get(),
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            (texture, view)
        };
        let color = make("hp2 MSAA color", format);
        let depth = make("hp2 MSAA depth", draw::DEPTH_FORMAT);
        let poll = device.poll(wgpu::PollType::wait_indefinitely());
        let error = pollster::block_on(memory.pop())
            .or(pollster::block_on(internal.pop()))
            .or(pollster::block_on(validation.pop()));
        poll.map_err(|e| format!("MSAA device poll failed: {e}"))?;
        if let Some(error) = error {
            return Err(format!("MSAA allocation failed: {error}"));
        }
        Ok(Self {
            count,
            size,
            format,
            color: Some(color),
            depth: Some(depth),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setting_values_are_exact() {
        for (setting, count) in [(0, 1), (2, 2), (4, 4)] {
            let value = SampleCount::from_setting(setting).unwrap();
            assert_eq!((value.setting(), value.get()), (setting, count));
        }
        for setting in [-1, 1, 3, 8] {
            assert!(SampleCount::from_setting(setting).is_err());
        }
    }

    #[test]
    fn real_gpu_prepares_and_resolves_and_rejected_change_keeps_previous() {
        let gpu = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("GPU adapter required");
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let count = [4, 2]
            .into_iter()
            .map(|n| SampleCount::from_setting(n).unwrap())
            .find(|&count| PreparedMsaa::prepare(&gpu, format, (16, 16), count).is_ok())
            .expect("GPU must support 2x or 4x MSAA for this test");
        let mut targets = MsaaTargets::default();
        targets.commit(PreparedMsaa::prepare(&gpu, format, (16, 16), count).unwrap());
        let canceled = PreparedMsaa::prepare(&gpu, format, (8, 8), count).unwrap();
        drop(canceled);
        assert!(PreparedMsaa::prepare(&gpu, format, (0, 16), count).is_err());
        assert!(
            PreparedMsaa::prepare(&gpu, wgpu::TextureFormat::Bc1RgbaUnorm, (16, 16), count)
                .is_err()
        );
        let active = targets.active().unwrap();
        assert_eq!(active.size, (16, 16));
        let resolve = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA resolve test"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = resolve.create_view(&Default::default());
        let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MSAA coverage test"), source: wgpu::ShaderSource::Wgsl(r#"
                @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
                    let p = array<vec2f, 3>(vec2f(-0.91, -0.91), vec2f(0.91, -0.91), vec2f(-0.91, 0.83));
                    return vec4f(p[i], 0., 1.);
                }
                @fragment fn fs() -> @location(0) vec4f { return vec4f(1., 0., 0., 1.); }
            "#.into()),
        });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("MSAA coverage test"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: count.get(),
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let _depth = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("MSAA depth clear"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &active.depth.as_ref().unwrap().1,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("MSAA resolve test"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &active.color.as_ref().unwrap().1,
                    depth_slice: None,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        assert!(pollster::block_on(validation.pop()).is_none());
        let pixels = crate::capture::read_texture_rgba8(&gpu, &resolve).unwrap();
        assert!(
            pixels.chunks_exact(4).any(|p| p[0] > 0 && p[0] < 255),
            "resolved triangle must have partially covered pixels"
        );
    }
}
