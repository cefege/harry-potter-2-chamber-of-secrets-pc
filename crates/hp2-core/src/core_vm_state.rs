//! Direct Core host operations for the shared state and label policy.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::name_owner::FindName;
use crate::vm_state::{self, StateOps};
use hp2_abi::engine::generated::{FStateFrame, TArrayRaw, UClass, UField, UFunction, UObject, UState, UStruct};
use std::ffi::c_void;
use std::ptr;

const PROBE_MIN: i32 = 197;
const PROBE_MAX: i32 = 261;

/// Selects whether a state-frame probe bit is set or cleared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeMode {
    Enable,
    Disable,
}

fn object_name(object: *mut UObject) -> i32 {
    // SAFETY: Callers validate the object and use the engine's stable UObject layout.
    unsafe { i32::from_ne_bytes(ptr::addr_of!((*object).name).read()) }
}

fn frame(object: *mut UObject) -> *mut FStateFrame {
    // SAFETY: Callers validate the object and use the engine's stable UObject layout.
    unsafe { ptr::addr_of!((*object).state_frame).read().cast() }
}

fn frame_state(object: *mut UObject) -> *mut UState {
    let frame = frame(object);
    if frame.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: The live state frame stores a state node pointer.
    unsafe { ptr::addr_of!((*frame).state_node).read().cast() }
}

struct CoreStateOps<'a> {
    access: CoreAccess<'a>,
    auto_name: i32,
    begin_name: i32,
    end_name: i32,
    state_class: *mut UClass,
}

impl<'a> CoreStateOps<'a> {
    fn new(access: CoreAccess<'a>) -> RuntimeResult<Self> {
        let auto_name = access.intern_str("Auto", FindName::Find)?;
        let begin_name = access.intern_str("BeginState", FindName::Find)?;
        let end_name = access.intern_str("EndState", FindName::Find)?;
        let state_class = access.native_class("State")?;
        Ok(Self { access, auto_name, begin_name, end_name, state_class })
    }

    fn call_state_event(&mut self, object: *mut UObject, event: &str) -> RuntimeResult<()> {
        let event_name = self.access.intern_str(event, FindName::Find)?;
        let function_class = self.access.native_class("Function")?;
        let state = frame_state(object);
        if state.is_null() {
            return Ok(());
        }
        let function = unsafe {
            self.access.find_field(state.cast(), event_name, function_class).cast::<UFunction>()
        };
        if !function.is_null() {
            unsafe { self.access.execute_vm_event(object, function, ptr::null_mut(), ptr::null_mut())?; }
        }
        Ok(())
    }
}

impl StateOps for CoreStateOps<'_> {
    type Error = RuntimeError;

    fn auto_name(&self) -> i32 { self.auto_name }
    fn begin_state_name(&self) -> i32 { self.begin_name }
    fn end_state_name(&self) -> i32 { self.end_name }

    fn has_frame(&mut self, object: *mut c_void) -> RuntimeResult<bool> {
        Ok(!object.is_null() && !frame(object.cast()).is_null())
    }

    fn clear_latent(&mut self, object: *mut c_void) -> RuntimeResult<()> {
        let state_frame = frame(object.cast());
        if !state_frame.is_null() {
            unsafe { ptr::addr_of_mut!((*state_frame).latent_action).write(0) };
        }
        Ok(())
    }

    fn frame_state(&mut self, object: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(frame_state(object.cast()).cast())
    }

    fn object_class(&mut self, object: *mut c_void) -> RuntimeResult<*mut c_void> {
        if object.is_null() { return Ok(ptr::null_mut()); }
        Ok(unsafe { ptr::addr_of!((*object.cast::<UObject>()).class).read() })
    }

    fn name(&mut self, object: *mut c_void) -> RuntimeResult<i32> {
        if object.is_null() { return Err(RuntimeError::new("Invalid Core state name")); }
        Ok(object_name(object.cast()))
    }

    fn find_state(&mut self, object: *mut c_void, name: i32) -> RuntimeResult<*mut c_void> {
        if object.is_null() { return Ok(ptr::null_mut()); }
        let class = unsafe { ptr::addr_of!((*object.cast::<UObject>()).class).read().cast::<UClass>() };
        Ok(unsafe { self.access.find_field(class.cast(), name, self.state_class).cast() })
    }

    fn children(&mut self, owner: *mut c_void) -> RuntimeResult<*mut c_void> {
        if owner.is_null() { return Ok(ptr::null_mut()); }
        Ok(unsafe { ptr::addr_of!((*owner.cast::<UStruct>()).children).read().cast() })
    }

    fn next_field(&mut self, field: *mut c_void) -> RuntimeResult<*mut c_void> {
        if field.is_null() { return Ok(ptr::null_mut()); }
        Ok(unsafe { ptr::addr_of!((*field.cast::<UField>()).next).read().cast() })
    }

    fn inheritance_super(&mut self, owner: *mut c_void) -> RuntimeResult<*mut c_void> {
        if owner.is_null() { return Ok(ptr::null_mut()); }
        Ok(unsafe { ptr::addr_of!((*owner.cast::<UField>()).super_field).read() })
    }

    fn is_state(&mut self, field: *mut c_void) -> RuntimeResult<bool> {
        Ok(!field.is_null() && unsafe { crate::object_model::is_a(field, self.state_class.cast()) })
    }

    fn state_flags(&mut self, state: *mut c_void) -> RuntimeResult<u32> {
        Ok(unsafe { ptr::addr_of!((*state.cast::<UState>()).state_flags).read() })
    }

    fn object_flags(&mut self, object: *mut c_void) -> RuntimeResult<u32> {
        Ok(unsafe { ptr::addr_of!((*object.cast::<UObject>()).object_flags).read() })
    }

    fn clear_flags(&mut self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        let object = object.cast::<UObject>();
        let old = unsafe { ptr::addr_of!((*object).object_flags).read() };
        unsafe { ptr::addr_of_mut!((*object).object_flags).write(old & !flags) };
        Ok(())
    }

    fn set_flags(&mut self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        let object = object.cast::<UObject>();
        let old = unsafe { ptr::addr_of!((*object).object_flags).read() };
        unsafe { ptr::addr_of_mut!((*object).object_flags).write(old | flags) };
        Ok(())
    }

    fn probing(&mut self, object: *mut c_void, name: i32) -> RuntimeResult<bool> {
        if object.is_null() { return Ok(false); }
        if crate::object_instance::probe_bit(name).is_none() {
            return Ok(true);
        }
        let state_frame = frame(object.cast());
        let mask = if state_frame.is_null() {
            None
        } else {
            Some(unsafe { ptr::addr_of!((*state_frame).probe_mask).read() })
        };
        Ok(crate::object_instance::probe_enabled(name, mask))
    }

    fn end_state(&mut self, object: *mut c_void) -> RuntimeResult<()> {
        self.call_state_event(object.cast(), "EndState")
    }

    fn begin_state(&mut self, object: *mut c_void) -> RuntimeResult<()> {
        self.call_state_event(object.cast(), "BeginState")
    }

    fn publish_state(&mut self, object: *mut c_void, state: *mut c_void) -> RuntimeResult<()> {
        let state_frame = frame(object.cast());
        if !state_frame.is_null() {
            unsafe {
                ptr::addr_of_mut!((*state_frame).node).write(state);
                ptr::addr_of_mut!((*state_frame).state_node).write(state);
                ptr::addr_of_mut!((*state_frame).code).write(ptr::null_mut());
            }
        }
        Ok(())
    }

    fn probe_mask(&mut self, state: *mut c_void) -> RuntimeResult<u64> {
        Ok(unsafe { ptr::addr_of!((*state.cast::<UState>()).probe_mask).read() })
    }

    fn ignore_mask(&mut self, state: *mut c_void) -> RuntimeResult<u64> {
        Ok(unsafe { ptr::addr_of!((*state.cast::<UState>()).ignore_mask).read() })
    }

    fn set_probe_mask(&mut self, object: *mut c_void, mask: u64) -> RuntimeResult<()> {
        let state_frame = frame(object.cast());
        if !state_frame.is_null() {
            unsafe { ptr::addr_of_mut!((*state_frame).probe_mask).write(mask) };
        }
        Ok(())
    }

    fn super_state(&mut self, state: *mut c_void) -> RuntimeResult<*mut c_void> {
        if state.is_null() { return Ok(ptr::null_mut()); }
        let parent = unsafe { ptr::addr_of!((*state.cast::<UField>()).super_field).read() };
        if !parent.is_null() && !unsafe { crate::object_model::is_a(parent, self.state_class.cast()) } {
            return Ok(ptr::null_mut());
        }
        Ok(parent)
    }

    fn label_table(&mut self, state: *mut c_void) -> RuntimeResult<u16> {
        Ok(unsafe { ptr::addr_of!((*state.cast::<UState>()).label_table_offset).read() })
    }

    fn script_int(&mut self, state: *mut c_void, offset: i32) -> RuntimeResult<i32> {
        let script = unsafe { ptr::addr_of!((*state.cast::<UStruct>()).script).read() };
        if script.data.is_null() || offset < 0 || offset >= script.num {
            return Err(RuntimeError::new("Script index out of range"));
        }
        // SAFETY: Legacy Script(offset) validates one byte then reads an unaligned INT.
        Ok(unsafe { script.data.cast::<u8>().add(offset as usize).cast::<i32>().read_unaligned() })
    }

    fn label_matches(&mut self, name: i32, label: i32) -> RuntimeResult<bool> {
        self.access.name_text(name)?;
        Ok(name == label)
    }

    fn set_node(&mut self, object: *mut c_void, state: *mut c_void) -> RuntimeResult<()> {
        let state_frame = frame(object.cast());
        if !state_frame.is_null() {
            unsafe { ptr::addr_of_mut!((*state_frame).node).write(state) };
        }
        Ok(())
    }

    fn set_code(&mut self, object: *mut c_void, state: *mut c_void, offset: i32) -> RuntimeResult<()> {
        let script = unsafe { ptr::addr_of!((*state.cast::<UStruct>()).script).read() };
        if script.data.is_null() || offset < 0 || offset >= script.num {
            return Err(RuntimeError::new("Script index out of range"));
        }
        let code = unsafe { script.data.cast::<u8>().add(offset as usize) };
        let state_frame = frame(object.cast());
        if !state_frame.is_null() {
            unsafe { ptr::addr_of_mut!((*state_frame).code).write(code) };
        }
        Ok(())
    }

    fn clear_code(&mut self, object: *mut c_void) -> RuntimeResult<()> {
        let state_frame = frame(object.cast());
        if !state_frame.is_null() {
            unsafe { ptr::addr_of_mut!((*state_frame).code).write(ptr::null_mut()) };
        }
        Ok(())
    }
}

struct CoreProbeOps<'a> {
    access: CoreAccess<'a>,
}

impl crate::vm_object::ProbeOps for CoreProbeOps<'_> {
    type Error = RuntimeError;

    fn has_frame(&mut self, object: *mut c_void) -> RuntimeResult<bool> {
        Ok(!object.is_null() && !frame(object.cast()).is_null())
    }

    fn frame_state(&mut self, object: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(frame_state(object.cast()).cast())
    }

    fn object_class(&mut self, object: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { ptr::addr_of!((*object.cast::<UObject>()).class).read() })
    }

    fn probe_mask(&mut self, state: *mut c_void) -> RuntimeResult<u64> {
        Ok(unsafe { ptr::addr_of!((*state.cast::<UState>()).probe_mask).read() })
    }

    fn ignore_mask(&mut self, state: *mut c_void) -> RuntimeResult<u64> {
        Ok(unsafe { ptr::addr_of!((*state.cast::<UState>()).ignore_mask).read() })
    }

    fn frame_probe_mask(&mut self, object: *mut c_void) -> RuntimeResult<u64> {
        let state_frame = frame(object.cast());
        if state_frame.is_null() {
            return Ok(0);
        }
        Ok(unsafe { ptr::addr_of!((*state_frame).probe_mask).read() })
    }

    fn set_probe_mask(&mut self, object: *mut c_void, mask: u64) -> RuntimeResult<()> {
        let state_frame = frame(object.cast());
        if !state_frame.is_null() {
            unsafe { ptr::addr_of_mut!((*state_frame).probe_mask).write(mask) };
        }
        Ok(())
    }

    fn invalid_probe(&mut self, name: i32) -> RuntimeResult<()> {
        let text = self.access.name_text(name)?;
        let mut warning = "Enable: '".chars().map(u32::from).collect::<Vec<_>>();
        warning.extend(text);
        warning.extend("' is not a probe function".chars().map(u32::from));
        self.access.emit_warning(&warning)
    }
}

impl CoreAccess<'_> {
    pub(crate) fn goto_state(self, object: *mut UObject, requested: i32) -> RuntimeResult<i32> {
        if requested == self.get_state_name(object)? {
            return Ok(vm_state::GOTO_SUCCESS);
        }
        let mut ops = CoreStateOps::new(self)?;
        vm_state::goto_state_with(object.cast(), requested, &mut ops)
    }

    fn transition_state(self, object: *mut UObject, requested: i32) -> RuntimeResult<i32> {
        let mut ops = CoreStateOps::new(self)?;
        vm_state::goto_state_with(object.cast(), requested, &mut ops)
    }

    pub(crate) fn goto_label(self, object: *mut UObject, label: i32) -> RuntimeResult<i32> {
        if object.is_null() {
            return Err(RuntimeError::new("Invalid Core state object"));
        }
        let mut ops = CoreStateOps::new(self)?;
        vm_state::goto_label_with(object.cast(), label, &mut ops)
    }

    pub(crate) fn goto_label_or_warn(self, object: *mut UObject, label: i32) -> RuntimeResult<i32> {
        let found = self.goto_label(object, label)?;
        if found == 0 {
            let label_text = self.name_text(label)?;
            let mut warning = "GotoLabel (".chars().map(u32::from).collect::<Vec<_>>();
            warning.extend(label_text);
            warning.extend("): Label not found".chars().map(u32::from));
            self.emit_warning(&warning)?;
        }
        Ok(found)
    }

    pub(crate) fn goto_state_and_label(
        self,
        object: *mut UObject,
        requested: i32,
        label: i32,
        current_before_operands: i32,
    ) -> RuntimeResult<()> {
        let auto_name = self.intern_str("Auto", FindName::Find)?;
        let warning = crate::vm_object::goto_state_and_label_with(
            || CoreStateOps::new(self),
            object.cast(),
            current_before_operands,
            requested,
            label,
            auto_name,
            || self.intern_str("Begin", FindName::Find),
        )?;
        match warning {
            Some(crate::vm_object::StateWarning::MissingLabel { requested, label }) => {
                self.emit_state_warning(requested, label, "Label not found")?;
            }
            Some(crate::vm_object::StateWarning::MissingState { requested, label }) => {
                self.emit_state_warning(requested, label, "State not found")?;
            }
            None => {}
        }
        Ok(())
    }

    fn emit_state_warning(self, requested: i32, label: i32, message: &str) -> RuntimeResult<()> {
        let requested = self.name_text(requested)?;
        let label = self.name_text(label)?;
        let mut text = "GotoState (".chars().map(u32::from).collect::<Vec<_>>();
        text.extend(requested);
        text.push(u32::from(' '));
        text.extend(label);
        text.extend("): ".chars().map(u32::from));
        text.extend(message.chars().map(u32::from));
        self.emit_warning(&text)
    }

    pub(crate) fn set_probe(self, object: *mut UObject, name: i32, mode: ProbeMode) -> RuntimeResult<()> {
        if object.is_null() {
            return Err(RuntimeError::new("Invalid Core state object"));
        }
        let mut ops = CoreProbeOps { access: self };
        crate::vm_object::probe_with(
            &mut ops,
            object.cast(),
            name,
            mode == ProbeMode::Enable,
            PROBE_MIN,
            PROBE_MAX,
        )
    }

    /// Reports whether the object's live state frame is probing an event.
    ///
    /// # Errors
    /// Returns an invalid-object error when `object` is null.
    ///
    /// # Safety
    /// `object` is a live canonical object whose execution frame, if present,
    /// remains live for this call.
    pub unsafe fn is_probing(self, object: *mut UObject, name: i32) -> RuntimeResult<bool> {
        if object.is_null() {
            return Err(RuntimeError::new("Invalid Core state object"));
        }
        if crate::object_instance::probe_bit(name).is_none() {
            return Ok(true);
        }
        let state_frame = frame(object);
        let mask = if state_frame.is_null() {
            None
        } else {
            Some(unsafe { ptr::addr_of!((*state_frame).probe_mask).read() })
        };
        Ok(crate::object_instance::probe_enabled(name, mask))
    }

    /// Executes a named event declared on the object's current state, if present.
    ///
    /// # Errors
    /// Returns reflection or VM execution failures.
    ///
    /// # Safety
    /// `object` and its current state frame remain live through event dispatch.
    pub unsafe fn execute_state_event(
        self,
        object: *mut UObject,
        event: &str,
    ) -> RuntimeResult<()> {
        if object.is_null() {
            return Err(RuntimeError::new("Invalid Core state object"));
        }
        let state = frame_state(object);
        if state.is_null() {
            return Ok(());
        }
        let name = self.intern_str(event, FindName::Find)?;
        let function_class = self.native_class("Function")?;
        let function = unsafe { self.find_field(state.cast(), name, function_class) }
            .cast::<UFunction>();
        if !function.is_null() {
            unsafe {
                self.execute_vm_event(object, function, ptr::null_mut(), ptr::null_mut())?;
            }
        }
        Ok(())
    }

    pub(crate) fn is_in_state(self, object: *mut UObject, requested: i32) -> RuntimeResult<i32> {
        if object.is_null() {
            return Err(RuntimeError::new("Invalid Core state object"));
        }
        let state_frame = frame(object);
        if state_frame.is_null() {
            return Ok(0);
        }
        let mut current = unsafe { ptr::addr_of!((*state_frame).state_node).read() };
        if current.is_null() {
            return Ok(0);
        }
        let state_class = self.native_class("State")?;
        loop {
            if object_name(current.cast()) == requested {
                return Ok(1);
            }
            let parent = unsafe { ptr::addr_of!((*current.cast::<UField>()).super_field).read() };
            if parent.is_null() || !unsafe { crate::object_model::is_a(parent, state_class.cast()) } {
                return Ok(0);
            }
            current = parent.cast();
        }
    }

    pub(crate) fn get_state_name(self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() {
            return Err(RuntimeError::new("Invalid Core state object"));
        }
        let state = frame_state(object);
        if state.is_null() {
            Ok(vm_state::NAME_NONE)
        } else {
            Ok(object_name(state.cast()))
        }
    }
}
