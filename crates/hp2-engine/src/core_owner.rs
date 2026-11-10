//! One canonical Core graph for the implemented Engine resource/metadata classes.

use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::NativeClassSpec;
use hp2_abi::engine::generated::{AActor, FName, UClass, UClient, UEngine, UObject, USound};
use std::{cell::{Cell, RefCell}, ptr::NonNull, rc::Rc};
/// Events emitted by canonical spawn and CreatureGenerator native paths.
#[derive(Clone, Copy)]
pub enum SpawnInstrumentationEvent {
    TransitionRequest { class: *mut UClass, name: FName, owner: *mut AActor },
    CreatureRequest { class: *mut UClass, owner: *mut AActor },
    TransitionResult { owner: *mut AActor, actor: *mut AActor },
    CreatureResult { owner: *mut AActor, actor: *mut AActor },
    TransitionPublished { owner: *mut AActor, actor: *mut AActor },
    CreaturePublished { owner: *mut AActor, actor: *mut AActor },
    PreBeginBefore { actor: *mut AActor },
    PreBeginAfter { actor: *mut AActor },
    SoftwareRendering { actor: *mut AActor, software: bool },
}

/// Receives typed launcher instrumentation events without native callback tables.
pub trait SpawnInstrumentation {
    fn on_spawn_instrumentation(
        &self,
        access: CoreAccess<'_>,
        event: SpawnInstrumentationEvent,
    ) -> RuntimeResult<()>;
}

/// Receives the distinct `OnActorSpawned` publication notification.
pub type ActorSpawnedObserver = fn(CoreAccess<'_>, NonNull<AActor>) -> RuntimeResult<()>;

/// The canonical Rust sound registry's `UnregisterSound` operation.
pub type AudioUnregisterSound =
    fn(CoreAccess<'_>, NonNull<UObject>, NonNull<USound>) -> RuntimeResult<()>;
/// The canonical Rust audio subsystem's `NoteDestroy` operation.
pub type AudioNoteDestroy =
    fn(CoreAccess<'_>, NonNull<UObject>, NonNull<AActor>) -> RuntimeResult<()>;
/// Native lifecycle stages retain their identity across Rust observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorLifecycleEvent {
    Spawned,
    PreBeginPlay,
    BeginPlay,
    PostBeginPlay,
    SetInitialState,
    ProcessState,
    Destroying,
}

/// Optional canonical Rust lifecycle observation, replacing the test-only
/// `GActorLifecycleObserver` C++ seam.
pub type ActorLifecycleObserver =
    fn(CoreAccess<'_>, NonNull<AActor>, ActorLifecycleEvent) -> RuntimeResult<()>;

/// Source actor movement callbacks observed during canonical collision changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorMovementEvent {
    Bump,
    Touch,
    UnTouch,
}
pub type ActorMovementObserver =
    fn(CoreAccess<'_>, ActorMovementEvent, NonNull<AActor>, NonNull<AActor>) -> RuntimeResult<()>;


/// Reports one actor movement/touch event to the owner-local observer, if installed.
pub(crate) fn observe_actor_movement(
    access: CoreAccess<'_>,
    event: ActorMovementEvent,
    recipient: *mut AActor,
    other: *mut AActor,
) -> RuntimeResult<()> {
    let state = access.owner_context::<EngineState>()
        .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
    if let Some(observer) = state.actor_movement_observer.get() {
        let recipient = NonNull::new(recipient)
            .ok_or_else(|| RuntimeError::new("Null Actor recipient in movement observer"))?;
        let other = NonNull::new(other)
            .ok_or_else(|| RuntimeError::new("Null Actor other in movement observer"))?;
        observer(access, event, recipient, other)?;
    }
    Ok(())
}

/// Source Fire texture trace observation emitted only when a Rust observer is installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireTraceEvent {
    InitTables { before: u64, after: u64, fingerprint: u64 },
    SpeedRand { index: u32, value: u8 },
    BurnWrite {
        texture: *mut std::ffi::c_void,
        spark: i32,
        offset: u32,
        before: u8,
        value: u8,
    },
}

/// Source `HP2ActorSlotDump*` callbacks at level deserialize boundaries.
#[derive(Clone, Copy)]
pub enum ActorSlotDumpEvent {
    PostDeserialize { level: *mut std::ffi::c_void },
    BeginRawTable { level: *mut std::ffi::c_void },
    RawActor {
        level: *mut std::ffi::c_void,
        slot: i32,
        actor: *mut std::ffi::c_void,
        reference_offset_before: i32,
        reference_offset_after: i32,
        compact_package_index: i32,
        compact_package_index_captured: bool,
        compact_package_index_supported: bool,
    },
    EndRawTable { level: *mut std::ffi::c_void },
}
/// Optional synchronous actor-slot reporter hook owned by one canonical Engine graph.
pub type ActorSlotDumpObserver = fn(CoreAccess<'_>, ActorSlotDumpEvent) -> RuntimeResult<()>;
/// Returns whether the canonical Engine graph has its optional source actor-slot provider.
pub fn actor_slot_dump_available(access: CoreAccess<'_>) -> RuntimeResult<bool> {
    let state = access.owner_context::<EngineState>()
        .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
    Ok(state.actor_slot_dump_observer.get().is_some())
}

/// Forwards one genuine source actor-slot deserialize observation.
pub fn observe_actor_slot_dump(
    access: CoreAccess<'_>,
    event: ActorSlotDumpEvent,
) -> RuntimeResult<()> {
    let state = access.owner_context::<EngineState>()
        .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
    if let Some(observer) = state.actor_slot_dump_observer.get() {
        observer(access, event)
    } else {
        Err(RuntimeError::new("Canonical actor-slot dump observer is not installed"))
    }
}
/// Optional synchronous observer for the three source Fire trace events.
pub type FireTraceObserver = fn(CoreAccess<'_>, FireTraceEvent) -> RuntimeResult<()>;

/// Complete synchronous `OnActorAllActors` payload, including the terminal scan event.
#[derive(Clone, Copy)]
pub struct ActorAllActorsEvent {
    pub requestor: NonNull<AActor>,
    pub requested_class: NonNull<UClass>,
    pub tag: i32,
    pub yielded: Option<NonNull<AActor>>,
    pub slot: i32,
    pub exhausted: bool,
}
pub type ActorAllActorsObserver =
    fn(CoreAccess<'_>, ActorAllActorsEvent) -> RuntimeResult<()>;

/// A weak canonical audio receiver and its genuine Rust sound operations.
/// Audio initialization publishes this binding; destruction clears it before
/// releasing the receiver. Neither pointer may identify a shipping C++ object.
#[derive(Clone, Copy)]
pub(crate) struct AudioBinding {
    pub(crate) receiver: NonNull<UObject>,
    pub(crate) unregister_sound: AudioUnregisterSound,
    pub(crate) note_destroy: AudioNoteDestroy,
}

/// Weak bitmap-client identity, matching native UBitmap::__Client.
/// A present identity must be a live canonical client in this owner, never a
/// shipping C++ client. Client initialization/destruction publish and clear it.
pub(crate) struct EngineState {
    pub(crate) bitmap_client: Cell<Option<NonNull<UClient>>>,
    pub(crate) texture_lod_cache: Cell<Option<[i32; 8]>>,
    pub(crate) stock_fps_menu: RefCell<crate::stock_fps_menu::State>,
    pub(crate) sound_audio: Cell<Option<AudioBinding>>,
    pub(crate) platform: Rc<crate::platform_owner::PlatformOwnerState>,
    pub(crate) audio_resources: Rc<crate::audio_owner::AudioOwnerState>,
    pub(crate) render_resources: Rc<crate::render_device_owner::RenderOwnerState>,
    pub(crate) render_graph: Rc<crate::render_owner::RenderGraphState>,
    pub(crate) console: Rc<crate::console_owner::ConsoleOwnerState>,
    pub(crate) exec_hook: Cell<Option<crate::engine_exec_owner::ExecHook>>,
    pub(crate) load_slot_consumed: Cell<bool>,
    pub(crate) repair_save_slot: Cell<i32>,
    pub(crate) repair_save_pending: Cell<i32>,
    pub(crate) actor_lifecycle_observer: Cell<Option<ActorLifecycleObserver>>,
    pub(crate) actor_all_actors_observer: Cell<Option<ActorAllActorsObserver>>,
    pub(crate) actor_movement_observer: Cell<Option<ActorMovementObserver>>,
    pub(crate) actor_spawned_observer: Cell<Option<ActorSpawnedObserver>>,
    pub(crate) fire_trace_observer: Cell<Option<FireTraceObserver>>,
    pub(crate) actor_slot_dump_observer: Cell<Option<ActorSlotDumpObserver>>,
    pub(crate) spawn_instrumentation: RefCell<Option<Rc<dyn SpawnInstrumentation>>>,
    pub(crate) wind: crate::wind_owner::WindState,
    pub(crate) url_defaults: std::cell::RefCell<std::rc::Rc<crate::url_owner::UrlDefaults>>,
    pub(crate) cache: std::cell::RefCell<Option<hp2_core::cache_owner::CacheOwner>>,
    pub(crate) particle_lists: RefCell<std::collections::HashMap<usize, crate::particle_list_owner::ParticleListState>>,
    /// Per actor class: byte offsets of every object-reference slot, in the order
    /// `level_lifecycle_owner` visits them when it clears references to deleted actors.
    pub(crate) cleanup_slots: RefCell<std::collections::HashMap<usize, std::rc::Rc<[usize]>>>,
    pub(crate) memory: hp2_core::mem_owner::MemStackOwner,
}

impl EngineState {
    fn new(fire_trace_observer: Option<FireTraceObserver>) -> RuntimeResult<Self> {
        Ok(Self {
            actor_lifecycle_observer: Cell::new(None),
            actor_all_actors_observer: Cell::new(None),
            actor_movement_observer: Cell::new(None),
            actor_spawned_observer: Cell::new(None),
            spawn_instrumentation: RefCell::new(None),
            fire_trace_observer: Cell::new(fire_trace_observer),
            actor_slot_dump_observer: Cell::new(None),
            bitmap_client: Cell::new(None),
            texture_lod_cache: Cell::new(None),
            stock_fps_menu: RefCell::new(crate::stock_fps_menu::State::default()),
            sound_audio: Cell::new(None),
            platform: Rc::new(crate::platform_owner::PlatformOwnerState::default()),
            audio_resources: Rc::default(),
            render_resources: Rc::default(),
            render_graph: Rc::new(crate::render_owner::RenderGraphState::new()),
            console: Rc::default(),
            exec_hook: Cell::new(None),
            load_slot_consumed: Cell::new(false),
            repair_save_slot: Cell::new(-1),
            repair_save_pending: Cell::new(0),
            wind: crate::wind_owner::WindState::default(),
            url_defaults: Default::default(),
            cache: std::cell::RefCell::new(Some(hp2_core::cache_owner::CacheOwner::uninitialized()?)),
            particle_lists: RefCell::new(std::collections::HashMap::new()),
            cleanup_slots: RefCell::new(std::collections::HashMap::new()),
            memory: hp2_core::mem_owner::MemStackOwner::new(
                crate::uengine::ENGINE_MEM_BYTES, &[71, 69, 110, 103, 105, 110, 101, 77, 101, 109, 0],
            ),
        })
    }
}

// Compose immutable declarations at compile time, not a second registry or an
// allocated/leaked startup list. All identities are published by CoreOwner.
static NATIVE_CLASSES: &[NativeClassSpec] = &[
    crate::engine_owner::NATIVE_CLASSES[0],
    crate::audio_owner::NATIVE_CLASSES[0],
    crate::audio_owner::NATIVE_CLASSES[1],
    crate::render_device_owner::NATIVE_CLASSES[0],
    crate::render_device_owner::NATIVE_CLASSES[1],
    crate::console_owner::NATIVE_CLASSES[0],
    crate::canvas_owner::NATIVE_CLASSES[0],
    crate::render_owner::NATIVE_CLASSES[0],
    crate::render_owner::NATIVE_CLASSES[1],
    crate::game_engine_owner::NATIVE_CLASSES[0],
    crate::player_owner::NATIVE_CLASSES[0],
    crate::platform_owner::NATIVE_CLASSES[0],
    crate::platform_owner::NATIVE_CLASSES[1],
    crate::platform_owner::NATIVE_CLASSES[2],
    crate::platform_owner::NATIVE_CLASSES[3],
    crate::platform_owner::NATIVE_CLASSES[4],
    crate::connection_owner::NATIVE_CLASSES[0],
    crate::network_owner::NATIVE_CLASSES[0],
    crate::network_owner::NATIVE_CLASSES[1],
    crate::network_owner::NATIVE_CLASSES[2],
    crate::network_owner::NATIVE_CLASSES[3],
    crate::network_owner::NATIVE_CLASSES[4],
    crate::channel_owner::NATIVE_CLASSES[0],
    crate::channel_owner::NATIVE_CLASSES[1],
    crate::actor_channel_owner::NATIVE_CLASSES[0],
    crate::file_channel_owner::NATIVE_CLASSES[0],
    crate::download_owner::NATIVE_CLASSES[0],
    crate::download_owner::NATIVE_CLASSES[1],
    crate::demo_owner::NATIVE_CLASSES[0],
    crate::demo_owner::NATIVE_CLASSES[1],
    crate::primitive_owner::NATIVE_CLASSES[0],
    crate::primitive_owner::NATIVE_CLASSES[1],
    crate::primitive_owner::NATIVE_CLASSES[2],
    crate::particle_list_owner::NATIVE_CLASSES[0],
    crate::particle_list_owner::NATIVE_CLASSES[1],
    crate::primitive_shape_owner::NATIVE_CLASSES[0],
    crate::primitive_shape_owner::NATIVE_CLASSES[1],
    crate::primitive_shape_owner::NATIVE_CLASSES[2],
    crate::model_owner::NATIVE_CLASSES[0],
    crate::model_owner::NATIVE_CLASSES[1],
    crate::model_owner::NATIVE_CLASSES[2],
    crate::model_owner::NATIVE_CLASSES[3],
    crate::model_owner::NATIVE_CLASSES[4],
    crate::model_owner::NATIVE_CLASSES[5],
    crate::animation_owner::NATIVE_CLASSES[0],
    crate::animation_owner::NATIVE_CLASSES[1],
    crate::mesh_owner::NATIVE_CLASSES[0],
    crate::mesh_owner::NATIVE_CLASSES[1],
    crate::skeletal_owner::NATIVE_CLASSES[0],
    crate::palette_owner::NATIVE_CLASSES[0],
    crate::sound_owner::NATIVE_CLASSES[0],
    crate::sound_owner::NATIVE_CLASSES[1],
    crate::texture_owner::NATIVE_CLASSES[0],
    crate::texture_owner::NATIVE_CLASSES[1],
    crate::fractal_owner::NATIVE_CLASSES[0],
    crate::fractal_owner::NATIVE_CLASSES[1],
    crate::fractal_owner::NATIVE_CLASSES[2],
    crate::fractal_owner::NATIVE_CLASSES[3],
    crate::fractal_owner::NATIVE_CLASSES[4],
    crate::fractal_owner::NATIVE_CLASSES[5],
    crate::texture_owner::image::NATIVE_CLASSES[0],
    crate::scripted_texture_owner::NATIVE_CLASSES[0],
    crate::font_owner::NATIVE_CLASSES[0],
    crate::actor_owner::NATIVE_CLASSES[0],
    crate::actor_owner::NATIVE_CLASSES[1],
    crate::actor_owner::NATIVE_CLASSES[2],
    crate::actor_owner::NATIVE_CLASSES[3],
    crate::actor_owner::NATIVE_CLASSES[4],
    crate::decal_owner::NATIVE_CLASSES[0],
    crate::particle_fx_owner::NATIVE_CLASSES[0],
    crate::navigation_owner::NATIVE_CLASSES[0],
    crate::navigation_owner::NATIVE_CLASSES[1],
    crate::trigger_owner::NATIVE_CLASSES[0],
    crate::trigger_owner::NATIVE_CLASSES[1],
    crate::brush_owner::NATIVE_CLASSES[0],
    crate::mover_owner::NATIVE_CLASSES[0],
    crate::info_owner::NATIVE_CLASSES[0],
    crate::info_owner::NATIVE_CLASSES[6],
    crate::info_owner::NATIVE_CLASSES[7],
    crate::info_owner::NATIVE_CLASSES[8],
    crate::info_owner::NATIVE_CLASSES[3],
    crate::info_owner::NATIVE_CLASSES[4],
    crate::info_owner::NATIVE_CLASSES[5],
    crate::game_info_owner::NATIVE_CLASSES[0],
    crate::actor_files_owner::NATIVE_CLASSES[0],
    crate::stat_log_owner::NATIVE_CLASSES[0],
    crate::stat_log_owner::NATIVE_CLASSES[1],
    crate::stat_log_owner::NATIVE_CLASSES[2],
    crate::editor_transaction_owner::NATIVE_CLASSES[0],
    crate::editor_transaction_owner::NATIVE_CLASSES[1],
    crate::info_owner::NATIVE_CLASSES[1],
    crate::warp_owner::NATIVE_CLASSES[0],
    crate::info_owner::NATIVE_CLASSES[2],
    crate::pawn_owner::NATIVE_CLASSES[0],
    crate::pawn_owner::NATIVE_CLASSES[1],
    crate::level_owner::NATIVE_CLASSES[0],
    crate::level_owner::NATIVE_CLASSES[1],
    crate::wind_owner::NATIVE_CLASSES[0],
    crate::level_summary::NATIVE_CLASSES[0],
];

/// Owns Core and the implemented canonical Engine resource/metadata classes.
///
/// This is an object/package owner, not a world, renderer or editor runtime.
/// Native shipping objects are not admitted to its graph.
pub struct EngineOwner {
    core: CoreOwner,
}

impl EngineOwner {
    /// Initializes Core and publishes the implemented Rust Engine classes.
    ///
    /// # Errors
    /// Returns initialization or class-registration failures.
    pub fn new(options: CoreOptions) -> RuntimeResult<Self> {
        Self::initialize(options, None)
    }

    /// Installs source Fire observation before native class-default constructors.
    pub fn new_with_fire_trace_observer(
        options: CoreOptions,
        observer: FireTraceObserver,
    ) -> RuntimeResult<Self> {
        Self::initialize(options, Some(observer))
    }

    fn initialize(options: CoreOptions, observer: Option<FireTraceObserver>) -> RuntimeResult<Self> {
        // SAFETY: static declarations use compiler-probed layouts and complete
        // Rust ClassOps. Every object/reference belongs to the same CoreOwner.
        let core = unsafe {
            CoreOwner::with_native_classes_and_context(options, NATIVE_CLASSES, EngineState::new(observer)?)?
        };
        let access = core.access();
        access.set_cache_command_handler(crate::engine_exec_owner::execute_cache_command_actual);
        access.register_vm_natives(crate::engine_vm_native::ENGINE_VM_NATIVES)?;
        access.system()?;
        Ok(Self { core })
    }

    /// Borrows the owner's canonical object graph synchronously.
    pub fn access(&self) -> CoreAccess<'_> {
        self.core.access()
    }

    /// Publishes the canonical Rust audio receiver and its real sound operations.
    ///
    /// # Errors
    /// Rejects an engine or receiver not owned by this Core graph or a receiver
    /// that does not equal the engine's current `Audio` field.
    pub fn publish_audio(
        &self,
        engine: NonNull<UEngine>,
        receiver: NonNull<UObject>,
        unregister_sound: AudioUnregisterSound,
        note_destroy: AudioNoteDestroy,
    ) -> RuntimeResult<()> {
        let access = self.access();
        let engine_object = engine.as_ptr().cast::<UObject>();
        let engine_index = unsafe { (*engine_object).index };
        if engine_index < 0 || access.object_at(engine_index) != engine_object {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Engine audio publisher is not owned by this Core",
            ));
        }
        if unsafe { (*engine.as_ptr()).audio } != receiver.as_ptr().cast() {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Audio receiver does not match Engine.Audio",
            ));
        }
        let object = receiver.as_ptr();
        let index = unsafe { (*object).index };
        if index < 0 || access.object_at(index) != object {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Audio receiver is not owned by this Core",
            ));
        }
        let state = access.owner_context::<EngineState>().ok_or_else(|| {
            hp2_core::core_access::RuntimeError::new("Engine owner context is unavailable")
        })?;
        state.sound_audio.set(Some(AudioBinding {
            receiver,
            unregister_sound,
            note_destroy,
        }));
        Ok(())
    }

    /// Clears the published audio receiver before its canonical teardown.
    ///
    /// # Errors
    /// Rejects a receiver that is not the current canonical `Engine.Audio`.
    pub fn clear_audio(
        &self,
        engine: NonNull<UEngine>,
        receiver: NonNull<UObject>,
    ) -> RuntimeResult<()> {
        let access = self.access();
        let engine_object = engine.as_ptr().cast::<UObject>();
        let engine_index = unsafe { (*engine_object).index };
        if engine_index < 0 || access.object_at(engine_index) != engine_object {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Engine audio publisher is not owned by this Core",
            ));
        }
        if unsafe { (*engine.as_ptr()).audio } != receiver.as_ptr().cast() {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Audio receiver does not match Engine.Audio",
            ));
        }
        let state = access.owner_context::<EngineState>().ok_or_else(|| {
            hp2_core::core_access::RuntimeError::new("Engine owner context is unavailable")
        })?;
        match state.sound_audio.get() {
            Some(binding) if binding.receiver == receiver => {
                state.sound_audio.set(None);
                Ok(())
            }
            _ => Err(hp2_core::core_access::RuntimeError::new(
                "Audio receiver does not match the published binding",
            )),
        }
    }

    /// Publishes a Rust observer for canonical actor lifecycle stages and teardown.
    pub fn publish_actor_lifecycle_observer(
        &self,
        observer: ActorLifecycleObserver,
    ) -> RuntimeResult<()> {
        let access = self.access();
        let state = access.owner_context::<EngineState>().ok_or_else(|| {
            hp2_core::core_access::RuntimeError::new("Engine owner context is unavailable")
        })?;
        state.actor_lifecycle_observer.set(Some(observer));
        Ok(())
    }

    /// Removes the canonical actor lifecycle observer.
    pub fn clear_actor_lifecycle_observer(&self) -> RuntimeResult<()> {
        let access = self.access();
        let state = access.owner_context::<EngineState>().ok_or_else(|| {
            hp2_core::core_access::RuntimeError::new("Engine owner context is unavailable")
        })?;
        state.actor_lifecycle_observer.set(None);
        Ok(())
    }

    /// Installs an optional Rust observer for the source Fire trace events.
    pub fn publish_fire_trace_observer(&self, observer: FireTraceObserver) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.fire_trace_observer.set(Some(observer));
        Ok(())
    }

    /// Removes the optional Rust observer for source Fire trace events.
    pub fn clear_fire_trace_observer(&self) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.fire_trace_observer.set(None);
        Ok(())
    }

    /// Installs the optional Rust owner-local source actor-slot provider.
    pub fn publish_actor_slot_dump_observer(&self, observer: ActorSlotDumpObserver) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.actor_slot_dump_observer.set(Some(observer));
        Ok(())
    }

    /// Detaches the source actor-slot provider before EngineOwner shutdown.
    pub fn clear_actor_slot_dump_observer(&self) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.actor_slot_dump_observer.set(None);
        Ok(())
    }

    /// Publishes genuine Rust observation of AllActors yields and natural exhaustion.
    pub fn publish_actor_all_actors_observer(&self, observer: ActorAllActorsObserver) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.actor_all_actors_observer.set(Some(observer));
        Ok(())
    }

    pub fn clear_actor_all_actors_observer(&self) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.actor_all_actors_observer.set(None);
        Ok(())
    }

    /// Publishes the genuine optional Rust actor-movement observer.
    pub fn publish_actor_movement_observer(
        &self,
        observer: ActorMovementObserver,
    ) -> RuntimeResult<()> {
        let access = self.access();
        let state = access.owner_context::<EngineState>().ok_or_else(|| {
            RuntimeError::new("Engine owner context is unavailable")
        })?;
        state.actor_movement_observer.set(Some(observer));
        Ok(())
    }

    /// Clears the optional Rust actor-movement observer.
    pub fn clear_actor_movement_observer(&self) -> RuntimeResult<()> {
        let access = self.access();
        let state = access.owner_context::<EngineState>().ok_or_else(|| {
            RuntimeError::new("Engine owner context is unavailable")
        })?;
        state.actor_movement_observer.set(None);
        Ok(())
    }
    /// Publishes a canonical Rust `OnActorSpawned` observer.
    pub fn publish_actor_spawned_observer(&self, observer: ActorSpawnedObserver) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.actor_spawned_observer.set(Some(observer));
        Ok(())
    }

    /// Clears the canonical Rust `OnActorSpawned` observer.
    pub fn clear_actor_spawned_observer(&self) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        state.actor_spawned_observer.set(None);
        Ok(())
    }

    /// Publishes typed Rust instrumentation for canonical Actor spawn events.
    pub fn publish_spawn_instrumentation(&self, provider: Rc<dyn SpawnInstrumentation>) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        *state.spawn_instrumentation.borrow_mut() = Some(provider);
        Ok(())
    }

    /// Removes the optional canonical Actor spawn instrumentation provider.
    pub fn clear_spawn_instrumentation(&self) -> RuntimeResult<()> {
        let state = self.access().owner_context::<EngineState>()
            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
        *state.spawn_instrumentation.borrow_mut() = None;
        Ok(())
    }

    /// Runs Core's source-ordered object pre-exit phase without releasing the owner.
    ///
    /// Observer callbacks must be detached before this closes the object graph.
    pub fn pre_exit(&self) -> RuntimeResult<()> {
        self.core.pre_exit()
    }

    /// Destroys objects and class storage before releasing this owner.
    ///
    /// # Errors
    /// Returns the first ordered teardown error.
    pub fn shutdown(self) -> RuntimeResult<()> {
        // Let a background save finish before the process (and its parent) goes away.
        let waited = crate::game_save_owner::finish_pending_save(self.core.access());
        let result = self.core.shutdown();
        waited.and(result)
    }
}
