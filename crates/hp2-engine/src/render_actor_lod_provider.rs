//! Callback-free Core operations for canonical `ULodMesh` rendering.

use super::super::super::URender;
use hp2_abi::engine::generated::{
    self as generated, AActor, CFSkelHeader, FMeshAnimSeq, FTextureInfo, FSceneNode, MotionChunk,
    TArrayRaw, UAnimation, UCanvas, UClient, ULodMesh, ULevel, USkeletalMesh, UTexture, UViewport,
    VBoneInfIndex, VBoneInfluence,
};
use hp2_abi::render_particle::FTransTexture;
use hp2_abi::render_scene::FDynamicSprite;
use hp2_abi::render_span::FMemMark;
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_render::mesh_lod_pipeline::{
    self, LodAllocation, LodAnimationKind, LodAnimationOwner, LodArrayView, LodDrawOps,
    LodDrawState, LodDebug, LodLightVertex, LodLine, LodLiveState, LodTexture,
};
use hp2_render::mesh_shared::{self, MeshBatchDraw, MeshSharedGeometry, MeshSharedOps, MeshTextureInfo};
use std::{cell::{Cell, RefCell}, ffi::{c_void, CStr}, fmt::{self, Write as _}, mem, ptr, sync::Mutex};

static SKEL_LINKUP: Mutex<()> = Mutex::new(());
impl CoreSkelHost<'_, '_> {
    fn array<T>(raw: &TArrayRaw) -> &[T] {
        if raw.num <= 0 || raw.data.is_null() { &[] }
        else { unsafe { std::slice::from_raw_parts(raw.data.cast::<T>(), raw.num as usize) } }
    }

    fn set_array<T>(raw: *mut TArrayRaw, values: &mut Vec<T>) {
        unsafe {
            (*raw).data = values.as_mut_ptr().cast();
            (*raw).num = values.len() as i32;
            (*raw).max = values.len() as i32;
        }
    }
}

impl crate::skeletal::SkelHost for CoreSkelHost<'_, '_> {
    fn sin_tab(&self, index: i32) -> f32 {
        hp2_core::math::sin_tab_inline(index, hp2_core::math::trig_table())
    }

    fn cos_tab(&self, index: i32) -> f32 {
        hp2_core::math::cos_tab_inline(index, hp2_core::math::trig_table())
    }

    fn cache_get(&self, owner: *const AActor, item: &mut *mut c_void) -> crate::skeletal::SkelResult<*mut u8> {
        let mut cache = self.engine.cache.try_borrow_mut()
            .map_err(|_| self.record(RuntimeError::new("Skeletal cache is already borrowed")))?;
        let Some(cache) = cache.as_mut() else { return Err(self.record(RuntimeError::new("Skeletal cache is unavailable"))); };
        match unsafe { cache.get(self.access, Self::cache_id(owner), 16) } {
            Some((data, cache_item)) => { *item = cache_item.cast(); Ok(data) }
            None => { *item = ptr::null_mut(); Ok(ptr::null_mut()) }
        }
    }

    fn cache_create(
        &self,
        owner: *const AActor,
        bytes: usize,
        item: &mut *mut c_void,
    ) -> crate::skeletal::SkelResult<*mut u8> {
        let Ok(size) = i32::try_from(bytes) else {
            return Err(self.record(RuntimeError::new("Skeletal cache allocation exceeds engine limits")));
        };
        let mut cache = self.engine.cache.try_borrow_mut()
            .map_err(|_| self.record(RuntimeError::new("Skeletal cache is already borrowed")))?;
        let Some(cache) = cache.as_mut() else { return Err(self.record(RuntimeError::new("Skeletal cache is unavailable"))); };
        match unsafe { cache.create(self.access, Self::cache_id(owner), size, 16, 0) } {
            Ok((data, cache_item)) => { *item = cache_item.cast(); Ok(data) }
            Err(error) => Err(self.record(error)),
        }
    }

    fn cache_flush(&self, owner: *const AActor) -> crate::skeletal::SkelResult<()> {
        let mut cache = self.engine.cache.try_borrow_mut()
            .map_err(|_| self.record(RuntimeError::new("Skeletal cache is already borrowed")))?;
        let Some(cache) = cache.as_mut() else { return Err(self.record(RuntimeError::new("Skeletal cache is unavailable"))); };
        cache.flush(self.access, Self::cache_id(owner), u32::MAX, 0).map_err(|error| self.record(error))
    }

    fn cache_unlock(&self, item: *mut c_void) -> crate::skeletal::SkelResult<()> {
        if item.is_null() { return Ok(()); }
        let mut cache = self.engine.cache.try_borrow_mut()
            .map_err(|_| self.record(RuntimeError::new("Skeletal cache is already borrowed")))?;
        let Some(cache) = cache.as_mut() else { return Err(self.record(RuntimeError::new("Skeletal cache is unavailable"))); };
        unsafe { cache.unlock(self.access, item.cast()) }.map_err(|error| self.record(error))
    }

    fn warn_missing_link(&self, mesh: *const USkeletalMesh, bone: usize) -> crate::skeletal::SkelResult<()> {
        let result = (|| {
            let mesh = unsafe { &*mesh };
            let bones = Self::array::<generated::FMeshBone>(&mesh.ref_skeleton);
            let name = bones.get(bone)
                .map(|entry| self.access.name_text(i32::from_ne_bytes(entry.name)))
                .transpose()?;
            let category = self.access.intern_str("Warning", hp2_core::name_owner::FindName::Add)?;
            let mut message = "Warning: missing bone/animation link".chars().map(u32::from).collect::<Vec<_>>();
            if let Some(name) = name {
                message.extend(" for ".chars().map(u32::from));
                message.extend(name.into_iter().take_while(|unit| *unit != 0));
            }
            self.access.emit_log(category, &message)
        })();
        result.map_err(|error| self.record(error))
    }

    fn remove_aux(
        &self,
        owner: *mut AActor,
        index: usize,
        channel: *mut AActor,
    ) -> crate::skeletal::SkelResult<()> {
        let result = (|| {
            let level = unsafe { (*owner).x_level.cast::<ULevel>() };
            unsafe { crate::level_lifecycle_owner::destroy_actor(self.access, level, channel, false)?; }
            let array = unsafe { ptr::addr_of_mut!((*owner).aux_anims) };
            let num = unsafe { (*array).num };
            if index >= num.max(0) as usize {
                return Err(RuntimeError::new("Skeletal auxiliary animation index is out of range"));
            }
            unsafe {
                let data = (*array).data.cast::<*mut AActor>();
                ptr::copy(data.add(index + 1), data.add(index), num as usize - index - 1);
                (*array).num -= 1;
            }
            Ok(())
        })();
        result.map_err(|error| self.record(error))
    }

    fn debug_pivots(&self, mesh: *mut USkeletalMesh, bases: *const FCoords, num: usize) -> crate::skeletal::SkelResult<()> {
        self.remember_debug_arrays(mesh);
        let result = (|| {
            let ref_skeleton = unsafe { ptr::read(ptr::addr_of!((*mesh).ref_skeleton)) };
            let bones = Self::array::<generated::FMeshBone>(&ref_skeleton);
            if num > bones.len() { return Err(RuntimeError::new("Skeletal pivot count exceeds reference skeleton")); }
            let mut pivots = self.debug_pivots.borrow_mut();
            let mut parents = self.debug_parents.borrow_mut();
            pivots.clear(); parents.clear();
            pivots.reserve(num);
            parents.reserve(num);
            for i in 0..num {
                pivots.push(unsafe { (*bases.add(i)).origin });
                parents.push(bones[i].parent_index);
            }
            Self::set_array(unsafe { ptr::addr_of_mut!((*mesh).debug_pivots) }, &mut pivots);
            Self::set_array(unsafe { ptr::addr_of_mut!((*mesh).debug_parents) }, &mut parents);
            Ok(())
        })();
        result.map_err(|error| self.record(error))
    }

    fn debug_influences(&self, mesh: *mut USkeletalMesh) -> crate::skeletal::SkelResult<()> {
        self.remember_debug_arrays(mesh);
        let result = (|| {
            let frame_verts = unsafe { (*mesh).frame_verts };
            let bone_weight_idx = unsafe { ptr::read(ptr::addr_of!((*mesh).bone_weight_idx)) };
            let bone_weights = unsafe { ptr::read(ptr::addr_of!((*mesh).bone_weights)) };
            let count = usize::try_from(frame_verts.max(0))
                .map_err(|_| RuntimeError::new("Invalid skeletal frame vertex count"))?;
            let indices = Self::array::<VBoneInfIndex>(&bone_weight_idx);
            let weights = Self::array::<VBoneInfluence>(&bone_weights);
            let mut influences = self.debug_influences.borrow_mut();
            influences.clear();
            influences.resize(count, 0);
            for index in indices {
                let start = usize::from(index.weight_index);
                let end = start.saturating_add(usize::from(index.number)).min(weights.len());
                for weight in weights.get(start..end).unwrap_or_default() {
                    let point = usize::from(weight.point_index);
                    if weight.bone_weight != 0 {
                        if let Some(total) = influences.get_mut(point) { *total = total.saturating_add(1); }
                    }
                }
            }
            Self::set_array(unsafe { ptr::addr_of_mut!((*mesh).debug_verts) }, &mut influences);
            Ok(())
        })();
        result.map_err(|error| self.record(error))
    }

    fn fail_check(&self, expr: &CStr) -> crate::skeletal::SkelResult<()> {
        Err(self.record(RuntimeError::new(&format!("Skeletal mesh assertion failed: {}", expr.to_string_lossy()))))
    }

    fn animation_sequence(
        &self,
        anim: *mut UAnimation,
        name: [u8; 4],
    ) -> crate::skeletal::SkelResult<*const FMeshAnimSeq> {
        if anim.is_null() { return Ok(ptr::null()); }
        let sequences = Self::array::<FMeshAnimSeq>(unsafe { &(*anim).anim_seqs });
        Ok(sequences.iter().find(|sequence| sequence.name == name)
            .map_or(ptr::null(), |sequence| sequence as *const FMeshAnimSeq))
    }

    fn animation_movement(
        &self,
        anim: *mut UAnimation,
        name: [u8; 4],
    ) -> crate::skeletal::SkelResult<*const MotionChunk> {
        if anim.is_null() { return Ok(ptr::null()); }
        let animation = unsafe { &*anim };
        let sequences = Self::array::<FMeshAnimSeq>(&animation.anim_seqs);
        let Some(index) = sequences.iter().position(|sequence| sequence.name == name) else { return Ok(ptr::null()); };
        let movements = Self::array::<MotionChunk>(&animation.moves);
        Ok(movements.get(index).map_or(ptr::null(), ptr::from_ref))
    }

    fn with_cache<R>(
        &self,
        owner: *const AActor,
        body: impl FnOnce(*mut CFSkelHeader) -> crate::skeletal::SkelResult<R>,
    ) -> crate::skeletal::SkelResult<R> {
        let mut item = ptr::null_mut();
        let cache = self.cache_get(owner, &mut item)?;
        let result = body(cache.cast());
        let unlock = self.cache_unlock(item);
        match result { Err(error) => { let _ = unlock; Err(error) }, Ok(value) => { unlock?; Ok(value) } }
    }

    fn with_linkup<R>(&self, body: impl FnOnce() -> crate::skeletal::SkelResult<R>) -> crate::skeletal::SkelResult<R> {
        let _guard = SKEL_LINKUP.lock()
            .map_err(|_| self.record(RuntimeError::new("Skeletal link-up lock is poisoned")))?;
        body()
    }

    fn with_bases<R>(
        &self,
        num: usize,
        body: impl FnOnce(*mut FCoords) -> crate::skeletal::SkelResult<R>,
    ) -> crate::skeletal::SkelResult<R> {
        unsafe { self.with_temp(num, mem::size_of::<FCoords>(), false, |ptr| body(ptr.cast())) }
    }

    fn with_vertices<R>(
        &self,
        num: usize,
        body: impl FnOnce(*mut FVector) -> crate::skeletal::SkelResult<R>,
    ) -> crate::skeletal::SkelResult<R> {
        unsafe { self.with_temp(num, mem::size_of::<FVector>(), true, |ptr| body(ptr.cast())) }
    }
}
/// Builds the one-shot skeletal host used by particle mesh queries.
fn particle_skel_host<'a, 'b>(
    access: CoreAccess<'a>,
    engine: &'a crate::core_owner::EngineState,
    error: &'b RefCell<Option<RuntimeError>>,
    debug_pivots: &'b RefCell<Vec<FVector>>,
    debug_parents: &'b RefCell<Vec<i32>>,
    debug_influences: &'b RefCell<Vec<u8>>,
    debug_original: &'b RefCell<Option<[TArrayRaw; 3]>>,
) -> CoreSkelHost<'a, 'b> {
    CoreSkelHost { access, engine, error, debug_pivots, debug_parents, debug_influences, debug_original }
}

/// `USkeletalMesh::GetRenderBoundingBox(Owner, 0)` for the canonical particle system.
pub(super) unsafe fn skeletal_bounds(
    access: CoreAccess<'_>,
    mesh: *mut USkeletalMesh,
    actor: *mut AActor,
) -> RuntimeResult<FCoords> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Particle skeletal bounds have no Engine owner"))?;
    let error = RefCell::new(None);
    let pivots = RefCell::new(Vec::new());
    let parents = RefCell::new(Vec::new());
    let influences = RefCell::new(Vec::new());
    let original = RefCell::new(None);
    let host = particle_skel_host(access, engine, &error, &pivots, &parents, &influences, &original);
    let result = unsafe { crate::skeletal::get_render_bounding_box(&host, mesh, actor) };
    match (result, error.into_inner()) {
        (Ok(coords), None) => Ok(coords),
        (_, Some(error)) => Err(error),
        (Err(error), None) => Err(RuntimeError::new(&format!(
            "USkeletalMesh::GetRenderBoundingBox failed ({})", error.0,
        ))),
    }
}

/// Runs a canonical skeletal-mesh query with the particle-style Core cache host.
fn with_particle_skel_host<R>(
    access: CoreAccess<'_>,
    label: &'static str,
    body: impl FnOnce(&CoreSkelHost<'_, '_>) -> crate::skeletal::SkelResult<R>,
) -> RuntimeResult<R> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new(label))?;
    let error = RefCell::new(None);
    let pivots = RefCell::new(Vec::new());
    let parents = RefCell::new(Vec::new());
    let influences = RefCell::new(Vec::new());
    let original = RefCell::new(None);
    let host = particle_skel_host(access, engine, &error, &pivots, &parents, &influences, &original);
    let result = body(&host);
    match (result, error.into_inner()) {
        (Ok(value), None) => Ok(value),
        (_, Some(error)) => Err(error),
        (Err(error), None) => Err(RuntimeError::new(&format!("{label} failed ({})", error.0))),
    }
}

/// `USkeletalMesh::GetRootMovement(Owner)` for canonical actor ticking.
pub(crate) unsafe fn skeletal_root_movement(
    access: CoreAccess<'_>,
    mesh: *mut USkeletalMesh,
    owner: *mut AActor,
) -> RuntimeResult<FVector> {
    with_particle_skel_host(access, "USkeletalMesh::GetRootMovement failed", |host| {
        unsafe { crate::skeletal::get_root_movement(host, mesh, owner) }
    })
}

/// `USkeletalMesh::AdjustRootMovement(Owner, Adjust)` for canonical actor ticking.
pub(crate) unsafe fn skeletal_adjust_root_movement(
    access: CoreAccess<'_>,
    mesh: *mut USkeletalMesh,
    owner: *mut AActor,
    adjust: FVector,
) -> RuntimeResult<()> {
    with_particle_skel_host(access, "USkeletalMesh::AdjustRootMovement failed", |host| {
        unsafe { crate::skeletal::adjust_root_movement(host, owner, adjust) }
    })
}

/// `USkeletalMesh::GetBoneCoords(Owner, Bone)` for canonical physics.
pub(crate) unsafe fn skeletal_bone_coords(
    access: CoreAccess<'_>,
    mesh: *mut USkeletalMesh,
    owner: *mut AActor,
    bone: i32,
) -> RuntimeResult<FCoords> {
    with_particle_skel_host(access, "USkeletalMesh::GetBoneCoords failed", |host| {
        unsafe { crate::skeletal::get_bone_coords(host, mesh, owner, bone) }
    })
}

/// `USkeletalMesh::GetFrame` into a `FVector` array for the canonical particle system.
pub(super) unsafe fn skeletal_frame(
    access: CoreAccess<'_>,
    mesh: *mut USkeletalMesh,
    actor: *mut AActor,
    coords: FCoords,
    points: &mut [FVector],
) -> RuntimeResult<()> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Particle skeletal frame has no Engine owner"))?;
    let error = RefCell::new(None);
    let pivots = RefCell::new(Vec::new());
    let parents = RefCell::new(Vec::new());
    let influences = RefCell::new(Vec::new());
    let original = RefCell::new(None);
    let host = particle_skel_host(access, engine, &error, &pivots, &parents, &influences, &original);
    let mut requested = points.len() as i32;
    let result = unsafe {
        crate::skeletal::get_frame(
            &host,
            mesh,
            points.as_mut_ptr().cast::<u8>(),
            mem::size_of::<FVector>() as i32,
            coords,
            actor,
            &mut requested,
        )
    };
    match (result, error.into_inner()) {
        (Ok(()), None) => Ok(()),
        (_, Some(error)) => Err(error),
        (Err(error), None) => Err(RuntimeError::new(&format!(
            "USkeletalMesh::GetFrame failed ({})", error.0,
        ))),
    }
}

struct CoreSkelHost<'a, 'b> {
    access: CoreAccess<'a>,
    engine: &'a crate::core_owner::EngineState,
    error: &'b RefCell<Option<RuntimeError>>,
    debug_pivots: &'b RefCell<Vec<FVector>>,
    debug_parents: &'b RefCell<Vec<i32>>,
    debug_influences: &'b RefCell<Vec<u8>>,
    debug_original: &'b RefCell<Option<[TArrayRaw; 3]>>,
}

impl CoreSkelHost<'_, '_> {
    fn record(&self, error: RuntimeError) -> crate::skeletal::SkelError {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() { *slot = Some(error); }
        crate::skeletal::SkelError(1)
    }

    fn cache_id(owner: *const AActor) -> u64 {
        unsafe { hp2_core::cache_id::object(0x34, (*owner).index as u32, 0) }
    }

    unsafe fn with_temp<R>(
        &self,
        num: usize,
        stride: usize,
        clear: bool,
        body: impl FnOnce(*mut u8) -> crate::skeletal::SkelResult<R>,
    ) -> crate::skeletal::SkelResult<R> {
        let Some(bytes) = num.checked_mul(stride) else {
            return Err(self.record(RuntimeError::new("Skeletal temporary array size overflow")));
        };
        let Ok(size) = i32::try_from(bytes) else {
            return Err(self.record(RuntimeError::new("Skeletal temporary array exceeds engine limits")));
        };
        let mark = unsafe { self.engine.memory.mark() };
        struct Pop<'a> { memory: &'a hp2_core::mem_owner::MemStackOwner, mark: FMemMark }
        impl Drop for Pop<'_> { fn drop(&mut self) { unsafe { self.memory.pop(&self.mark); } } }
        let _pop = Pop { memory: &self.engine.memory, mark };
        let ptr = match unsafe { self.engine.memory.push_bytes(size, 16) } {
            Ok(ptr) => ptr.cast::<u8>(),
            Err(error) => return Err(self.record(error)),
        };
        if clear && !ptr.is_null() { unsafe { ptr::write_bytes(ptr, 0, bytes); } }
        body(ptr)
    }

    fn remember_debug_arrays(&self, mesh: *mut USkeletalMesh) {
        let mut originals = self.debug_original.borrow_mut();
        if originals.is_none() {
            unsafe {
                *originals = Some([
                    ptr::read(ptr::addr_of!((*mesh).debug_pivots)),
                    ptr::read(ptr::addr_of!((*mesh).debug_parents)),
                    ptr::read(ptr::addr_of!((*mesh).debug_verts)),
                ]);
            }
        }
    }
}

struct CoreLodDrawOps<'a> {
    access: CoreAccess<'a>,
    render: *mut URender,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
    actor: *mut AActor,
    mesh: *mut ULodMesh,
    viewport: *mut UViewport,
    device: *mut generated::URenderDevice,
    client: *mut UClient,
    state: *mut LodDrawState,
    error: RefCell<Option<RuntimeError>>,
    light_setup: RefCell<Option<crate::render_owner::lighting::ActorSetup<'a>>>,
    texture_started: RefCell<Option<u32>>,
    mesh_started: RefCell<Option<u32>>,
    draw_started: RefCell<Option<u32>>,
    frame_started: RefCell<Option<u32>>,
    light_started: RefCell<Option<u32>>,
    light_setup_started: RefCell<Option<u32>>,
    animation_owner: Cell<*mut AActor>,
    environment: *mut UTexture,
    locked_textures: u16,
    debug_pivots: RefCell<Vec<FVector>>,
    debug_parents: RefCell<Vec<i32>>,
    debug_influences: RefCell<Vec<u8>>,
    debug_original: RefCell<Option<[TArrayRaw; 3]>>,
}
impl Drop for CoreLodDrawOps<'_> {
    fn drop(&mut self) {
        if let Some([pivots, parents, influences]) = self.debug_original.get_mut().take() {
            unsafe {
                ptr::write(ptr::addr_of_mut!((*self.mesh.cast::<USkeletalMesh>()).debug_pivots), pivots);
                ptr::write(ptr::addr_of_mut!((*self.mesh.cast::<USkeletalMesh>()).debug_parents), parents);
                ptr::write(ptr::addr_of_mut!((*self.mesh.cast::<USkeletalMesh>()).debug_verts), influences);
            }
        }
    }
}

impl CoreLodDrawOps<'_> {
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

    fn stat_add(&self, field: unsafe fn(*mut generated::FStats) -> *mut i32, amount: i32) {
        if generated::ENGINE_BUILD_STATS == 0 { return; }
        let Some(engine) = self.access.owner_context::<crate::core_owner::EngineState>() else { return; };
        let stats = engine.render_graph.stats_ptr();
        if !stats.is_null() {
            // SAFETY: the active Engine owns its mutable statistics record on the render thread.
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
                if !stats.is_null() { unsafe { let slot = field(stats); slot.write_unaligned(slot.read_unaligned().wrapping_add(elapsed)); } }
            }
        }
    }

    fn take_error(&self) -> Option<RuntimeError> { self.error.borrow_mut().take() }

    fn array<T>(&self, raw: *const generated::TArrayRaw, label: &'static str) -> RuntimeResult<(*mut T, usize)> {
        if raw.is_null() { return Err(RuntimeError::new(label)); }
        // SAFETY: each pointer addresses a live TArray header in the draw mesh.
        let raw = unsafe { &*raw };
        if raw.num < 0 || raw.max < raw.num || (raw.num != 0 && raw.data.is_null()) {
            return Err(RuntimeError::new(label));
        }
        Ok((raw.data.cast(), raw.num as usize))
    }

    fn texture_info_ptr(index: usize) -> *mut FTextureInfo {
        unsafe { ptr::addr_of_mut!((*mesh_shared::storage()).texture_info).cast::<FTextureInfo>().add(index) }
    }

    fn environment_info_ptr() -> *mut FTextureInfo {
        unsafe { ptr::addr_of_mut!((*mesh_shared::storage()).environment_info).cast() }
    }

    fn stat_result<T>(&self, result: RuntimeResult<T>) -> Result<T, ()> { self.result(result) }

    unsafe fn mesh_view(&self) -> RuntimeResult<(crate::mesh::MeshView<'_>, crate::mesh::OwnerView)> {
        unsafe { super::lod_views(self.mesh, self.actor) }
    }

    fn mesh_texture_ptr(&self, index: i32) -> Result<*mut UTexture, ()> {
        let state = unsafe { &*self.state };
        let live = unsafe { &*state.live };
        if index < 0 || index >= live.texture_count { return self.fail("LOD mesh texture index is out of range"); }
        // Matches UMesh::GetTexture's per-actor multi-skin precedence for indexed materials.
        unsafe {
            let multi = ptr::addr_of!((*self.actor).multi_skins).cast::<*mut c_void>().add(index as usize).read_unaligned();
            if !multi.is_null() { return Ok(multi.cast()); }
            let texture = (*self.mesh).base.textures.data.cast::<*mut c_void>().add(index as usize).read_unaligned();
            if index != 0 && !texture.is_null() { return Ok(texture.cast()); }
            if !(*self.actor).skin.is_null() { return Ok((*self.actor).skin.cast()); }
            Ok(texture.cast())
        }
    }

    fn finish_lighting(&mut self) -> Result<(), ()> {
        if let Some(setup) = self.light_setup.borrow_mut().take() {
            self.stat_result(crate::render_owner::lighting::finish_actor(self.access, setup))?;
        }
        Ok(())
    }
}

fn stat_mesh() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_time) } }
fn stat_get_frame() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_get_frame_time) } }
fn stat_process() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_process_time) } }
fn stat_light_setup() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_light_setup_time) } }
fn stat_light() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_light_time) } }
fn stat_tmap() -> unsafe fn(*mut generated::FStats) -> *mut i32 { |s| unsafe { ptr::addr_of_mut!((*s).mesh_tmap_time) } }

impl MeshSharedOps for CoreLodDrawOps<'_> {
    type Error = ();

    unsafe fn allocate_bytes(&mut self, memory: *mut c_void, bytes: usize, alignment: i32, slow_checks: bool, memory_error: *mut i32) -> Result<*mut c_void, ()> {
        if memory.is_null() { return self.fail("LOD mesh has no canonical memory stack"); }
        let size = i32::try_from(bytes).map_err(|_| { self.record(RuntimeError::new("LOD mesh allocation exceeds engine limits")); () })?;
        let _ = (memory, slow_checks);
        let engine = self.access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| { self.record(RuntimeError::new("LOD mesh allocation has no Engine owner")); () })?;
        match unsafe { engine.memory.push_bytes(size, alignment) } {
            Ok(ptr) => Ok(ptr.cast()),
            Err(error) => {
                if !memory_error.is_null() { unsafe { *memory_error = 1; } }
                self.record(error);
                Err(())
            }
        }
    }

    fn batch_capacity(&mut self) -> Result<usize, ()> {
        self.stat_result(unsafe { crate::render_device_owner::max_vertices(self.access, self.device) }).map(|n| n.max(0) as usize)
    }

    fn draw_triangles(&mut self, batch: &mut MeshBatchDraw) -> Result<(), ()> {
        self.stat_result(unsafe { crate::render_device_owner::draw_triangles(self.access, self.device, batch.frame.cast(), batch.texture.cast(), batch.vertices.cast::<*mut generated::FTransTexture>().cast(), batch.vertex_count, batch.indices, batch.index_count, batch.flags, batch.span) })
    }

    fn subsurface_begin(&mut self) -> Result<(), ()> { Ok(()) }
    fn surface_ready(&mut self) -> Result<(), ()> { Ok(()) }
    fn surface_timer_begin(&mut self) -> Result<(), ()> { self.texture_started.replace(self.stat_start(stat_tmap())); Ok(()) }
    fn surface_timer_end(&mut self) -> Result<(), ()> { self.stat_finish(stat_tmap(), &self.texture_started); Ok(()) }
    fn check_allocation_size(&mut self, bytes: &mut usize) -> Result<(), ()> {
        if *bytes > i32::MAX as usize { self.fail("LOD mesh allocation exceeds engine limits") } else { Ok(()) }
    }
}

impl LodDrawOps for CoreLodDrawOps<'_> {
    fn record_vertex_subset(&mut self, count: i32) -> Result<(), ()> {
        let started = self.stat_start(stat_mesh());
        self.mesh_started.replace(started);
        self.draw_started.replace(started);
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_vert_count) }, unsafe { (*self.mesh).model_verts });
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_sub_count) }, count);
        Ok(())
    }

    fn enable_skeletal_debug(&mut self) -> Result<(), ()> {
        if self.state.is_null() { return self.fail("LOD draw has no live state"); }
        unsafe {
            let skeletal = self.mesh.cast::<USkeletalMesh>();
            (*skeletal).display_bones = (*self.render).bone_show as i32;
            (*skeletal).display_influences = (*self.render).blend_show as i32;
        }
        Ok(())
    }

    fn allocate_vertices(&mut self, count: i32) -> Result<*mut FTransTexture, ()> {
        if count < 0 { return self.fail("LOD mesh vertex count is negative"); }
        let bytes = (count as usize).checked_mul(mem::size_of::<FTransTexture>()).ok_or_else(|| { self.record(RuntimeError::new("LOD mesh vertex allocation overflow")); () })?;
        if bytes > i32::MAX as usize { return self.fail("LOD mesh vertex allocation exceeds engine limits"); }
        let engine = self.access.owner_context::<crate::core_owner::EngineState>().ok_or_else(|| { self.record(RuntimeError::new("LOD mesh has no Engine owner")); () })?;
        // SAFETY: the enclosing draw mark outlives all vertices allocated here.
        let ptr = unsafe { engine.memory.push_bytes(bytes as i32, 16) }.map_err(|e| { self.record(e); () })?;
        if !ptr.is_null() { unsafe { ptr::write_bytes(ptr.cast::<FTransTexture>(), 0, count as usize); } }
        Ok(ptr.cast())
    }

    fn transform_vertices(&mut self, allocation: &mut LodAllocation, coordinate_mode: mesh_lod_pipeline::LodCoordinateMode) -> Result<(), ()> {
        *self.frame_started.borrow_mut() = self.stat_start(stat_get_frame());
        if allocation.count < 0 || allocation.data.is_null() && allocation.count != 0 { return self.fail("Invalid LOD mesh vertex allocation"); }
        let coords = if coordinate_mode == mesh_lod_pipeline::LodCoordinateMode::Unit {
            hp2_core::math::unit_coords()
        } else {
            unsafe { *self.state.as_ref().ok_or(())?.coords }
        };
        if unsafe { self.state.as_ref().is_some_and(|state| state.skeletal != 0) } {
            let engine = self.access.owner_context::<crate::core_owner::EngineState>()
                .ok_or_else(|| { self.record(RuntimeError::new("Skeletal mesh draw has no Engine owner")); () })?;
            let host = CoreSkelHost {
                access: self.access,
                engine,
                error: &self.error,
                debug_pivots: &self.debug_pivots,
                debug_parents: &self.debug_parents,
                debug_influences: &self.debug_influences,
                debug_original: &self.debug_original,
            };
            let mut requested = allocation.count;
            let result = unsafe {
                crate::skeletal::get_frame(
                    &host,
                    self.mesh.cast::<USkeletalMesh>(),
                    allocation.data.cast(),
                    mem::size_of::<FTransTexture>() as i32,
                    coords,
                    self.actor,
                    &mut requested,
                )
            };
            allocation.count = requested;
            if let Err(error) = result {
                if self.error.borrow().is_none() {
                    self.record(RuntimeError::new(&format!("USkeletalMesh::GetFrame failed ({})", error.0)));
                }
                self.stat_finish(stat_get_frame(), &self.frame_started);
                return Err(());
            }
            self.stat_finish(stat_get_frame(), &self.frame_started);
            return Ok(());
        }
        let (mesh_view, owner) = match unsafe { self.mesh_view() } { Ok(view) => view, Err(error) => return { self.record(error); Err(()) } };
        let lod = crate::lodmesh::LodView { base: mesh_view, special_verts: unsafe { (*self.mesh).special_verts }, lod_request: allocation.count };
        let trig_table = hp2_core::math::trig_table();
        let sin = |angle| hp2_core::math::sin_tab_inline(angle, trig_table);
        let cos = |angle| hp2_core::math::cos_tab_inline(angle, trig_table);
        let trig = crate::coords::Trig { sin: &sin, cos: &cos };
        let engine = self.access.owner_context::<crate::core_owner::EngineState>().ok_or_else(|| { self.record(RuntimeError::new("LOD mesh has no Engine owner")); () })?;
        let mut cache = engine.cache.try_borrow_mut().map_err(|_| { self.record(RuntimeError::new("Canonical LOD mesh cache is already borrowed")); () })?;
        let Some(cache) = cache.as_mut() else { return self.fail("Canonical LOD mesh cache is unavailable"); };
        let mut frame_cache = CoreFrameCache { access: self.access, cache, error: None, fallback: None };
        let key = crate::mesh::CacheKey { id: hp2_core::cache_id::object(0x30, unsafe { (*self.actor).index as u32 }, 0), mesh: self.mesh as usize as u64 };
        let result = crate::lodmesh::get_frame(&lod, &owner, coords, &trig, &mut frame_cache, key, &mut |index, point| unsafe {
            if index < allocation.count as usize { (*allocation.data.add(index)).transform.point = point; }
        });
        allocation.count = result;
        if let Some(error) = frame_cache.error.take() { return { self.record(error); Err(()) }; }
        self.stat_finish(stat_get_frame(), &self.frame_started);
        Ok(())
    }

    fn skeletal_debug(&mut self, debug: &mut LodDebug) -> Result<(), ()> {
        unsafe {
            let skel = self.mesh.cast::<USkeletalMesh>();
            debug.pivots = (*skel).debug_pivots.data.cast();
            debug.parents = (*skel).debug_parents.data.cast();
            debug.pivot_count = (*skel).debug_pivots.num;
            debug.influences = array_view(ptr::addr_of!((*skel).debug_verts));
            debug.weapon_bone = (*skel).weapon_bone_index;
            debug.weapon_coords = (*skel).classic_weapon_coords;
        }
        Ok(())
    }

    fn draw_line(&mut self, line: &LodLine) -> Result<(), ()> {
        self.stat_result(unsafe { crate::render_device_owner::draw_3d_line(self.access, self.device, self.frame, line.color, line.flags, line.a, line.b) })
    }
    fn draw_screen_line(&mut self, line: &LodLine) -> Result<(), ()> {
        self.stat_result(unsafe { crate::render_device_owner::draw_2d_line(self.access, self.device, self.frame, line.color, line.flags, line.a, line.b) })
    }
    fn finish_wireframe(&mut self) -> Result<(), ()> { self.stat_finish(stat_mesh(), &self.mesh_started); Ok(()) }
    fn begin_material_batch(&mut self) -> Result<(), ()> {
        if unsafe { (*self.mesh).base.textures.num } > 16 { return self.fail("LOD mesh has more than 16 textures"); }
        self.texture_started.replace(self.stat_start(stat_tmap()));
        Ok(())
    }
    fn mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        let object = self.mesh_texture_ptr(index)?;
        texture.object = object;
        texture.info = Self::texture_info_ptr(index as usize);
        unsafe { (*mesh_shared::storage()).textures[index as usize] = object.cast(); }
        Ok(())
    }
    fn resolve_mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        let time = unsafe { i64::from_ne_bytes((*self.viewport).current_time) };
        let object = self.stat_result(unsafe { crate::texture_owner::get(self.access, texture.object, time) })?;
        texture.object = object;
        let lod_set = unsafe { (*object).lod_set as usize };
        if lod_set >= unsafe { (*self.client).texture_lod_set.len() } { return self.fail("Texture LOD set index is out of range"); }
        texture.lod = unsafe { (*self.client).texture_lod_set[lod_set] };
        unsafe { (*mesh_shared::storage()).textures[index as usize] = object.cast(); }
        Ok(())
    }
    fn lock_mesh_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        if !(0..16).contains(&index) { return self.fail("LOD mesh texture index is out of range"); }
        let info = Self::texture_info_ptr(index as usize);
        let time = unsafe { i64::from_ne_bytes((*self.viewport).current_time) };
        let result = unsafe { crate::texture_owner::lock(self.access, texture.object, info, time, texture.lod, self.device) };
        self.stat_result(result)?;
        let shared = unsafe { &*info.cast::<MeshTextureInfo>() };
        texture.info = info; texture.u_scale = shared.u_scale; texture.v_scale = shared.v_scale; texture.u_size = shared.u_size; texture.v_size = shared.v_size;
        self.locked_textures |= 1 << index;
        Ok(())
    }
    fn finish_material_preparation(&mut self) -> Result<(), ()> { self.stat_finish(stat_tmap(), &self.texture_started); Ok(()) }
    fn lock_environment(&mut self, texture: &mut *mut UTexture) -> Result<(), ()> {
        if (*texture).is_null() { return self.fail("LOD environment texture is null"); }
        self.environment = *texture;
        self.texture_started.replace(self.stat_start(stat_tmap()));
        let info = Self::environment_info_ptr();
        let time = unsafe { i64::from_ne_bytes((*self.viewport).current_time) };
        let result = unsafe { crate::texture_owner::lock(self.access, self.environment, info, time, -1, self.device) };
        self.stat_result(result)?;
        Ok(())
    }
    fn setup_actor_lighting(&mut self, flags: &mut u32) -> Result<(), ()> {
        *self.light_setup_started.borrow_mut() = self.stat_start(stat_light_setup());
        let lighting_actor = unsafe { (*self.sprite).actor.cast::<AActor>() };
        let (setup_flags, setup) = match unsafe { crate::render_owner::lighting::setup_actor(self.access, self.frame, lighting_actor, unsafe { (*self.sprite).leaf_lights.cast() }, unsafe { (*self.sprite).volumetrics.cast() }) } {
            Ok(value) => value, Err(error) => { self.record(error); return Err(()); }
        };
        *flags |= setup_flags;
        *self.light_setup.borrow_mut() = Some(setup);
        self.stat_finish(stat_light_setup(), &self.light_setup_started);
        Ok(())
    }
    fn begin_vertex_lighting(&mut self) -> Result<(), ()> { self.light_started.replace(self.stat_start(stat_light())); Ok(()) }
    fn light_and_fog(&mut self, vertex: &mut LodLightVertex, flags: u32) -> Result<(), ()> {
        if vertex.sample.is_null() { return self.fail("LOD light vertex has no sample"); }
        // SAFETY: the policy passes a live scratch vertex allocated for this draw.
        let sample = unsafe { &mut *vertex.sample };
        let mut fog = hp2_render::light::FogVertex { point: sample.transform.point, normal: FVector { x: sample.normal.x, y: sample.normal.y, z: sample.normal.z }, light: [sample.light.x, sample.light.y, sample.light.z, sample.light.w], fog: [sample.fog.x, sample.fog.y, sample.fog.z, sample.fog.w] };
        self.light_and_fog_rust(&mut fog, flags)?;
        sample.light = FPlane { x: fog.light[0], y: fog.light[1], z: fog.light[2], w: fog.light[3] };
        sample.fog = FPlane { x: fog.fog[0], y: fog.fog[1], z: fog.fog[2], w: fog.fog[3] };
        Ok(())
    }
    fn light_and_fog_rust(&mut self, vertex: &mut hp2_render::light::FogVertex, flags: u32) -> Result<bool, ()> {
        self.stat_result(crate::render_owner::lighting::light_and_fog(self.access, vertex, flags))
    }
    fn record_visible_faces(&mut self, count: i32) -> Result<(), ()> { self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_poly_count) }, count); Ok(()) }
    fn finish_actor_lighting(&mut self) -> Result<(), ()> {
        self.stat_finish(stat_light(), &self.light_started);
        self.finish_lighting()
    }
    fn begin_texture_rendering(&mut self) -> Result<(), ()> { self.texture_started.replace(self.stat_start(stat_tmap())); Ok(()) }
    fn unlock_mesh_texture(&mut self, index: i32) -> Result<(), ()> {
        if !(0..16).contains(&index) { return self.fail("LOD mesh texture index is out of range"); }
        if self.locked_textures & (1 << index) != 0 {
            let texture = unsafe { (*mesh_shared::storage()).textures[index as usize] }.cast::<UTexture>();
            let info = Self::texture_info_ptr(index as usize);
            let result = unsafe { crate::texture_owner::unlock(self.access, texture, info) };
            self.stat_result(result)?;
            self.locked_textures &= !(1 << index);
        }
        Ok(())
    }
    fn release_wire_vertices(&mut self) -> Result<(), ()> { Ok(()) }
    fn uses_rust_light_manager(&mut self) -> Result<bool, ()> { Ok(true) }
    fn unlock_environment(&mut self, texture: &mut *mut UTexture) -> Result<(), ()> {
        if !self.environment.is_null() {
            let info = Self::environment_info_ptr();
            let result = unsafe { crate::texture_owner::unlock(self.access, self.environment, info) };
            self.stat_result(result)?;
            self.environment = ptr::null_mut();
        }
        self.stat_finish(stat_tmap(), &self.texture_started);
        *texture = ptr::null_mut();
        Ok(())
    }
    fn finish_mesh_draw(&mut self) -> Result<(), ()> {
        self.finish_lighting()?;
        self.stat_finish(stat_mesh(), &self.mesh_started);
        self.stat_add(|s| unsafe { ptr::addr_of_mut!((*s).mesh_count) }, 1);
        Ok(())
    }
    fn animation_sequence(&mut self, owner: LodAnimationOwner, anim: &mut hp2_render::mesh_lod_pipeline::LodAnim) -> Result<(), ()> {
        let anim_owner = if owner == LodAnimationOwner::Owner { unsafe { (*self.actor).owner.cast::<AActor>() } } else { self.actor };
        let anim_owner = if anim_owner.is_null() { self.actor } else { anim_owner };
        self.animation_owner.set(anim_owner);
        let name = unsafe { i32::from_ne_bytes((*anim_owner).anim_sequence) };
        anim.has_sequence = i32::from(name != 0);
        Ok(())
    }
    fn animation_details(&mut self, kind: LodAnimationKind, anim: &mut hp2_render::mesh_lod_pipeline::LodAnim) -> Result<(), ()> {
        let owner = self.animation_owner.get();
        let owner = if owner.is_null() { self.actor } else { owner };
        let name = unsafe { i32::from_ne_bytes((*owner).anim_sequence) };
        let sequence = if kind == LodAnimationKind::Skeletal {
            let animation = unsafe { (*owner).skel_anim.cast::<UAnimation>() };
            if animation.is_null() { None } else {
                let (seqs, count) = self.array::<crate::mesh::AnimSeqRaw>(unsafe { ptr::addr_of!((*animation).anim_seqs) }, "Invalid skeletal animation sequence array").map_err(|error| { self.record(error); () })?;
                if count == 0 { None } else { unsafe { std::slice::from_raw_parts(seqs, count).iter().find(|sequence| sequence.name == name).copied() } }
            }
        } else {
            let (seqs, count) = self.array::<crate::mesh::AnimSeqRaw>(unsafe { ptr::addr_of!((*self.mesh).base.anim_seqs) }, "Invalid LOD animation sequence array").map_err(|error| { self.record(error); () })?;
            if count == 0 { None } else { unsafe { std::slice::from_raw_parts(seqs, count).iter().find(|sequence| sequence.name == name).copied() } }
        };
        anim.found_sequence = i32::from(sequence.is_some());
        anim.frames = sequence.map_or(0, |sequence| sequence.num_frames);
        anim.frame = unsafe { (*owner).anim_frame };
        anim.stat_line = unsafe { (*self.render).stat_line };
        let canvas = unsafe { (*self.viewport).canvas.cast::<UCanvas>() };
        anim.canvas_height = if canvas.is_null() { 0 } else { unsafe { (*canvas).y } };
        Ok(())
    }
    fn draw_animation_stats(&mut self, vertex_count: i32, anim: &mut hp2_render::mesh_lod_pipeline::LodAnim) -> Result<(), ()> {
        let canvas = unsafe { (*self.viewport).canvas.cast::<UCanvas>() };
        if canvas.is_null() { return self.fail("LOD animation stats have no Canvas"); }
        let actor_name = match self.access.name_text(unsafe { i32::from_ne_bytes((*self.actor.cast::<generated::UObject>()).name) }) {
            Ok(name) => name,
            Err(error) => return { self.record(error); Err(()) },
        };
        let owner = self.animation_owner.get();
        let owner = if owner.is_null() { self.actor } else { owner };
        let sequence_name = match self.access.name_text(unsafe { i32::from_ne_bytes((*owner).anim_sequence) }) {
            Ok(name) => name,
            Err(error) => return { self.record(error); Err(()) },
        };
        let elapsed = self.draw_started.borrow().map_or(0, |start| hp2_core::unix_clock::cycles().wrapping_sub(start) as i32);
        let milliseconds = elapsed as f32 * hp2_core::unix_clock::seconds_per_cycle() * 1000.0;
        unsafe {
            (*canvas).cur_y = (*self.render).stat_line.wrapping_add(8) as f32;
            (*self.render).stat_line = (*canvas).cur_y as i32;
            (*canvas).color = if anim.tween != 0 { [215, 215, 255, 255] } else { [230, 230, 230, 255] };
        }
        let mut text = WideText::default();
        text.write_char(' ').and_then(|_| text.append_padded(&actor_name, 21)).map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
        if anim.tween != 0 {
            text.write_str(" tween:").map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
            text.append_padded(&sequence_name, 10).map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
            write!(&mut text, " [  -> /{:2}] rate:{:4.2} verts:{:4} msec:{:4.2} alpha:{:4.2}", anim.frames, unsafe { (*owner).tween_rate }, vertex_count, milliseconds, anim.alpha).map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
        } else {
            text.write_str("  anim:").map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
            text.append_padded(&sequence_name, 10).map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
            write!(&mut text, " [{:5.2}/{:2}] rate:{:4.2} verts:{:4} msec:{:4.2}", anim.frame, anim.frames, unsafe { (*owner).anim_rate }, vertex_count, milliseconds).map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
        }
        text.write_str("\n").map_err(|_| { self.record(RuntimeError::new("LOD animation stats text exceeds the source buffer")); () })?;
        text.units[text.len] = 0;
        let font = unsafe { (*canvas).small_font.cast() };
        self.stat_result(unsafe { crate::canvas_owner::wrapped_printf(self.access, canvas, font, false, &text.units[..=text.len]) })
    }
}

struct WideText { units: [u16; 512], len: usize }
impl Default for WideText { fn default() -> Self { Self { units: [0; 512], len: 0 } } }
impl fmt::Write for WideText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for ch in value.chars() { self.write_char(ch)?; }
        Ok(())
    }
}
impl WideText {
    fn write_char(&mut self, value: char) -> fmt::Result {
        let mut encoded = [0; 2];
        for unit in value.encode_utf16(&mut encoded).iter().copied() {
            if self.len + 1 >= self.units.len() { return Err(fmt::Error); }
            self.units[self.len] = unit;
            self.len += 1;
        }
        Ok(())
    }
    fn append_padded(&mut self, units: &[u32], width: usize) -> fmt::Result {
        let count = units.iter().take_while(|unit| **unit != 0).count();
        for _ in count..width { self.write_char(' ')?; }
        for &unit in units.iter().take(count) { self.write_char(char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))?; }
        Ok(())
    }
}

pub(super) struct CoreFrameCache<'a, 'b> {
    access: CoreAccess<'a>,
    cache: &'b mut hp2_core::cache_owner::CacheOwner,
    error: Option<RuntimeError>,
    fallback: Option<Vec<u8>>,
}
impl<'a, 'b> CoreFrameCache<'a, 'b> {
    pub(super) fn new(access: CoreAccess<'a>, cache: &'b mut hp2_core::cache_owner::CacheOwner) -> Self {
        Self { access, cache, error: None, fallback: None }
    }

    pub(super) fn take_error(&mut self) -> Option<RuntimeError> { self.error.take() }

    fn allocation_failure(&mut self, len: usize) -> crate::mesh::CacheBlock {
        let mut bytes = vec![0u8; len];
        let data = bytes.as_mut_ptr();
        self.fallback = Some(bytes);
        crate::mesh::CacheBlock { ptr: data, len, item: ptr::null_mut() }
    }
}
impl crate::mesh::FrameCache for CoreFrameCache<'_, '_> {
    fn get(&mut self, id: u64, len: usize) -> Option<crate::mesh::CacheBlock> {
        match unsafe { self.cache.get(self.access, id, 16) } {
            Some((ptr, item)) => Some(crate::mesh::CacheBlock { ptr, len, item: item.cast() }), None => None,
        }
    }
    fn create(&mut self, id: u64, len: usize) -> crate::mesh::CacheBlock {
        let Ok(size) = i32::try_from(len) else {
            self.error = Some(RuntimeError::new("LOD frame cache allocation exceeds engine limits"));
            return self.allocation_failure(len);
        };
        match unsafe { self.cache.create(self.access, id, size, 16, 0) } {
            Ok((ptr, item)) => crate::mesh::CacheBlock { ptr, len, item: item.cast() },
            Err(error) => { self.error = Some(error); self.allocation_failure(len) }
        }
    }
    fn flush(&mut self, id: u64) { if let Err(error) = self.cache.flush(self.access, id, u32::MAX, 0) { self.error = Some(error); } }
    fn unlock(&mut self, block: crate::mesh::CacheBlock) { if !block.item.is_null() { if let Err(error) = unsafe { self.cache.unlock(self.access, block.item.cast()) } { self.error = Some(error); } } }
}

unsafe fn load_mesh_arrays(access: CoreAccess<'_>, mesh: *mut ULodMesh) -> RuntimeResult<()> {
    unsafe {
        let mesh = ptr::addr_of_mut!((*mesh).base);
        let verts = ptr::addr_of_mut!((*mesh).verts);
        let tris = ptr::addr_of_mut!((*mesh).tris);
        let connects = ptr::addr_of_mut!((*mesh).connects);
        let links = ptr::addr_of_mut!((*mesh).vert_links);
        for (base, offset, loader, archive, position) in [
            (verts.cast::<u8>(), generated::TLazyMeshVerts::TARRAY_FMESHVERT_OFFSET, generated::TLazyMeshVerts::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*verts).saved_ar), ptr::addr_of_mut!((*verts).saved_pos)),
            (tris.cast::<u8>(), generated::TLazyMeshTris::TARRAY_FMESHTRI_OFFSET, generated::TLazyMeshTris::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*tris).saved_ar), ptr::addr_of_mut!((*tris).saved_pos)),
            (connects.cast::<u8>(), generated::TLazyMeshConnects::TARRAY_FMESHVERTCONNECT_OFFSET, generated::TLazyMeshConnects::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*connects).saved_ar), ptr::addr_of_mut!((*connects).saved_pos)),
            (links.cast::<u8>(), generated::TLazyMeshLinks::TARRAY_INT_OFFSET, generated::TLazyMeshLinks::FLAZYLOADER_OFFSET, ptr::addr_of_mut!((*links).saved_ar), ptr::addr_of_mut!((*links).saved_pos)),
        ] {
            let slots = hp2_core::lazy_array::LazyArraySlots { array: base.add(offset).cast(), loader: base.add(loader).cast(), saved_archive: archive, saved_position: position };
            hp2_core::lazy_array::lazy_array_load(access, slots)?;
        }
    }
    Ok(())
}

fn array_view(raw: *const generated::TArrayRaw) -> LodArrayView {
    unsafe { LodArrayView { data: ptr::addr_of!((*raw).data).cast(), len: ptr::addr_of!((*raw).num) } }
}

/// Draws an actor's LOD mesh through the callback-free typed Core renderer.
///
/// # Safety
/// All pointers must be live canonical renderer, frame, sprite, actor, and mesh records.
pub(super) unsafe fn draw_lod_mesh(
    access: CoreAccess<'_>, render: *mut URender, frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite, actor: *mut AActor, coords: *const FCoords, flags: u32,
) -> RuntimeResult<()> {
    unsafe { draw_actor_mesh(access, render, frame, sprite, actor, coords, flags, false) }
}

pub(super) unsafe fn draw_skeletal_mesh(
    access: CoreAccess<'_>, render: *mut URender, frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite, actor: *mut AActor, coords: *const FCoords, flags: u32,
) -> RuntimeResult<()> {
    unsafe { draw_actor_mesh(access, render, frame, sprite, actor, coords, flags, true) }
}

unsafe fn draw_actor_mesh(
    access: CoreAccess<'_>, render: *mut URender, frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite, actor: *mut AActor, coords: *const FCoords, flags: u32,
    skeletal: bool,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() || sprite.is_null() || actor.is_null() || coords.is_null() {
        return Err(RuntimeError::new("Invalid canonical LOD mesh draw arguments"));
    }
    let mesh_object = unsafe { (*actor).mesh.cast::<generated::UObject>() };
    if mesh_object.is_null() { return Err(RuntimeError::new("LOD mesh draw requires a mesh")); }
    let skeletal_class = access.native_class("Engine.SkeletalMesh")?;
    let actual_skeletal = hp2_core::object_model::is_a(mesh_object.cast(), skeletal_class.cast());
    if skeletal != actual_skeletal {
        return Err(RuntimeError::new(if skeletal { "Skeletal mesh draw requires a USkeletalMesh" } else { "LOD mesh draw received a USkeletalMesh" }));
    }
    let mesh = mesh_object.cast::<ULodMesh>();
    unsafe { load_mesh_arrays(access, mesh)?; }
    let viewport = unsafe { (*frame).viewport.cast::<UViewport>() };
    if viewport.is_null() { return Err(RuntimeError::new("LOD mesh draw has no viewport")); }
    let device = unsafe { (*viewport).ren_dev.cast::<generated::URenderDevice>() };
    if device.is_null() { return Err(RuntimeError::new("LOD mesh draw has no render device")); }
    let engine_object = unsafe { (*render).base.engine.cast::<generated::UEngine>() };
    let client = if engine_object.is_null() { ptr::null_mut() } else { unsafe { (*engine_object).client.cast::<UClient>() } };
    if client.is_null() { return Err(RuntimeError::new("LOD mesh draw has no Engine client")); }
    let engine = access.owner_context::<crate::core_owner::EngineState>().ok_or_else(|| RuntimeError::new("LOD mesh draw has no Engine owner"))?;
    let memory = engine.memory.as_ptr().cast::<c_void>();
    let mark = unsafe { engine.memory.mark() };
    struct MarkGuard<'a> { memory: &'a hp2_core::mem_owner::MemStackOwner, mark: FMemMark }
    impl Drop for MarkGuard<'_> { fn drop(&mut self) { unsafe { self.memory.pop(&self.mark); } } }
    let _mark = MarkGuard { memory: &engine.memory, mark };
    let mut live = LodLiveState {
        selected: unsafe { i32::from((*actor).b_selected()) },
        two_sided_bias: unsafe { i32::from((*actor).b_two_sided_bias()) },
        curvy: unsafe { i32::from((*actor).b_mesh_curvy()) },
        anim_by_owner: unsafe { i32::from((*actor).b_anim_by_owner()) }, has_owner: unsafe { i32::from(!(*actor).owner.is_null()) },
        ortho: unsafe { i32::from((*viewport).actor.cast::<generated::APlayerPawn>().as_ref().is_some_and(|a| (generated::ENGINE_REN_ORTHXY..=generated::ENGINE_REN_ORTHYZ).contains(&a.rend_map))) },
        collision_height: unsafe { (*sprite).actor.cast::<AActor>().as_ref().map_or(0.0, |a| a.collision_height) },
        world_lod: unsafe { (*client).texture_lod_set[generated::ENGINE_LODSET_WORLD as usize] },
        recommended_lod: unsafe { (*device).recommended_lod },
        texture_count: unsafe { (*mesh).base.textures.num },
    };
    let mut shared_error = 0;
    let shared_geometry = MeshSharedGeometry {
        frame: frame.cast(), span: unsafe { (*sprite).span_buffer.cast() },
        clip: [unsafe { ptr::addr_of!((*frame).prj_xm) }, unsafe { ptr::addr_of!((*frame).prj_xp) }, unsafe { ptr::addr_of!((*frame).prj_ym) }, unsafe { ptr::addr_of!((*frame).prj_yp) }],
        projection: unsafe { ptr::addr_of!((*frame).proj.z) },
        screen: [unsafe { ptr::addr_of!((*frame).fx15) }, unsafe { ptr::addr_of!((*frame).fy15) }, unsafe { ptr::addr_of!((*frame).fx) }, unsafe { ptr::addr_of!((*frame).fy) }],
        near_clip: unsafe { ptr::addr_of!((*frame.cast::<hp2_abi::render_scene::FSceneNode>()).near_clip) }, uncoords: unsafe { ptr::addr_of!((*frame).uncoords) },
    };
    let shared = mesh_shared::MeshSharedState { geometry: shared_geometry, memory, memory_error: &mut shared_error, alignment: 8, slow_checks: cfg!(debug_assertions) };
    let stats = engine.render_graph.stats_ptr();
    let light_count_stat = if generated::ENGINE_BUILD_STATS == 0 || stats.is_null() { ptr::null_mut() } else { unsafe { ptr::addr_of_mut!((*stats).mesh_vert_light_count) } };
    let mut state: LodDrawState = unsafe { mem::zeroed() };
    let faces = unsafe { ptr::addr_of!((*mesh).faces) };
    let special_faces = unsafe { ptr::addr_of!((*mesh).special_faces) };
    let wedges = unsafe { ptr::addr_of!((*mesh).wedges) };
    let materials = unsafe { ptr::addr_of!((*mesh).materials) };
    let levels = unsafe { ptr::addr_of!((*mesh).face_level) };
    let collapse_points = unsafe { ptr::addr_of!((*mesh).collapse_point_thus) };
    let collapse_wedges = unsafe { ptr::addr_of!((*mesh).collapse_wedge_thus) };
    let live_ptr = &mut live as *mut LodLiveState;
    let mut shared_stage = 0;
    state.light_count_stat = light_count_stat;
    state.live = live_ptr;
    state.shared = shared;
    state.shared_stage = &mut shared_stage;
    state.span = unsafe { ptr::addr_of!((*sprite).span_buffer).cast() };
    state.faces = array_view(faces); state.special_faces = array_view(special_faces);
    state.wedges = array_view(wedges); state.materials = array_view(materials); state.levels = array_view(levels);
    state.collapse_points = array_view(collapse_points); state.collapse_wedges = array_view(collapse_wedges);
    state.model_verts = unsafe { (*mesh).model_verts };
    state.special_verts = unsafe { ptr::addr_of!((*mesh).special_verts) };
    state.collapse_count = unsafe { (*mesh).collapse_point_thus.num };
    state.lod_min = unsafe { (*mesh).lod_min_verts };
    state.lod_strength = unsafe { (*mesh).lod_strength };
    state.lod_bias = unsafe { (*actor).lod_bias };
    state.global_lod = unsafe { (*render).global_shape_lod };
    state.global_adjust = unsafe { (*render).global_shape_lod_adjust };
    state.rect = unsafe { [(*sprite).x1, (*sprite).y1, (*sprite).x2, (*sprite).y2] };
    state.fx = unsafe { (*frame).fx };
    state.mode = unsafe { ptr::addr_of!((*render).shape_lod_mode) };
    state.fix = unsafe { (*render).shape_lod_fix };
    state.morph = unsafe { ptr::addr_of!((*mesh).lod_morph) };
    state.morph_enabled = 0;
    state.wire = unsafe { (*render).wire_show as i32 };
    state.rend_wire = unsafe { i32::from((*viewport).actor.cast::<generated::APlayerPawn>().as_ref().is_some_and(|a| a.rend_map == 1)) };
    state.skeletal = i32::from(skeletal);
    state.bones = unsafe { ptr::addr_of!((*render).bone_show).cast::<i32>() };
    state.blend = unsafe { ptr::addr_of!((*render).blend_show).cast::<i32>() };
    state.software = unsafe { i32::from((*device).span_based != 0) };
    state.not_weapon = unsafe { i32::from((*actor).owner != (*viewport).actor) };
    let editor = i32::from(access.process_mode().is_editor);
    state.editor = &editor;
    state.anim_stats = unsafe { ptr::addr_of!((*render).anim_stats).cast::<i32>() };
    state.scale_glow = unsafe { ptr::addr_of!((*actor).scale_glow) };
    state.ambient_glow = unsafe { ptr::addr_of!((*actor).ambient_glow) };
    state.fatness = unsafe { ptr::addr_of!((*actor).fatness) };
    state.opacity = unsafe { ptr::addr_of!((*actor).opacity) };
    state.mirror = unsafe { ptr::addr_of!((*frame.cast::<hp2_abi::render_scene::FSceneNode>()).mirror) };
    state.clip = [unsafe { ptr::addr_of!((*frame).prj_xm) }, unsafe { ptr::addr_of!((*frame).prj_xp) }, unsafe { ptr::addr_of!((*frame).prj_ym) }, unsafe { ptr::addr_of!((*frame).prj_yp) }];
    state.projection = unsafe { ptr::addr_of!((*frame).proj.z) };
    state.screen = [unsafe { ptr::addr_of!((*frame).fx15) }, unsafe { ptr::addr_of!((*frame).fy15) }, unsafe { ptr::addr_of!((*frame).fx2) }, unsafe { ptr::addr_of!((*frame).fy2) }];
    state.coords = coords;
    state.debug_coords = unsafe { ptr::addr_of!((*frame).coords) };
    state.location = unsafe { ptr::addr_of!((*actor).location) };
    state.flags = flags;
    let storage = mesh_shared::storage();
    state.u_scale = unsafe { ptr::addr_of_mut!((*storage).u_scale) };
    state.v_scale = unsafe { ptr::addr_of_mut!((*storage).v_scale) };
    state.influences = LodArrayView { data: ptr::null(), len: ptr::null() };
    state.weapon_bone = -1;
    state.weapon_coords = unsafe { (*frame).coords };
    let graph = super::super::super::state(access)?;
    state.randoms = graph.random_values_ptr();
    state.owner = actor;
    let mut ops = CoreLodDrawOps {
        access, render, frame, sprite, actor, mesh, viewport, device, client, state: &mut state,
        error: RefCell::new(None), light_setup: RefCell::new(None), texture_started: RefCell::new(None),
        mesh_started: RefCell::new(None), frame_started: RefCell::new(None), light_started: RefCell::new(None),
        light_setup_started: RefCell::new(None),
        draw_started: RefCell::new(None),
        animation_owner: Cell::new(actor),
        environment: ptr::null_mut(), locked_textures: 0,
        debug_pivots: RefCell::new(Vec::new()), debug_parents: RefCell::new(Vec::new()),
        debug_influences: RefCell::new(Vec::new()), debug_original: RefCell::new(None),
    };
    let result = unsafe { mesh_lod_pipeline::draw_with_ops(&mut state, &mut ops) };
    if let Some(error) = ops.take_error() { return Err(error); }
    result.map_err(|_| RuntimeError::new("Canonical typed LOD mesh draw failed"))
}
