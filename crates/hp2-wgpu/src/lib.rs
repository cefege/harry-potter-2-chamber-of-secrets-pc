//! Rust port of the renderer, replacing `ThirdParty/XOpenGLDrv`/
//! `ThirdParty/UT99VulkanDrv` with one `wgpu`-backed render device.
//! Phase 3.2 of `local://rust-inplace-port-plan.md`.
//!
//! **Scope of this crate today: adapter/device creation, plus macOS
//! window-surface creation from a raw `NSView*`, wired into the real
//! `UWgpuRenderDevice::Init`.** Wired to `hp2-ffi` behind the
//! `renderer-wgpu` feature, consumed by the real `HarryPotter2/Unreal/
//! WgpuDrv` C++ package (`UWgpuRenderDevice : URenderDevice`,
//! registered as `WgpuDrv.WgpuRenderDevice`, gated behind the
//! `HP2_ENABLE_WGPU_DRIVER` CMake option, default OFF). Verified end
//! to end on real hardware, not assumed: `Init` calls
//! `SDL_Metal_CreateView` on the real game window
//! (`Viewport->GetWindow()`) and passes the resulting `NSView*` to
//! `hp2_rs_wgpu_create`, which shares one `wgpu::Instance` between
//! surface and adapter/device creation; `Exit` tears both down in the
//! safe order (device/surface before the `SDL_MetalView`). A real
//! `hp2_game -window` process launch with `[Engine.Engine]
//! GameRenderDevice=WgpuDrv.WgpuRenderDevice` logs `WgpuDrv: adapter
//! "Apple M5" backend Metal` with no "continuing headless" fallback
//! line (`Build/run_wgpu_smoke.py`, this machine's real GPU, confirmed
//! present via `system_profiler SPDisplaysDataType` before this crate
//! was started) -- the absence of that fallback line is itself the
//! evidence the real surface path, not the headless one, ran.
//! `request_adapter_sync`/`create_device_sync`'s own `#[ignore]`d
//! tests (CI runners have no GPU, see their own doc) cover the
//! headless half at the Rust level directly.
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
//! Still open, not started: resize handling, all WGSL shader
//! transcription, real draw-call forwarding for any of
//! `URenderDevice`'s pure virtuals, and a stronger surface-working
//! proof than "a `Surface` value was constructed and is compatible
//! with the adapter" -- the plan's frame capture and editor hit
//! testing will eventually need `surface.configure(&device, ...)` plus
//! `get_current_texture()` to actually succeed and present, a
//! reasonable next test to add but not required to prove either
//! existing slice's own narrower claim.

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
