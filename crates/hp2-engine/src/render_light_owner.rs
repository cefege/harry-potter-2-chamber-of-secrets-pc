//! Canonical Core host for the shared renderer lighting policies.

use std::{cell::Cell, ffi::c_void, mem, ptr, rc::Rc};

use hp2_abi::{FCoords, FVector};
use hp2_abi::engine::generated::{
    AMover, AActor, ALevelInfo, APawn, APlayerPawn, AZoneInfo, FCacheItem, FTextureInfo,
    FMipmap, UClass, ULevel, UModel, UPalette, UTexture, UViewport,
};
use hp2_abi::render_scene::{FActorLink, FBspDrawList, FLeaf, FSceneNode, FVolActorLink};
use hp2_abi::render_span::FMemMark;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::{light, light_actor, light_manager, light_surf};

use super::RenderGraphState;

// `CID_ActorLightCache` from `Core/Inc/UnCId.h`.
const CID_ACTOR_LIGHT_CACHE: u32 = 0x30;

/// A surface lighting map and the owner scope keeping its GMem data and cache locks live.
pub(super) struct SurfaceSetup<'a> {
    pub(super) light_map: *mut FTextureInfo,
    pub(super) fog_map: *mut FTextureInfo,
    access: CoreAccess<'a>,
    graph: Rc<RenderGraphState>,
    active: bool,
}

impl Drop for SurfaceSetup<'_> {
    fn drop(&mut self) {
        if self.active {
            let graph = Rc::clone(&self.graph);
            graph.with_light_manager(|| {
                let _ = finish_surface_inner(self.access, &graph);
            });
            self.active = false;
        }
    }
}

/// Runs the shared `SetupForSurf` policy on this Engine owner's selected manager.
///
/// # Safety
/// `frame`, `map_coords`, and `draw` must be live source records for the active surface.
pub(super) unsafe fn setup_surface<'a>(
    access: CoreAccess<'a>,
    frame: *mut hp2_abi::engine::generated::FSceneNode,
    map_coords: *mut FCoords,
    draw: *mut FBspDrawList,
    merged: i32,
) -> RuntimeResult<SurfaceSetup<'a>> {
    if frame.is_null() || map_coords.is_null() || draw.is_null() {
        return Err(RuntimeError::new("Canonical surface lighting requires live inputs"));
    }
    let graph = super::state(access)?;
    graph.initialize_light_manager()?;
    let mut maps = (ptr::null_mut(), ptr::null_mut());
    let result = graph.with_light_manager(|| unsafe {
        setup_surface_selected(access, &graph, frame, map_coords, draw, merged, &mut maps)
    });
    if let Err(error) = result {
        graph.with_light_manager(|| {
            let _ = finish_surface_inner(access, &graph);
        });
        return Err(error);
    }
    Ok(SurfaceSetup {
        light_map: maps.0,
        fog_map: maps.1,
        access,
        graph,
        active: true,
    })
}

/// Finishes the live surface scope in source order: pop GMem, then unlock cache items.
pub(super) fn finish_surface(
    access: CoreAccess<'_>,
    mut setup: SurfaceSetup<'_>,
) -> RuntimeResult<()> {
    let graph = super::state(access)?;
    if !Rc::ptr_eq(&graph, &setup.graph) {
        return Err(RuntimeError::new("Surface lighting scope belongs to another Engine owner"));
    }
    let result = graph.with_light_manager(|| finish_surface_inner(access, &graph));
    setup.active = false;
    result
}

unsafe fn setup_surface_selected(
    access: CoreAccess<'_>,
    graph: &RenderGraphState,
    frame: *mut hp2_abi::engine::generated::FSceneNode,
    map_coords: *mut FCoords,
    draw: *mut FBspDrawList,
    merged: i32,
    maps: &mut (*mut FTextureInfo, *mut FTextureInfo),
) -> RuntimeResult<()> {
    let scene = frame.cast::<FSceneNode>();
    let level = unsafe { (*scene).level.cast::<ULevel>() };
    if level.is_null() || unsafe { (*level).model.is_null() } {
        return Err(RuntimeError::new("Surface lighting scene has no canonical level model"));
    }
    let level_model = unsafe { (*level).model.cast::<UModel>() };
    // Match FLightManager::SetupForSurf's per-surface state setup before any
    // light candidate can append through LastLight.
    let level_info = unsafe { crate::package_map_level_owner::level_info(level)? };
    let state = light_manager::state();
    unsafe {
        (*state).Frame = frame.cast();
        (*state).Level = level.cast();
        (*state).LevelInfo = level_info.cast();
        (*state).TemporaryTablesBuilt = 0;
        (*state).Zone = ptr::addr_of!((*draw).zone).read_unaligned().cast();
        (*state).MapCoords = map_coords.cast();
        (*state).LastLight = ptr::addr_of_mut!((*state).FirstLight).cast();
    }
    let draw_record = unsafe { &*draw };
    let surf = unsafe { model_array::<hp2_abi::engine::generated::FBspSurf>(ptr::addr_of!((*level_model).surfs).cast(), draw_record.i_surf)? };
    let i_light_map = unsafe { ptr::addr_of!((*surf).i_light_map).read_unaligned() };
    let mover_candidate = if unsafe { !(*level).brush_tracker.is_null() }
        && unsafe { crate::dynbsp_owner::surf_is_dynamic(level, draw_record.i_surf)? }
    {
        unsafe { ptr::addr_of!((*surf).actor).read_unaligned().cast::<AMover>() }
    } else {
        ptr::null_mut()
    };
    let active_model = if mover_candidate.is_null() {
        level_model
    } else {
        unsafe { ptr::addr_of!((*mover_candidate).base.brush).read_unaligned().cast::<UModel>() }
    };
    let light_index = unsafe { model_array::<hp2_abi::engine::generated::FLightMapIndex>(ptr::addr_of!((*active_model).light_map).cast(), i_light_map)? };
    let light_index = unsafe { &*light_index };
    let mover_dynamic = if mover_candidate.is_null() {
        0
    } else {
        u32::from(unsafe { (*mover_candidate).b_dynamic_light_mover() })
    };
    let mover = if mover_dynamic != 0 { mover_candidate } else { ptr::null_mut() };
    let zone = draw_record.zone.cast::<AZoneInfo>();
    let leaves = unsafe { ptr::addr_of!((*level_model).leaves).cast::<hp2_abi::engine::generated::TArrayRaw>().read_unaligned() };
    let model_has_leaves = u32::from(leaves.num != 0);
    let mover_leaf = if mover.is_null() { -1 } else { unsafe { (*mover).base.region.i_leaf } };
    let leaf_permeating = if mover.is_null() || mover_leaf == -1 || leaves.num == 0 {
        -1
    } else {
        let leaf = unsafe { model_array::<FLeaf>(ptr::addr_of!((*level_model).leaves).cast(), mover_leaf)? };
        unsafe { ptr::addr_of!((*leaf).i_permeating).read_unaligned() }
    };
    let static_light_bits = if mover.is_null() {
        unsafe { (*active_model).light_bits.data.cast::<u8>().offset((*light_index).data_offset as isize) }
    } else {
        ptr::null_mut()
    };
    let zone_fog = if zone.is_null() { 0 } else { u32::from(unsafe { (*zone).b_fog_zone() }) };
    let state = light_manager::state();
    let mut light_out = ptr::null_mut();
    let mut fog_out = ptr::null_mut();
    let mut params = light_surf::SurfSetupParams {
        manager: state.cast(),
        frame: frame.cast(),
        draw: draw.cast(),
        input_coords: map_coords.cast(),
        level_model: level_model.cast(),
        active_model: active_model.cast(),
        mover: mover.cast(),
        zone: zone.cast(),
        light_map: ptr::addr_of_mut!((*state).LightMap).cast(),
        fog_map: ptr::addr_of_mut!((*state).FogMap).cast(),
        light_mip: ptr::addr_of_mut!((*state).LightMip).cast(),
        fog_mip: ptr::addr_of_mut!((*state).FogMip).cast(),
        out_light_map: ptr::addr_of_mut!(light_out),
        out_fog_map: ptr::addr_of_mut!(fog_out),
        i_surf: draw_record.i_surf,
        i_light_map,
        zone_id: if zone.is_null() { 255 } else { unsafe { (*zone).base.base.region.zone_number as u32 } },
        poly_flags: draw_record.poly_flags,
        merged: u32::from(merged != 0),
        uclamp: (*light_index).u_clamp,
        vclamp: (*light_index).v_clamp,
        dims: Default::default(),
        pan: (*light_index).pan,
        uscale: (*light_index).u_scale,
        vscale: (*light_index).v_scale,
        mover_dynamic_light: mover_dynamic,
        mover_leaf,
        mover_special_lit: if mover.is_null() { 0 } else { u32::from(unsafe { (*mover).base.b_special_lit() }) },
        model_has_leaves,
        leaf_permeating,
        zone_fog,
        static_light_index: (*light_index).i_light_actors,
        static_light_bits,
        surf_lights: draw_record.surf_lights.cast(),
        volumetrics: draw_record.volumetrics.cast(),
        light_capacity: 256,
        model_cache_index: unsafe { (*active_model.cast::<hp2_abi::engine::generated::UObject>()).index as u32 },
        current_time: 0,
        mover_stamp: light_surf::MoverStamp {
            i_leaf: mover_leaf,
            location: if mover.is_null() { [0.0, 0.0, 0.0] } else { let p = unsafe { (*mover).base.location }; [p.x, p.y, p.z] },
            rotation: if mover.is_null() { [0, 0, 0] } else { let r = unsafe { (*mover).base.rotation }; [r.pitch, r.yaw, r.roll] },
        },
        frame_coords: unsafe { [(*scene).coords.origin, (*scene).coords.x_axis, (*scene).coords.y_axis, (*scene).coords.z_axis] },
        vertex_base: unsafe { FVector { x: (*state).VertexBase[0], y: (*state).VertexBase[1], z: (*state).VertexBase[2] } },
        vertex_du: unsafe { FVector { x: (*state).VertexDU[0], y: (*state).VertexDU[1], z: (*state).VertexDU[2] } },
        vertex_dv: unsafe { FVector { x: (*state).VertexDV[0], y: (*state).VertexDV[1], z: (*state).VertexDV[2] } },
        byte_fog: ptr::addr_of!((*state).ByteFog).cast(),
        fog_rejection: ptr::addr_of_mut!((*state).FogRejectionMethod),
        light_data: ptr::null_mut(),
        fog_data: ptr::null_mut(),
        light_cache_id: 0,
        fog_cache_id: 0,
        light_realtime_changed: 0,
        fog_realtime_changed: 0,
    };
    let ops = CoreSurfSetupOps { access, graph };
    unsafe { light_surf::setup_for_surf_with(&mut params, &ops, &light::GMath::canonical())? };
    *maps = (light_out.cast(), fog_out.cast());
    Ok(())
}

fn finish_surface_inner(access: CoreAccess<'_>, graph: &RenderGraphState) -> RuntimeResult<()> {
    let state = unsafe { light_manager::state() };
    if unsafe { (*state).SurfManager.is_null() } {
        return Err(RuntimeError::new("Canonical surface lighting finish has no active setup"));
    }
    let light_map = unsafe { ptr::addr_of!((*state).LightMap).cast::<FTextureInfo>().read_unaligned() };
    let mut first_error = None;
    for slot in [1, 0] {
        let active = unsafe { light_mark_active(state, slot) };
        match active {
            Ok(true) => if let Err(error) = unsafe { pop_light_mark(access, state, slot) } {
                first_error.get_or_insert(error);
            },
            Ok(false) => (),
            Err(error) => { first_error.get_or_insert(error); }
        }
    }
    if let Err(error) = unlock_pending(access) {
        first_error.get_or_insert(error);
    }
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
        let mut stats = graph.stats.get();
        let area = light_map.u_clamp.wrapping_mul(light_map.v_clamp);
        stats.lightage = stats.lightage.wrapping_add(area);
        stats.light_mem = stats.light_mem.wrapping_add(area.wrapping_mul(mem::size_of::<f32>() as i32));
        graph.stats.set(stats);
    }
    first_error.map_or(Ok(()), Err)
}

struct CoreSurfSetupOps<'a> {
    access: CoreAccess<'a>,
    graph: &'a RenderGraphState,
}

impl light_surf::SurfSetupOps for CoreSurfSetupOps<'_> {
    type Error = RuntimeError;

    unsafe fn add_light(&self, mover: *mut c_void, actor: *mut c_void) -> RuntimeResult<u32> {
        Ok(u32::from(unsafe { add_light(self.access, mover.cast(), actor.cast())? }))
    }

    unsafe fn surf_map_setup(&self, params: *mut light_surf::SurfSetupParams, which: u32) -> RuntimeResult<()> {
        unsafe { core_surface_map_setup(params, which, self.graph) }
    }

    unsafe fn surf_gmem_mark(&self, slot: u32, pop: u32) -> RuntimeResult<()> {
        let state = light_manager::state();
        if pop == 0 {
            let engine = engine_state(self.access)?;
            let mark = unsafe { engine.memory.mark() };
            unsafe { write_light_mark(state, slot, &mark)?; }
        } else {
            unsafe { pop_light_mark(self.access, state, slot)?; }
        }
        Ok(())
    }

    unsafe fn surf_gmem(&self, bytes: u64) -> RuntimeResult<*mut c_void> {
        let bytes = i32::try_from(bytes).map_err(|_| RuntimeError::new("Surface lighting GMem allocation exceeds INT size"))?;
        let memory = &engine_state(self.access)?.memory;
        let pointer = unsafe { memory.push_bytes(bytes, 1)? };
        Ok(pointer.cast())
    }

    unsafe fn surf_light_compute(&self, index: u32, which: u32) -> RuntimeResult<()> {
        let state = light_manager::state();
        let map: *mut FTextureInfo = if which == 0 { ptr::addr_of_mut!((*state).LightMap).cast() } else { ptr::addr_of_mut!((*state).FogMap).cast() };
        unsafe { compute_light_with_map(self.access, index, map.cast()) }
    }

    unsafe fn surf_cache_get(&self, id: u64) -> RuntimeResult<*mut c_void> {
        cache_get(self.access, id)
    }

    unsafe fn surf_cache_create(&self, id: u64, size: u64, pad: u64, check: u32) -> RuntimeResult<*mut c_void> {
        cache_create(self.access, id, size, pad, check != 0)
    }

    unsafe fn surf_zone_ambient(&self, out: *mut c_void) -> RuntimeResult<()> {
        let zone = unsafe { (*light_manager::state()).Zone.cast::<AZoneInfo>() };
        let ambient = crate::tex::fget_hsv(unsafe { (*zone).ambient_hue }, unsafe { (*zone).ambient_saturation }, unsafe { (*zone).ambient_brightness });
        unsafe { ptr::copy_nonoverlapping(ambient.as_ptr(), out.cast::<f32>(), 3); }
        Ok(())
    }

    unsafe fn surf_shadow_gen(&self, index: u32, use_bits: u32, dest: *mut c_void) -> RuntimeResult<()> {
        unsafe { shadow_map_gen(index, use_bits != 0, dest.cast()) }
    }

    unsafe fn surf_light_spatial_fx(&self, index: u32, shadow: *mut c_void) -> RuntimeResult<()> {
        unsafe { spatial_apply(self.access, self.graph, index, shadow.cast()) }
    }

    unsafe fn random_base_data(&self) -> RuntimeResult<*const f32> {
        Ok(self.graph.random_bases.get().cast())
    }

    unsafe fn random_data(&self) -> RuntimeResult<*const f32> {
        Ok(self.graph.random_values.get().cast())
    }
}

fn engine_state<'a>(access: CoreAccess<'a>) -> RuntimeResult<&'a crate::core_owner::EngineState> {
    access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine lighting owner state is unavailable"))
}

unsafe fn model_array<T>(array: *const c_void, index: i32) -> RuntimeResult<*mut T> {
    if array.is_null() || index < 0 {
        return Err(RuntimeError::new("Invalid canonical model array index"));
    }
    let array = unsafe { &*array.cast::<hp2_abi::engine::generated::TArrayRaw>() };
    if index >= array.num || (array.num != 0 && array.data.is_null()) {
        return Err(RuntimeError::new("Canonical model array index is out of range"));
    }
    Ok(unsafe { array.data.cast::<T>().add(index as usize) })
}

unsafe fn write_light_mark(state: *mut light_manager::State, slot: u32, mark: &FMemMark) -> RuntimeResult<()> {
    let destination = match slot {
        0 => ptr::addr_of_mut!((*state).Mark).cast::<u8>(),
        1 => ptr::addr_of_mut!((*state).SurfLocalMark).cast::<u8>(),
        _ => return Err(RuntimeError::new("Invalid canonical lighting GMem mark slot")),
    };
    if mem::size_of::<FMemMark>() != 3 * mem::size_of::<usize>() {
        return Err(RuntimeError::new("Canonical lighting GMem mark layout mismatch"));
    }
    unsafe { ptr::copy_nonoverlapping((mark as *const FMemMark).cast::<u8>(), destination, mem::size_of::<FMemMark>()); }
    if slot == 0 {
        unsafe { (*state).SurfManager = state.cast(); }
    }
    Ok(())
}

unsafe fn light_mark_active(state: *mut light_manager::State, slot: u32) -> RuntimeResult<bool> {
    let bytes = match slot {
        0 => ptr::addr_of!((*state).Mark).cast::<u8>(),
        1 => ptr::addr_of!((*state).SurfLocalMark).cast::<u8>(),
        _ => return Err(RuntimeError::new("Invalid canonical lighting GMem mark slot")),
    };
    Ok(unsafe { ptr::read_unaligned(bytes.cast::<*mut c_void>()) } != ptr::null_mut())
}

unsafe fn pop_light_mark(access: CoreAccess<'_>, state: *mut light_manager::State, slot: u32) -> RuntimeResult<()> {
    if !unsafe { light_mark_active(state, slot)? } {
        return Err(RuntimeError::new("Canonical lighting GMem mark is not active"));
    }
    let bytes = match slot {
        0 => ptr::addr_of_mut!((*state).Mark).cast::<u8>(),
        1 => ptr::addr_of_mut!((*state).SurfLocalMark).cast::<u8>(),
        _ => return Err(RuntimeError::new("Invalid canonical lighting GMem mark slot")),
    };
    let mark = unsafe { ptr::read_unaligned(bytes.cast::<FMemMark>()) };
    unsafe { engine_state(access)?.memory.pop(&mark); }
    unsafe { ptr::write_bytes(bytes, 0, mem::size_of::<FMemMark>()); }
    if slot == 0 {
        unsafe { (*state).SurfManager = ptr::null_mut(); }
    }
    Ok(())
}

thread_local! {
    static CORE_CACHE_BUSY: Cell<bool> = const { Cell::new(false) };
}

struct CoreCacheBusyGuard;

impl Drop for CoreCacheBusyGuard {
    fn drop(&mut self) {
        CORE_CACHE_BUSY.with(|busy| busy.set(false));
    }
}

fn with_cache_owner<T>(
    access: CoreAccess<'_>,
    operation: impl FnOnce(&mut hp2_core::cache_owner::CacheOwner) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    if CORE_CACHE_BUSY.with(|busy| busy.replace(true)) {
        return Err(RuntimeError::new("Canonical Engine cache operation reentered"));
    }
    let _guard = CoreCacheBusyGuard;
    let owner = engine_state(access)?;
    let cache_ptr = {
        let mut cache = owner.cache.try_borrow_mut()
            .map_err(|_| RuntimeError::new("Canonical Engine cache owner is already borrowed"))?;
        cache.as_mut()
            .map(|cache| cache as *mut hp2_core::cache_owner::CacheOwner)
            .ok_or_else(|| RuntimeError::new("Canonical Engine cache is unavailable during lighting"))?
    };
    unsafe { operation(&mut *cache_ptr) }
}

fn cache_get(access: CoreAccess<'_>, id: u64) -> RuntimeResult<*mut c_void> {
    let slot = unsafe { light_manager::reserve_slot() };
    unsafe { slot.write(ptr::null_mut()); }
    let result = with_cache_owner(access, |cache| Ok(unsafe { cache.get(access, id, 16) }))?;
    match result {
        Some((data, item)) => {
            unsafe { slot.write(item.cast()); }
            Ok(data.cast())
        }
        None => Ok(ptr::null_mut()),
    }
}

fn cache_create(access: CoreAccess<'_>, id: u64, size: u64, safety_pad: u64, check: bool) -> RuntimeResult<*mut c_void> {
    let size = i32::try_from(size)
        .map_err(|_| RuntimeError::new("Canonical lighting cache item exceeds INT size"))?;
    let safety_pad = if check {
        i32::try_from(safety_pad)
            .map_err(|_| RuntimeError::new("Canonical lighting cache padding exceeds INT size"))?
    } else {
        0
    };
    let (data, item) = with_cache_owner(access, |cache| unsafe {
        cache.create(access, id, size, 16, safety_pad)
    })?;
    unsafe { light_manager::previous_slot().write(item.cast::<c_void>()); }
    Ok(data.cast())
}

fn unlock_pending(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let mut first_error = None;
    while unsafe { light_manager::unlock_pending() } {
        let item = unsafe { light_manager::pop_unlock() }.cast::<FCacheItem>();
        if item.is_null() { continue; }
        if let Err(error) = with_cache_owner(access, |cache| unsafe { cache.unlock(access, item) }) {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

unsafe fn core_surface_map_setup(
    params: *mut light_surf::SurfSetupParams,
    which: u32,
    _graph: &RenderGraphState,
) -> RuntimeResult<()> {
    let p = unsafe { &mut *params };
    if which == 3 {
        let frame = p.frame.cast::<FSceneNode>();
        let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
        let time = unsafe { ptr::addr_of!((*viewport).current_time).read_unaligned() };
        p.current_time = i64::from_ne_bytes(time);
        return Ok(());
    }
    let map = if which == 1 { p.fog_map.cast::<FTextureInfo>() } else { p.light_map.cast::<FTextureInfo>() };
    let mip = if which == 1 { p.fog_mip.cast::<FMipmap>() } else { p.light_mip.cast::<FMipmap>() };
    if which == 2 {
        unsafe {
            set_mipmap_data(p.light_mip.cast(), p.light_data.cast());
            set_mipmap_data(p.fog_mip.cast(), p.fog_data.cast());
            if !p.light_data.is_null() {
                (*p.light_map.cast::<FTextureInfo>()).max_color = p.light_data.cast::<u8>().wrapping_sub(mem::size_of::<hp2_abi::FColor>()).cast();
            }
            (*p.light_map.cast::<FTextureInfo>()).cache_id = p.light_cache_id;
            set_realtime_changed(p.light_map.cast(), p.light_realtime_changed != 0);
            set_realtime_changed(p.fog_map.cast(), p.fog_realtime_changed != 0);
            if !p.fog_data.is_null() {
                (*p.fog_map.cast::<FTextureInfo>()).max_color = p.fog_data.cast::<u8>().wrapping_sub(mem::size_of::<hp2_abi::FColor>()).cast();
                (*p.fog_map.cast::<FTextureInfo>()).cache_id = p.fog_cache_id;
            }
        }
        return Ok(());
    }
    unsafe {
        let state = light_manager::state();
        if which == 0 {
            (*state).ShadowMaskU = p.dims.shadow_mask_u;
            (*state).ShadowMaskSpace = p.dims.shadow_mask_space;
            (*state).ShadowSkip = p.dims.shadow_skip;
            if !p.out_light_map.is_null() {
                *p.out_light_map = p.light_map;
            }
        }
        if which == 1 && !p.out_fog_map.is_null() && p.fog_realtime_changed != 0 {
            *p.out_fog_map = p.fog_map;
        }
        set_realtime_changed(map.cast(), if which == 1 { p.fog_realtime_changed != 0 } else { p.light_realtime_changed != 0 });
        (*map).num_mips = 1;
        (*map).mips[0] = mip.cast();
        (*map).format = 1; // TEXF_RGBA7.
        (*map).palette = ptr::null_mut();
        set_mipmap_data(mip.cast(), if which == 1 { p.fog_data.cast() } else { p.light_data.cast() });
        set_mipmap_dimensions(mip.cast(), p.dims.u_bits, p.dims.v_bits, p.dims.u_size, p.dims.v_size);
        (*map).pan = p.pan;
        (*map).u_scale = p.uscale;
        (*map).v_scale = p.vscale;
        (*map).u_clamp = p.uclamp;
        (*map).v_clamp = p.vclamp;
        (*map).u_size = p.dims.u_size;
        (*map).v_size = p.dims.v_size;
        let cache_id = if which == 1 { p.fog_cache_id } else { p.light_cache_id };
        if cache_id != 0 {
            (*map).cache_id = cache_id;
        }
        if which == 1 && !p.fog_data.is_null() {
            (*map).max_color = p.fog_data.cast::<u8>().wrapping_sub(mem::size_of::<hp2_abi::FColor>()).cast();
        }
        if which == 0 && !p.light_data.is_null() {
            (*map).max_color = p.light_data.cast::<u8>().wrapping_sub(mem::size_of::<hp2_abi::FColor>()).cast();
        }
    }
    Ok(())
}

unsafe fn set_mipmap_data(mip: *mut FMipmap, data: *mut u8) {
    unsafe { (*mip).base.data_ptr = data.cast(); }
}

unsafe fn set_mipmap_dimensions(mip: *mut FMipmap, u_bits: i32, v_bits: i32, u_size: i32, v_size: i32) {
    unsafe {
        (*mip).base.u_bits = u_bits as u8;
        (*mip).base.v_bits = v_bits as u8;
        (*mip).base.u_size = u_size;
        (*mip).base.v_size = v_size;
    }
}

unsafe fn set_realtime_changed(map: *mut FTextureInfo, changed: bool) {
    unsafe {
        (*map).set_b_realtime_changed(changed);
    }
}

fn actor_view_vector(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<FVector> {
    if actor.is_null() { return Err(RuntimeError::new("Canonical light has no actor")); }
    let pawn_class = access.native_class("Engine.Pawn")?;
    let is_pawn = unsafe { hp2_core::object_model::is_a(actor.cast(), pawn_class.cast()) };
    let rotation = if is_pawn {
        unsafe { (*actor.cast::<APawn>()).view_rotation }
    } else {
        unsafe { (*actor).rotation }
    };
    Ok(hp2_core::math::rotator_vector(
        hp2_core::math::FRotator { pitch: rotation.pitch, yaw: rotation.yaw, roll: rotation.roll },
        hp2_core::math::trig_table(),
    ))
}

pub(super) fn light_and_fog(
    access: CoreAccess<'_>,
    vertex: &mut light::FogVertex,
    flags: u32,
) -> RuntimeResult<bool> {
    let graph = super::state(access)?;
    graph.with_light_manager(|| unsafe {
        Ok(light_manager::light_and_fog_with_gmath(
            &light::GMath::canonical(),
            vertex,
            flags,
        ))
    })
}

fn add_light(access: CoreAccess<'_>, actor: *mut AActor, light_actor: *mut AActor) -> RuntimeResult<bool> {
    if light_actor.is_null() {
        return Err(RuntimeError::new("Canonical AddLight received a null light actor"));
    }
    if !unsafe { light_manager::light_has_room() } {
        return Ok(false);
    }
    let client = engine_state(access)?
        .bitmap_client
        .get()
        .ok_or_else(|| RuntimeError::new("Canonical AddLight has no Engine client lighting settings"))?;
    let no_dynamic = unsafe { client.as_ref().no_dynamic_lights() };
    let mut io = light_actor::LightAddIo {
        actor: actor.cast(),
        light: light_actor.cast(),
        accept: 0,
        opt: 0,
        moving_delta: 0,
        static_delta: 0,
        dynamic_delta: 0,
        static_lighting_changed: 0,
    };
    light_actor::add_light_decide(
        &mut io,
        || no_dynamic,
        |effect| unsafe { light_manager::effect_flags(effect) },
    );
    if io.accept == 0 {
        return Ok(false);
    }
    unsafe {
        let state = light_manager::state();
        let slot = (*state).LastLight;
        (*slot).Opt = io.opt as i32;
        (*slot).Actor = light_actor.cast();
        (*slot).IsVolumetric = 0;
        (*slot).ShadowBits = ptr::null_mut();
        (*state).MovingLights = (*state).MovingLights.wrapping_add(io.moving_delta as i32);
        (*state).StaticLights = (*state).StaticLights.wrapping_add(io.static_delta as i32);
        (*state).DynamicLights = (*state).DynamicLights.wrapping_add(io.dynamic_delta as i32);
        if io.static_lighting_changed != 0 {
            (*state).StaticLightingChanged = 1;
        }
        (*state).LastLight = slot.add(1);
    }
    Ok(true)
}

unsafe fn shadow_map_gen(index: u32, use_bits: bool, dest: *mut u8) -> RuntimeResult<()> {
    let state = unsafe { light_manager::state() };
    let count = unsafe { light_manager::light_count() };
    if index as usize >= count {
        return Err(RuntimeError::new("Canonical shadow generation light index is out of range"));
    }
    let light_map = unsafe { ptr::addr_of!((*state).LightMap).cast::<FTextureInfo>().read_unaligned() };
    let shadow_u = unsafe { (*state).ShadowMaskU.max(0) as usize };
    let v_clamp = light_map.v_clamp.max(0) as usize;
    let words = if v_clamp == 1 { shadow_u.saturating_mul(4) } else { shadow_u.saturating_mul(v_clamp).saturating_mul(2) };
    let src_len = unsafe { ((*state).ShadowMaskSpace.max(0) as usize).saturating_mul(8) };
    let bits = unsafe { (*state).FirstLight[index as usize].ShadowBits };
    let src = if use_bits && !bits.is_null() {
        Some(unsafe { std::slice::from_raw_parts(bits, src_len) })
    } else {
        None
    };
    let dest = unsafe { std::slice::from_raw_parts_mut(dest.cast::<u32>(), words) };
    unsafe { light::shadow_map_gen(&(*state).FilterTab, shadow_u, v_clamp, src, dest); }
    Ok(())
}

unsafe fn spatial_apply(
    access: CoreAccess<'_>,
    _graph: &RenderGraphState,
    index: u32,
    shadow: *mut c_void,
) -> RuntimeResult<()> {
    let state = unsafe { light_manager::state() };
    let count = unsafe { light_manager::light_count() };
    if index as usize >= count {
        return Err(RuntimeError::new("Canonical spatial light index is out of range"));
    }
    let info = unsafe { ptr::addr_of!((*state).FirstLight[index as usize]) };
    let actor = unsafe { (*info).Actor.cast::<AActor>() };
    if actor.is_null() {
        return Err(RuntimeError::new("Canonical spatial light has no actor"));
    }
    let effect = unsafe { u32::from((*actor).light_effect).min((hp2_render::light_info::LE_MAX - 1) as u32) as u8 };
    let kind = unsafe { light_manager::spatial_for_effect(effect) };
    let map = unsafe { ptr::addr_of!((*state).LightMap).cast::<FTextureInfo>() };
    // SetupForSurf passes FirstLight[I].IlluminationMap as the spatial
    // callback's destination. It is the per-light GMem/cache allocation and
    // is installed immediately before this callback; LightMap's mip is the
    // eventual merged surface texture, not the spatial-effect output.
    let dest = unsafe { (*info).IlluminationMap };
    if dest.is_null() {
        return Err(RuntimeError::new("Canonical spatial illumination map has no data"));
    }
    let src_len = unsafe { ((*state).ShadowMaskSpace.max(0) as usize).saturating_mul(8) };
    let dest_len = unsafe {
        ((*map).u_clamp.max(0) as usize).saturating_mul((*map).v_clamp.max(0) as usize)
    };
    if shadow.is_null() && src_len != 0 {
        return Err(RuntimeError::new("Canonical spatial effect requires a shadow map"));
    }
    let source = unsafe { std::slice::from_raw_parts(shadow.cast::<u8>(), src_len) };
    let destination = unsafe { std::slice::from_raw_parts_mut(dest, dest_len) };
    let view = if kind == light::Spatial::Spotlight {
        actor_view_vector(access, actor)?
    } else {
        FVector { x: 0.0, y: 0.0, z: 0.0 }
    };
    unsafe {
        light_manager::spatial_apply_with_gmath(
            &light::GMath::canonical(),
            kind,
            map.cast(),
            info.cast(),
            source,
            destination,
            view,
        );
    }
    Ok(())
}
 
/// Builds the source `LeafVolumetricLighting` list using graph-owned dynamic memory.
///
/// # Safety
/// `frame`, `model`, and `leaf` must describe the active source scene.
pub(super) unsafe fn leaf_volumetric_lighting(
    access: CoreAccess<'_>,
    frame: *mut hp2_abi::engine::generated::FSceneNode,
    model: *mut UModel,
    leaf: i32,
) -> RuntimeResult<()> {
    if frame.is_null() || model.is_null() {
        return Err(RuntimeError::new("Canonical leaf lighting requires live scene inputs"));
    }
    let graph = super::state(access)?;
    let leaf_record = unsafe { model_array::<FLeaf>(ptr::addr_of!((*model).leaves).cast(), leaf)? };
    let first_static = unsafe { ptr::addr_of!((*leaf_record).i_volumetric).read_unaligned() };
    let dynamic = graph.with_leaf_lights(|links| {
        if links.is_empty() {
            Ok(ptr::null_mut())
        } else {
            links.get(leaf as usize).copied()
                .ok_or_else(|| RuntimeError::new("Canonical leaf lighting dynamic-link index is out of range"))
        }
    })?;
    let render = graph.render()?.as_ptr();
    let first = unsafe { ptr::addr_of_mut!((*render).first_volumetric).cast::<*mut FVolActorLink>() };
    let stamp = unsafe { (*graph.render_state_ptr()).stamp };
    let lights = ptr::addr_of!((*model).lights).cast::<c_void>();
    let mut ops = CoreLeafLightingOps { graph: &graph };
    unsafe {
        hp2_render::scene_pipeline::helpers_core::leaf_lighting_with_ops(
            &mut ops,
            frame.cast(),
            lights,
            first_static,
            dynamic,
            first,
            stamp,
        )
    }
}

struct CoreLeafLightingOps<'a> {
    graph: &'a RenderGraphState,
}

impl hp2_render::scene_pipeline::helpers_core::LeafLightingOps for CoreLeafLightingOps<'_> {
    type Error = RuntimeError;

    unsafe fn actor(
        &mut self,
        actor: *mut c_void,
        out: &mut hp2_render::scene_pipeline::helpers_core::Hp2ScenePipelineHelperLightingActor,
    ) -> RuntimeResult<()> {
        if actor.is_null() {
            return Err(RuntimeError::new("Canonical leaf lighting encountered a null actor"));
        }
        let actor = actor.cast::<AActor>();
        unsafe {
            out.location = (*actor).location;
            out.volume_radius = (*actor).volume_radius as u8;
            out.tag = ptr::addr_of_mut!((*actor).lighting_tag);
        }
        Ok(())
    }

    unsafe fn allocate_link(&mut self) -> RuntimeResult<*mut FVolActorLink> {
        let bytes = i32::try_from(mem::size_of::<FVolActorLink>())
            .map_err(|_| RuntimeError::new("Canonical volumetric link exceeds INT size"))?;
        let memory = self.graph.dynamic_memory()?;
        let storage = unsafe { memory.push_bytes(bytes, hp2_render::span::DEFAULT_ALIGNMENT as i32)? };
        Ok(storage.cast())
    }

    unsafe fn construct_static(
        &mut self,
        frame: *mut FSceneNode,
        actor: *mut c_void,
        next: *mut FVolActorLink,
        storage: *mut FVolActorLink,
    ) -> RuntimeResult<()> {
        if storage.is_null() || frame.is_null() || actor.is_null() {
            return Err(RuntimeError::new("Canonical static volumetric link construction has invalid input"));
        }
        let actor_ptr = actor.cast::<AActor>();
        let location = unsafe {
            hp2_core::math::vector_transform_point((*actor_ptr).location, (*frame).coords)
        };
        unsafe {
            ptr::write(storage, FVolActorLink { location, actor, next, volumetric: 1 });
        }
        Ok(())
    }

    unsafe fn construct_dynamic(
        &mut self,
        link: *mut FVolActorLink,
        next: *mut FVolActorLink,
        storage: *mut FVolActorLink,
    ) -> RuntimeResult<()> {
        if storage.is_null() || link.is_null() {
            return Err(RuntimeError::new("Canonical dynamic volumetric link construction has invalid input"));
        }
        unsafe {
            let mut copy = *link;
            copy.next = next;
            ptr::write(storage, copy);
        }
        Ok(())
    }
}

pub(super) fn compute_light(
    access: CoreAccess<'_>,
    index: u32,
    frame: *mut hp2_abi::engine::generated::FSceneNode,
    map: *mut FTextureInfo,
) -> RuntimeResult<()> {
    if !frame.is_null() {
        let state = unsafe { light_manager::state() };
        unsafe { (*state).Frame = frame.cast() };
    }
    compute_light_with_map(access, index, map.cast())
}

pub(super) fn global_lighting(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    brightness: &mut f32,
    color: &mut hp2_abi::FPlane,
) -> RuntimeResult<()> {
    if actor.is_null() {
        return Err(RuntimeError::new("Canonical global lighting has no owner"));
    }
    let graph = super::state(access)?;
    graph.initialize_light_manager()?;
    graph.with_light_manager(|| {
        let actor = actor.cast::<AActor>();
        let manager = unsafe { light_manager::state() };
        let mut params = hp2_render::light_info::GlobalParams {
            owner: actor.cast(),
            realtime: 0,
            light_type: unsafe { (*actor).light_type as u32 },
            light_period: unsafe { (*actor).light_period as u32 },
            light_phase: unsafe { (*actor).light_phase as u32 },
            light_hue: unsafe { (*actor).light_hue as u32 },
            light_saturation: unsafe { (*actor).light_saturation as u32 },
            brightness: *brightness,
            color: [color.x, color.y, color.z, color.w],
            strobe_last: unsafe { (*manager).StrobeLastUpdateTime },
            strobe_toggle: unsafe { (*manager).StrobeToggle },
        };
        let mut ops = CoreComputeOps { access, graph: &graph, clock_start: None };
        let random = unsafe { &*graph.random_bases.get() };
        hp2_render::light_info::global_with_ops(
            &light::GMath::canonical(),
            &mut ops,
            random,
            &mut params,
        )?;
        *brightness = params.brightness;
        color.x = params.color[0];
        color.y = params.color[1];
        color.z = params.color[2];
        color.w = params.color[3];
        unsafe {
            (*manager).StrobeLastUpdateTime = params.strobe_last;
            (*manager).StrobeToggle = params.strobe_toggle;
        }
        Ok(())
    })
}

fn compute_light_with_map(
    access: CoreAccess<'_>,
    index: u32,
    map: *mut c_void,
) -> RuntimeResult<()> {
    let graph = super::state(access)?;
    graph.initialize_light_manager()?;
    graph.with_light_manager(|| unsafe { compute_light_selected(access, &graph, index, map) })
}

unsafe fn compute_light_selected(
    access: CoreAccess<'_>,
    graph: &RenderGraphState,
    index: u32,
    map: *mut c_void,
) -> RuntimeResult<()> {
    let state = unsafe { light_manager::state() };
    if index as usize >= unsafe { light_manager::light_count() } {
        return Err(RuntimeError::new("Canonical light computation index is out of range"));
    }
    let info = unsafe { ptr::addr_of_mut!((*state).FirstLight[index as usize]) };
    let actor = unsafe { (*info).Actor.cast::<AActor>() };
    let frame = unsafe { (*state).Frame.cast::<hp2_abi::render_scene::FSceneNode>() };
    if actor.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical light computation has no actor or frame"));
    }
    let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
    if viewport.is_null() || unsafe { (*viewport).actor.is_null() } {
        return Err(RuntimeError::new("Canonical light computation has no viewport actor"));
    }
    let map_texture = map.cast::<FTextureInfo>();
    let has_map = !map.is_null();
    let map_coords = if has_map {
        let coords = unsafe { (*state).MapCoords };
        if coords.is_null() {
            return Err(RuntimeError::new("Canonical surface light computation has no map coordinates"));
        }
        light_coords(unsafe { coords.cast::<FCoords>().read_unaligned() })
    } else {
        Default::default()
    };
    let light_map = unsafe { ptr::addr_of!((*state).LightMap).cast::<FTextureInfo>().read_unaligned() };
    let actor_level = unsafe { (*actor).level.cast::<ALevelInfo>() };
    if actor_level.is_null() {
        return Err(RuntimeError::new("Canonical light actor has no level"));
    }
    let map_texture_value = if has_map { unsafe { map_texture.read_unaligned() } } else { unsafe { mem::zeroed() } };
    let mut params = hp2_render::light_info::ComputeParams {
        actor: actor.cast(),
        actor_location: [unsafe { (*actor).location.x }, unsafe { (*actor).location.y }, unsafe { (*actor).location.z }],
        light_radius_inner: unsafe { (*actor).light_radius_inner as f32 },
        light_brightness: unsafe { (*actor).light_brightness as f32 },
        level_brightness: unsafe { (*actor_level).brightness },
        light_source: unsafe { (*actor).light_source as u32 },
        light_effect: unsafe { (*actor).light_effect as u32 },
        light_type: unsafe { (*actor).light_type as u32 },
        light_period: unsafe { (*actor).light_period as u32 },
        light_phase: unsafe { (*actor).light_phase as u32 },
        light_hue: unsafe { (*actor).light_hue as u32 },
        light_saturation: unsafe { (*actor).light_saturation as u32 },
        volume_fog: unsafe { (*actor).volume_fog as f32 },
        volume_brightness: unsafe { (*actor).volume_brightness as f32 },
        light_changed: u32::from(unsafe { (*actor).b_light_changed() }),
        show_flags: unsafe { (*(*viewport).actor.cast::<APlayerPawn>()).show_flags as u32 },
        is_volumetric: u32::from(unsafe { (*info).IsVolumetric != 0 }),
        frame_coords: light_coords(unsafe { (*frame).coords }),
        has_map: u32::from(has_map),
        map_coords,
        map_pan_x: map_texture_value.pan.x,
        map_pan_y: map_texture_value.pan.y,
        map_uscale: map_texture_value.u_scale,
        map_vscale: map_texture_value.v_scale,
        light_pan_x: light_map.pan.x,
        light_pan_y: light_map.pan.y,
        light_uscale: light_map.u_scale,
        light_vscale: light_map.v_scale,
        light_uclamp: light_map.u_clamp,
        light_vclamp: light_map.v_clamp,
        light_dist: unsafe { ptr::addr_of!((*state).LightDist).cast() },
        light_dist_inc: unsafe { ptr::addr_of!((*state).LightDistInc).cast() },
        cid_palette: 0x17,
        cid_volumetric: 0x20,
        effect_out: unsafe { ptr::addr_of_mut!((*info).Effect).cast() },
        temporary_tables_built: unsafe { (*state).TemporaryTablesBuilt as u32 },
        map_uncoords: unsafe { ptr::addr_of!((*state).MapUncoords).cast::<hp2_render::light_info::LightCoords>().read_unaligned() },
        vertex_base: unsafe { (*state).VertexBase },
        vertex_du: unsafe { (*state).VertexDU },
        vertex_dv: unsafe { (*state).VertexDV },
        strobe_last: unsafe { (*state).StrobeLastUpdateTime },
        strobe_toggle: unsafe { (*state).StrobeToggle },
        palette_offsets: [
            std::mem::offset_of!(hp2_abi::FColor, b) as u32,
            std::mem::offset_of!(hp2_abi::FColor, g) as u32,
            std::mem::offset_of!(hp2_abi::FColor, r) as u32,
            std::mem::offset_of!(hp2_abi::FColor, a) as u32,
        ],
        radius: unsafe { (*info).Radius },
        radius_inner: unsafe { (*info).RadiusInner },
        rradius: unsafe { (*info).RRadius },
        rradius_mult: unsafe { (*info).RRadiusMult },
        location: unsafe { (*info).Location },
        brightness: unsafe { (*info).Brightness },
        float_color: unsafe { (*info).FloatColor },
        max_spec_dir: unsafe { (*info).MaxSpecDir },
        light_dir: unsafe { (*info).LightDir },
        diffuse: unsafe { (*info).Diffuse },
        light_dist_tab: unsafe { (*info).LightDistTab.cast() },
        min_u: unsafe { (*info).MinU },
        max_u: unsafe { (*info).MaxU },
        min_v: unsafe { (*info).MinV },
        max_v: unsafe { (*info).MaxV },
        palette: unsafe { (*info).Palette },
        vol_palette: unsafe { (*info).VolPalette },
        volumetric_color: unsafe { (*info).VolumetricColor },
        vol_radius: unsafe { (*info).VolRadius },
        vol_radius_squared: unsafe { (*info).VolRadiusSquared },
        rvol_radius: unsafe { (*info).RVolRadius },
        rvol_radius_squared: unsafe { (*info).RVolRadiusSquared },
        location_size_squared: unsafe { (*info).LocationSizeSquared },
        vol_brightness: unsafe { (*info).VolBrightness },
        vol_inside: u32::from(unsafe { (*info).VolInside != 0 }),
    };
    let mut ops = CoreComputeOps { access, graph, clock_start: None };
    let random = unsafe { &*graph.random_bases.get() };
    hp2_render::light_info::compute_from_actor_with_ops(
        &light::GMath::canonical(),
        &mut ops,
        random,
        &mut params,
    )?;
    unsafe {
        (*info).Radius = params.radius;
        (*info).RadiusInner = params.radius_inner;
        (*info).RRadius = params.rradius;
        (*info).RRadiusMult = params.rradius_mult;
        (*info).Location = params.location;
        (*info).Brightness = params.brightness;
        (*info).FloatColor = params.float_color;
        (*info).MaxSpecDir = params.max_spec_dir;
        (*info).LightDir = params.light_dir;
        (*info).Diffuse = params.diffuse;
        (*info).LightDistTab = params.light_dist_tab.cast();
        (*info).MinU = params.min_u;
        (*info).MaxU = params.max_u;
        (*info).MinV = params.min_v;
        (*info).MaxV = params.max_v;
        (*info).Palette = params.palette;
        (*info).VolPalette = params.vol_palette;
        (*info).VolumetricColor = params.volumetric_color;
        (*info).VolRadius = params.vol_radius;
        (*info).VolRadiusSquared = params.vol_radius_squared;
        (*info).RVolRadius = params.rvol_radius;
        (*info).RVolRadiusSquared = params.rvol_radius_squared;
        (*info).LocationSizeSquared = params.location_size_squared;
        (*info).VolBrightness = params.vol_brightness;
        (*info).VolInside = params.vol_inside as i32;
        (*state).TemporaryTablesBuilt = params.temporary_tables_built as i32;
        (*state).MapUncoords = *(&params.map_uncoords as *const hp2_render::light_info::LightCoords).cast::<[f32; 12]>();
        (*state).VertexBase = params.vertex_base;
        (*state).VertexDU = params.vertex_du;
        (*state).VertexDV = params.vertex_dv;
        (*state).StrobeLastUpdateTime = params.strobe_last;
        (*state).StrobeToggle = params.strobe_toggle;
    }
    Ok(())
}

fn light_coords(coords: FCoords) -> hp2_render::light_info::LightCoords {
    hp2_render::light_info::LightCoords {
        origin: [coords.origin.x, coords.origin.y, coords.origin.z],
        x_axis: [coords.x_axis.x, coords.x_axis.y, coords.x_axis.z],
        y_axis: [coords.y_axis.x, coords.y_axis.y, coords.y_axis.z],
        z_axis: [coords.z_axis.x, coords.z_axis.y, coords.z_axis.z],
    }
}

fn actor_cache_id(base: u32, actor: *mut c_void) -> RuntimeResult<u64> {
    if actor.is_null() {
        return Err(RuntimeError::new("Canonical actor cache key has a null actor"));
    }
    let index = unsafe { (*actor.cast::<AActor>()).index as u32 };
    Ok(hp2_core::cache_id::object(base, index, 0))
}

pub(super) struct ActorSetup<'a> {
    access: CoreAccess<'a>,
    graph: Rc<RenderGraphState>,
    active: bool,
}

impl Drop for ActorSetup<'_> {
    fn drop(&mut self) {
        if self.active {
            let graph = Rc::clone(&self.graph);
            let _ = graph.with_light_manager(|| finish_actor_inner(self.access, &graph));
            self.active = false;
        }
    }
}

pub(super) unsafe fn setup_actor<'a>(
    access: CoreAccess<'a>,
    frame: *mut hp2_abi::engine::generated::FSceneNode,
    actor: *mut AActor,
    leaf_lights: *mut FVolActorLink,
    volumetrics: *mut FActorLink,
) -> RuntimeResult<(u32, ActorSetup<'a>)> {
    if frame.is_null() || actor.is_null() {
        return Err(RuntimeError::new("Canonical actor lighting requires live frame and actor"));
    }
    let graph = super::state(access)?;
    graph.initialize_light_manager()?;
    let mut setup = ActorSetup { access, graph: Rc::clone(&graph), active: false };
    let result = graph.with_light_manager(|| unsafe {
        let mut ops = CoreActorSetupOps {
            access,
            graph: &graph,
            setup: &mut setup,
            candidate: CoreComputeOps { access, graph: &graph, clock_start: None },
        };
        let mut input = hp2_render::light_manager::ActorInput {
            frame: frame.cast(),
            actor: actor.cast(),
            leaf_lights: leaf_lights.cast(),
            volumetrics: volumetrics.cast(),
            level: ptr::null_mut(),
            level_info: ptr::null_mut(),
            zone: ptr::null_mut(),
            zone_number: 0,
            begin: hp2_render::light_actor::ActorBeginIo {
                scale_glow: 0.0,
                specular_glow: 0.0,
                specular_width: 0,
                ambient_glow: 0,
                time: 0.0,
                hsv: [0.0; 3],
                diffuse: 0.0,
                specular: 0.0,
                spec_min_cos: 0.0,
                ambient: [0.0; 4],
            },
            unlit: 0,
            leaf: -1,
            leaves: 0,
            rend_map: 0,
            no_lighting: 0,
            cache_id: 0,
            candidates: hp2_render::light_actor::CandidateIo {
                actor: actor.cast(),
                link_head: leaf_lights.cast(),
                actor_location: [0.0; 3],
                actor_special_lit: 0,
                permeating: -1,
                delta: 0,
                frame_count: 0,
                first_see: 0,
            },
            delta_time: 0.0,
        };
        hp2_render::light_manager::setup_actor_with_ops(&mut ops, &mut input)
    });
    match result {
        Ok(result) => Ok((result, setup)),
        Err(error) => {
            drop(setup);
            Err(error)
        }
    }
}

pub(super) fn finish_actor(
    access: CoreAccess<'_>,
    mut setup: ActorSetup<'_>,
) -> RuntimeResult<()> {
    let graph = super::state(access)?;
    if !Rc::ptr_eq(&graph, &setup.graph) {
        return Err(RuntimeError::new("Actor lighting scope belongs to another Engine owner"));
    }
    let result = graph.with_light_manager(|| finish_actor_inner(access, &graph));
    setup.active = false;
    result
}

fn finish_actor_inner(access: CoreAccess<'_>, _graph: &RenderGraphState) -> RuntimeResult<()> {
    let manager = unsafe { light_manager::state() };
    unsafe { pop_light_mark(access, manager, 0)?; }
    unlock_pending(access)
}

struct CoreActorSetupOps<'a, 'g, 's> {
    access: CoreAccess<'a>,
    graph: &'g RenderGraphState,
    setup: &'s mut ActorSetup<'a>,
    candidate: CoreComputeOps<'g>,
}

impl CoreActorSetupOps<'_, '_, '_> {
    unsafe fn input_actor(input: *mut hp2_render::light_manager::ActorInput) -> RuntimeResult<*mut AActor> {
        let actor = unsafe { (*input).actor.cast::<AActor>() };
        if actor.is_null() { Err(RuntimeError::new("Canonical actor lighting input has no actor")) } else { Ok(actor) }
    }
}

impl light_actor::SetupTailOps for CoreActorSetupOps<'_, '_, '_> {
    type Error = RuntimeError;
    fn compute_light(&mut self, index: u32, frame: *mut c_void) -> RuntimeResult<()> {
        compute_light(self.access, index, frame.cast(), ptr::null_mut())
    }
    fn stat_vtric_inc(&mut self) -> RuntimeResult<()> {
        if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
            let mut stats = self.graph.stats.get();
            stats.mesh_vtric_count = stats.mesh_vtric_count.wrapping_add(1);
            self.graph.stats.set(stats);
        }
        Ok(())
    }
    fn stat_mesh_light_add(&mut self, count: u32) -> RuntimeResult<()> {
        if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
            let mut stats = self.graph.stats.get();
            stats.mesh_light_count = stats.mesh_light_count.wrapping_add(count as i32);
            self.graph.stats.set(stats);
        }
        Ok(())
    }
}

impl<'a, 'g, 's> hp2_render::light_manager::ActorSetupOps for CoreActorSetupOps<'a, 'g, 's> {
    type Error = RuntimeError;
    type Candidate = CoreComputeOps<'g>;
    type Tail = Self;

    fn mark_begin(&mut self) -> RuntimeResult<()> {
        let mark = unsafe { engine_state(self.access)?.memory.mark() };
        let manager = unsafe { light_manager::state() };
        unsafe { write_light_mark(manager, 0, &mark)?; }
        self.setup.active = true;
        Ok(())
    }
    fn read_context(&mut self, input: *mut hp2_render::light_manager::ActorInput, field: u32) -> RuntimeResult<()> {
        if input.is_null() { return Err(RuntimeError::new("Canonical actor lighting input is null")); }
        match field {
            0 => {
                let frame = unsafe { (*input).frame.cast::<hp2_abi::render_scene::FSceneNode>() };
                unsafe { (*input).level = (*frame).level.cast(); }
            }
            1 => {
                let level = unsafe { (*input).level.cast::<ULevel>() };
                let level_info = unsafe { crate::package_map_level_owner::level_info(level)? };
                unsafe { (*input).level_info = level_info.cast(); }
            }
            2 => {
                let actor = unsafe { Self::input_actor(input)? };
                let zone = unsafe { (*actor).region.zone };
                let level_info = unsafe { (*input).level_info };
                unsafe {
                    (*input).zone = if zone.is_null() { level_info } else { zone.cast() };
                    (*input).zone_number = (*actor).region.zone_number as i32;
                }
            }
            _ => return Err(RuntimeError::new("Canonical actor lighting context field is invalid")),
        }
        Ok(())
    }
    fn set_zone(&mut self) -> RuntimeResult<()> {
        let manager = unsafe { light_manager::state() };
        let level = unsafe { (*manager).Level.cast::<ULevel>() };
        let actor = unsafe { (*manager).Actor.cast::<AActor>() };
        unsafe { crate::level_zone_owner::set_actor_zone(self.access, level, actor, true, false) }
    }
    fn read_begin(&mut self, input: *mut hp2_render::light_manager::ActorInput) -> RuntimeResult<()> {
        let actor = unsafe { Self::input_actor(input)? };
        let manager = unsafe { light_manager::state() };
        let zone = unsafe { (*manager).Zone.cast::<AZoneInfo>() };
        let frame = unsafe { (*manager).Frame.cast::<hp2_abi::render_scene::FSceneNode>() };
        let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
        if zone.is_null() || viewport.is_null() {
            return Err(RuntimeError::new("Canonical actor lighting has no ambient zone or viewport"));
        }
        let begin = unsafe { &mut (*input).begin };
        unsafe {
            begin.scale_glow = (*actor).scale_glow;
            begin.specular_glow = (*actor).specular_glow;
            begin.specular_width = (*actor).specular_width as u32;
            begin.ambient_glow = (*actor).ambient_glow as u32;
            begin.time = if (*actor).ambient_glow == 255 {
                hp2_core::time::get_float(i64::from_ne_bytes((*viewport).current_time))
            } else {
                0.0
            };
            let hsv = crate::tex::fget_hsv(
                (*zone).ambient_hue,
                (*zone).ambient_saturation,
                (*zone).ambient_brightness,
            );
            begin.hsv = [hsv[0], hsv[1], hsv[2]];
        }
        Ok(())
    }
    fn read_gate(&mut self, input: *mut hp2_render::light_manager::ActorInput, field: u32) -> RuntimeResult<()> {
        let actor = unsafe { Self::input_actor(input)? };
        match field {
            0 => unsafe { (*input).unlit = u32::from((*actor).b_unlit()) },
            1 => unsafe { (*input).leaf = (*actor).region.i_leaf },
            2 => {
                let level = unsafe { (*input).level.cast::<ULevel>() };
                let model = unsafe { (*level).model.cast::<UModel>() };
                unsafe { (*input).leaves = (*ptr::addr_of!((*model).leaves).cast::<hp2_abi::engine::generated::TArrayRaw>()).num; }
            }
            3 => {
                let manager = unsafe { light_manager::state() };
                let viewport = unsafe { (*(*manager).Frame.cast::<hp2_abi::render_scene::FSceneNode>()).viewport.cast::<UViewport>() };
                if viewport.is_null() || unsafe { (*viewport).actor.is_null() } {
                    return Err(RuntimeError::new("Canonical actor lighting has no viewport actor"));
                }
                unsafe { (*input).rend_map = (*(*viewport).actor.cast::<APlayerPawn>()).rend_map as i32; }
            }
            4 => {
                let manager = unsafe { light_manager::state() };
                let viewport = unsafe { (*(*manager).Frame.cast::<hp2_abi::render_scene::FSceneNode>()).viewport.cast::<UViewport>() };
                let client = if viewport.is_null() { ptr::null_mut() } else { unsafe { (*viewport).outer.cast::<hp2_abi::engine::generated::UClient>() } };
                if client.is_null() { return Err(RuntimeError::new("Canonical actor lighting has no viewport client")); }
                unsafe { (*input).no_lighting = u32::from((*client).no_lighting()); }
            }
            _ => return Err(RuntimeError::new("Canonical actor lighting gate field is invalid")),
        }
        Ok(())
    }
    fn read_cache(&mut self, input: *mut hp2_render::light_manager::ActorInput) -> RuntimeResult<()> {
        let manager = unsafe { light_manager::state() };
        let frame = unsafe { (*manager).Frame.cast::<hp2_abi::render_scene::FSceneNode>() };
        let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
        if viewport.is_null() { return Err(RuntimeError::new("Canonical actor lighting has no viewport")); }
        let current = i64::from_ne_bytes(unsafe { (*viewport).current_time });
        let previous = i64::from_ne_bytes(unsafe { (*viewport).last_update_time });
        unsafe {
            (*input).delta_time = hp2_core::time::subtract(current, previous);
            (*input).candidates.frame_count = (*viewport).frame_count;
            (*input).cache_id = hp2_core::cache_id::object(CID_ACTOR_LIGHT_CACHE, (*(*manager).Actor.cast::<AActor>()).index as u32, 0);
        }
        Ok(())
    }
    fn cache_get(&mut self, id: u64) -> RuntimeResult<*mut c_void> {
        cache_get(self.access, id)
    }
    fn cache_create(&mut self, id: u64, size: u64) -> RuntimeResult<std::ptr::NonNull<c_void>> {
        let data = cache_create(self.access, id, size, 0, false)?;
        std::ptr::NonNull::new(data).ok_or_else(|| RuntimeError::new("Canonical actor light cache allocation failed"))
    }
    fn read_candidates(&mut self, input: *mut hp2_render::light_manager::ActorInput) -> RuntimeResult<()> {
        let actor = unsafe { Self::input_actor(input)? };
        let level = unsafe { (*input).level.cast::<ULevel>() };
        let model = unsafe { (*level).model.cast::<UModel>() };
        let leaf_index = unsafe { (*actor).region.i_leaf };
        let leaves = unsafe { ptr::addr_of!((*model).leaves).cast::<hp2_abi::engine::generated::TArrayRaw>() };
        let leaf = unsafe { model_array::<FLeaf>(leaves.cast(), leaf_index)? };
        unsafe {
            (*input).candidates.actor = actor.cast();
            (*input).candidates.link_head = (*input).leaf_lights;
            (*input).candidates.actor_location = [(*actor).location.x, (*actor).location.y, (*actor).location.z];
            (*input).candidates.actor_special_lit = u32::from((*actor).b_special_lit());
            (*input).candidates.permeating = (*leaf).i_permeating;
        }
        Ok(())
    }
    fn candidate_ops(&mut self) -> &mut Self::Candidate {
        &mut self.candidate
    }
    fn tail_ops(&mut self) -> &mut Self::Tail { self }
}
struct CoreComputeOps<'a> {
    access: CoreAccess<'a>,
    graph: &'a RenderGraphState,
    clock_start: Option<u32>,
}

impl hp2_render::light_info::ComputeOps for CoreComputeOps<'_> {
    type Error = RuntimeError;

    fn global_hsv(&mut self, hue: u8, saturation: u8, out: &mut [f32; 4]) -> RuntimeResult<()> {
        let hsv = crate::tex::fget_hsv(hue, saturation, 255);
        out.copy_from_slice(&hsv);
        Ok(())
    }
    fn owner_time_seconds(&mut self, owner: *mut c_void) -> RuntimeResult<f32> {
        if owner.is_null() { return Err(RuntimeError::new("Canonical global lighting has no owner")); }
        let level = unsafe { (*owner.cast::<AActor>()).level.cast::<ALevelInfo>() };
        if level.is_null() { return Err(RuntimeError::new("Canonical light owner has no level")); }
        Ok(unsafe { (*level).time_seconds })
    }
    fn owner_life_fraction(&mut self, owner: *mut c_void) -> RuntimeResult<f32> {
        if owner.is_null() { return Err(RuntimeError::new("Canonical texture light has no owner")); }
        let class = unsafe { (*owner.cast::<hp2_abi::engine::generated::UObject>()).class.cast::<UClass>() };
        if class.is_null() || unsafe { (*class).defaults.data.is_null() } {
            return Err(RuntimeError::new("Canonical light owner has no class defaults"));
        }
        let default_actor = unsafe { (*class).defaults.data.cast::<AActor>() };
        let default_life = unsafe { (*default_actor).life_span as f32 };
        let life = 1.0 - unsafe { (*owner.cast::<AActor>()).life_span as f32 } / default_life;
        Ok(if life < 0.0 { 0.0 } else if life < 1.0 { life } else { 1.0 })
    }
    fn owner_palette(&mut self, owner: *mut c_void) -> RuntimeResult<*mut c_void> {
        if owner.is_null() { return Err(RuntimeError::new("Canonical texture light has no owner")); }
        let skin = unsafe { (*owner.cast::<AActor>()).skin.cast::<hp2_abi::engine::generated::UTexture>() };
        Ok(if skin.is_null() { ptr::null_mut() } else { unsafe { (*skin).base.palette } })
    }
    fn palette_color(&mut self, palette: *mut c_void, index: i32) -> RuntimeResult<(u8, u8, u8)> {
        if palette.is_null() || index < 0 { return Err(RuntimeError::new("Canonical light palette index is invalid")); }
        let colors = unsafe { &(*palette.cast::<UPalette>()).colors };
        if index >= colors.num || colors.data.is_null() {
            return Err(RuntimeError::new("Canonical light palette data is unavailable"));
        }
        let color = unsafe { &*colors.data.cast::<hp2_abi::FColor>().add(index as usize) };
        Ok((color.r, color.g, color.b))
    }
    fn world_light_radius(&mut self, actor: *mut c_void) -> RuntimeResult<f32> {
        if actor.is_null() { return Err(RuntimeError::new("Canonical light has no actor")); }
        Ok(crate::actor::world_light_radius(unsafe { &*actor.cast::<AActor>() }))
    }
    fn copy_effect(&mut self, effect: u32, out: *mut c_void) -> RuntimeResult<()> {
        if out.is_null() { return Err(RuntimeError::new("Canonical light effect output is null")); }
        unsafe { light_manager::copy_effect(effect, out); }
        Ok(())
    }
    fn view_vector(&mut self, actor: *mut c_void) -> RuntimeResult<[f32; 3]> {
        let vector = actor_view_vector(self.access, actor.cast())?;
        Ok([vector.x, vector.y, vector.z])
    }
    fn stat_clock(&mut self) -> RuntimeResult<()> {
        if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
            self.clock_start = Some(hp2_core::unix_clock::cycles());
        }
        Ok(())
    }
    fn stat_unclock(&mut self) -> RuntimeResult<()> {
        if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
            if let Some(start) = self.clock_start.take() {
                let elapsed = hp2_core::unix_clock::cycles().wrapping_sub(start).wrapping_sub(34) as i32;
                let mut stats = self.graph.stats.get();
                stats.extra_time = stats.extra_time.wrapping_add(elapsed);
                self.graph.stats.set(stats);
            }
        }
        Ok(())
    }
    fn cache_get(&mut self, id: u32, actor: *mut c_void) -> RuntimeResult<Option<*mut c_void>> {
        let id = actor_cache_id(id, actor)?;
        let data = cache_get(self.access, id)?;
        Ok((!data.is_null()).then_some(data))
    }
    fn cache_create(&mut self, id: u32, actor: *mut c_void, size: u32) -> RuntimeResult<*mut c_void> {
        let id = actor_cache_id(id, actor)?;
        let data = cache_create(self.access, id, u64::from(size), 0, false)?;
        if data.is_null() {
            return Err(RuntimeError::new("Canonical lighting cache allocation failed"));
        }
        Ok(data)
    }
    fn map_uncoords(&mut self, coords: &hp2_render::light_info::LightCoords) -> RuntimeResult<hp2_render::light_info::LightCoords> {
        Ok(surface_map_uncoords(coords))
    }
    fn world_volumetric_radius(&mut self, actor: *mut c_void) -> RuntimeResult<f32> {
        if actor.is_null() { return Err(RuntimeError::new("Canonical volumetric light has no actor")); }
        Ok(crate::actor::world_volumetric_radius(unsafe { &*actor.cast::<AActor>() }))
    }
}

fn surface_map_uncoords(coords: &hp2_render::light_info::LightCoords) -> hp2_render::light_info::LightCoords {
    let coords = FCoords {
        origin: FVector { x: coords.origin[0], y: coords.origin[1], z: coords.origin[2] },
        x_axis: FVector { x: coords.x_axis[0], y: coords.x_axis[1], z: coords.x_axis[2] },
        y_axis: FVector { x: coords.y_axis[0], y: coords.y_axis[1], z: coords.y_axis[2] },
        z_axis: FVector { x: coords.z_axis[0], y: coords.z_axis[1], z: coords.z_axis[2] },
    };
    let inverse = hp2_core::math::coords_inverse(&coords);
    light_coords(hp2_core::math::coords_transpose(inverse))
}

impl light_actor::CandidateOps for CoreComputeOps<'_> {
    type Error = RuntimeError;

    fn sender_alive(&mut self, entry: &light_actor::SenderEntry) -> bool {
        if entry.index < 0 || entry.index >= self.access.object_count() {
            return false;
        }
        let actor = self.access.object_at(entry.index);
        actor.cast::<c_void>() == entry.actor
            && !actor.is_null()
            && !unsafe { (*actor.cast::<AActor>()).b_delete_me() }
    }

    fn world_light_radius(&mut self, actor: *mut c_void) -> f32 {
        crate::actor::world_light_radius(unsafe { &*actor.cast::<AActor>() })
    }

    fn line_check(&mut self, actor: *mut c_void, light_actor: *mut c_void) -> RuntimeResult<bool> {
        if actor.is_null() || light_actor.is_null() {
            return Err(RuntimeError::new("Canonical light visibility check has a null actor"));
        }
        let manager = unsafe { light_manager::state() };
        let level = unsafe { (*manager).Level.cast::<ULevel>() };
        if level.is_null() || unsafe { (*level).model.is_null() } {
            return Err(RuntimeError::new("Canonical light visibility check has no level model"));
        }
        let end = unsafe { (*light_actor.cast::<AActor>()).location };
        let start = unsafe { (*actor.cast::<AActor>()).location };
        unsafe { crate::collision_owner::model_fast_line_check((*level).model.cast(), end, start) }
    }

    fn add_light(&mut self, actor: *mut c_void, light_actor: *mut c_void) -> RuntimeResult<()> {
        let _ = add_light(self.access, actor.cast(), light_actor.cast())?;
        Ok(())
    }

    fn validate_sender_count(&mut self, count: i32, capacity: usize) -> RuntimeResult<()> {
        if count < 0
            || count as usize > light_actor::MAX_ACTOR_LIGHTS
            || count as usize > capacity
            || capacity != light_actor::MAX_ACTOR_LIGHTS
        {
            return Err(RuntimeError::new("Canonical actor light sender count exceeds fixed source capacity"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod render_host_contracts {
    use super::*;

    #[test]
    fn pulsing_actor_glow_consumes_native_viewport_ftime() {
        use hp2_render::light_manager::ActorSetupOps;

        let owner = crate::render_owner::render_test_support::owner("actor-ftime-glow");
        let access = owner.access();
        let actor = crate::render_owner::render_test_support::construct(access, "Engine.Light")
            .cast::<AActor>();
        let zone = crate::render_owner::render_test_support::construct(access, "Engine.ZoneInfo")
            .cast::<AZoneInfo>();
        let mut viewport: UViewport = unsafe { std::mem::zeroed() };
        viewport.current_time = hp2_core::time::from_double(0.25).to_ne_bytes();
        let mut frame: hp2_abi::render_scene::FSceneNode = unsafe { std::mem::zeroed() };
        frame.viewport = (&mut viewport as *mut UViewport).cast();
        let graph = crate::render_owner::state(access).unwrap();
        graph.initialize_light_manager().unwrap();
        graph.with_light_manager(|| {
            let manager = unsafe { light_manager::state() };
            unsafe {
                (*manager).Frame = (&mut frame as *mut hp2_abi::render_scene::FSceneNode).cast();
                (*manager).Zone = zone.cast();
                (*actor).ambient_glow = 255;
                (*actor).scale_glow = 0.0;
                (*actor).specular_width = 85;
                (*zone).ambient_brightness = 0;
            }
            let mut setup = ActorSetup { access, graph: Rc::clone(&graph), active: false };
            let mut ops = CoreActorSetupOps {
                access, graph: &graph, setup: &mut setup,
                candidate: CoreComputeOps { access, graph: &graph, clock_start: None },
            };
            let mut input: hp2_render::light_manager::ActorInput = unsafe { std::mem::zeroed() };
            input.actor = actor.cast();
            ops.read_begin(&mut input)?;
            light_actor::actor_begin(&mut input.begin);
            // At t=1/4, the native pulsing branch is 0.25 + 0.2f*sin(2).
            assert_eq!(input.begin.ambient[0], 0.4318595);
            Ok::<(), RuntimeError>(())
        }).unwrap();
        owner.shutdown().unwrap();
    }

    #[test]
    fn pawn_spotlight_follows_view_rotation_instead_of_actor_rotation() {
        let owner = crate::render_owner::render_test_support::owner("view-light");
        let access = owner.access();
        let actor = crate::render_owner::render_test_support::construct(access, "Engine.Light")
            .cast::<AActor>();
        let pawn = crate::render_owner::render_test_support::construct(access, "Engine.Camera")
            .cast::<APawn>();
        unsafe {
            (*actor).rotation = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: -16384, roll: 0 };
            (*pawn).base.rotation = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 };
            (*pawn).view_rotation = hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 16384, roll: 0 };
        }
        let regular = actor_view_vector(access, actor).unwrap();
        let viewed = actor_view_vector(access, pawn.cast()).unwrap();
        assert!(regular.x.abs() < 0.00001 && regular.z.abs() < 0.00001);
        assert!((regular.y + 1.0).abs() < 0.00001);
        assert!(viewed.x.abs() < 0.00001 && viewed.z.abs() < 0.00001);
        assert!((viewed.y - 1.0).abs() < 0.00001);
        owner.shutdown().unwrap();
    }

    #[test]
    fn light_map_uncoords_transposes_inverse_with_non_symmetric_origin() {
        let map = hp2_render::light_info::LightCoords {
            origin: [2.0, 3.0, 5.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [1.0, 1.0, 0.0],
            z_axis: [0.0, 0.0, 1.0],
        };
        let result = surface_map_uncoords(&map);
        assert_eq!(result.origin, [-1.0, 4.0, 5.0]);
        assert_eq!(result.x_axis, [1.0, -1.0, 0.0]);
        assert_eq!(result.y_axis, [0.0, 1.0, 0.0]);
        assert_eq!(result.z_axis, [0.0, 0.0, 1.0]);
    }
}
