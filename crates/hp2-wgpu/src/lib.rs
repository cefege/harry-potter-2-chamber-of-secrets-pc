//! Rust port of the renderer, replacing `ThirdParty/XOpenGLDrv`/
//! `ThirdParty/UT99VulkanDrv` with one `wgpu`-backed render device.
//! Phase 3.2 of `local://rust-inplace-port-plan.md`.
//!
//! **Scope of this crate today: adapter/device creation, macOS
//! window-surface creation from a raw `NSView*`, and a real per-tick
//! configure/acquire/clear/present loop (`lock_frame`/`unlock_frame`),
//! all wired into the real `UWgpuRenderDevice::Init`/`Lock`/`Unlock`/
//! `Exit`.** Wired to `hp2-ffi` behind the `renderer-wgpu` feature,
//! consumed by the real `HarryPotter2/Unreal/WgpuDrv` C++ package
//! (`UWgpuRenderDevice : URenderDevice`, registered as
//! `WgpuDrv.WgpuRenderDevice`, gated behind the
//! `HP2_ENABLE_WGPU_DRIVER` CMake option, default OFF).
//!
//! `create_device_sync` shares one `Instance` between surface and
//! adapter creation, as wgpu's `compatible_surface` check requires (an
//! earlier version built two separate, unrelated `Instance`s for the
//! headless device and the standalone surface-creation test, which
//! could never have been wired together correctly -- caught and
//! redesigned before anything downstream depended on it, see
//! `CreatedDevice`/`create_device_sync`'s own doc for the detail), and
//! validates `surface.get_capabilities(&adapter).formats` is
//! non-empty before returning a device at all -- an empty list is
//! wgpu's own documented signal that the surface is incompatible with
//! the adapter, so this is a real failure, not a cosmetic check (an
//! earlier version only checked `surface.is_some()` in one caller's
//! own test, which was vacuous -- caught on review, fixed in the
//! production function itself, not just the test).
//!
//! `lock_frame`/`unlock_frame` prove the stronger claim the capability
//! check alone could not: `surface.configure` plus `get_current_
//! texture`/`Queue::present` actually succeeding on a real window, not
//! just the surface being reported compatible. `Tests/
//! WgpuSurfaceTests.cpp` exercises the real `hp2_rs_wgpu_lock`/
//! `hp2_rs_wgpu_unlock` pair against a real SDL window and passes
//! reliably on this project's own dev machine, confirmed over several
//! repeated runs; getting there needed several precautions
//! (`SDL_HINT_MAC_BACKGROUND_APP`, `SDL_RaiseWindow`,
//! `SDL_WINDOW_ALWAYS_ON_TOP`, and polling the lock call itself for up
//! to ~2 seconds) documented in detail in that file. Which single
//! element of that combination is actually necessary was not isolated
//! (no ablation study was run); a *persistent* `Occluded` outcome
//! after that polling window is treated as an environment gap (ctest
//! `SKIP_RETURN_CODE 77` on that one check) rather than a hard
//! failure, but every run so far in this project's own dev environment
//! has passed for real, not skipped.
//!
//! That same test-harness pass does **not** mean the real game
//! presents frames, and this doc does not claim otherwise: a real
//! `hp2_game -window -testticks=60` launch (`Build/run_wgpu_smoke.py`)
//! on the same machine got `FrameOutcome::Occluded` on all 60 of 60
//! real per-tick `Lock` calls -- zero frames ever actually reached the
//! screen in that run, confirmed via the real cumulative tally
//! `Lock` itself logs every call (`WgpuDrv: Lock outcome=... tally
//! presented=... occluded=...`), not inferred from anything indirect.
//! The most likely difference from the passing test-harness case is
//! that a real `hp2_game -window` launch applies none of the
//! precautions above and makes only one attempt per tick (deliberately
//! -- blocking the real game loop for up to ~2 seconds on every single
//! frame to work around this would be a bad trade even if it helped,
//! since a frame failing to acquire is already an expected, transient
//! condition in real usage that the next tick's own `Lock` call
//! retries for free), but this was not isolated either.
//! `run_wgpu_smoke.py` reports this run as `blocked`, not `pass` --
//! deliberately not suppressed via ctest's `SKIP_RETURN_CODE` either,
//! since that would hide a real, current gap behind a green summary
//! (an earlier version of this change did exactly that to keep a
//! build directory's ctest count looking stable, caught on review as
//! the wrong tradeoff before it was committed).
//!
//! `request_adapter_sync`/`create_device_sync`'s own `#[ignore]`d
//! tests (CI runners have no GPU, see their own doc) cover the
//! headless half at the Rust level directly.
//!
//! Still open, not started: resize handling beyond what `lock_frame`'s
//! own per-call drawable-size query already gets for free, all WGSL
//! shader transcription, real draw-call forwarding for any of
//! `URenderDevice`'s pure virtuals (`lock_frame` presents an empty,
//! cleared frame, nothing drawn yet), and -- the open question this
//! doc's own "zero frames presented in a real launch" paragraph above
//! leaves unresolved -- why a real, non-test-harness launch behaves
//! differently from the dedicated ctest on the same machine.

/// Backend selection, matching the plan's `[WgpuDrv.WgpuRenderDevice]
/// Backend=Auto|Metal|Vulkan|Dx12|Gl` ini key (not yet read from an ini
/// file by this crate -- the ini key's C++ side doesn't exist yet
/// either; this enum is the Rust-side vocabulary that key will
/// eventually map onto).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendChoice {
    Auto,
    Metal,
    Vulkan,
    Dx12,
    Gl,
}

impl BackendChoice {
    fn to_wgpu(self) -> wgpu::Backends {
        match self {
            BackendChoice::Auto => wgpu::Backends::PRIMARY | wgpu::Backends::GL,
            BackendChoice::Metal => wgpu::Backends::METAL,
            BackendChoice::Vulkan => wgpu::Backends::VULKAN,
            BackendChoice::Dx12 => wgpu::Backends::DX12,
            BackendChoice::Gl => wgpu::Backends::GL,
        }
    }
}

/// Human-readable backend name for the `WgpuDrv: adapter "<name>"
/// backend <X>` log line the plan specifies (`Phase 3.2`'s "Device
/// creation" bullet), matching the plan's own `Metal|Vulkan|Dx12|Gl|
/// BrowserWebGpu` vocabulary exactly.
pub fn backend_name(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::Metal => "Metal",
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Dx12 => "Dx12",
        wgpu::Backend::Gl => "Gl",
        wgpu::Backend::BrowserWebGpu => "BrowserWebGpu",
        wgpu::Backend::Noop => "Noop",
    }
}

/// Shared `Instance` construction: the chosen backend set. Used both
/// for headless adapter/device creation (`request_adapter_sync`/
/// `create_device_sync` below) and, on macOS, for window-surface
/// creation (`macos::create_surface_from_ns_view`) -- the same
/// `Instance` must be shared between the two for `request_adapter`'s
/// `compatible_surface` check to mean anything once that integration
/// lands (not yet; see this module's own doc for what's still open).
/// Public so callers needing that integration can construct one
/// instance and pass it to both halves themselves.
pub fn build_instance(choice: BackendChoice) -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: choice.to_wgpu(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

/// Function-pointer shorthand for the "no surface" case when calling
/// `create_device_sync` headlessly -- `None::<HeadlessSurfaceCreator>`
/// instead of spelling out the full closure bound every call site
/// needs (clippy's `type_complexity` lint flagged the inline version).
pub type HeadlessSurfaceCreator =
    fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>;

/// Shared adapter request: `PowerPreference::HighPerformance`.
/// `compatible_surface` must come from the *same* `Instance` passed
/// here -- wgpu's adapter/surface compatibility is instance-scoped,
/// so a surface built from a different `Instance` would make this
/// check meaningless (a real defect an earlier version of this
/// module had: `create_device_sync` built and dropped its own
/// throwaway `Instance` internally, and the standalone surface-
/// creation test function built a second, unrelated one -- neither
/// could ever have been wired together correctly without this
/// rework, caught on review before anything downstream depended on
/// it).
fn request_adapter_raw(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Option<wgpu::Adapter> {
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface,
        ..Default::default()
    }))
    .ok()
}

/// Replaces the adapter-request half of `WgpuDrv`'s future `Init`
/// (plan's "Device creation" bullet). Returns the adapter's name and
/// backend on success, or `None` on total failure (the real C++
/// forwarding function will turn that into the plan's `appErrorf(
/// TEXT("WgpuDrv: no compatible GPU adapter (tried %s)"), ...)` once it
/// exists). Headless (no surface) -- `create_device_sync` is the one
/// that supports an optional surface.
pub fn request_adapter_sync(choice: BackendChoice) -> Option<(String, wgpu::Backend)> {
    let adapter = request_adapter_raw(&build_instance(choice), None)?;
    let info = adapter.get_info();
    Some((info.name, info.backend))
}

/// Result of `create_device_sync`. A plain struct instead of a growing
/// tuple: positional fields were getting hard to read correctly at
/// call sites. Holds the `Instance`/`Adapter` (and `Surface`, if one
/// was requested) alongside the `Device`/`Queue` so the whole group
/// stays alive and drops together -- dropping the `Instance` early
/// while the `Device`/`Surface` are still in use would be a use-
/// after-free at the API boundary wgpu cannot catch for us. Field
/// order is drop order (Rust drops struct fields in declaration
/// order): `surface` first, `instance` last, so the root owner of
/// everything outlives every resource built from it.
pub struct CreatedDevice {
    /// `Some` only if `create_device_sync` was given a
    /// `create_surface` closure that succeeded. Caller-owned window
    /// resources behind the surface (e.g. the `SDL_MetalView` on
    /// macOS) must outlive this field -- drop `CreatedDevice` (or at
    /// least take and drop `surface` specifically) before destroying
    /// them.
    pub surface: Option<wgpu::Surface<'static>>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Kept (not dropped once `request_device` returns -- it takes
    /// `&self`, not `self`) so callers can query real
    /// surface/adapter compatibility themselves, e.g.
    /// `surface.get_capabilities(&adapter)`, rather than only trusting
    /// that `surface.is_some()` means something (it doesn't by
    /// itself: a failed `create_surface` closure already makes
    /// `create_device_sync` return `None` before this struct exists
    /// at all, so `surface.is_some()` alone is tautological once you
    /// have a `CreatedDevice` -- caught on review, see this crate's
    /// own tests for the real check).
    pub adapter: wgpu::Adapter,
    pub adapter_name: String,
    pub backend: wgpu::Backend,
    /// The adapter's own limits, which `required_limits` was set to --
    /// `device.limits()` is guaranteed equal to this (not re-queried
    /// here; returned so callers, including this module's own tests,
    /// can check that guarantee against the real API instead of taking
    /// it on faith).
    pub adapter_limits: wgpu::Limits,
    pub instance: wgpu::Instance,
}

/// Replaces the device/queue-request half of `WgpuDrv`'s future `Init`
/// (plan's "Device creation" bullet). `create_surface`, if given, is
/// called with the freshly-built `Instance` *before* the adapter is
/// requested, and the resulting `Surface` (if `Ok`) is passed as
/// `compatible_surface` to `request_adapter` -- wgpu's own
/// compatibility check is instance-scoped, so sharing one `Instance`
/// between surface and adapter creation (not two separate ones, an
/// earlier version of this function's own mistake, caught on review)
/// is what makes that check mean anything. `None` skips surface
/// creation entirely (headless, today's only real caller via
/// `hp2_rs_wgpu_create` until window-surface wiring into
/// `UWgpuRenderDevice::Init` lands).
///
/// `required_limits: adapter.limits()`, not `Limits::default()`:
/// `request_device` *panics* (not `Err`, confirmed in `wgpu-30.0.1/
/// src/api/adapter.rs`'s own doc comment) if the requested limits
/// exceed what the adapter actually supports, and `Limits::default()`
/// is a fixed WebGPU-spec baseline that a GL/GLES backend -- the
/// plan's own Raspberry Pi fallback -- can fall short of. Asking for
/// exactly `adapter.limits()` can never exceed what the adapter
/// reports, by construction, so this can never hit that panic.
/// Placeholder, not the final policy: `adapter.limits()` means
/// renderer code validated only on a high-end adapter (this machine's
/// M5 reports `max_texture_dimension_2d=32768`) can end up silently
/// depending on capability a lower-end backend doesn't have -- nothing
/// here catches that until it's actually run on one. Once the
/// renderer's real per-feature requirements are known, replace this
/// with a fixed, deliberately-chosen baseline (e.g.
/// `Limits::downlevel_webgl2_defaults()` raised to exactly the texture/
/// buffer sizes HP2 needs, via `.using_resolution(adapter.limits())`)
/// and fail with a clear error when an adapter can't meet it, rather
/// than silently taking whatever the adapter happens to offer. Returns
/// `None` on total failure (same real-C++-forwarding note as
/// `request_adapter_sync`).
pub fn create_device_sync(
    choice: BackendChoice,
    create_surface: Option<
        impl FnOnce(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>,
    >,
) -> Option<CreatedDevice> {
    let instance = build_instance(choice);
    let surface = match create_surface {
        Some(f) => Some(f(&instance).ok()?),
        None => None,
    };
    let adapter = request_adapter_raw(&instance, surface.as_ref())?;
    // A non-null surface that is actually incompatible with the
    // adapter (empty format list -- wgpu-types-30.0.1/src/surface.rs's
    // own doc comment on `SurfaceCapabilities::formats`: "Returns an
    // empty vector if the surface is incompatible with the adapter")
    // is as much a real failure as no adapter at all: a surface that
    // can never be configured is useless to the caller. Checked here,
    // not left for callers to discover later -- an earlier version of
    // this crate only checked `surface.is_some()` in one caller's own
    // test, which was vacuous (caught on review): this validates the
    // actual production path every caller goes through, not just a
    // test-only assertion.
    if let Some(surface) = &surface
        && surface.get_capabilities(&adapter).formats.is_empty()
    {
        return None;
    }
    let info = adapter.get_info();
    let adapter_limits = adapter.limits();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("hp2-wgpu device"),
        required_limits: adapter_limits.clone(),
        ..Default::default()
    }))
    .ok()?;
    Some(CreatedDevice {
        surface,
        device,
        queue,
        adapter,
        adapter_name: info.name,
        backend: info.backend,
        adapter_limits,
        instance,
    })
}

/// Specific result of `lock_frame`, not a bare bool: a bool would
/// hide exactly which step failed and which real
/// `wgpu::CurrentSurfaceTexture` variant was observed -- needed both
/// to diagnose failures (a bare bool hid the real cause of this
/// enum's own predecessor's first failing run, caught on review
/// before a comment that guessed at the cause, wrongly claiming it
/// was "confirmed empirically", ever landed) and so this crate's own
/// callers never have to guess either.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOutcome {
    /// Configured, acquired, and presented a frame.
    Presented = 0,
    /// `created.surface` was `None` (headless device).
    NoSurface = 1,
    /// `Surface::get_default_config` returned `None`: the surface has
    /// zero supported formats or present modes for this adapter.
    NoDefaultConfig = 2,
    /// `get_current_texture` returned `Timeout`.
    Timeout = 3,
    /// `get_current_texture` returned `Occluded` (window minimized or
    /// behind another window -- or, on macOS, possibly never actually
    /// mapped to the screen).
    Occluded = 4,
    /// `get_current_texture` returned `Outdated`.
    Outdated = 5,
    /// `get_current_texture` returned `Lost`.
    Lost = 6,
    /// `get_current_texture` returned `Validation`.
    Validation = 7,
    /// Not returned by `lock_frame` itself -- reserved for its FFI
    /// wrapper (`hp2-ffi`'s `hp2_rs_wgpu_lock`) to report a null
    /// `handle`, without reusing any of the above codes to mean
    /// something they do not.
    InternalError = 8,
    /// Not returned by `lock_frame` itself -- reserved for its FFI
    /// wrapper to pass as `guard`'s `on_panic` value, distinct from
    /// `InternalError`, so a caught panic (`hp2-ffi-support::guard`
    /// logs it via `log::error!`, which a standalone ctest binary may
    /// have no logger installed to show) is still distinguishable
    /// from a plain null-handle call in the returned code itself, not
    /// only in a log that might not be visible.
    Panicked = 9,
}

/// One in-flight frame between `lock_frame` and `unlock_frame`:
/// `surface.get_current_texture()`'s own `SurfaceTexture`, the
/// `TextureView` built from it (`encoder.begin_render_pass` needs a
/// `&TextureView`, not the texture itself), and the `CommandEncoder`
/// the clear render pass was recorded into. `texture`/`view` are both
/// read in `unlock_frame` (`view` only transitively, via the encoder
/// already holding everything it needs -- kept here anyway so this
/// struct, not a borrow of it, is what crosses the `lock_frame`/
/// `unlock_frame` boundary; `SurfaceTexture`'s own `Drop` impl
/// discards it safely if `unlock_frame` is never actually called with
/// `present: true`, matching wgpu's own documented behavior).
struct InProgressFrame {
    texture: wgpu::SurfaceTexture,
    #[allow(dead_code)] // kept alive for texture's sake; never read directly
    view: wgpu::TextureView,
    encoder: wgpu::CommandEncoder,
}

/// Per-live-`CreatedDevice` tally of every `lock_frame` outcome, not
/// just the latest one: a single real/production run can mix
/// `Presented` and `Occluded` calls across many ticks (confirmed on
/// this project's own real `hp2_game -window -testticks=60` run,
/// which got `Occluded` on all 60 of 60 ticks -- see `WgpuDrv.h`'s
/// own doc), so a single last-outcome value would hide that a run
/// consisting entirely of failures still "completed" without ever
/// actually drawing anything. `UWgpuRenderDevice::Exit` logs these
/// counts; `Build/run_wgpu_smoke.py` uses that log line to tell a run
/// that really presented frames apart from one that merely didn't
/// crash while trying to.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCounts {
    pub presented: u32,
    pub occluded: u32,
    pub outdated: u32,
    pub other_failure: u32,
}

/// Per-`CreatedDevice` state that must persist across the
/// `lock_frame`/`unlock_frame` pair (`UWgpuRenderDevice::Lock`/
/// `Unlock`'s future Rust half) and across repeated Lock/Unlock
/// cycles: `configured_size` so every `lock_frame` call does not
/// reconfigure the surface (`Surface::configure`'s own doc: "If the
/// surface is already configured, this will wait for the GPU to come
/// idle before recreating the swapchain" -- real per-frame cost, not
/// just wasted work, if done when the size has not actually changed).
/// One `RenderLoop` per live `CreatedDevice`; `WgpuHandle` (`hp2-ffi`)
/// owns both together.
pub struct RenderLoop {
    configured_size: Option<(u32, u32)>,
    in_progress: Option<InProgressFrame>,
    counts: FrameCounts,
}

impl Default for RenderLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderLoop {
    pub fn new() -> Self {
        Self {
            configured_size: None,
            in_progress: None,
            counts: FrameCounts::default(),
        }
    }

    pub fn counts(&self) -> FrameCounts {
        self.counts
    }
}

/// Replaces the per-frame-begin half of `UWgpuRenderDevice::Lock`
/// (plan's still-open "real draw-call forwarding" item's first piece:
/// nothing is drawn by this function itself, but it is the real
/// per-tick entry point any future draw-call forwarding will record
/// into). Reconfigures the surface only when `width`/`height` differ
/// from the last configured size (`loop_state.configured_size`) --
/// cheap on every other call, including every call at a steady
/// resolution, which is the common case. Begins a render pass that
/// clears to `clear_color` and records nothing else; the pass is
/// ended (not `unlock_frame`'s concern -- `begin_render_pass` returning
/// a pass borrowing `encoder` would make storing both in
/// `InProgressFrame` a self-referential-struct problem) before this
/// function returns, so a draw-call forwarder landing later will need
/// its own render pass, not this clear pass still open.
///
/// `Outdated` additionally clears `loop_state.configured_size`, so the
/// *next* `lock_frame` call reconfigures rather than repeating the
/// same stale configuration -- `Surface::get_current_texture`'s own
/// doc says exactly that ("Call Surface::configure() and try again")
/// for this code.
///
/// Fails (and leaves `loop_state.in_progress` untouched, i.e. `None`
/// or whatever a previous successful `lock_frame` left it as) on a
/// non-`None` surface whose format/present-mode list is empty, or on
/// `get_current_texture` returning anything other than `Success`/
/// `Suboptimal` -- see `FrameOutcome`'s own doc for what each code
/// means. Every outcome is tallied into `loop_state.counts()`.
pub fn lock_frame(
    created: &CreatedDevice,
    loop_state: &mut RenderLoop,
    width: u32,
    height: u32,
    clear_color: wgpu::Color,
) -> FrameOutcome {
    let Some(surface) = created.surface.as_ref() else {
        loop_state.counts.other_failure += 1;
        return FrameOutcome::NoSurface;
    };
    if loop_state.configured_size != Some((width, height)) {
        let Some(config) = surface.get_default_config(&created.adapter, width, height) else {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::NoDefaultConfig;
        };
        surface.configure(&created.device, &config);
        loop_state.configured_size = Some((width, height));
    }
    let texture = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(texture) => texture,
        wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        wgpu::CurrentSurfaceTexture::Timeout => {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::Timeout;
        }
        wgpu::CurrentSurfaceTexture::Occluded => {
            loop_state.counts.occluded += 1;
            return FrameOutcome::Occluded;
        }
        wgpu::CurrentSurfaceTexture::Outdated => {
            loop_state.configured_size = None;
            loop_state.counts.outdated += 1;
            return FrameOutcome::Outdated;
        }
        wgpu::CurrentSurfaceTexture::Lost => {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::Lost;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            loop_state.counts.other_failure += 1;
            return FrameOutcome::Validation;
        }
    };
    let view = texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = created
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("hp2-wgpu frame encoder"),
        });
    {
        let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hp2-wgpu clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    loop_state.in_progress = Some(InProgressFrame {
        texture,
        view,
        encoder,
    });
    loop_state.counts.presented += 1;
    FrameOutcome::Presented
}

/// Replaces the per-frame-end half of `UWgpuRenderDevice::Unlock`.
/// `present`, matching the real `Unlock(UBOOL Blit)`'s own parameter
/// (`ThirdParty/XOpenGLDrv/Src/XOpenGL.cpp`'s own `Unlock` only blits
/// when `Blit` is set -- used for render passes that should not
/// actually reach the screen, e.g. hit-testing passes): `true` submits
/// and presents the frame `lock_frame` began; `false` submits (so the
/// GPU work still completes, matching XOpenGL's own real behavior of
/// still finishing the frame) but never calls `queue.present`, letting
/// `SurfaceTexture::drop` discard it instead (wgpu's own documented,
/// safe behavior for an un-presented texture).
///
/// Returns `false` if `loop_state.in_progress` is `None` -- `unlock_
/// frame` called without a matching successful `lock_frame` first is a
/// caller bug, not a real runtime condition to report via
/// `FrameOutcome`.
pub fn unlock_frame(created: &CreatedDevice, loop_state: &mut RenderLoop, present: bool) -> bool {
    let Some(frame) = loop_state.in_progress.take() else {
        return false;
    };
    created
        .queue
        .submit(std::iter::once(frame.encoder.finish()));
    if present {
        created.queue.present(frame.texture);
    }
    true
}

/// Window-surface creation from a C++-owned `SDL_Window*`'s raw view
/// pointer. Only macOS today -- the plan's "Approved deviations" SDL2
/// entry's reasoning applies here too: no `sdl2` crate, so no
/// `raw-window-handle` feature on a crate dependency either; C++ gets
/// the raw `NSView*` itself via `SDL_Metal_CreateView`/
/// `SDL_Metal_GetLayer` (SDL functions, not an `sdl2` crate call) and
/// passes the pointer across FFI, same pattern as every other
/// SDL-touching slice this session.
#[cfg(target_os = "macos")]
pub mod macos {
    use std::ffi::c_void;
    use std::ptr::NonNull;

    /// Replaces the window-surface half of `WgpuDrv`'s future `Init`
    /// (plan's "Window surface" bullet). `ns_view` is the pointer
    /// `SDL_Metal_CreateView` returns (itself "can be cast directly to
    /// an NSView", per `SDL_metal.h`'s own doc comment) -- C++ calls
    /// `SDL_Metal_CreateView`, not this function, since it owns the
    /// `SDL_Window*` the view is attached to; this function only turns
    /// the resulting raw pointer into a `wgpu::Surface`.
    ///
    /// # Safety
    /// `ns_view` must be a valid, non-null pointer to an `NSView`
    /// (typically a `CAMetalLayer`-backed one from
    /// `SDL_Metal_CreateView`) that outlives the returned `Surface`.
    /// Must be called from the application's main thread: AppKit views
    /// are only safely accessed from there (see
    /// `raw_window_handle::AppKitWindowHandle`'s own doc comment).
    pub unsafe fn create_surface_from_ns_view(
        instance: &wgpu::Instance,
        ns_view: *mut c_void,
    ) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError> {
        let ns_view = NonNull::new(ns_view).expect("ns_view must not be null");
        let raw_window_handle = raw_window_handle::RawWindowHandle::AppKit(
            raw_window_handle::AppKitWindowHandle::new(ns_view),
        );
        let raw_display_handle = raw_window_handle::RawDisplayHandle::AppKit(
            raw_window_handle::AppKitDisplayHandle::new(),
        );
        unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle),
                raw_window_handle,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Needs a real GPU adapter. CI runners (ci.yml) have none. Run with
    // `cargo test -p hp2-wgpu -- --ignored`. Don't make it pass when no
    // adapter is found: that would hide real failures on machines that
    // do have one.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn request_adapter_sync_finds_a_real_adapter_on_this_machine() {
        let result = request_adapter_sync(BackendChoice::Auto);
        assert!(
            result.is_some(),
            "expected a real GPU adapter to be available"
        );
        let (name, backend) = result.unwrap();
        assert!(!name.is_empty(), "adapter name should not be empty");
        println!("found adapter {name:?} backend {}", backend_name(backend));
    }

    // Same #[ignore] reasoning as above. Proves genuine device
    // communication, not just that a handle type-checks: device.limits()
    // is asserted equal to the adapter's own limits (not merely nonzero
    // -- a nonzero check alone would pass for the fixed Limits::default()
    // baseline too, proving nothing hardware-specific), and a real (if
    // empty) command buffer is submitted and polled for actual
    // completion.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn create_device_sync_produces_a_working_device() {
        let result = create_device_sync(BackendChoice::Auto, None::<HeadlessSurfaceCreator>);
        assert!(result.is_some(), "expected a real device to be created");
        let created = result.unwrap();
        assert!(!created.adapter_name.is_empty());
        assert_eq!(
            created.device.limits(),
            created.adapter_limits,
            "the device must report exactly the limits it was created with"
        );
        let encoder = created
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        created.queue.submit(std::iter::once(encoder.finish()));
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        println!(
            "created device on adapter {:?} backend {}, max_texture_dimension_2d={}",
            created.adapter_name,
            backend_name(created.backend),
            created.adapter_limits.max_texture_dimension_2d
        );
    }

    #[test]
    fn backend_name_covers_every_plan_named_backend() {
        assert_eq!(backend_name(wgpu::Backend::Metal), "Metal");
        assert_eq!(backend_name(wgpu::Backend::Vulkan), "Vulkan");
        assert_eq!(backend_name(wgpu::Backend::Dx12), "Dx12");
        assert_eq!(backend_name(wgpu::Backend::Gl), "Gl");
        assert_eq!(backend_name(wgpu::Backend::BrowserWebGpu), "BrowserWebGpu");
    }
}
