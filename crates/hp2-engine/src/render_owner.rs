//! Canonical renderer graph memory and active scene publication.

use hp2_abi::{
    engine::generated::{AActor, FRotator, FSceneNode, FStats, UEngine, ULevel, UObject, URender, URenderBase, URenderDevice, UViewport, UTexture, UModel, UClass, UClient, FTextureInfo},
    render_scene::{FDynamicsCache, FActorLink, FVolActorLink, FBspSurf},
    render_span::FMemMark,
};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind},
    object_archive::ObjectArchive,
    object_owner::ClassOps,
    mem_owner::MemStackOwner,
};
use std::{alloc::Layout, cell::{Cell, Ref, RefCell, UnsafeCell}, ptr, ptr::NonNull, rc::Rc};

#[path = "render_corona_owner.rs"]
mod corona;
#[path = "render_stats_owner.rs"]
mod stats;
#[path = "render_light_owner.rs"]
mod lighting;
#[path = "render_surface_owner.rs"]
mod surface;
#[path = "render_actor_owner.rs"]
mod actor;
pub(crate) use actor::set_high_detail_models;
#[path = "render_scene_draw_owner.rs"]
mod scene_draw;
#[path = "render_occlusion_owner.rs"]
mod occlusion;
#[path = "render_world_owner.rs"]
mod world;

pub(crate) use actor::mesh::{
    bone_coords, particle_mesh_bounds, particle_mesh_frame, particle_mesh_triangle,
    particle_mesh_triangles, skeletal_adjust_root_movement, skeletal_bone_coords,
    skeletal_root_movement,
};
pub(crate) use world::draw_world;

const MAX_PUBLISHED_FRAMES: usize = 4;
const CLASS_ABSTRACT: u32 = 1;
const LIGHTING_INITIALIZED: &[u32] = &[
    76, 105, 103, 104, 116, 105, 110, 103, 32, 115, 117, 98, 115, 121, 115, 116, 101, 109, 32,
    105, 110, 105, 116, 105, 97, 108, 105, 122, 101, 100,
];
const LIGHTING_SHUT_DOWN: &[u32] = &[
    76, 105, 103, 104, 116, 105, 110, 103, 32, 115, 117, 98, 115, 121, 115, 116, 101, 109, 32,
    115, 104, 117, 116, 32, 100, 111, 119, 110,
];
const RENDERING_INITIALIZED: &[u32] = &[
    82, 101, 110, 100, 101, 114, 105, 110, 103, 32, 105, 110, 105, 116, 105, 97, 108, 105, 122,
    101, 100,
];
const RENDERING_SHUT_DOWN: &[u32] =
    &[82, 101, 110, 100, 101, 114, 105, 110, 103, 32, 115, 104, 117, 116, 32, 100, 111, 119, 110];
const CLASS_CONFIG: u32 = 4;
const CLASS_TRANSIENT: u32 = 8;
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const BASE: ClassOps = NativeRecordKind::Subsystem.operations();
const RENDER_BASE_OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.serialize = serialize_render_base;
    operations.visit_references = visit_render_base;
    operations
};
const RENDER_OPERATIONS: ClassOps = {
    let mut operations = RENDER_BASE_OPERATIONS;
    operations.destroy = destroy_render_object;
    operations
};

/// Canonical RenderBase and Render declarations, superclass-first.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "RenderBase", package: "Engine", super_name: Some("Subsystem"),
        within: "Object", flags: CLASS_ABSTRACT | CLASS_CONFIG | CLASS_TRANSIENT,
        object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(native_layout::<URenderBase>(URenderBase::NATIVE_ALIGNMENT)),
        operations: Some(RENDER_BASE_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "Render", package: "Render", super_name: Some("Engine.RenderBase"),
        within: "Object", flags: CLASS_CONFIG, object_flags: OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(native_layout::<URender>(URender::NATIVE_ALIGNMENT)),
        operations: Some(RENDER_OPERATIONS), properties: &[], defaults: &[],
    },
];

const fn native_layout<T>(alignment: usize) -> Layout {
    match Layout::from_size_align(std::mem::size_of::<T>(), alignment) {
        Ok(layout) => layout,
        Err(_) => panic!("compiler-probed Render class alignment is invalid"),
    }
}

unsafe fn serialize_render_base(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical RenderBase")); }
    unsafe {
        (BASE.serialize)(access, object, archive)?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*object.cast::<URenderBase>()).engine).cast())?;
    }
    Ok(())
}

unsafe fn visit_render_base(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical RenderBase")); }
    unsafe {
        (BASE.visit_references)(access, object, archive)?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*object.cast::<URenderBase>()).engine).cast())?;
    }
    Ok(())
}
unsafe fn destroy_render_object(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical Render"));
    }
    unsafe { destroy(access, object.cast::<URender>()) }
}


/// Implements the source `URenderBase::Init` field assignment without C++ dispatch.
pub(crate) fn initialize_base(
    _access: CoreAccess<'_>,
    render_base: *mut URenderBase,
    engine: *mut UEngine,
) -> RuntimeResult<()> {
    if render_base.is_null() || engine.is_null() {
        return Err(RuntimeError::new("Canonical RenderBase::Init requires live Engine records"));
    }
    unsafe { ptr::addr_of_mut!((*render_base).engine).write(engine.cast()) };
    Ok(())
}

const DYNAMIC_MEMORY_TAG: &[u32] = &[71, 68, 121, 110, 77, 101, 109, 0];
const SCENE_MEMORY_TAG: &[u32] = &[71, 83, 99, 101, 110, 101, 77, 101, 109, 0];
const VECTOR_MEMORY_TAG: &[u32] = &[86, 101, 99, 116, 111, 114, 77, 101, 109, 0];
const DYNAMIC_MEMORY_SIZE: i32 = 65_536;
const SCENE_MEMORY_SIZE: i32 = 32_768;
const VECTOR_MEMORY_SIZE: i32 = 16_384;

/// Owns the canonical renderer's global stacks and live frame publication.
///
/// The Core owner context keeps this state stable for every stack mark and scene callback.
pub(crate) struct RenderGraphState {
    dynamic_memory: RefCell<Option<MemStackOwner>>,
    scene_memory: RefCell<Option<MemStackOwner>>,
    vector_memory: RefCell<Option<MemStackOwner>>,
    render: Cell<Option<NonNull<URender>>>,
    frames: RefCell<[Option<NonNull<FSceneNode>>; MAX_PUBLISHED_FRAMES]>,
    frame_count: Cell<usize>,
    dynamic_light_surfs: Cell<i32>,
    dynamic_light_leaves: Cell<i32>,
    stats: Cell<FStats>,
    previous_stats: Cell<FStats>,
    light_manager: UnsafeCell<hp2_render::light_manager::State>,
    light_manager_constructed: Cell<bool>,
    render_state: UnsafeCell<hp2_render::state::RenderState>,
    dynamics_cache: RefCell<Vec<FDynamicsCache>>,
    dyn_light_surfs: UnsafeCell<[i32; 2048]>,
    dyn_light_leaves: UnsafeCell<[i32; 1024]>,
    max_surf_lights: Cell<i32>,
    max_leaf_lights: Cell<i32>,
    surf_lights: RefCell<Vec<*mut FActorLink>>,
    leaf_lights: RefCell<Vec<*mut FVolActorLink>>,
    last_process_time: Cell<f32>,
    random_bases: UnsafeCell<[f32; hp2_render::random::N_RANDS]>,
    random_values: UnsafeCell<[f32; hp2_render::random::N_RANDS]>,
    random_deltas: UnsafeCell<[f32; hp2_render::random::N_RANDS]>,
    random_last_ticks: Cell<u32>,
    source_mark: UnsafeCell<FMemMark>,
    mover_debug_enabled: Cell<Option<bool>>,
    mover_lumos_logs: Cell<i32>,
    mover_zone_rejects: Cell<i32>,
    old_net_stats: Cell<bool>,
    network_saved_time: Cell<[u8; 8]>,
}

impl Default for RenderGraphState {
    fn default() -> Self {
        Self {
            dynamic_memory: RefCell::new(None),
            scene_memory: RefCell::new(None),
            vector_memory: RefCell::new(None),
            render: Cell::new(None),
            frames: RefCell::new([None; MAX_PUBLISHED_FRAMES]),
            frame_count: Cell::new(0),
            dynamic_light_surfs: Cell::new(0),
            dynamic_light_leaves: Cell::new(0),
            // SAFETY: FStats is a C record of scalar fields and padding; zero is valid.
            stats: Cell::new(unsafe { std::mem::zeroed() }),
            // SAFETY: FStats is a C record of scalar fields and padding; zero is valid.
            previous_stats: Cell::new(unsafe { std::mem::zeroed() }),
            // SAFETY: the shared light-manager State mirrors a zero-initialized C global.
            light_manager: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            last_process_time: Cell::new(0.0),
            random_bases: UnsafeCell::new([0.0; hp2_render::random::N_RANDS]),
            random_values: UnsafeCell::new([0.0; hp2_render::random::N_RANDS]),
            random_deltas: UnsafeCell::new([0.0; hp2_render::random::N_RANDS]),
            random_last_ticks: Cell::new(0),
            source_mark: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            mover_debug_enabled: Cell::new(None),
            mover_lumos_logs: Cell::new(0),
            mover_zone_rejects: Cell::new(0),
            old_net_stats: Cell::new(false),
            network_saved_time: Cell::new([0; 8]),
            light_manager_constructed: Cell::new(false),
            render_state: UnsafeCell::new(hp2_render::state::RenderState::default()),
            dynamics_cache: RefCell::new(Vec::new()),
            dyn_light_surfs: UnsafeCell::new([0; 2048]),
            dyn_light_leaves: UnsafeCell::new([0; 1024]),
            max_surf_lights: Cell::new(0),
            max_leaf_lights: Cell::new(0),
            surf_lights: RefCell::new(Vec::new()),
            leaf_lights: RefCell::new(Vec::new()),
        }
    }
}

impl RenderGraphState {
    /// Creates the empty graph; native memory stacks are initialized in renderer source order.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Borrows the stable owner-thread random table used by source mesh debug drawing.
    pub(crate) fn random_values_ptr(&self) -> *const f32 {
        self.random_values.get().cast::<f32>().cast_const()
    }

    pub(crate) fn mover_debug_enabled(&self) -> bool {
        if let Some(enabled) = self.mover_debug_enabled.get() { return enabled; }
        // Match GMoverDebugEnabled's cached first-byte getenv query without
        // allocating or retaining a pointer into the process environment.
        let enabled = unsafe {
            let value = libc::getenv(c"HP2_MOVER_DEBUG".as_ptr());
            !value.is_null() && *value == b'1' as libc::c_char
        };
        self.mover_debug_enabled.set(Some(enabled));
        enabled
    }

    pub(crate) fn initialize_light_manager(&self) -> RuntimeResult<()> {
        // Per-light callbacks reenter this owner while surface maps and cache
        // locks are live. Native renderer initialization runs only once.
        if self.light_manager_constructed.get() {
            return Ok(());
        }
        // SAFETY: Core draw and lifecycle policy are serialized on the engine thread.
        let initialized = unsafe {
            hp2_render::light_manager::initialize_core_state(
                self.light_manager.get(),
                true,
            )
        };
        if !initialized {
            return Err(RuntimeError::new("Canonical light-manager tables failed validation"));
        }
        self.light_manager_constructed.set(true);
        Ok(())
    }

    pub(crate) fn with_light_manager<R>(&self, operation: impl FnOnce() -> R) -> R {
        // SAFETY: the owner graph outlives the synchronous policy scope.
        unsafe { hp2_render::light_manager::with_state(self.light_manager.get(), operation) }
    }


    pub(crate) fn initialize_dynamic_memory(&self) -> RuntimeResult<()> {
        initialize_stack(&self.dynamic_memory, DYNAMIC_MEMORY_SIZE, DYNAMIC_MEMORY_TAG, "GDynMem")
    }

    pub(crate) fn initialize_scene_memory(&self) -> RuntimeResult<()> {
        initialize_stack(&self.scene_memory, SCENE_MEMORY_SIZE, SCENE_MEMORY_TAG, "GSceneMem")
    }

    pub(crate) fn initialize_vector_memory(&self) -> RuntimeResult<()> {
        initialize_stack(&self.vector_memory, VECTOR_MEMORY_SIZE, VECTOR_MEMORY_TAG, "VectorMem")
    }

    pub(crate) fn dynamic_memory(&self) -> RuntimeResult<Ref<'_, MemStackOwner>> {
        memory(&self.dynamic_memory, "GDynMem")
    }

    pub(crate) fn scene_memory(&self) -> RuntimeResult<Ref<'_, MemStackOwner>> {
        memory(&self.scene_memory, "GSceneMem")
    }

    pub(crate) fn vector_memory(&self) -> RuntimeResult<Ref<'_, MemStackOwner>> {
        memory(&self.vector_memory, "VectorMem")
    }

    pub(crate) fn exit_dynamic_memory(&self) -> RuntimeResult<()> {
        exit_stack(&self.dynamic_memory, "GDynMem")
    }

    pub(crate) fn exit_scene_memory(&self) -> RuntimeResult<()> {
        exit_stack(&self.scene_memory, "GSceneMem")
    }

    pub(crate) fn exit_vector_memory(&self) -> RuntimeResult<()> {
        exit_stack(&self.vector_memory, "VectorMem")
    }

    pub(crate) fn publish_render(&self, render: *mut URender) -> RuntimeResult<()> {
        let render = NonNull::new(render)
            .ok_or_else(|| RuntimeError::new("Cannot publish a null canonical Render"))?;
        self.render.set(Some(render));
        Ok(())
    }

    pub(crate) fn unpublish_render(&self, render: *mut URender) -> RuntimeResult<()> {
        if self.render.get().map(NonNull::as_ptr) != Some(render) {
            return Err(RuntimeError::new("Canonical Render publication does not match"));
        }
        self.render.set(None);
        Ok(())
    }

    pub(crate) fn render(&self) -> RuntimeResult<NonNull<URender>> {
        self.render.get().ok_or_else(|| RuntimeError::new("Canonical Render is not initialized"))
    }

    pub(crate) fn publish_frame(&self, frame: *mut FSceneNode) -> RuntimeResult<()> {
        let frame = NonNull::new(frame)
            .ok_or_else(|| RuntimeError::new("Cannot publish a null canonical scene frame"))?;
        let count = self.frame_count.get();
        if count == MAX_PUBLISHED_FRAMES {
            return Err(RuntimeError::new("Canonical scene frame publication exceeded source recursion limit"));
        }
        self.frames.borrow_mut()[count] = Some(frame);
        self.frame_count.set(count + 1);
        Ok(())
    }

    pub(crate) fn unpublish_frame(&self, frame: *mut FSceneNode) -> RuntimeResult<()> {
        let count = self.frame_count.get();
        if count == 0 || self.frames.borrow()[count - 1].map(NonNull::as_ptr) != Some(frame) {
            return Err(RuntimeError::new("Canonical scene frames must be unpublished in LIFO order"));
        }
        self.frames.borrow_mut()[count - 1] = None;
        self.frame_count.set(count - 1);
        Ok(())
    }

    pub(crate) fn active_frame(&self) -> Option<NonNull<FSceneNode>> {
        let count = self.frame_count.get();
        if count == 0 { None } else { self.frames.borrow()[count - 1] }
    }
    pub(crate) fn reset_statistics(&self) {
        // SAFETY: FStats is an all-zero-valid C scalar record.
        self.stats.set(unsafe { std::mem::zeroed() });
        // SAFETY: FStats is an all-zero-valid C scalar record.
        self.previous_stats.set(unsafe { std::mem::zeroed() });
    }

    pub(crate) fn dynamic_light_surfs(&self) -> i32 {
        self.dynamic_light_surfs.get()
    }

    pub(crate) fn set_dynamic_light_surfs(&self, count: i32) {
        self.dynamic_light_surfs.set(count);
    }

    pub(crate) fn dynamic_light_leaves(&self) -> i32 {
        self.dynamic_light_leaves.get()
    }

    pub(crate) fn set_dynamic_light_leaves(&self, count: i32) {
        self.dynamic_light_leaves.set(count);
    }

    pub(crate) fn render_state_ptr(&self) -> *mut hp2_render::state::RenderState {
        self.render_state.get()
    }

    pub(crate) fn stats_ptr(&self) -> *mut FStats {
        self.stats.as_ptr()
    }

    pub(crate) fn previous_stats_ptr(&self) -> *mut FStats {
        self.previous_stats.as_ptr()
    }

    pub(crate) fn with_dynamics_cache<R>(
        &self,
        operation: impl FnOnce(&mut Vec<FDynamicsCache>) -> R,
    ) -> R {
        operation(&mut self.dynamics_cache.borrow_mut())
    }

    pub(crate) fn dyn_light_surfs_ptr(&self) -> *mut i32 {
        self.dyn_light_surfs.get().cast()
    }

    pub(crate) fn dyn_light_leaves_ptr(&self) -> *mut i32 {
        self.dyn_light_leaves.get().cast()
    }

    pub(crate) fn max_surf_lights(&self) -> i32 {
        self.max_surf_lights.get()
    }

    pub(crate) fn set_max_surf_lights(&self, count: i32) {
        self.max_surf_lights.set(count);
    }

    pub(crate) fn max_leaf_lights(&self) -> i32 {
        self.max_leaf_lights.get()
    }

    pub(crate) fn set_max_leaf_lights(&self, count: i32) {
        self.max_leaf_lights.set(count);
    }

    pub(crate) fn with_surf_lights<R>(
        &self,
        operation: impl FnOnce(&mut Vec<*mut FActorLink>) -> R,
    ) -> R {
        operation(&mut self.surf_lights.borrow_mut())
    }

    pub(crate) fn with_leaf_lights<R>(
        &self,
        operation: impl FnOnce(&mut Vec<*mut FVolActorLink>) -> R,
    ) -> R {
        operation(&mut self.leaf_lights.borrow_mut())
    }

    fn release_dynamic_light_storage(&self) {
        drop(std::mem::take(&mut *self.surf_lights.borrow_mut()));
        drop(std::mem::take(&mut *self.leaf_lights.borrow_mut()));
        self.max_surf_lights.set(0);
        self.max_leaf_lights.set(0);
        self.set_dynamic_light_surfs(0);
        self.set_dynamic_light_leaves(0);
    }

}
/// Allocates and saves marks on the canonical Core-owned renderer stacks.
pub(crate) struct CoreSceneStackOps<'a> {
    engine_memory: &'a MemStackOwner,
    dynamic_memory: Ref<'a, MemStackOwner>,
    scene_memory: Ref<'a, MemStackOwner>,
    vector_memory: Ref<'a, MemStackOwner>,
    error: RefCell<Option<RuntimeError>>,
}

impl<'a> CoreSceneStackOps<'a> {
    pub(crate) fn new(access: CoreAccess<'a>) -> RuntimeResult<Self> {
        let engine = access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("Canonical scene stacks have no Engine owner"))?;
        let graph: &'a RenderGraphState = engine.render_graph.as_ref();
        Ok(Self {
            engine_memory: &engine.memory,
            dynamic_memory: graph.dynamic_memory()?,
            scene_memory: graph.scene_memory()?,
            vector_memory: graph.vector_memory()?,
            error: RefCell::new(None),
        })
    }

    pub(crate) fn stack_ptrs(&self) -> [*mut std::ffi::c_void; 4] {
        [
            self.engine_memory.as_ptr().cast(),
            self.dynamic_memory.as_ptr().cast(),
            self.scene_memory.as_ptr().cast(),
            self.vector_memory.as_ptr().cast(),
        ]
    }

    pub(crate) fn take_error(&self) -> Option<RuntimeError> {
        self.error.borrow_mut().take()
    }

    fn owner(&self, stack: *mut std::ffi::c_void) -> Option<&MemStackOwner> {
        [
            self.engine_memory,
            &self.dynamic_memory,
            &self.scene_memory,
            &self.vector_memory,
        ]
        .into_iter()
        .find(|owner| owner.as_ptr().cast::<std::ffi::c_void>() == stack)
    }

    fn record(&self, error: RuntimeError) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    }
}

impl hp2_render::scene_pipeline::helper_frames::SceneStackOps for CoreSceneStackOps<'_> {
    unsafe fn push(
        &self,
        _context: *mut std::ffi::c_void,
        stack: *mut std::ffi::c_void,
        size: usize,
        alignment: i32,
        kind: u32,
    ) -> Result<*mut u8, i32> {
        let size = i32::try_from(size).map_err(|_| {
            self.record(RuntimeError::new("Canonical scene allocation exceeds the stack size limit"));
            1
        })?;
        let owner = self.owner(stack).ok_or_else(|| {
            self.record(RuntimeError::new("Canonical scene allocation used an unowned memory stack"));
            1
        })?;
        // SAFETY: caller supplies a live engine-thread-owned stack and keeps its mark active.
        unsafe { owner.push_bytes(size, alignment) }.map_err(|error| {
            self.record(error.with_context(&format!("scene allocation kind {kind}")));
            1
        })
    }

    unsafe fn mark_save(&self, stack: *mut std::ffi::c_void, mark: *mut FMemMark) {
        if let Some(owner) = self.owner(stack) {
            // SAFETY: the scene policy provides a live mark slot and the owner is stable.
            unsafe { mark.write(owner.mark()) };
        } else {
            self.record(RuntimeError::new("Canonical scene mark used an unowned memory stack"));
            // SAFETY: FMemMark is a pointer-only record; zero is its empty representation.
            unsafe { mark.write(std::mem::zeroed()) };
        }
    }

    unsafe fn mark_pop(&self, mark: *const FMemMark) {
        // SAFETY: policy only supplies a mark saved by this owner.
        let stack = unsafe { (*mark).mem.cast::<std::ffi::c_void>() };
        if let Some(owner) = self.owner(stack) {
            // SAFETY: marks are popped in the source's strict LIFO order.
            unsafe { owner.pop(&*mark) };
        } else {
            self.record(RuntimeError::new("Canonical scene mark belongs to an unowned memory stack"));
        }
    }
}
struct CoreMasterFrameContext<'a> {
    access: CoreAccess<'a>,
    error: RefCell<Option<RuntimeError>>,
}

impl CoreMasterFrameContext<'_> {
    fn record(&self, error: RuntimeError) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    }

    fn failure(&self, fallback: &str) -> i32 {
        self.record(RuntimeError::new(fallback));
        0
    }
}

impl hp2_render::scene_pipeline::helper_frames::ChildFrameOps for CoreMasterFrameContext<'_> {
    unsafe fn construct_frame(&self, _storage: *mut std::ffi::c_void) -> Result<(), ()> {
        // Plain frame storage is initialized by the shared parent-frame copy.
        Ok(())
    }

    unsafe fn construct_span(&self, _storage: *mut std::ffi::c_void) -> Result<(), ()> {
        // The shared span policy publishes the complete buffer header.
        Ok(())
    }

    unsafe fn size(
        &self,
        frame: *mut hp2_abi::render_scene::FSceneNode,
    ) -> Result<(), ()> {
        let result = unsafe {
            crate::camera::compute_render_size_with(frame.cast(), |viewport, frame| {
                let device = ptr::addr_of!((*viewport).ren_dev)
                    .read_unaligned().cast::<URenderDevice>();
                crate::render_device_owner::set_scene_node(self.access, device, frame)
                    .map_err(|error| {
                        self.record(error);
                        -1
                    })
            })
        };
        result.map_err(|_| {
            self.record(RuntimeError::new("Canonical child frame sizing failed"));
        })
    }
}

unsafe extern "C" fn master_frame_viewport(
    context: *mut std::ffi::c_void,
    viewport: *mut std::ffi::c_void,
    x: *mut i32,
    y: *mut i32,
    level: *mut *mut std::ffi::c_void,
) -> i32 {
    let viewport = viewport.cast::<UViewport>();
    if context.is_null() || viewport.is_null() || unsafe { (*viewport).actor.is_null() } {
        return 0;
    }
    unsafe {
        let actor = (*viewport).actor.cast::<AActor>();
        x.write((*viewport).size_x);
        y.write((*viewport).size_y);
        level.write(crate::actor::get_level(actor).cast());
    }
    1
}

unsafe extern "C" fn master_frame_construct(
    _context: *mut std::ffi::c_void,
    _storage: *mut std::ffi::c_void,
) -> i32 {
    // FSceneNode has no user-defined constructor; placement default-initialization is a no-op.
    1
}

unsafe extern "C" fn master_span_construct(
    _context: *mut std::ffi::c_void,
    _storage: *mut std::ffi::c_void,
) -> i32 {
    // FSpanBuffer has no user-defined constructor; span policy publishes its header next.
    1
}

unsafe extern "C" fn master_frame_coords(
    raw: *mut std::ffi::c_void,
    frame: *mut hp2_abi::render_scene::FSceneNode,
    location: *const hp2_abi::FVector,
    rotation: *const std::ffi::c_void,
) -> i32 {
    if raw.is_null() || frame.is_null() || location.is_null() || rotation.is_null() {
        return 0;
    }
    let context = unsafe { &*raw.cast::<CoreMasterFrameContext<'static>>() };
    let location = unsafe { location.read_unaligned() };
    let rotation = unsafe { rotation.cast::<FRotator>().read_unaligned() };
    let result = unsafe {
        crate::camera::compute_render_coords_with(frame.cast::<FSceneNode>(), location, rotation, |frame| {
            crate::camera::compute_render_size_with(frame, |viewport, frame| {
                let device = unsafe { ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast::<URenderDevice>() };
                unsafe { crate::render_device_owner::set_scene_node(context.access, device, frame) }
                    .map_err(|error| {
                        context.record(error);
                        -1
                    })
            })
        })
    };
    match result {
        Ok(()) => 1,
        Err(_) => context.failure("Canonical camera frame sizing failed"),
    }
}

unsafe extern "C" fn master_frame_zone(
    raw: *mut std::ffi::c_void,
    viewport: *mut std::ffi::c_void,
    origin: *const hp2_abi::FVector,
    out: *mut i32,
) -> i32 {
    if raw.is_null() || viewport.is_null() || origin.is_null() || out.is_null() {
        return 0;
    }
    let context = unsafe { &*raw.cast::<CoreMasterFrameContext<'static>>() };
    let result = unsafe {
        let actor = (*viewport.cast::<UViewport>()).actor.cast::<AActor>();
        // `AActor::Level` is the LevelInfo; `ULevel::GetLevelInfo()` reads the
        // level-info actor from `XLevel`.
        let level = (*actor).x_level.cast::<ULevel>();
        if level.is_null() {
            return context.failure("Canonical scene viewport actor has no level");
        }
        let level_info = match crate::package_map_level_owner::level_info(level) {
            Ok(info) => info,
            Err(error) => {
                context.record(error);
                return 0;
            }
        };
        let location = origin.read_unaligned();
        crate::level_owner::model_point_region(level, level_info.cast(), location)
    };
    match result {
        Ok(region) => {
            unsafe { out.write(region.zone_number as i32) };
            1
        }
        Err(error) => {
            context.record(error);
            0
        }
    }
}

unsafe extern "C" fn master_frame_count_failure(raw: *mut std::ffi::c_void) -> i32 {
    if raw.is_null() {
        return 0;
    }
    unsafe { &*raw.cast::<CoreMasterFrameContext<'static>>() }
        .failure("Canonical scene frame count underflow")
}

fn master_frame_services<'a>(
    context: &'a mut CoreMasterFrameContext<'_>,
    render: *mut URender,
    stacks: [*mut std::ffi::c_void; 4],
) -> hp2_render::scene_pipeline::helper_frames::MasterFrames {
    let context = (context as *mut CoreMasterFrameContext<'_>).cast();
    unsafe {
        hp2_render::scene_pipeline::helper_frames::MasterFrames {
            context,
            scene_count: ptr::addr_of_mut!((*render).scene_count),
            master_marks: [
                ptr::addr_of_mut!((*render).mem_mark).cast(),
                ptr::addr_of_mut!((*render).dyn_mark).cast(),
                ptr::addr_of_mut!((*render).scene_mark).cast(),
            ],
            stacks,
            viewport: master_frame_viewport,
            frame_construct: master_frame_construct,
            span_construct: master_span_construct,
            coords: master_frame_coords,
            zone: master_frame_zone,
            count_failure: master_frame_count_failure,
        }
    }
}

/// Builds a source-ordered master scene frame using the shared Native/Core helper policy.
///
/// # Safety
/// `render` and `viewport` are live canonical records and remain stable through the call.
pub(crate) unsafe fn create_master_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    viewport: *mut UViewport,
    location: hp2_abi::FVector,
    rotation: FRotator,
) -> RuntimeResult<*mut FSceneNode> {
    if render.is_null() || viewport.is_null() {
        return Err(RuntimeError::new("Canonical master frame requires a live renderer and viewport"));
    }
    let graph = state(access)?;
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("Canonical master frame renderer is not published"));
    }
    let ops = CoreSceneStackOps::new(access)?;
    let mut context = CoreMasterFrameContext { access, error: RefCell::new(None) };
    let frames = master_frame_services(&mut context, render, ops.stack_ptrs());
    let mut frame = ptr::null_mut();
    let result = unsafe {
        hp2_render::scene_pipeline::helper_frames::master_with(
            &frames, &ops, viewport.cast(), location, (&rotation as *const FRotator).cast(), &mut frame,
        )
    };
    if let Some(error) = context.error.borrow_mut().take().or_else(|| ops.take_error()) {
        return Err(error);
    }
    result.map_err(|_| RuntimeError::new("Canonical master-frame construction failed"))?;
    graph.publish_frame(frame.cast())?;
    Ok(frame.cast())
}

/// Creates or merges a child in the parent's active canonical scene scope.
///
/// # Safety
/// All records and span/bounds storage belong to this owner and remain live
/// through the enclosing master-frame scope. `bounds` may be null.
pub(crate) unsafe fn create_child_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    parent: *mut FSceneNode,
    spans: *const hp2_abi::render_span::FSpanBuffer,
    level: *mut ULevel,
    surf: i32,
    zone: i32,
    mirror: f32,
    near: hp2_abi::FPlane,
    coords: hp2_abi::FCoords,
    bounds: *const hp2_abi::render_scene::FScreenBounds,
) -> RuntimeResult<*mut FSceneNode> {
    if render.is_null() || parent.is_null() || spans.is_null() {
        return Err(RuntimeError::new("Canonical child frame requires live renderer, parent and spans"));
    }
    let graph = state(access)?;
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("Canonical child frame renderer is not published"));
    }
    let allocator = CoreSceneStackOps::new(access)?;
    let context = CoreMasterFrameContext { access, error: RefCell::new(None) };
    let mut frame = ptr::null_mut();
    let result = unsafe {
        hp2_render::scene_pipeline::helper_frames::child_with(
            &allocator, &context, ptr::null_mut(), allocator.stack_ptrs()[2],
            parent.cast(), spans, level.cast(), surf, zone, mirror, near, coords,
            bounds, &mut frame,
        )
    };
    if let Some(error) = context.error.borrow_mut().take().or_else(|| allocator.take_error()) {
        return Err(error);
    }
    result.map_err(|_| RuntimeError::new("Canonical child-frame construction failed"))?;
    Ok(frame.cast())
}

/// Finishes the current master frame, releases its marks, and unpublishes it.
///
/// # Safety
/// `render` and `frame` are live objects from the active canonical draw.
pub(crate) unsafe fn finish_master_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical master-frame finish requires live renderer and frame"));
    }
    let graph = state(access)?;
    let mut publication = FramePublicationGuard { graph: &graph, frame, active: true };
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("Canonical master-frame renderer is not published"));
    }
    let ops = CoreSceneStackOps::new(access)?;
    let mut context = CoreMasterFrameContext { access, error: RefCell::new(None) };
    let frames = master_frame_services(&mut context, render, ops.stack_ptrs());
    let result = unsafe { hp2_render::scene_pipeline::helper_frames::finish_with(&frames, &ops) };
    let unpublish = publication.unpublish();
    if let Some(error) = context.error.borrow_mut().take().or_else(|| ops.take_error()) {
        return Err(error);
    }
    result.map_err(|_| RuntimeError::new("Canonical master-frame finish failed"))?;
    unpublish
}


/// Returns the one renderer graph state installed in the canonical Engine owner context.
pub(crate) fn state(access: CoreAccess<'_>) -> RuntimeResult<Rc<RenderGraphState>> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine render graph state is unavailable"))?;
    Ok(engine.render_graph.clone())
}
struct FramePublicationGuard<'a> {
    graph: &'a RenderGraphState,
    frame: *mut FSceneNode,
    active: bool,
}

impl FramePublicationGuard<'_> {
    fn unpublish(&mut self) -> RuntimeResult<()> {
        let result = self.graph.unpublish_frame(self.frame);
        self.active = false;
        result
    }
}

impl Drop for FramePublicationGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.graph.unpublish_frame(self.frame);
            self.active = false;
        }
    }
}


/// Flushes the Engine-owned object cache used by renderer initialization.
pub(crate) fn flush_cache(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let owner = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Render cache flush has no Engine owner"))?;
    let mut cache = owner.cache.try_borrow_mut()
        .map_err(|_| RuntimeError::new("Canonical Engine cache flush reentered"))?;
    cache.as_mut()
        .ok_or_else(|| RuntimeError::new("Canonical Engine cache is unavailable during Render::Init"))?
        .flush(access, 0, u32::MAX, 0)
}

/// Runs the exact shared `URender::Init` leaf order against canonical Core state.
///
/// # Safety
/// `render` and `engine` must be live records in this Engine owner's object graph.
pub(crate) unsafe fn initialize(
    access: CoreAccess<'_>,
    render: *mut URender,
    engine: *mut UEngine,
) -> RuntimeResult<()> {
    if render.is_null() || engine.is_null() {
        return Err(RuntimeError::new("Canonical Render::Init requires live renderer and Engine records"));
    }
    let graph = state(access)?;
    let settings = hp2_render::lifecycle::defaults();
    hp2_render::lifecycle::initialize(|leaf| match leaf {
        hp2_render::lifecycle::InitLeaf::DynamicMemory => graph.initialize_dynamic_memory(),
        hp2_render::lifecycle::InitLeaf::SceneMemory => graph.initialize_scene_memory(),
        hp2_render::lifecycle::InitLeaf::Base => unsafe {
            initialize_base(access, ptr::addr_of_mut!((*render).base), engine)
        },
        hp2_render::lifecycle::InitLeaf::GlobalRender => graph.publish_render(render),
        hp2_render::lifecycle::InitLeaf::ApplySettings => {
            unsafe { apply_settings(render, &settings) };
            graph.set_dynamic_light_surfs(settings.num_dyn_light_surfs);
            graph.set_dynamic_light_leaves(settings.num_dyn_light_leaves);
            Ok(())
        }
        hp2_render::lifecycle::InitLeaf::Cache => flush_cache(access),
        hp2_render::lifecycle::InitLeaf::VectorMemory => graph.initialize_vector_memory(),
        hp2_render::lifecycle::InitLeaf::Statistics => {
            graph.reset_statistics();
            Ok(())
        }
        hp2_render::lifecycle::InitLeaf::LightManager => {
            graph.initialize_light_manager()?;
            emit_renderer_log(access, "Init", LIGHTING_INITIALIZED)
        }
        hp2_render::lifecycle::InitLeaf::Log => {
            emit_renderer_log(access, "Init", RENDERING_INITIALIZED)
        }
    })
}

/// Runs the exact shared `URender::Destroy` leaf order against canonical Core state.
///
/// # Safety
/// `render` must be the live canonical URender being destroyed by this owner.
pub(crate) unsafe fn destroy(access: CoreAccess<'_>, render: *mut URender) -> RuntimeResult<()> {
    if render.is_null() {
        return Err(RuntimeError::new("Null canonical Render::Destroy target"));
    }
    let graph = state(access)?;
    hp2_render::lifecycle::destroy(|leaf| match leaf {
        hp2_render::lifecycle::DestroyLeaf::DynamicMemory => graph.exit_dynamic_memory(),
        hp2_render::lifecycle::DestroyLeaf::SceneMemory => graph.exit_scene_memory(),
        hp2_render::lifecycle::DestroyLeaf::DynamicLightStorage => {
            graph.release_dynamic_light_storage();
            Ok(())
        }
        hp2_render::lifecycle::DestroyLeaf::LightManager => {
            emit_renderer_log(access, "Exit", LIGHTING_SHUT_DOWN)
        }
        hp2_render::lifecycle::DestroyLeaf::VectorMemory => graph.exit_vector_memory(),
        hp2_render::lifecycle::DestroyLeaf::Log => {
            emit_renderer_log(access, "Exit", RENDERING_SHUT_DOWN)
        }
        hp2_render::lifecycle::DestroyLeaf::Base => {
            unsafe { (BASE.destroy)(access, render.cast())? };
            graph.unpublish_render(render)
        }
    })
}

fn emit_renderer_log(access: CoreAccess<'_>, category: &str, message: &[u32]) -> RuntimeResult<()> {
    let category = access.intern_str(category, hp2_core::name_owner::FindName::Add)?;
    access.emit_log(category, message)
}

unsafe fn apply_settings(render: *mut URender, settings: &hp2_render::lifecycle::RenderInitSettings) {
    unsafe {
        ptr::addr_of_mut!((*render).global_mesh_lod).write_unaligned(settings.global_mesh_lod);
        ptr::addr_of_mut!((*render).global_shape_lod).write_unaligned(settings.global_shape_lod);
        ptr::addr_of_mut!((*render).global_shape_lod_adjust).write_unaligned(settings.global_shape_lod_adjust);
        ptr::addr_of_mut!((*render).shape_lod_mode).write_unaligned(settings.shape_lod_mode);
        ptr::addr_of_mut!((*render).shape_lod_fix).write_unaligned(settings.shape_lod_fix);
        ptr::addr_of_mut!((*render).max_particle_size).write_unaligned(settings.max_particle_size);
        ptr::addr_of_mut!((*render).max_particle_render_time).write_unaligned(settings.max_particle_render_time);
        ptr::addr_of_mut!((*render).average_time).write_unaligned(settings.average_time);
        ptr::addr_of_mut!((*render).total_time).write_unaligned(settings.total_time);
        ptr::addr_of_mut!((*render).peak_frame_time).write_unaligned(settings.peak_frame_time);
    }
}

/// Executes the shared URender console grammar with direct canonical field and output leaves.
///
/// # Safety
/// `render` is a live canonical URender and `command` remains NUL-terminated and readable
/// for the synchronous call. `output` does not retain its borrowed message.
pub(crate) unsafe fn exec(
    _access: CoreAccess<'_>,
    render: *mut URender,
    command: *const u32,
    mut output: impl FnMut(&[u32]) -> RuntimeResult<()>,
) -> RuntimeResult<bool> {
    if render.is_null() {
        return Err(RuntimeError::new("Null canonical Render::Exec receiver"));
    }
    // SAFETY: the caller guarantees the live command buffer for the duration of dispatch.
    unsafe {
        hp2_render::console::exec_with(command.cast(), |leaf, text| {
            match leaf {
                hp2_render::console::Hp2RenderConsoleLeaf::StatNet => toggle(render, |r| ptr::addr_of_mut!((*r).net_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatFps => toggle(render, |r| ptr::addr_of_mut!((*r).fps_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatGlobal => toggle(render, |r| ptr::addr_of_mut!((*r).global_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatMesh => toggle(render, |r| ptr::addr_of_mut!((*r).mesh_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatParticle => toggle(render, |r| ptr::addr_of_mut!((*r).particle_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatActor => toggle(render, |r| ptr::addr_of_mut!((*r).actor_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatFilter => toggle(render, |r| ptr::addr_of_mut!((*r).filter_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatReject => toggle(render, |r| ptr::addr_of_mut!((*r).reject_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatSpan => toggle(render, |r| ptr::addr_of_mut!((*r).span_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatZone => toggle(render, |r| ptr::addr_of_mut!((*r).zone_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatLight => toggle(render, |r| ptr::addr_of_mut!((*r).light_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatOcclusion => toggle(render, |r| ptr::addr_of_mut!((*r).occlusion_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatGame => toggle(render, |r| ptr::addr_of_mut!((*r).game_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatSoft => toggle(render, |r| ptr::addr_of_mut!((*r).soft_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatCache => toggle(render, |r| ptr::addr_of_mut!((*r).cache_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatPolyV => toggle(render, |r| ptr::addr_of_mut!((*r).poly_v_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatPolyC => toggle(render, |r| ptr::addr_of_mut!((*r).poly_c_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatIllum => toggle(render, |r| ptr::addr_of_mut!((*r).illum_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatHardware => toggle(render, |r| ptr::addr_of_mut!((*r).hardware_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatEari => toggle(render, |r| ptr::addr_of_mut!((*r).eari_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatEariDetail => toggle(render, |r| ptr::addr_of_mut!((*r).eari_details)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatAnim => toggle(render, |r| ptr::addr_of_mut!((*r).anim_stats)),
                hp2_render::console::Hp2RenderConsoleLeaf::StatMissingAverage => output_text(&mut output, "Missing Average time")?,
                hp2_render::console::Hp2RenderConsoleLeaf::StatSetAverage => {
                    let value = atof_wide(text)?;
                    ptr::addr_of_mut!((*render).average_time).write_unaligned(value);
                    ptr::addr_of_mut!((*render).total_time).write_unaligned(0.0);
                    ptr::addr_of_mut!((*render).peak_frame_time).write_unaligned(0.0);
                    output_text(&mut output, &format!("Stat Average time set to {value:.6}"))?;
                }
                hp2_render::console::Hp2RenderConsoleLeaf::RendLeak => toggle(render, |r| ptr::addr_of_mut!((*r).leak_check)),
                hp2_render::console::Hp2RenderConsoleLeaf::RendWire => toggle(render, |r| ptr::addr_of_mut!((*r).wire_show)),
                hp2_render::console::Hp2RenderConsoleLeaf::RendBone => toggle(render, |r| ptr::addr_of_mut!((*r).bone_show)),
                hp2_render::console::Hp2RenderConsoleLeaf::RendBlend => toggle(render, |r| ptr::addr_of_mut!((*r).blend_show)),
                hp2_render::console::Hp2RenderConsoleLeaf::RendToggle => toggle(render, |r| ptr::addr_of_mut!((*r).toggle)),
                hp2_render::console::Hp2RenderConsoleLeaf::RendRecognized => output_text(&mut output, "Rendering option recognized")?,
                hp2_render::console::Hp2RenderConsoleLeaf::Tlod => {
                    let mut value = ptr::addr_of!((*render).global_mesh_lod).read_unaligned();
                    if !wide_empty(text) {
                        let parsed = atof_wide(text)?;
                        if parsed > 0.0 { value = parsed; ptr::addr_of_mut!((*render).global_mesh_lod).write_unaligned(value); }
                        output_text(&mut output, &format!("Global mesh texture LOD distance {value:.6}"))?;
                    } else { output_text(&mut output, &format!("{value:.6}"))?; }
                }
                hp2_render::console::Hp2RenderConsoleLeaf::Mlod => {
                    let mut value = ptr::addr_of!((*render).global_shape_lod).read_unaligned();
                    if !wide_empty(text) {
                        let parsed = atof_wide(text)?;
                        if parsed > 0.0 { value = parsed; ptr::addr_of_mut!((*render).global_shape_lod).write_unaligned(value); }
                        output_text(&mut output, &format!("Global mesh shape LOD distance {value:.6}"))?;
                    } else { output_text(&mut output, &format!("{value:.6}"))?; }
                }
                hp2_render::console::Hp2RenderConsoleLeaf::Mlmode => {
                    let mut value = ptr::addr_of!((*render).shape_lod_mode).read_unaligned();
                    if !wide_empty(text) {
                        let parsed = atoi_wide(text)?;
                        if parsed >= 0 { value = parsed; ptr::addr_of_mut!((*render).shape_lod_mode).write_unaligned(value); }
                        output_text(&mut output, &format!("Shape LOD draw mode {value}"))?;
                    } else { output_text(&mut output, &format!("{value}"))?; }
                }
                hp2_render::console::Hp2RenderConsoleLeaf::Mlfix => {
                    let mut value = ptr::addr_of!((*render).shape_lod_fix).read_unaligned();
                    if !wide_empty(text) {
                        let parsed = atof_wide(text)?;
                        if parsed >= 0.0 { value = parsed; ptr::addr_of_mut!((*render).shape_lod_fix).write_unaligned(value); }
                        output_text(&mut output, &format!("Shape LOD scaling fixed at {value:.6}"))?;
                    } else { output_text(&mut output, &format!("{value:.6}"))?; }
                }
            }
            Ok(())
        })
    }
    }

unsafe fn toggle(render: *mut URender, field: impl FnOnce(*mut URender) -> *mut u32) {
    unsafe {
        let field = field(render);
        field.write_unaligned(field.read_unaligned() ^ 1);
    }
}

fn output_text(output: &mut impl FnMut(&[u32]) -> RuntimeResult<()>, text: &str) -> RuntimeResult<()> {
    let units: Vec<u32> = text.chars().map(u32::from).collect();
    output(&units)
}

unsafe fn wide_empty(text: *const i32) -> bool {
    text.is_null() || unsafe { *text == 0 }
}

unsafe fn atof_wide(text: *const i32) -> RuntimeResult<f32> {
    if text.is_null() { return Err(RuntimeError::new("Null Render console scalar")); }
    // SAFETY: the scalar suffix is live and NUL-terminated; atof consumes this
    // shared ANSI conversion result synchronously before its ring slot can change.
    let ansi = unsafe { hp2_core::ansi::to_ansi_string(text) };
    // SAFETY: `to_ansi_string` returns the shared terminated appToAnsi buffer.
    Ok(unsafe { libc::atof(ansi.cast()) as f32 })
}

unsafe fn atoi_wide(text: *const i32) -> RuntimeResult<i32> {
    if text.is_null() { return Err(RuntimeError::new("Null Render console scalar")); }
    unsafe { hp2_core::ansi::atoi_checked(text) }
        .map_err(|value| RuntimeError::new(&format!("Render console integer is outside native range: {value}")))
}

/// Runs the shared Precache traversal over canonical Core object records.
pub(crate) unsafe fn precache(
    access: CoreAccess<'_>,
    render: *mut URender,
    viewport: *mut UViewport,
) -> RuntimeResult<()> {
    if render.is_null() || viewport.is_null() {
        return Err(RuntimeError::new("Canonical Render::Precache requires live records"));
    }
    let model_class = access.native_class("Engine.Model")?;
    let texture_class = access.native_class("Engine.Texture")?;
    let mut model_cursor = 0;
    let mut texture_cursor = 0;
    let mut model = ptr::null_mut::<UModel>();
    let mut texture = ptr::null_mut::<UTexture>();
    let query = hp2_render::precache::precache(|operation, index| unsafe {
        match operation {
            0 => {
                model = next_class_instance(access, model_class, &mut model_cursor).cast();
                Ok(i32::from(model.is_null()))
            }
            1 => Ok(if model.is_null() { 0 } else { (*model).surfs.array_num }),
            2 => {
                let surface = surface_at(model, index).map_err(|_| ())?;
                Ok(i32::from(!(*surface).texture.is_null()))
            }
            3 => {
                let surface = surface_at(model, index).map_err(|_| ())?;
                Ok((*surface).poly_flags as i32)
            }
            4 => {
                let surface = surface_at(model, index).map_err(|_| ())?;
                let surface_flags = (*surface).poly_flags & (0x0000_0002 | 0x0000_0800);
                let tex = (*surface).texture.cast::<UTexture>();
                (*tex).poly_flags |= surface_flags;
                Ok(0)
            }
            5 => {
                let surface = surface_at(model, index).map_err(|_| ())?;
                Ok(i32::from((*(*surface).texture.cast::<UTexture>()).b_parametric()))
            }
            6 => {
                let surface = surface_at(model, index).map_err(|_| ())?;
                let tex = (*surface).texture.cast::<UTexture>();
                lock_precache_unlock(access, viewport, tex).map_err(|_| ())?;
                Ok(0)
            }
            7 => Ok(0),
            8 => {
                texture = next_class_instance(access, texture_class, &mut texture_cursor).cast();
                Ok(i32::from(texture.is_null()))
            }
            9 => Ok(i32::from((*texture).b_parametric())),
            10 => {
                lock_precache_unlock(access, viewport, texture).map_err(|_| ())?;
                Ok(0)
            }
            11 => Ok(0),
            _ => Err(()),
        }
    });
    query.map_err(|_| RuntimeError::new("Canonical Render::Precache failed"))
}

unsafe fn next_class_instance(
    access: CoreAccess<'_>,
    expected_class: *mut UClass,
    cursor: &mut i32,
) -> *mut UObject {
    while *cursor < access.object_count() {
        let index = *cursor;
        *cursor += 1;
        let object = access.object_at(index);
        if !object.is_null() && class_is_child_of(unsafe { (*object).class.cast() }, expected_class) {
            return object;
        }
    }
    ptr::null_mut()
}


unsafe fn class_is_child_of(mut class: *mut UClass, expected: *mut UClass) -> bool {
    while !class.is_null() {
        if class == expected { return true; }
        class = unsafe { (*class).super_field.cast() };
    }
    false
}

unsafe fn surface_at(model: *mut UModel, index: i32) -> RuntimeResult<*mut FBspSurf> {
    if model.is_null() || index < 0 || index >= unsafe { (*model).surfs.array_num } {
        return Err(RuntimeError::new("Canonical model surface index is invalid"));
    }
    let surfaces = unsafe { (*model).surfs.data.cast::<FBspSurf>() };
    if surfaces.is_null() { return Err(RuntimeError::new("Canonical model surface storage is null")); }
    Ok(unsafe { surfaces.add(index as usize) })
}

unsafe fn lock_precache_unlock(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    texture: *mut UTexture,
) -> RuntimeResult<()> {
    if texture.is_null() { return Err(RuntimeError::new("Canonical texture is null")); }
    let device = unsafe { (*viewport).ren_dev.cast::<URenderDevice>() };
    let time = i64::from_ne_bytes(unsafe { (*viewport).last_update_time });
    let mut info = std::mem::MaybeUninit::<FTextureInfo>::uninit();
    let info = info.as_mut_ptr();
    unsafe { crate::texture_owner::lock(access, texture, info, time, -1, device)?; }
    let upload = unsafe {
        crate::render_device_owner::precache_texture(
            access,
            device,
            info,
            (*texture).poly_flags,
        )
    };
    let unlock = unsafe { crate::texture_owner::unlock(access, texture, info) };
    upload?;
    unlock
}

/// Runs the shared `URender::PreRender` sequence against canonical Core records.
pub(crate) unsafe fn pre_render(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical Render::PreRender requires live records"));
    }
    let graph = state(access)?;
    hp2_render::pre_render::run(|leaf| unsafe {
        use hp2_render::pre_render::PreRenderLeaf::*;
        match leaf {
            CopyEndTime => (*render).last_end_time = (*render).end_time,
            StartTime => (*render).start_time = hp2_core::unix_clock::seconds_slow().to_ne_bytes(),
            ClearCounts => { (*render).nodes_draw = 0; (*render).polys_draw = 0; }
            MarkMemory => *graph.source_mark.get() = access.scratch_memory().mark(),
            EnableFastMath => hp2_core::unix_process::enable_fast_math(),
            TickRandoms => {
                let viewport = (*frame).viewport.cast::<UViewport>();
                if viewport.is_null() || (*viewport).actor.is_null() {
                    return Err(RuntimeError::new("Canonical PreRender frame has no viewport actor"));
                }
                let actor = (*viewport).actor.cast::<AActor>();
                // `GetLevel()` is `AActor::XLevel`; `AActor::Level` is the LevelInfo.
                let level = (*actor).x_level.cast::<ULevel>();
                if level.is_null() { return Err(RuntimeError::new("Canonical PreRender actor has no level")); }
                let info = crate::package_map_level_owner::level_info(level)?;
                let time = (*info).time_seconds;
                let mut frand = || access.rand_range_float(0.0, 1.0);
                let mut last_ticks = graph.random_last_ticks.get();
                hp2_render::random::tick(
                    &mut frand,
                    time,
                    &mut *graph.random_bases.get(),
                    &mut *graph.random_values.get(),
                    &mut *graph.random_deltas.get(),
                    &mut last_ticks,
                );
                graph.random_last_ticks.set(last_ticks);
            }
        }
        Ok(())
    }).map_err(|_| RuntimeError::new("Canonical Render::PreRender failed"))
}

/// Applies the shared renderer timing and adaptive-detail update to Core records.
pub(crate) unsafe fn post_render(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Canonical Render::PostRender requires live records"));
    }
    let graph = state(access)?;
    let end_time = hp2_core::unix_clock::seconds_slow();
    unsafe { (*render).end_time = end_time.to_ne_bytes(); }
    let start_time = f64::from_ne_bytes(unsafe { (*render).start_time });
    let last_end_time = f64::from_ne_bytes(unsafe { (*render).last_end_time });
    let mut stats = graph.stats.get();
    let seconds_per_cycle = hp2_core::unix_clock::seconds_per_cycle();
    let mut timing = hp2_render::post_timing::RenderPostTiming {
        frame_time: (end_time - last_end_time) as f32,
        render_time: (end_time - start_time) as f32,
        seconds_per_cycle,
        process_time: hp2_core::unix_clock::process_seconds() as f32,
        last_process_time: graph.last_process_time.get(),
        stat_frame_time: stats.frame_time,
        stat_render_time: stats.render_time,
        stat_external_time: stats.external_time,
    };
    hp2_render::post_timing::update(&mut timing);
    graph.last_process_time.set(timing.last_process_time);
    stats.frame_time = timing.stat_frame_time;
    stats.render_time = timing.stat_render_time;
    stats.external_time = timing.stat_external_time;
    graph.stats.set(stats);

    let engine = unsafe { (*render).base.engine.cast::<UEngine>() };
    let client = if engine.is_null() {
        ptr::null_mut::<UClient>()
    } else {
        unsafe { (*engine).client.cast::<UClient>() }
    };
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 && !client.is_null() {
        let mut detail = hp2_render::post_render::RenderDetailSettings {
            min_desired_frame_rate: unsafe { (*client).min_desired_frame_rate },
            frame_time: timing.frame_time,
            global_shape_lod_adjust: unsafe { (*render).global_shape_lod_adjust },
            particle_raster_time: stats.particle_raster_time,
            seconds_per_cycle,
            max_particle_render_time: unsafe { (*render).max_particle_render_time },
            particle_area: stats.particle_area,
            particle_area_rendered: stats.particle_area_rendered,
            max_particle_size: unsafe { (*render).max_particle_size },
        };
        hp2_render::post_render::adjust_detail(&mut detail);
        unsafe {
            (*render).global_shape_lod_adjust = detail.global_shape_lod_adjust;
            (*render).max_particle_size = detail.max_particle_size;
        }
    }
    if hp2_abi::engine::generated::ENGINE_BUILD_STATS != 0 {
        // SAFETY: a live scene links its canonical Viewport and PlayerPawn, as in PostRender.
        let actor = unsafe {
            (*(*frame).viewport.cast::<UViewport>())
                .actor.cast::<hp2_abi::engine::generated::APlayerPawn>()
        };
        if matches!(
            unsafe { ptr::addr_of!((*actor).rend_map).read_unaligned() },
            hp2_abi::engine::generated::ENGINE_REN_POLYS
                | hp2_abi::engine::generated::ENGINE_REN_POLY_CUTS
                | hp2_abi::engine::generated::ENGINE_REN_DYN_LIGHT
                | hp2_abi::engine::generated::ENGINE_REN_PLAIN_TEX
        ) {
            unsafe { stats::draw_stats(access, render, frame)?; }
        }
    }
    Ok(())
}

/// Initializes one renderer-owned scratch arena exactly once.
fn initialize_stack(
    slot: &RefCell<Option<MemStackOwner>>,
    default_chunk_size: i32,
    tag: &'static [u32],
    name: &str,
) -> RuntimeResult<()> {
    let mut slot = slot.borrow_mut();
    if slot.is_some() {
        return Err(RuntimeError::new(&format!("Canonical {name} is already initialized")));
    }
    *slot = Some(MemStackOwner::new(default_chunk_size, tag));
    Ok(())
}

fn memory<'a>(slot: &'a RefCell<Option<MemStackOwner>>, name: &str) -> RuntimeResult<Ref<'a, MemStackOwner>> {
    Ref::filter_map(slot.borrow(), Option::as_ref)
        .map_err(|_| RuntimeError::new(&format!("Canonical {name} is not initialized")))
}

fn exit_stack(slot: &RefCell<Option<MemStackOwner>>, name: &str) -> RuntimeResult<()> {
    let mut slot = slot.borrow_mut();
    let Some(memory) = slot.as_ref() else {
        return Err(RuntimeError::new(&format!("Canonical {name} is not initialized")));
    };
    memory.exit()?;
    slot.take();
    Ok(())
}


/// Draws an actor through the canonical source dynamic-item renderer.
///
/// # Safety
/// `render`, `frame`, and `actor` are live canonical records for the synchronous draw.
pub(crate) unsafe fn draw_actor(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
    actor: *mut AActor,
) -> RuntimeResult<()> {
    unsafe { actor::draw_actor(access, render, frame, actor) }
}

#[cfg(test)]
mod light_manager_lifetime_tests {
    use super::RenderGraphState;
    use hp2_abi::engine::generated::FTextureInfo;

    #[test]
    fn nested_light_compute_preserves_active_surface_map_metadata() {
        let graph = RenderGraphState::new();
        graph.initialize_light_manager().unwrap();
        let manager = graph.light_manager.get();
        let light = unsafe { std::ptr::addr_of_mut!((*manager).LightMap).cast::<FTextureInfo>() };
        let fog = unsafe { std::ptr::addr_of_mut!((*manager).FogMap).cast::<FTextureInfo>() };
        for (map, u_scale, v_scale) in [(light, 34.15, 36.607143), (fog, 16.0, 32.0)] {
            unsafe {
                (*map).format = 1; // TEXF_RGBA7, not paletted diffuse data.
                (*map).u_scale = u_scale;
                (*map).v_scale = v_scale;
                (*map).pan = hp2_abi::FVector { x: -0.125, y: -256.125, z: 0.0 };
            }
        }

        // Surface setup invokes per-light computation within the same manager
        // scope; ensuring initialization there must not clear published maps.
        graph.with_light_manager(|| graph.initialize_light_manager()).unwrap();

        for (map, u_scale, v_scale) in [(light, 34.15, 36.607143), (fog, 16.0, 32.0)] {
            let map = unsafe { map.read_unaligned() };
            assert_eq!(
                (map.format, map.u_scale, map.v_scale, map.pan.x, map.pan.y),
                (1, u_scale, v_scale, -0.125, -256.125),
            );
        }
    }
}

#[cfg(test)]
pub(crate) mod render_test_support {
    use crate::EngineOwner;
    use hp2_core::core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult};
    use std::{fs, io::Write, path::{Path, PathBuf}};

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn output(access: CoreAccess<'_>, category: i32, text: &[u32]) -> RuntimeResult<()> {
        access.write_log_record(&access.name_text(category)?, text)
    }

    pub(crate) fn owner(label: &str) -> EngineOwner {
        let data = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prototype data"));
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("render-{label}-{}", std::process::id()));
        copy_tree(&data.join("System"), &root.join("System"));
        for directory in ["Maps", "Textures", "Sounds", "Music"] {
            let source = data.join(directory);
            if source.is_dir() {
                std::os::unix::fs::symlink(fs::canonicalize(source).unwrap(), root.join(directory)).unwrap();
            }
        }
        let mut log = fs::File::create(root.join("runtime.log")).unwrap();
        log.write_all(&[0xff, 0xfe]).unwrap();
        let owner = EngineOwner::new(CoreOptions {
            data_root: root.clone(), system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(),
            output, log_file: Some(log),
        }).unwrap();
        let access = owner.access();
        access.set_process_mode(CoreProcessMode {
            is_client: true, is_server: true, is_scriptable: true, ..access.process_mode()
        });
        unsafe { access.load_package(&root.join("System/Engine.u"), 1) }.unwrap();
        owner
    }

    pub(crate) fn construct(
        access: CoreAccess<'_>, class_name: &str,
    ) -> *mut hp2_abi::engine::generated::UObject {
        let name: Vec<u32> = class_name.chars().map(u32::from).chain([0]).collect();
        let class = unsafe {
            access.static_load_class(
                access.native_class("Core.Object").unwrap(), std::ptr::null_mut(), &name, 1,
            )
        }.unwrap();
        unsafe {
            access.construct_object(class, access.transient_package(), 0, 0x4000, std::ptr::null_mut())
        }.unwrap()
    }
}
