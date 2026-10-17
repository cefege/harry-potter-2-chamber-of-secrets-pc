//! Rust port of the renderer, replacing `ThirdParty/XOpenGLDrv`/
//! `ThirdParty/UT99VulkanDrv` with one `wgpu`-backed render device.
//! Phase 3.2 of `local://rust-inplace-port-plan.md`.
//!
//! **Scope of this crate today: adapter creation only**, with no window
//! surface, no `WgpuDrv` C++ package yet, and nothing wired to
//! `hp2-ffi`. This is deliberately the very first, smallest slice: it
//! answers "does `wgpu` actually produce a working adapter on the real
//! hardware this project targets" before any of the surrounding C++
//! package scaffolding (`UWgpuRenderDevice : URenderDevice`, the
//! `IMPLEMENT_PACKAGE(WgpuDrv)` boilerplate, `AUTO_INITIALIZE_REGISTRANTS_
//! WGPUDRV`, the full `hp2_rs_wgpu_<method>` virtual-forwarding surface)
//! gets built around it. Verified on real hardware, not assumed: this
//! machine has a real Apple M5 GPU (Metal 4, confirmed via
//! `system_profiler SPDisplaysDataType` before this crate was started),
//! so `request_adapter_sync`'s own test confirms a genuine adapter from
//! the real Metal backend, not a software/CI fallback -- but that test
//! is `#[ignore]`d (see its own doc) because `.github/workflows/ci.yml`
//! runs `cargo test --workspace` on GitHub-hosted Linux/Windows runners
//! with no real GPU; run it explicitly with `cargo test -p hp2-wgpu --
//! --ignored` on a machine that has one.
//!
//! Still open, not started: the window-surface half (`Instance::
//! create_surface`, needs `raw-window-handle` values built by hand from
//! `SDL_GetWindowWMInfo`'s output per the plan's "Approved deviations"
//! SDL2 entry -- no `sdl2` crate, same reasoning as every other
//! SDL-touching slice this session), device/queue creation
//! (`request_device`), the `WgpuDrv` C++ package itself, the render
//! ABI's remaining pointer-containing types (`hp2-abi`'s own module doc
//! explains why those wait for this crate's actual consumer logic to
//! exist), and all WGSL shader transcription.

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

/// Replaces the device/queue-request half of `WgpuDrv`'s future `Init`
/// (plan's "Device creation" bullet, same scope note as
/// `request_adapter_sync`'s own doc above: headless, no window surface
/// or `compatible_surface` yet). Default `DeviceDescriptor` -- no
/// required features/limits beyond the adapter's own defaults, matching
/// "ask for exactly what the baseline renderer needs," which today is
/// nothing beyond what every adapter already supports. Returns the
/// device, queue, and the same name/backend `request_adapter_sync`
/// reports, or `None` on total failure (same real-C++-forwarding note
/// as `request_adapter_sync`).
pub fn create_device_sync(
    choice: BackendChoice,
) -> Option<(wgpu::Device, wgpu::Queue, String, wgpu::Backend)> {
    let adapter = request_adapter_raw(&build_instance(choice))?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("hp2-wgpu device"),
        ..Default::default()
    }))
    .ok()?;
    Some((device, queue, info.name, info.backend))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real hardware test, not a mock: requests a genuine adapter from
    // this machine's actual GPU (Apple M5, Metal 4, confirmed present
    // via `system_profiler` before this crate was written). `#[ignore]`:
    // `.github/workflows/ci.yml` runs `cargo test --workspace` on
    // `ubuntu-24.04`, `ubuntu-24.04-arm`, and `windows-2025` GitHub-hosted
    // runners -- none of which have a real GPU adapter available. A
    // genuinely-informative-sounding earlier version of this comment
    // argued "if this fails in CI, that's a fact worth knowing" -- wrong:
    // on those runners it is an entirely expected, unavoidable, already-
    // understood limitation, not a new fact, and letting it fail there
    // would make `cargo test --workspace` red for a reason unrelated to
    // any real regression, which risks masking an actual one in the same
    // run. Run explicitly on a machine with a real adapter:
    // `cargo test -p hp2-wgpu -- --ignored`. Must not be changed to
    // silently pass when no adapter is found instead: that would hide
    // real failures on machines (like this one) that do have a GPU.
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

    // Same #[ignore] reasoning as the adapter test above: needs a real
    // GPU, CI runners don't have one. Proves genuine device
    // communication, not just that a handle type-checks: submits a
    // real command buffer (an empty one is still a real round trip
    // through the queue) and polls the device for completion, which
    // would hang or error on a device that wasn't actually working.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn create_device_sync_produces_a_working_device() {
        let result = create_device_sync(BackendChoice::Auto);
        assert!(result.is_some(), "expected a real device to be created");
        let (device, queue, name, backend) = result.unwrap();
        assert!(!name.is_empty());
        let limits = device.limits();
        assert!(
            limits.max_texture_dimension_2d > 0,
            "a working device must report a nonzero max 2D texture dimension"
        );
        // Real round trip: submit an empty command buffer and wait for
        // the GPU to actually process it.
        let encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        queue.submit(std::iter::once(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        println!(
            "created device on adapter {name:?} backend {}, max_texture_dimension_2d={}",
            backend_name(backend),
            limits.max_texture_dimension_2d
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
