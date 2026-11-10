use hp2_abi::FColor;
use hp2_abi::engine::generated::{
    AActor, FStateFrame, TArrayRaw, UALAudioSubsystem, UBoolProperty, UClass, UEngine, UGameEngine,
    ULevel, UObject, UProperty, UStruct, UViewport,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult, VmObservation};
use hp2_engine::{EngineOwner, SpawnInstrumentation, SpawnInstrumentationEvent};
use hp2_launcher::{
    boot::Hp2LauncherBootLeaf as BootLeaf,
    main_loop::{
        Hp2LauncherMainLoopLeaf as LoopLeaf, Hp2LauncherMainLoopReporter as Reporter,
        Hp2LauncherMainLoopReporterOperation as Operation,
    },
    reporters::{
        actor_transition_ledger, creature_generator, fidelity, global_tick, shadow_admission,
        trace_services,
    },
};
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, c_char, c_void},
    ptr,
    rc::Rc,
};

struct ReportContext {
    owner: *const EngineOwner,
    engine: Cell<*mut UEngine>,
    text: RefCell<Vec<u32>>,
    fidelity: RefCell<Option<Box<fidelity::State>>>,
}

thread_local! {
    static ACTIVE: Cell<*const ReportContext> = const { Cell::new(ptr::null()) };
    static CALLBACK_ERROR: RefCell<Option<RuntimeError>> = const { RefCell::new(None) };
}

fn retain_callback_error(error: RuntimeError) {
    CALLBACK_ERROR.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    });
}

/// Plain-ABI transport only: a retained failure is checked before report output.
/// No context or owner borrow is held across the actual operation or its reentry.
pub(super) fn with_owner<T>(fallback: T, body: impl FnOnce(&EngineOwner) -> RuntimeResult<T>) -> T {
    let context = ACTIVE.with(Cell::get);
    if context.is_null() {
        retain_callback_error(RuntimeError::new(
            "Canonical reporter callback has no attached owner",
        ));
        return fallback;
    }
    let owner = unsafe { (*context).owner };
    match body(unsafe { &*owner }) {
        Ok(value) => value,
        Err(error) => {
            retain_callback_error(error);
            fallback
        }
    }
}

pub(super) fn check_callback_error() -> RuntimeResult<()> {
    match CALLBACK_ERROR.with(|slot| slot.borrow_mut().take()) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(super) struct RuntimeReporters {
    context: Box<ReportContext>,
    gates: crate::gate_reporters::GateReporters,
    attached: bool,
}

impl RuntimeReporters {
    /// # Safety
    /// `owner` must remain live at the same address until these reporters detach
    /// and are dropped. The boot host drops reporters before its owner.
    pub(super) unsafe fn new(owner: *const EngineOwner) -> RuntimeResult<Self> {
        if owner.is_null() {
            return Err(RuntimeError::new(
                "Runtime reporters require a live EngineOwner",
            ));
        }
        if ACTIVE.with(|slot| !slot.get().is_null()) {
            return Err(RuntimeError::new(
                "A canonical reporter owner is already attached",
            ));
        }
        check_callback_error()?;
        let context = Box::new(ReportContext {
            owner,
            engine: Cell::new(ptr::null_mut()),
            text: RefCell::new(Vec::new()),
            fidelity: RefCell::new(None),
        });
        ACTIVE.with(|slot| slot.set(ptr::from_ref(&*context)));
        unsafe {
            trace_services::publish_services(TRACE_SERVICES);
        }
        let gates = match crate::gate_reporters::GateReporters::new(owner) {
            Ok(gates) => gates,
            Err(error) => {
                ACTIVE.with(|slot| slot.set(ptr::null()));
                return Err(error);
            }
        };
        let reporters = Self {
            context,
            gates,
            attached: true,
        };
        let owner = unsafe { &*owner };
        // Once the lexical owner exists, failed setup releases all published hooks.
        crate::fire_probe::initialize_services(owner)?;
        crate::runtime_trace::install(owner)?;
        owner.publish_spawn_instrumentation(Rc::new(SpawnHooks))?;
        owner.access().set_vm_observer(Some(observe_vm));
        Ok(reporters)
    }

    pub(super) fn bind_engine(&mut self, engine: *mut UEngine) {
        self.context.engine.set(engine);
    }

    pub(super) fn boot_leaf(
        &mut self,
        leaf: BootLeaf,
        argument: i32,
    ) -> RuntimeResult<Option<i32>> {
        check_callback_error()?;
        if let Some(value) = self.gates.boot_leaf(leaf, argument)? {
            return Ok(Some(value));
        }
        crate::fire_probe::boot_leaf(leaf, argument)
    }

    pub(super) fn loop_leaf(&mut self, leaf: LoopLeaf) -> RuntimeResult<Option<i32>> {
        check_callback_error()?;
        match leaf {
            LoopLeaf::FidelityEnabled => Ok(Some(i32::from(fidelity::is_enabled()))),
            LoopLeaf::CreateFidelityReporter => {
                let engine = self.context.engine.get();
                if engine.is_null() {
                    return Err(RuntimeError::new(
                        "Fidelity reporter requires the canonical engine",
                    ));
                }
                *self.context.fidelity.borrow_mut() =
                    Some(fidelity::create(FIDELITY_SERVICES, engine.cast()));
                Ok(Some(0))
            }
            _ => crate::fire_probe::loop_leaf(leaf, self.context.engine.get()),
        }
    }

    pub(super) fn reporter(
        &mut self,
        kind: Reporter,
        operation: Operation,
        argument: f32,
        trace_tick: u64,
        test_ticks: i32,
        fixed_delta: f32,
    ) -> RuntimeResult<i32> {
        check_callback_error()?;
        if let Some(value) = self.gates.reporter(
            kind,
            operation,
            argument,
            trace_tick,
            test_ticks,
            fixed_delta,
            self.context.engine.get(),
        )? {
            return Ok(value);
        }
        let value = match (kind, operation) {
            (Reporter::Fidelity, Operation::Present) => {
                i32::from(self.context.fidelity.borrow().is_some())
            }
            (Reporter::Fidelity, Operation::AfterTick) => {
                if let Some(state) = self.context.fidelity.borrow_mut().as_mut() {
                    state.after_tick();
                }
                0
            }
            (Reporter::Fidelity, Operation::Flush) => {
                if let Some(state) = self.context.fidelity.borrow().as_ref() {
                    state.flush(test_ticks, fixed_delta);
                }
                0
            }
            (Reporter::ShadowAdmission, Operation::Present) => unsafe {
                shadow_admission::present()
            },
            (Reporter::ShadowAdmission, Operation::SetFrame) => {
                unsafe {
                    shadow_admission::set_frame(trace_tick);
                }
                0
            }
            (Reporter::ShadowAdmission, Operation::Flush) => {
                unsafe {
                    shadow_admission::flush();
                }
                0
            }
            (Reporter::ActorTransition, Operation::Present) => unsafe {
                actor_transition_ledger::present()
            },
            (Reporter::ActorTransition, Operation::Flush) => {
                unsafe {
                    actor_transition_ledger::flush();
                }
                0
            }
            (Reporter::CreatureGenerator, Operation::Present) => unsafe {
                creature_generator::present()
            },
            (Reporter::CreatureGenerator, Operation::SetFrame) => {
                unsafe {
                    creature_generator::set_frame(trace_tick);
                }
                0
            }
            (Reporter::CreatureGenerator, Operation::Flush) => {
                unsafe {
                    creature_generator::flush();
                }
                0
            }
            (Reporter::GlobalTick, Operation::Present) => unsafe { global_tick::present() },
            (Reporter::GlobalTick, Operation::SetFrame) => {
                unsafe {
                    global_tick::set_frame(trace_tick);
                }
                0
            }
            (Reporter::GlobalTick, Operation::Flush) => {
                unsafe {
                    global_tick::flush();
                }
                0
            }
            _ => return Err(RuntimeError::new("Invalid canonical reporter operation")),
        };
        check_callback_error()?;
        Ok(value)
    }

    pub(super) fn end_loop_scope(&mut self) {
        // This is the native lexical destructor, not an End/Flush phase on failure.
        self.context.fidelity.borrow_mut().take();
    }

    pub(super) fn detach(&mut self) -> RuntimeResult<()> {
        if !self.attached {
            return Ok(());
        }
        self.end_loop_scope();
        let mut result = self.gates.detach();
        let owner = unsafe { &*self.context.owner };
        owner.access().clear_vm_observer(observe_vm);
        result = result.and(owner.clear_spawn_instrumentation());
        result = result.and(owner.clear_fire_trace_observer());
        result = result.and(crate::fire_probe::detach(owner));
        result = result.and(crate::runtime_trace::detach(owner));
        ACTIVE.with(|slot| {
            if slot.get() == ptr::from_ref(&*self.context) {
                slot.set(ptr::null());
            }
        });
        self.attached = false;
        result.and(check_callback_error())
    }
}

impl Drop for RuntimeReporters {
    fn drop(&mut self) {
        if let Err(error) = self.detach() {
            retain_callback_error(error);
        }
    }
}

pub(super) fn fire_observer(
    access: CoreAccess<'_>,
    event: hp2_engine::FireTraceEvent,
) -> RuntimeResult<()> {
    crate::fire_probe::fire_observer(access, event)
}

struct SpawnHooks;
impl SpawnInstrumentation for SpawnHooks {
    fn on_spawn_instrumentation(
        &self,
        _access: CoreAccess<'_>,
        event: SpawnInstrumentationEvent,
    ) -> RuntimeResult<()> {
        use SpawnInstrumentationEvent as E;
        unsafe {
            match event {
                E::TransitionRequest { class, name, owner } => {
                    actor_transition_ledger::spawn_request(class.cast(), name.index, owner.cast())
                }
                E::CreatureRequest { class, owner } => {
                    creature_generator::spawn_request(class.cast(), owner.cast())
                }
                E::TransitionResult { owner, actor } => {
                    actor_transition_ledger::spawn_result(owner.cast(), actor.cast())
                }
                E::CreatureResult { owner, actor } => {
                    creature_generator::spawn_result(owner.cast(), actor.cast())
                }
                E::TransitionPublished { owner, actor } => {
                    actor_transition_ledger::spawn_published(owner.cast(), actor.cast())
                }
                E::CreaturePublished { owner, actor } => {
                    creature_generator::spawn_published(owner.cast(), actor.cast())
                }
                E::PreBeginBefore { actor } => {
                    actor_transition_ledger::pre_begin_before(actor.cast())
                }
                E::PreBeginAfter { actor } => {
                    actor_transition_ledger::pre_begin_after(actor.cast())
                }
                E::SoftwareRendering { actor, software } => {
                    creature_generator::software_rendering(actor.cast(), i32::from(software))
                }
            }
        }
        check_callback_error()
    }
}

fn observe_vm(_access: CoreAccess<'_>, event: VmObservation) -> RuntimeResult<()> {
    let context = ACTIVE.with(Cell::get);
    if context.is_null() {
        return Err(RuntimeError::new(
            "VM reporter observer outlived its canonical owner",
        ));
    }
    let context = unsafe { &*context };
    unsafe {
        match event {
            VmObservation::ScriptCallEnter {
                object, function, ..
            } => {
                if let Some(state) = context.fidelity.borrow_mut().as_mut() {
                    let name = object_name_ansi(function.cast());
                    let bytes = if name.is_null() {
                        &[]
                    } else {
                        CStr::from_ptr(name).to_bytes()
                    };
                    state.on_call_enter(object.cast(), bytes);
                }
            }
            VmObservation::ScriptCallExit {
                object,
                function,
                result,
                ..
            } => {
                if let Some(state) = context.fidelity.borrow_mut().as_mut() {
                    let name = object_name_ansi(function.cast());
                    let bytes = if name.is_null() {
                        &[]
                    } else {
                        CStr::from_ptr(name).to_bytes()
                    };
                    // Only this boolean-returning method consumes the result in the reporter.
                    let returned =
                        if bytes.eq_ignore_ascii_case(b"GetNextCommand") && !result.is_null() {
                            Some(result.cast::<i32>().read_unaligned() != 0)
                        } else {
                            None
                        };
                    state.on_call_exit(object.cast(), bytes, returned);
                }
            }
            VmObservation::FunctionEnter {
                object, function, ..
            } => creature_generator::function_enter(object.cast(), function.cast()),
            VmObservation::FunctionExit {
                object,
                function,
                result,
                ..
            } => creature_generator::function_exit(object.cast(), function.cast(), result.cast()),
            VmObservation::Rand {
                object,
                raw,
                bound,
                index,
            } => creature_generator::rand(object.cast(), raw, bound, index),
            VmObservation::RandRange {
                object,
                raw,
                min,
                max,
                value,
            } => creature_generator::rand_range(object.cast(), raw, min, max, value),
        }
    }
    check_callback_error()
}

/// Text leaves perform only direct metadata/name reads; the scratch borrow never
/// spans VM execution or another reporter callback. appToAnsi owns its source ring.
fn text_ansi(fill: impl FnOnce(&EngineOwner, &mut Vec<u32>) -> RuntimeResult<()>) -> *const c_char {
    with_owner(ptr::null(), |owner| {
        let context = ACTIVE.with(Cell::get);
        let mut text = unsafe { &*context }.text.borrow_mut();
        fill(owner, &mut text)?;
        text.push(0);
        Ok(unsafe { hp2_core::ansi::to_ansi_string(text.as_ptr().cast()).cast() })
    })
}

unsafe extern "C" fn object_path_ansi(object: *mut c_void) -> *const c_char {
    text_ansi(|owner, text| unsafe { owner.access().write_object_path_text(object.cast(), text) })
}
unsafe extern "C" fn object_name_ansi(object: *mut c_void) -> *const c_char {
    text_ansi(|owner, text| {
        let name = i32::from_ne_bytes(unsafe {
            ptr::addr_of!((*object.cast::<UObject>()).name).read_unaligned()
        });
        owner.access().write_name_text(name, text)
    })
}
unsafe extern "C" fn object_class(object: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*object.cast::<UObject>()).class).read_unaligned() }
}
unsafe extern "C" fn fname_ansi_or_empty(index: i32) -> *const c_char {
    if index == 0 {
        return c"".as_ptr();
    }
    text_ansi(|owner, text| owner.access().write_name_text(index, text))
}
unsafe extern "C" fn actor_tag_fname(actor: *mut c_void) -> i32 {
    i32::from_ne_bytes(unsafe { ptr::addr_of!((*actor.cast::<AActor>()).tag).read_unaligned() })
}
unsafe extern "C" fn actor_shadow(actor: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*actor.cast::<AActor>()).shadow).read_unaligned() }
}
unsafe extern "C" fn actor_x_level(actor: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*actor.cast::<AActor>()).x_level).read_unaligned() }
}
unsafe extern "C" fn actor_owner(actor: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*actor.cast::<AActor>()).owner).read_unaligned() }
}
unsafe extern "C" fn actor_delete_marked(actor: *mut c_void) -> i32 {
    i32::from(unsafe { (*actor.cast::<AActor>()).b_delete_me() })
}
unsafe extern "C" fn actor_pending_kill(actor: *mut c_void) -> i32 {
    i32::from(unsafe { hp2_engine::actor::is_pending_kill(actor.cast()) })
}
unsafe extern "C" fn actor_ticked(actor: *mut c_void) -> i32 {
    i32::from(unsafe { (*actor.cast::<AActor>()).b_ticked() })
}
unsafe extern "C" fn actor_game_relevant(actor: *mut c_void) -> i32 {
    i32::from(unsafe { (*actor.cast::<AActor>()).b_game_relevant() })
}
unsafe extern "C" fn actor_role(actor: *mut c_void) -> i32 {
    i32::from(unsafe { ptr::addr_of!((*actor.cast::<AActor>()).role).read_unaligned() })
}
unsafe extern "C" fn actor_remote_role(actor: *mut c_void) -> i32 {
    i32::from(unsafe { ptr::addr_of!((*actor.cast::<AActor>()).remote_role).read_unaligned() })
}
unsafe extern "C" fn level_actor_count(level: *mut c_void) -> i32 {
    unsafe { ptr::addr_of!((*level.cast::<ULevel>()).base.actors.array_num).read_unaligned() }
}
unsafe extern "C" fn level_actor_at(level: *mut c_void, slot: i32) -> *mut c_void {
    let data =
        unsafe { ptr::addr_of!((*level.cast::<ULevel>()).base.actors.data).read_unaligned() };
    unsafe {
        data.cast::<*mut c_void>()
            .offset(slot as isize)
            .read_unaligned()
    }
}
unsafe extern "C" fn level_first_dynamic_actor(level: *mut c_void) -> i32 {
    unsafe { ptr::addr_of!((*level.cast::<ULevel>()).i_first_dynamic_actor).read_unaligned() }
}
unsafe extern "C" fn level_first_net_relevant_actor(level: *mut c_void) -> i32 {
    unsafe { ptr::addr_of!((*level.cast::<ULevel>()).i_first_net_relevant_actor).read_unaligned() }
}
unsafe fn game_engine(owner: &EngineOwner, engine: *mut c_void) -> RuntimeResult<*mut UGameEngine> {
    let class = owner.access().native_class("Engine.GameEngine")?;
    Ok(
        if unsafe { hp2_core::object_model::is_a(engine, class.cast()) } {
            engine.cast()
        } else {
            ptr::null_mut()
        },
    )
}
unsafe extern "C" fn game_engine_level(engine: *mut c_void) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| {
        let game = unsafe { game_engine(owner, engine)? };
        Ok(if game.is_null() {
            ptr::null_mut()
        } else {
            unsafe { ptr::addr_of!((*game).g_level).read_unaligned() }
        })
    })
}
unsafe extern "C" fn class_super(class: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*class.cast::<UClass>()).super_field).read_unaligned() }
}
unsafe extern "C" fn class_default_actor(class: *mut c_void) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| unsafe {
        owner
            .access()
            .default_object(class.cast())
            .map(|value| value.cast())
    })
}
unsafe fn cast_object(
    owner: &EngineOwner,
    object: *mut c_void,
    class: &str,
) -> RuntimeResult<*mut c_void> {
    let class = owner.access().native_class(class)?;
    Ok(
        if unsafe { hp2_core::object_model::is_a(object, class.cast()) } {
            object
        } else {
            ptr::null_mut()
        },
    )
}
unsafe extern "C" fn cast_to_class(object: *mut c_void) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| unsafe {
        cast_object(owner, object, "Core.Class")
    })
}
unsafe extern "C" fn cast_to_actor(object: *mut c_void) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| unsafe {
        cast_object(owner, object, "Engine.Actor")
    })
}

unsafe fn property_named(
    access: CoreAccess<'_>,
    class: *mut UClass,
    name: *const c_char,
    expected: Option<&str>,
) -> RuntimeResult<*mut UProperty> {
    let wide = unsafe { hp2_core::ansi::from_ansi(name.cast()) };
    let mut length = 0;
    while unsafe { wide.add(length).read() } != 0 {
        length += 1;
    }
    let name =
        access.find_name(unsafe { std::slice::from_raw_parts(wide.cast::<u32>(), length) })?;
    let property_class = access.native_class("Core.Property")?;
    let property = unsafe { access.find_field(class.cast::<UStruct>(), name, property_class) }
        .cast::<UProperty>();
    if let Some(expected) = expected {
        let expected = access.native_class(expected)?;
        if !property.is_null()
            && !unsafe { hp2_core::object_model::is_a(property.cast(), expected.cast()) }
        {
            return Ok(ptr::null_mut());
        }
    }
    Ok(property)
}
unsafe extern "C" fn find_property(class: *mut c_void, name: *const c_char) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| unsafe {
        property_named(owner.access(), class.cast(), name, None).map(|value| value.cast())
    })
}
unsafe extern "C" fn find_bool_property(class: *mut c_void, name: *const c_char) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| unsafe {
        property_named(
            owner.access(),
            class.cast(),
            name,
            Some("Core.BoolProperty"),
        )
        .map(|value| value.cast())
    })
}
unsafe extern "C" fn find_int_property(class: *mut c_void, name: *const c_char) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| unsafe {
        property_named(owner.access(), class.cast(), name, Some("Core.IntProperty"))
            .map(|value| value.cast())
    })
}
unsafe extern "C" fn find_float_property(class: *mut c_void, name: *const c_char) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| unsafe {
        property_named(
            owner.access(),
            class.cast(),
            name,
            Some("Core.FloatProperty"),
        )
        .map(|value| value.cast())
    })
}
unsafe extern "C" fn property_offset(property: *mut c_void) -> i32 {
    unsafe { ptr::addr_of!((*property.cast::<UProperty>()).offset).read_unaligned() }
}
unsafe extern "C" fn property_array_dim(property: *mut c_void) -> i32 {
    unsafe { ptr::addr_of!((*property.cast::<UProperty>()).array_dim).read_unaligned() }
}
unsafe extern "C" fn property_element_size(property: *mut c_void) -> i32 {
    unsafe { ptr::addr_of!((*property.cast::<UProperty>()).element_size).read_unaligned() }
}
unsafe extern "C" fn bool_property_bitmask(property: *mut c_void) -> u32 {
    unsafe { ptr::addr_of!((*property.cast::<UBoolProperty>()).bit_mask).read_unaligned() }
}
unsafe extern "C" fn property_outer(property: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*property.cast::<UObject>()).outer).read_unaligned() }
}
unsafe extern "C" fn rand_trace_ordinal() -> u64 {
    hp2_core::ansi_trace::with_process_state(|state| hp2_core::ansi_trace::get_ordinal(state))
}

const TRACE_SERVICES: trace_services::Hp2TraceReporterServices =
    trace_services::Hp2TraceReporterServices {
        object_path_ansi,
        object_name_ansi,
        object_class,
        fname_ansi_or_empty,
        actor_tag_fname,
        actor_shadow,
        actor_x_level,
        actor_owner,
        actor_delete_marked,
        actor_pending_kill,
        actor_ticked,
        actor_game_relevant,
        actor_role,
        actor_remote_role,
        level_actor_count,
        level_actor_at,
        level_first_dynamic_actor,
        level_first_net_relevant_actor,
        game_engine_level,
        class_super,
        class_default_actor,
        cast_to_class,
        cast_to_actor,
        find_property,
        find_bool_property,
        find_int_property,
        find_float_property,
        property_offset,
        property_array_dim,
        property_element_size,
        bool_property_bitmask,
        property_outer,
        rand_trace_ordinal,
    };

unsafe extern "C" fn engine_viewport(engine: *mut c_void) -> *mut c_void {
    with_owner(ptr::null_mut(), |owner| {
        let game = unsafe { game_engine(owner, engine)? };
        Ok(if game.is_null() {
            ptr::null_mut()
        } else {
            unsafe { ptr::addr_of!((*game).g_viewport).read_unaligned() }
        })
    })
}
unsafe extern "C" fn viewport_render_device(viewport: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*viewport.cast::<UViewport>()).ren_dev).read_unaligned() }
}
unsafe extern "C" fn viewport_size(viewport: *mut c_void, x: *mut i32, y: *mut i32) {
    unsafe {
        x.write(ptr::addr_of!((*viewport.cast::<UViewport>()).size_x).read_unaligned());
        y.write(ptr::addr_of!((*viewport.cast::<UViewport>()).size_y).read_unaligned());
    }
}
unsafe extern "C" fn viewport_actor(viewport: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of!((*viewport.cast::<UViewport>()).actor).read_unaligned() }
}
unsafe extern "C" fn read_pixels(device: *mut c_void, buffer: *mut u8, pixel_count: usize) {
    with_owner((), |owner| unsafe {
        hp2_engine::render_device_owner::read_pixels(
            owner.access(),
            device.cast(),
            std::slice::from_raw_parts_mut(buffer.cast::<FColor>(), pixel_count),
        )
    });
}
unsafe fn write_ansi_bytes(text: *const c_char, buffer: *mut u8, capacity: usize) -> i32 {
    if text.is_null() {
        return 0;
    }
    let bytes = unsafe { CStr::from_ptr(text) }.to_bytes();
    if capacity != 0 {
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len().min(capacity));
        }
    }
    bytes.len() as i32
}
unsafe extern "C" fn actor_state_name(actor: *mut c_void, buffer: *mut u8, capacity: usize) -> i32 {
    let node = if actor.is_null() {
        ptr::null_mut()
    } else {
        let frame =
            unsafe { ptr::addr_of!((*actor.cast::<UObject>()).state_frame).read_unaligned() }
                .cast::<FStateFrame>();
        if frame.is_null() {
            ptr::null_mut()
        } else {
            unsafe { ptr::addr_of!((*frame).state_node).read_unaligned() }
        }
    };
    let text = if node.is_null() || node == unsafe { object_class(actor) } {
        c"None".as_ptr()
    } else {
        unsafe { object_name_ansi(node) }
    };
    unsafe { write_ansi_bytes(text, buffer, capacity) }
}
unsafe extern "C" fn actor_anim_sequence_name(
    actor: *mut c_void,
    buffer: *mut u8,
    capacity: usize,
) -> i32 {
    let name = i32::from_ne_bytes(unsafe {
        ptr::addr_of!((*actor.cast::<AActor>()).anim_sequence).read_unaligned()
    });
    let text = text_ansi(|owner, text| owner.access().write_name_text(name, text));
    unsafe { write_ansi_bytes(text, buffer, capacity) }
}
unsafe fn fidelity_property(
    access: CoreAccess<'_>,
    class: *mut UClass,
    bytes: *const u8,
    length: usize,
    expected: Option<&str>,
) -> RuntimeResult<*mut UProperty> {
    let mut scratch = [0u8; 128];
    let length = length.min(scratch.len() - 1);
    if length != 0 {
        unsafe {
            ptr::copy_nonoverlapping(bytes, scratch.as_mut_ptr(), length);
        }
    }
    unsafe { property_named(access, class, scratch.as_ptr().cast(), expected) }
}
unsafe extern "C" fn string_array(
    object: *mut c_void,
    name: *const u8,
    name_len: usize,
    count_name: *const u8,
    count_len: usize,
    buffer: *mut u8,
    capacity: usize,
) -> i32 {
    with_owner(0, |owner| {
        if object.is_null() {
            return Ok(0);
        }
        let access = owner.access();
        let class = unsafe { object_class(object) }.cast::<UClass>();
        if class.is_null() {
            return Ok(0);
        }
        let field = unsafe { fidelity_property(access, class, name, name_len, None)? };
        let count_field = unsafe {
            fidelity_property(
                access,
                class,
                count_name,
                count_len,
                Some("Core.IntProperty"),
            )?
        };
        if field.is_null() || count_field.is_null() {
            return Ok(0);
        }
        let count = unsafe {
            access
                .property_address(object.cast(), count_field)?
                .cast::<i32>()
                .read_unaligned()
        }
        .clamp(0, unsafe { (*field).array_dim });
        let base = unsafe { access.property_address(object.cast(), field)? };
        let stride = unsafe { (*field).element_size } as usize;
        let mut total = 0usize;
        for index in 0..count as usize {
            let value = unsafe {
                base.add(index * stride)
                    .cast::<TArrayRaw>()
                    .read_unaligned()
            };
            if value.num <= 1 {
                continue;
            }
            let ansi = unsafe { hp2_core::ansi::to_ansi_string(value.data.cast()) };
            if ansi.is_null() {
                continue;
            }
            let bytes = unsafe { CStr::from_ptr(ansi.cast()) }.to_bytes();
            if bytes.is_empty() {
                continue;
            }
            if total < capacity {
                unsafe {
                    ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        buffer.add(total),
                        bytes.len().min(capacity - total),
                    );
                }
            }
            total += bytes.len();
            if total < capacity {
                unsafe {
                    buffer.add(total).write(0);
                }
            }
            total += 1;
        }
        Ok(total as i32)
    })
}
unsafe fn fidelity_audio(
    owner: &EngineOwner,
    engine: *mut c_void,
) -> RuntimeResult<Option<std::ptr::NonNull<UALAudioSubsystem>>> {
    if engine.is_null() {
        return Ok(None);
    }
    let audio = unsafe { ptr::addr_of!((*engine.cast::<UEngine>()).audio).read_unaligned() };
    let class = owner.access().native_class("ALAudio.ALAudioSubsystem")?;
    if !unsafe { hp2_core::object_model::is_a(audio, class.cast()) } {
        return Ok(None);
    }
    Ok(std::ptr::NonNull::new(audio.cast()))
}
unsafe extern "C" fn audio_fidelity_source_count(engine: *mut c_void) -> i32 {
    with_owner(0, |owner| {
        let Some(audio) = (unsafe { fidelity_audio(owner, engine)? }) else {
            return Ok(0);
        };
        hp2_engine::audio_owner::fidelity_active_source_count(owner.access(), audio)
            .map(|count| count as i32)
    })
}
unsafe extern "C" fn audio_fidelity_source_id(
    engine: *mut c_void,
    index: i32,
    buffer: *mut u8,
    capacity: usize,
) -> i32 {
    with_owner(0, |owner| {
        if index < 0 {
            return Ok(0);
        }
        let Some(audio) = (unsafe { fidelity_audio(owner, engine)? }) else {
            return Ok(0);
        };
        let Some((sound, _)) =
            hp2_engine::audio_owner::fidelity_active_source(owner.access(), audio, index as usize)?
        else {
            return Ok(0);
        };
        let path = unsafe { object_path_ansi(sound as *mut c_void) };
        Ok(unsafe { write_ansi_bytes(path, buffer, capacity) })
    })
}
unsafe extern "C" fn audio_fidelity_source_paused(engine: *mut c_void, index: i32) -> i32 {
    with_owner(0, |owner| {
        if index < 0 {
            return Ok(0);
        }
        let Some(audio) = (unsafe { fidelity_audio(owner, engine)? }) else {
            return Ok(0);
        };
        Ok(
            hp2_engine::audio_owner::fidelity_active_source(owner.access(), audio, index as usize)?
                .map_or(0, |(_, paused)| i32::from(paused)),
        )
    })
}
unsafe extern "C" fn build_runtime_locator(
    thread: *mut c_void,
    actor_buffer: *mut u8,
    actor_capacity: usize,
    path_buffer: *mut u8,
    path_capacity: usize,
    result: *mut fidelity::Hp2RuntimeLocatorResult,
) -> i32 {
    if result.is_null() {
        return 0;
    }
    unsafe {
        result.write(fidelity::Hp2RuntimeLocatorResult {
            kind: 0,
            thread_slot: -1,
            cursor: -1,
            actor_len: 0,
            path_len: 0,
        });
    }
    with_owner(0, |owner| {
        if thread.is_null() {
            return Ok(0);
        }
        let access = owner.access();
        let class = unsafe { object_class(thread) }.cast();
        let disk_cursor = unsafe { property_named(access, class, c"curScriptLine".as_ptr(), None)? };
        let inline_cursor = unsafe { property_named(access, class, c"curScriptPosition".as_ptr(), None)? };
        let cursor = if !disk_cursor.is_null() {
            disk_cursor
        } else {
            inline_cursor
        };
        let parent = unsafe { property_named(access, class, c"parentCutScene".as_ptr(), None)? };
        if cursor.is_null() || parent.is_null() {
            return Ok(0);
        }
        let scene = unsafe {
            access
                .property_address(thread.cast(), parent)?
                .cast::<*mut UObject>()
                .read_unaligned()
        };
        if scene.is_null() {
            return Ok(0);
        }
        let scene_class = unsafe { (*scene).class.cast() };
        let filename = unsafe { property_named(access, scene_class, c"FileName".as_ptr(), None)? };
        let threads = unsafe { property_named(access, scene_class, c"aThreads".as_ptr(), None)? };
        if filename.is_null() || threads.is_null() {
            return Ok(0);
        }
        let mut value = fidelity::Hp2RuntimeLocatorResult {
            kind: 0,
            thread_slot: -1,
            cursor: unsafe {
                access.property_address(thread.cast(), cursor)?.cast::<i32>().read_unaligned()
            },
            actor_len: unsafe {
                write_ansi_bytes(object_path_ansi(scene.cast()), actor_buffer, actor_capacity)
            },
            path_len: 0,
        };
        let file = unsafe {
            access
                .property_address(scene, filename)?
                .cast::<TArrayRaw>()
                .read_unaligned()
        };
        if file.num > 1 {
            let ansi = unsafe { hp2_core::ansi::to_ansi_string(file.data.cast()) };
            value.path_len = unsafe { write_ansi_bytes(ansi.cast(), path_buffer, path_capacity) };
            value.kind = 1;
        } else if !inline_cursor.is_null() {
            let script = unsafe { property_named(access, class, c"script".as_ptr(), None)? };
            if script.is_null() {
                return Ok(0);
            }
            let script = unsafe {
                access
                    .property_address(thread.cast(), script)?
                    .cast::<TArrayRaw>()
                    .read_unaligned()
            };
            if script.num <= 1 {
                return Ok(0);
            }
            value.kind = 2;
        } else {
            return Ok(0);
        }
        let values = unsafe { access.property_address(scene, threads)? };
        for index in 0..unsafe { (*threads).array_dim } {
            let stride = unsafe { (*threads).element_size } as usize;
            if unsafe {
                values
                    .add(index as usize * stride)
                    .cast::<*mut c_void>()
                    .read_unaligned()
            } == thread
            {
                value.thread_slot = index;
                break;
            }
        }
        if value.thread_slot < 0 || value.cursor < 0 || value.actor_len <= 0
            || (value.kind == 1 && value.path_len <= 0)
        {
            return Ok(0);
        }
        unsafe { result.write(value) };
        Ok(value.kind)
    })
}

const FIDELITY_SERVICES: fidelity::Hp2FidelityServices = fidelity::Hp2FidelityServices {
    engine_viewport,
    viewport_render_device,
    viewport_size,
    viewport_actor,
    read_pixels,
    actor_state_name,
    actor_anim_sequence_name,
    string_array,
    audio_fidelity_source_count,
    audio_fidelity_source_id,
    audio_fidelity_source_paused,
    build_runtime_locator,
};
