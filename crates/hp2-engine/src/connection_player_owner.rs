//! Shared connection player handoff with typed native and canonical operations.

use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, UClient, UEngine, ULevel, UNetConnection, UViewport};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::{CStr, c_void}, ptr};

pub(crate) trait ClientPlayerOps {
    type Error;
    fn check(&mut self, valid: bool, expression: &CStr, file: &CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn client(&mut self, pawn: *mut APlayerPawn) -> Result<*mut c_void, Self::Error>;
    unsafe fn viewport_count(&mut self, client: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn viewport0(&mut self, client: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn clear_old_player(&mut self, viewport: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn net_speed(&mut self, viewport: *mut c_void, speed: i32) -> Result<(), Self::Error>;
    unsafe fn role(&mut self, pawn: *mut APlayerPawn, role: u8) -> Result<(), Self::Error>;
    unsafe fn show_flags(&mut self, pawn: *mut APlayerPawn, flags: u32) -> Result<(), Self::Error>;
    unsafe fn rend_map(&mut self, pawn: *mut APlayerPawn, map: i32) -> Result<(), Self::Error>;
    unsafe fn set_player(&mut self, pawn: *mut APlayerPawn, viewport: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn reset_input(&mut self, pawn: *mut APlayerPawn) -> Result<(), Self::Error>;
    unsafe fn level_action(&mut self, pawn: *mut APlayerPawn, action: u8) -> Result<(), Self::Error>;
}

/// Runs the base UNetConnection policy, including its source-order checks.
///
/// # Safety
/// All records and the client/viewport graph remain live through adapter calls.
pub(crate) unsafe fn handle_base_with<O: ClientPlayerOps>(
    ops: &mut O, connection: *mut UNetConnection, pawn: *mut APlayerPawn,
) -> Result<(), O::Error> {
    unsafe {
        let client = ops.client(pawn)?;
        ops.check(!client.is_null(), c"Pawn->GetLevel()->Engine->Client", c"UnConn.cpp", 907)?;
        let count = ops.viewport_count(client)?;
        ops.check(count != 0, c"Pawn->GetLevel()->Engine->Client->Viewports.Num()", c"UnConn.cpp", 908)?;
        let viewport = ops.viewport0(client)?;
        ops.clear_old_player(viewport)?;
        ops.net_speed(viewport, (*connection).current_net_speed)?;
        ops.role(pawn, 3)?;
        ops.show_flags(pawn, 0x480c)?;
        ops.rend_map(pawn, 5)?;
        ops.set_player(pawn, viewport)?;
        // Possess may mutate the graph. The operation reacquires the current
        // client/viewport rather than retaining the pre-event viewport.
        ops.reset_input(pawn)?;
        ops.level_action(pawn, 0)?;
        ops.check((*connection).state == 2, c"State==USOCK_Pending", c"UnConn.cpp", 924)?;
        (*connection).state = 3;
        (*connection).actor = pawn.cast();
    }
    Ok(())
}

struct NativeClientPlayerOps<'a>(&'a crate::net_conn_abi::Hp2NetConnServices);
fn native_status(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}
impl ClientPlayerOps for NativeClientPlayerOps<'_> {
    type Error = i32;
    fn check(&mut self, valid: bool, expression: &CStr, file: &CStr, line: i32) -> Result<(), i32> {
        if valid { Ok(()) } else {
            native_status(unsafe { (self.0.conn_check_fail)(expression.as_ptr(), file.as_ptr().cast(), line) })
        }
    }
    unsafe fn client(&mut self, pawn: *mut APlayerPawn) -> Result<*mut c_void, i32> {
        let mut client = ptr::null_mut();
        native_status(unsafe { (self.0.pawn_client)(pawn.cast(), &mut client) })?;
        Ok(client)
    }
    unsafe fn viewport_count(&mut self, client: *mut c_void) -> Result<i32, i32> {
        Ok(unsafe { (self.0.client_viewport_count)(client) })
    }
    unsafe fn viewport0(&mut self, client: *mut c_void) -> Result<*mut c_void, i32> {
        let mut viewport = ptr::null_mut();
        native_status(unsafe { (self.0.client_viewport0)(client, &mut viewport) })?;
        Ok(viewport)
    }
    unsafe fn clear_old_player(&mut self, viewport: *mut c_void) -> Result<(), i32> {
        unsafe { (self.0.viewport_clear_actor_player)(viewport); }
        Ok(())
    }
    unsafe fn net_speed(&mut self, viewport: *mut c_void, speed: i32) -> Result<(), i32> {
        unsafe { (self.0.viewport_set_net_speed)(viewport, speed); }
        Ok(())
    }
    unsafe fn role(&mut self, pawn: *mut APlayerPawn, role: u8) -> Result<(), i32> {
        unsafe { (self.0.pawn_set_role)(pawn.cast(), i32::from(role)); }
        Ok(())
    }
    unsafe fn show_flags(&mut self, pawn: *mut APlayerPawn, flags: u32) -> Result<(), i32> {
        unsafe { (self.0.pawn_set_show_flags)(pawn.cast(), flags); }
        Ok(())
    }
    unsafe fn rend_map(&mut self, pawn: *mut APlayerPawn, map: i32) -> Result<(), i32> {
        unsafe { (self.0.pawn_set_rend_map)(pawn.cast(), map); }
        Ok(())
    }
    unsafe fn set_player(&mut self, pawn: *mut APlayerPawn, viewport: *mut c_void) -> Result<(), i32> {
        native_status(unsafe { (self.0.pawn_set_player)(pawn.cast(), viewport) })
    }
    unsafe fn reset_input(&mut self, pawn: *mut APlayerPawn) -> Result<(), i32> {
        native_status(unsafe { (self.0.pawn_reset_input)(pawn.cast()) })
    }
    unsafe fn level_action(&mut self, pawn: *mut APlayerPawn, action: u8) -> Result<(), i32> {
        unsafe { (self.0.pawn_set_level_action)(pawn.cast(), i32::from(action)); }
        Ok(())
    }
}

/// # Safety
/// Source-native connection, pawn and services have their original C++ lifetime.
pub(crate) unsafe fn handle_native_client_player(
    connection: *mut UNetConnection, pawn: *mut c_void,
    services: &crate::net_conn_abi::Hp2NetConnServices,
) -> Result<(), i32> {
    unsafe { handle_base_with(&mut NativeClientPlayerOps(services), connection, pawn.cast()) }
}

pub(crate) struct CoreClientPlayerOps<'a>(pub(crate) CoreAccess<'a>);
impl ClientPlayerOps for CoreClientPlayerOps<'_> {
    type Error = RuntimeError;
    fn check(&mut self, valid: bool, expression: &CStr, file: &CStr, line: i32) -> RuntimeResult<()> {
        if valid { Ok(()) } else {
            Err(RuntimeError::new(&expression.to_string_lossy())
                .with_context(&format!("{}:{line}", file.to_string_lossy())))
        }
    }
    unsafe fn client(&mut self, pawn: *mut APlayerPawn) -> RuntimeResult<*mut c_void> {
        let level = unsafe { (*pawn.cast::<AActor>()).x_level.cast::<ULevel>() };
        let engine = unsafe { (*level).base.engine.cast::<UEngine>() };
        Ok(unsafe { (*engine).client })
    }
    unsafe fn viewport_count(&mut self, client: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { (*client.cast::<UClient>()).viewports.num })
    }
    unsafe fn viewport0(&mut self, client: *mut c_void) -> RuntimeResult<*mut c_void> {
        let viewports = unsafe { (*client.cast::<UClient>()).viewports };
        Ok(unsafe { viewports.data.cast::<*mut c_void>().read() })
    }
    unsafe fn clear_old_player(&mut self, viewport: *mut c_void) -> RuntimeResult<()> {
        let old = unsafe { (*viewport.cast::<UViewport>()).actor.cast::<APlayerPawn>() };
        unsafe { (*old).player = ptr::null_mut(); }
        Ok(())
    }
    unsafe fn net_speed(&mut self, viewport: *mut c_void, speed: i32) -> RuntimeResult<()> {
        unsafe { (*viewport.cast::<UViewport>()).current_net_speed = speed; }
        Ok(())
    }
    unsafe fn role(&mut self, pawn: *mut APlayerPawn, role: u8) -> RuntimeResult<()> {
        unsafe { (*pawn.cast::<AActor>()).role = role; }
        Ok(())
    }
    unsafe fn show_flags(&mut self, pawn: *mut APlayerPawn, flags: u32) -> RuntimeResult<()> {
        unsafe { (*pawn).show_flags = flags as i32; }
        Ok(())
    }
    unsafe fn rend_map(&mut self, pawn: *mut APlayerPawn, map: i32) -> RuntimeResult<()> {
        unsafe { (*pawn).rend_map = map; }
        Ok(())
    }
    unsafe fn set_player(&mut self, pawn: *mut APlayerPawn, viewport: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::player_owner::set_player(self.0, pawn, viewport.cast()) }
    }
    unsafe fn reset_input(&mut self, pawn: *mut APlayerPawn) -> RuntimeResult<()> {
        let client = unsafe { self.client(pawn)? };
        let viewport = unsafe { self.viewport0(client)? };
        unsafe { crate::camera_owner::reset_input(self.0, viewport.cast()) }
    }
    unsafe fn level_action(&mut self, pawn: *mut APlayerPawn, action: u8) -> RuntimeResult<()> {
        let level = unsafe { (*pawn.cast::<AActor>()).level.cast::<ALevelInfo>() };
        unsafe { (*level).level_action = action; }
        Ok(())
    }
}

/// Executes the source-qualified UNetConnection implementation, not an override.
///
/// # Safety
/// The canonical pawn, client graph and connection remain live through script.
pub unsafe fn handle_base_client_player(
    access: CoreAccess<'_>, connection: *mut UNetConnection, pawn: *mut APlayerPawn,
) -> RuntimeResult<()> {
    unsafe { handle_base_with(&mut CoreClientPlayerOps(access), connection, pawn) }
}

/// Uses the canonical connection's real virtual HandleClientPlayer override.
///
/// # Safety
/// All canonical records, reflected script metadata and SDK viewport resources
/// remain live across synchronous script callbacks.
pub unsafe fn handle_client_player(
    access: CoreAccess<'_>, connection: *mut UNetConnection, pawn: *mut APlayerPawn,
) -> RuntimeResult<()> {
    let demo = access.native_class("Engine.DemoRecConnection")?;
    if unsafe { hp2_core::object_model::is_a(connection.cast(), demo.cast()) } {
        unsafe { crate::net_demo::conn_handle_client_player_with(
            &mut CoreDemoClientPlayerOps(access), connection.cast(), pawn.cast(),
        ) }
    } else {
        unsafe { handle_base_client_player(access, connection, pawn) }
    }
}

struct CoreDemoClientPlayerOps<'a>(CoreAccess<'a>);

impl crate::net_demo::DemoClientPlayerOps for CoreDemoClientPlayerOps<'_> {
    type Error = RuntimeError;
    fn check_fail(&mut self, expression: &CStr, line: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(&expression.to_string_lossy())
            .with_context(&format!("UnDemoRec.cpp:{line}")))
    }
    unsafe fn spectator_name(&mut self, driver: *mut hp2_abi::engine::generated::UDemoRecDriver) -> RuntimeResult<*const u32> {
        let name = unsafe { ptr::addr_of!((*driver).demo_spectator_class).read_unaligned() };
        Ok(if name.num == 0 { hp2_core::lit!("").as_ptr() } else { name.data.cast() })
    }
    unsafe fn set_spectator_name(&mut self, driver: *mut hp2_abi::engine::generated::UDemoRecDriver, name: *const u32) -> RuntimeResult<()> {
        let mut length = 0;
        while unsafe { *name.add(length) } != 0 { length += 1; }
        let text = unsafe { std::slice::from_raw_parts(name, length + 1) };
        unsafe { self.0.assign_fstring(ptr::addr_of_mut!((*driver).demo_spectator_class), text) }
    }
    unsafe fn load_spectator_class(&mut self, name: *const u32) -> RuntimeResult<*mut c_void> {
        let mut length = 0;
        while unsafe { *name.add(length) } != 0 { length += 1; }
        let name = unsafe { std::slice::from_raw_parts(name, length + 1) };
        let base = self.0.native_class("Engine.PlayerPawn")?;
        // Source StaticLoadClass uses LOAD_NoFail (UnObjBas.h:19).
        unsafe { self.0.static_load_class(base, ptr::null_mut(), name, 0x0001).map(|class| class.cast()) }
    }
    unsafe fn actor_count(&mut self, pawn: *mut c_void) -> RuntimeResult<i32> {
        let level = unsafe { (*pawn.cast::<AActor>()).x_level.cast::<ULevel>() };
        unsafe { crate::level_owner::actor_count(level) }
    }
    unsafe fn actor_at(&mut self, pawn: *mut c_void, index: i32) -> RuntimeResult<*mut c_void> {
        let level = unsafe { (*pawn.cast::<AActor>()).x_level.cast::<ULevel>() };
        Ok(unsafe { crate::level_owner::actor_at(level, index).cast() })
    }
    unsafe fn is_player_start(&mut self, actor: *mut c_void) -> RuntimeResult<bool> {
        let class = self.0.native_class("Engine.PlayerStart")?;
        Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
    }
    unsafe fn spawn_spectator(&mut self, pawn: *mut c_void, class: *mut c_void, location: &[f32; 3], rotation: &[i32; 3]) -> RuntimeResult<*mut c_void> {
        use hp2_abi::engine::generated::FRotator;
        use hp2_abi::FVector;
        let level = unsafe { (*pawn.cast::<AActor>()).x_level.cast::<ULevel>() };
        let location = FVector { x: location[0], y: location[1], z: location[2] };
        let rotation = FRotator { pitch: rotation[0], yaw: rotation[1], roll: rotation[2] };
        unsafe { crate::level_spawn_actor_owner::spawn_actor(
            self.0, level, class.cast(), hp2_abi::engine::generated::FName { index: 0 }, ptr::null_mut(), ptr::null_mut(),
            location, rotation, ptr::null_mut(), true, false,
        ).map(|actor| actor.cast()) }
    }
    unsafe fn client(&mut self, pawn: *mut c_void) -> RuntimeResult<*mut UClient> {
        unsafe { CoreClientPlayerOps(self.0).client(pawn.cast()).map(|client| client.cast()) }
    }
    unsafe fn viewport0(&mut self, client: *mut UClient) -> RuntimeResult<*mut UViewport> {
        unsafe { CoreClientPlayerOps(self.0).viewport0(client.cast()).map(|viewport| viewport.cast()) }
    }
    unsafe fn set_player(&mut self, pawn: *mut c_void, viewport: *mut UViewport) -> RuntimeResult<()> {
        unsafe { crate::player_owner::set_player(self.0, pawn.cast(), viewport.cast()) }
    }
    unsafe fn set_admin(&mut self, pawn: *mut c_void) -> RuntimeResult<()> {
        // Source: `((APlayerPawn*)Pawn)->bAdmin = 1;` (UnDemoRec.cpp).
        unsafe { (*pawn.cast::<APlayerPawn>()).set_b_admin(true); }
        Ok(())
    }
    unsafe fn reset_input(&mut self, viewport: *mut UViewport) -> RuntimeResult<()> {
        unsafe { crate::camera_owner::reset_input(self.0, viewport) }
    }
    unsafe fn base_handle(&mut self, connection: *mut UNetConnection, pawn: *mut c_void) -> RuntimeResult<()> {
        unsafe { handle_base_client_player(self.0, connection, pawn.cast()) }
    }
    unsafe fn copy_song(&mut self, pawn: *mut c_void) -> RuntimeResult<()> {
        let level = unsafe { (*pawn.cast::<AActor>()).level.cast::<ALevelInfo>() };
        let song = unsafe { ptr::addr_of!((*level).song).read_unaligned() };
        let text = if song.num == 0 { hp2_core::lit!("") } else {
            unsafe { std::slice::from_raw_parts(song.data.cast::<u32>(), song.num as usize) }
        };
        unsafe { self.0.assign_fstring(ptr::addr_of_mut!((*pawn.cast::<APlayerPawn>()).song), text) }
    }
    unsafe fn set_transition_fade(&mut self, pawn: *mut c_void) -> RuntimeResult<()> {
        // EMusicTransition::MTRAN_Fade, EngineClasses.h:480.
        unsafe { (*pawn.cast::<APlayerPawn>()).transition = 3; }
        Ok(())
    }
    unsafe fn console(&mut self, viewport: *mut UViewport) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*viewport).console })
    }
    unsafe fn is_time_demo(&mut self, console: *mut c_void) -> RuntimeResult<bool> {
        // UConsole::IsTimeDemo is non-virtual and returns bTimeDemo (UnCon.h:115).
        Ok(unsafe { (*console.cast::<hp2_abi::engine::generated::UConsole>()).b_time_demo() })
    }
}
