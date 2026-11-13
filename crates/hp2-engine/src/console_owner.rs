//! Canonical UConsole metadata and direct operations.
use hp2_abi::engine::generated::{FSceneNode, UCanvas, UConsole, UConsoleEventMessageParams, UObject, UViewport};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind},
    object_archive::ObjectArchive,
    object_owner::ClassOps,
    name_owner::FindName,
};
use std::{alloc::Layout, cell::RefCell, collections::HashMap, ptr, rc::Rc};

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const TRANSIENT: u32 = 0x0000_2000;
const fn prop(name: &'static str, kind: StartupPropertyKind, offset: usize, array_dim: usize) -> StartupProperty {
    StartupProperty { name, kind, category: "Console", property_flags: TRANSIENT, object_flags: 4, inner_name: None, offset, array_dim }
}

static PROPERTIES: &[StartupProperty] = &[
    prop("Viewport", StartupPropertyKind::Object { class: "Engine.Viewport" }, std::mem::offset_of!(UConsole, viewport), 1),
    prop("TypedStr", StartupPropertyKind::String, std::mem::offset_of!(UConsole, typed_str), 1),
    prop("HistoryTop", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, history_top), 1),
    prop("HistoryBot", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, history_bot), 1),
    prop("HistoryCur", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, history_cur), 1),
    prop("History", StartupPropertyKind::String, std::mem::offset_of!(UConsole, history), 16),
    prop("Scrollback", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, scrollback), 1),
    prop("numLines", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, num_lines), 1),
    prop("TopLine", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, top_line), 1),
    prop("TextLines", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, text_lines), 1),
    prop("MsgTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, msg_time), 1),
    prop("MsgTickTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, msg_tick_time), 1),
    prop("MsgText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, msg_text), 64),
    prop("MsgType", StartupPropertyKind::Name, std::mem::offset_of!(UConsole, msg_type), 64),
    prop("MsgPlayer", StartupPropertyKind::Object { class: "Engine.PlayerReplicationInfo" }, std::mem::offset_of!(UConsole, msg_player), 64),
    prop("ConBackground", StartupPropertyKind::Object { class: "Engine.Texture" }, std::mem::offset_of!(UConsole, con_background), 1),
    prop("MsgTick", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, msg_tick), 64),
    prop("BorderSize", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, border_size), 1),
    prop("ConsoleLines", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, console_lines), 1),
    prop("BorderLines", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, border_lines), 1),
    prop("BorderPixels", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, border_pixels), 1),
    prop("ConsolePos", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, console_pos), 1),
    prop("ConsoleDest", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, console_dest), 1),
    prop("FrameX", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, frame_x), 1),
    prop("FrameY", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, frame_y), 1),
    prop("Border", StartupPropertyKind::Object { class: "Engine.Texture" }, std::mem::offset_of!(UConsole, border), 1),
    prop("Font", StartupPropertyKind::Object { class: "Engine.Font" }, std::mem::offset_of!(UConsole, font), 1),
    prop("LoadingMessage", StartupPropertyKind::String, std::mem::offset_of!(UConsole, loading_message), 1),
    prop("SavingMessage", StartupPropertyKind::String, std::mem::offset_of!(UConsole, saving_message), 1),
    prop("StartTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, start_time), 1),
    prop("ExtraTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, extra_time), 1),
    prop("LastFrameTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, last_frame_time), 1),
    prop("LastSecondStartTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, last_second_start_time), 1),
    prop("FrameCount", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, frame_count), 1),
    prop("LastSecondFrameCount", StartupPropertyKind::Int, std::mem::offset_of!(UConsole, last_second_frame_count), 1),
    prop("MinFPS", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, min_fps), 1),
    prop("MaxFPS", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, max_fps), 1),
    prop("LastSecFPS", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, last_sec_fps), 1),
    prop("ConnectingMessage", StartupPropertyKind::String, std::mem::offset_of!(UConsole, connecting_message), 1),
    prop("PausedMessage", StartupPropertyKind::String, std::mem::offset_of!(UConsole, paused_message), 1),
    prop("FadeoutTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, fadeout_time), 1),
    prop("FadeinTime", StartupPropertyKind::Float, std::mem::offset_of!(UConsole, fadein_time), 1),
    prop("PrecachingMessage", StartupPropertyKind::String, std::mem::offset_of!(UConsole, precaching_message), 1),
    prop("FrameRateText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, frame_rate_text), 1),
    prop("AvgText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, avg_text), 1),
    prop("LastSecText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, last_sec_text), 1),
    prop("MinText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, min_text), 1),
    prop("MaxText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, max_text), 1),
    prop("fpsText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, fps_text), 1),
    prop("SecondsText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, seconds_text), 1),
    prop("FramesText", StartupPropertyKind::String, std::mem::offset_of!(UConsole, frames_text), 1),
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Console", package: "Engine", super_name: Some("Object"), within: "Object",
    flags: 8, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
    kind: NativeRecordKind::Object, layout: Some(Layout::new::<UConsole>()), operations: Some(OPERATIONS),
    properties: PROPERTIES, defaults: &[],
}];

const BASE: ClassOps = NativeRecordKind::Object.operations();
const OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.construct = construct_object;
    operations.destroy = destroy_object;
    operations.finalize_members = finalize_members;
    operations.serialize = serialize;
    operations.visit_references = visit_references;
    operations
};
unsafe fn construct_object(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (BASE.construct)(access, object) }
}

unsafe fn destroy_object(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UConsole")); }
    unsafe { (BASE.destroy)(access, object) }
}

unsafe fn finalize_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UConsole")); }
    if let Some(owner) = access.owner_context::<crate::core_owner::EngineState>() {
        owner.console.remove(object.cast());
    }
    Ok(())
}

unsafe fn serialize(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (BASE.serialize)(access, object, archive) }
}

unsafe fn visit_references(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (BASE.visit_references)(access, object, archive) }
}

/// Per-owner frame snapshots used by canonical PreRender/PostRender pairs.
#[derive(Default)]
pub(crate) struct ConsoleOwnerState { frames: RefCell<HashMap<usize, FSceneNode>> }

impl ConsoleOwnerState {
    pub(crate) fn insert_saved_frame(&self, console: *mut UConsole, frame: FSceneNode) {
        self.frames.borrow_mut().insert(console as usize, frame);
    }
    pub(crate) fn saved_frame(&self, console: *mut UConsole) -> RuntimeResult<FSceneNode> {
        self.frames.borrow().get(&(console as usize)).copied()
            .ok_or_else(|| RuntimeError::new("Canonical Console has no saved render frame"))
    }
    pub(crate) fn remove(&self, console: *mut UConsole) { self.frames.borrow_mut().remove(&(console as usize)); }
}

pub(crate) fn state(access: CoreAccess<'_>) -> RuntimeResult<Rc<ConsoleOwnerState>> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine Console state is unavailable"))?;
    Ok(engine.console.clone())
}

/// Initializes a canonical Console without native-vptr dispatch.
///
/// # Safety
/// `console` must be canonical; `viewport` must be a live canonical viewport.
pub(crate) unsafe fn initialize(
    access: CoreAccess<'_>, console: *mut UConsole, viewport: *mut std::ffi::c_void,
) -> RuntimeResult<()> {
    if console.is_null() { return Err(RuntimeError::new("Null canonical UConsole")); }
    unsafe {
        (*console).viewport = viewport;
        (*console).top_line = crate::console::MAX_LINES - 1;
        (*console).border_size = 1;
        access.initialize_execution(console.cast())?;
    }
    let general = [b'G' as u32, b'e' as u32, b'n' as u32, b'e' as u32, b'r' as u32, b'a' as u32, b'l' as u32, 0];
    let core = [b'C' as u32, b'o' as u32, b'r' as u32, b'e' as u32, 0];
    let log_type = access.intern_str("Log", FindName::Find)?;
    for key in ["Engine", "Copyright"] {
        let mut key_units: Vec<u32> = key.bytes().map(u32::from).collect();
        key_units.push(0);
        let mut text = access.localize_text(&general, &key_units, Some(&core), None, false)?;
        text.push(0);
        unsafe { event_message(access, console, text.as_ptr(), log_type)?; }
    }
    let blank = [b' ' as u32, 0];
    unsafe {
        event_message(access, console, blank.as_ptr(), log_type)?;
        event_message(access, console, blank.as_ptr(), log_type)?;
    }
    Ok(())
}

/// Constructs and initializes a canonical UConsole for a viewport.
///
/// # Safety
/// `viewport` must be a live canonical viewport owned by `access`.
pub(crate) unsafe fn construct(
    access: CoreAccess<'_>, viewport: *mut std::ffi::c_void,
) -> RuntimeResult<*mut UConsole> {
    let class = access.native_class("Engine.Console")?;
    let console = unsafe {
        access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())?.cast::<UConsole>()
    };
    if let Err(error) = unsafe { initialize(access, console, viewport) } {
        unsafe { access.conditional_destroy(console.cast())?; }
        return Err(error);
    }
    Ok(console)
}

/// Calls the script Tick event through canonical metadata.
/// # Safety
/// `console` must be a live canonical Console owned by `access`.
pub(crate) unsafe fn tick(access: CoreAccess<'_>, console: *mut UConsole, delta: f32) -> RuntimeResult<()> {
    if console.is_null() { return Err(RuntimeError::new("Null canonical UConsole")); }
    let name = access.intern_str("Tick", hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(console.cast(), name, false)? };
    if function.is_null() { return Err(RuntimeError::new("Canonical Console Tick metadata is unavailable")); }
    #[repr(C)] struct Params { delta_time: f32 }
    let mut params = Params { delta_time: delta };
    unsafe { access.execute_event(console.cast(), function, (&mut params as *mut Params).cast(), std::ptr::null_mut()) }
}

/// Saves the active frame, ticks the script Console, and applies the shared pre-render policy.
///
/// # Safety
/// Console/frame must be live canonical records; callbacks perform direct Core operations.
pub(crate) unsafe fn pre_render(
    access: CoreAccess<'_>, console: *mut UConsole, frame: *mut FSceneNode, delta: f32,
    compute_render_size: impl FnOnce(*mut FSceneNode) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    if console.is_null() || frame.is_null() { return Err(RuntimeError::new("Null canonical Console render input")); }
    let state = state(access)?;
    let mut saved = unsafe { *frame };
    let result = unsafe {
        crate::console::pre_render_with(
            console, frame, &mut saved, delta,
            |object, delta| tick(access, object, delta),
            compute_render_size,
        )
    };
    if result.is_ok() { state.insert_saved_frame(console, saved); }
    result
}

/// Applies Console PreRender using the viewport's source-backed FTime delta.
///
/// # Safety
/// Console, viewport, and frame must be live canonical records.
pub(crate) unsafe fn pre_render_from_viewport(
    access: CoreAccess<'_>, console: *mut UConsole, frame: *mut FSceneNode,
    compute_render_size: impl FnOnce(*mut FSceneNode) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    if console.is_null() { return Err(RuntimeError::new("Null canonical UConsole")); }
    let viewport = unsafe { ptr::addr_of!((*console).viewport).read_unaligned().cast::<UViewport>() };
    if viewport.is_null() { return Err(RuntimeError::new("Canonical Console has no viewport")); }
    let current = i64::from_ne_bytes(unsafe { ptr::addr_of!((*viewport).current_time).read_unaligned() });
    let previous = i64::from_ne_bytes(unsafe { ptr::addr_of!((*viewport).last_update_time).read_unaligned() });
    let delta = hp2_core::time::subtract(current, previous);
    unsafe { pre_render(access, console, frame, delta, compute_render_size) }
}

/// Restores the caller-owned frame snapshot after Console rendering.
///
/// # Safety
/// Console/frame must be live canonical records owned by `access`.
pub(crate) unsafe fn post_render(access: CoreAccess<'_>, console: *mut UConsole, frame: *mut FSceneNode) -> RuntimeResult<()> {
    if console.is_null() || frame.is_null() { return Err(RuntimeError::new("Null canonical Console render input")); }
    let state = state(access)?;
    let saved = state.saved_frame(console)?;
    unsafe { crate::console::post_render_with(console, frame, &saved); }
    state.remove(console);
    Ok(())
}
/// Dispatches Console's PostRender script event through canonical Core metadata.
///
/// # Safety
/// Both records must be live canonical objects owned by `access`.
pub(crate) unsafe fn event_post_render(
    access: CoreAccess<'_>,
    console: *mut UConsole,
    canvas: *mut UCanvas,
) -> RuntimeResult<()> {
    if console.is_null() || canvas.is_null() {
        return Err(RuntimeError::new("Null canonical Console PostRender argument"));
    }
    let name = access.intern_str("PostRender", FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(console.cast(), name, false)? };
    if function.is_null() {
        return Err(RuntimeError::new("Canonical Console PostRender metadata is unavailable"));
    }
    if unsafe { crate::ui_console_views::post_render(access, console.cast(), canvas.cast(), function)? } {
        return Ok(());
    }
    #[repr(C)] struct Params { canvas: *mut UCanvas }
    let mut params = Params { canvas };
    unsafe { access.execute_event(console.cast(), function, (&mut params as *mut Params).cast(), ptr::null_mut()) }
}

/// Returns Console's script-controlled world-draw flag.
///
/// # Safety
/// `console` must be a live canonical UConsole.
pub(crate) unsafe fn get_draw_world(console: *mut UConsole) -> RuntimeResult<i32> {
    if console.is_null() { return Err(RuntimeError::new("Null canonical UConsole")); }
    Ok(unsafe { i32::from(!(*console).b_no_draw_world()) })
}


/// Routes one serialized output line through the script Message event.
///
/// # Safety
/// `console` must be canonical and `data` a NUL-terminated TCHAR string.
pub(crate) unsafe fn event_message(
    access: CoreAccess<'_>, console: *mut UConsole, data: *const u32, name: i32,
) -> RuntimeResult<()> {
    if console.is_null() || data.is_null() {
        return Err(RuntimeError::new("Invalid canonical Console Message input"));
    }
    let mut count = 0usize;
    while unsafe { data.add(count).read_unaligned() } != 0 { count += 1; }
    let mut message = Vec::with_capacity(count + 1);
    for index in 0..count { message.push(unsafe { data.add(index).read_unaligned() }); }
    message.push(0);

    let function_name = access.intern_str("Message", FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(console.cast(), function_name, false)? };
    if function.is_null() { return Err(RuntimeError::new("Canonical Console Message metadata is unavailable")); }

    let mut params: UConsoleEventMessageParams = unsafe { std::mem::zeroed() };
    params.n = name.to_ne_bytes();
    unsafe { access.assign_fstring(ptr::addr_of_mut!(params.s), &message)?; }
    let dispatched = unsafe {
        access.execute_event(console.cast(), function, (&mut params as *mut UConsoleEventMessageParams).cast(), ptr::null_mut())
    };
    let released = unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!(params.s)) };
    match (dispatched, released) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}


/// Executes a Console command through the canonical viewport/Core dispatchers.
///
/// # Safety
/// Console and command must remain live; command is NUL-terminated TCHAR data.
pub(crate) unsafe fn exec_command(
    access: CoreAccess<'_>, console: *mut UConsole, command: *const u32,
) -> RuntimeResult<bool> {
    if console.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid canonical Console command input"));
    }
    unsafe { crate::console::exec_command_with(console, command.cast(), |viewport, command, _| {
        if viewport.is_null() { return Err(RuntimeError::new("Canonical Console has no viewport")); }
        let log_type = access.intern_str("Log", FindName::Find)?;
        let mut output = |text: &[u32]| {
            let mut terminated = [0u32; 4096];
            let mut length = 0;
            for &unit in text.iter().take_while(|&&unit| unit != 0).take(terminated.len() - 1) {
                terminated[length] = unit;
                length += 1;
            }
            terminated[length] = 0;
            event_message(access, console, terminated.as_ptr(), log_type)
        };
        crate::camera_owner::exec(access, viewport.cast(), command.cast(), &mut output).map(i32::from)
    }) }.map(|value| value != 0)
}
