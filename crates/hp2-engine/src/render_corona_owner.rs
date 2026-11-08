use crate::core_owner::EngineState;
use hp2_abi::engine::generated::{AActor, APlayerPawn, FCacheItem, FCheckResult, FSceneNode, ULevel, UModel, UViewport};
use hp2_abi::render_scene::{FBspDrawList, FLeaf, FVolActorLink};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::scene_pipeline::draw::{DrawOperation, Hp2ScenePipelineDrawPacket};
use std::cell::Cell;
use std::ptr;

const CORONA_CACHE_ID: u32 = 0x27;
const CORONA_CACHE_BYTES: i32 = 32 * 16;
const TRACE_VIS_BLOCKING: u32 = 0x06;

pub(super) struct CoronaState<'a> {
    access: CoreAccess<'a>,
    locked_item: Cell<*mut FCacheItem>,
}

impl<'a> CoronaState<'a> {
    pub(super) fn new(access: CoreAccess<'a>) -> Self {
        Self { access, locked_item: Cell::new(ptr::null_mut()) }
    }
    /// Releases the cache item held by this draw scope and reports unlock failures.
    pub(super) fn finish(&self) -> RuntimeResult<()> {
        let item = self.locked_item.replace(ptr::null_mut());
        if item.is_null() {
            return Ok(());
        }
        let Some(state) = self.access.owner_context::<EngineState>() else {
            self.locked_item.set(item);
            return Err(RuntimeError::new("Corona cache unlock has no canonical Engine state"));
        };
        let mut cache = state.cache.borrow_mut();
        let Some(cache) = cache.as_mut() else {
            self.locked_item.set(item);
            return Err(RuntimeError::new("Canonical cache is unavailable during corona cleanup"));
        };
        if let Err(error) = unsafe { cache.unlock(self.access, item) } {
            self.locked_item.set(item);
            return Err(error);
        }
        Ok(())
    }


    pub(super) unsafe fn call(
        &self,
        _render: *mut hp2_abi::engine::generated::URender,
        frame: *mut FSceneNode,
        op: DrawOperation,
        packet: &mut Hp2ScenePipelineDrawPacket,
    ) -> RuntimeResult<()> {
        let state = self.access.owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Corona draw requires canonical Engine state"))?;
        match op {
            DrawOperation::CacheGet => {
                let mut cache = state.cache.borrow_mut();
                let cache = cache.as_mut().ok_or_else(|| RuntimeError::new("Canonical cache is unavailable"))?;
                let id = hp2_core::cache_id::frame(CORONA_CACHE_ID, 0);
                if let Some((data, item)) = unsafe { cache.get(self.access, id, 4) } {
                    self.locked_item.set(item);
                    packet.p[0] = data.cast();
                    packet.p[1] = item.cast();
                }
            }
            DrawOperation::CacheCreate => {
                let mut cache = state.cache.borrow_mut();
                let cache = cache.as_mut().ok_or_else(|| RuntimeError::new("Canonical cache is unavailable"))?;
                let id = hp2_core::cache_id::frame(CORONA_CACHE_ID, 0);
                // SAFETY: the source cache payload is 32 naturally aligned 16-byte Corona records.
                let (data, item) = unsafe { cache.create(self.access, id, CORONA_CACHE_BYTES, 4, 0)? };
                self.locked_item.set(item);
                packet.p[0] = data.cast();
                packet.p[1] = item.cast();
            }
            DrawOperation::CacheUnlock => {
                let item = packet.p[0].cast::<FCacheItem>();
                if item.is_null() { return Ok(()); }
                let mut cache = state.cache.borrow_mut();
                let cache = cache.as_mut().ok_or_else(|| RuntimeError::new("Canonical cache is unavailable"))?;
                // Keep the item tracked if unlock fails, so Drop can make a final release attempt.
                let tracked = self.locked_item.get() == item;
                if tracked { self.locked_item.set(ptr::null_mut()); }
                if let Err(error) = unsafe { cache.unlock(self.access, item) } {
                    if tracked { self.locked_item.set(item); }
                    return Err(error);
                }
            }
            DrawOperation::Now => {
                let time = hp2_core::time::from_double(hp2_core::unix_clock::seconds_slow());
                packet.d[0] = f64::from_bits(time as u64);
            }
            DrawOperation::Trace => {
                let actor = packet.p[0].cast::<AActor>();
                if actor.is_null() || frame.is_null() { return Err(RuntimeError::new("Invalid Corona trace arguments")); }
                let level = unsafe { (*frame).level.cast::<ULevel>() };
                let mut hit: FCheckResult = crate::levact::check_result(1.0);
                let clear = unsafe {
                    crate::level_trace_owner::single_line_check(
                        self.access, level, &mut hit, ptr::null_mut(), (*actor).location,
                        (*frame).coords.origin, TRACE_VIS_BLOCKING,
                        hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 }, 0,
                    )?
                };
                packet.n[0] = i32::from(clear);
            }
            DrawOperation::ActorIndex => {
                let actor = packet.p[0].cast::<AActor>();
                if actor.is_null() { return Err(RuntimeError::new("Invalid Corona actor index argument")); }
                let level = unsafe { (*actor).x_level.cast::<ULevel>() };
                let count = unsafe { crate::level_owner::actor_count(level)? };
                packet.n[0] = -1;
                for index in 0..count {
                    if unsafe { crate::level_owner::actor_at(level, index) } == actor {
                        packet.n[0] = index;
                        break;
                    }
                }
            }
            DrawOperation::CoronaSources => {
                if frame.is_null() { return Err(RuntimeError::new("Invalid Corona frame")); }
                let actor = unsafe { (*(*frame).viewport.cast::<UViewport>()).actor.cast::<AActor>() };
                if actor.is_null() { return Err(RuntimeError::new("Corona frame has no viewport actor")); }
                let level = unsafe { (*actor).x_level.cast::<ULevel>() };
                if level.is_null() { return Err(RuntimeError::new("Corona viewport actor has no Level")); }
                let model = unsafe { (*level).model.cast::<UModel>() };
                let leaf_index = unsafe { (*actor).region.i_leaf };
                if leaf_index < 0 { return Err(RuntimeError::new("Invalid Corona source leaf")); }
                if model.is_null() { return Err(RuntimeError::new("Invalid Corona source model")); }
                let leaves = unsafe { (*model).leaves };
                if leaves.data.is_null() || leaf_index >= leaves.num { return Err(RuntimeError::new("Corona source leaf is out of range")); }
                let leaf = unsafe { &*leaves.data.cast::<FLeaf>().add(leaf_index as usize) };
                packet.n[0] = leaf.i_permeating;
                packet.p[0] = ptr::addr_of!((*model).lights).cast_mut().cast();
                let graph = &state.render_graph;
                let leaf_lights = graph.leaf_lights.borrow();
                packet.p[1] = leaf_lights.get(leaf_index as usize).copied().unwrap_or(ptr::null_mut()).cast::<FVolActorLink>().cast();
            }
            DrawOperation::LevelActor => {
                if frame.is_null() { return Err(RuntimeError::new("Invalid Corona frame")); }
                let level = unsafe { (*frame).level.cast::<ULevel>() };
                if packet.n[1] == 0 {
                    packet.n[0] = unsafe { crate::level_owner::actor_count(level)? };
                } else if packet.n[1] == 1 {
                    let count = unsafe { crate::level_owner::actor_count(level)? };
                    if packet.n[0] >= 0 && packet.n[0] < count {
                        packet.p[0] = unsafe { crate::level_owner::actor_at(level, packet.n[0]) }.cast();
                    }
                }
            }
            DrawOperation::Lumos => {
                if hp2_abi::engine::generated::ENGINE_LUMOS_ENABLED != 0 {
                    unsafe { self.lumos(frame, packet)?; }
                }
            }
            DrawOperation::Debug => {
                match packet.n[0] {
                    0 => packet.n[0] = i32::from(state.render_graph.mover_debug_enabled()),
                    1 => {
                        packet.n[1] = state.render_graph.mover_lumos_logs.get();
                        state.render_graph.mover_lumos_logs.set(packet.n[1].wrapping_add(1));
                    }
                    _ => {}
                }
            }
            DrawOperation::Log => unsafe { self.log_mover_lumos(frame, packet)?; },
            DrawOperation::Flare => { /* LEGEND is never defined in this build; source case 37 is empty. */ }
            _ => return Err(RuntimeError::new("Unexpected operation delegated to Corona draw owner")),
        }
        Ok(())
    }

    unsafe fn lumos(&self, frame: *mut FSceneNode, packet: &mut Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        packet.n[1] = 1;
        packet.n[2] = i32::from(self.access.process_mode().is_editor);
        if frame.is_null() { return Err(RuntimeError::new("Invalid Lumos frame")); }
        let model = unsafe { (*(*frame).level.cast::<ULevel>()).model.cast::<UModel>() };
        let actor = unsafe { (*(*frame).viewport.cast::<UViewport>()).actor.cast::<AActor>() };
        match packet.n[5] {
            0 => {
                if model.is_null() { return Err(RuntimeError::new("Lumos frame has no model")); }
                let draw = packet.p[0].cast::<FBspDrawList>();
                let nodes = unsafe { (*model).nodes };
                if draw.is_null() || nodes.data.is_null() {
                    return Err(RuntimeError::new("Invalid Lumos BSP node"));
                }
                let node_index = unsafe { (*draw).i_node };
                if node_index < 0 || node_index >= nodes.array_num {
                    return Err(RuntimeError::new("Invalid Lumos BSP node"));
                }
                let node = unsafe { nodes.data.cast::<hp2_abi::render_scene::FBspNode>().add((*draw).i_node as usize) };
                let surf = unsafe { (*node).i_surf };
                let surfs = unsafe { (*model).surfs };
                if surf < 0 || surf >= surfs.array_num || surfs.data.is_null() { return Err(RuntimeError::new("Invalid Lumos BSP surface")); }
                packet.n[0] = unsafe { (*surfs.data.cast::<hp2_abi::render_scene::FBspSurf>().add(surf as usize)).poly_flags as i32 };
            }
            1 => {
                let class = self.access.native_class("Engine.PlayerPawn")?;
                packet.n[3] = i32::from(!actor.is_null() && unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) });
            }
            2 => {
                if packet.n[3] != 0 {
                    let pawn = actor.cast::<APlayerPawn>();
                    packet.n[4] = i32::from(unsafe { (*pawn).b_lumos_on() });
                    packet.f[0] = unsafe { (*pawn).f_lumos_radius };
                }
            }
            _ => {}
        }
        Ok(())
    }

    unsafe fn log_mover_lumos(&self, frame: *mut FSceneNode, packet: &Hp2ScenePipelineDrawPacket) -> RuntimeResult<()> {
        if frame.is_null() { return Err(RuntimeError::new("Invalid Lumos log frame")); }
        let model = unsafe { (*(*frame).level.cast::<ULevel>()).model.cast::<UModel>() };
        let draw = packet.p[0].cast::<FBspDrawList>();
        if model.is_null() || draw.is_null() { return Err(RuntimeError::new("Invalid Lumos log surface")); }
        let nodes = unsafe { (*model).nodes };
        let node_index = unsafe { (*draw).i_node };
        if nodes.data.is_null() || node_index < 0 || node_index >= nodes.array_num { return Err(RuntimeError::new("Invalid Lumos log BSP node")); }
        let node = unsafe { nodes.data.cast::<hp2_abi::render_scene::FBspNode>().add(node_index as usize) };
        let surf = unsafe { (*node).i_surf };
        let surfs = unsafe { (*model).surfs };
        if surfs.data.is_null() || surf < 0 || surf >= surfs.array_num { return Err(RuntimeError::new("Invalid Lumos log surface index")); }
        let flags = unsafe { (*surfs.data.cast::<hp2_abi::render_scene::FBspSurf>().add(surf as usize)).poly_flags };
        let actor = unsafe { (*(*frame).viewport.cast::<UViewport>()).actor.cast::<APlayerPawn>() };
        let lumos_on = !actor.is_null() && unsafe { (*actor).b_lumos_on() };
        let occlude = packet.n[0] & 0x8000_0000_u32 as i32 != 0;
        let message = format!("MoverLumosDraw: surf {surf} flags=0x{flags:08x} alpha={} lumosOn={} occlude={}", packet.n[1], i32::from(lumos_on), i32::from(occlude));
        let category = self.access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
        self.access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())
    }
}

impl Drop for CoronaState<'_> {
    fn drop(&mut self) {
        let item = self.locked_item.replace(ptr::null_mut());
        if item.is_null() { return; }
        if let Some(state) = self.access.owner_context::<EngineState>() {
            if let Some(cache) = state.cache.borrow_mut().as_mut() {
                // SAFETY: item is the still-locked entry retained from CacheGet/Create; Drop runs before cache teardown.
                let _ = unsafe { cache.unlock(self.access, item) };
            }
        }
    }
}
