//! Object natives over live reflection/state storage. Native callbacks are
//! exception boundaries, not policy; outputs are published at legacy sequence points.
use crate::ansi::Tchar;
use crate::array_storage::{self, Hp2ArrayServices};
use crate::vm_frame::{self, Hp2VmFrameServices, Hp2VmNativeTraceServices};
use crate::vm_registry::Hp2VmNativeHandler;
use crate::vm_state::{self, Hp2VmStateServices, NAME_NONE};
use crate::vm_string;
use hp2_abi::engine::generated::{FFrame, TArrayRaw, UObject};
use std::ffi::c_void;
use std::ptr::{null, null_mut};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmObjectServices {
    pub state: *const Hp2VmStateServices,
    pub script_mask: u32,
    pub public_flag: u32,
    pub const_flag: u32,
    pub load_no_warn: u32,
    pub load_quiet: u32,
    pub probe_min: i32,
    pub probe_max: i32,
    pub begin_name: i32,
    pub rand_max: f32,
    pub script_log_name: i32,
    /// kind: 0 forbidden new flags, 1 missing label, 2 missing state, 3 bad probe.
    pub warning: unsafe extern "C" fn(*mut c_void, u32, u32, i32) -> i32,
    pub transient: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    pub construct: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        i32,
        u32,
        *mut *mut c_void,
    ) -> i32,
    pub random: unsafe extern "C" fn(*mut i32) -> i32,
    pub trace_range: unsafe extern "C" fn(*mut c_void, i32, f32, f32, f32) -> i32,
    pub super_class: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub super_struct: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub frame_probe_mask: unsafe extern "C" fn(*mut c_void) -> u64,
    pub is_property: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub field_matches: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
    pub field_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub property_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub export_text: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub assign_text: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub import_text: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> i32,
    pub is_enum: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub enum_len: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub enum_name: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub load: unsafe extern "C" fn(*mut c_void, *const c_void, u32, *mut *mut c_void) -> i32,
    pub log_event: unsafe extern "C" fn(i32, *const c_void) -> i32,
    pub localize:
        unsafe extern "C" fn(*const c_void, *const c_void, *const c_void, *mut c_void) -> i32,
    pub get_language: unsafe extern "C" fn(*mut c_void) -> i32,
    pub save_config: unsafe extern "C" fn(*mut c_void) -> i32,
    pub reset_config: unsafe extern "C" fn(*mut c_void) -> i32,
    pub mem_tag_push: unsafe extern "C" fn() -> i32,
    pub mem_tag_pop: unsafe extern "C" fn() -> i32,
    pub default_object: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
}

static mut OBJECT_SERVICES: Option<Hp2VmObjectServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmObjectServices) {
    unsafe {
        (&raw mut OBJECT_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmObjectServices, i32> {
    unsafe { (&raw const OBJECT_SERVICES).read().ok_or(1) }
}
fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// # Safety
/// Pointers and services remain live; callbacks capture exceptions and may reenter.
pub unsafe fn new_object(
    context: *mut c_void,
    class: *mut c_void,
    mut outer: *mut c_void,
    name: i32,
    flags: u32,
    result: *mut *mut c_void,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let forbidden = flags & !s.script_mask;
        if forbidden != 0 {
            status((s.warning)(context, 0, forbidden, 0))?;
        }
        if outer.is_null() {
            status((s.transient)(&mut outer))?;
        }
        let mut object = null_mut();
        status((s.construct)(
            context,
            class,
            outer,
            name,
            flags & s.script_mask,
            &mut object,
        ))?;
        result.write(object);
        Ok(())
    }
}

/// # Safety
/// The native RNG scope surrounds this call; result is writable before trace reentry.
pub unsafe fn rand_range(
    context: *mut c_void,
    min: f32,
    max: f32,
    result: *mut f32,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut raw = 0;
        status((s.random)(&mut raw))?;
        // ARM64 contracts the final multiply/add, not the subtraction or division.
        let value = (raw as f32 / s.rand_max).mul_add(max - min, min);
        result.write(value);
        status((s.trace_range)(context, raw, min, max, value))
    }
}

/// # Safety
/// Class handles and live superclass callbacks satisfy the native reflection ABI.
pub unsafe fn class_child(
    mut class: *mut c_void,
    parent: *mut c_void,
    result: *mut u32,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        if !parent.is_null() {
            while !class.is_null() {
                if class == parent {
                    result.write(1);
                    return Ok(());
                }
                status((s.super_struct)(class, &mut class))?;
            }
        }
        result.write(0);
        Ok(())
    }
}

/// # Safety
/// State context matches the supplied state services and remains live.
pub unsafe fn current_state_name(state: *mut c_void, s: &Hp2VmObjectServices) -> i32 {
    unsafe {
        let t = &*s.state;
        if (t.has_frame)(state) != 0 {
            let node = (t.frame_state)(state);
            if node != (t.object_class)(state) {
                return (t.name)(node);
            }
        }
        NAME_NONE
    }
}

/// A non-fatal state request warning produced after the shared transition policy.
pub(crate) enum StateWarning {
    MissingLabel { requested: i32, label: i32 },
    MissingState { requested: i32, label: i32 },
}

/// Runs the GotoState/label sequence using one ordinary-Rust state host.
pub(crate) fn goto_state_and_label_with<F, O, E>(
    mut make_ops: F,
    state: *mut c_void,
    current_before_operands: i32,
    requested: i32,
    label: i32,
    auto_name: i32,
    begin_name: impl FnOnce() -> Result<i32, E>,
) -> Result<Option<StateWarning>, E>
where
    F: FnMut() -> Result<O, E>,
    O: vm_state::StateOps<Error = E>,
{
    let outcome = if requested == current_before_operands {
        vm_state::GOTO_SUCCESS
    } else {
        let mut ops = make_ops()?;
        vm_state::goto_state_with(state, requested, &mut ops)?
    };
    if outcome == vm_state::GOTO_SUCCESS {
        let begin = begin_name()?;
        let requested_label = label;
        let effective_label = if requested_label == vm_state::NAME_NONE { begin } else { requested_label };
        let mut ops = make_ops()?;
        let found = vm_state::goto_label_with(state, effective_label, &mut ops)?;
        if found == 0 && requested_label != vm_state::NAME_NONE {
            return Ok(Some(StateWarning::MissingLabel { requested, label: requested_label }));
        }
    } else if outcome == vm_state::GOTO_NOT_FOUND
        && requested != vm_state::NAME_NONE
        && requested != auto_name
    {
        return Ok(Some(StateWarning::MissingState { requested, label }));
    }
    Ok(None)
}

/// # Safety
/// Requested name is also installed in the native state context before this call.
pub unsafe fn goto_state(
    context: *mut c_void,
    state: *mut c_void,
    current: i32,
    name: i32,
    label: i32,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    let services = unsafe { *s.state };
    if let Some(warning) = goto_state_and_label_with(
        || Ok(services),
        state,
        current,
        name,
        label,
        services.auto_name,
        || Ok(s.begin_name),
    )? {
        let (kind, requested, label) = match warning {
            StateWarning::MissingLabel { requested, label } => (1, requested, label),
            StateWarning::MissingState { requested, label } => (2, requested, label),
        };
        status(unsafe { (s.warning)(context, kind, requested as u32, label) })?;
    }
    Ok(())
}

/// Operations needed to update a probe mask on a stateful object.
pub(crate) trait ProbeOps {
    type Error;

    fn has_frame(&mut self, object: *mut c_void) -> Result<bool, Self::Error>;
    fn frame_state(&mut self, object: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn object_class(&mut self, object: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn probe_mask(&mut self, state: *mut c_void) -> Result<u64, Self::Error>;
    fn ignore_mask(&mut self, state: *mut c_void) -> Result<u64, Self::Error>;
    fn frame_probe_mask(&mut self, object: *mut c_void) -> Result<u64, Self::Error>;
    fn set_probe_mask(&mut self, object: *mut c_void, mask: u64) -> Result<(), Self::Error>;
    fn invalid_probe(&mut self, name: i32) -> Result<(), Self::Error>;
}

/// Applies the shared probe-mask policy with host-specific diagnostics.
pub(crate) fn probe_with<O: ProbeOps>(
    ops: &mut O,
    object: *mut c_void,
    name: i32,
    enable: bool,
    probe_min: i32,
    probe_max: i32,
) -> Result<(), O::Error> {
    if name >= probe_min && name < probe_max && ops.has_frame(object)? {
        let bit = 1_u64 << (name - probe_min);
        let mask = if enable {
            let state = ops.frame_state(object)?;
            let class = ops.object_class(object)?;
            let base = (ops.probe_mask(state)? | ops.probe_mask(class)?) & ops.ignore_mask(state)?;
            ops.frame_probe_mask(object)? | (base & bit)
        } else {
            ops.frame_probe_mask(object)? & !bit
        };
        ops.set_probe_mask(object, mask)?;
    } else {
        ops.invalid_probe(name)?;
    }
    Ok(())
}

struct LegacyProbeOps<'a> {
    context: *mut c_void,
    services: &'a Hp2VmObjectServices,
}

impl ProbeOps for LegacyProbeOps<'_> {
    type Error = i32;

    fn has_frame(&mut self, object: *mut c_void) -> Result<bool, i32> {
        let state = unsafe { &*self.services.state };
        Ok(unsafe { (state.has_frame)(object) != 0 })
    }
    fn frame_state(&mut self, object: *mut c_void) -> Result<*mut c_void, i32> {
        let state = unsafe { &*self.services.state };
        Ok(unsafe { (state.frame_state)(object) })
    }
    fn object_class(&mut self, object: *mut c_void) -> Result<*mut c_void, i32> {
        let state = unsafe { &*self.services.state };
        Ok(unsafe { (state.object_class)(object) })
    }
    fn probe_mask(&mut self, state: *mut c_void) -> Result<u64, i32> {
        let services = unsafe { &*self.services.state };
        Ok(unsafe { (services.probe_mask)(state) })
    }
    fn ignore_mask(&mut self, state: *mut c_void) -> Result<u64, i32> {
        let services = unsafe { &*self.services.state };
        Ok(unsafe { (services.ignore_mask)(state) })
    }
    fn frame_probe_mask(&mut self, object: *mut c_void) -> Result<u64, i32> {
        Ok(unsafe { (self.services.frame_probe_mask)(object) })
    }
    fn set_probe_mask(&mut self, object: *mut c_void, mask: u64) -> Result<(), i32> {
        let state = unsafe { &*self.services.state };
        unsafe { (state.set_probe_mask)(object, mask) };
        Ok(())
    }
    fn invalid_probe(&mut self, name: i32) -> Result<(), i32> {
        status(unsafe { (self.services.warning)(self.context, 3, name as u32, 0) })
    }
}

/// # Safety
/// Enable is called before native P_FINISH, disable after it. No frame snapshot survives reentry.
pub unsafe fn probe(
    context: *mut c_void,
    state: *mut c_void,
    name: i32,
    enable: bool,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    let mut ops = LegacyProbeOps { context, services: s };
    probe_with(&mut ops, state, name, enable, s.probe_min, s.probe_max)
}

unsafe fn find_property(
    state: *mut c_void,
    text: *const c_void,
    s: &Hp2VmObjectServices,
) -> Result<*mut c_void, i32> {
    unsafe {
        let t = &*s.state;
        let mut owner = (t.object_class)(state);
        while !owner.is_null() {
            let mut field = (t.children)(owner);
            while !field.is_null() {
                let mut matches = 0;
                status((s.is_property)(field, &mut matches))?;
                if matches != 0 {
                    status((s.field_matches)(field, text, &mut matches))?;
                    if matches != 0 {
                        return Ok(field);
                    }
                }
                field = (t.next_field)(field);
            }
            status((t.inheritance_super)(owner, &mut owner))?;
        }
        Ok(null_mut())
    }
}

/// # Safety
/// Native shell owns zeroed TCHAR[1024] scratch and FString arguments/results.
/// Only get uses scratch/result; only set uses value. No storage is borrowed across callbacks.
pub unsafe fn property_text(
    context: *mut c_void,
    state: *mut c_void,
    text: *const c_void,
    value: *const c_void,
    scratch: *mut c_void,
    result: *mut c_void,
    set: bool,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let property = find_property(state, text, s)?;
        if !property.is_null() && (s.field_flags)(property) & s.public_flag != 0 {
            if set {
                if (s.property_flags)(property) & s.const_flag == 0 {
                    status((s.import_text)(context, property, value))?;
                }
            } else {
                status((s.export_text)(context, property, scratch))?;
            }
        }
        if !set {
            status((s.assign_text)(result, scratch))?;
        }
        Ok(())
    }
}

/// # Safety
/// Object and output are native enum/name storage; output defaults before cast/bounds work.
pub unsafe fn get_enum(
    object: *mut c_void,
    index: i32,
    result: *mut i32,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        result.write(NAME_NONE);
        let mut matches = 0;
        status((s.is_enum)(object, &mut matches))?;
        if matches != 0 && index >= 0 {
            // The legacy expression casts separately for the length and element.
            status((s.is_enum)(object, &mut matches))?;
            let enumeration = if matches != 0 { object } else { null_mut() };
            let mut count = 0;
            status((s.enum_len)(enumeration, &mut count))?;
            if index < count {
                status((s.is_enum)(object, &mut matches))?;
                let enumeration = if matches != 0 { object } else { null_mut() };
                let mut name = NAME_NONE;
                status((s.enum_name)(enumeration, index, &mut name))?;
                result.write(name);
            }
        }
        Ok(())
    }
}

/// # Safety
/// Class/text/output remain valid inside the native memory-tag scope.
pub unsafe fn dynamic_load(
    class: *mut c_void,
    text: *const c_void,
    may_fail: u32,
    result: *mut *mut c_void,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let flags = s.load_no_warn | if may_fail != 0 { s.load_quiet } else { 0 };
        let mut object = null_mut();
        status((s.load)(class, text, flags, &mut object))?;
        result.write(object);
        Ok(())
    }
}

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2VmObjectQuery {
    IsInState = 0,
    GetStateName = 1,
    IsA = 2,
}

/// # Safety
/// State context and reflection callbacks stay live during inherited traversal.
pub unsafe fn query(
    op: Hp2VmObjectQuery,
    state: *mut c_void,
    name: i32,
    result: *mut i32,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let t = &*s.state;
        let mut node = match op {
            Hp2VmObjectQuery::IsA => (t.object_class)(state),
            _ => {
                if (t.has_frame)(state) != 0 {
                    (t.frame_state)(state)
                } else {
                    null_mut()
                }
            }
        };
        if matches!(op, Hp2VmObjectQuery::GetStateName) {
            result.write(if node.is_null() {
                NAME_NONE
            } else {
                (t.name)(node)
            });
            return Ok(());
        }
        while !node.is_null() {
            if (t.name)(node) == name {
                result.write(1);
                return Ok(());
            }
            match op {
                Hp2VmObjectQuery::IsA => status((s.super_class)(node, &mut node))?,
                _ => status((t.super_state)(node, &mut node))?,
            }
        }
        result.write(0);
        Ok(())
    }
}

/// Mirrors the native `FVmStateContext` leaf layout: this object and a
/// requested state name (unused by the ClassIsChildOf/IsA queries).
#[repr(C)]
struct StateContext {
    object: *mut c_void,
    requested_name: i32,
}

/// Mirrors the native `FVmObjectContext` leaf layout used by RandRange's trace.
#[repr(C)]
struct ObjectContext {
    state: StateContext,
    stack: *mut c_void,
}

/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_class_is_child_of(
    frame: *mut FFrame,
    result: *mut u32,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut class: *mut c_void = null_mut();
        vm_frame::step_argument(frame, (&raw mut class).cast(), native_table)?;
        let mut parent: *mut c_void = null_mut();
        vm_frame::step_argument(frame, (&raw mut parent).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        class_child(class, parent, result, s)
    }
}

/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_is_a(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut i32,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut class_name = NAME_NONE;
        vm_frame::step_argument(frame, (&raw mut class_name).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let context = StateContext {
            object,
            requested_name: NAME_NONE,
        };
        query(
            Hp2VmObjectQuery::IsA,
            (&raw const context).cast_mut().cast(),
            class_name,
            result,
            s,
        )
    }
}

/// Compute CallsiteOffset, decode Min/Max, then draw inside the native RNG
/// scope, which always opens and closes on every path.
///
/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_rand_range(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut f32,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
    trace: &Hp2VmNativeTraceServices,
) -> Result<(), i32> {
    unsafe {
        let offset = vm_frame::callsite_offset(frame, &trace.frame)?;
        let mut min = 0.0f32;
        vm_frame::step_argument(frame, (&raw mut min).cast(), native_table)?;
        let mut max = 0.0f32;
        vm_frame::step_argument(frame, (&raw mut max).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let node = vm_frame::open_native_trace_scope(
            frame,
            c"Core.Object.RandRange".as_ptr(),
            1033,
            offset,
            trace,
        )?;
        let context = ObjectContext {
            state: StateContext {
                object,
                requested_name: NAME_NONE,
            },
            stack: frame.cast(),
        };
        let draw = rand_range((&raw const context).cast_mut().cast(), min, max, result, s);
        let end = vm_frame::close_native_trace_scope(node, trace);
        draw?;
        end
    }
}

/// Outer/Name/Flags/Class are all optional, defaulting to
/// `GetIndex()!=INDEX_NONE ? this : NULL`/None/0/NULL. New has no P_FINISH in
/// the native opcode stream: the terminator is never consumed here.
///
/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_new(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let index = (*object.cast::<UObject>()).index;
        let mut outer = if index != -1 { object } else { null_mut() };
        vm_frame::step_argument(frame, (&raw mut outer).cast(), native_table)?;
        let mut name = NAME_NONE;
        vm_frame::step_argument(frame, (&raw mut name).cast(), native_table)?;
        let mut flags: u32 = 0;
        vm_frame::step_argument(frame, (&raw mut flags).cast(), native_table)?;
        let mut class: *mut c_void = null_mut();
        vm_frame::step_argument(frame, (&raw mut class).cast(), native_table)?;
        let context = ObjectContext {
            state: StateContext {
                object,
                requested_name: NAME_NONE,
            },
            stack: frame.cast(),
        };
        new_object(
            (&raw const context).cast_mut().cast(),
            class,
            outer,
            name,
            flags,
            result,
            s,
        )
    }
}

/// S defaults to the current state name (computed before any operand is
/// decoded), L defaults to None. GotoState has no result.
///
/// # Safety
/// Frame, live native table and services are live.
pub unsafe fn execute_goto_state(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut context = ObjectContext {
            state: StateContext {
                object,
                requested_name: NAME_NONE,
            },
            stack: frame.cast(),
        };
        let current = current_state_name((&raw mut context.state).cast(), s);
        let mut requested = current;
        vm_frame::step_argument(frame, (&raw mut requested).cast(), native_table)?;
        let mut label = NAME_NONE;
        vm_frame::step_argument(frame, (&raw mut label).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        context.state.requested_name = requested;
        goto_state(
            (&raw mut context).cast(),
            (&raw mut context.state).cast(),
            current,
            requested,
            label,
            s,
        )
    }
}

/// Probes before P_FINISH, matching the legacy evaluation order.
///
/// # Safety
/// Frame, live native table and services are live.
pub unsafe fn execute_enable(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut name = NAME_NONE;
        vm_frame::step_argument(frame, (&raw mut name).cast(), native_table)?;
        let context = ObjectContext {
            state: StateContext {
                object,
                requested_name: NAME_NONE,
            },
            stack: frame.cast(),
        };
        probe(
            (&raw const context).cast_mut().cast(),
            (&raw const context.state).cast_mut().cast(),
            name,
            true,
            s,
        )?;
        vm_frame::finish_arguments(frame, native_table)
    }
}

/// Probes after P_FINISH, matching the legacy evaluation order.
///
/// # Safety
/// Frame, live native table and services are live.
pub unsafe fn execute_disable(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut name = NAME_NONE;
        vm_frame::step_argument(frame, (&raw mut name).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let context = ObjectContext {
            state: StateContext {
                object,
                requested_name: NAME_NONE,
            },
            stack: frame.cast(),
        };
        probe(
            (&raw const context).cast_mut().cast(),
            (&raw const context.state).cast_mut().cast(),
            name,
            false,
            s,
        )
    }
}

/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_is_in_state(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut i32,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut state_name = NAME_NONE;
        vm_frame::step_argument(frame, (&raw mut state_name).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        let context = StateContext {
            object,
            requested_name: NAME_NONE,
        };
        query(
            Hp2VmObjectQuery::IsInState,
            (&raw const context).cast_mut().cast(),
            state_name,
            result,
            s,
        )
    }
}

/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_get_state_name(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut i32,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        vm_frame::finish_arguments(frame, native_table)?;
        let context = StateContext {
            object,
            requested_name: NAME_NONE,
        };
        query(
            Hp2VmObjectQuery::GetStateName,
            (&raw const context).cast_mut().cast(),
            NAME_NONE,
            result,
            s,
        )
    }
}

/// S is an owning FString argument; N defaults to the published ScriptLog
/// name index.
///
/// # Safety
/// Frame, live native table, services and the published array allocator are
/// live. Argument storage is destroyed on every path, preserving the first error.
pub unsafe fn execute_log(
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let mut text = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let text = &raw mut text;
        let operation = (|| {
            vm_frame::step_argument(frame, text.cast(), native_table)?;
            let mut event = s.script_log_name;
            vm_frame::step_argument(frame, (&raw mut event).cast(), native_table)?;
            vm_frame::finish_arguments(frame, native_table)?;
            status((s.log_event)(event, vm_string::view(text).data))
        })();
        let destroy = array_storage::release(vm_string::slots(text), arrays);
        operation.and(destroy)
    }
}

/// S is an owning FString argument. `Stack.Logf(TEXT("%s"), *S)` is a single
/// composite leaf: the real FOutputDevice::Logf path (SafeSuppressed(NAME_Log)
/// check, 4096-char formatting buffer) runs in C++ ahead of FFrame::Serialize,
/// not the bare `vm_frame::serialize` used by other FFrame::Serialize callers.
///
/// # Safety
/// Frame, live native table, the published array allocator and frame services
/// are live. Argument storage is destroyed on every path.
pub unsafe fn execute_warn(
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    frame_services: &Hp2VmFrameServices,
) -> Result<(), i32> {
    unsafe {
        let mut text = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let text = &raw mut text;
        let operation = (|| {
            vm_frame::step_argument(frame, text.cast(), native_table)?;
            vm_frame::finish_arguments(frame, native_table)?;
            status((frame_services.logf)(
                frame.cast(),
                vm_string::view(text).data,
            ))
        })();
        let destroy = array_storage::release(vm_string::slots(text), arrays);
        operation.and(destroy)
    }
}

/// `UCommandlet::execMain` (`Core/Src/UnObj.cpp`): Parms is an owning
/// FString argument; Result receives `((UCommandlet*)Object)->Main(*Parms)`.
///
/// # Safety
/// Object, frame, result, live native table, the published array allocator
/// and `main` are live. Argument storage is destroyed on every path,
/// preserving the first error.
pub unsafe fn execute_commandlet_main(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    arrays: Hp2ArrayServices,
    main: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
) -> Result<(), i32> {
    unsafe {
        let mut parms = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let parms = &raw mut parms;
        let operation = (|| {
            vm_frame::step_argument(frame, parms.cast(), native_table)?;
            vm_frame::finish_arguments(frame, native_table)?;
            let mut value: i32 = 0;
            status(main(object, vm_string::view(parms).data, &raw mut value))?;
            result.cast::<i32>().write(value);
            Ok(())
        })();
        let destroy = array_storage::release(vm_string::slots(parms), arrays);
        operation.and(destroy)
    }
}

/// SectionName/KeyName/PackageName are owning FString arguments; the result is
/// written entirely by the native leaf, mirroring the string-result assignment
/// used by property text.
///
/// # Safety
/// Frame, live native table, services, the published array allocator and a
/// writable FString result are live. Argument storage is destroyed on every path.
pub unsafe fn execute_localize(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let mut section = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let mut key = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let mut package = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let section = &raw mut section;
        let key = &raw mut key;
        let package = &raw mut package;
        let operation = (|| {
            vm_frame::step_argument(frame, section.cast(), native_table)?;
            vm_frame::step_argument(frame, key.cast(), native_table)?;
            vm_frame::step_argument(frame, package.cast(), native_table)?;
            vm_frame::finish_arguments(frame, native_table)?;
            status((s.localize)(
                vm_string::view(section).data,
                vm_string::view(key).data,
                vm_string::view(package).data,
                result,
            ))
        })();
        let destroy_package = array_storage::release(vm_string::slots(package), arrays);
        let destroy_key = array_storage::release(vm_string::slots(key), arrays);
        let destroy_section = array_storage::release(vm_string::slots(section), arrays);
        operation
            .and(destroy_package)
            .and(destroy_key)
            .and(destroy_section)
    }
}

/// PropName is an owning FString argument; the result is written through a
/// zeroed TCHAR[1024] scratch buffer, matching the native shell's stack scratch.
///
/// # Safety
/// Frame, live native table, services, the published array allocator and a
/// writable FString result are live. Argument storage is destroyed on every path.
pub unsafe fn execute_get_property_text(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let mut text = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let text = &raw mut text;
        let operation = (|| {
            vm_frame::step_argument(frame, text.cast(), native_table)?;
            vm_frame::finish_arguments(frame, native_table)?;
            let context = ObjectContext {
                state: StateContext {
                    object,
                    requested_name: NAME_NONE,
                },
                stack: frame.cast(),
            };
            let mut scratch: [Tchar; 1024] = [0; 1024];
            property_text(
                (&raw const context).cast_mut().cast(),
                (&raw const context.state).cast_mut().cast(),
                vm_string::view(text).data,
                null(),
                scratch.as_mut_ptr().cast(),
                result,
                false,
                s,
            )
        })();
        let destroy = array_storage::release(vm_string::slots(text), arrays);
        operation.and(destroy)
    }
}

/// PropName/PropValue are owning FString arguments; no result is written.
///
/// # Safety
/// Frame, live native table, services and the published array allocator are
/// live. Argument storage is destroyed on every path.
pub unsafe fn execute_set_property_text(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let mut name = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let mut value = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let name = &raw mut name;
        let value = &raw mut value;
        let operation = (|| {
            vm_frame::step_argument(frame, name.cast(), native_table)?;
            vm_frame::step_argument(frame, value.cast(), native_table)?;
            vm_frame::finish_arguments(frame, native_table)?;
            let context = ObjectContext {
                state: StateContext {
                    object,
                    requested_name: NAME_NONE,
                },
                stack: frame.cast(),
            };
            property_text(
                (&raw const context).cast_mut().cast(),
                (&raw const context.state).cast_mut().cast(),
                vm_string::view(name).data,
                vm_string::view(value).data,
                null_mut(),
                null_mut(),
                true,
                s,
            )
        })();
        let destroy_value = array_storage::release(vm_string::slots(value), arrays);
        let destroy_name = array_storage::release(vm_string::slots(name), arrays);
        operation.and(destroy_value).and(destroy_name)
    }
}

/// # Safety
/// Frame, object and services are live.
pub unsafe fn execute_save_config(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        vm_frame::finish_arguments(frame, native_table)?;
        status((s.save_config)(object))
    }
}

/// `ResetConfig(GetClass())`: GetClass() just returns the object's own Class field.
///
/// # Safety
/// Frame, object and services are live.
pub unsafe fn execute_reset_config(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        vm_frame::finish_arguments(frame, native_table)?;
        let class = (*object.cast::<UObject>()).class;
        status((s.reset_config)(class))
    }
}

/// `Class->GetDefaultObject()->SaveConfig()`: GetDefaultObject() asserts
/// `Defaults.Num()==GetPropertiesSize()`, so the default object is obtained
/// through the same native leaf used by the variable-access family, not a
/// raw field read.
///
/// # Safety
/// Frame, object and services are live.
pub unsafe fn execute_static_save_config(
    object: *mut c_void,
    frame: *mut FFrame,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        vm_frame::finish_arguments(frame, native_table)?;
        let class = (*object.cast::<UObject>()).class;
        let mut default_object: *mut c_void = null_mut();
        status((s.default_object)(class, &raw mut default_object))?;
        status((s.save_config)(default_object))
    }
}

/// # Safety
/// Frame, live native table and services are live; result is a writable slot.
pub unsafe fn execute_get_enum(
    frame: *mut FFrame,
    result: *mut i32,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        let mut object: *mut c_void = null_mut();
        vm_frame::step_argument(frame, (&raw mut object).cast(), native_table)?;
        let mut index = 0_i32;
        vm_frame::step_argument(frame, (&raw mut index).cast(), native_table)?;
        vm_frame::finish_arguments(frame, native_table)?;
        get_enum(object, index, result, s)
    }
}

/// # Safety
/// Frame, services and a writable FString result are live.
pub unsafe fn execute_get_language(
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
) -> Result<(), i32> {
    unsafe {
        vm_frame::finish_arguments(frame, native_table)?;
        status((s.get_language)(result))
    }
}

/// Name is an owning FString argument; Class and bMayFail are plain operands.
/// The native memory-tag scope wraps only the load call, matching
/// `FPushMemTag Push(TEXT("DynamicLoadObject"))`.
///
/// # Safety
/// Frame, live native table, services and the published array allocator are
/// live. Argument storage is destroyed on every path.
pub unsafe fn execute_dynamic_load_object(
    frame: *mut FFrame,
    result: *mut *mut c_void,
    native_table: *const Hp2VmNativeHandler,
    s: &Hp2VmObjectServices,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    unsafe {
        let mut name = TArrayRaw {
            data: null_mut(),
            num: 0,
            max: 0,
        };
        let name = &raw mut name;
        let operation = (|| {
            vm_frame::step_argument(frame, name.cast(), native_table)?;
            let mut class: *mut c_void = null_mut();
            vm_frame::step_argument(frame, (&raw mut class).cast(), native_table)?;
            let mut may_fail: u32 = 0;
            vm_frame::step_argument(frame, (&raw mut may_fail).cast(), native_table)?;
            vm_frame::finish_arguments(frame, native_table)?;
            status((s.mem_tag_push)())?;
            let load = dynamic_load(class, vm_string::view(name).data, may_fail, result, s);
            let pop = status((s.mem_tag_pop)());
            load.and(pop)
        })();
        let destroy = array_storage::release(vm_string::slots(name), arrays);
        operation.and(destroy)
    }
}
