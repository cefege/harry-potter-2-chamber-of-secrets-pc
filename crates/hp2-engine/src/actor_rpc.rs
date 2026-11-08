//! Actor RPC routing and demo-recording policy. C++ owns engine metadata,
//! script bytecode evaluation and the single packet-serialization operation.
//! Failures stop immediately: legacy code does not unwind demo flags, roles or
//! Stack->Code after a throwing script/serializer call.

use core::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2ActorRpcSnapshot {
    pub is_static: i32,
    pub is_net: i32,
    pub is_simulated: i32,
    pub is_reliable: i32,
    pub deleted: i32,
    pub net_temporary: i32,
    pub authority: i32,
    pub simulated_proxy_or_less: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2ActorRpcMode {
    pub standalone: i32,
    pub client: i32,
    pub server: i32,
}

#[repr(C)]
pub struct Hp2ActorRpcServices {
    pub context: *mut c_void,
    pub snapshot: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Hp2ActorRpcSnapshot) -> i32,
    /// Returns driver presence and whether it has a server connection (playback).
    pub demo_driver: unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32) -> i32,
    /// Guard-only bridge to the demo method, which reenters `process_demo`.
    pub record_demo: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Demo uses GetLevel()->GetLevelInfo(); ordinary RPC uses Actor->Level.
    pub mode: unsafe extern "C" fn(*mut c_void, i32, *mut Hp2ActorRpcMode) -> i32,
    pub top_owner: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub client_connection: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub super_function: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub condition: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub server_connection: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_connection: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub check_connection: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub net_ready: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub send: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, i32) -> i32,
    pub exchange_roles: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_flags: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    pub reset_client_demo_net_func: unsafe extern "C" fn(*mut c_void) -> i32,
    pub stack_code: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub restore_stack_code: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// Update the caller's function slot as traversal progresses, preserving the
/// function named by the C++ guard if a later operation throws.
unsafe fn resolve(s: &Hp2ActorRpcServices, function: &mut *mut c_void) -> Result<(), i32> {
    loop {
        let mut parent = core::ptr::null_mut();
        status(unsafe { (s.super_function)(s.context, *function, &mut parent) })?;
        if parent.is_null() {
            return Ok(());
        }
        *function = parent;
    }
}

/// # Safety
/// Services and engine handles must remain valid for the synchronous call.
/// `function` is also the C++ diagnostic guard's current function slot.
pub unsafe fn process_remote(
    s: &Hp2ActorRpcServices,
    function: &mut *mut c_void,
) -> Result<i32, i32> {
    unsafe {
        let mut state = Hp2ActorRpcSnapshot::default();
        status((s.snapshot)(s.context, *function, &mut state))?;
        if state.is_static != 0 || state.deleted != 0 {
            return Ok(0);
        }
        let absorb = i32::from(state.simulated_proxy_or_less != 0 && state.is_simulated == 0);
        let (mut demo, mut playback) = (0, 0);
        status((s.demo_driver)(s.context, &mut demo, &mut playback))?;
        if demo != 0 {
            if playback != 0 {
                return Ok(absorb);
            }
            status((s.record_demo)(s.context, *function))?;
        }
        let mut mode = Hp2ActorRpcMode::default();
        status((s.mode)(s.context, 0, &mut mode))?;
        if mode.standalone != 0 {
            return Ok(0);
        }
        // Demo script evaluation can change actor state. Absorb remains the
        // value computed before demo dispatch, while subsequent reads are fresh.
        status((s.snapshot)(s.context, *function, &mut state))?;
        if state.is_net == 0 {
            return Ok(absorb);
        }
        let mut top = core::ptr::null_mut();
        status((s.top_owner)(s.context, &mut top))?;
        let mut connection = core::ptr::null_mut();
        if state.authority != 0 {
            if top.is_null() {
                return Ok(absorb);
            }
            status((s.client_connection)(s.context, top, &mut connection))?;
            if connection.is_null() {
                return Ok(absorb);
            }
        }
        resolve(s, function)?;
        let mut condition = 0;
        status((s.condition)(s.context, *function, &mut condition))?;
        if condition == 0 {
            return Ok(absorb);
        }
        status((s.mode)(s.context, 0, &mut mode))?;
        if mode.server == 0 {
            status((s.server_connection)(s.context, &mut connection))?;
        }
        status((s.check_connection)(s.context, connection))?;
        status((s.snapshot)(s.context, *function, &mut state))?;
        if state.is_reliable == 0 {
            let mut ready = 0;
            status((s.net_ready)(s.context, connection, &mut ready))?;
            if ready == 0 {
                return Ok(1);
            }
        }
        status((s.send)(s.context, connection, *function, mode.server))?;
        Ok(1)
    }
}

/// # Safety
/// Same contract as `process_remote`. Stack access services handle a null stack.
pub unsafe fn process_demo(s: &Hp2ActorRpcServices, function: &mut *mut c_void) -> Result<(), i32> {
    unsafe {
        let mut state = Hp2ActorRpcSnapshot::default();
        status((s.snapshot)(s.context, *function, &mut state))?;
        if state.is_static != 0 || state.is_net == 0 || state.net_temporary != 0 {
            return Ok(());
        }
        let mut mode = Hp2ActorRpcMode::default();
        status((s.mode)(s.context, 1, &mut mode))?;
        if mode.client != 0 && state.authority != 0 {
            return Ok(());
        }
        resolve(s, function)?;
        if mode.client != 0 {
            status((s.exchange_roles)(s.context))?;
        }
        status((s.demo_flags)(s.context, 1, mode.client))?;
        let mut condition = 0;
        status((s.condition)(s.context, *function, &mut condition))?;
        status((s.demo_flags)(s.context, 0, 0))?;
        if mode.client != 0 {
            status((s.exchange_roles)(s.context))?;
        }
        status((s.reset_client_demo_net_func)(s.context))?;
        if condition == 0 {
            return Ok(());
        }
        let mut connection = core::ptr::null_mut();
        status((s.demo_connection)(s.context, &mut connection))?;
        status((s.check_connection)(s.context, connection))?;
        let mut saved_code = core::ptr::null_mut();
        status((s.stack_code)(s.context, &mut saved_code))?;
        status((s.send)(s.context, connection, *function, 1))?;
        status((s.restore_stack_code)(s.context, saved_code))?;
        Ok(())
    }
}
