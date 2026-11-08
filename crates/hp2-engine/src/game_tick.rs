//! Port of the `UGameEngine` Draw, Tick and console UI scale groups of
//! `HarryPotter2/Unreal/Engine/Src/UnGame.cpp`.
//!
//! Tick policy is shared through typed `TickOps`; Draw's Native adapter uses
//! [`Hp2GameTickServices`], filled by `Hp2GameTickServices.cpp`. Each entry
//! performs one oracle statement group. Engine handles cross as `*mut c_void`.
//!
//! Native exceptions are captured at each leaf and propagated as status to the
//! unchanged native guard. No later policy mutation occurs after a failed leaf.

use std::ffi::{c_char, c_void};

use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APlayerPawn, FSceneNode, UCanvas, UClient, UGameEngine, ULevel,
    UNetConnection, UNetDriver, UPendingLevel, URenderDevice, UViewport,
};

pub mod draw;
pub mod tick;

/// `LEVACT_None` (`EngineClasses.h:2755`).
pub const LEVACT_NONE: i32 = 0;
/// `LOCKR_ClearScreen` (`UnRenDev.h:21`).
pub const LOCKR_CLEAR_SCREEN: u32 = 1;
/// `IST_MAX` (`EngineClasses.h:401`).
pub const IST_MAX: i32 = 5;
/// `IK_MouseX` (`EngineClasses.h:359`).
pub const IK_MOUSE_X: i32 = 228;
/// `IK_MouseY` (`EngineClasses.h:360`).
pub const IK_MOUSE_Y: i32 = 229;

/// Viewport mouse axis ids for `viewport_mouse` / `set_viewport_mouse`.
pub const AXIS_X: i32 = 0;
pub const AXIS_Y: i32 = 1;

/// Checked native leaves: zero succeeds, nonzero stops the current Rust continuation.
/// Query results are written only on success. The immutable table is safe to share
/// across callbacks; native objects are always opaque raw pointers.
#[repr(C)]
pub struct Hp2GameTickServices {
    pub app_cycles: unsafe extern "C" fn(out: *mut u32) -> i32,
    pub fail_assert:
        unsafe extern "C" fn(expr: *const c_char, file: *const c_char, line: i32) -> i32,
    pub app_request_exit: unsafe extern "C" fn(code: i32) -> i32,
    pub g_ticks_increment: unsafe extern "C" fn() -> i32,
    pub static_tick: unsafe extern "C" fn() -> i32,
    pub gcache_tick: unsafe extern "C" fn() -> i32,
    pub log_all_windows_closed: unsafe extern "C" fn() -> i32,
    pub update_connecting_message: unsafe extern "C" fn(e: *mut c_void) -> i32,
    pub engine_key: unsafe extern "C" fn(e: *mut c_void, vp: *mut c_void, key: i32) -> i32,
    pub engine_input_event: unsafe extern "C" fn(
        e: *mut c_void,
        vp: *mut c_void,
        key: i32,
        state: i32,
        delta: f32,
    ) -> i32,
    pub engine_save_game: unsafe extern "C" fn(e: *mut c_void, slot: i32) -> i32,
    pub engine_exec_restart_sounds: unsafe extern "C" fn(e: *mut c_void) -> i32,
    pub engine_flush: unsafe extern "C" fn(e: *mut c_void) -> i32,
    pub server_switch_travel: unsafe extern "C" fn(e: *mut c_void, mode: i32) -> i32,
    pub client_travel_browse:
        unsafe extern "C" fn(e: *mut c_void, vp: *mut c_void, mode: i32) -> i32,
    pub replay_paused: unsafe extern "C" fn(e: *mut c_void, out: *mut i32) -> i32,
    pub replay_replaying: unsafe extern "C" fn(e: *mut c_void, out: *mut i32) -> i32,
    pub replay_recording: unsafe extern "C" fn(e: *mut c_void, out: *mut i32) -> i32,
    pub replay_serialize_frame_tick:
        unsafe extern "C" fn(e: *mut c_void, dt: f32, out: *mut f32) -> i32,
    pub replay_serialize_frame_input: unsafe extern "C" fn(
        e: *mut c_void,
        key: *mut i32,
        state: *mut i32,
        delta: *mut f32,
        out: *mut i32,
    ) -> i32,
    pub canvas_replay_text:
        unsafe extern "C" fn(e: *mut c_void, canvas: *mut c_void, recording: i32) -> i32,
    pub client_tick: unsafe extern "C" fn(client: *mut c_void) -> i32,
    pub viewport_set_mouse_capture:
        unsafe extern "C" fn(vp: *mut c_void, capture: i32, clip: i32, focus_only: i32) -> i32,
    pub viewport_lock: unsafe extern "C" fn(
        vp: *mut c_void,
        flash_scale: *const f32,
        flash_fog: *const f32,
        screen_clear: *const f32,
        flags: u32,
        hit_data: *mut u8,
        hit_size: *mut i32,
        out: *mut i32,
    ) -> i32,
    pub viewport_unlock: unsafe extern "C" fn(vp: *mut c_void, blit: i32) -> i32,
    pub viewport_ui_scale: unsafe extern "C" fn(vp: *mut c_void, out: *mut f32) -> i32,
    pub viewport_event_view_flash: unsafe extern "C" fn(vp: *mut c_void, dt: f32) -> i32,
    pub viewport_travel_is_restart: unsafe extern "C" fn(vp: *mut c_void, out: *mut i32) -> i32,
    pub view_calc: unsafe extern "C" fn(
        vp: *mut c_void,
        loc: *mut f32,
        rot: *mut i32,
        out: *mut *mut c_void,
    ) -> i32,
    pub actor_event_pre_render:
        unsafe extern "C" fn(actor: *mut c_void, canvas: *mut c_void) -> i32,
    pub actor_event_post_render:
        unsafe extern "C" fn(actor: *mut c_void, canvas: *mut c_void) -> i32,
    pub level_info: unsafe extern "C" fn(level: *mut c_void, out: *mut *mut c_void) -> i32,
    pub level_info_next_url_is_restart: unsafe extern "C" fn(li: *mut c_void, out: *mut i32) -> i32,
    pub level_tick_all: unsafe extern "C" fn(level: *mut c_void, dt: f32) -> i32,
    pub level_point_check_viewer:
        unsafe extern "C" fn(level: *mut c_void, loc: *const f32, out: *mut i32) -> i32,
    pub renderdev_end_flash: unsafe extern "C" fn(rd: *mut c_void) -> i32,
    pub renderdev_begin_ui: unsafe extern "C" fn(rd: *mut c_void, frame: *mut c_void) -> i32,
    pub render_precache: unsafe extern "C" fn(render: *mut c_void, vp: *mut c_void) -> i32,
    pub render_create_master_frame: unsafe extern "C" fn(
        render: *mut c_void,
        vp: *mut c_void,
        loc: *const f32,
        rot: *const i32,
        out: *mut *mut c_void,
    ) -> i32,
    pub render_pre_render: unsafe extern "C" fn(render: *mut c_void, frame: *mut c_void) -> i32,
    pub render_draw_world: unsafe extern "C" fn(render: *mut c_void, frame: *mut c_void) -> i32,
    pub render_post_render: unsafe extern "C" fn(render: *mut c_void, frame: *mut c_void) -> i32,
    pub render_finish_master_frame: unsafe extern "C" fn(render: *mut c_void) -> i32,
    pub audio_update: unsafe extern "C" fn(audio: *mut c_void, frame: *mut c_void) -> i32,
    pub audio_post_render: unsafe extern "C" fn(audio: *mut c_void, frame: *mut c_void) -> i32,
    pub canvas_update: unsafe extern "C" fn(canvas: *mut c_void, frame: *mut c_void) -> i32,
    pub canvas_strlen_fps: unsafe extern "C" fn(
        canvas: *mut c_void,
        fps: i32,
        width: *mut i32,
        height: *mut i32,
    ) -> i32,
    pub canvas_printf_fps: unsafe extern "C" fn(canvas: *mut c_void, fps: i32) -> i32,
    pub console_pre_render: unsafe extern "C" fn(console: *mut c_void, frame: *mut c_void) -> i32,
    pub console_post_render: unsafe extern "C" fn(console: *mut c_void, frame: *mut c_void) -> i32,
    pub console_event_post_render:
        unsafe extern "C" fn(console: *mut c_void, canvas: *mut c_void) -> i32,
    pub console_get_draw_world: unsafe extern "C" fn(console: *mut c_void, out: *mut i32) -> i32,
    pub console_root: unsafe extern "C" fn(console: *mut c_void, out: *mut *mut c_void) -> i32,
    pub root_scale_props:
        unsafe extern "C" fn(root: *mut c_void, out: *mut f32, result_out: *mut i32) -> i32,
    pub root_set_scale: unsafe extern "C" fn(root: *mut c_void, scale: f32) -> i32,
    pub pending_tick: unsafe extern "C" fn(p: *mut c_void, dt: f32) -> i32,
    pub console_connect_failure: unsafe extern "C" fn(console: *mut c_void, p: *mut c_void) -> i32,
    pub parse_param_lan_play: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub g_is_client: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub g_repair_save_slot: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub g_repair_save_pending: unsafe extern "C" fn(out: *mut i32) -> i32,
    pub set_g_repair_save_slot: unsafe extern "C" fn(value: i32) -> i32,
    pub set_g_repair_save_pending: unsafe extern "C" fn(value: i32) -> i32,
    pub log_save_repair: unsafe extern "C" fn(slot: i32) -> i32,
    pub system_save_path: unsafe extern "C" fn(out: *mut *const c_void) -> i32,
    pub system_save_slot_path: unsafe extern "C" fn(out: *mut *const c_void) -> i32,
    pub make_directory: unsafe extern "C" fn(path: *const c_void, tree: i32) -> i32,
    pub log_saving_game: unsafe extern "C" fn(filename: *const c_void) -> i32,
    pub pending_set_failure_progress: unsafe extern "C" fn(e: *mut c_void, p: *mut c_void) -> i32,
    pub pending_log_failure: unsafe extern "C" fn(p: *mut c_void) -> i32,
    pub string_nonempty: unsafe extern "C" fn(value: *mut c_void, out: *mut i32) -> i32,
    pub string_data: unsafe extern "C" fn(value: *mut c_void, out: *mut *const c_void) -> i32,
    pub pending_load_map:
        unsafe extern "C" fn(e: *mut c_void, p: *mut c_void, error: *mut c_void) -> i32,
    pub localize_connection_failed: unsafe extern "C" fn(out: *mut *const c_void) -> i32,
    pub set_progress:
        unsafe extern "C" fn(e: *mut c_void, a: *const c_void, b: *const c_void, sec: f32) -> i32,
    pub pending_send_join: unsafe extern "C" fn(p: *mut c_void) -> i32,
    pub pending_level_delete: unsafe extern "C" fn(p: *mut c_void) -> i32,
    pub pending_load_scope:
        unsafe extern "C" fn(services: *const Hp2GameTickServices, e: *mut c_void) -> i32,
}

// All native leaves capture before returning. Never inspect an out value after failure.
macro_rules! checked_method {
    ($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty) => {
        #[inline]
        pub unsafe fn $name(&self, $($arg: $ty),*) -> Result<$ret, i32> {
            let mut out = core::mem::MaybeUninit::<$ret>::uninit();
            let status = unsafe { (self.$name)($($arg,)* out.as_mut_ptr()) };
            if status == 0 { Ok(unsafe { out.assume_init() }) } else { Err(status) }
        }
    };
    ($name:ident($($arg:ident: $ty:ty),*)) => {
        #[inline]
        pub unsafe fn $name(&self, $($arg: $ty),*) -> Result<(), i32> {
            let status = unsafe { (self.$name)($($arg),*) };
            if status == 0 { Ok(()) } else { Err(status) }
        }
    };
}

impl Hp2GameTickServices {
    checked_method!(app_cycles() -> u32);
    checked_method!(fail_assert(expr: *const c_char, file: *const c_char, line: i32));
    checked_method!(app_request_exit(code: i32));
    checked_method!(g_ticks_increment());
    checked_method!(static_tick());
    checked_method!(gcache_tick());
    checked_method!(log_all_windows_closed());
    checked_method!(update_connecting_message(e: *mut c_void));
    checked_method!(engine_key(e: *mut c_void, vp: *mut c_void, key: i32));
    checked_method!(engine_input_event(e: *mut c_void, vp: *mut c_void, key: i32, state: i32, delta: f32));
    checked_method!(engine_save_game(e: *mut c_void, slot: i32));
    checked_method!(engine_exec_restart_sounds(e: *mut c_void));
    checked_method!(engine_flush(e: *mut c_void));
    checked_method!(server_switch_travel(e: *mut c_void, mode: i32));
    checked_method!(client_travel_browse(e: *mut c_void, vp: *mut c_void, mode: i32));
    checked_method!(replay_paused(e: *mut c_void) -> i32);
    checked_method!(replay_replaying(e: *mut c_void) -> i32);
    checked_method!(replay_recording(e: *mut c_void) -> i32);
    checked_method!(replay_serialize_frame_tick(e: *mut c_void, dt: f32) -> f32);
    checked_method!(replay_serialize_frame_input(e: *mut c_void, key: *mut i32, state: *mut i32, delta: *mut f32) -> i32);
    checked_method!(canvas_replay_text(e: *mut c_void, canvas: *mut c_void, recording: i32));
    checked_method!(client_tick(client: *mut c_void));
    checked_method!(viewport_set_mouse_capture(vp: *mut c_void, capture: i32, clip: i32, focus_only: i32));
    checked_method!(viewport_lock(vp: *mut c_void, flash_scale: *const f32, flash_fog: *const f32, screen_clear: *const f32, flags: u32, hit_data: *mut u8, hit_size: *mut i32) -> i32);
    checked_method!(viewport_unlock(vp: *mut c_void, blit: i32));
    checked_method!(viewport_ui_scale(vp: *mut c_void) -> f32);
    checked_method!(viewport_event_view_flash(vp: *mut c_void, dt: f32));
    checked_method!(viewport_travel_is_restart(vp: *mut c_void) -> i32);
    checked_method!(view_calc(vp: *mut c_void, loc: *mut f32, rot: *mut i32) -> *mut c_void);
    checked_method!(actor_event_pre_render(actor: *mut c_void, canvas: *mut c_void));
    checked_method!(actor_event_post_render(actor: *mut c_void, canvas: *mut c_void));
    checked_method!(level_info(level: *mut c_void) -> *mut c_void);
    checked_method!(level_info_next_url_is_restart(li: *mut c_void) -> i32);
    checked_method!(level_tick_all(level: *mut c_void, dt: f32));
    checked_method!(level_point_check_viewer(level: *mut c_void, loc: *const f32) -> i32);
    checked_method!(renderdev_end_flash(rd: *mut c_void));
    checked_method!(renderdev_begin_ui(rd: *mut c_void, frame: *mut c_void));
    checked_method!(render_precache(render: *mut c_void, vp: *mut c_void));
    checked_method!(render_create_master_frame(render: *mut c_void, vp: *mut c_void, loc: *const f32, rot: *const i32) -> *mut c_void);
    checked_method!(render_pre_render(render: *mut c_void, frame: *mut c_void));
    checked_method!(render_draw_world(render: *mut c_void, frame: *mut c_void));
    checked_method!(render_post_render(render: *mut c_void, frame: *mut c_void));
    checked_method!(render_finish_master_frame(render: *mut c_void));
    checked_method!(audio_update(audio: *mut c_void, frame: *mut c_void));
    checked_method!(audio_post_render(audio: *mut c_void, frame: *mut c_void));
    checked_method!(canvas_update(canvas: *mut c_void, frame: *mut c_void));
    checked_method!(canvas_strlen_fps(canvas: *mut c_void, fps: i32, width: *mut i32, height: *mut i32));
    checked_method!(canvas_printf_fps(canvas: *mut c_void, fps: i32));
    checked_method!(console_pre_render(console: *mut c_void, frame: *mut c_void));
    checked_method!(console_post_render(console: *mut c_void, frame: *mut c_void));
    checked_method!(console_event_post_render(console: *mut c_void, canvas: *mut c_void));
    checked_method!(console_get_draw_world(console: *mut c_void) -> i32);
    checked_method!(console_root(console: *mut c_void) -> *mut c_void);
    checked_method!(root_scale_props(root: *mut c_void, out: *mut f32) -> i32);
    checked_method!(root_set_scale(root: *mut c_void, scale: f32));
    checked_method!(pending_tick(p: *mut c_void, dt: f32));
    checked_method!(console_connect_failure(console: *mut c_void, p: *mut c_void));
    checked_method!(parse_param_lan_play() -> i32);
    checked_method!(g_is_client() -> i32);
    checked_method!(g_repair_save_slot() -> i32);
    checked_method!(g_repair_save_pending() -> i32);
    checked_method!(set_g_repair_save_slot(value: i32));
    checked_method!(set_g_repair_save_pending(value: i32));
    checked_method!(log_save_repair(slot: i32));
    checked_method!(system_save_path() -> *const c_void);
    checked_method!(system_save_slot_path() -> *const c_void);
    checked_method!(make_directory(path: *const c_void, tree: i32));
    checked_method!(log_saving_game(filename: *const c_void));
    checked_method!(pending_set_failure_progress(e: *mut c_void, p: *mut c_void));
    checked_method!(pending_log_failure(p: *mut c_void));
    checked_method!(string_nonempty(value: *mut c_void) -> i32);
    checked_method!(string_data(value: *mut c_void) -> *const c_void);
    checked_method!(pending_load_map(e: *mut c_void, p: *mut c_void, error: *mut c_void));
    checked_method!(localize_connection_failed() -> *const c_void);
    checked_method!(set_progress(e: *mut c_void, a: *const c_void, b: *const c_void, sec: f32));
    checked_method!(pending_send_join(p: *mut c_void));
    checked_method!(pending_level_delete(p: *mut c_void));
    checked_method!(pending_load_scope(services: *const Hp2GameTickServices, e: *mut c_void));
}

/// Former table leaves now read/written directly through `hp2_abi` mirrors
/// (round 10, rule 1). Null handling matches the deleted C++ leaves: the
/// crash on a null pointer is preserved, not guarded against.

/// `AActor::GetLevel()`, already ported as `crate::actor::get_level`.
///
/// # Safety
/// `actor` must be a live `AActor*`.
#[inline]
pub unsafe fn actor_level(actor: *mut c_void) -> *mut c_void {
    unsafe { crate::actor::get_level(actor.cast()) }
}

/// `((AActor*)Actor)->Level->LevelAction`.
///
/// # Safety
/// `actor` must be a live `AActor*` with a non-null `Level`.
#[inline]
pub unsafe fn actor_level_action(actor: *mut c_void) -> i32 {
    unsafe {
        let level = (*actor.cast::<AActor>()).level;
        i32::from((*level.cast::<ALevelInfo>()).level_action)
    }
}

/// `((APlayerPawn*)Actor)->bShowMenu`.
///
/// # Safety
/// `actor` must be a live `APlayerPawn*`.
#[inline]
pub unsafe fn actor_show_menu(actor: *mut c_void) -> i32 {
    unsafe { (*actor.cast::<APlayerPawn>()).b_show_menu() as i32 }
}

/// `((ALevelInfo*)Li)->Pauser != TEXT("")`, delegated to the shared
/// FString-nonempty bridge with the field address taken through the mirror.
///
/// # Safety
/// `li` must be a live `ALevelInfo*`.
#[inline]
pub unsafe fn level_info_pauser_nonempty(s: &Hp2GameTickServices, li: *mut c_void) -> Result<i32, i32> {
    unsafe { s.string_nonempty(core::ptr::addr_of_mut!((*li.cast::<ALevelInfo>()).pauser).cast()) }
}

/// `((ALevelInfo*)Info)->LevelAction = Action`.
///
/// # Safety
/// `info` must be a live `ALevelInfo*`.
#[inline]
pub unsafe fn level_info_set_action(info: *mut c_void, action: i32) {
    unsafe { (*info.cast::<ALevelInfo>()).level_action = action as u8 };
}

/// `((ULevel*)Level)->AudioTickCycles`.
///
/// # Safety
/// `level` must be a live `ULevel*`.
#[inline]
pub unsafe fn level_get_audio_tick_cycles(level: *mut c_void) -> i32 {
    unsafe { (*level.cast::<ULevel>()).audio_tick_cycles }
}

/// `((ULevel*)Level)->AudioTickCycles = Value`.
///
/// # Safety
/// `level` must be a live `ULevel*`.
#[inline]
pub unsafe fn level_set_audio_tick_cycles(level: *mut c_void, value: i32) {
    unsafe { (*level.cast::<ULevel>()).audio_tick_cycles = value };
}

/// `((ULevel*)Level)->NetDriver`.
///
/// # Safety
/// `level` must be a live `ULevel*`.
#[inline]
pub unsafe fn level_net_driver(level: *mut c_void) -> *mut c_void {
    unsafe { (*level.cast::<ULevel>()).base.net_driver }
}

/// `((ULevel*)Level)->DemoRecDriver`.
///
/// # Safety
/// `level` must be a live `ULevel*`.
#[inline]
pub unsafe fn level_demo_rec_driver(level: *mut c_void) -> *mut c_void {
    unsafe { (*level.cast::<ULevel>()).base.demo_rec_driver }
}

/// `((UViewport*)Vp)->Actor`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_actor(vp: *mut c_void) -> *mut c_void {
    unsafe { (*vp.cast::<UViewport>()).actor }
}

/// `((UViewport*)Vp)->RenDev`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_renderdev(vp: *mut c_void) -> *mut c_void {
    unsafe { (*vp.cast::<UViewport>()).ren_dev }
}

/// `((UViewport*)Vp)->Canvas`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_canvas(vp: *mut c_void) -> *mut c_void {
    unsafe { (*vp.cast::<UViewport>()).canvas }
}

/// `((FSceneNode*)Frame)->X`.
///
/// # Safety
/// `frame` must be a live `FSceneNode*`.
#[inline]
pub unsafe fn frame_x(frame: *mut c_void) -> i32 {
    unsafe { (*frame.cast::<FSceneNode>()).x }
}

/// `((FSceneNode*)Frame)->Y`.
///
/// # Safety
/// `frame` must be a live `FSceneNode*`.
#[inline]
pub unsafe fn frame_y(frame: *mut c_void) -> i32 {
    unsafe { (*frame.cast::<FSceneNode>()).y }
}

/// `((UPendingLevel*)Pending)->Error != TEXT("")`, delegated to the shared
/// FString-nonempty bridge with the field address taken through the mirror.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_error_nonempty(s: &Hp2GameTickServices, p: *mut c_void) -> Result<i32, i32> {
    unsafe { s.string_nonempty(core::ptr::addr_of_mut!((*p.cast::<UPendingLevel>()).error).cast()) }
}

/// `((UPendingLevel*)Pending)->FailCode != TEXT("")`, delegated to the shared
/// FString-nonempty bridge with the field address taken through the mirror.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_fail_code_nonempty(s: &Hp2GameTickServices, p: *mut c_void) -> Result<i32, i32> {
    unsafe { s.string_nonempty(core::ptr::addr_of_mut!((*p.cast::<UPendingLevel>()).fail_code).cast()) }
}

/// `((UPendingLevel*)Pending)->Success`.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_success(p: *mut c_void) -> i32 {
    unsafe { (*p.cast::<UPendingLevel>()).success }
}

/// `((UPendingLevel*)Pending)->FilesNeeded`.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_files_needed(p: *mut c_void) -> i32 {
    unsafe { (*p.cast::<UPendingLevel>()).files_needed }
}

/// `((UPendingLevel*)Pending)->SentJoin`.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_sent_join(p: *mut c_void) -> i32 {
    unsafe { (*p.cast::<UPendingLevel>()).sent_join }
}

/// `((UPendingLevel*)Pending)->LonePlayer`.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_lone_player(p: *mut c_void) -> i32 {
    unsafe { (*p.cast::<UPendingLevel>()).lone_player }
}

/// `((UPendingLevel*)Pending)->NetDriver = NULL`.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_clear_net_driver(p: *mut c_void) {
    unsafe { (*p.cast::<UPendingLevel>()).net_driver = core::ptr::null_mut() };
}

/// `((UPendingLevel*)Pending)->DemoRecDriver = NULL`.
///
/// # Safety
/// `p` must be a live `UPendingLevel*`.
#[inline]
pub unsafe fn pending_clear_demo_driver(p: *mut c_void) {
    unsafe { (*p.cast::<UPendingLevel>()).demo_rec_driver = core::ptr::null_mut() };
}

/// `((UNetDriver*)Net)->LanServerMaxTickRate` (`INT`, implicitly widened to
/// `FLOAT` by the deleted oracle leaf).
///
/// # Safety
/// `net` must be a live `UNetDriver*`.
#[inline]
pub unsafe fn net_driver_lan_server_max_tick_rate(net: *mut c_void) -> f32 {
    unsafe { (*net.cast::<UNetDriver>()).lan_server_max_tick_rate as f32 }
}

/// `((UNetDriver*)Net)->NetServerMaxTickRate` (`INT`, implicitly widened to
/// `FLOAT` by the deleted oracle leaf).
///
/// # Safety
/// `net` must be a live `UNetDriver*`.
#[inline]
pub unsafe fn net_driver_server_max_tick_rate(net: *mut c_void) -> f32 {
    unsafe { (*net.cast::<UNetDriver>()).net_server_max_tick_rate as f32 }
}

/// `((UNetDriver*)Net)->ServerConnection`.
///
/// # Safety
/// `net` must be a live `UNetDriver*`.
#[inline]
pub unsafe fn net_driver_server_connection(net: *mut c_void) -> *mut c_void {
    unsafe { (*net.cast::<UNetDriver>()).server_connection }
}

/// `((UNetConnection*)Connection)->CurrentNetSpeed`.
///
/// # Safety
/// `conn` must be a live `UNetConnection*`.
#[inline]
pub unsafe fn connection_current_net_speed(conn: *mut c_void) -> i32 {
    unsafe { (*conn.cast::<UNetConnection>()).current_net_speed }
}

/// `((UDemoRecDriver*)Demo)->ServerConnection` (inherited from `UNetDriver`).
///
/// # Safety
/// `demo` must be a live `UDemoRecDriver*`.
#[inline]
pub unsafe fn demo_rec_driver_server_connection(demo: *mut c_void) -> *mut c_void {
    unsafe { (*demo.cast::<UNetDriver>()).server_connection }
}

/// `((UDemoRecDriver*)Demo)->NetServerMaxTickRate` (`INT`, inherited from
/// `UNetDriver`, implicitly widened to `FLOAT` by the deleted oracle leaf).
///
/// # Safety
/// `demo` must be a live `UDemoRecDriver*`.
#[inline]
pub unsafe fn demo_rec_driver_net_server_max_tick_rate(demo: *mut c_void) -> f32 {
    unsafe { (*demo.cast::<UNetDriver>()).net_server_max_tick_rate as f32 }
}

/// Batch 2 (round 10): `UGameEngine`/`UClient`/`UViewport`/`APlayerPawn`/
/// `ALevelInfo` pure field leaves, read/written directly through `hp2_abi`
/// mirrors. Null handling matches the deleted C++ leaves.

/// `((UGameEngine*)Engine)->GIsDrawing = Value != 0`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn set_is_drawing(engine: *mut c_void, value: i32) {
    unsafe { (*engine.cast::<UGameEngine>()).g_is_drawing = value != 0 };
}

/// `((UGameEngine*)Engine)->GLevel`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn g_level(engine: *mut c_void) -> *mut c_void {
    unsafe { (*engine.cast::<UGameEngine>()).g_level }
}

/// `((UGameEngine*)Engine)->GEntry`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn g_entry(engine: *mut c_void) -> *mut c_void {
    unsafe { (*engine.cast::<UGameEngine>()).g_entry }
}

/// `((UGameEngine*)Engine)->GPendingLevel`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn g_pending(engine: *mut c_void) -> *mut c_void {
    unsafe { (*engine.cast::<UGameEngine>()).g_pending_level }
}

/// `((UGameEngine*)Engine)->GPendingLevel = Pending`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn set_g_pending(engine: *mut c_void, pending: *mut c_void) {
    unsafe { (*engine.cast::<UGameEngine>()).g_pending_level = pending };
}

/// `((UGameEngine*)Engine)->GViewport`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn g_viewport(engine: *mut c_void) -> *mut c_void {
    unsafe { (*engine.cast::<UGameEngine>()).g_viewport }
}

/// `((UGameEngine*)Engine)->Client`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn engine_client(engine: *mut c_void) -> *mut c_void {
    unsafe { (*engine.cast::<UGameEngine>()).client }
}

/// `((UGameEngine*)Engine)->Render`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn engine_render(engine: *mut c_void) -> *mut c_void {
    unsafe { (*engine.cast::<UGameEngine>()).render }
}

/// `((UGameEngine*)Engine)->Audio`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn engine_audio(engine: *mut c_void) -> *mut c_void {
    unsafe { (*engine.cast::<UGameEngine>()).audio }
}

/// `((UGameEngine*)Engine)->CurrentTickRate`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn engine_current_tick_rate(engine: *mut c_void) -> f32 {
    unsafe { (*engine.cast::<UGameEngine>()).current_tick_rate }
}

/// `((UGameEngine*)Engine)->GameCycles`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn engine_game_cycles(engine: *mut c_void) -> i32 {
    unsafe { (*engine.cast::<UGameEngine>()).game_cycles }
}

/// `((UGameEngine*)Engine)->GameCycles = Value`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn set_engine_game_cycles(engine: *mut c_void, value: i32) {
    unsafe { (*engine.cast::<UGameEngine>()).game_cycles = value };
}

/// `((UGameEngine*)Engine)->TickCycles = Value`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn set_engine_tick_cycles(engine: *mut c_void, value: i32) {
    unsafe { (*engine.cast::<UGameEngine>()).tick_cycles = value };
}

/// `((UGameEngine*)Engine)->ClientCycles = Value`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn set_engine_client_cycles(engine: *mut c_void, value: i32) {
    unsafe { (*engine.cast::<UGameEngine>()).client_cycles = value };
}

/// `((UGameEngine*)Engine)->FrameRateLimit`.
///
/// # Safety
/// `engine` must be a live `UGameEngine*`.
#[inline]
pub unsafe fn engine_frame_rate_limit(engine: *mut c_void) -> f32 {
    unsafe { (*engine.cast::<UGameEngine>()).frame_rate_limit }
}

/// `((UClient*)Client)->Viewports.Num()`.
///
/// # Safety
/// `client` must be a live `UClient*`.
#[inline]
pub unsafe fn client_viewport_count(client: *mut c_void) -> i32 {
    unsafe { (*client.cast::<UClient>()).viewports.num }
}

/// `((UClient*)Client)->Viewports(Index)`.
///
/// # Safety
/// `client` must be a live `UClient*` with `Index` in bounds.
#[inline]
pub unsafe fn client_viewport(client: *mut c_void, index: i32) -> *mut c_void {
    unsafe {
        let data = (*client.cast::<UClient>()).viewports.data.cast::<*mut c_void>();
        *data.add(index as usize)
    }
}

/// `((UClient*)Client)->CaptureMouse`.
///
/// # Safety
/// `client` must be a live `UClient*`.
#[inline]
pub unsafe fn client_capture_mouse(client: *mut c_void) -> i32 {
    unsafe { (*client.cast::<UClient>()).capture_mouse() as i32 }
}

/// `((UClient*)Client)->ScreenFlashes`.
///
/// # Safety
/// `client` must be a live `UClient*`.
#[inline]
pub unsafe fn client_screen_flashes(client: *mut c_void) -> i32 {
    unsafe { (*client.cast::<UClient>()).screen_flashes() as i32 }
}

/// `((UClient*)Client)->ShowFPS`.
///
/// # Safety
/// `client` must be a live `UClient*`.
#[inline]
pub unsafe fn client_show_fps(client: *mut c_void) -> i32 {
    unsafe { (*client.cast::<UClient>()).show_fps() as i32 }
}

/// `((UViewport*)Vp)->Console`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_console(vp: *mut c_void) -> *mut c_void {
    unsafe { (*vp.cast::<UViewport>()).console }
}

/// `((UViewport*)Vp)->bSuspendPrecaching`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_suspend_precaching(vp: *mut c_void) -> i32 {
    unsafe { (*vp.cast::<UViewport>()).b_suspend_precaching() as i32 }
}

/// `((UViewport*)Vp)->bShowWindowsMouse`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_show_windows_mouse(vp: *mut c_void) -> i32 {
    unsafe { (*vp.cast::<UViewport>()).b_show_windows_mouse() as i32 }
}

/// `Axis==0 ? Viewport->WindowsMouseX : Viewport->WindowsMouseY`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_mouse(vp: *mut c_void, axis: i32) -> f32 {
    unsafe {
        let v = &*vp.cast::<UViewport>();
        if axis == 0 { v.windows_mouse_x } else { v.windows_mouse_y }
    }
}

/// `if(Axis==0) Viewport->WindowsMouseX=Value; else Viewport->WindowsMouseY=Value;`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn set_viewport_mouse(vp: *mut c_void, axis: i32, value: f32) {
    unsafe {
        let v = &mut *vp.cast::<UViewport>();
        if axis == 0 {
            v.windows_mouse_x = value;
        } else {
            v.windows_mouse_y = value;
        }
    }
}

/// `((UViewport*)Vp)->bTravelItems`.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_travel_items(vp: *mut c_void) -> i32 {
    unsafe { (*vp.cast::<UViewport>()).b_travel_items() as i32 }
}

/// `((UViewport*)Vp)->TravelURL != TEXT("")`, delegated to the shared
/// FString-nonempty bridge with the field address taken through the mirror.
///
/// # Safety
/// `vp` must be a live `UViewport*`.
#[inline]
pub unsafe fn viewport_travel_url_nonempty(s: &Hp2GameTickServices, vp: *mut c_void) -> Result<i32, i32> {
    unsafe { s.string_nonempty(core::ptr::addr_of_mut!((*vp.cast::<UViewport>()).travel_url).cast()) }
}

/// `((AActor*)Actor)->Level->bNeverPrecache`.
///
/// # Safety
/// `actor` must be a live `AActor*` with a non-null `Level`.
#[inline]
pub unsafe fn actor_level_never_precache(actor: *mut c_void) -> i32 {
    unsafe {
        let level = (*actor.cast::<AActor>()).level;
        (*level.cast::<ALevelInfo>()).b_never_precache() as i32
    }
}

/// `((APlayerPawn*)Actor)->FlashFog` copied into `Out[0..4]` as `X,Y,Z,W`.
///
/// # Safety
/// `actor` must be a live `APlayerPawn*`; `out` must point to 4 writable `f32`s.
#[inline]
pub unsafe fn actor_flash_fog(actor: *mut c_void, out: *mut f32) {
    unsafe {
        let fog = (*actor.cast::<APlayerPawn>()).flash_fog;
        *out.add(0) = fog.x;
        *out.add(1) = fog.y;
        *out.add(2) = fog.z;
        *out.add(3) = fog.w;
    }
}

/// `((APlayerPawn*)Actor)->bQueuedToSaveGame`.
///
/// # Safety
/// `actor` must be a live `APlayerPawn*`.
#[inline]
pub unsafe fn player_queued_save(actor: *mut c_void) -> i32 {
    unsafe { (*actor.cast::<APlayerPawn>()).b_queued_to_save_game() as i32 }
}

/// `((APlayerPawn*)Actor)->bQueuedToSaveGame = Value != 0`.
///
/// # Safety
/// `actor` must be a live `APlayerPawn*`.
#[inline]
pub unsafe fn set_player_queued_save(actor: *mut c_void, value: i32) {
    unsafe { (*actor.cast::<APlayerPawn>()).set_b_queued_to_save_game(value != 0) };
}

/// `((ALevelInfo*)Li)->NextURL != TEXT("")`, delegated to the shared
/// FString-nonempty bridge with the field address taken through the mirror.
///
/// # Safety
/// `li` must be a live `ALevelInfo*`.
#[inline]
pub unsafe fn level_info_next_url_nonempty(s: &Hp2GameTickServices, li: *mut c_void) -> Result<i32, i32> {
    unsafe { s.string_nonempty(core::ptr::addr_of_mut!((*li.cast::<ALevelInfo>()).next_url).cast()) }
}

/// `((ALevelInfo*)Li)->bNextItems`.
///
/// # Safety
/// `li` must be a live `ALevelInfo*`.
#[inline]
pub unsafe fn level_info_next_items(li: *mut c_void) -> i32 {
    unsafe { (*li.cast::<ALevelInfo>()).b_next_items() as i32 }
}

/// `((ALevelInfo*)Li)->NextSwitchCountdown`.
///
/// # Safety
/// `li` must be a live `ALevelInfo*`.
#[inline]
pub unsafe fn level_info_next_switch_countdown(li: *mut c_void) -> f32 {
    unsafe { (*li.cast::<ALevelInfo>()).next_switch_countdown }
}

/// `((ALevelInfo*)Li)->NextSwitchCountdown = Value`.
///
/// # Safety
/// `li` must be a live `ALevelInfo*`.
#[inline]
pub unsafe fn set_level_info_next_switch_countdown(li: *mut c_void, value: f32) {
    unsafe { (*li.cast::<ALevelInfo>()).next_switch_countdown = value };
}

/// C++ `Min(A,B)` (`UnTemplate.h`): `(A<=B)?A:B`.
#[inline]
pub fn min(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

/// C++ `Max(A,B)` (`UnTemplate.h`): `(A>=B)?A:B`.
#[inline]
pub fn max(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// C++ `Clamp(X,Min,Max)` (`UnTemplate.h`): `X<Min?Min:X<Max?X:Max`.
#[inline]
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x < hi {
        x
    } else {
        hi
    }
}

/// C++ `Abs(A)` (`UnTemplate.h`): `(A>=0)?A:-A`.
#[inline]
pub fn abs(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `UGameEngine::GetConsoleUIScale` (oracle 1614–1622).
#[inline]
pub fn get_console_ui_scale(width: f32, height: f32, user_scale: f32) -> f32 {
    if width <= 0.0 || height <= 0.0 {
        return 1.0;
    }
    let mut user = user_scale;
    if user.is_nan() {
        user = 1.0;
    }
    user = clamp(user, 0.75, 2.0);
    min(width / 640.0, height / 480.0) * user
}

/// Applies the shared Console UI-scale policy through typed root and viewport leaves.
///
/// # Errors
/// Returns an error from the first typed Core leaf that fails.
///
/// # Safety
/// `vp` must be a live `UViewport*` or null.
pub unsafe fn apply_console_ui_scale_with<E>(
    vp: *mut c_void,
    mut console_root: impl FnMut(*mut c_void) -> Result<*mut c_void, E>,
    mut root_scale_props: impl FnMut(*mut c_void, &mut [f32; 3]) -> Result<i32, E>,
    mut viewport_ui_scale: impl FnMut(*mut c_void) -> Result<f32, E>,
    mut root_set_scale: impl FnMut(*mut c_void, f32) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        if vp.is_null() {
            return Ok(());
        }
        let console = viewport_console(vp);
        if console.is_null() {
            return Ok(());
        }
        let root = console_root(console)?;
        if root.is_null() {
            return Ok(());
        }
        let mut props = [0.0; 3];
        if root_scale_props(root, &mut props)? == 0 {
            return Ok(());
        }
        let desired = get_console_ui_scale(props[0], props[1], viewport_ui_scale(vp)?);
        if abs(props[2] - desired) > 0.0001 {
            root_set_scale(root, desired)?;
        }
        Ok(())
    }
}

/// `UGameEngine::ApplyConsoleUIScale` (oracle 1624–1673).
///
/// # Safety
/// `vp` must be a live `UViewport*` or null, as the C++ caller passes.
pub unsafe fn apply_console_ui_scale(s: &Hp2GameTickServices, vp: *mut c_void) -> Result<(), i32> {
    unsafe {
        apply_console_ui_scale_with(
            vp,
            |console| s.console_root(console),
            |root, props| s.root_scale_props(root, props.as_mut_ptr()),
            |viewport| s.viewport_ui_scale(viewport),
            |root, scale| s.root_set_scale(root, scale),
        )
    }
}

/// `((URenderDevice*)Rd)->PrecacheOnFlip`.
///
/// # Safety
/// `rd` must be a live `URenderDevice*`.
#[inline]
pub unsafe fn renderdev_precache_on_flip(rd: *mut c_void) -> bool {
    unsafe { (*rd.cast::<URenderDevice>()).precache_on_flip != 0 }
}

/// `((URenderDevice*)Rd)->PrecacheOnFlip = Value`.
///
/// # Safety
/// `rd` must be a live `URenderDevice*`.
#[inline]
pub unsafe fn set_renderdev_precache_on_flip(rd: *mut c_void, value: bool) {
    unsafe { (*rd.cast::<URenderDevice>()).precache_on_flip = value as u32 }
}

/// `((UCanvas*)Canvas)->Render = (URenderBase*)Render`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_set_render(canvas: *mut c_void, render: *mut c_void) {
    unsafe { (*canvas.cast::<UCanvas>()).render = render };
}

/// `((UCanvas*)Canvas)->OrgX`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_org_x(canvas: *mut c_void) -> f32 {
    unsafe { (*canvas.cast::<UCanvas>()).org_x }
}

/// `((UCanvas*)Canvas)->OrgX = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_org_x(canvas: *mut c_void, value: f32) {
    unsafe { (*canvas.cast::<UCanvas>()).org_x = value };
}

/// `((UCanvas*)Canvas)->OrgY`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_org_y(canvas: *mut c_void) -> f32 {
    unsafe { (*canvas.cast::<UCanvas>()).org_y }
}

/// `((UCanvas*)Canvas)->OrgY = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_org_y(canvas: *mut c_void, value: f32) {
    unsafe { (*canvas.cast::<UCanvas>()).org_y = value };
}

/// `((UCanvas*)Canvas)->ClipX`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_clip_x(canvas: *mut c_void) -> f32 {
    unsafe { (*canvas.cast::<UCanvas>()).clip_x }
}

/// `((UCanvas*)Canvas)->ClipX = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_clip_x(canvas: *mut c_void, value: f32) {
    unsafe { (*canvas.cast::<UCanvas>()).clip_x = value };
}

/// `((UCanvas*)Canvas)->ClipY`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_clip_y(canvas: *mut c_void) -> f32 {
    unsafe { (*canvas.cast::<UCanvas>()).clip_y }
}

/// `((UCanvas*)Canvas)->ClipY = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_clip_y(canvas: *mut c_void, value: f32) {
    unsafe { (*canvas.cast::<UCanvas>()).clip_y = value };
}

/// `((UCanvas*)Canvas)->CurX`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_cur_x(canvas: *mut c_void) -> f32 {
    unsafe { (*canvas.cast::<UCanvas>()).cur_x }
}

/// `((UCanvas*)Canvas)->CurX = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_cur_x(canvas: *mut c_void, value: f32) {
    unsafe { (*canvas.cast::<UCanvas>()).cur_x = value };
}

/// `((UCanvas*)Canvas)->CurY`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_cur_y(canvas: *mut c_void) -> f32 {
    unsafe { (*canvas.cast::<UCanvas>()).cur_y }
}

/// `((UCanvas*)Canvas)->CurY = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_cur_y(canvas: *mut c_void, value: f32) {
    unsafe { (*canvas.cast::<UCanvas>()).cur_y = value };
}

/// `((UCanvas*)Canvas)->X`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_size_x(canvas: *mut c_void) -> i32 {
    unsafe { (*canvas.cast::<UCanvas>()).x }
}

/// `((UCanvas*)Canvas)->X = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_size_x(canvas: *mut c_void, value: i32) {
    unsafe { (*canvas.cast::<UCanvas>()).x = value };
}

/// `((UCanvas*)Canvas)->Y`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_size_y(canvas: *mut c_void) -> i32 {
    unsafe { (*canvas.cast::<UCanvas>()).y }
}

/// `((UCanvas*)Canvas)->Y = Value`.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_size_y(canvas: *mut c_void, value: i32) {
    unsafe { (*canvas.cast::<UCanvas>()).y = value };
}

/// `((UCanvas*)Canvas)->Color`, as raw `(R,G,B,A)` bytes.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_color(canvas: *mut c_void) -> [u8; 4] {
    unsafe { (*canvas.cast::<UCanvas>()).color }
}

/// `((UCanvas*)Canvas)->Color = FColor(...)`, as raw `(R,G,B,A)` bytes.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn set_canvas_color(canvas: *mut c_void, color: [u8; 4]) {
    unsafe { (*canvas.cast::<UCanvas>()).color = color };
}

/// `((UCanvas*)Cv)->Color = FColor(R,G,B)`. The 3-arg `FColor` constructor
/// (`UnTex.h:81`) never initializes alpha, so the existing alpha byte is
/// left in place rather than reset.
///
/// # Safety
/// `canvas` must be a live `UCanvas*`.
#[inline]
pub unsafe fn canvas_set_color_rgb(canvas: *mut c_void, r: u8, g: u8, b: u8) {
    unsafe {
        let color = &mut (*canvas.cast::<UCanvas>()).color;
        color[0] = r;
        color[1] = g;
        color[2] = b;
    }
}
