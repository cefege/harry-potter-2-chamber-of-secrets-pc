//! Callback-free Core operations for ordinary `UMesh` rendering.

use super::super::super::URender;
use crate::mesh;
use hp2_abi::engine::generated::{
    self as generated, AActor, AZoneInfo, FSceneNode, FTextureInfo, UClient, UMesh, UTexture, UViewport,
};
use hp2_abi::render_particle::FTransTexture;
use hp2_abi::render_scene::FDynamicSprite;
use hp2_abi::render_span::FMemMark;
use hp2_abi::{FCoords, FPlane};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::mesh_lod_pipeline::{LodLightVertex, LodTexture};
use hp2_render::mesh_pipeline::{self, MeshDrawLive, MeshDrawOps, MeshDrawState, MeshWireDraw};
use hp2_render::mesh_shared::{self, MeshBatchDraw, MeshSharedGeometry, MeshSharedOps, MeshTextureInfo};
use std::{cell::RefCell, ffi::c_void, mem, ptr};

struct CoreMeshDrawOps<'a> {
    access: CoreAccess<'a>,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
    actor: *mut AActor,
    mesh: *mut UMesh,
    coords: *const FCoords,
    device: *mut generated::URenderDevice,
    viewport: *mut UViewport,
    client: *mut UClient,
    live: *mut MeshDrawLive,
    setup_flags: u32,
    error: RefCell<Option<RuntimeError>>,
    light_setup: RefCell<Option<crate::render_owner::lighting::ActorSetup<'a>>>,
    mesh_started: RefCell<Option<u32>>,
    frame_started: RefCell<Option<u32>>,
    process_started: RefCell<Option<u32>>,
    clip_started: RefCell<Option<u32>>,
    tmap_started: RefCell<Option<u32>>,
}

impl CoreMeshDrawOps<'_> {
    fn record(&self, error: RuntimeError) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() { *slot = Some(error); }
    }

    fn result<T>(&self, result: RuntimeResult<T>) -> Result<T, ()> {
        result.map_err(|error| self.record(error))
    }

    fn fail<T>(&self, message: &'static str) -> Result<T, ()> {
        self.record(RuntimeError::new(message));
        Err(())
    }

    fn fail_runtime<T>(&self, error: RuntimeError) -> Result<T, ()> {
        self.record(error);
        Err(())
    }

    fn take_error(&self) -> Option<RuntimeError> { self.error.borrow_mut().take() }

    fn refresh(&mut self) {
        unsafe {
            let selected = (*self.actor).b_selected();
            let live = &mut *self.live;
            live.selected = i32::from(selected);
            let viewport_actor = (*self.viewport).actor.cast::<generated::APlayerPawn>();
            let rend_map = if viewport_actor.is_null() { -1 } else { (*viewport_actor).rend_map };
            live.wire = i32::from(matches!(rend_map, 1 | 13..=15));
            live.span_based = (*self.device).span_based as i32;
            live.texture_count = (*self.mesh).textures.num;
            live.world_lod = (*self.client).texture_lod_set[generated::ENGINE_LODSET_WORLD as usize];
            live.recommended_lod = (*self.device).recommended_lod;
        }
    }

    fn texture_ptr(&self, index: i32) -> Result<*mut UTexture, ()> {
        let texture_count = unsafe { (*self.live).texture_count };
        if index < 0 || index >= texture_count { return self.fail("Mesh texture index is out of range"); }
        unsafe {
            let multi = if index < 8 {
                ptr::addr_of!((*self.actor).multi_skins).cast::<*mut c_void>().add(index as usize).read_unaligned()
            } else { ptr::null_mut() };
            if !multi.is_null() { return Ok(multi.cast()); }
            let array = &(*self.mesh).textures;
            if array.data.is_null() || array.num <= index { return self.fail("UMesh texture array is invalid"); }
            let mesh_texture = array.data.cast::<*mut c_void>().add(index as usize).read_unaligned();
            if index != 0 && !mesh_texture.is_null() { return Ok(mesh_texture.cast()); }
            if !(*self.actor).skin.is_null() { return Ok((*self.actor).skin.cast()); }
            Ok(mesh_texture.cast())
        }
    }

    fn texture_info_ptr(index: i32) -> *mut FTextureInfo {
        unsafe { ptr::addr_of_mut!((*mesh_shared::storage()).texture_info).cast::<FTextureInfo>().add(index as usize) }
    }

    fn environment_info_ptr() -> *mut FTextureInfo {
        unsafe { ptr::addr_of_mut!((*mesh_shared::storage()).environment_info).cast() }
    }

    unsafe fn samples_view(&self) -> RuntimeResult<(mesh::MeshView<'_>, mesh::OwnerView)> {
        unsafe { super::ordinary_views(self.mesh, self.actor) }
    }

    fn stat_add(&self, field: unsafe fn(*mut generated::FStats) -> *mut i32, amount: i32) {
        if generated::ENGINE_BUILD_STATS == 0 { return; }
        let Some(engine) = self.access.owner_context::<crate::core_owner::EngineState>() else { return; };
        let stats = engine.render_graph.stats_ptr();
        if !stats.is_null() {
            // SAFETY: the active engine owns mutable FStats on this render thread.
            unsafe { let slot = field(stats); slot.write_unaligned(slot.read_unaligned().wrapping_add(amount)); }
        }
    }

    fn stat_start(&self, field: unsafe fn(*mut generated::FStats) -> *mut i32) -> Option<u32> {
        if generated::ENGINE_BUILD_STATS == 0 { return None; }
        let engine = self.access.owner_context::<crate::core_owner::EngineState>()?;
        let stats = engine.render_graph.stats_ptr();
        if stats.is_null() { return None; }
        let slot = unsafe { field(stats) };
        let now = hp2_core::unix_clock::cycles();
        unsafe { slot.write_unaligned(slot.read_unaligned().wrapping_sub(now as i32)); }
        Some(now)
    }

    fn stat_finish(&self, field: unsafe fn(*mut generated::FStats) -> *mut i32, start: &RefCell<Option<u32>>) {
        if let Some(began) = start.borrow_mut().take() {
            let elapsed = hp2_core::unix_clock::cycles().wrapping_sub(began).wrapping_sub(34) as i32;
            if let Some(engine) = self.access.owner_context::<crate::core_owner::EngineState>() {
                let stats = engine.render_graph.stats_ptr();
                if !stats.is_null() {
                    // SAFETY: the active engine owns mutable FStats on this render thread.
                    unsafe { let slot = field(stats); slot.write_unaligned(slot.read_unaligned().wrapping_add(elapsed)); }
                }
            }
        }
    }

    fn stat_finish_start(&self, field: unsafe fn(*mut generated::FStats) -> *mut i32, started: Option<u32>) {
        if let Some(began) = started {
            let elapsed = hp2_core::unix_clock::cycles().wrapping_sub(began).wrapping_sub(34) as i32;
            if let Some(engine) = self.access.owner_context::<crate::core_owner::EngineState>() {
                let stats = engine.render_graph.stats_ptr();
                if !stats.is_null() {
                    unsafe { let slot = field(stats); slot.write_unaligned(slot.read_unaligned().wrapping_add(elapsed)); }
                }
            }
        }
    }
}

unsafe fn raw_array<T>(array: *const generated::TArrayRaw, message: &'static str) -> RuntimeResult<(*const T, usize)> {
    if array.is_null() { return Err(RuntimeError::new(message)); }
    unsafe {
        let array = &*array;
        if array.num < 0 || array.max < array.num || (array.num != 0 && array.data.is_null()) {
            return Err(RuntimeError::new(message));
        }
        Ok((array.data.cast(), array.num as usize))
    }
}

fn stat_mesh() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_time) } }
fn stat_frame() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_get_frame_time) } }
fn stat_process() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_process_time) } }
fn stat_light_setup() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_light_setup_time) } }
fn stat_clip() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_clip_time) } }
fn stat_tmap() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_tmap_time) } }

impl MeshSharedOps for CoreMeshDrawOps<'_> {
    type Error = ();

    unsafe fn allocate_bytes(
        &mut self,
        memory: *mut c_void,
        bytes: usize,
        alignment: i32,
        _slow_checks: bool,
        memory_error: *mut i32,
    ) -> Result<*mut c_void, ()> {
        if memory.is_null() { return self.fail("Ordinary mesh draw has no canonical memory stack"); }
        let Some(engine) = self.access.owner_context::<crate::core_owner::EngineState>() else {
            return self.fail("Ordinary mesh draw has no Engine owner");
        };
        let size = i32::try_from(bytes).map_err(|_| {
            self.record(RuntimeError::new("Ordinary mesh allocation is too large"));
        })?;
        match unsafe { engine.memory.push_bytes(size, alignment) } {
            Ok(ptr) => Ok(ptr.cast()),
            Err(error) => {
                if !memory_error.is_null() { unsafe { *memory_error = 1; } }
                self.fail_runtime(error)
            }
        }
    }

    fn batch_capacity(&mut self) -> Result<usize, ()> {
        self.result(unsafe { crate::render_device_owner::max_vertices(self.access, self.device) })
            .map(|count| count.max(0) as usize)
    }

    fn draw_triangles(&mut self, batch: &mut MeshBatchDraw) -> Result<(), ()> {
        self.result(unsafe {
            crate::render_device_owner::draw_triangles(
                self.access,
                self.device,
                batch.frame.cast(),
                batch.texture.cast(),
                batch.vertices.cast::<*mut generated::FTransTexture>().cast(),
                batch.vertex_count,
                batch.indices,
                batch.index_count,
                batch.flags,
                batch.span,
            )
        })
    }

    fn subsurface_begin(&mut self) -> Result<(), ()> {
        *self.clip_started.borrow_mut() = self.stat_start(stat_clip());
        Ok(())
    }
    fn surface_ready(&mut self) -> Result<(), ()> {
        self.stat_finish(stat_clip(), &self.clip_started);
        Ok(())
    }
    fn surface_timer_begin(&mut self) -> Result<(), ()> {
        *self.tmap_started.borrow_mut() = self.stat_start(stat_tmap());
        Ok(())
    }
    fn surface_timer_end(&mut self) -> Result<(), ()> {
        self.stat_finish(stat_tmap(), &self.tmap_started);
        Ok(())
    }
    fn check_allocation_size(&mut self, bytes: &mut usize) -> Result<(), ()> {
        if *bytes > i32::MAX as usize { self.fail("Ordinary mesh allocation size exceeds engine limits") } else { Ok(()) }
    }
}

impl MeshDrawOps for CoreMeshDrawOps<'_> {
    unsafe fn memory_mark(&mut self, memory: *mut c_void) -> Result<FMemMark, ()> {
        if memory.is_null() { return self.fail("Ordinary mesh draw has no canonical memory stack"); }
        let mut mark = mem::MaybeUninit::uninit();
        unsafe { hp2_core::mem::mark_init(mark.as_mut_ptr(), memory.cast()); Ok(mark.assume_init()) }
    }
    unsafe fn memory_pop(&mut self, mark: &FMemMark) { unsafe { hp2_core::mem::mark_pop(mark); } }

    fn begin_mesh(&mut self) -> Result<(), ()> {
        *self.mesh_started.borrow_mut() = self.stat_start(stat_mesh());
        self.refresh();
        Ok(())
    }

    fn transform_samples(&mut self, _wire: bool, samples: *mut FTransTexture, count: i32) -> Result<(), ()> {
        *self.frame_started.borrow_mut() = self.stat_start(stat_frame());
        let (view, owner) = match unsafe { self.samples_view() } {
            Ok(view) => view,
            Err(error) => return self.fail_runtime(error),
        };
        if count < 0 || count != view.frame_verts || (samples.is_null() && count != 0) {
            return self.fail("Invalid ordinary mesh vertex allocation");
        }
        let table = hp2_core::math::trig_table();
        let sin = |angle| hp2_core::math::sin_tab_inline(angle, table);
        let cos = |angle| hp2_core::math::cos_tab_inline(angle, table);
        let trig = crate::coords::Trig { sin: &sin, cos: &cos };
        let engine = match self.access.owner_context::<crate::core_owner::EngineState>() {
            Some(engine) => engine,
            None => return self.fail("Ordinary mesh draw has no Engine owner"),
        };
        let mut cache_owner = match engine.cache.try_borrow_mut() {
            Ok(owner) => owner,
            Err(_) => return self.fail("Canonical mesh cache is already borrowed"),
        };
        let Some(cache) = cache_owner.as_mut() else { return self.fail("Canonical mesh cache is unavailable"); };
        let actor_index = unsafe { (*self.actor).index };
        let mut cache = CoreFrameCache { access: self.access, cache, error: None, fallback: Vec::new() };
        let key = mesh::CacheKey {
            id: hp2_core::cache_id::object(0x1d, actor_index as u32, 0),
            mesh: self.mesh as usize as u64,
        };
        mesh::get_frame(&view, &owner, unsafe { *self.coords }, &trig, &mut cache, key, &mut |index, point| unsafe {
            (*samples.add(index)).transform.point = point;
        });
        if let Some(error) = cache.error.take() { return self.fail_runtime(error); }
        self.stat_finish(stat_frame(), &self.frame_started);
        Ok(())
    }

    fn samples_ready(&mut self, count: i32) -> Result<(), ()> {
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_vert_count) }, count);
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_sub_count) }, count);
        Ok(())
    }

    fn draw_wire(&mut self, line: &mut MeshWireDraw) -> Result<(), ()> {
        self.result(unsafe {
            crate::render_device_owner::draw_3d_line(
                self.access, self.device, self.frame, line.color, line.flags, line.a, line.b,
            )
        })
    }
    fn finish_wire(&mut self) -> Result<(), ()> {
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_count) }, 1);
        self.stat_finish(stat_mesh(), &self.mesh_started);
        Ok(())
    }

    fn initialize_lighting(&mut self) -> Result<(), ()> {
        *self.process_started.borrow_mut() = self.stat_start(stat_process());
        let started = self.stat_start(stat_light_setup());
        let result = unsafe {
            crate::render_owner::lighting::setup_actor(
                self.access,
                self.frame,
                self.actor,
                (*self.sprite).leaf_lights,
                (*self.sprite).volumetrics,
            )
        };
        match result {
            Ok((flags, scope)) => {
                self.setup_flags = flags;
                *self.light_setup.borrow_mut() = Some(scope);
                self.stat_finish_start(stat_light_setup(), started);
                Ok(())
            }
            Err(error) => {
                self.stat_finish_start(stat_light_setup(), started);
                self.fail_runtime(error)
            }
        }
    }

    fn visibility_ready(&mut self) -> Result<(), ()> {
        self.stat_finish(stat_process(), &self.process_started);
        Ok(())
    }

    fn begin_textures(&mut self) -> Result<(), ()> {
        let count = unsafe { (*self.live).texture_count };
        if !(0..=16).contains(&count) { return self.fail("UMesh texture count exceeds shared mesh storage"); }
        Ok(())
    }

    fn texture_info(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        texture.object = self.texture_ptr(index)?;
        texture.info = Self::texture_info_ptr(index);
        unsafe { (*mesh_shared::storage()).textures[index as usize] = texture.object.cast(); }
        Ok(())
    }

    fn lock_texture(&mut self, _index: i32) -> Result<(), ()> {
        *self.tmap_started.borrow_mut() = self.stat_start(stat_tmap());
        Ok(())
    }

    fn texture_lod(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        let time = unsafe { i64::from_ne_bytes((*self.viewport).current_time) };
        let texture_ptr = self.result(unsafe { crate::texture_owner::get(self.access, texture.object, time) })?;
        texture.object = texture_ptr;
        let lod_set = unsafe { (*texture_ptr).lod_set as usize };
        if lod_set >= 8 { return self.fail("Texture LOD set index is out of range"); }
        texture.lod = unsafe { (*self.client).texture_lod_set[lod_set] };
        let _ = index;
        Ok(())
    }

    fn resolve_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        let info = Self::texture_info_ptr(index);
        let time = unsafe { i64::from_ne_bytes((*self.viewport).current_time) };
        self.result(unsafe { crate::texture_owner::lock(self.access, texture.object, info, time, texture.lod, self.device) })?;
        unsafe {
            let shared = &*info.cast::<MeshTextureInfo>();
            texture.info = info;
            texture.u_scale = shared.u_scale;
            texture.v_scale = shared.v_scale;
            texture.u_size = shared.u_size;
            texture.v_size = shared.v_size;
            (*mesh_shared::storage()).textures[index as usize] = texture.object.cast();
        }
        Ok(())
    }

    fn bind_environment(&mut self, environment: &mut *mut UTexture) -> Result<(), ()> {
        if environment.is_null() { return self.fail("Mesh environment texture is null"); }
        let time = unsafe { i64::from_ne_bytes((*self.viewport).current_time) };
        let texture = self.result(unsafe { crate::texture_owner::get(self.access, *environment, time) })?;
        let info = Self::environment_info_ptr();
        self.result(unsafe { crate::texture_owner::lock(self.access, texture, info, time, -1, self.device) })?;
        *environment = texture;
        Ok(())
    }

    fn environment_flags(&mut self, flags: &mut u32) -> Result<(), ()> {
        if self.light_setup.borrow().is_none() { return self.fail("Mesh environment flags require actor lighting"); }
        *flags |= self.setup_flags;
        Ok(())
    }

    fn environment_maps(&mut self, owner: *const AActor) -> Result<mesh_pipeline::MeshEnvironmentMaps, ()> {
        if owner.is_null() { return self.fail("Mesh environment lookup has no owner"); }
        unsafe {
            let actor = &*owner;
            let zone = actor.region.zone.cast::<AZoneInfo>();
            let level = actor.level.cast::<AZoneInfo>();
            Ok(mesh_pipeline::MeshEnvironmentMaps {
                texture: actor.texture.cast(),
                zone: if zone.is_null() { ptr::null_mut() } else { (*zone).environment_map.cast() },
                level: if level.is_null() { ptr::null_mut() } else { (*level).environment_map.cast() },
            })
        }
    }

    fn light_vertex(&mut self, vertex: &mut LodLightVertex) -> Result<(), ()> {
        let sample = unsafe { vertex.sample };
        if sample.is_null() { return self.fail("Mesh light vertex has no sample"); }
        let mut fog = unsafe {
            hp2_render::light::FogVertex {
                point: (*sample).transform.point,
                normal: hp2_abi::FVector { x: (*sample).normal.x, y: (*sample).normal.y, z: (*sample).normal.z },
                light: [(*sample).light.x, (*sample).light.y, (*sample).light.z, (*sample).light.w],
                fog: [(*sample).fog.x, (*sample).fog.y, (*sample).fog.z, (*sample).fog.w],
            }
        };
        let lit = self.light_and_fog(&mut fog, vertex.flags);
        unsafe {
            (*sample).light = FPlane { x: fog.light[0], y: fog.light[1], z: fog.light[2], w: fog.light[3] };
            (*sample).fog = FPlane { x: fog.fog[0], y: fog.fog[1], z: fog.fog[2], w: fog.fog[3] };
        }
        if !lit && self.error.borrow().is_some() { Err(()) } else { Ok(()) }
    }

    fn light_and_fog(&mut self, vertex: &mut hp2_render::light::FogVertex, flags: u32) -> bool {
        match crate::render_owner::lighting::light_and_fog(self.access, vertex, flags) {
            Ok(lit) => lit,
            Err(error) => { self.record(error); false }
        }
    }

    fn begin_polygons(&mut self, visible: i32) -> Result<(), ()> {
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_poly_count) }, visible);
        Ok(())
    }

    fn finish_polygons(&mut self) -> Result<(), ()> {
        self.stat_finish(stat_process(), &self.process_started);
        if let Some(scope) = self.light_setup.borrow_mut().take() {
            self.result(crate::render_owner::lighting::finish_actor(self.access, scope))?;
        }
        Ok(())
    }

    fn unlock_texture(&mut self, index: i32) -> Result<(), ()> {
        let texture = unsafe { (*mesh_shared::storage()).textures[index as usize] }.cast::<UTexture>();
        if !texture.is_null() {
            self.result(unsafe { crate::texture_owner::unlock(self.access, texture, Self::texture_info_ptr(index).cast()) })?;
        }
        Ok(())
    }

    fn end_mesh(&mut self) -> Result<(), ()> {
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_count) }, 1);
        self.stat_finish(stat_mesh(), &self.mesh_started);
        Ok(())
    }

    fn texture_transition_complete(&mut self) -> Result<(), ()> {
        self.stat_finish(stat_tmap(), &self.tmap_started);
        Ok(())
    }

    fn unbind_environment(&mut self, environment: &mut *mut UTexture) -> Result<(), ()> {
        if !(*environment).is_null() {
            self.result(unsafe { crate::texture_owner::unlock(self.access, *environment, Self::environment_info_ptr()) })?;
        }
        *environment = ptr::null_mut();
        Ok(())
    }
}

pub(super) struct CoreFrameCache<'a, 'b> {
    access: CoreAccess<'a>,
    cache: &'b mut hp2_core::cache_owner::CacheOwner,
    error: Option<RuntimeError>,
    fallback: Vec<u8>,
}

impl<'a, 'b> CoreFrameCache<'a, 'b> {
    pub(super) fn new(access: CoreAccess<'a>, cache: &'b mut hp2_core::cache_owner::CacheOwner) -> Self {
        Self { access, cache, error: None, fallback: Vec::new() }
    }

    pub(super) fn take_error(&mut self) -> Option<RuntimeError> { self.error.take() }

    fn fallback(&mut self, len: usize) -> mesh::CacheBlock {
        self.fallback.resize(len, 0);
        mesh::CacheBlock { ptr: self.fallback.as_mut_ptr(), len, item: ptr::null_mut() }
    }
}

impl mesh::FrameCache for CoreFrameCache<'_, '_> {
    fn get(&mut self, id: u64, len: usize) -> Option<mesh::CacheBlock> {
        match unsafe { self.cache.get(self.access, id, 16) } {
            Some((ptr, item)) => Some(mesh::CacheBlock { ptr, len, item: item.cast() }),
            None => None,
        }
    }
    fn create(&mut self, id: u64, len: usize) -> mesh::CacheBlock {
        let Ok(size) = i32::try_from(len) else {
            self.error = Some(RuntimeError::new("Mesh frame cache allocation is too large"));
            return self.fallback(len);
        };
        match unsafe { self.cache.create(self.access, id, size, 16, 0) } {
            Ok((data, item)) if !data.is_null() && !item.is_null() => {
                mesh::CacheBlock { ptr: data, len, item: item.cast() }
            }
            Ok(_) => {
                self.error = Some(RuntimeError::new("Mesh frame cache create returned null storage"));
                self.fallback(len)
            }
            Err(error) => { self.error = Some(error); self.fallback(len) }
        }
    }
    fn flush(&mut self, id: u64) {
        if let Err(error) = self.cache.flush(self.access, id, u32::MAX, 0) { self.error = Some(error); }
    }
    fn unlock(&mut self, block: mesh::CacheBlock) {
        if block.item.is_null() { return; }
        if let Err(error) = unsafe { self.cache.unlock(self.access, block.item.cast()) } { self.error = Some(error); }
    }
}

unsafe fn lazy_slots(
    lazy: *mut c_void,
    offset: usize,
    loader_offset: usize,
    saved_archive: *mut *mut c_void,
    saved_position: *mut i32,
) -> hp2_core::lazy_array::LazyArraySlots {
    let bytes = lazy.cast::<u8>();
    unsafe {
        hp2_core::lazy_array::LazyArraySlots {
            array: bytes.add(offset).cast(),
            loader: bytes.add(loader_offset).cast(),
            saved_archive,
            saved_position,
        }
    }
}

unsafe fn load_mesh_arrays(access: CoreAccess<'_>, mesh: *mut UMesh) -> RuntimeResult<()> {
    if mesh.is_null() { return Err(RuntimeError::new("Ordinary mesh draw requires a UMesh")); }
    unsafe {
        let verts = ptr::addr_of_mut!((*mesh).verts);
        let tris = ptr::addr_of_mut!((*mesh).tris);
        let connects = ptr::addr_of_mut!((*mesh).connects);
        let links = ptr::addr_of_mut!((*mesh).vert_links);
        for slots in [
            lazy_slots(ptr::addr_of_mut!((*mesh).verts).cast(), generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET, generated::TLazyMeshVerts::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*verts).saved_ar), ptr::addr_of_mut!((*verts).saved_pos)),
            lazy_slots(ptr::addr_of_mut!((*mesh).tris).cast(), generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET, generated::TLazyMeshTris::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*tris).saved_ar), ptr::addr_of_mut!((*tris).saved_pos)),
            lazy_slots(ptr::addr_of_mut!((*mesh).connects).cast(), generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET, generated::TLazyMeshConnects::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*connects).saved_ar), ptr::addr_of_mut!((*connects).saved_pos)),
            lazy_slots(ptr::addr_of_mut!((*mesh).vert_links).cast(), generated::TLazyMeshLinks::TARRAY_INT_OFFSET, generated::TLazyMeshLinks::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*links).saved_ar), ptr::addr_of_mut!((*links).saved_pos)),
        ] {
            hp2_core::lazy_array::lazy_array_load(access, slots)?;
        }
    }
    Ok(())
}

/// Draws the ordinary `UMesh` path through typed Core leaves and shared geometry policy.
///
/// # Safety
/// All pointers must be live canonical renderer, frame, sprite, actor, and mesh records.
pub(super) unsafe fn draw_ordinary_mesh(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
    actor: *mut AActor,
    coords: *const FCoords,
    flags: u32,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() || sprite.is_null() || actor.is_null() || coords.is_null() {
        return Err(RuntimeError::new("Invalid canonical ordinary mesh draw arguments"));
    }
    let mesh = unsafe { (*actor).mesh.cast::<UMesh>() };
    unsafe { load_mesh_arrays(access, mesh)?; }
    let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
    if viewport.is_null() { return Err(RuntimeError::new("Ordinary mesh draw has no viewport")); }
    let device = unsafe { (*viewport).ren_dev.cast::<generated::URenderDevice>() };
    if device.is_null() { return Err(RuntimeError::new("Ordinary mesh draw has no render device")); }
    let engine_record = unsafe { (*render).base.engine.cast::<generated::UEngine>() };
    if engine_record.is_null() { return Err(RuntimeError::new("Ordinary mesh draw has no Engine record")); }
    let client = unsafe { (*engine_record).client.cast::<UClient>() };
    if client.is_null() { return Err(RuntimeError::new("Ordinary mesh draw has no Engine client")); }
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Ordinary mesh draw has no Engine owner"))?;
    let memory = engine.memory.as_ptr().cast::<c_void>();
    let mut live = MeshDrawLive {
        selected: 0, wire: 0, span_based: 0, texture_count: 0, world_lod: 0,
        recommended_lod: 0, rust_light_manager: 1,
    };
    let mut ops = CoreMeshDrawOps {
        access, frame, sprite, actor, mesh, coords, device, viewport, client,
        live: &mut live, setup_flags: 0, error: RefCell::new(None),
        light_setup: RefCell::new(None), mesh_started: RefCell::new(None), frame_started: RefCell::new(None),
        process_started: RefCell::new(None), clip_started: RefCell::new(None), tmap_started: RefCell::new(None),
    };
    ops.refresh();
    let mut memory_error = 0;
    let mut error_stage = 0;
    let editor = i32::from(access.process_mode().is_editor);
    let shared = MeshSharedGeometry {
        frame: frame.cast(),
        span: unsafe { (*sprite).span_buffer.cast() },
        clip: [
            unsafe { ptr::addr_of!((*frame).prj_xm) }, unsafe { ptr::addr_of!((*frame).prj_xp) },
            unsafe { ptr::addr_of!((*frame).prj_ym) }, unsafe { ptr::addr_of!((*frame).prj_yp) },
        ],
        projection: unsafe { ptr::addr_of!((*frame).proj.z) },
        screen: [
            unsafe { ptr::addr_of!((*frame).fx15) }, unsafe { ptr::addr_of!((*frame).fy15) },
            unsafe { ptr::addr_of!((*frame).fx) }, unsafe { ptr::addr_of!((*frame).fy) },
        ],
        near_clip: unsafe { frame.cast::<u8>().add(56).cast() },
        uncoords: unsafe { ptr::addr_of!((*frame).uncoords) },
    };
    let triangles = array_view(unsafe { ptr::addr_of!((*mesh).tris).cast::<u8>().add(generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET).cast() });
    let connects = array_view(unsafe { ptr::addr_of!((*mesh).connects).cast::<u8>().add(generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET).cast() });
    let links = array_view(unsafe { ptr::addr_of!((*mesh).vert_links).cast::<u8>().add(generated::TLazyMeshLinks::TARRAY_INT_OFFSET).cast() });
    let stats = engine.render_graph.stats_ptr();
    let mirror = unsafe { frame.cast::<u8>().add(52).cast::<f32>() };
    let state = MeshDrawState {
        shared,
        memory,
        memory_error: &mut memory_error,
        alignment: 8,
        slow_checks: cfg!(debug_assertions),
        live: &live,
        error_stage: &mut error_stage,
        frame_verts: unsafe { ptr::addr_of!((*mesh).frame_verts) },
        triangles,
        connects,
        links,
        not_weapon: i32::from(unsafe { (*actor).owner != (*viewport).actor }),
        editor: &editor,
        scale_glow: unsafe { ptr::addr_of!((*actor).scale_glow) },
        ambient_glow: unsafe { ptr::addr_of!((*actor).ambient_glow) },
        fatness: unsafe { ptr::addr_of!((*actor).fatness) },
        mirror,
        flags,
        light_count_stat: if generated::ENGINE_BUILD_STATS == 0 || stats.is_null() {
            ptr::null_mut()
        } else {
            unsafe { ptr::addr_of_mut!((*stats).mesh_vert_light_count) }
        },
        owner: actor.cast(),
    };
    let result = unsafe { mesh_pipeline::draw_with_ops(&state, &mut ops) };
    if let Some(error) = ops.take_error() { return Err(error); }
    result.map_err(|()| RuntimeError::new("Canonical ordinary mesh draw failed"))
}

fn array_view(array: *const generated::TArrayRaw) -> hp2_render::mesh_lod_pipeline::LodArrayView {
    unsafe {
        hp2_render::mesh_lod_pipeline::LodArrayView {
            data: ptr::addr_of!((*array).data).cast(),
            len: ptr::addr_of!((*array).num),
        }
    }
}
