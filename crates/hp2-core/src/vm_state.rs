//! Shared state/label policy over ordinary Rust operations or legacy adapters.

use std::ffi::c_void;
use std::ptr::null_mut;

pub const NAME_NONE: i32 = 0;
pub const STATE_AUTO: u32 = 2;
pub const RF_STATE_CHANGED: u32 = 0x1000;
pub const RF_IN_END_STATE: u32 = 0x2000;
pub const NO_LABEL_TABLE: u16 = 0xffff;
pub const GOTO_NOT_FOUND: i32 = 0;
pub const GOTO_SUCCESS: i32 = 1;
pub const GOTO_PREEMPTED: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmStateServices {
    /// Effective native EName values; registration macro arguments are not IDs.
    pub auto_name: i32,
    pub begin_state_name: i32,
    pub end_state_name: i32,
    pub has_frame: unsafe extern "C" fn(*mut c_void) -> i32,
    pub clear_latent: unsafe extern "C" fn(*mut c_void),
    pub frame_state: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub object_class: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub name: unsafe extern "C" fn(*mut c_void) -> i32,
    pub find_state: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub children: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub next_field: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub inheritance_super: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub is_state: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub state_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub object_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub clear_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    pub set_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    pub probing: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub end_state: unsafe extern "C" fn(*mut c_void) -> i32,
    pub begin_state: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Assign Node, StateNode, then clear Code, on the current live frame.
    pub publish_state: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub probe_mask: unsafe extern "C" fn(*mut c_void) -> u64,
    pub ignore_mask: unsafe extern "C" fn(*mut c_void) -> u64,
    pub set_probe_mask: unsafe extern "C" fn(*mut c_void, u64),
    pub super_state: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub label_table: unsafe extern "C" fn(*mut c_void) -> u16,
    /// Native Script(offset) checks the first byte, then reads one unaligned INT.
    pub script_int: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    /// Construct the legacy FName (including checkSlow) and compare.
    pub label_matches: unsafe extern "C" fn(i32, i32, *mut i32) -> i32,
    pub set_node: unsafe extern "C" fn(*mut c_void, *mut c_void),
    /// Check Script(offset) before assigning Code; Node is already published.
    pub set_code: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub clear_code: unsafe extern "C" fn(*mut c_void),
}

/// Ordinary Rust operations required by state and label policy.
pub(crate) trait StateOps {
    type Error;

    fn auto_name(&self) -> i32;
    fn begin_state_name(&self) -> i32;
    fn end_state_name(&self) -> i32;
    fn has_frame(&mut self, object: *mut c_void) -> Result<bool, Self::Error>;
    fn clear_latent(&mut self, object: *mut c_void) -> Result<(), Self::Error>;
    fn frame_state(&mut self, object: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn object_class(&mut self, object: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn name(&mut self, object: *mut c_void) -> Result<i32, Self::Error>;
    fn find_state(&mut self, object: *mut c_void, name: i32) -> Result<*mut c_void, Self::Error>;
    fn children(&mut self, owner: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn next_field(&mut self, field: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn inheritance_super(&mut self, owner: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn is_state(&mut self, field: *mut c_void) -> Result<bool, Self::Error>;
    fn state_flags(&mut self, state: *mut c_void) -> Result<u32, Self::Error>;
    fn object_flags(&mut self, object: *mut c_void) -> Result<u32, Self::Error>;
    fn clear_flags(&mut self, object: *mut c_void, flags: u32) -> Result<(), Self::Error>;
    fn set_flags(&mut self, object: *mut c_void, flags: u32) -> Result<(), Self::Error>;
    fn probing(&mut self, object: *mut c_void, name: i32) -> Result<bool, Self::Error>;
    fn end_state(&mut self, object: *mut c_void) -> Result<(), Self::Error>;
    fn begin_state(&mut self, object: *mut c_void) -> Result<(), Self::Error>;
    fn publish_state(&mut self, object: *mut c_void, state: *mut c_void) -> Result<(), Self::Error>;
    fn probe_mask(&mut self, state: *mut c_void) -> Result<u64, Self::Error>;
    fn ignore_mask(&mut self, state: *mut c_void) -> Result<u64, Self::Error>;
    fn set_probe_mask(&mut self, object: *mut c_void, mask: u64) -> Result<(), Self::Error>;
    fn super_state(&mut self, state: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn label_table(&mut self, state: *mut c_void) -> Result<u16, Self::Error>;
    fn script_int(&mut self, state: *mut c_void, offset: i32) -> Result<i32, Self::Error>;
    fn label_matches(&mut self, name: i32, label: i32) -> Result<bool, Self::Error>;
    fn set_node(&mut self, object: *mut c_void, state: *mut c_void) -> Result<(), Self::Error>;
    fn set_code(&mut self, object: *mut c_void, state: *mut c_void, offset: i32) -> Result<(), Self::Error>;
    fn clear_code(&mut self, object: *mut c_void) -> Result<(), Self::Error>;
}

fn next_state_field<O: StateOps>(
    ops: &mut O,
    owner: &mut *mut c_void,
    field: &mut *mut c_void,
) -> Result<(), O::Error> {
    while !owner.is_null() {
        while !field.is_null() {
            if ops.is_state(*field)? {
                return Ok(());
            }
            *field = ops.next_field(*field)?;
        }
        *owner = ops.inheritance_super(*owner)?;
        if !owner.is_null() {
            *field = ops.children(*owner)?;
        }
    }
    Ok(())
}

/// Runs the state transition policy against an ordinary-Rust host.
pub(crate) fn goto_state_with<O: StateOps>(
    object: *mut c_void,
    mut new_name: i32,
    ops: &mut O,
) -> Result<i32, O::Error> {
    if !ops.has_frame(object)? {
        return Ok(GOTO_NOT_FOUND);
    }
    ops.clear_latent(object)?;
    let old_state = ops.frame_state(object)?;
    let old_name = if old_state != ops.object_class(object)? {
        ops.name(old_state)?
    } else {
        NAME_NONE
    };
    let mut state = null_mut();
    if new_name != ops.auto_name() {
        state = ops.find_state(object, new_name)?;
    } else {
        let mut owner = ops.object_class(object)?;
        let mut field = if owner.is_null() { null_mut() } else { ops.children(owner)? };
        next_state_field(ops, &mut owner, &mut field)?;
        while !field.is_null() && state.is_null() {
            if ops.state_flags(field)? & STATE_AUTO != 0 {
                state = field;
            }
            // The original for-loop increments even after selecting auto.
            field = ops.next_field(field)?;
            next_state_field(ops, &mut owner, &mut field)?;
        }
    }
    if state.is_null() {
        new_name = NAME_NONE;
        state = ops.object_class(object)?;
    } else if new_name == ops.auto_name() {
        new_name = ops.name(state)?;
    }

    if old_name != NAME_NONE && new_name != old_name {
        if ops.probing(object, ops.end_state_name())? && ops.object_flags(object)? & RF_IN_END_STATE == 0 {
            ops.clear_flags(object, RF_STATE_CHANGED)?;
            ops.set_flags(object, RF_IN_END_STATE)?;
            ops.end_state(object)?;
            ops.clear_flags(object, RF_IN_END_STATE)?;
            if ops.object_flags(object)? & RF_STATE_CHANGED != 0 {
                return Ok(GOTO_PREEMPTED);
            }
        }
    }
    ops.publish_state(object, state)?;
    let class = ops.object_class(object)?;
    let mask = (ops.probe_mask(state)? | ops.probe_mask(class)?) & ops.ignore_mask(state)?;
    ops.set_probe_mask(object, mask)?;
    if new_name != NAME_NONE && new_name != old_name && ops.probing(object, ops.begin_state_name())? {
        ops.clear_flags(object, RF_STATE_CHANGED)?;
        ops.begin_state(object)?;
        if ops.object_flags(object)? & RF_STATE_CHANGED != 0 {
            return Ok(GOTO_PREEMPTED);
        }
    }
    if new_name != NAME_NONE {
        ops.set_flags(object, RF_STATE_CHANGED)?;
        Ok(GOTO_SUCCESS)
    } else {
        Ok(GOTO_NOT_FOUND)
    }
}

/// Runs label lookup and code publication against an ordinary-Rust host.
pub(crate) fn goto_label_with<O: StateOps>(
    object: *mut c_void,
    label: i32,
    ops: &mut O,
) -> Result<i32, O::Error> {
    if !ops.has_frame(object)? {
        return Ok(0);
    }
    ops.clear_latent(object)?;
    if label != NAME_NONE {
        let mut source = ops.frame_state(object)?;
        while !source.is_null() {
            let table = ops.label_table(source)?;
            if table != NO_LABEL_TABLE {
                let mut offset = i32::from(table);
                loop {
                    let name = ops.script_int(source, offset)?;
                    let code = ops.script_int(source, offset.wrapping_add(4))?;
                    if name == NAME_NONE {
                        break;
                    }
                    if ops.label_matches(name, label)? {
                        ops.set_node(object, source)?;
                        ops.set_code(object, source, code)?;
                        return Ok(1);
                    }
                    offset = offset.wrapping_add(8);
                }
            }
            source = ops.super_state(source)?;
        }
    }
    ops.clear_code(object)?;
    Ok(0)
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

impl StateOps for Hp2VmStateServices {
    type Error = i32;

    fn auto_name(&self) -> i32 { self.auto_name }
    fn begin_state_name(&self) -> i32 { self.begin_state_name }
    fn end_state_name(&self) -> i32 { self.end_state_name }
    fn has_frame(&mut self, object: *mut c_void) -> Result<bool, i32> {
        // SAFETY: The legacy adapter contract keeps callbacks live and non-unwinding.
        Ok(unsafe { (self.has_frame)(object) != 0 })
    }
    fn clear_latent(&mut self, object: *mut c_void) -> Result<(), i32> {
        unsafe { (self.clear_latent)(object) }; Ok(())
    }
    fn frame_state(&mut self, object: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (self.frame_state)(object) })
    }
    fn object_class(&mut self, object: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (self.object_class)(object) })
    }
    fn name(&mut self, object: *mut c_void) -> Result<i32, i32> { Ok(unsafe { (self.name)(object) }) }
    fn find_state(&mut self, object: *mut c_void, name: i32) -> Result<*mut c_void, i32> {
        let mut result = null_mut(); status(unsafe { (self.find_state)(object, name, &mut result) })?; Ok(result)
    }
    fn children(&mut self, owner: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (self.children)(owner) })
    }
    fn next_field(&mut self, field: *mut c_void) -> Result<*mut c_void, i32> {
        Ok(unsafe { (self.next_field)(field) })
    }
    fn inheritance_super(&mut self, owner: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = owner; status(unsafe { (self.inheritance_super)(owner, &mut result) })?; Ok(result)
    }
    fn is_state(&mut self, field: *mut c_void) -> Result<bool, i32> {
        let mut result = 0; status(unsafe { (self.is_state)(field, &mut result) })?; Ok(result != 0)
    }
    fn state_flags(&mut self, state: *mut c_void) -> Result<u32, i32> { Ok(unsafe { (self.state_flags)(state) }) }
    fn object_flags(&mut self, object: *mut c_void) -> Result<u32, i32> { Ok(unsafe { (self.object_flags)(object) }) }
    fn clear_flags(&mut self, object: *mut c_void, flags: u32) -> Result<(), i32> {
        status(unsafe { (self.clear_flags)(object, flags) })
    }
    fn set_flags(&mut self, object: *mut c_void, flags: u32) -> Result<(), i32> {
        status(unsafe { (self.set_flags)(object, flags) })
    }
    fn probing(&mut self, object: *mut c_void, name: i32) -> Result<bool, i32> {
        let mut result = 0; status(unsafe { (self.probing)(object, name, &mut result) })?; Ok(result != 0)
    }
    fn end_state(&mut self, object: *mut c_void) -> Result<(), i32> { status(unsafe { (self.end_state)(object) }) }
    fn begin_state(&mut self, object: *mut c_void) -> Result<(), i32> { status(unsafe { (self.begin_state)(object) }) }
    fn publish_state(&mut self, object: *mut c_void, state: *mut c_void) -> Result<(), i32> {
        unsafe { (self.publish_state)(object, state) }; Ok(())
    }
    fn probe_mask(&mut self, state: *mut c_void) -> Result<u64, i32> { Ok(unsafe { (self.probe_mask)(state) }) }
    fn ignore_mask(&mut self, state: *mut c_void) -> Result<u64, i32> { Ok(unsafe { (self.ignore_mask)(state) }) }
    fn set_probe_mask(&mut self, object: *mut c_void, mask: u64) -> Result<(), i32> {
        unsafe { (self.set_probe_mask)(object, mask) }; Ok(())
    }
    fn super_state(&mut self, state: *mut c_void) -> Result<*mut c_void, i32> {
        let mut result = state; status(unsafe { (self.super_state)(state, &mut result) })?; Ok(result)
    }
    fn label_table(&mut self, state: *mut c_void) -> Result<u16, i32> { Ok(unsafe { (self.label_table)(state) }) }
    fn script_int(&mut self, state: *mut c_void, offset: i32) -> Result<i32, i32> {
        let mut result = 0; status(unsafe { (self.script_int)(state, offset, &mut result) })?; Ok(result)
    }
    fn label_matches(&mut self, name: i32, label: i32) -> Result<bool, i32> {
        let mut result = 0; status(unsafe { (self.label_matches)(name, label, &mut result) })?; Ok(result != 0)
    }
    fn set_node(&mut self, object: *mut c_void, state: *mut c_void) -> Result<(), i32> {
        unsafe { (self.set_node)(object, state) }; Ok(())
    }
    fn set_code(&mut self, object: *mut c_void, state: *mut c_void, offset: i32) -> Result<(), i32> {
        status(unsafe { (self.set_code)(object, state, offset) })
    }
    fn clear_code(&mut self, object: *mut c_void) -> Result<(), i32> {
        unsafe { (self.clear_code)(object) }; Ok(())
    }
}

/// # Safety
/// All callbacks/storage remain valid during synchronous, possibly reentrant notifications.
pub unsafe fn goto_state(object: *mut c_void, new_name: i32, services: &Hp2VmStateServices) -> Result<i32, i32> {
    let mut ops = *services;
    goto_state_with(object, new_name, &mut ops)
}

/// # Safety
/// The script storage, callbacks, and live frame satisfy the legacy service contract.
pub unsafe fn goto_label(object: *mut c_void, label: i32, services: &Hp2VmStateServices) -> Result<i32, i32> {
    let mut ops = *services;
    goto_label_with(object, label, &mut ops)
}

