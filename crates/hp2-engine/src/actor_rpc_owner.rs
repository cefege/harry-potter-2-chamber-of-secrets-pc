//! Canonical CoreAccess services for Actor remote-function routing.

use crate::actor_rpc::{self, Hp2ActorRpcMode, Hp2ActorRpcServices, Hp2ActorRpcSnapshot};
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APlayerPawn, FFrame, UClass, UField, UFunction, ULevel,
    UNetConnection, UNetDriver, UObject, UStruct,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::c_void, ptr};

const FUNC_NET: u32 = 0x40;
const FUNC_NET_RELIABLE: u32 = 0x80;
const FUNC_SIMULATED: u32 = 0x100;
const FUNC_STATIC: u32 = 0x2000;

struct Context<'a> {
    access: CoreAccess<'a>,
    actor: *mut AActor,
    parameters: *mut c_void,
    stack: *mut FFrame,
    error: Option<RuntimeError>,
}

fn fail(context: &mut Context<'_>, error: RuntimeError) -> i32 {
    if context.error.is_none() { context.error = Some(error); }
    1
}


unsafe extern "C" fn snapshot(raw: *mut c_void, function: *mut c_void, out: *mut Hp2ActorRpcSnapshot) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if function.is_null() || out.is_null() { return fail(context, RuntimeError::new("Invalid Actor RPC snapshot arguments")); }
    let actor = unsafe { &*context.actor };
    let flags = unsafe { (*function.cast::<UFunction>()).function_flags };
    unsafe { out.write(Hp2ActorRpcSnapshot {
        is_static: i32::from(flags & FUNC_STATIC != 0), is_net: i32::from(flags & FUNC_NET != 0),
        is_simulated: i32::from(flags & FUNC_SIMULATED != 0), is_reliable: i32::from(flags & FUNC_NET_RELIABLE != 0),
        deleted: i32::from(actor.b_delete_me()), net_temporary: i32::from(actor.b_net_temporary()),
        authority: i32::from(actor.role ==  ROLE_AUTHORITY), simulated_proxy_or_less: i32::from(actor.role <= ROLE_SIMULATED_PROXY),
    }); }
    0
}

const ROLE_AUTHORITY: u8 = 4;
const ROLE_SIMULATED_PROXY: u8 = 2;

unsafe extern "C" fn demo_driver(raw: *mut c_void, present: *mut i32, playback: *mut i32) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if context.actor.is_null() { return fail(context, RuntimeError::new("Null Actor in demo-driver lookup")); }
    let level = unsafe { (*context.actor).x_level.cast::<ULevel>() };
    if level.is_null() { return fail(context, RuntimeError::new("Actor has no XLevel for demo-driver lookup")); }
    let driver = unsafe { (*level).base.demo_rec_driver.cast::<UNetDriver>() };
    unsafe { present.write(i32::from(!driver.is_null())); playback.write(i32::from(!driver.is_null() && !(*driver).server_connection.is_null())); }
    0
}

unsafe extern "C" fn record_demo(raw: *mut c_void, function: *mut c_void) -> i32 {
    let mut function = function;
    match unsafe { actor_rpc::process_demo(&services(raw), &mut function) } {
        Ok(()) => 0,
        Err(code) => unsafe {
            fail(
                &mut *raw.cast::<Context<'_>>(),
                RuntimeError::new("Actor demo RPC service failed").with_context(&code.to_string()),
            )
        },
    }
}

unsafe extern "C" fn mode(raw: *mut c_void, demo: i32, out: *mut Hp2ActorRpcMode) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if context.actor.is_null() || out.is_null() { return fail(context, RuntimeError::new("Invalid Actor net-mode query")); }
    let actor = unsafe { &*context.actor };
    let mode = if demo != 0 {
        let level = actor.x_level.cast::<ULevel>();
        if level.is_null() { return fail(context, RuntimeError::new("Actor has no XLevel for demo net-mode query")); }
        let actors = unsafe { (*level).base.actors };
        if actors.array_num <= 0 || actors.data.is_null() {
            return fail(context, RuntimeError::new("Actor XLevel has no level-info actor"));
        }
        let info = unsafe { (*actors.data.cast::<*mut AActor>()).cast::<ALevelInfo>() };
        if info.is_null() { return fail(context, RuntimeError::new("Actor XLevel actor zero is null")); }
        unsafe { (*info).net_mode }
    } else {
        let level = actor.level.cast::<ALevelInfo>();
        if level.is_null() { return fail(context, RuntimeError::new("Actor has no LevelInfo for net-mode query")); }
        unsafe { (*level).net_mode }
    };
    unsafe { out.write(Hp2ActorRpcMode { standalone: i32::from(mode == 0), client: i32::from(mode == 3), server: i32::from(mode == 1 || mode == 2) }); }
    0
}

unsafe extern "C" fn top_owner(raw: *mut c_void, out: *mut *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if context.actor.is_null() || out.is_null() { return fail(context, RuntimeError::new("Invalid Actor top-owner query")); }
    let last = unsafe { crate::actor::get_top_owner(context.actor) };
    let class = unsafe { (*last.cast::<UObject>()).class };
    let player_class = match context.access.native_class("Engine.PlayerPawn") {
        Ok(class) => class,
        Err(error) => return fail(context, error),
    };
    unsafe { out.write(if !class.is_null() && hp2_core::object_model::is_a(last.cast(), player_class.cast()) { last.cast() } else { ptr::null_mut() }); }
    0
}

unsafe extern "C" fn client_connection(raw: *mut c_void, top: *mut c_void, out: *mut *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if top.is_null() || out.is_null() { return fail(context, RuntimeError::new("Invalid Actor RPC client connection")); }
    let connection_class = match context.access.native_class("Engine.NetConnection") {
        Ok(class) => class,
        Err(error) => return fail(context, error),
    };
    let player = unsafe { (*top.cast::<APlayerPawn>()).player.cast::<UObject>() };
    let connection = if !player.is_null()
        && unsafe { hp2_core::object_model::is_a(player.cast(), connection_class.cast()) }
    {
        player.cast()
    } else {
        ptr::null_mut()
    };
    unsafe { out.write(connection); }
    0
}

unsafe extern "C" fn super_function(raw: *mut c_void, function: *mut c_void, out: *mut *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if function.is_null() || out.is_null() { return fail(context, RuntimeError::new("Invalid Actor RPC super-function query")); }
    unsafe { out.write((*function.cast::<UField>()).super_field); }
    0
}

unsafe extern "C" fn condition(raw: *mut c_void, function: *mut c_void, out: *mut i32) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if function.is_null() || out.is_null() { return fail(context, RuntimeError::new("Invalid Actor replication condition")); }
    let function = function.cast::<UFunction>();
    let class_class = match context.access.native_class("Core.Class") {
        Ok(class) => class,
        Err(error) => return fail(context, error),
    };
    let mut owner = function.cast::<UObject>();
    while !owner.is_null()
        && !unsafe { hp2_core::object_model::is_a(owner.cast(), class_class.cast()) }
    {
        owner = unsafe { (*owner).outer.cast() };
    }
    if owner.is_null() {
        return fail(context, RuntimeError::new("RPC function has no owning class"));
    }
    let class = owner.cast::<UClass>();
    let offset = unsafe { (*function).rep_offset } as i32;
    match unsafe { context.access.eval_rep_condition(context.actor.cast(), class, offset, ptr::null_mut()) } {
        Ok(value) => { unsafe { out.write(value as i32); } 0 }
        Err(error) => fail(context, error),
    }
}

unsafe extern "C" fn server_connection(raw: *mut c_void, out: *mut *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    let level = unsafe { (*context.actor).x_level.cast::<ULevel>() };
    if level.is_null() { return fail(context, RuntimeError::new("Actor has no XLevel for server connection")); }
    let driver = unsafe { (*level).base.net_driver.cast::<UNetDriver>() };
    unsafe { out.write(if driver.is_null() { ptr::null_mut() } else { (*driver).server_connection }); }
    0
}

unsafe extern "C" fn demo_connection(raw: *mut c_void, out: *mut *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    let level = unsafe { (*context.actor).x_level.cast::<ULevel>() };
    if level.is_null() { return fail(context, RuntimeError::new("Actor has no XLevel for demo connection")); }
    let driver = unsafe { (*level).base.demo_rec_driver.cast::<UNetDriver>() };
    if driver.is_null() { return fail(context, RuntimeError::new("Actor has no demo recorder")); }
    let connections = unsafe { (*driver).client_connections };
    if connections.num <= 0 || connections.data.is_null() { unsafe { out.write(ptr::null_mut()); } return 0; }
    unsafe { out.write(*connections.data.cast::<*mut c_void>()); }
    0
}

unsafe extern "C" fn check_connection(raw: *mut c_void, connection: *mut c_void) -> i32 {
    if connection.is_null() { return unsafe { fail(&mut *raw.cast::<Context<'_>>(), RuntimeError::new("Actor RPC connection is null")) }; }
    0
}

unsafe extern "C" fn net_ready(raw: *mut c_void, connection: *mut c_void, out: *mut i32) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if connection.is_null() { return fail(context, RuntimeError::new("Actor RPC connection is null")); }
    match unsafe { crate::connection_owner::is_net_ready(context.access, connection.cast::<UNetConnection>(), 0) } {
        Ok(value) => { unsafe { out.write(value); } 0 }, Err(error) => fail(context, error),
    }
}

unsafe extern "C" fn send(raw: *mut c_void, connection: *mut c_void, function: *mut c_void, server: i32) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    match unsafe { send_remote(context, connection.cast(), function.cast(), server != 0) } {
        Ok(()) => 0, Err(error) => fail(context, error),
    }
}

unsafe extern "C" fn exchange_roles(raw: *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if context.actor.is_null() { return fail(context, RuntimeError::new("Null Actor in demo role exchange")); }
    unsafe { let actor = &mut *context.actor; std::mem::swap(&mut actor.role, &mut actor.remote_role); }
    0
}
unsafe extern "C" fn demo_flags(raw: *mut c_void, recording: i32, client: i32) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if context.actor.is_null() { return fail(context, RuntimeError::new("Null Actor in demo recording flags")); }
    unsafe { (*context.actor).set_b_demo_recording(recording != 0); (*context.actor).set_b_client_demo_recording(client != 0); }
    0
}
unsafe extern "C" fn reset_client_demo_net_func(raw: *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if context.actor.is_null() { return fail(context, RuntimeError::new("Null Actor in demo RPC reset")); }
    unsafe { (*context.actor).set_b_client_demo_net_func(false); }
    0
}
unsafe extern "C" fn stack_code(raw: *mut c_void, out: *mut *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    unsafe { out.write(if context.stack.is_null() { ptr::null_mut() } else { (*context.stack).code.cast() }); }
    0
}
unsafe extern "C" fn restore_stack_code(raw: *mut c_void, code: *mut c_void) -> i32 {
    let context = unsafe { &mut *raw.cast::<Context<'_>>() };
    if !context.stack.is_null() { unsafe { (*context.stack).code = code.cast(); } }
    0
}

fn services(context: *mut c_void) -> Hp2ActorRpcServices {
    Hp2ActorRpcServices { context, snapshot, demo_driver, record_demo, mode, top_owner, client_connection, super_function, condition, server_connection, demo_connection, check_connection, net_ready, send, exchange_roles, demo_flags, reset_client_demo_net_func, stack_code, restore_stack_code }
}

pub(crate) unsafe fn process_remote(access: CoreAccess<'_>, actor: *mut AActor, function: *mut UFunction, parameters: *mut c_void, stack: *mut FFrame) -> RuntimeResult<bool> {
    if actor.is_null() || function.is_null() { return Err(RuntimeError::new("Invalid Actor remote-function arguments")); }
    let mut context = Context { access, actor, parameters, stack, error: None };
    let service = services((&mut context as *mut Context<'_>).cast());
    let mut function = function.cast::<c_void>();
    match unsafe { actor_rpc::process_remote(&service, &mut function) } {
        Ok(value) => Ok(value != 0),
        Err(code) => Err(context.error.unwrap_or_else(|| RuntimeError::new("Actor RPC operation failed").with_context(&code.to_string()))),
    }
}

unsafe fn send_remote(
    context: &mut Context<'_>,
    connection: *mut UNetConnection,
    function: *mut UFunction,
    server: bool,
) -> RuntimeResult<()> {
    unsafe {
        crate::actor_channel_owner::send_remote_function(
            context.access,
            context.actor,
            connection,
            function,
            context.parameters,
            context.stack,
            server,
        )
    }
}
