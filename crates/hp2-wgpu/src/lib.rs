//! Rust port of the renderer, replacing `ThirdParty/XOpenGLDrv`/
//! `ThirdParty/UT99VulkanDrv` with one `wgpu`-backed render device.
//! Phase 3.2 of `local://rust-inplace-port-plan.md`.
//!
//! **Scope of this crate today: headless adapter and device/queue
//! creation**, with no window surface, no `WgpuDrv` C++ package yet,
//! and nothing wired to `hp2-ffi`. This is deliberately the smallest
//! verifiable slice: it answers "does `wgpu` actually produce a
//! working adapter and device on the real hardware this project
//! targets" before any of the surrounding C++ package scaffolding
//! (`UWgpuRenderDevice : URenderDevice`, the `IMPLEMENT_PACKAGE(WgpuDrv)`
//! boilerplate, `AUTO_INITIALIZE_REGISTRANTS_WGPUDRV`, the full
//! `hp2_rs_wgpu_<method>` virtual-forwarding surface) gets built around
//! it. Verified on real hardware, not assumed: this machine has a real
//! Apple M5 GPU (Metal 4, confirmed via `system_profiler
//! SPDisplaysDataType` before this crate was started), so
//! `request_adapter_sync`/`create_device_sync`'s own tests confirm a
//! genuine adapter and a genuine working device from the real Metal
//! backend, not a software/CI fallback -- but both tests are
//! `#[ignore]`d (see their own doc) because `.github/workflows/ci.yml`
//! runs `cargo test --workspace` on GitHub-hosted Linux/Windows runners
//! with no real GPU; run them explicitly with `cargo test -p hp2-wgpu
//! -- --ignored` on a machine that has one.
//!
//! Still open, not started: the window-surface half (`Instance::
//! create_surface`, needs `raw-window-handle` values built by hand from
//! `SDL_GetWindowWMInfo`'s output per the plan's "Approved deviations"
//! SDL2 entry -- no `sdl2` crate, same reasoning as every other
//! SDL-touching slice this session), the `WgpuDrv` C++ package itself,
//! the render ABI's remaining pointer-containing types (`hp2-abi`'s own
//! module doc explains why those wait for this crate's actual consumer
//! logic to exist), all WGSL shader transcription, and a stronger
//! device-working proof than the current command-buffer-submit-and-poll
//! round trip -- the plan's frame capture and editor hit testing will
//! eventually need a real texture-clear-then-`copy_texture_to_buffer`-
//! then-`map_async` readback path, which is a reasonable next test to
//! add but is not required to prove this slice's own claim (a working
//! device, not yet a working renderer).

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

/// Shared `Instance` construction: the chosen backend set, headless
/// (`new_without_display_handle`) since there's no `SDL_Window*` yet.
fn build_instance(choice: BackendChoice) -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: choice.to_wgpu(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

/// Shared adapter request: `PowerPreference::HighPerformance`, no
/// compatible surface (headless -- a real window surface requires the
/// C++-owned `SDL_Window*` this crate doesn't have yet).
fn request_adapter_raw(instance: &wgpu::Instance) -> Option<wgpu::Adapter> {
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()
}

/// Replaces the adapter-request half of `WgpuDrv`'s future `Init`
/// (plan's "Device creation" bullet). Returns the adapter's name and
/// backend on success, or `None` on total failure (the real C++
/// forwarding function will turn that into the plan's `appErrorf(
/// TEXT("WgpuDrv: no compatible GPU adapter (tried %s)"), ...)` once it
/// exists).
pub fn request_adapter_sync(choice: BackendChoice) -> Option<(String, wgpu::Backend)> {
    let adapter = request_adapter_raw(&build_instance(choice))?;
    let info = adapter.get_info();
    Some((info.name, info.backend))
}

/// Result of `create_device_sync`. A plain struct instead of a growing
/// tuple: positional `(Device, Queue, String, Backend, Limits)` was
/// getting hard to read correctly at call sites.
pub struct CreatedDevice {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub backend: wgpu::Backend,
    /// The adapter's own limits, which `required_limits` was set to --
    /// `device.limits()` is guaranteed equal to this (not re-queried
    /// here; returned so callers, including this module's own tests,
    /// can check that guarantee against the real API instead of taking
    /// it on faith).
    pub adapter_limits: wgpu::Limits,
}

/// Replaces the device/queue-request half of `WgpuDrv`'s future `Init`
/// (plan's "Device creation" bullet, same scope note as
/// `request_adapter_sync`'s own doc above: headless, no window surface
/// or `compatible_surface` yet). `required_limits: adapter.limits()`,
/// not `Limits::default()`: `request_device` *panics* (not `Err`,
/// confirmed in `wgpu-30.0.1/src/api/adapter.rs`'s own doc comment) if
/// the requested limits exceed what the adapter actually supports, and
/// `Limits::default()` is a fixed WebGPU-spec baseline that a GL/GLES
/// backend -- the plan's own Raspberry Pi fallback -- can fall short
/// of. Asking for exactly `adapter.limits()` can never exceed what the
/// adapter reports, by construction, so this can never hit that panic.
/// Returns `None` on total failure (same real-C++-forwarding note as
/// `request_adapter_sync`).
pub fn create_device_sync(choice: BackendChoice) -> Option<CreatedDevice> {
    let adapter = request_adapter_raw(&build_instance(choice))?;
    let info = adapter.get_info();
    let adapter_limits = adapter.limits();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("hp2-wgpu device"),
        required_limits: adapter_limits.clone(),
        ..Default::default()
    }))
    .ok()?;
    Some(CreatedDevice {
        device,
        queue,
        adapter_name: info.name,
        backend: info.backend,
        adapter_limits,
    })
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
        let result = create_device_sync(BackendChoice::Auto);
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
