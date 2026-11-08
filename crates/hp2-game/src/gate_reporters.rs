use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, FBox, FCheckResult, FName, FRotator, FStateFrame, UClass, UEngine, ULevel, UObject,
    UProperty, UState, UStruct,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_engine::{ActorAllActorsEvent, ActorLifecycleEvent, ActorMovementEvent, EngineOwner};
use hp2_launcher::{
    boot::Hp2LauncherBootLeaf as BootLeaf,
    main_loop::{
        Hp2LauncherMainLoopReporter as Reporter, Hp2LauncherMainLoopReporterOperation as Operation,
    },
    reporters::{
        lifecycle_gate::{self, Hp2LifecycleGateServices},
        static_bsp_probe::{self, Hp2StaticBspProbeServices},
        world_collision_gate::{self, Hp2WorldCollisionGateServices, MovementEvent, Protocol},
    },
};
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, c_void},
    ptr,
};

const ZERO_VECTOR: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};
const ZERO_ROTATOR: FRotator = FRotator {
    pitch: 0,
    yaw: 0,
    roll: 0,
};

struct ObserverContext {
    lifecycle: Cell<*mut lifecycle_gate::State>,
    lifecycle_active: Cell<bool>,
    movement: RefCell<Option<world_collision_gate::MovementObserver>>,
}

thread_local! {
    static OBSERVER_CONTEXT: Cell<*mut ObserverContext> = const { Cell::new(ptr::null_mut()) };
}

/// Owns the three opt-in gate reporters and their canonical Engine observer hooks.
pub(super) struct GateReporters {
    owner: *const EngineOwner,
    context: Box<ObserverContext>,
    lifecycle: Option<Box<lifecycle_gate::State>>,
    world_collision: Option<Box<world_collision_gate::State>>,
    static_bsp: Option<Box<static_bsp_probe::State>>,
    lifecycle_hooks: bool,
    movement_hook: bool,
}

impl GateReporters {
    /// Creates the stable callback context before any reporter can be installed.
    pub(super) fn new(owner: *const EngineOwner) -> RuntimeResult<Self> {
        if owner.is_null() {
            return Err(RuntimeError::new(
                "Gate reporters require a live EngineOwner",
            ));
        }
        let already_active = OBSERVER_CONTEXT.with(|active| !active.get().is_null());
        if already_active {
            return Err(RuntimeError::new(
                "Gate reporter observer context is already active",
            ));
        }
        let mut context = Box::new(ObserverContext {
            lifecycle: Cell::new(ptr::null_mut()),
            lifecycle_active: Cell::new(false),
            movement: RefCell::new(None),
        });
        OBSERVER_CONTEXT.with(|active| active.set((&mut *context) as *mut ObserverContext));
        Ok(Self {
            owner,
            context,
            lifecycle: None,
            world_collision: None,
            static_bsp: None,
            lifecycle_hooks: false,
            movement_hook: false,
        })
    }

    /// Handles only lifecycle, world-collision and static-BSP boot leaves.
    pub(super) fn boot_leaf(
        &mut self,
        leaf: BootLeaf,
        argument: i32,
    ) -> RuntimeResult<Option<i32>> {
        match leaf {
            BootLeaf::LifecycleEnabled => Ok(Some(i32::from(lifecycle_gate::is_enabled()))),
            BootLeaf::CreateLifecycle => {
                let state = lifecycle_gate::create(Hp2LifecycleGateServices {
                    stable_actor_key: lifecycle_stable_actor_key,
                    state_name: lifecycle_state_name,
                    state_pc_diagnostic: lifecycle_state_pc_diagnostic,
                    actor_state_latent_is_sleep: lifecycle_actor_state_latent_is_sleep,
                });
                let state_ptr =
                    (&*state) as *const lifecycle_gate::State as *mut lifecycle_gate::State;
                self.lifecycle = Some(state);
                self.context.lifecycle.set(state_ptr);
                if !self.lifecycle_hooks {
                    if let Err(error) = self.install_lifecycle_hooks() {
                        if !self.lifecycle_hooks {
                            self.context.lifecycle.set(ptr::null_mut());
                            self.lifecycle = None;
                        }
                        return Err(error);
                    }
                }
                Ok(Some(0))
            }
            BootLeaf::WorldCollisionFullEnabled => {
                Ok(Some(i32::from(world_collision_gate::is_enabled())))
            }
            BootLeaf::WorldCollisionTouchEnabled => {
                Ok(Some(i32::from(world_collision_gate::is_touch_enabled())))
            }
            BootLeaf::WorldCollisionBumpEnabled => {
                Ok(Some(i32::from(world_collision_gate::is_bump_enabled())))
            }
            BootLeaf::ErrorWorldCollisionConflict => Err(RuntimeError::new(
                "only one world collision gate protocol may be enabled",
            )),
            BootLeaf::CreateWorldCollision => {
                let protocol = match argument {
                    0 => Protocol::Full,
                    1 => Protocol::Touch,
                    2 => Protocol::Bump,
                    _ => return Err(RuntimeError::new("Invalid world collision gate protocol")),
                };
                let state = world_collision_gate::create(protocol, world_services());
                let movement = state.movement_observer();
                self.world_collision = Some(state);
                *self.context.movement.borrow_mut() = Some(movement);
                if !self.movement_hook {
                    let owner_ptr = self.owner;
                    // SAFETY: the host keeps this EngineOwner alive until after reporter detach.
                    let owner = unsafe { &*owner_ptr };
                    if let Err(error) =
                        owner.publish_actor_movement_observer(actor_movement_observer)
                    {
                        *self.context.movement.borrow_mut() = None;
                        self.world_collision = None;
                        return Err(error);
                    }
                    self.movement_hook = true;
                }
                Ok(Some(0))
            }
            BootLeaf::StaticBspEnabled => Ok(Some(i32::from(static_bsp_probe::is_enabled()))),
            BootLeaf::CreateStaticBsp => {
                self.static_bsp = Some(static_bsp_probe::create(static_bsp_services()));
                Ok(Some(0))
            }
            _ => Ok(None),
        }
    }

    fn install_lifecycle_hooks(&mut self) -> RuntimeResult<()> {
        let owner_ptr = self.owner;
        // SAFETY: the host keeps this EngineOwner alive until after reporter detach.
        let owner = unsafe { &*owner_ptr };
        owner.publish_actor_lifecycle_observer(actor_lifecycle_observer)?;
        self.lifecycle_hooks = true;
        if let Err(error) = owner.publish_actor_spawned_observer(actor_spawned_observer) {
            if owner.clear_actor_lifecycle_observer().is_ok() {
                self.lifecycle_hooks = false;
            }
            return Err(error);
        }
        if let Err(error) = owner.publish_actor_all_actors_observer(actor_all_actors_observer) {
            let spawned_cleared = owner.clear_actor_spawned_observer().is_ok();
            let lifecycle_cleared = owner.clear_actor_lifecycle_observer().is_ok();
            if spawned_cleared && lifecycle_cleared {
                self.lifecycle_hooks = false;
            }
            return Err(error);
        }
        Ok(())
    }

    /// Runs or flushes only a gate whose native reporter kind/operation this bridge owns.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn reporter(
        &mut self,
        kind: Reporter,
        operation: Operation,
        _argument: f32,
        trace_tick: u64,
        test_ticks: i32,
        fixed_delta: f32,
        engine: *mut UEngine,
    ) -> RuntimeResult<Option<i32>> {
        match (kind, operation) {
            (Reporter::Lifecycle, Operation::Present) => {
                Ok(Some(i32::from(self.lifecycle.is_some())))
            }
            (Reporter::Lifecycle, Operation::BeginTick) => {
                let Some(state) = self.lifecycle.as_mut() else {
                    return Ok(None);
                };
                state.begin_tick(trace_tick as i32);
                Ok(Some(0))
            }
            (Reporter::Lifecycle, Operation::Flush) => {
                let Some(state) = self.lifecycle.as_ref() else {
                    return Ok(None);
                };
                crate::reporters::check_callback_error()?;
                state.flush(test_ticks, fixed_delta);
                Ok(Some(0))
            }
            (Reporter::WorldCollision, Operation::Present) => {
                Ok(Some(i32::from(self.world_collision.is_some())))
            }
            (Reporter::WorldCollision, Operation::Run) => {
                let Some(state) = self.world_collision.as_mut() else {
                    return Ok(None);
                };
                state.run(engine.cast());
                crate::reporters::check_callback_error()?;
                Ok(Some(0))
            }
            (Reporter::WorldCollision, Operation::Flush) => {
                let Some(state) = self.world_collision.as_ref() else {
                    return Ok(None);
                };
                crate::reporters::check_callback_error()?;
                state.flush();
                Ok(Some(0))
            }
            (Reporter::StaticBsp, Operation::Present) => {
                Ok(Some(i32::from(self.static_bsp.is_some())))
            }
            (Reporter::StaticBsp, Operation::Run) => {
                let Some(state) = self.static_bsp.as_mut() else {
                    return Ok(None);
                };
                state.run(engine.cast());
                crate::reporters::check_callback_error()?;
                Ok(Some(0))
            }
            (Reporter::StaticBsp, Operation::Flush) => {
                let Some(state) = self.static_bsp.as_ref() else {
                    return Ok(None);
                };
                crate::reporters::check_callback_error()?;
                state.flush();
                Ok(Some(0))
            }
            _ => Ok(None),
        }
    }

    /// Clears every published source observer before EngineOwner teardown.
    pub(super) fn detach(&mut self) -> RuntimeResult<()> {
        let owner_ptr = self.owner;
        // SAFETY: the host keeps this EngineOwner alive until after reporter detach.
        let owner = unsafe { &*owner_ptr };
        let mut first_error = None;
        if self.lifecycle_hooks {
            let mut all_lifecycle_cleared = true;
            if let Err(error) = owner.clear_actor_all_actors_observer() {
                all_lifecycle_cleared = false;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            if let Err(error) = owner.clear_actor_spawned_observer() {
                all_lifecycle_cleared = false;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            if let Err(error) = owner.clear_actor_lifecycle_observer() {
                all_lifecycle_cleared = false;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            if all_lifecycle_cleared {
                self.lifecycle_hooks = false;
            }
        }
        if self.movement_hook {
            if let Err(error) = owner.clear_actor_movement_observer() {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            } else {
                self.movement_hook = false;
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        self.context.lifecycle.set(ptr::null_mut());
        *self.context.movement.borrow_mut() = None;
        OBSERVER_CONTEXT.with(|active| {
            if active.get() == (&mut *self.context) as *mut ObserverContext {
                active.set(ptr::null_mut());
            }
        });
        Ok(())
    }
}

fn observer_context() -> RuntimeResult<&'static ObserverContext> {
    let context = OBSERVER_CONTEXT.with(Cell::get);
    if context.is_null() {
        return Err(RuntimeError::new(
            "Gate reporter observer context is not active",
        ));
    }
    // SAFETY: GateReporters owns this Box and clears the TLS slot during detach.
    Ok(unsafe { &*context })
}

fn with_lifecycle_state<T>(
    body: impl FnOnce(&mut lifecycle_gate::State) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    let context = observer_context()?;
    if context.lifecycle_active.replace(true) {
        return Err(RuntimeError::new("Lifecycle gate observer was reentered"));
    }
    struct Reset<'a>(&'a Cell<bool>);
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }
    let _reset = Reset(&context.lifecycle_active);
    let state = context.lifecycle.get();
    if state.is_null() {
        return Err(RuntimeError::new("Lifecycle gate observer has no reporter"));
    }
    // SAFETY: the reentry guard ensures this state has only one mutable borrow.
    body(unsafe { &mut *state })
}

fn actor_lifecycle_observer(
    _access: CoreAccess<'_>,
    actor: std::ptr::NonNull<AActor>,
    event: ActorLifecycleEvent,
) -> RuntimeResult<()> {
    with_lifecycle_state(|state| {
        match event {
            ActorLifecycleEvent::Spawned => {
                state.on_actor_lifecycle_event(actor.as_ptr().cast(), Some(b"Spawned"))
            }
            ActorLifecycleEvent::PreBeginPlay => {
                state.on_actor_lifecycle_event(actor.as_ptr().cast(), Some(b"PreBeginPlay"))
            }
            ActorLifecycleEvent::BeginPlay => {
                state.on_actor_lifecycle_event(actor.as_ptr().cast(), Some(b"BeginPlay"))
            }
            ActorLifecycleEvent::PostBeginPlay => {
                state.on_actor_lifecycle_event(actor.as_ptr().cast(), Some(b"PostBeginPlay"))
            }
            ActorLifecycleEvent::SetInitialState => {
                state.on_actor_lifecycle_event(actor.as_ptr().cast(), Some(b"SetInitialState"))
            }
            ActorLifecycleEvent::ProcessState => {
                state.on_actor_process_state(actor.as_ptr().cast())
            }
            ActorLifecycleEvent::Destroying => state.on_actor_destroying(actor.as_ptr().cast()),
        }
        Ok(())
    })?;
    crate::reporters::check_callback_error()
}

fn actor_spawned_observer(
    _access: CoreAccess<'_>,
    actor: std::ptr::NonNull<AActor>,
) -> RuntimeResult<()> {
    with_lifecycle_state(|state| {
        state.on_actor_spawned(actor.as_ptr().cast());
        Ok(())
    })?;
    crate::reporters::check_callback_error()
}

fn actor_all_actors_observer(
    access: CoreAccess<'_>,
    event: ActorAllActorsEvent,
) -> RuntimeResult<()> {
    let class_index =
        unsafe { i32::from_ne_bytes((*event.requested_class.as_ptr().cast::<UObject>()).name) };
    let class_name = ansi_name(access, class_index)?;
    let tag = ansi_name(access, event.tag)?;
    with_lifecycle_state(|state| {
        state.on_actor_all_actors(
            event.requestor.as_ptr().cast(),
            Some(&class_name),
            &tag,
            event
                .yielded
                .map_or(ptr::null_mut(), |actor| actor.as_ptr().cast()),
            event.exhausted,
        );
        Ok(())
    })?;
    crate::reporters::check_callback_error()
}

fn actor_movement_observer(
    _access: CoreAccess<'_>,
    event: ActorMovementEvent,
    recipient: std::ptr::NonNull<AActor>,
    other: std::ptr::NonNull<AActor>,
) -> RuntimeResult<()> {
    let observer = observer_context()?
        .movement
        .try_borrow()
        .map_err(|_| RuntimeError::new("World collision observer is already borrowed"))?
        .clone()
        .ok_or_else(|| RuntimeError::new("World collision movement hook has no reporter"))?;
    let event = match event {
        ActorMovementEvent::Bump => MovementEvent::Bump,
        ActorMovementEvent::Touch => MovementEvent::Touch,
        ActorMovementEvent::UnTouch => MovementEvent::UnTouch,
    };
    observer
        .observe(event, recipient.as_ptr().cast(), other.as_ptr().cast())
        .map_err(|_| RuntimeError::new("World collision movement event was reentered"))
}

fn ansi_name(access: CoreAccess<'_>, index: i32) -> RuntimeResult<Vec<u8>> {
    let mut name = Vec::new();
    access.write_name_text(index, &mut name)?;
    name.push(0);
    // SAFETY: `name` is terminated TCHAR text and the returned ANSI ring slot
    // is copied before another conversion can reuse it.
    let ansi = unsafe { hp2_core::ansi::to_ansi_string(name.as_ptr().cast()) };
    if ansi.is_null() {
        return Err(RuntimeError::new("Name-to-ANSI conversion returned null"));
    }
    Ok(unsafe { CStr::from_ptr(ansi.cast()) }.to_bytes().to_vec())
}

unsafe fn output_text(
    access: CoreAccess<'_>,
    name_index: i32,
    output: *mut u8,
    capacity: usize,
) -> RuntimeResult<usize> {
    let name = ansi_name(access, name_index)?;
    let copied = name.len().min(capacity);
    if copied != 0 {
        if output.is_null() {
            return Err(RuntimeError::new("Null gate reporter text output"));
        }
        unsafe {
            ptr::copy_nonoverlapping(name.as_ptr(), output, copied);
        }
    }
    Ok(name.len())
}

unsafe extern "C" fn lifecycle_stable_actor_key(
    actor: *mut c_void,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    crate::reporters::with_owner(0, |owner| unsafe {
        if actor.is_null() {
            return Ok(0);
        }
        if output.is_null() && capacity != 0 {
            return Err(RuntimeError::new("Null lifecycle stable-key output"));
        }
        let object = actor.cast::<UObject>();
        let class = ptr::addr_of!((*object).class)
            .read_unaligned()
            .cast::<UClass>();
        if class.is_null() {
            return Ok(0);
        }
        let access = owner.access();
        let property_class = access.native_class("NameProperty")?;
        let property_name = access.intern_str("StableKey", FindName::Find)?;
        let property = unsafe {
            access.find_field(
                class.cast::<UStruct>(),
                property_name,
                property_class.cast(),
            )
        };
        if property.is_null() {
            return Ok(0);
        }
        let offset = ptr::addr_of!((*property.cast::<UProperty>()).offset).read_unaligned();
        if offset < 0 {
            return Err(RuntimeError::new(
                "Lifecycle StableKey property has a negative offset",
            ));
        }
        let key = ptr::addr_of!((*actor.cast::<u8>().add(offset as usize).cast::<FName>()).index)
            .read_unaligned();
        let class_name =
            i32::from_ne_bytes(ptr::addr_of!((*class.cast::<UObject>()).name).read_unaligned());
        let mut length = unsafe { output_text(access, class_name, output, capacity)? };
        if length < capacity {
            output.add(length).write(b':');
        }
        length += 1;
        length += unsafe {
            output_text(
                access,
                key,
                output.wrapping_add(length.min(capacity)),
                capacity.saturating_sub(length),
            )?
        };
        i32::try_from(length)
            .map_err(|_| RuntimeError::new("Lifecycle stable key exceeds signed length"))
    })
}

unsafe extern "C" fn lifecycle_state_name(
    actor: *mut c_void,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    crate::reporters::with_owner(0, |owner| unsafe {
        if actor.is_null() {
            return Ok(0);
        }
        if output.is_null() && capacity != 0 {
            return Err(RuntimeError::new("Null lifecycle state-name output"));
        }
        let object = actor.cast::<UObject>();
        let frame = ptr::addr_of!((*object).state_frame)
            .read_unaligned()
            .cast::<FStateFrame>();
        if frame.is_null() {
            return Ok(0);
        }
        let node = ptr::addr_of!((*frame).state_node)
            .read_unaligned()
            .cast::<UState>();
        let class = ptr::addr_of!((*object).class).read_unaligned();
        if node.is_null() || node.cast::<c_void>() == class {
            return Ok(0);
        }
        let name_index =
            i32::from_ne_bytes(ptr::addr_of!((*node.cast::<UObject>()).name).read_unaligned());
        let length = unsafe { output_text(owner.access(), name_index, output, capacity)? };
        i32::try_from(length)
            .map_err(|_| RuntimeError::new("Lifecycle state name exceeds signed length"))
    })
}

unsafe extern "C" fn lifecycle_state_pc_diagnostic(actor: *mut c_void, output: *mut i32) -> i32 {
    crate::reporters::with_owner(0, |owner| unsafe {
        if actor.is_null() {
            return Ok(0);
        }
        if output.is_null() {
            return Err(RuntimeError::new("Null lifecycle PC-diagnostic output"));
        }
        let object = actor.cast::<UObject>();
        let frame = ptr::addr_of!((*object).state_frame)
            .read_unaligned()
            .cast::<FStateFrame>();
        if frame.is_null() {
            return Ok(0);
        }
        let node = ptr::addr_of!((*frame).node)
            .read_unaligned()
            .cast::<UStruct>();
        let code = ptr::addr_of!((*frame).code).read_unaligned();
        if node.is_null() || code.is_null() {
            return Ok(0);
        }
        let script = ptr::addr_of!((*node).script).read_unaligned();
        if script.num <= 0 || script.data.is_null() {
            return Ok(0);
        }
        let begin = script.data.cast::<u8>().addr();
        let end = begin
            .checked_add(script.num as usize)
            .ok_or_else(|| RuntimeError::new("Lifecycle script byte-range overflow"))?;
        let current = code.addr();
        if current < begin || current > end {
            return Ok(0);
        }
        let offset = i32::try_from(current - begin)
            .map_err(|_| RuntimeError::new("Lifecycle script offset exceeds signed range"))?;
        output.write(offset);
        let _ = owner;
        Ok(1)
    })
}

unsafe extern "C" fn lifecycle_actor_state_latent_is_sleep(actor: *mut c_void) -> i32 {
    crate::reporters::with_owner(0, |_owner| unsafe {
        if actor.is_null() {
            return Ok(0);
        }
        let frame = ptr::addr_of!((*actor.cast::<UObject>()).state_frame)
            .read_unaligned()
            .cast::<FStateFrame>();
        Ok(i32::from(
            !frame.is_null() && (*frame).latent_action == hp2_abi::engine::generated::EPOLL_Sleep,
        ))
    })
}

fn zero_box() -> FBox {
    // FBox is POD; zero vectors and IsValid=0 match the native invalid box.
    unsafe { std::mem::zeroed() }
}

fn world_services() -> Hp2WorldCollisionGateServices {
    Hp2WorldCollisionGateServices {
        static_load_actor_class: world_static_load_actor_class,
        actor_tag: world_actor_tag,
        actor_cylinder_extent: world_actor_cylinder_extent,
        actor_set_collision: world_actor_set_collision,
        actor_set_collision_size: world_actor_set_collision_size,
        actor_set_base: world_actor_set_base,
        actor_end_touch: world_actor_end_touch,
        actor_collision_bounding_box: world_actor_collision_bounding_box,
        find_spot: world_find_spot,
        far_move_actor: world_far_move_actor,
        move_actor: world_move_actor,
        spawn_actor: world_spawn_actor,
    }
}

fn static_bsp_services() -> Hp2StaticBspProbeServices {
    Hp2StaticBspProbeServices {
        level_model: static_level_model,
        level_destroy_actor: static_level_destroy_actor,
        static_load_actor_class: world_static_load_actor_class,
        actor_is_a_level_info: static_actor_is_level_info,
        model_point_check: static_model_point_check,
        actor_tag: world_actor_tag,
        actor_set_collision: world_actor_set_collision,
        actor_set_collision_size: world_actor_set_collision_size,
        find_spot: world_find_spot,
        far_move_actor: world_far_move_actor,
        move_actor: world_move_actor,
        spawn_actor: world_spawn_actor,
    }
}

unsafe extern "C" fn world_static_load_actor_class(
    name: *const u8,
    name_len: usize,
    flags: u32,
) -> *mut c_void {
    crate::reporters::with_owner(ptr::null_mut(), |owner| {
        if name.is_null() && name_len != 0 {
            return Err(RuntimeError::new("Null static actor-class name"));
        }
        let bytes = if name_len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(name, name_len) }
        };
        let name = std::str::from_utf8(bytes)
            .map_err(|_| RuntimeError::new("Invalid UTF-8 actor-class name"))?;
        let units = name.chars().map(u32::from).chain([0]).collect::<Vec<_>>();
        let access = owner.access();
        let base = access.native_class("Engine.Actor")?;
        unsafe {
            access
                .static_load_class(base, ptr::null_mut(), &units, flags)
                .map(|class| class.cast())
        }
    })
}

unsafe extern "C" fn world_actor_tag(actor: *mut c_void, output: *mut u8, capacity: usize) -> i32 {
    crate::reporters::with_owner(0, |owner| unsafe {
        if actor.is_null() {
            return Ok(0);
        }
        let index =
            i32::from_ne_bytes(ptr::addr_of!((*actor.cast::<AActor>()).tag).read_unaligned());
        let length = output_text(owner.access(), index, output, capacity)?;
        i32::try_from(length).map_err(|_| RuntimeError::new("Actor tag exceeds signed length"))
    })
}

unsafe extern "C" fn world_actor_cylinder_extent(actor: *mut c_void) -> FVector {
    crate::reporters::with_owner(ZERO_VECTOR, |owner| {
        if actor.is_null() {
            return Err(RuntimeError::new("Null Actor in collision-cylinder extent"));
        }
        unsafe { hp2_engine::collision_owner::actor_cylinder_extent(owner.access(), actor.cast()) }
    })
}

unsafe extern "C" fn world_actor_set_collision(
    actor: *mut c_void,
    collide_actors: i32,
    block_actors: i32,
    block_players: i32,
) {
    crate::reporters::with_owner((), |owner| {
        if actor.is_null() {
            return Err(RuntimeError::new("Null Actor in SetCollision"));
        }
        unsafe {
            hp2_engine::collision_owner::actor_set_collision(
                owner.access(),
                actor.cast(),
                collide_actors != 0,
                block_actors != 0,
                block_players != 0,
            )
        }
    });
}

unsafe extern "C" fn world_actor_set_collision_size(actor: *mut c_void, radius: f32, height: f32) {
    crate::reporters::with_owner((), |owner| {
        if actor.is_null() {
            return Err(RuntimeError::new("Null Actor in SetCollisionSize"));
        }
        unsafe {
            hp2_engine::collision_owner::actor_set_collision_size(
                owner.access(),
                actor.cast(),
                radius,
                height,
            )
        }
    });
}

unsafe extern "C" fn world_actor_set_base(actor: *mut c_void, base: *mut c_void) {
    crate::reporters::with_owner((), |owner| {
        if actor.is_null() {
            return Err(RuntimeError::new("Null Actor in SetBase"));
        }
        unsafe {
            hp2_engine::level_lifecycle_owner::set_base(owner.access(), actor.cast(), base.cast(), true)
        }
    });
}

unsafe extern "C" fn world_actor_end_touch(
    actor: *mut c_void,
    other: *mut c_void,
    no_notify_self: i32,
) {
    crate::reporters::with_owner((), |owner| {
        if actor.is_null() || other.is_null() {
            return Err(RuntimeError::new("Null Actor in EndTouch"));
        }
        unsafe {
            hp2_engine::level_lifecycle_owner::end_touch(
                owner.access(),
                actor.cast(),
                other.cast(),
                no_notify_self,
            )
        }
    });
}

unsafe extern "C" fn world_actor_collision_bounding_box(actor: *mut c_void, world: i32) -> FBox {
    crate::reporters::with_owner(zero_box(), |owner| {
        if actor.is_null() {
            return Err(RuntimeError::new("Null Actor in collision bounds"));
        }
        unsafe {
            if world != 0 {
                hp2_engine::collision_owner::actor_collision_box_visual(
                    owner.access(),
                    actor.cast(),
                    false,
                )
            } else {
                hp2_engine::collision_owner::actor_collision_box_local(owner.access(), actor.cast())
            }
        }
    })
}

unsafe extern "C" fn world_find_spot(
    level: *mut c_void,
    extent: FVector,
    location: *mut FVector,
    check_actors: i32,
    assume_fit: i32,
) -> i32 {
    crate::reporters::with_owner(0, |owner| {
        if level.is_null() || location.is_null() {
            return Err(RuntimeError::new("Invalid FindSpot arguments"));
        }
        unsafe {
            hp2_engine::level_zone_owner::find_spot(
                owner.access(),
                level.cast(),
                extent,
                location,
                check_actors != 0,
                assume_fit != 0,
            )
            .map(i32::from)
        }
    })
}

unsafe extern "C" fn world_far_move_actor(
    level: *mut c_void,
    actor: *mut c_void,
    destination: FVector,
    test: i32,
    no_check: i32,
) -> i32 {
    crate::reporters::with_owner(0, |owner| {
        if level.is_null() || actor.is_null() {
            return Err(RuntimeError::new("Invalid FarMoveActor arguments"));
        }
        unsafe {
            hp2_engine::level_move_owner::far_move_actor(
                owner.access(),
                level.cast(),
                actor.cast(),
                destination,
                test != 0,
                no_check != 0,
            )
            .map(i32::from)
        }
    })
}

unsafe extern "C" fn world_move_actor(
    level: *mut c_void,
    actor: *mut c_void,
    delta: FVector,
    rotation: FRotator,
    hit: *mut FCheckResult,
    test: i32,
    ignore_pawns: i32,
    ignore_bases: i32,
    no_fail: i32,
) -> i32 {
    crate::reporters::with_owner(0, |owner| {
        if level.is_null() || actor.is_null() || hit.is_null() {
            return Err(RuntimeError::new("Invalid MoveActor arguments"));
        }
        unsafe {
            hp2_engine::level_move_owner::move_actor(
                owner.access(),
                level.cast(),
                actor.cast(),
                delta,
                rotation,
                hit,
                test != 0,
                ignore_pawns != 0,
                ignore_bases != 0,
                no_fail != 0,
            )
            .map(i32::from)
        }
    })
}

unsafe extern "C" fn world_spawn_actor(
    level: *mut c_void,
    class: *mut c_void,
    location: FVector,
    rotation: FRotator,
    no_collision_fail: i32,
) -> *mut c_void {
    crate::reporters::with_owner(ptr::null_mut(), |owner| {
        if level.is_null() || class.is_null() {
            return Err(RuntimeError::new("Invalid SpawnActor arguments"));
        }
        unsafe {
            hp2_engine::level_spawn_actor_owner::spawn_actor(
                owner.access(),
                level.cast::<ULevel>(),
                class.cast::<UClass>(),
                FName { index: 0 },
                ptr::null_mut(),
                ptr::null_mut(),
                location,
                rotation,
                ptr::null_mut(),
                no_collision_fail != 0,
                false,
            )
            .map(|actor| actor.cast())
        }
    })
}

unsafe extern "C" fn static_level_model(level: *mut c_void) -> *mut c_void {
    crate::reporters::with_owner(ptr::null_mut(), |_owner| {
        if level.is_null() {
            return Err(RuntimeError::new("Null Level in Static BSP model lookup"));
        }
        Ok(unsafe {
            ptr::addr_of!((*level.cast::<ULevel>()).model)
                .read_unaligned()
                .cast()
        })
    })
}

unsafe extern "C" fn static_level_destroy_actor(level: *mut c_void, actor: *mut c_void) {
    crate::reporters::with_owner((), |owner| {
        if level.is_null() || actor.is_null() {
            return Err(RuntimeError::new("Invalid DestroyActor arguments"));
        }
        unsafe {
            hp2_engine::level_lifecycle_owner::destroy_actor(
                owner.access(),
                level.cast(),
                actor.cast(),
                false,
            )
            .map(|_| ())
        }
    });
}

unsafe extern "C" fn static_actor_is_level_info(actor: *mut c_void) -> i32 {
    crate::reporters::with_owner(0, |owner| {
        if actor.is_null() {
            return Ok(0);
        }
        let class = owner.access().native_class("Engine.LevelInfo")?;
        Ok(i32::from(unsafe {
            hp2_core::object_model::is_a(actor, class.cast())
        }))
    })
}

unsafe extern "C" fn static_model_point_check(
    model: *mut c_void,
    hit: *mut FCheckResult,
    _source_actor: *mut c_void,
    location: FVector,
    extent: FVector,
    extra_node_flags: u32,
) -> i32 {
    crate::reporters::with_owner(0, |owner| {
        if model.is_null() || hit.is_null() {
            return Err(RuntimeError::new(
                "Invalid Model or Hit in static BSP point check",
            ));
        }
        unsafe {
            hp2_engine::collision_owner::world_point_check(
                owner.access(),
                model.cast(),
                hit,
                location,
                extent,
                extra_node_flags,
            )
            .map(i32::from)
        }
    })
}
