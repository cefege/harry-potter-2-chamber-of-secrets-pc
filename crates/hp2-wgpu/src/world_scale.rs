//! Transactional storage for a scaled world target, separate from the UI target.
//!
//! This module does not switch draw destinations or persist settings. The caller
//! must composite the world before drawing the UI at the original size.

use crate::{PreparedLogicalTargets, RenderLoop};

/// A percentage of the full world viewport. Above 100 the world is supersampled: rendered larger
/// and filtered down by the composite pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldScale(u8);

impl WorldScale {
    pub fn new(percent: u8) -> Result<Self, &'static str> {
        if (1..=200).contains(&percent) { Ok(Self(percent)) } else { Err("world scale must be between 1 and 200 percent") }
    }

    pub fn percent(self) -> u8 { self.0 }

    /// Round to the nearest pixel, retaining at least one pixel per axis.
    pub fn dimensions(self, width: u32, height: u32) -> Result<(u32, u32), &'static str> {
        if width == 0 || height == 0 { return Err("world viewport must not be empty"); }
        let scale = u64::from(self.0);
        let axis = |n: u32| ((u64::from(n) * scale + 50) / 100).max(1) as u32;
        Ok((axis(width), axis(height)))
    }
}

/// Allocated but unpublished targets. Drop this value to cancel a change.
pub struct PreparedWorldScale {
    scale: WorldScale,
    dimensions: (u32, u32),
    targets: PreparedLogicalTargets,
}

/// World-only color and depth attachments. They never replace the UI target.
#[derive(Default)]
pub struct WorldScaleTargets {
    current: Option<PreparedWorldScale>,
}

impl WorldScaleTargets {
    pub fn prepare(
        renderer: &RenderLoop,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        viewport: (u32, u32),
        scale: WorldScale,
    ) -> Result<PreparedWorldScale, String> {
        let dimensions = scale.dimensions(viewport.0, viewport.1).map_err(str::to_owned)?;
        let targets = renderer.prepare_logical_targets(device, format, dimensions.0, dimensions.1)?;
        Ok(PreparedWorldScale { scale, dimensions, targets })
    }

    /// Publish only after allocation and any accompanying settings transaction succeed.
    pub fn commit(&mut self, prepared: PreparedWorldScale) { self.current = Some(prepared); }

    pub fn scale(&self) -> Option<WorldScale> { self.current.as_ref().map(|s| s.scale) }
    pub fn dimensions(&self) -> Option<(u32, u32)> { self.current.as_ref().map(|s| s.dimensions) }
    pub fn color(&self) -> Option<&wgpu::Texture> { self.current.as_ref().map(|s| &s.targets.color.0) }
    pub fn color_view(&self) -> Option<&wgpu::TextureView> { self.current.as_ref().map(|s| &s.targets.color.1) }
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> { self.current.as_ref().map(|s| &s.targets.depth.1) }
}

/// Per-renderer state. The active scope owns the full-resolution attachments
/// while world draws use the smaller attachments.
/// Frame-scope observations, including 100% scopes, for runtime diagnosis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldScaleObservations {
    pub scopes: u64,
    pub prior_color: u64,
    pub scaled: u64,
    /// Scopes where a requested sub-100% scale was blocked.
    pub unavailable: u64,
    /// Scopes where requested MSAA was blocked.
    pub msaa_unavailable: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameAvailability {
    scale_observed: bool,
    msaa_observed: bool,
    scale_unavailable: bool,
    msaa_unavailable: bool,
}

impl FrameAvailability {
    fn record(&mut self, scale_blocked: bool, msaa_blocked: bool) {
        self.scale_observed = true;
        self.msaa_observed = true;
        self.scale_unavailable |= scale_blocked;
        self.msaa_unavailable |= msaa_blocked;
    }
}

pub(crate) struct WorldScaleState {
    percent: u8,
    observations: WorldScaleObservations,
    current_frame: FrameAvailability,
    completed_frame: FrameAvailability,
    cached: Option<PreparedLogicalTargets>,
    active: Option<WorldScope>,
    pipeline: Option<(wgpu::TextureFormat, crate::draw::PostProcessPipeline)>,
    identity: Option<wgpu::Buffer>,
    msaa_setting: i32,
    msaa_cached: Option<crate::msaa::PreparedMsaa>,
    msaa_seed: Option<(wgpu::TextureFormat, u32, crate::draw::PostProcessPipeline)>,
    bloom: bool,
    scene_depth_requested: bool,
    bloom_pipeline: Option<(wgpu::TextureFormat, crate::draw::BloomPipeline)>,
}

impl Default for WorldScaleState {
    fn default() -> Self {
        Self { percent: 100, observations: WorldScaleObservations::default(), current_frame: FrameAvailability::default(), completed_frame: FrameAvailability::default(), cached: None, active: None, pipeline: None, identity: None, msaa_setting: 0, msaa_cached: None, msaa_seed: None, bloom: std::env::var_os("HP2_BLOOM").is_some_and(|v| v == "1"), scene_depth_requested: false, bloom_pipeline: None }
    }
}

impl WorldScaleState {
    pub(crate) fn is_active(&self) -> bool { self.active.is_some() }
    pub(crate) fn begin_frame(&mut self) { self.current_frame = FrameAvailability::default(); }
    pub(crate) fn finish_frame(&mut self) { self.completed_frame = self.current_frame; }
    fn composition_failed(&mut self, error: &str) {
        self.current_frame.record(self.percent < 100, self.msaa_setting != 0);
        eprintln!("WgpuDrv: WARNING: world composition unavailable this frame; using full resolution without MSAA: {error}");
    }
}

/// The active world attachment. Single-sample consumers use `texture_depth_2d`;
/// MSAA consumers use `texture_depth_multisampled_2d` and choose a sample.
/// Depth is not resolved or linearized. The caller must use the matching scene
/// projection and must not sample while the attachment is bound for writing.
pub(crate) struct SceneDepth<'a> {
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) size: (u32, u32),
    pub(crate) samples: u32,
}

struct WorldScope {
    full_view: wgpu::TextureView,
    full_depth: Option<(wgpu::Texture, wgpu::TextureView)>,
    world_color: (wgpu::Texture, wgpu::TextureView),
    world_depth: Option<(wgpu::Texture, wgpu::TextureView)>,
    msaa: Option<crate::msaa::PreparedMsaa>,
}

impl RenderLoop {
    fn prepare_world_targets(&self, created: &crate::CreatedDevice, format: wgpu::TextureFormat, size: (u32, u32)) -> Result<PreparedLogicalTargets, String> {
        let device = &created.device;
        if !self.world_scale_state.scene_depth_requested {
            return self.prepare_logical_targets(device, format, size.0, size.1);
        }
        let limit = device.limits().max_texture_dimension_2d;
        if size.0 == 0 || size.1 == 0 || size.0 > limit || size.1 > limit {
            return Err("logical target dimensions exceed device limits".into());
        }
        let features = created.adapter.get_texture_format_features(crate::draw::DEPTH_FORMAT);
        if !features.allowed_usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING) {
            return Err("sampleable world depth is unsupported".into());
        }
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hp2-wgpu prepared scene color target"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = crate::draw::create_depth_target_with_sampling(device, size.0, size.1, true);
        let poll = device.poll(wgpu::PollType::wait_indefinitely());
        let memory_error = pollster::block_on(memory.pop());
        let internal_error = pollster::block_on(internal.pop());
        let validation_error = pollster::block_on(validation.pop());
        poll.map_err(|error| format!("world depth poll failed: {error}"))?;
        if let Some(error) = memory_error.or(internal_error).or(validation_error) {
            return Err(format!("world depth allocation failed: {error}"));
        }
        Ok(PreparedLogicalTargets { color: (color, view), depth })
    }

    /// Request a sampleable world depth attachment. Disabled by default.
    /// Rejected changes retain the previously published targets and request.
    pub(crate) fn set_scene_depth_requested(&mut self, created: &crate::CreatedDevice, requested: bool) -> Result<(), String> {
        if self.world_scale_state.active.is_some() { return Err("world render scope is active".into()); }
        if self.world_scale_state.scene_depth_requested == requested { return Ok(()); }
        let previous = self.world_scale_state.scene_depth_requested;
        self.world_scale_state.scene_depth_requested = requested;
        if let Err(error) = self.set_world_scale(created, self.world_scale_state.percent) {
            self.world_scale_state.scene_depth_requested = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Flush queued world draws and borrow the active sampleable depth attachment.
    /// This is the depth at this exact point, not necessarily opaque-only depth.
    /// `None` means the world scope was unavailable or depth was not requested.
    pub(crate) fn scene_depth(&mut self, created: &crate::CreatedDevice) -> Option<SceneDepth<'_>> {
        if !self.world_scale_state.scene_depth_requested || self.world_scale_state.active.is_none() { return None; }
        let mut frame = self.in_progress.take()?;
        let batches = std::mem::take(&mut self.batches);
        crate::dispatch_batches(created, self, &mut frame, batches);
        self.in_progress = Some(frame);
        let (texture, view) = self.depth_target.as_ref()?;
        if !texture.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING) { return None; }
        Some(SceneDepth { view, size: (texture.width(), texture.height()), samples: texture.sample_count() })
    }

    /// Stage color and depth attachments before changing the live setting.
    pub fn set_msaa(&mut self, created: &crate::CreatedDevice, setting: i32) -> Result<(), String> {
        let count = crate::msaa::SampleCount::from_setting(setting).map_err(str::to_owned)?;
        if self.world_scale_state.active.is_some() { return Err("world render scope is active".into()); }
        let (size, format) = if let Some(frame) = self.in_progress.as_ref() {
            if count.get() > 1 && !frame.color_sampleable { return Err("scene cannot be sampled for MSAA composition".into()); }
            (frame.size, frame.color_format)
        } else if let Some((texture, _)) = self.scene_target.as_ref() {
            ((texture.width(), texture.height()), texture.format())
        } else { return Err("no scene available to validate MSAA".into()); };
        let size = WorldScale::new(self.world_scale_state.percent).unwrap().dimensions(size.0, size.1).map_err(str::to_owned)?;
        if count.get() > 1 {
            let features = created.adapter.get_texture_format_features(format);
            if !features.allowed_usages.contains(wgpu::TextureUsages::TEXTURE_BINDING)
                || !features.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE) {
                return Err("scene format cannot be sampled for MSAA composition".into());
            }
        }
        let staged = crate::msaa::PreparedMsaa::prepare_with_sampleable_depth(created, format, size, count, self.world_scale_state.scene_depth_requested)?;
        let world = if count.get() > 1 { Some(self.prepare_world_targets(created, format, size)?) } else { None };
        let mut staged_pipelines = None;
        let mut staged_world_pipeline = None;
        let mut staged_identity = None;
        let seed = if count.get() > 1 {
            let validation = created.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let internal = created.device.push_error_scope(wgpu::ErrorFilter::Internal);
            let memory = created.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
            let pipeline = crate::draw::PostProcessPipeline::new_with_samples(&created.device, format, count.get());
            let world_pipeline = crate::draw::PostProcessPipeline::new(&created.device, format);
            use wgpu::util::DeviceExt;
            let uniforms = crate::FrameUniforms { gamma: 1.0, y_scale: 1.0, ..bytemuck::Zeroable::zeroed() };
            let identity = created.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("world scale identity uniforms"), contents: bytemuck::bytes_of(&uniforms), usage: wgpu::BufferUsages::UNIFORM,
            });
            // These are the exact state constructors used by queue_*: stage
            // every reachable blend, mask and depth combination before commit.
            let bits = [0x1, 0x4, 0x40, 0x800000, 0x1000000, 0x80000000];
            let mut tiles = std::collections::HashSet::new();
            let mut surfaces = std::collections::HashSet::new();
            for mask in 0..(1 << bits.len()) {
                let flags = bits.iter().enumerate().fold(0u32, |v, (i, bit)| v | if mask & (1 << i) != 0 { *bit } else { 0 });
                tiles.insert(crate::draw::RenderState::for_poly_flags(flags, true));
                surfaces.insert(crate::draw::RenderState::for_poly_flags(flags, false));
            }
            let lines: std::collections::HashSet<_> = [0, 2].into_iter().map(crate::draw::RenderState::for_line_flags).collect();
            let mut line_pipelines = Vec::new();
            let mut point_pipelines = Vec::new();
            let mut tile_pipelines = Vec::new();
            let mut gouraud_pipelines = Vec::new();
            let mut complex_pipelines = Vec::new();
            for state in lines {
                line_pipelines.push((state, crate::draw::SimpleLinePipeline::new_with_samples(&created.device, format, state, count.get())));
                point_pipelines.push((state, crate::draw::SimpleTrianglePipeline::new_with_samples(&created.device, format, state, count.get())));
            }
            for state in tiles {
                tile_pipelines.push((state, crate::draw::TilePipeline::new_with_samples(&created.device, format, state, count.get())));
            }
            for state in surfaces {
                gouraud_pipelines.push((state, crate::draw::GouraudPipeline::new_with_samples(&created.device, format, state, count.get())));
                complex_pipelines.push((state, crate::draw::ComplexPipeline::new_with_samples(&created.device, format, state, count.get())));
            }
            let staged = (line_pipelines, tile_pipelines, point_pipelines, gouraud_pipelines, complex_pipelines);
            let poll = created.device.poll(wgpu::PollType::wait_indefinitely());
            let memory_error = pollster::block_on(memory.pop());
            let internal_error = pollster::block_on(internal.pop());
            let validation_error = pollster::block_on(validation.pop());
            poll.map_err(|e| format!("MSAA pipeline poll failed: {e}"))?;
            if let Some(e) = memory_error.or(internal_error).or(validation_error) { return Err(format!("MSAA pipeline preparation failed: {e}")); }
            staged_pipelines = Some(staged);
            staged_world_pipeline = Some((format, world_pipeline));
            staged_identity = Some(identity);
            Some((format, count.get(), pipeline))
        } else { None };
        if let Some((lines, tiles, points, gourauds, complexes)) = staged_pipelines {
            for (state, pipeline) in lines { self.line_pipelines.insert((format, state, count.get()), pipeline); }
            for (state, pipeline) in tiles { self.tile_pipelines.insert((format, state, count.get()), pipeline); }
            for (state, pipeline) in points { self.point_pipelines.insert((format, state, count.get()), pipeline); }
            for (state, pipeline) in gourauds { self.gouraud_pipelines.insert((format, state, count.get()), pipeline); }
            for (state, pipeline) in complexes { self.complex_pipelines.insert((format, state, count.get()), pipeline); }
        }
        if let Some(world) = world { self.world_scale_state.cached = Some(world); }
        if let Some(pipeline) = staged_world_pipeline { self.world_scale_state.pipeline = Some(pipeline); }
        if let Some(identity) = staged_identity { self.world_scale_state.identity = Some(identity); }
        if self.world_scale_state.msaa_setting != setting {
            self.world_scale_state.current_frame.msaa_observed = false;
            self.world_scale_state.current_frame.msaa_unavailable = false;
            self.world_scale_state.completed_frame.msaa_observed = false;
            self.world_scale_state.completed_frame.msaa_unavailable = false;
        }
        self.world_scale_state.msaa_setting = setting;
        self.world_scale_state.msaa_cached = Some(staged);
        self.world_scale_state.msaa_seed = seed;
        Ok(())
    }

    pub fn msaa(&self) -> i32 { self.world_scale_state.msaa_setting }
    pub fn msaa_observed(&self) -> bool { self.world_scale_state.completed_frame.msaa_observed }
    pub fn msaa_unavailable(&self) -> bool { self.world_scale_state.completed_frame.msaa_unavailable }

    /// Changes the scale after validating and allocating the new targets.
    /// A rejected change leaves the old value and attachments intact.
    pub fn set_world_scale(&mut self, created: &crate::CreatedDevice, percent: u8) -> Result<(), String> {
        let scale = WorldScale::new(percent).map_err(str::to_owned)?;
        if self.world_scale_state.active.is_some() { return Err("world render scope is active".into()); }
        let staged = if percent == 100 && self.world_scale_state.msaa_setting == 0 && !self.world_scale_state.bloom && !self.world_scale_state.scene_depth_requested { None } else {
            // Settings may be applied between frames. Use the most recent
            // renderer-owned scene in that case, and still validate before
            // publishing the requested value.
            let (size, format) = if let Some(frame) = self.in_progress.as_ref() {
                if !frame.color_sampleable { return Err("scene cannot be sampled for world composition".into()); }
                (frame.size, frame.color_format)
            } else if let Some((texture, _)) = self.scene_target.as_ref() {
                ((texture.width(), texture.height()), texture.format())
            } else {
                return Err("no scene available to validate world scale".into());
            };
            let features = created.adapter.get_texture_format_features(format);
            if !features.allowed_usages.contains(wgpu::TextureUsages::TEXTURE_BINDING)
                || !features.flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE) {
                return Err("scene format cannot be sampled for world composition".into());
            }
            let (w, h) = scale.dimensions(size.0, size.1).map_err(str::to_owned)?;
            Some(self.prepare_world_targets(created, format, (w, h))?)
        };
        let staged_msaa = if self.world_scale_state.msaa_setting != 0 {
            let (size, format) = if let Some(frame) = self.in_progress.as_ref() {
                (frame.size, frame.color_format)
            } else if let Some((texture, _)) = self.scene_target.as_ref() {
                ((texture.width(), texture.height()), texture.format())
            } else { return Err("no scene available to validate world scale".into()); };
            let size = scale.dimensions(size.0, size.1).map_err(str::to_owned)?;
            Some(crate::msaa::PreparedMsaa::prepare_with_sampleable_depth(created, format, size,
                crate::msaa::SampleCount::from_setting(self.world_scale_state.msaa_setting).unwrap(), self.world_scale_state.scene_depth_requested)?)
        } else { None };
        let (pipeline, identity) = if let Some(ref targets) = staged {
            let format = targets.color.0.format();
            let need_pipeline = !matches!(&self.world_scale_state.pipeline, Some((f, _)) if *f == format);
            let need_identity = self.world_scale_state.identity.is_none();
            if need_pipeline || need_identity {
                let device = &created.device;
                let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
                let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
                let pipeline = need_pipeline.then(|| (format, crate::draw::PostProcessPipeline::new(device, format)));
                let identity = if need_identity {
                    use wgpu::util::DeviceExt;
                    let uniforms = crate::FrameUniforms { gamma: 1.0, y_scale: 1.0, ..bytemuck::Zeroable::zeroed() };
                    Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("world scale identity uniforms"), contents: bytemuck::bytes_of(&uniforms), usage: wgpu::BufferUsages::UNIFORM,
                    }))
                } else { None };
                let poll = device.poll(wgpu::PollType::wait_indefinitely());
                let memory_error = pollster::block_on(memory.pop());
                let internal_error = pollster::block_on(internal.pop());
                let validation_error = pollster::block_on(validation.pop());
                poll.map_err(|e| format!("world composition poll failed: {e}"))?;
                if let Some(error) = memory_error.or(internal_error).or(validation_error) { return Err(format!("world composition preparation failed: {error}")); }
                (pipeline, identity)
            } else { (None, None) }
        } else { (None, None) };
        let bloom_pipeline = if self.world_scale_state.bloom {
            let format = staged.as_ref().ok_or("bloom requires world targets")?.color.0.format();
            if !matches!(&self.world_scale_state.bloom_pipeline, Some((f, _)) if *f == format) {
                let device = &created.device;
                let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
                let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
                let pipeline = crate::draw::BloomPipeline::new(device, format);
                let poll = device.poll(wgpu::PollType::wait_indefinitely());
                let memory_error = pollster::block_on(memory.pop());
                let internal_error = pollster::block_on(internal.pop());
                let validation_error = pollster::block_on(validation.pop());
                poll.map_err(|e| format!("bloom pipeline poll failed: {e}"))?;
                if let Some(error) = memory_error.or(internal_error).or(validation_error) {
                    return Err(format!("bloom pipeline preparation failed: {error}"));
                }
                Some((format, pipeline))
            } else { None }
        } else { None };
        if let Some(pipeline) = bloom_pipeline { self.world_scale_state.bloom_pipeline = Some(pipeline); }
        if let Some(pipeline) = pipeline { self.world_scale_state.pipeline = Some(pipeline); }
        if let Some(identity) = identity { self.world_scale_state.identity = Some(identity); }
        if self.world_scale_state.percent != percent {
            self.world_scale_state.current_frame.scale_observed = false;
            self.world_scale_state.current_frame.scale_unavailable = false;
            self.world_scale_state.completed_frame.scale_observed = false;
            self.world_scale_state.completed_frame.scale_unavailable = false;
        }
        self.world_scale_state.percent = percent;
        self.world_scale_state.cached = staged;
        self.world_scale_state.msaa_cached = staged_msaa;
        Ok(())
    }

    pub fn world_scale(&self) -> u8 { self.world_scale_state.percent }

    /// A scale can be preflighted once the first drawable scene exists.
    pub fn has_world_scale_frame(&self) -> bool { self.in_progress.is_some() }

    pub fn world_scale_observations(&self) -> WorldScaleObservations {
        self.world_scale_state.observations
    }

    /// Whether the most recent world scope could not apply a sub-100% scale.
    /// The UI can display this as a temporary unavailable state; the requested
    /// setting is retained and may become usable on the next frame.
    pub fn world_scale_unavailable(&self) -> bool {
        self.world_scale_state.completed_frame.scale_unavailable
    }

    /// Flush prior UI, seed a separate low-resolution target, and switch world
    /// draws and depth operations to that target. The caller must call end even
    /// when world drawing reports an error.
    pub fn begin_scaled_world(&mut self, created: &crate::CreatedDevice) -> Result<bool, String> {
        if self.world_scale_state.active.is_some() { return Err("nested world render scope".into()); }
        // Inspect queued and already-flushed batches without changing their
        // ordering. Track 100% scopes too, so retail traces do not require a
        // settings change to reveal PreRender activity.
        let Some(frame) = self.in_progress.as_ref() else { return Ok(false); };
        let prior = frame.color_drawn || self.batches.iter().any(|batch| batch.state.color_write);
        let requested = self.world_scale_state.percent;
        let scale_unavailable = requested < 100 && prior;
        let msaa_unavailable = self.world_scale_state.msaa_setting != 0 && prior;
        let unavailable = scale_unavailable || msaa_unavailable;
        self.world_scale_state.current_frame.record(scale_unavailable, msaa_unavailable);
        let stats = &mut self.world_scale_state.observations;
        stats.scopes += 1;
        stats.prior_color += u64::from(prior);
        stats.unavailable += u64::from(scale_unavailable);
        stats.msaa_unavailable += u64::from(msaa_unavailable);
        if std::env::var_os("HP2_WORLD_SCALE_TRACE").is_some() && (stats.scopes == 1 || stats.scopes % 120 == 0) {
            // The standalone Rust launcher does not install the FFI log sink.
            // Write the opt-in trace to stderr so replay harnesses can capture it.
            eprintln!("WgpuDrv: world-scale trace scopes={} prior_color={} scaled={} unavailable={} requested={}%", stats.scopes, stats.prior_color, stats.scaled, stats.unavailable, requested);
        }
        if unavailable && !self.world_scale_state.completed_frame.scale_unavailable && !self.world_scale_state.completed_frame.msaa_unavailable {
            eprintln!("WgpuDrv: WARNING: world scale {}% / MSAA {}x unavailable for this frame: earlier color draws require full-resolution composition", requested, self.world_scale_state.msaa_setting);
        }
        if requested == 100 && self.world_scale_state.msaa_setting == 0 && !self.world_scale_state.bloom && !self.world_scale_state.scene_depth_requested { return Ok(false); }
        // A lost or outdated surface can leave the engine issuing draws without
        // an acquired frame. Match the ordinary draw path's no-op behavior.
        let mut frame = self.in_progress.take().unwrap();
        // Preserve the exact ordering and resolution of earlier UI. Until the
        // compositor can retain per-channel transmittance for every blend mode,
        // rendering this world at full resolution is safer than downsampling its
        // existing background and replacing the whole frame on composite.
        let batches = std::mem::take(&mut self.batches);
        crate::dispatch_batches(created, self, &mut frame, batches);
        let has_prior_color = frame.color_drawn;
        self.in_progress = Some(frame);
        if has_prior_color { return Ok(false); }
        let frame = self.in_progress.as_ref().unwrap();
        if !frame.color_sampleable {
            self.world_scale_state.composition_failed("scene cannot be sampled for world composition");
            return Ok(false);
        }
        let size = WorldScale::new(self.world_scale_state.percent).unwrap().dimensions(frame.size.0, frame.size.1).map_err(str::to_owned)?;
        let format = frame.color_format;
        let stale = self.world_scale_state.cached.as_ref().is_none_or(|t| (t.color.0.width(), t.color.0.height()) != size || t.color.0.format() != format);
        let pipeline_stale = !matches!(&self.world_scale_state.pipeline, Some((f, _)) if *f == format) || self.world_scale_state.identity.is_none()
            || (self.world_scale_state.bloom && !matches!(&self.world_scale_state.bloom_pipeline, Some((f, _)) if *f == format));
        if (stale || pipeline_stale) && let Err(error) = self.set_world_scale(created, requested) {
            self.world_scale_state.composition_failed(&error);
            return Ok(false);
        }
        let setting = self.world_scale_state.msaa_setting;
        if setting != 0 {
            let count = crate::msaa::SampleCount::from_setting(setting).unwrap();
            let stale = self.world_scale_state.msaa_cached.as_ref().is_none_or(|m| m.size != size || m.format != format || m.count != count);
            let seed_stale = self.world_scale_state.msaa_seed.as_ref().is_none_or(|(f, n, _)| *f != format || *n != count.get());
            if (stale || seed_stale) && let Err(error) = self.set_msaa(created, setting) {
                self.world_scale_state.composition_failed(&error);
                return Ok(false);
            }
        }
        let mut frame = self.in_progress.take().unwrap();
        let targets = self.world_scale_state.cached.take().unwrap();
        self.world_scale_state.pipeline.as_ref().unwrap().1.record(&created.device, &mut frame.encoder,
            self.world_scale_state.identity.as_ref().unwrap(), &frame.view, &targets.color.1);
        let msaa = if setting == 0 { None } else { self.world_scale_state.msaa_cached.take() };
        let (target_view, depth, world_depth, samples) = if let Some(ref prepared) = msaa {
            let color = &prepared.color.as_ref().unwrap().1;
            self.world_scale_state.msaa_seed.as_ref().unwrap().2.record(&created.device, &mut frame.encoder,
                self.world_scale_state.identity.as_ref().unwrap(), &targets.color.1, color);
            (color.clone(), prepared.depth.as_ref().unwrap().clone(), Some(targets.depth), prepared.count.get())
        } else { (targets.color.1.clone(), targets.depth, None, 1) };
        let full_view = std::mem::replace(&mut frame.view, target_view);
        let full_depth = self.depth_target.replace(depth);
        frame.samples = samples;
        {
            let _clear = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scaled world depth clear"), color_attachments: &[],
                depth_stencil_attachment: Some(crate::depth_clear_attachment(&self.depth_target.as_ref().unwrap().1)),
                ..Default::default()
            });
        }
        self.world_scale_state.observations.scaled += u64::from(requested < 100);
        self.world_scale_state.active = Some(WorldScope { full_view, full_depth, world_color: targets.color, world_depth, msaa });
        self.in_progress = Some(frame);
        Ok(true)
    }

    /// Flush world draws, composite once, and restore the original attachments.
    pub fn end_scaled_world(&mut self, created: &crate::CreatedDevice) -> Result<(), String> {
        let Some(scope) = self.world_scale_state.active.take() else { return Ok(()); };
        let mut frame = self.in_progress.take().ok_or("scaled world frame was lost")?;
        let batches = std::mem::take(&mut self.batches);
        crate::dispatch_batches(created, self, &mut frame, batches);
        if scope.msaa.is_some() {
            let _resolve = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("resolve multisampled world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view, depth_slice: None, resolve_target: Some(&scope.world_color.1),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })], ..Default::default()
            });
        }
        if self.world_scale_state.bloom {
            self.world_scale_state.bloom_pipeline.as_ref().unwrap().1.record(&created.device, &mut frame.encoder,
                &scope.world_color.1, &scope.full_view);
        } else {
            self.world_scale_state.pipeline.as_ref().unwrap().1.record(&created.device, &mut frame.encoder,
                self.world_scale_state.identity.as_ref().unwrap(), &scope.world_color.1, &scope.full_view);
        }
        frame.view = scope.full_view;
        frame.samples = 1;
        let drawn_depth = std::mem::replace(&mut self.depth_target, scope.full_depth).ok_or("scaled world depth was lost")?;
        let world_depth = scope.world_depth.unwrap_or(drawn_depth);
        self.world_scale_state.cached = Some(PreparedLogicalTargets { color: scope.world_color, depth: world_depth });
        self.world_scale_state.msaa_cached = scope.msaa;
        self.in_progress = Some(frame);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloom_shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("shaders/bloom.wgsl")).expect("parse bloom WGSL");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module).expect("validate bloom WGSL");
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_bloom_lifecycle_and_disabled_equivalence() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).unwrap();
        let device = &gpu.device;
        // Pipeline creation under an error scope validates the production WGSL.
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _pipeline = crate::draw::BloomPipeline::new(device, wgpu::TextureFormat::Rgba8Unorm);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(pollster::block_on(validation.pop()).is_none(), "bloom shader validation");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom test geometry"), source: wgpu::ShaderSource::Wgsl(r#"
                @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
                    let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
                    return vec4f(p[i], 0., 1.);
                }
                @fragment fn fs() -> @location(0) vec4f { return vec4f(1., 1., 1., 1.); }
            "#.into()),
        });
        let white = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bloom test geometry"), layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
        });
        let render = |bloom: bool| {
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("bloom lifecycle target"), size: wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
                mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let mut renderer = RenderLoop::new();
            renderer.world_scale_state.bloom = bloom;
            crate::begin_offscreen_frame(&gpu, &mut renderer, &target, wgpu::Color::BLACK);
            assert_eq!(renderer.begin_scaled_world(&gpu).unwrap(), bloom);
            if bloom { assert!(renderer.set_world_scale(&gpu, 50).is_err()); }
            {
                let frame = renderer.in_progress.as_mut().unwrap();
                let _pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("world color"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view, depth_slice: None, resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.2, g: 0.4, b: 0.6, a: 1.0 }), store: wgpu::StoreOp::Store },
                    })], ..Default::default()
                });
            }
            {
                let frame = renderer.in_progress.as_mut().unwrap();
                let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bright world region"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view, depth_slice: None, resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })], ..Default::default()
                });
                pass.set_pipeline(&white);
                pass.set_scissor_rect(6, 6, 4, 4);
                pass.draw(0..3, 0..1);
            }
            renderer.end_scaled_world(&gpu).unwrap();
            assert!(!renderer.world_scale_state.is_active());
            renderer.end_scaled_world(&gpu).unwrap();
            // This UI pixel is drawn after composition and must not acquire a halo.
            {
                let frame = renderer.in_progress.as_mut().unwrap();
                let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("UI after bloom"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view, depth_slice: None, resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })], ..Default::default()
                });
                pass.set_pipeline(&white);
                pass.set_scissor_rect(1, 1, 1, 1);
                pass.draw(0..3, 0..1);
            }
            crate::unlock_frame(&gpu, &mut renderer, false);
            crate::capture::read_texture_rgba8(&gpu, &target).unwrap()
        };
        let disabled = render(false);
        assert_eq!(disabled, render(false), "disabled rendering must remain identical");
        let enabled = render(true);
        let pixel = |p: &[u8], x: usize, y: usize| -> [u8; 4] { p[(y * 16 + x) * 4..(y * 16 + x + 1) * 4].try_into().unwrap() };
        assert_eq!(pixel(&disabled, 5, 7), [51, 102, 153, 255], "disabled world background must retain its color");
        assert_eq!(pixel(&disabled, 7, 7), [255; 4], "disabled world highlight must retain its color");
        assert!(pixel(&enabled, 5, 7)[0] > pixel(&disabled, 5, 7)[0], "bright world region must create a halo");
        assert_eq!(pixel(&enabled, 1, 1), [255; 4]);
        assert_eq!(pixel(&enabled, 2, 1), pixel(&disabled, 2, 1), "UI must not create a halo");
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_bloom_handles_msaa_scale_and_prior_color() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).unwrap();
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bloom scaled target"), size: wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut renderer = RenderLoop::new();
        renderer.world_scale_state.bloom = true;
        crate::begin_offscreen_frame(&gpu, &mut renderer, &target, wgpu::Color::BLACK);
        renderer.set_world_scale(&gpu, 50).unwrap();
        renderer.set_msaa(&gpu, 4).unwrap();
        assert!(renderer.begin_scaled_world(&gpu).unwrap());
        assert_eq!(renderer.in_progress.as_ref().unwrap().samples, 4);
        assert_eq!(renderer.depth_target.as_ref().unwrap().0.width(), 8);
        renderer.end_scaled_world(&gpu).unwrap();
        assert_eq!(renderer.in_progress.as_ref().unwrap().samples, 1);
        assert_eq!(renderer.depth_target.as_ref().unwrap().0.width(), 16);
        renderer.in_progress.as_mut().unwrap().color_drawn = true;
        assert!(!renderer.begin_scaled_world(&gpu).unwrap(), "prior UI must not be postprocessed");
        renderer.end_scaled_world(&gpu).unwrap();
        crate::unlock_frame(&gpu, &mut renderer, false);
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_bloom_recreates_targets_on_resize_and_format_change() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).unwrap();
        let device = &gpu.device;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let target = |width, height, format| device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bloom resize test target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let cases = [
            (16, 12, wgpu::TextureFormat::Rgba8Unorm),
            (10, 6, wgpu::TextureFormat::Rgba8Unorm),
            (10, 6, wgpu::TextureFormat::Bgra8Unorm),
        ];
        let mut renderer = RenderLoop::new();
        renderer.world_scale_state.bloom = true;
        for (index, (width, height, format)) in cases.into_iter().enumerate() {
            let texture = target(width, height, format);
            crate::begin_offscreen_frame(&gpu, &mut renderer, &texture, wgpu::Color::BLACK);
            if index == 0 { renderer.set_world_scale(&gpu, 50).unwrap(); }
            assert!(renderer.begin_scaled_world(&gpu).unwrap());
            let active_depth = &renderer.depth_target.as_ref().unwrap().0;
            assert_eq!((active_depth.width(), active_depth.height()), (width / 2, height / 2));
            renderer.end_scaled_world(&gpu).unwrap();
            let cached = renderer.world_scale_state.cached.as_ref().unwrap();
            assert_eq!((cached.color.0.width(), cached.color.0.height()), (width / 2, height / 2));
            assert_eq!(cached.color.0.format(), format);
            assert_eq!((cached.depth.0.width(), cached.depth.0.height()), (width / 2, height / 2));
            assert_eq!(renderer.world_scale_state.pipeline.as_ref().unwrap().0, format);
            assert_eq!(renderer.world_scale_state.bloom_pipeline.as_ref().unwrap().0, format);
            let restored_depth = &renderer.depth_target.as_ref().unwrap().0;
            assert_eq!((restored_depth.width(), restored_depth.height()), (width, height));
            crate::unlock_frame(&gpu, &mut renderer, false);
        }
        let mut disabled = RenderLoop::new();
        disabled.world_scale_state.bloom = false;
        for (width, height, format) in cases {
            let texture = target(width, height, format);
            crate::begin_offscreen_frame(&gpu, &mut disabled, &texture, wgpu::Color::BLACK);
            assert!(!disabled.begin_scaled_world(&gpu).unwrap());
            disabled.end_scaled_world(&gpu).unwrap();
            assert!(disabled.world_scale_state.cached.is_none());
            assert!(disabled.world_scale_state.pipeline.is_none());
            assert!(disabled.world_scale_state.bloom_pipeline.is_none());
            crate::unlock_frame(&gpu, &mut disabled, false);
        }
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(pollster::block_on(validation.pop()).is_none(), "resize and format changes must validate on the GPU");
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_scene_depth_reads_actual_single_and_supported_msaa_samples() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).unwrap();
        let device = &gpu.device;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let color_features = crate::msaa::effective_format_features(&gpu, format);
        let depth_features = crate::msaa::effective_format_features(&gpu, crate::draw::DEPTH_FORMAT);
        assert!(depth_features.allowed_usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING));
        for samples in [1, 2, 4] {
            if samples > 1 && (!color_features.flags.sample_count_supported(samples)
                || !depth_features.flags.sample_count_supported(samples)
                || !color_features.flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)) {
                if samples == 2 {
                    let target = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("MSAA rejection transaction target"),
                        size: wgpu::Extent3d { width: 16, height: 12, depth_or_array_layers: 1 },
                        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });
                    let mut renderer = RenderLoop::new();
                    renderer.world_scale_state.bloom = false;
                    crate::begin_offscreen_frame(&gpu, &mut renderer, &target, wgpu::Color::BLACK);
                    if color_features.flags.sample_count_supported(4)
                        && depth_features.flags.sample_count_supported(4)
                        && color_features.flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE) {
                        renderer.set_msaa(&gpu, 4).unwrap();
                    }
                    let previous = renderer.msaa();
                    let cached = renderer.world_scale_state.msaa_cached.as_ref().map(|value| (value.count.get(), value.size, value.format));
                    let error = renderer.set_msaa(&gpu, 2).expect_err("unsupported 2x MSAA must be rejected");
                    assert_eq!(error, "2x MSAA is unsupported for the color/depth formats");
                    assert_eq!(renderer.msaa(), previous);
                    assert_eq!(renderer.world_scale_state.msaa_cached.as_ref().map(|value| (value.count.get(), value.size, value.format)), cached);
                    crate::unlock_frame(&gpu, &mut renderer, false);
                    eprintln!("scene depth: unsupported 2x rejected before allocation; published MSAA state unchanged");
                }
                eprintln!("scene depth: {samples}x unsupported by the created device");
                continue;
            }
            let texture = |label| device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label), size: wgpu::Extent3d { width: 16, height: 12, depth_or_array_layers: 1 },
                mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            if samples > 1 {
                let count = crate::msaa::SampleCount::from_setting(samples as i32).unwrap();
                let ordinary = crate::msaa::PreparedMsaa::prepare(&gpu, format, (16, 12), count).unwrap();
                assert!(!ordinary.depth.as_ref().unwrap().0.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING));
            }
            let target = texture("scene depth world target");
            let output = texture("scene depth readback");
            let mut renderer = RenderLoop::new();
            renderer.world_scale_state.bloom = false;
            crate::begin_offscreen_frame(&gpu, &mut renderer, &target, wgpu::Color::BLACK);
            renderer.set_scene_depth_requested(&gpu, true).unwrap();
            if samples > 1 { renderer.set_msaa(&gpu, samples as i32).unwrap(); }
            assert!(renderer.begin_scaled_world(&gpu).unwrap());
            let view = renderer.depth_target.as_ref().unwrap().1.clone();
            {
                let frame = renderer.in_progress.as_mut().unwrap();
                let _clear = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("scene depth known value"), color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.25), store: wgpu::StoreOp::Store }), stencil_ops: None,
                    }), ..Default::default()
                });
            }
            let depth = renderer.scene_depth(&gpu).unwrap();
            assert_eq!((depth.size, depth.samples), ((16, 12), samples));
            let view = depth.view.clone();
            let declaration = if samples == 1 { "texture_depth_2d" } else { "texture_depth_multisampled_2d" };
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("scene depth sampling test"),
                source: wgpu::ShaderSource::Wgsl(format!(r#"
                    @group(0) @binding(0) var depth: {declaration};
                    @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {{
                        let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
                        return vec4f(p[i], 0., 1.);
                    }}
                    @fragment fn fs() -> @location(0) vec4f {{
                        let value = textureLoad(depth, vec2i(0, 0), 0);
                        return vec4f(value, value, value, 1.);
                    }}
                "#).into()),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene depth sampling test"), layout: None,
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
                primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene depth sampling test"), layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
            });
            let output_view = output.create_view(&Default::default());
            {
                let frame = renderer.in_progress.as_mut().unwrap();
                let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("scene depth sampling test"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output_view, depth_slice: None, resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    })], ..Default::default()
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.draw(0..3, 0..1);
            }
            renderer.end_scaled_world(&gpu).unwrap();
            assert!(renderer.scene_depth(&gpu).is_none());
            crate::unlock_frame(&gpu, &mut renderer, false);
            let pixels = crate::capture::read_texture_rgba8(&gpu, &output).unwrap();
            assert!((63..=64).contains(&pixels[0]), "{samples}x depth must read 0.25, got {}", pixels[0]);
            eprintln!("scene depth: {samples}x uniform clear/sample 0 read verified");
        }
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_scene_depth_scope_resize_and_transaction() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).unwrap();
        let target = |width, height, format| gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene depth lifecycle target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2, format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut renderer = RenderLoop::new();
        renderer.world_scale_state.bloom = false;
        for (index, (width, height, format, scale)) in [
            (16, 12, wgpu::TextureFormat::Rgba8Unorm, 50),
            (10, 6, wgpu::TextureFormat::Rgba8Unorm, 100),
            (12, 8, wgpu::TextureFormat::Bgra8Unorm, 200),
        ].into_iter().enumerate() {
            let texture = target(width, height, format);
            crate::begin_offscreen_frame(&gpu, &mut renderer, &texture, wgpu::Color::BLACK);
            if index == 0 {
                assert!(!renderer.depth_target.as_ref().unwrap().0.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING));
                renderer.set_scene_depth_requested(&gpu, true).unwrap();
            }
            renderer.set_world_scale(&gpu, scale).unwrap();
            let before = renderer.world_scale_state.cached.as_ref().unwrap().depth.0.width();
            assert!(renderer.set_world_scale(&gpu, 201).is_err());
            assert!(renderer.prepare_world_targets(&gpu, wgpu::TextureFormat::Bc1RgbaUnorm, (8, 8)).is_err());
            assert_eq!(renderer.world_scale_state.cached.as_ref().unwrap().depth.0.width(), before);
            let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            assert!(pollster::block_on(validation.pop()).is_none(), "failed staging must leave error scopes balanced");
            assert!(renderer.begin_scaled_world(&gpu).unwrap());
            assert!(renderer.set_scene_depth_requested(&gpu, false).is_err());
            let depth = renderer.scene_depth(&gpu).unwrap();
            assert_eq!(depth.size, WorldScale::new(scale).unwrap().dimensions(width, height).unwrap());
            assert_eq!(depth.samples, 1);
            renderer.end_scaled_world(&gpu).unwrap();
            renderer.in_progress.as_mut().unwrap().color_drawn = true;
            assert!(!renderer.begin_scaled_world(&gpu).unwrap(), "prior UI must disable the scope");
            assert!(renderer.scene_depth(&gpu).is_none());
            crate::unlock_frame(&gpu, &mut renderer, false);
        }
    }

    #[test]
    fn frame_availability_aggregates_in_both_orders_and_resets() {
        for order in [[(true, false), (false, true)], [(false, true), (true, false)]] {
            let mut frame = FrameAvailability::default();
            for (scale, msaa) in order { frame.record(scale, msaa); }
            frame.record(false, false);
            assert!(frame.scale_unavailable && frame.msaa_unavailable);
            assert!(frame.scale_observed && frame.msaa_observed);
            let clean = FrameAvailability::default();
            assert!(!clean.scale_observed && !clean.msaa_observed);
        }
    }

    #[test]
    fn dimensions_validate_and_round_without_overflow() {
        assert!(WorldScale::new(0).is_err());
        assert!(WorldScale::new(201).is_err());
        assert_eq!(WorldScale::new(200).unwrap().dimensions(800, 600), Ok((1600, 1200)));
        let half = WorldScale::new(50).unwrap();
        assert_eq!(half.dimensions(1921, 1081), Ok((961, 541)));
        assert_eq!(half.dimensions(1, 1), Ok((1, 1)));
        assert!(half.dimensions(0, 1).is_err());
        assert_eq!(WorldScale::new(100).unwrap().dimensions(u32::MAX, 1), Ok((u32::MAX, 1)));
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_world_scope_restores_full_resolution_for_subsequent_draws() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).unwrap();
        let device = &gpu.device;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("world scale pixel test"), size: wgpu::Extent3d { width: 8, height: 8, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut renderer = RenderLoop::new();
        crate::begin_offscreen_frame(&gpu, &mut renderer, &target, wgpu::Color::BLACK);
        renderer.set_world_scale(&gpu, 50).unwrap();
        assert!(renderer.set_world_scale(&gpu, 0).is_err());
        assert_eq!(renderer.world_scale(), 50);
        assert!(renderer.begin_scaled_world(&gpu).unwrap());
        assert!(renderer.set_world_scale(&gpu, 75).is_err());
        assert_eq!(renderer.world_scale(), 50);
        assert_eq!(renderer.depth_target.as_ref().unwrap().0.width(), 4);
        {
            let frame = renderer.in_progress.as_mut().unwrap();
            let _pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world red"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view, depth_slice: None, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::RED), store: wgpu::StoreOp::Store },
                })], ..Default::default()
            });
        }
        renderer.end_scaled_world(&gpu).unwrap();
        assert_eq!(renderer.depth_target.as_ref().unwrap().0.width(), 8);
        renderer.set_world_scale(&gpu, 75).unwrap();
        assert_eq!(renderer.world_scale(), 75);
        assert!(renderer.begin_scaled_world(&gpu).unwrap());
        assert_eq!(renderer.depth_target.as_ref().unwrap().0.width(), 6);
        renderer.end_scaled_world(&gpu).unwrap();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("one-pixel UI"), source: wgpu::ShaderSource::Wgsl(r#"
                @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
                    let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
                    return vec4f(p[i], 0., 1.);
                }
                @fragment fn fs() -> @location(0) vec4f { return vec4f(0., 1., 0., 1.); }
            "#.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("one-pixel UI"), layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: target.format(), blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
        });
        {
            let frame = renderer.in_progress.as_mut().unwrap();
            let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("full resolution UI"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view, depth_slice: None, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })], ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_scissor_rect(1, 1, 1, 1);
            pass.draw(0..3, 0..1);
        }
        crate::unlock_frame(&gpu, &mut renderer, false);
        let pixels = crate::capture::read_texture_rgba8(&gpu, &target).unwrap();
        let pixel = |x: usize, y: usize| &pixels[(y * 8 + x) * 4..(y * 8 + x + 1) * 4];
        assert_eq!(pixel(1, 1), &[0, 255, 0, 255]);
        assert_eq!(pixel(2, 1), &[255, 0, 0, 255]);
        assert_eq!(pixel(1, 2), &[255, 0, 0, 255]);
        // The production renderer retains its scene between frames.
        renderer.scene_target = Some((target.clone(), target.create_view(&Default::default())));
        renderer.set_world_scale(&gpu, 25).unwrap();
        assert_eq!(renderer.world_scale(), 25);
        assert_eq!(renderer.world_scale_state.cached.as_ref().unwrap().color.0.width(), 2);
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_fullscreen_logical_ui_unchanged_at_half_world_scale() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).unwrap();
        let device = &gpu.device;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fullscreen logical UI test"), source: wgpu::ShaderSource::Wgsl(r#"
                @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
                    let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
                    return vec4f(p[i], 0., 1.);
                }
                @fragment fn fs() -> @location(0) vec4f { return vec4f(0., 1., 0., 1.); }
            "#.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fullscreen logical UI test"), layout: None,
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
        });
        let render = |scale| {
            let mut renderer = RenderLoop::new();
            renderer.set_logical_scene_size(Some((8, 8)));
            crate::begin_capture_only_frame(&gpu, &mut renderer, 16, 8, format, wgpu::Color::BLACK);
            renderer.set_world_scale(&gpu, scale).unwrap();
            assert_eq!(renderer.begin_scaled_world(&gpu).unwrap(), scale < 100);
            assert_eq!(renderer.depth_target.as_ref().unwrap().0.width(), if scale < 100 { 4 } else { 8 });
            {
                let frame = renderer.in_progress.as_mut().unwrap();
                let _pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("world background"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view, depth_slice: None, resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::RED), store: wgpu::StoreOp::Store },
                    })], ..Default::default()
                });
            }
            renderer.end_scaled_world(&gpu).unwrap();
            {
                let frame = renderer.in_progress.as_mut().unwrap();
                let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("one logical UI pixel"), color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view, depth_slice: None, resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })], ..Default::default()
                });
                pass.set_pipeline(&pipeline);
                pass.set_scissor_rect(1, 1, 1, 1);
                pass.draw(0..3, 0..1);
            }
            assert!(crate::unlock_frame(&gpu, &mut renderer, true));
            crate::capture::read_texture_rgba8(&gpu, renderer.capture_target.as_ref().unwrap()).unwrap()
        };
        let full = render(100);
        let half = render(50);
        assert_eq!(full, half, "UI pixel and letterboxed output must not change with world scale");
        let pixel = |x: usize, y: usize| &full[(y * 16 + x) * 4..(y * 16 + x + 1) * 4];
        assert_eq!(pixel(0, 1), &[0, 0, 0, 255]);
        assert_eq!(pixel(5, 1), &[0, 255, 0, 255]);
        assert_eq!(pixel(6, 1), &[255, 0, 0, 255]);
    }

    #[test]
    #[ignore = "requires a real GPU adapter"]
    fn gpu_staging_cancel_and_failure_preserve_current_targets() {
        let gpu = crate::create_device_sync(crate::BackendChoice::Auto, None::<crate::HeadlessSurfaceCreator>).expect("GPU adapter");
        let renderer = RenderLoop::new();
        let mut world = WorldScaleTargets::default();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let prepare = |scale| WorldScaleTargets::prepare(&renderer, &gpu.device, format, (640, 480), WorldScale::new(scale).unwrap());
        world.commit(prepare(100).unwrap());
        assert_eq!(world.dimensions(), Some((640, 480)));
        drop(prepare(50).unwrap());
        assert_eq!(world.dimensions(), Some((640, 480)));
        world.commit(prepare(50).unwrap());
        assert_eq!(world.dimensions(), Some((320, 240)));
        assert_eq!(world.color().unwrap().width(), 320);
        let bad = WorldScaleTargets::prepare(&renderer, &gpu.device, wgpu::TextureFormat::Bc1RgbaUnorm, (640, 480), WorldScale::new(75).unwrap());
        assert!(bad.is_err());
        assert_eq!(world.scale().unwrap().percent(), 50);
        assert_eq!(world.dimensions(), Some((320, 240)));
    }
}
