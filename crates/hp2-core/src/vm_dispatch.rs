//! Script function/event policy over live frames. Rust owns event frames and
//! local bytes; native continuations retain trace RAII. Errors deliberately leave
//! counters, flags and locals exactly where the failing operation stopped.

use crate::vm_frame::{self, Hp2VmFrameInitializationServices};
use crate::vm_registry::Hp2VmNativeHandler;
use hp2_abi::engine::generated::FFrame;
use std::ffi::c_void;
use std::mem::MaybeUninit;

pub const FUNC_SINGULAR: u32 = 0x20;
pub const FUNC_NATIVE: u32 = 0x400;
pub const EX_RETURN: u8 = 0x04;
pub const EX_DEBUG_INFO: u8 = 0x38;

#[repr(C)]
pub struct Hp2VmInternalServices {
    pub function_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub remote: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub probing: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub object_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub set_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    pub clear_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    pub trace_enter: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Zero sizeof(FString), not the entire native 1024-byte scratch buffer.
    pub initialize_buffer: unsafe extern "C" fn(*mut c_void) -> i32,
    pub recursion_error: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub opcode: unsafe extern "C" fn(*mut c_void) -> u8,
    pub advance: unsafe extern "C" fn(*mut c_void),
    /// Zero selects scratch storage; one selects the caller's result address.
    pub step: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub trace_return: unsafe extern "C" fn(*mut c_void) -> i32,
    pub trace_exit: unsafe extern "C" fn(*mut c_void) -> i32,
}

fn status(code: i32) -> Result<(), i32> {
    if code == 0 { Ok(()) } else { Err(code) }
}

pub(crate) enum InternalEntryQuery {
    RemoteConsumed,
    Probing,
    SingularActive,
}

pub(crate) fn accept_internal_with<E>(
    mut query: impl FnMut(InternalEntryQuery) -> Result<bool, E>,
) -> Result<bool, E> {
    use InternalEntryQuery::*;
    Ok(!query(RemoteConsumed)? && query(Probing)? && !query(SingularActive)?)
}

/// # Safety
/// Context and callbacks must remain valid through reentry. Nonzero guard_enabled
/// selects DO_GUARD and requires serialized access to the process-wide VM guards.
/// No callback may unwind across the ABI.
pub unsafe fn process_internal(
    context: *mut c_void,
    guard_enabled: u32,
    services: &Hp2VmInternalServices,
) -> Result<(), i32> {
    unsafe {
        let singular = (services.function_flags)(context) & FUNC_SINGULAR;
        let mut value = 0;
        let accepted = accept_internal_with::<i32>(|query| {
            use InternalEntryQuery::*;
            match query {
                RemoteConsumed => { status((services.remote)(context, &mut value))?; Ok(value != 0) }
                Probing => { status((services.probing)(context, &mut value))?; Ok(value != 0) }
                SingularActive => Ok((services.object_flags)(context) & singular != 0),
            }
        })?;
        if !accepted { return Ok(()); }
        status((services.trace_enter)(context))?;
        status((services.set_flags)(context, singular))?;
        status((services.initialize_buffer)(context))?;
        if guard_enabled != 0 && crate::vm_guard::enter_recursion() {
            status((services.recursion_error)(
                context,
                crate::vm_guard::HP2_VM_RECURSE_LIMIT,
            ))?;
        }
        while (services.opcode)(context) != EX_RETURN {
            status((services.step)(context, 0))?;
        }
        (services.advance)(context);
        status((services.step)(context, 1))?;
        status((services.trace_return)(context))?;
        if (services.opcode)(context) == EX_DEBUG_INFO {
            status((services.step)(context, 1))?;
        }
        status((services.trace_exit)(context))?;
        status((services.clear_flags)(context, singular))?;
        if guard_enabled != 0 {
            crate::vm_guard::leave_recursion();
        }
        Ok(())
    }
}

#[repr(C)]
pub struct Hp2VmEventServices {
    pub scriptable: unsafe extern "C" fn(*mut c_void) -> i32,
    pub probing: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub pending_kill: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub native_index: unsafe extern "C" fn(*mut c_void) -> i32,
    pub function_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub remote: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub check_parameters: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Establish the outer trace scope, then call event_accepted synchronously.
    pub outer_scope: unsafe extern "C" fn(*mut c_void) -> i32,
    pub entry_tag: unsafe extern "C" fn(*mut c_void) -> *mut i32,
    pub clock: unsafe extern "C" fn(*mut c_void) -> i32,
    pub unclock: unsafe extern "C" fn(*mut c_void) -> i32,
    pub locals_size: unsafe extern "C" fn(*mut c_void) -> i32,
    pub parameters: unsafe extern "C" fn(*mut c_void) -> *mut u8,
    /// Return the current checked Function->Func (native or ProcessInternal).
    pub native_handler: unsafe extern "C" fn(*mut c_void) -> Hp2VmNativeHandler,
    pub return_value_offset: unsafe extern "C" fn(*mut c_void) -> i32,
    pub first_constructor: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub next_constructor: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub property_offset: unsafe extern "C" fn(*mut c_void) -> i32,
    pub parameters_size: unsafe extern "C" fn(*mut c_void) -> i32,
    pub destroy: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub frame_object: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub frame_node: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    /// Publish the fully initialized Rust-owned frame for this continuation only.
    pub publish_frame: unsafe extern "C" fn(*mut c_void, *mut FFrame),
    pub frame_initialization: *const Hp2VmFrameInitializationServices,
}

/// Source-ordered ProcessEvent admission queries. Ineligible events do not
/// inspect later fields or invoke remote routing.
pub(crate) enum EventEntryQuery {
    Scriptable,
    Probing,
    PendingKill,
    IndexedNative,
    NativeFunction,
    RemoteConsumed,
}

pub(crate) fn accept_event_with<E>(
    mut query: impl FnMut(EventEntryQuery) -> Result<bool, E>,
) -> Result<bool, E> {
    use EventEntryQuery::*;
    if !query(Scriptable)? || !query(Probing)? || query(PendingKill)? || query(IndexedNative)? {
        return Ok(false);
    }
    if query(NativeFunction)? && query(RemoteConsumed)? {
        return Ok(false);
    }
    Ok(true)
}

/// # Safety
/// Context and live object/function state must remain valid through callbacks.
/// Native scopes must invoke their Rust continuations synchronously, catching
/// native exceptions and preserving the first error status.
pub unsafe fn process_event(
    context: *mut c_void,
    services: &Hp2VmEventServices,
) -> Result<(), i32> {
    unsafe {
        let mut value = 0;
        let accepted = accept_event_with::<i32>(|query| {
            use EventEntryQuery::*;
            match query {
                Scriptable => Ok((services.scriptable)(context) != 0),
                Probing => { status((services.probing)(context, &mut value))?; Ok(value != 0) }
                PendingKill => { status((services.pending_kill)(context, &mut value))?; Ok(value != 0) }
                IndexedNative => Ok((services.native_index)(context) != 0),
                NativeFunction => Ok((services.function_flags)(context) & FUNC_NATIVE != 0),
                RemoteConsumed => { status((services.remote)(context, &mut value))?; Ok(value != 0) }
            }
        })?;
        if !accepted { return Ok(()); }
        status((services.check_parameters)(context))?;
        status((services.outer_scope)(context))
    }
}

/// # Safety
/// Called only inside the accepted event's native outer trace scope. The tag
/// points to the shared live counter, including during nested dispatch. Sizes must
/// describe valid local/parameter storage; metadata getters must be nonthrowing
/// and nonreentrant. Parameters must not overlap the owned locals. An unused
/// return offset may be a legacy void sentinel; handlers must not dereference it.
pub unsafe fn event_accepted(
    context: *mut c_void,
    services: &Hp2VmEventServices,
) -> Result<(), i32> {
    unsafe {
        let tag = (services.entry_tag)(context);
        *tag = (*tag).wrapping_add(1);
        if *tag == 1 {
            status((services.clock)(context))?;
        }
        let size = (services.locals_size)(context) as usize;
        crate::vm_locals::with_locals(size, |locals| event_frame(context, locals, services))
    }
}

/// # Safety
/// Called synchronously inside the Rust local-storage owner for this event.
/// frame_initialization must point to a valid shared initializer service table;
/// object/node getters must return live metadata and must not unwind. The checked
/// script-code callback must preserve the first error and may reenter the VM.
/// Only a successfully initialized frame is published. Context and callbacks may
/// use its pointer only until this function returns, including on failure.
/// No Rust reference to the frame or locals is held across callbacks. Constructor
/// links and metadata are read live, including after destructors; callbacks must
/// not unwind or retain pointers to this frame or its locals.
unsafe fn event_frame(
    context: *mut c_void,
    locals: *mut u8,
    services: &Hp2VmEventServices,
) -> Result<(), i32> {
    unsafe {
        let mut frame = MaybeUninit::<FFrame>::uninit();
        let object = (services.frame_object)(context);
        let node = (services.frame_node)(context);
        vm_frame::initialize(
            frame.as_mut_ptr(),
            object,
            node,
            0,
            locals,
            std::ptr::null_mut(),
            &*services.frame_initialization,
        )?;
        (services.publish_frame)(context, frame.as_mut_ptr());
        let parameters_size = (services.parameters_size)(context) as usize;
        let parameters = (services.parameters)(context);
        let frame_locals = std::ptr::addr_of!((*frame.as_ptr()).locals).read();
        if parameters_size != 0 {
            std::ptr::copy_nonoverlapping(parameters, frame_locals, parameters_size);
        }
        let locals_size = (services.locals_size)(context) as usize;
        let parameters_size = (services.parameters_size)(context) as usize;
        let frame_locals = std::ptr::addr_of!((*frame.as_ptr()).locals).read();
        let trailing_size = locals_size - parameters_size;
        if trailing_size != 0 {
            std::ptr::write_bytes(frame_locals.add(parameters_size), 0, trailing_size);
        }
        let handler = (services.native_handler)(context);
        let object = (services.frame_object)(context);
        let return_offset = (services.return_value_offset)(context) as isize;
        let frame_locals = std::ptr::addr_of!((*frame.as_ptr()).locals).read();
        status(handler(
            object,
            frame.as_mut_ptr().cast(),
            frame_locals.wrapping_offset(return_offset).cast(),
        ))?;
        let frame_locals = std::ptr::addr_of!((*frame.as_ptr()).locals).read();
        let parameters = (services.parameters)(context);
        let parameters_size = (services.parameters_size)(context) as usize;
        if parameters_size != 0 {
            std::ptr::copy_nonoverlapping(frame_locals, parameters, parameters_size);
        }
        let mut property = (services.first_constructor)(context);
        while !property.is_null() {
            if (services.property_offset)(property) >= (services.parameters_size)(context) {
                status((services.destroy)(context, property))?;
            }
            property = (services.next_constructor)(property);
        }
        let tag = (services.entry_tag)(context);
        *tag = (*tag).wrapping_sub(1);
        if *tag == 0 {
            status((services.unclock)(context))?;
        }
        Ok(())
    }
}
