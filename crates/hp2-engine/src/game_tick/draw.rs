//! `UGameEngine::Draw` body (oracle 1678–1889). The C++ shell keeps the
//! `GIsRunning`/`GIsDrawing` gate and the `FPushMemTag`, and calls [`draw`]
//! with `__FILE__` as `source_file`: `check()` reports that path, so it must
//! expand to the compile-time file path as the oracle does.

use std::ffi::{c_char, c_void};

use super::{
    AXIS_X, AXIS_Y, Hp2GameTickServices as S, LEVACT_NONE, LOCKR_CLEAR_SCREEN,
    apply_console_ui_scale, clamp, get_console_ui_scale, max, min,
};
use crate::math::app_round;

/// Host operations for the shared `UGameEngine::Draw` policy.
///
/// The native adapter and canonical Core adapter both run this one source-order
/// algorithm; the latter implements leaves with typed Rust owners.
pub(crate) trait DrawOps {
    type Error;

    unsafe fn update_connecting_message(&self, engine: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn view_calc(
        &self,
        viewport: *mut c_void,
        location: *mut f32,
        rotation: *mut i32,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn fail_assert(
        &self,
        expression: *const c_char,
        file: *const c_char,
        line: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn render_precache(&self, render: *mut c_void, viewport: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn level_point_check_viewer(
        &self,
        level: *mut c_void,
        location: *const f32,
    ) -> Result<i32, Self::Error>;
    unsafe fn viewport_lock(
        &self,
        viewport: *mut c_void,
        flash_scale: *const f32,
        flash_fog: *const f32,
        screen_clear: *const f32,
        flags: u32,
        hit_data: *mut u8,
        hit_size: *mut i32,
    ) -> Result<i32, Self::Error>;
    unsafe fn render_create_master_frame(
        &self,
        render: *mut c_void,
        viewport: *mut c_void,
        location: *const f32,
        rotation: *const i32,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn app_cycles(&self) -> Result<u32, Self::Error>;
    unsafe fn audio_update(&self, audio: *mut c_void, frame: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn render_pre_render(&self, render: *mut c_void, frame: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn console_pre_render(
        &self,
        console: *mut c_void,
        frame: *mut c_void,
        viewport: *mut c_void,
    ) -> Result<(), Self::Error>;
    unsafe fn canvas_update(&self, canvas: *mut c_void, frame: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn canvas_set_render(&self, canvas: *mut c_void, render: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn actor_event_pre_render(&self, actor: *mut c_void, canvas: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn console_get_draw_world(&self, console: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn render_draw_world(&self, render: *mut c_void, frame: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn renderdev_end_flash(&self, device: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn renderdev_begin_ui(
        &self,
        _device: *mut c_void,
        _frame: *mut c_void,
    ) -> Result<(), Self::Error> {
        // URenderDevice::BeginUI is the inherited empty source implementation.
        Ok(())
    }
    unsafe fn actor_event_post_render(&self, actor: *mut c_void, canvas: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn viewport_ui_scale(&self, viewport: *mut c_void) -> Result<f32, Self::Error>;
    unsafe fn apply_console_ui_scale(&self, viewport: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn console_post_render(&self, console: *mut c_void, frame: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn console_event_post_render(&self, console: *mut c_void, canvas: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn in_game_overlay(&self, _viewport: *mut c_void, _canvas: *mut c_void) -> Result<(), Self::Error> { Ok(()) }
    unsafe fn replay_replaying(&self, engine: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn replay_recording(&self, engine: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn canvas_replay_text(
        &self,
        engine: *mut c_void,
        canvas: *mut c_void,
        recording: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn audio_post_render(&self, audio: *mut c_void, frame: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn canvas_strlen_fps(
        &self,
        canvas: *mut c_void,
        fps: i32,
        width: *mut i32,
        height: *mut i32,
    ) -> Result<(), Self::Error>;
    unsafe fn canvas_printf_fps(&self, canvas: *mut c_void, fps: i32)
        -> Result<(), Self::Error>;
    unsafe fn render_post_render(&self, render: *mut c_void, frame: *mut c_void)
        -> Result<(), Self::Error>;
    unsafe fn viewport_unlock(&self, viewport: *mut c_void, blit: i32)
        -> Result<(), Self::Error>;
    unsafe fn render_finish_master_frame(
        &self,
        render: *mut c_void,
        frame: *mut c_void,
    ) -> Result<(), Self::Error>;
}

impl DrawOps for S {
    type Error = i32;

    unsafe fn update_connecting_message(&self, e: *mut c_void) -> Result<(), i32> {
        unsafe { S::update_connecting_message(self, e) }
    }
    unsafe fn view_calc(&self, vp: *mut c_void, loc: *mut f32, rot: *mut i32) -> Result<*mut c_void, i32> {
        unsafe { S::view_calc(self, vp, loc, rot) }
    }
    unsafe fn fail_assert(&self, expr: *const c_char, file: *const c_char, line: i32) -> Result<(), i32> {
        unsafe { S::fail_assert(self, expr, file, line) }
    }
    unsafe fn render_precache(&self, render: *mut c_void, vp: *mut c_void) -> Result<(), i32> {
        unsafe { S::render_precache(self, render, vp) }
    }
    unsafe fn level_point_check_viewer(&self, level: *mut c_void, loc: *const f32) -> Result<i32, i32> {
        unsafe { S::level_point_check_viewer(self, level, loc) }
    }
    unsafe fn viewport_lock(&self, vp: *mut c_void, scale: *const f32, fog: *const f32, clear: *const f32, flags: u32, data: *mut u8, size: *mut i32) -> Result<i32, i32> {
        unsafe { S::viewport_lock(self, vp, scale, fog, clear, flags, data, size) }
    }
    unsafe fn render_create_master_frame(&self, render: *mut c_void, vp: *mut c_void, loc: *const f32, rot: *const i32) -> Result<*mut c_void, i32> {
        unsafe { S::render_create_master_frame(self, render, vp, loc, rot) }
    }
    unsafe fn app_cycles(&self) -> Result<u32, i32> { unsafe { S::app_cycles(self) } }
    unsafe fn audio_update(&self, audio: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::audio_update(self, audio, frame) }
    }
    unsafe fn render_pre_render(&self, render: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::render_pre_render(self, render, frame) }
    }
    unsafe fn console_pre_render(
        &self,
        console: *mut c_void,
        frame: *mut c_void,
        _viewport: *mut c_void,
    ) -> Result<(), i32> {
        unsafe { S::console_pre_render(self, console, frame) }
    }
    unsafe fn canvas_update(&self, canvas: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::canvas_update(self, canvas, frame) }
    }
    unsafe fn canvas_set_render(&self, canvas: *mut c_void, render: *mut c_void) -> Result<(), i32> {
        unsafe { super::canvas_set_render(canvas, render); }
        Ok(())
    }
    unsafe fn actor_event_pre_render(&self, actor: *mut c_void, canvas: *mut c_void) -> Result<(), i32> {
        unsafe { S::actor_event_pre_render(self, actor, canvas) }
    }
    unsafe fn console_get_draw_world(&self, console: *mut c_void) -> Result<i32, i32> {
        unsafe { S::console_get_draw_world(self, console) }
    }
    unsafe fn render_draw_world(&self, render: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::render_draw_world(self, render, frame) }
    }
    unsafe fn renderdev_end_flash(&self, rd: *mut c_void) -> Result<(), i32> {
        unsafe { S::renderdev_end_flash(self, rd) }
    }
    unsafe fn renderdev_begin_ui(&self, rd: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::renderdev_begin_ui(self, rd, frame) }
    }
    unsafe fn actor_event_post_render(&self, actor: *mut c_void, canvas: *mut c_void) -> Result<(), i32> {
        unsafe { S::actor_event_post_render(self, actor, canvas) }
    }
    unsafe fn viewport_ui_scale(&self, vp: *mut c_void) -> Result<f32, i32> {
        unsafe { S::viewport_ui_scale(self, vp) }
    }
    unsafe fn apply_console_ui_scale(&self, vp: *mut c_void) -> Result<(), i32> {
        unsafe { super::apply_console_ui_scale(self, vp) }
    }
    unsafe fn console_post_render(&self, console: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::console_post_render(self, console, frame) }
    }
    unsafe fn console_event_post_render(&self, console: *mut c_void, canvas: *mut c_void) -> Result<(), i32> {
        unsafe { S::console_event_post_render(self, console, canvas) }
    }
    unsafe fn replay_replaying(&self, e: *mut c_void) -> Result<i32, i32> {
        unsafe { S::replay_replaying(self, e) }
    }
    unsafe fn replay_recording(&self, e: *mut c_void) -> Result<i32, i32> {
        unsafe { S::replay_recording(self, e) }
    }
    unsafe fn canvas_replay_text(&self, e: *mut c_void, canvas: *mut c_void, recording: i32) -> Result<(), i32> {
        unsafe { S::canvas_replay_text(self, e, canvas, recording) }
    }
    unsafe fn audio_post_render(&self, audio: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::audio_post_render(self, audio, frame) }
    }
    unsafe fn canvas_strlen_fps(&self, canvas: *mut c_void, fps: i32, width: *mut i32, height: *mut i32) -> Result<(), i32> {
        unsafe { S::canvas_strlen_fps(self, canvas, fps, width, height) }
    }
    unsafe fn canvas_printf_fps(&self, canvas: *mut c_void, fps: i32) -> Result<(), i32> {
        unsafe { S::canvas_printf_fps(self, canvas, fps) }
    }
    unsafe fn render_post_render(&self, render: *mut c_void, frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::render_post_render(self, render, frame) }
    }
    unsafe fn viewport_unlock(&self, vp: *mut c_void, blit: i32) -> Result<(), i32> {
        unsafe { S::viewport_unlock(self, vp, blit) }
    }
    unsafe fn render_finish_master_frame(&self, render: *mut c_void, _frame: *mut c_void) -> Result<(), i32> {
        unsafe { S::render_finish_master_frame(self, render) }
    }
}

/// Body of `UGameEngine::Draw` after the shell's entry gate and `FPushMemTag`.
///
/// # Safety
/// Engine handles must be the live objects the C++ shell passes. A null
/// `viewport` dereferences as the original does.
pub unsafe fn draw(
    s: &S,
    e: *mut c_void,
    viewport: *mut c_void,
    blit: i32,
    hit_data: *mut u8,
    hit_size: *mut i32,
    source_file: *const c_char,
) -> Result<(), i32> {
    unsafe { draw_with(s, e, viewport, blit, hit_data, hit_size, source_file) }
}

/// Runs the source-order Draw algorithm against native or canonical host operations.
///
/// # Safety
/// Engine handles must be live; null `viewport` dereferences as the source does.
pub(crate) unsafe fn draw_with<O: DrawOps>(
    s: &O,
    e: *mut c_void,
    viewport: *mut c_void,
    blit: i32,
    hit_data: *mut u8,
    hit_size: *mut i32,
    source_file: *const c_char,
) -> Result<(), O::Error> {
    unsafe {
        s.update_connecting_message(e)?;

        // ViewActor=Viewport->Actor; ViewLocation/Rotation; eventPlayerCalcView; check(ViewActor).
        let mut view_location = [0f32; 3];
        let mut view_rotation = [0i32; 3];
        // eventPlayerCalcView takes ViewActor by reference; use the updated actor.
        let view_actor = s.view_calc(
            viewport,
            view_location.as_mut_ptr(),
            view_rotation.as_mut_ptr(),
        )?;
        if view_actor.is_null() {
            s.fail_assert(c"ViewActor".as_ptr(), source_file, 1694)?;
        }

        // Precache on flip.
        let rd = unsafe { super::viewport_renderdev(viewport) };
        if unsafe { super::renderdev_precache_on_flip(rd) }
            && unsafe { super::viewport_suspend_precaching(viewport) } == 0
            && unsafe { super::actor_level_action(view_actor) } == LEVACT_NONE
        {
            unsafe { super::set_renderdev_precache_on_flip(rd, false) };
            if unsafe { super::actor_level_never_precache(view_actor) } == 0 {
                s.render_precache(unsafe { super::engine_render(e) }, viewport)?;
            }
        }

        // LockFlags=0; if( !GLevel->Model->PointCheck(Hit,NULL,ViewLocation,FVector(0,0,0),0) ) LockFlags |= LOCKR_ClearScreen.
        let mut lock_flags: u32 = 0;
        if s.level_point_check_viewer(unsafe { super::g_level(e) }, view_location.as_ptr())? == 0 {
            lock_flags |= LOCKR_CLEAR_SCREEN;
        }

        let render = unsafe { super::engine_render(e) };
        if render.is_null() {
            s.fail_assert(c"Render".as_ptr(), source_file, 1732)?;
        }

        // FlashScale / FlashFog.
        let client = unsafe { super::engine_client(e) };
        let screen_flashes = unsafe { super::client_screen_flashes(client) } != 0;
        let mut fog = [0f32; 4];
        let mut scale = 0.5f32;
        if screen_flashes {
            let actor = unsafe { super::viewport_actor(viewport) };
            unsafe { super::actor_flash_fog(actor, fog.as_mut_ptr()) };
            scale = clamp(0.5 * fog[3], 0.0, 1.0);
        }
        // FlashFog = ScreenFlashes ? Actor->FlashFog : FVector(0,0,0); FPlane(FVector) sets W=0.
        for c in fog.iter_mut().take(3) {
            *c = clamp(*c, 0.0, 1.0);
        }
        fog[3] = 0.0;
        let flash_scale = [scale; 4];
        let screen_clear = [0f32; 4];

        if s.viewport_lock(
            viewport,
            flash_scale.as_ptr(),
            fog.as_ptr(),
            screen_clear.as_ptr(),
            lock_flags,
            hit_data,
            hit_size,
        )? != 0
        {
            let mut viewport_guard = ActiveViewportLock {
                ops: s,
                viewport,
                blit,
                active: true,
            };
            draw_locked(
                s,
                e,
                viewport,
                blit,
                render,
                rd,
                &view_location,
                &view_rotation,
                &mut viewport_guard,
            )?;
        }

        unsafe { super::set_is_drawing(e, 0) };
        Ok(())
    }
}

/// Body inside `if( Viewport->Lock(...) )` (oracle 1716–1884).
///
/// # Safety
/// As [`draw`].
#[allow(clippy::too_many_arguments)]
unsafe fn draw_locked<O: DrawOps>(
    s: &O,
    e: *mut c_void,
    viewport: *mut c_void,
    blit: i32,
    render: *mut c_void,
    rd: *mut c_void,
    view_location: &[f32; 3],
    view_rotation: &[i32; 3],
    viewport_guard: &mut ActiveViewportLock<'_, O>,
) -> Result<(), O::Error> {
    unsafe {
        let frame = s.render_create_master_frame(
            render,
            viewport,
            view_location.as_ptr(),
            view_rotation.as_ptr(),
        )?;
        let mut frame_guard = ActiveMasterFrame {
            ops: s,
            render,
            frame,
            active: true,
        };

        // Audio update, clocked against GLevel->AudioTickCycles.
        let audio = unsafe { super::engine_audio(e) };
        if !audio.is_null() {
            let level = unsafe { super::g_level(e) };
            let v = clock(s, unsafe { super::level_get_audio_tick_cycles(level) })?;
            unsafe { super::level_set_audio_tick_cycles(level, v) };
            s.audio_update(audio, frame)?;
            let level = unsafe { super::g_level(e) };
            let v = unclock(s, unsafe { super::level_get_audio_tick_cycles(level) })?;
            unsafe { super::level_set_audio_tick_cycles(level, v) };
        }

        s.render_pre_render(render, frame)?;
        let canvas = unsafe { super::viewport_canvas(viewport) };
        s.canvas_set_render(canvas, render)?;
        let mut canvas_guard = ActiveCanvasRender { ops: s, canvas, active: true };
        let console = unsafe { super::viewport_console(viewport) };
        if !console.is_null() {
            s.console_pre_render(console, frame, viewport)?;
        }
        s.canvas_update(unsafe { super::viewport_canvas(viewport) }, frame)?;
        s.actor_event_pre_render(
            unsafe { super::viewport_actor(viewport) },
            unsafe { super::viewport_canvas(viewport) },
        )?;
        if unsafe { super::frame_x(frame) } > 0
            && unsafe { super::frame_y(frame) } > 0
            && (console.is_null() || s.console_get_draw_world(console)? != 0)
        {
            s.render_draw_world(render, frame)?;
        }

        s.renderdev_end_flash(rd)?;
        s.renderdev_begin_ui(rd, frame)?;
        s.actor_event_post_render(unsafe { super::viewport_actor(viewport) }, unsafe {
            super::viewport_canvas(viewport)
        })?;

        // Console UI scale block (oracle 1764–1800).
        if !console.is_null() {
            console_block(s, viewport, frame, console)?;
        }

        #[cfg(feature = "in-game-overlay")]
        s.in_game_overlay(viewport, unsafe { super::viewport_canvas(viewport) })?;

        // Replay status text (oracle 1801–1811).
        if s.replay_replaying(e)? != 0 || s.replay_recording(e)? != 0 {
            let canvas = unsafe { super::viewport_canvas(viewport) };
            unsafe { super::canvas_set_color_rgb(canvas, 255, 255, 255) };
            unsafe { super::set_canvas_cur_x(canvas, 0.0) };
            unsafe { super::set_canvas_cur_y(canvas, super::canvas_clip_y(canvas) - 40.0) };
            s.canvas_replay_text(e, canvas, s.replay_recording(e)?)?;
        }

        if !audio.is_null() {
            s.audio_post_render(audio, frame)?;
        }

        fps_block(s, e, viewport)?;

        canvas_guard.clear()?;
        s.render_post_render(render, frame)?;
        viewport_guard.unlock()?;
        frame_guard.finish()?;
        Ok(())
    }
}

struct ActiveViewportLock<'a, O: DrawOps> {
    ops: &'a O,
    viewport: *mut c_void,
    blit: i32,
    active: bool,
}

impl<O: DrawOps> ActiveViewportLock<'_, O> {
    unsafe fn unlock(&mut self) -> Result<(), O::Error> {
        let result = unsafe { self.ops.viewport_unlock(self.viewport, self.blit) };
        if result.is_ok() {
            self.active = false;
        }
        result
    }
}

impl<O: DrawOps> Drop for ActiveViewportLock<'_, O> {
    fn drop(&mut self) {
        if self.active {
            // Preserve the primary Draw failure while still releasing the viewport lock.
            let _ = unsafe { self.ops.viewport_unlock(self.viewport, self.blit) };
        }
    }
}

struct ActiveMasterFrame<'a, O: DrawOps> {
    ops: &'a O,
    render: *mut c_void,
    frame: *mut c_void,
    active: bool,
}

impl<O: DrawOps> ActiveMasterFrame<'_, O> {
    unsafe fn finish(&mut self) -> Result<(), O::Error> {
        // The renderer owner unpublishes even when finishing reports an error.
        self.active = false;
        unsafe { self.ops.render_finish_master_frame(self.render, self.frame) }
    }
}

impl<O: DrawOps> Drop for ActiveMasterFrame<'_, O> {
    fn drop(&mut self) {
        if self.active {
            let _ = unsafe { self.ops.render_finish_master_frame(self.render, self.frame) };
        }
    }
}

struct ActiveCanvasRender<'a, O: DrawOps> {
    ops: &'a O,
    canvas: *mut c_void,
    active: bool,
}

impl<O: DrawOps> ActiveCanvasRender<'_, O> {
    unsafe fn clear(&mut self) -> Result<(), O::Error> {
        let result = unsafe { self.ops.canvas_set_render(self.canvas, std::ptr::null_mut()) };
        if result.is_ok() {
            self.active = false;
        }
        result
    }
}

impl<O: DrawOps> Drop for ActiveCanvasRender<'_, O> {
    fn drop(&mut self) {
        if self.active {
            let _ = unsafe { self.ops.canvas_set_render(self.canvas, std::ptr::null_mut()) };
        }
    }
}

struct ConsoleRenderState {
    canvas: *mut c_void,
    viewport: *mut c_void,
    org_x: f32,
    org_y: f32,
    clip_x: f32,
    clip_y: f32,
    size_x: f32,
    size_y: f32,
    mouse_x: f32,
    mouse_y: f32,
    active: bool,
}

impl ConsoleRenderState {
    unsafe fn restore(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        unsafe {
            super::set_canvas_org_x(self.canvas, self.org_x);
            super::set_canvas_org_y(self.canvas, self.org_y);
            super::set_canvas_clip_x(self.canvas, self.clip_x);
            super::set_canvas_clip_y(self.canvas, self.clip_y);
            super::set_canvas_size_x(self.canvas, self.size_x as i32);
            super::set_canvas_size_y(self.canvas, self.size_y as i32);
            super::set_viewport_mouse(self.viewport, AXIS_X, self.mouse_x);
            super::set_viewport_mouse(self.viewport, AXIS_Y, self.mouse_y);
        }
    }
}

impl Drop for ConsoleRenderState {
    fn drop(&mut self) {
        unsafe { self.restore() };
    }
}

/// Oracle 1764–1800: draw the console with the 640x480-scaled clip region.
///
/// # Safety
/// As [`draw`]. `console` must be non-null.
unsafe fn console_block<O: DrawOps>(
    s: &O,
    viewport: *mut c_void,
    frame: *mut c_void,
    console: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let canvas = unsafe { super::viewport_canvas(viewport) };
        let saved_org_x = unsafe { super::canvas_org_x(canvas) };
        let saved_org_y = unsafe { super::canvas_org_y(canvas) };
        let saved_clip_x = unsafe { super::canvas_clip_x(canvas) };
        let saved_clip_y = unsafe { super::canvas_clip_y(canvas) };
        let saved_mouse_x = unsafe { super::viewport_mouse(viewport, AXIS_X) };
        let saved_size_x = unsafe { super::canvas_size_x(canvas) };
        let saved_size_y = unsafe { super::canvas_size_y(canvas) };
        let saved_mouse_y = unsafe { super::viewport_mouse(viewport, AXIS_Y) };
        let mut saved_state = ConsoleRenderState {
            canvas,
            viewport,
            org_x: saved_org_x,
            org_y: saved_org_y,
            clip_x: saved_clip_x,
            clip_y: saved_clip_y,
            size_x: saved_size_x as f32,
            size_y: saved_size_y as f32,
            mouse_x: saved_mouse_x,
            mouse_y: saved_mouse_y,
            active: true,
        };

        let scale =
            get_console_ui_scale(saved_clip_x, saved_clip_y, s.viewport_ui_scale(viewport)?);
        let ui_width = min(saved_clip_x, 640.0 * scale);
        let ui_height = min(saved_clip_y, 480.0 * scale);
        let offset_x = max(0.0, (saved_clip_x - ui_width) * 0.5);
        let offset_y = max(0.0, (saved_clip_y - ui_height) * 0.5);

        unsafe { super::set_canvas_org_x(canvas, saved_org_x + offset_x) };
        unsafe { super::set_canvas_org_y(canvas, saved_org_y + offset_y) };
        unsafe { super::set_canvas_clip_x(canvas, ui_width) };
        unsafe { super::set_canvas_size_x(canvas, app_round(ui_width)) };
        unsafe { super::set_canvas_size_y(canvas, app_round(ui_height)) };
        unsafe { super::set_canvas_clip_y(canvas, ui_height) };
        unsafe { super::set_viewport_mouse(viewport, AXIS_X, saved_mouse_x - offset_x) };
        unsafe { super::set_viewport_mouse(viewport, AXIS_Y, saved_mouse_y - offset_y) };
        s.apply_console_ui_scale(viewport)?;
        s.console_post_render(console, frame)?;
        s.console_event_post_render(console, canvas)?;

        saved_state.restore();
        Ok(())
    }
}

/// Oracle 1847–1879: frames-per-second overlay.
///
/// # Safety
/// As [`draw`].
unsafe fn fps_block<O: DrawOps>(
    s: &O,
    e: *mut c_void,
    viewport: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let fps = app_round(unsafe { super::engine_current_tick_rate(e) });
        if unsafe { super::client_show_fps(super::engine_client(e)) } == 0 || fps <= 0 {
            return Ok(());
        }
        let canvas = unsafe { super::viewport_canvas(viewport) };
        let saved_org_x = unsafe { super::canvas_org_x(canvas) };
        let saved_org_y = unsafe { super::canvas_org_y(canvas) };
        let saved_clip_x = unsafe { super::canvas_clip_x(canvas) };
        let saved_clip_y = unsafe { super::canvas_clip_y(canvas) };
        let saved_cur_x = unsafe { super::canvas_cur_x(canvas) };
        let saved_cur_y = unsafe { super::canvas_cur_y(canvas) };
        let saved_color = unsafe { super::canvas_color(canvas) };

        unsafe { super::set_canvas_org_x(canvas, 0.0) };
        unsafe { super::set_canvas_org_y(canvas, 0.0) };
        unsafe { super::set_canvas_clip_x(canvas, super::canvas_size_x(canvas) as f32) };
        unsafe { super::set_canvas_clip_y(canvas, super::canvas_size_y(canvas) as f32) };
        unsafe { super::set_canvas_cur_x(canvas, 0.0) };
        unsafe { super::set_canvas_cur_y(canvas, 0.0) };

        let mut text_width = 0i32;
        let mut text_height = 0i32;
        s.canvas_strlen_fps(canvas, fps, &mut text_width, &mut text_height)?;
        // CurX = ClipX - TextWidth - 12.f, evaluated left to right.
        let clip_x = unsafe { super::canvas_clip_x(canvas) };
        unsafe { super::set_canvas_cur_x(canvas, (clip_x - text_width as f32) - 12.0) };
        unsafe { super::set_canvas_cur_y(canvas, 12.0) };
        unsafe { super::set_canvas_color(canvas, [190, 198, 204, 255]) };
        s.canvas_printf_fps(canvas, fps)?;

        unsafe { super::set_canvas_org_x(canvas, saved_org_x) };
        unsafe { super::set_canvas_org_y(canvas, saved_org_y) };
        unsafe { super::set_canvas_clip_x(canvas, saved_clip_x) };
        unsafe { super::set_canvas_clip_y(canvas, saved_clip_y) };
        unsafe { super::set_canvas_cur_x(canvas, saved_cur_x) };
        unsafe { super::set_canvas_cur_y(canvas, saved_cur_y) };
        unsafe { super::set_canvas_color(canvas, saved_color) };
        Ok(())
    }
}

/// Clock on a field: `T -= appCycles()` (`UnFile.h:198`).
#[inline]
unsafe fn clock<O: DrawOps>(s: &O, t: i32) -> Result<i32, O::Error> {
    unsafe { Ok(t.wrapping_sub(s.app_cycles()? as i32)) }
}

/// Unclock on a field: `T += appCycles()-34` (`UnFile.h:199`).
#[inline]
unsafe fn unclock<O: DrawOps>(s: &O, t: i32) -> Result<i32, O::Error> {
    unsafe { Ok(t.wrapping_add(s.app_cycles()?.wrapping_sub(34) as i32)) }
}
