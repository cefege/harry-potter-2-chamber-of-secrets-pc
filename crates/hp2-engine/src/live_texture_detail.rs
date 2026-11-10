//! Keep resident textures consistent with live UClient LOD changes.
//!
//! UTexture::Lock reads the current LOD on each lock, but the renderer's cache
//! is keyed by CacheID and can reuse an old mip chain. Flush once, after the
//! tick that changed the client LOD, so the next frame uploads the new mips.
use hp2_abi::engine::generated::{UClient, UGameEngine, URenderDevice, UViewport};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
#[path = "live_texture_detail_policy.rs"]
mod policy;
use policy::observe;

/// Read at tick entry so changes during this tick have a baseline.
/// The engine, its client, and owned viewports remain live for this call.
pub(crate) unsafe fn sync(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    let state = access.owner_context::<crate::core_owner::EngineState>().ok_or_else(|| RuntimeError::new("Texture detail requires Engine state"))?;
    let client = unsafe { (*engine).client.cast::<UClient>() };
    if client.is_null() { return observe(&state.texture_lod_cache, None, || Ok(())); }
    let lod = unsafe { std::ptr::addr_of!((*client).texture_lod_set).read_unaligned() };
    observe(&state.texture_lod_cache, Some(lod), || {
        let list = unsafe { (*client).viewports };
        if list.num < 0 || list.max < list.num || (list.num > 0 && list.data.is_null()) {
            return Err(RuntimeError::new("Invalid viewport array during texture detail refresh"));
        }
        let mut devices = Vec::<*mut URenderDevice>::new();
        for index in 0..list.num as usize {
            let viewport = unsafe { list.data.cast::<*mut UViewport>().add(index).read() };
            if viewport.is_null() { continue; }
            let device = unsafe { (*viewport).ren_dev.cast::<URenderDevice>() };
            if !device.is_null() && !devices.contains(&device) { devices.push(device); }
        }
        for device in devices { unsafe { crate::render_device_owner::flush(access, device, false)?; } }
        Ok(())
    })
}

