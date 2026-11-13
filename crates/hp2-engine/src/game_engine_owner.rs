//! Canonical Rust ClassOps for Engine.GameEngine.

use crate::replay_owner;
use hp2_abi::engine::generated::{ALevelInfo, APlayerPawn, FURL, TArrayRaw, UClient, UGameEngine, ULevel, UNetDriver, UObject, UPendingLevel, UViewport};
use hp2_abi::FPlane;
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupDefault, StartupProperty, StartupPropertyKind},
    object_archive::ObjectArchive,
    object_owner::ClassOps,
};
use std::{alloc::Layout, ptr};

/// Start an exclusive diagnostic session. False means disabled or incomplete.
#[doc(hidden)]
pub fn start_game_state_trace() -> bool { crate::game_load_map_owner::game_state_trace_start() }

/// Finish only the session successfully started by this owner.
#[doc(hidden)]
pub fn finish_game_state_trace(complete: bool) { crate::game_load_map_owner::game_state_trace_finish(complete); }

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_CONFIG: u32 = 0x0000_0004;
const CLASS_TRANSIENT: u32 = 0x0000_0008;
const BASE: ClassOps = crate::engine_owner::OPERATIONS;

const SERVER_ACTORS_OFFSET: usize = std::mem::offset_of!(UGameEngine, server_actors);
const SERVER_PACKAGES_OFFSET: usize = std::mem::offset_of!(UGameEngine, server_packages);

const PROPERTIES: &[StartupProperty] = &[
    StartupProperty {
        name: "ServerActors", kind: StartupPropertyKind::ArrayOfString,
        category: "Settings", property_flags: 0x0000_4000, object_flags: 4,
        inner_name: Some("StrProperty0"), offset: SERVER_ACTORS_OFFSET, array_dim: 1,
    },
    StartupProperty {
        name: "ServerPackages", kind: StartupPropertyKind::ArrayOfString,
        category: "Settings", property_flags: 0x0000_4000, object_flags: 4,
        inner_name: Some("StrProperty0"), offset: SERVER_PACKAGES_OFFSET, array_dim: 1,
    },
    StartupProperty {
        name: "FrameRateLimit", kind: StartupPropertyKind::Float,
        category: "GameEngine", property_flags: 0x0000_4000, object_flags: 4,
        inner_name: None, offset: std::mem::offset_of!(UGameEngine, frame_rate_limit), array_dim: 1,
    },
];
const DEFAULTS: &[StartupDefault] = &[StartupDefault::SetFloat { field: "FrameRateLimit", value: 144.0 }];

/// Canonical Engine.GameEngine declaration, using the compiler-probed UGameEngine allocation.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "GameEngine", package: "Engine", super_name: Some("Engine"), within: "Object",
    flags: CLASS_CONFIG | CLASS_TRANSIENT, object_flags: OBJECT_FLAGS, config: "System",
    guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<UGameEngine>()),
    operations: Some(OPERATIONS), properties: PROPERTIES, defaults: DEFAULTS,
}];

const OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.construct = construct;
    operations.destroy = destroy;
    operations.finalize_members = finalize_members;
    operations.serialize = serialize;
    operations.visit_references = serialize;
    operations
};

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine")); }
    // SAFETY: Engine is the genuine registered superclass at the UGameEngine prefix.
    unsafe { (BASE.construct)(access, object)?; }
    let game = object.cast::<UGameEngine>();
    let url = unsafe { ptr::addr_of_mut!((*game).last_url) };
    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    // FURL(TEXT("")) uses protocol/host/portal defaults and an empty map.
    // E_NoInit server arrays are left untouched for Core's reflected property links.
    unsafe {
        crate::url_owner::UrlOwner::construct_local_filename(access, &defaults, url, Some(&EMPTY_TEXT))?;
    }
    let replay = unsafe { ptr::addr_of_mut!((*game).replay) };
    unsafe { replay_owner::construct(access, replay)?; }
    Ok(())
}

/// Runs the source-shared canonical `UGameEngine::Init` policy.
///
/// # Safety
/// `engine` is a live Core-owned GameEngine record.
pub unsafe fn init(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    unsafe { crate::game_init_owner::init(access, engine) }
}
/// Executes the source-shared `UGameEngine::Exit` policy on canonical objects.
///
/// # Safety
/// `engine` is a live canonical GameEngine owned by `access`.
pub unsafe fn exit(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    struct CoreExit<'a>(CoreAccess<'a>);
    impl crate::game_engine::GameExitOps for CoreExit<'_> {
        type Error = RuntimeError;
        fn engine_exit_base(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<()> {
            unsafe { crate::engine_owner::exit(self.0, engine.cast()) }
        }
        fn level(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
            if engine.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine")); }
            Ok(unsafe { (*engine.cast::<UGameEngine>()).g_level.cast() })
        }
        fn level_net_driver(&mut self, level: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
            if level.is_null() { return Err(RuntimeError::new("Null canonical ULevel during GameEngine::Exit")); }
            Ok(unsafe { (*level.cast::<ULevel>()).base.net_driver.cast() })
        }
        fn delete_net_driver(&mut self, driver: *mut std::ffi::c_void) -> RuntimeResult<()> {
            unsafe { self.0.delete_object(driver.cast()) }
        }
        fn clear_level_net_driver(&mut self, level: *mut std::ffi::c_void) -> RuntimeResult<()> {
            if level.is_null() { return Err(RuntimeError::new("Null canonical ULevel during GameEngine::Exit")); }
            unsafe { (*level.cast::<ULevel>()).base.net_driver = ptr::null_mut(); }
            Ok(())
        }
    }
    unsafe { crate::game_engine::exit_with_ops(&mut CoreExit(access), engine.cast()) }
}

/// Advances the source-shared GameEngine policy on the canonical object graph.
///
/// # Safety
/// `engine` and its reachable world, client and network records remain live.
pub unsafe fn tick(access: CoreAccess<'_>, engine: *mut UGameEngine, delta: f32) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    // Reap a finished background save (and re-save in the foreground if it failed).
    crate::game_save_owner::poll_pending_save(access)?;
    // Cover stock input and timeout calls with one balanced transaction scope.
    #[cfg(feature = "sdl")]
    let _resolution_lifecycle = unsafe { crate::stock_resolution_lifecycle::install(access, crate::stock_resolution_transaction::callback)? };
    // Keep the display listener registered through post-tick rollback work.
    #[cfg(feature = "sdl")]
    let _display_mode = crate::stock_display_control::install()?;
    #[cfg(feature = "renderer-wgpu")]
    let _render_scale = crate::stock_render_scale_control::install()?;
    let _graphics_controls = crate::stock_graphics_menu::install()?;
    // The stock window scripts run synchronously during client input in this
    // tick. Drop the hook before returning to the owner or unloading packages.
    let _stock_fps = unsafe { crate::stock_fps_menu::install(access) };
    // Registered controls share the stock hook. End listener delivery before
    // dropping that hook, including on an early return from the tick.
    let _stock_menu_tick = if _stock_fps.is_some() { Some(crate::stock_menu_extensions::begin_tick()?) } else { None };
    let _graphics_input = match unsafe { crate::stock_graphics_dispatch::install_input(access) } {
        Ok(guard) => guard,
        Err(error) => {
            if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
                let _ = access.emit_log(category, &format!("Graphics input observer: {error}").chars().map(u32::from).collect::<Vec<_>>());
            }
            None
        }
    };
    let _controls_page = match unsafe { crate::stock_input_menu::install(access) } {
        Ok(guard) => guard,
        Err(error) => {
            if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
                let _ = access.emit_log(category, &format!("Controls page observer: {error}").chars().map(u32::from).collect::<Vec<_>>());
            }
            None
        }
    };
    let _ui_input = match unsafe { crate::ui_pages::install_input(access) } {
        Ok(guard) => guard,
        Err(error) => {
            if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
                let _ = access.emit_log(category, &format!("Rust menu input observer: {error}").chars().map(u32::from).collect::<Vec<_>>());
            }
            None
        }
    };
    let _ui_mirror_input = match unsafe { crate::ui_mirror::install_input(access) } {
        Ok(guard) => guard,
        Err(error) => {
            if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
                let _ = access.emit_log(category, &format!("Rust menu mirror input: {error}").chars().map(u32::from).collect::<Vec<_>>());
            }
            None
        }
    };
    let _ui_mirror_controls = match unsafe { crate::ui_mirror::install_control_input(access) } {
        Ok(guard) => guard,
        Err(error) => {
            if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
                let _ = access.emit_log(category, &format!("Rust menu mirror controls: {error}").chars().map(u32::from).collect::<Vec<_>>());
            }
            None
        }
    };
    let _ = unsafe { crate::ui_mirror::tick(access) };
    // Apply what the Rust menus queued, through the same Notify path a stock click takes.
    if let Err(error) = unsafe { crate::ui_pages::tick(access) } {
        if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
            let _ = access.emit_log(category, &format!("Rust menu tick: {error}").chars().map(u32::from).collect::<Vec<_>>());
        }
    }
    // Mirror the slider into the Controls page's number box before input dispatch.
    if let Err(error) = unsafe { crate::stock_input_menu::tick(access) } {
        if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
            let _ = access.emit_log(category, &format!("Controls page tick: {error}").chars().map(u32::from).collect::<Vec<_>>());
        }
    }
    // Hide unsupported or orphaned graphics controls before input dispatch.
    let graphics_result = unsafe { crate::stock_graphics_menu::tick(access) };
    if let Err(error) = &graphics_result {
        if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
            let _ = access.emit_log(category, &format!("Graphics controls tick: {error}").chars().map(u32::from).collect::<Vec<_>>());
        }
    }
    // Suspend orphaned controls before input when the hook is unavailable.
    // Refresh again after the game tick to observe renderer changes made there.
    #[cfg(feature = "renderer-wgpu")]
    let scale_pre_result = unsafe { crate::stock_render_scale_control::tick(access, crate::render_device_owner::world_scale_unavailable(access).ok()) };
    #[cfg(feature = "renderer-wgpu")]
    if let Err(error) = &scale_pre_result {
        if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
            let _ = access.emit_log(category, &format!("Render scale pre-tick: {error}").chars().map(u32::from).collect::<Vec<_>>());
        }
    }
    unsafe { crate::live_texture_detail::sync(access, engine)?; }
    let result = unsafe { crate::game_tick_owner::tick(access, engine, delta) };
    #[cfg(feature = "sdl")]
    unsafe { crate::stock_resolution_transaction::process_timeouts(access); }
    // Process pending rollback even when the hook is unavailable or the tick
    // failed. If both fail, log the display error and preserve the tick error.
    #[cfg(feature = "sdl")]
    let display_result = unsafe { crate::stock_display_control::tick(access) };
    #[cfg(feature = "renderer-wgpu")]
    let scale_result = unsafe { crate::stock_render_scale_control::tick(access, crate::render_device_owner::world_scale_unavailable(access).ok()) };
    #[cfg(feature = "renderer-wgpu")]
    if let Err(error) = &scale_result {
        if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
            let _ = access.emit_log(category, &format!("Render scale tick: {error}").chars().map(u32::from).collect::<Vec<_>>());
        }
    }
    #[cfg(feature = "sdl")]
    if let Err(error) = &display_result {
        if let Ok(category) = access.intern_str("Warning", hp2_core::name_owner::FindName::Add) {
            let _ = access.emit_log(category, &format!("Display mode tick: {error}").chars().map(u32::from).collect::<Vec<_>>());
        }
    }
    if result.is_ok() {
        graphics_result?;
        #[cfg(feature = "sdl")]
        display_result?;
        #[cfg(feature = "renderer-wgpu")]
        scale_pre_result?;
        #[cfg(feature = "renderer-wgpu")]
        scale_result?;
        unsafe { crate::live_texture_detail::sync(access, engine)?; }
        unsafe { crate::stock_resolution_deferred::process(access, engine)?; }
        unsafe { crate::stock_resolution_display::sync(access, engine)?; }
    }
    unsafe { crate::stock_fps_menu::probe(access); }
    result
}

/// Executes the source-shared canonical `UGameEngine::Exec` policy.
///
/// # Safety
/// `engine` is a live Core-owned GameEngine record and `cmd` is NUL-terminated UTF-32.
pub unsafe fn exec(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    cmd: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    unsafe { crate::game_exec_owner::exec(access, engine, cmd, output) }
}


/// Runs the complete shared native `UGameEngine::SaveGame` sequence on canonical
/// Core records and typed package/filesystem operations.
///
/// # Safety
/// `engine` is a live Core-owned GameEngine record.
pub unsafe fn save_game(access: CoreAccess<'_>, engine: *mut UGameEngine, position: i32) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    unsafe { crate::game_save_owner::save_game(access, engine, position) }
}

/// Performs the source `UGameEngine::PaintProgress` load-fog draw without a
/// native virtual call.
///
/// # Safety
/// `engine` and its Client/Viewport/PlayerPawn chain are live canonical records.
pub(crate) unsafe fn paint_progress(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine in PaintProgress"));
    }
    unsafe {
        crate::game_engine::paint_progress_with(
            &mut CorePaintProgressOps { access },
            engine.cast(),
        )
    }
}

struct CorePaintProgressOps<'a> { access: CoreAccess<'a> }

impl crate::game_engine::PaintProgressOps for CorePaintProgressOps<'_> {
    type Error = RuntimeError;

    unsafe fn engine_is_drawing(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<bool> {
        if engine.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine in PaintProgress")); }
        Ok(unsafe { (*engine.cast::<UGameEngine>()).g_is_drawing })
    }

    unsafe fn client(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        if engine.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine in PaintProgress")); }
        let client = unsafe { (*engine.cast::<UGameEngine>()).client };
        if client.is_null() { return Err(RuntimeError::new("PaintProgress requires a canonical UClient")); }
        Ok(client)
    }

    unsafe fn viewport0(&mut self, client: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        if client.is_null() { return Err(RuntimeError::new("Null canonical UClient in PaintProgress")); }
        let viewports = unsafe { ptr::addr_of!((*client.cast::<UClient>()).viewports).read_unaligned() };
        if viewports.num < 0 || viewports.max < viewports.num || (viewports.num > 0 && viewports.data.is_null()) {
            return Err(RuntimeError::new("Invalid canonical Client viewport array"));
        }
        if viewports.num == 0 { return Err(RuntimeError::new("PaintProgress requires viewport zero")); }
        let viewport = unsafe { viewports.data.cast::<*mut UViewport>().read_unaligned() };
        if viewport.is_null() { return Err(RuntimeError::new("PaintProgress viewport zero is null")); }
        Ok(viewport.cast())
    }

    unsafe fn draw_with_load_fog(
        &mut self,
        engine: *mut std::ffi::c_void,
        viewport: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        if engine.is_null() || viewport.is_null() {
            return Err(RuntimeError::new("PaintProgress requires live Engine and Viewport records"));
        }
        let actor = unsafe { (*viewport.cast::<UViewport>()).actor.cast::<APlayerPawn>() };
        if actor.is_null() { return Err(RuntimeError::new("PaintProgress viewport has no PlayerPawn")); }
        let fog_slot = unsafe { ptr::addr_of_mut!((*actor).flash_fog) };
        let old_fog = unsafe { fog_slot.read_unaligned() };
        unsafe { fog_slot.write_unaligned(FPlane { x: 0.0, y: 0.1, z: 0.25, w: 0.2 }); }
        struct FogRestore { actor: *mut APlayerPawn, fog: FPlane }
        impl Drop for FogRestore {
            fn drop(&mut self) {
                // SAFETY: the canonical pawn stays live throughout this synchronous draw.
                unsafe { ptr::addr_of_mut!((*self.actor).flash_fog).write_unaligned(self.fog); }
            }
        }
        let restore = FogRestore { actor, fog: old_fog };
        let result = unsafe {
            crate::game_draw_owner::draw(
                self.access, engine.cast::<UGameEngine>(), viewport.cast::<UViewport>(),
                true, ptr::null_mut(), ptr::null_mut(),
            )
        };
        drop(restore);
        result
    }
}


/// Computes the shared source `UGameEngine::GetMaxTickRate` policy for Core records.
///
/// # Safety
/// `engine` is a live Core-owned GameEngine; reached network records remain live.
pub unsafe fn get_max_tick_rate(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
) -> RuntimeResult<f32> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    unsafe {
        crate::game_tick::tick::get_max_tick_rate_with(
            &mut CoreMaxTickRateOps(access),
            engine.cast(),
        )
    }
}

struct CoreMaxTickRateOps<'a>(CoreAccess<'a>);
impl crate::game_tick::tick::MaxTickRateOps for CoreMaxTickRateOps<'_> {
    type Error = RuntimeError;
    fn parse_lan_play(&mut self) -> RuntimeResult<bool> {
        let line = self.0.command_line();
        Ok(!line.is_empty()
            && unsafe { hp2_core::search::parse_param(line.as_ptr().cast(), hp2_core::lit!("LANPLAY").as_ptr().cast()) })
    }
    fn is_client(&mut self) -> RuntimeResult<bool> {
        Ok(self.0.process_mode().is_client)
    }
    unsafe fn lan_demo_server_max_tick_rate(
        &mut self,
        net: *mut std::ffi::c_void,
        _demo: *mut std::ffi::c_void,
    ) -> RuntimeResult<f32> {
        if net.is_null() {
            return Err(RuntimeError::new("LanPlay demo tick rate requires a live Level.NetDriver"));
        }
        Ok(unsafe { (*net.cast::<UNetDriver>()).lan_server_max_tick_rate as f32 })
    }
}

/// Cancels and deletes the canonical GameEngine pending level in source order.
///
/// # Safety
/// `engine` is null or identifies a live Core-owned UGameEngine.
pub unsafe fn cancel_pending(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    unsafe { crate::network_owner::cancel_game_pending(access, engine) }
}

unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine")); }
    let game = object.cast::<UGameEngine>();
    crate::game_engine::destroy_with_ops(game.cast(), &mut GameDestroyOps(access))
}

unsafe fn finalize_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UGameEngine")); }
    let game = object.cast::<UGameEngine>();
    let mut first_error = None;
    remember(&mut first_error, unsafe { replay_owner::destroy(access, ptr::addr_of_mut!((*game).replay)) });
    remember(&mut first_error, unsafe { crate::url_owner::UrlOwner::destroy(access, ptr::addr_of_mut!((*game).last_url)) });
    match first_error { Some(error) => Err(error), None => Ok(()) }
}

struct GameDestroyOps<'a>(CoreAccess<'a>);
impl crate::game_engine::GameDestroyOps for GameDestroyOps<'_> {
    type Error = RuntimeError;
    fn has_pending(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<bool> {
        Ok(unsafe { !(*engine.cast::<UGameEngine>()).g_pending_level.is_null() })
    }
    fn cancel_pending(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { crate::network_owner::cancel_game_pending(self.0, engine.cast()) }
    }
    fn clear_level(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { (*engine.cast::<UGameEngine>()).g_level = ptr::null_mut(); }
        Ok(())
    }
    fn log_shutdown(&mut self) -> RuntimeResult<()> {
        let exit = self.0.intern_str("Exit", hp2_core::name_owner::FindName::Add)?;
        self.0.emit_log(exit, SHUTDOWN_MESSAGE)
    }
    fn destroy_base(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { (BASE.destroy)(self.0, engine.cast()) }
    }
}

unsafe fn serialize(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    // SAFETY: UEngine owns the inherited serialized prefix.
    unsafe { (BASE.serialize)(access, object, archive)?; }
    let game = object.cast::<UGameEngine>();
    for slot in unsafe { [
        ptr::addr_of_mut!((*game).g_level),
        ptr::addr_of_mut!((*game).g_entry),
        ptr::addr_of_mut!((*game).g_pending_level),
    ] } {
        // SAFETY: each slot is one of the three UObject references serialized by UGameEngine.
        unsafe { archive.serialize_object_slot(slot.cast::<*mut UObject>())?; }
    }
    Ok(())
}

fn remember(first: &mut Option<RuntimeError>, result: RuntimeResult<()>) {
    if let Err(error) = result {
        if first.is_none() { *first = Some(error); }
    }
}

const EMPTY_TEXT: [u32; 1] = [0];
const SHUTDOWN_MESSAGE: &[u32] = &[71, 97, 109, 101, 32, 101, 110, 103, 105, 110, 101, 32, 115, 104, 117, 116, 32, 100, 111, 119, 110];

struct CoreProgressOps<'a>(CoreAccess<'a>);

impl crate::game_engine::ProgressOps for CoreProgressOps<'_> {
    type Error = RuntimeError;
    unsafe fn client(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*engine.cast::<UGameEngine>()).client })
    }
    unsafe fn viewport_count(&mut self, client: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        Ok(unsafe { (*client.cast::<hp2_abi::engine::generated::UClient>()).viewports.num })
    }
    unsafe fn viewport0(&mut self, client: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        let viewports = unsafe { (*client.cast::<hp2_abi::engine::generated::UClient>()).viewports };
        Ok(unsafe { viewports.data.cast::<*mut std::ffi::c_void>().read() })
    }
    unsafe fn actor(&mut self, viewport: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*viewport.cast::<hp2_abi::engine::generated::UViewport>()).actor })
    }
    unsafe fn show_upgrade_menu(&mut self, actor: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let name = self.0.intern_str("ShowUpgradeMenu", hp2_core::name_owner::FindName::Find)?;
        let function = unsafe { self.0.find_object_function_by_name(actor.cast(), name, false)? };
        if function.is_null() {
            return Err(RuntimeError::new("ShowUpgradeMenu function is missing"));
        }
        unsafe { crate::actor_owner::process_event(
            self.0, actor.cast(), function, ptr::null_mut(), ptr::null_mut(),
        ) }
    }
    unsafe fn set_message(&mut self, actor: *mut std::ffi::c_void, slot: i32, text: *const u32) -> RuntimeResult<()> {
        use hp2_abi::engine::generated::APlayerPawn;
        let pawn = actor.cast::<APlayerPawn>();
        let mut length = 0;
        while unsafe { *text.add(length) } != 0 { length += 1; }
        let units = unsafe { std::slice::from_raw_parts(text, length + 1) };
        let message = unsafe { ptr::addr_of_mut!((*pawn).progress_message).cast::<hp2_abi::engine::generated::TArrayRaw>().add(slot as usize) };
        unsafe { self.0.assign_fstring(message, units)?; }
        // Leave alpha unchanged, matching the three native component writes.
        let color = unsafe { ptr::addr_of_mut!((*pawn).progress_color).cast::<hp2_abi::engine::generated::FColor>().add(slot as usize) };
        unsafe {
            ptr::addr_of_mut!((*color).r).write_unaligned(255);
            ptr::addr_of_mut!((*color).g).write_unaligned(255);
            ptr::addr_of_mut!((*color).b).write_unaligned(255);
        }
        Ok(())
    }
    unsafe fn set_timeout(&mut self, actor: *mut std::ffi::c_void, seconds: f32) -> RuntimeResult<()> {
        use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn};
        let level = unsafe { (*actor.cast::<AActor>()).level.cast::<ALevelInfo>() };
        let time = unsafe { ptr::addr_of!((*level).time_seconds).read_unaligned() };
        unsafe { ptr::addr_of_mut!((*actor.cast::<APlayerPawn>()).progress_time_out).write_unaligned(time + seconds); }
        Ok(())
    }
}

/// Updates the canonical local player's messages, colors and progress deadline.
///
/// # Safety
/// `engine` and its Client/Viewport/Pawn graph are canonical live records. The
/// Pawn has loaded script-property metadata; both texts are NUL-terminated and
/// do not overlap either destination progress-message allocation.
pub unsafe fn set_progress(
    access: CoreAccess<'_>, engine: *mut UGameEngine,
    message: *const u32, progress: *const u32, seconds: f32,
) -> RuntimeResult<()> {
    unsafe { crate::game_engine::set_progress_with(&mut CoreProgressOps(access), engine.cast(), message, progress, seconds) }
}
struct CoreConnectingMessageOps<'a>(CoreAccess<'a>);

impl CoreConnectingMessageOps<'_> {
    fn copy_text(raw: &TArrayRaw) -> RuntimeResult<Vec<u32>> {
        if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
            return Err(RuntimeError::new("Invalid pending-level URL string"));
        }
        if raw.num == 0 { return Ok(vec![0]); }
        let units = unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) };
        let end = units.iter().position(|unit| *unit == 0).unwrap_or(units.len());
        let mut text = units[..end].to_vec();
        text.push(0);
        Ok(text)
    }

    fn write_message(destination: *mut u32, text: &[u32]) -> RuntimeResult<()> {
        if destination.is_null() {
            return Err(RuntimeError::new("Null connecting-message buffer"));
        }
        let end = text.iter().position(|unit| *unit == 0).unwrap_or(text.len());
        if end >= 256 {
            return Err(RuntimeError::new("Connecting message exceeds source buffer capacity"));
        }
        unsafe {
            ptr::copy_nonoverlapping(text.as_ptr(), destination, end);
            destination.add(end).write(0);
        }
        Ok(())
    }

    fn substitute(format: &[u32], value: &[u32]) -> RuntimeResult<Vec<u32>> {
        let end = format.iter().position(|unit| *unit == 0).unwrap_or(format.len());
        let needle = [u32::from(b'%'), u32::from(b's')];
        let Some(index) = format[..end].windows(2).position(|window| window == needle) else {
            return Err(RuntimeError::new("Localized connecting format has no string placeholder"));
        };
        let value_end = value.iter().position(|unit| *unit == 0).unwrap_or(value.len());
        let mut result = Vec::with_capacity(end + value_end);
        result.extend_from_slice(&format[..index]);
        result.extend_from_slice(&value[..value_end]);
        result.extend_from_slice(&format[index + needle.len()..end]);
        result.push(0);
        Ok(result)
    }

    fn localized(&self, key: &str) -> RuntimeResult<Vec<u32>> {
        let key = key.chars().map(u32::from).chain(std::iter::once(0)).collect::<Vec<_>>();
        self.0.localize_text(
            hp2_core::lit!("Progress"),
            &key,
            Some(hp2_core::lit!("Engine")),
            None,
            false,
        )
    }
}

impl crate::game_engine::ConnectingMessageOps for CoreConnectingMessageOps<'_> {
    type Error = RuntimeError;

    unsafe fn pending(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*engine.cast::<UGameEngine>()).g_pending_level })
    }
    unsafe fn client(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*engine.cast::<UGameEngine>()).client })
    }
    unsafe fn viewport_count(&mut self, client: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let viewports = unsafe { ptr::addr_of!((*client.cast::<hp2_abi::engine::generated::UClient>()).viewports).read_unaligned() };
        if viewports.num < 0 || viewports.max < viewports.num || (viewports.num > 0 && viewports.data.is_null()) {
            return Err(RuntimeError::new("Invalid canonical Client viewport array"));
        }
        Ok(viewports.num)
    }
    unsafe fn viewport0(&mut self, client: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        let count = unsafe { self.viewport_count(client)? };
        if count == 0 { return Ok(ptr::null_mut()); }
        let views = unsafe { ptr::addr_of!((*client.cast::<hp2_abi::engine::generated::UClient>()).viewports).read_unaligned() };
        Ok(unsafe { views.data.cast::<*mut std::ffi::c_void>().read_unaligned() })
    }
    unsafe fn actor(&mut self, viewport: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*viewport.cast::<hp2_abi::engine::generated::UViewport>()).actor })
    }
    unsafe fn progress_expired(&mut self, actor: *mut std::ffi::c_void) -> RuntimeResult<bool> {
        if actor.is_null() {
            return Err(RuntimeError::new("Connecting progress pawn is null"));
        }
        let pawn = actor.cast::<APlayerPawn>();
        let timeout = unsafe { ptr::addr_of!((*pawn).progress_time_out).read_unaligned() };
        let level = unsafe { ptr::addr_of!((*pawn).base.base.level).read_unaligned().cast::<ALevelInfo>() };
        if level.is_null() {
            return Err(RuntimeError::new("Connecting progress pawn has no level"));
        }
        let time = unsafe { ptr::addr_of!((*level).time_seconds).read_unaligned() };
        Ok(timeout < time)
    }
    unsafe fn pending_is_demo(&mut self, pending: *mut std::ffi::c_void) -> RuntimeResult<bool> {
        Ok(!unsafe { (*pending.cast::<UPendingLevel>()).demo_rec_driver }.is_null())
    }
    unsafe fn format_demo(
        &mut self, pending: *mut std::ffi::c_void, msg1: *mut u32, msg2: *mut u32,
    ) -> RuntimeResult<()> {
        let url = unsafe { ptr::addr_of!((*pending.cast::<UPendingLevel>()).url).read_unaligned() };
        Self::write_message(msg1, &[0])?;
        let map = Self::copy_text(&url.map)?;
        Self::write_message(msg2, &map)
    }
    unsafe fn format_connecting(
        &mut self, pending: *mut std::ffi::c_void, msg1: *mut u32, msg2: *mut u32,
    ) -> RuntimeResult<()> {
        let url = unsafe { ptr::addr_of!((*pending.cast::<UPendingLevel>()).url).read_unaligned() };
        let first = self.localized("ConnectingText")?;
        Self::write_message(msg1, &first)?;
        let format = self.localized("ConnectingURL")?;
        let host = Self::copy_text(&url.host)?;
        let map = Self::copy_text(&url.map)?;
        let formatted = Self::substitute(&Self::substitute(&format, &host)?, &map)?;
        Self::write_message(msg2, &formatted)
    }
    unsafe fn set_progress(
        &mut self,
        engine: *mut std::ffi::c_void,
        msg1: *const u32,
        msg2: *const u32,
        seconds: f32,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::game_engine_owner::set_progress(self.0, engine.cast(), msg1, msg2, seconds)
        }
    }
}

/// Updates the canonical player's connection progress through the shared source policy.
///
/// # Safety
/// `engine` is a live Core-owned GameEngine; any selected records are canonical and live.
pub unsafe fn update_connecting_message(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    unsafe {
        crate::game_engine::update_connecting_message_with(
            &mut CoreConnectingMessageOps(access),
            engine.cast(),
        )
    }
}
/// Canonical UGameEngine::SetClientTravel, including its checked viewport cast.
///
/// # Safety
/// Player and terminated URL belong to the live canonical engine graph.
pub(crate) unsafe fn set_client_travel(
    access: CoreAccess<'_>, player: *mut std::ffi::c_void, url: *const u32,
    items: i32, travel: i32,
) -> RuntimeResult<()> {
    unsafe {
        crate::game_engine::set_client_travel_with(player, url, items, travel,
            |valid| if valid { Ok(()) } else { Err(RuntimeError::new("Assertion failed: Player in UGameEngine::SetClientTravel")) },
            |player, url, travel, items| {
                let class = access.native_class("Engine.Viewport")?;
                if !hp2_core::object_model::is_a(player.cast(), class.cast()) {
                    return Err(RuntimeError::new("CastChecked<UViewport> failed in SetClientTravel"));
                }
                let viewport = player.cast::<hp2_abi::engine::generated::UViewport>();
                let mut length = 0;
                while *url.add(length) != 0 { length += 1; }
                access.write_vm_fstring_result(ptr::addr_of_mut!((*viewport).travel_url), std::slice::from_raw_parts(url.cast(), length))?;
                (*viewport).travel_type = travel;
                // The native destination is an unsigned one-bit bitfield.
                (*viewport).set_b_travel_items(items & 1 != 0);
                Ok(())
            },
        )
    }
}
