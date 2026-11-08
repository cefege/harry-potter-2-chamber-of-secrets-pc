//! Canonical Core entry for `UGameEngine::Draw`.

use hp2_abi::{
    FPlane, FVector,
    engine::generated::{
        AActor, FCheckResult, FRotator, UFunction, UProperty, UObject, URender,
        URenderDevice, USDLClient, UViewport, UGameEngine, ULevel, FSceneNode,
    },
};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
};
use std::ptr;

#[cfg(feature = "in-game-overlay")]
unsafe fn with_overlay_canvas<T, E>(canvas: *mut hp2_abi::engine::generated::UCanvas, draw: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    // WrappedPrint changes these fields even when its host reports an error.
    let saved = unsafe { ((*canvas).cur_x, (*canvas).cur_y, (*canvas).cur_yl, (*canvas).clip_x,
        (*canvas).clip_y, (*canvas).org_x, (*canvas).org_y, (*canvas).color) };
    let result = draw();
    unsafe { ((*canvas).cur_x, (*canvas).cur_y, (*canvas).cur_yl, (*canvas).clip_x,
        (*canvas).clip_y, (*canvas).org_x, (*canvas).org_y, (*canvas).color) = saved; }
    result
}

#[cfg(all(test, feature = "in-game-overlay"))]
mod overlay_tests {
    use super::*;
    #[test]
    fn canvas_state_restored_after_success_and_error() {
        let mut canvas: hp2_abi::engine::generated::UCanvas = unsafe { std::mem::zeroed() };
        canvas.cur_x = 3.0; canvas.clip_y = 72.0; canvas.color = [1, 2, 3, 4];
        let p = &mut canvas as *mut _;
        for fails in [false, true] {
            let result: Result<(), ()> = unsafe { with_overlay_canvas(p, || {
                (*p).cur_x = 10.0; (*p).clip_y = 99.0; (*p).color = [9; 4];
                if fails { Err(()) } else { Ok(()) }
            }) };
            assert_eq!(result.is_err(), fails);
            let (x, clip, color) = (canvas.cur_x, canvas.clip_y, canvas.color);
            assert_eq!((x, clip, color), (3.0, 72.0, [1, 2, 3, 4]));
        }
    }
}


#[repr(C)]
pub(crate) struct PlayerCalcViewParams {
    pub view_actor: *mut AActor,
    pub camera_location: FVector,
    pub camera_rotation: FRotator,
}

unsafe fn event_function(
    access: CoreAccess<'_>,
    object: *mut UObject,
    event: &str,
) -> RuntimeResult<*mut UFunction> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical Draw event object"));
    }
    let name = access.intern_str(event, FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(object, name, false)? };
    if function.is_null() {
        return Err(RuntimeError::new("Canonical Draw event is missing"));
    }
    Ok(function)
}

unsafe fn player_calc_view(
    access: CoreAccess<'_>,
    actor: *mut AActor,
) -> RuntimeResult<PlayerCalcViewParams> {
    if actor.is_null() {
        return Err(RuntimeError::new("Null canonical draw view actor"));
    }
    let mut params = PlayerCalcViewParams {
        view_actor: actor,
        camera_location: unsafe { (*actor).location },
        camera_rotation: unsafe { (*actor).rotation },
    };
    unsafe { player_calc_view_params(access, actor, &mut params)?; }
    Ok(params)
}

/// Shared generated event wrapper for drawing, networking and ScreenToWorld.
///
/// # Safety
/// Actor, reflected function and caller-owned parameter frame remain live through script.
pub(crate) unsafe fn player_calc_view_params(
    access: CoreAccess<'_>, actor: *mut AActor, params: &mut PlayerCalcViewParams,
) -> RuntimeResult<()> {
    let function = unsafe { event_function(access, actor.cast(), "PlayerCalcView")? };
    if unsafe { (*function).parms_size as usize } != std::mem::size_of::<PlayerCalcViewParams>() {
        return Err(RuntimeError::new("Canonical PlayerCalcView parameters have an unexpected layout"));
    }
    unsafe { crate::actor_owner::process_event(access, actor, function, ptr::from_mut(params).cast(), ptr::null_mut()) }
}

#[repr(C)]
struct CanvasEventParams {
    canvas: *mut hp2_abi::engine::generated::UCanvas,
}

unsafe fn actor_canvas_event(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    canvas: *mut hp2_abi::engine::generated::UCanvas,
    event: &str,
) -> RuntimeResult<()> {
    let function = unsafe { event_function(access, actor.cast(), event)? };
    if unsafe { (*function).parms_size as usize } != std::mem::size_of::<CanvasEventParams>() {
        return Err(RuntimeError::new("Canonical actor render-event parameters have an unexpected layout"));
    }
    let mut params = CanvasEventParams { canvas };
    unsafe {
        crate::actor_owner::process_event(
            access,
            actor,
            function,
            (&mut params as *mut CanvasEventParams).cast(),
            std::ptr::null_mut(),
        )
    }
}

unsafe fn create_master_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    viewport: *mut UViewport,
    location: *const f32,
    rotation: *const i32,
) -> RuntimeResult<*mut FSceneNode> {
    if location.is_null() || rotation.is_null() {
        return Err(RuntimeError::new("Canonical Draw has invalid master-frame view parameters"));
    }
    let location = unsafe { std::ptr::read_unaligned(location.cast::<FVector>()) };
    let rotation = unsafe { std::ptr::read_unaligned(rotation.cast::<FRotator>()) };
    unsafe { crate::render_owner::create_master_frame(access, render, viewport, location, rotation) }
}

unsafe fn finish_master_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    unsafe { crate::render_owner::finish_master_frame(access, render, frame) }
}

unsafe fn level_point_check_viewer(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    location: *const f32,
) -> RuntimeResult<bool> {
    if level.is_null() || location.is_null() {
        return Err(RuntimeError::new("Invalid canonical viewer point-check input"));
    }
    let model: *mut hp2_abi::engine::generated::UModel = unsafe { (*level).model.cast() };
    if model.is_null() {
        return Err(RuntimeError::new("Canonical Draw level has no model for the viewer point-check"));
    }
    let location = unsafe { std::ptr::read_unaligned(location.cast::<FVector>()) };
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    unsafe {
        crate::collision_owner::world_point_check(
            access,
            model,
            &mut hit,
            location,
            FVector { x: 0.0, y: 0.0, z: 0.0 },
            0,
        )
    }
}

unsafe fn lock_viewport(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    flash_scale: *const f32,
    flash_fog: *const f32,
    screen_clear: *const f32,
    flags: u32,
    hit_data: *mut u8,
    hit_size: *mut i32,
) -> RuntimeResult<bool> {
    if viewport.is_null()
        || flash_scale.is_null()
        || flash_fog.is_null()
        || screen_clear.is_null()
    {
        return Err(RuntimeError::new("Invalid canonical Viewport::Lock input"));
    }
    unsafe {
        crate::platform_owner::lock_viewport(
            access,
            viewport,
            flash_scale.cast::<FPlane>(),
            flash_fog.cast::<FPlane>(),
            screen_clear.cast::<FPlane>(),
            flags,
            hit_data.cast(),
            hit_size,
        )
    }
}

unsafe fn compute_render_size(
    access: CoreAccess<'_>,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    let mut leaf_error = None;
    let result = unsafe {
        crate::camera::compute_render_size_with(frame, |viewport, frame| {
            let device = unsafe {
                std::ptr::addr_of!((*viewport).ren_dev)
                    .read_unaligned()
                    .cast::<URenderDevice>()
            };
            match unsafe { crate::render_device_owner::set_scene_node(access, device, frame) } {
                Ok(()) => Ok(()),
                Err(error) => {
                    leaf_error = Some(error);
                    Err(-1)
                }
            }
        })
    };
    if let Some(error) = leaf_error {
        return Err(error);
    }
    result.map_err(|code| {
        RuntimeError::new(&format!("Core FSceneNode::ComputeRenderSize failed with code {code}"))
    })
}

unsafe fn unlock_viewport(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    blit: i32,
) -> RuntimeResult<()> {
    unsafe { crate::platform_owner::unlock_viewport(access, viewport, blit != 0) }
}


unsafe fn reflected_property(
    access: CoreAccess<'_>,
    object: *mut UObject,
    name: &str,
    property_class: &str,
) -> RuntimeResult<*mut UProperty> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical object for reflected Draw property"));
    }
    let name = access.intern_str(name, FindName::Find)?;
    let property_class = access.native_class(property_class)?;
    let class = unsafe { std::ptr::addr_of!((*object).class).read_unaligned().cast() };
    Ok(unsafe { access.find_field(class, name, property_class) }.cast())
}

unsafe fn console_root(access: CoreAccess<'_>, console: *mut UObject) -> RuntimeResult<*mut UObject> {
    let property = unsafe { reflected_property(access, console, "Root", "ObjectProperty")? };
    if property.is_null() {
        return Ok(std::ptr::null_mut());
    }
    let offset = unsafe { std::ptr::addr_of!((*property).offset).read_unaligned() };
    if offset < 0 {
        return Err(RuntimeError::new("Canonical Console Root property has a negative offset"));
    }
    let root = unsafe {
        console
            .cast::<u8>()
            .add(offset as usize)
            .cast::<*mut UObject>()
            .read_unaligned()
    };
    Ok(root)
}

unsafe fn root_scale_props(
    access: CoreAccess<'_>,
    root: *mut UObject,
    values: &mut [f32; 3],
) -> RuntimeResult<i32> {
    let names = ["RealWidth", "RealHeight", "GUIScale"];
    let mut fields = [std::ptr::null_mut::<UProperty>(); 3];
    for (index, name) in names.iter().enumerate() {
        fields[index] = unsafe { reflected_property(access, root, name, "FloatProperty")? };
        if fields[index].is_null() {
            return Ok(0);
        }
    }
    for (value, property) in values.iter_mut().zip(fields) {
        let offset = unsafe { std::ptr::addr_of!((*property).offset).read_unaligned() };
        if offset < 0 {
            return Err(RuntimeError::new("Canonical Console root scale property has a negative offset"));
        }
        *value = unsafe {
            root.cast::<u8>()
                .add(offset as usize)
                .cast::<f32>()
                .read_unaligned()
        };
    }
    Ok(1)
}

unsafe fn root_set_scale(access: CoreAccess<'_>, root: *mut UObject, scale: f32) -> RuntimeResult<()> {
    if root.is_null() {
        return Ok(());
    }
    let name = access.intern_str("SetScale", FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(root, name, false)? };
    if function.is_null() {
        return Ok(());
    }
    #[repr(C)]
    struct Params {
        new_scale: f32,
    }
    let mut params = Params { new_scale: scale };
    unsafe {
        crate::actor_owner::process_event(
            access,
            root.cast(),
            function,
            (&mut params as *mut Params).cast(),
            std::ptr::null_mut(),
        )
    }
}

unsafe fn viewport_ui_scale(viewport: *mut UViewport) -> RuntimeResult<f32> {
    if viewport.is_null() {
        return Err(RuntimeError::new("Null canonical Viewport UI-scale source"));
    }
    let client = unsafe { (*viewport).outer.cast::<USDLClient>() };
    if client.is_null() {
        return Err(RuntimeError::new("Canonical Viewport has no SDL client for UI scale"));
    }
    Ok(unsafe { std::ptr::addr_of!((*client).ui_scale).read_unaligned() })
}

unsafe fn apply_console_ui_scale(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
) -> RuntimeResult<()> {
    unsafe {
        crate::game_tick::apply_console_ui_scale_with(
            viewport.cast(),
            |console| console_root(access, console.cast()).map(|root| root.cast()),
            |root, values| root_scale_props(access, root.cast(), values),
            |viewport| viewport_ui_scale(viewport.cast()),
            |root, scale| root_set_scale(access, root.cast(), scale),
        )
    }
}

struct CoreDrawOps<'a> {
    access: CoreAccess<'a>,
}

impl crate::game_tick::draw::DrawOps for CoreDrawOps<'_> {
    type Error = RuntimeError;

    unsafe fn update_connecting_message(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { crate::game_engine_owner::update_connecting_message(self.access, engine.cast()) }
    }

    unsafe fn view_calc(
        &self,
        viewport: *mut std::ffi::c_void,
        location: *mut f32,
        rotation: *mut i32,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        let actor = unsafe { (*viewport.cast::<UViewport>()).actor.cast::<AActor>() };
        let view = unsafe { player_calc_view(self.access, actor)? };
        unsafe {
            std::ptr::write_unaligned(location.cast::<FVector>(), view.camera_location);
            std::ptr::write_unaligned(rotation.cast::<FRotator>(), view.camera_rotation);
        }
        Ok(view.view_actor.cast())
    }

    unsafe fn fail_assert(
        &self,
        expression: *const std::ffi::c_char,
        file: *const std::ffi::c_char,
        line: i32,
    ) -> RuntimeResult<()> {
        let expression = if expression.is_null() {
            "canonical Draw check failed".to_owned()
        } else {
            unsafe { std::ffi::CStr::from_ptr(expression) }
                .to_string_lossy()
                .into_owned()
        };
        let file = if file.is_null() {
            "UnGame.cpp".to_owned()
        } else {
            unsafe { std::ffi::CStr::from_ptr(file) }
                .to_string_lossy()
                .into_owned()
        };
        Err(RuntimeError::new(&format!(
            "Draw check failed: {expression} ({file}:{line})"
        )))
    }

    unsafe fn render_precache(
        &self,
        render: *mut std::ffi::c_void,
        viewport: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::render_owner::precache(self.access, render.cast(), viewport.cast())
        }
    }

    unsafe fn level_point_check_viewer(
        &self,
        level: *mut std::ffi::c_void,
        location: *const f32,
    ) -> RuntimeResult<i32> {
        unsafe {
            level_point_check_viewer(self.access, level.cast(), location)
                .map(i32::from)
        }
    }

    unsafe fn viewport_lock(
        &self,
        viewport: *mut std::ffi::c_void,
        flash_scale: *const f32,
        flash_fog: *const f32,
        screen_clear: *const f32,
        flags: u32,
        hit_data: *mut u8,
        hit_size: *mut i32,
    ) -> RuntimeResult<i32> {
        unsafe {
            lock_viewport(
                self.access,
                viewport.cast(),
                flash_scale,
                flash_fog,
                screen_clear,
                flags,
                hit_data,
                hit_size,
            )
            .map(i32::from)
        }
    }

    unsafe fn render_create_master_frame(
        &self,
        render: *mut std::ffi::c_void,
        viewport: *mut std::ffi::c_void,
        location: *const f32,
        rotation: *const i32,
    ) -> RuntimeResult<*mut std::ffi::c_void> {
        unsafe {
            create_master_frame(
                self.access,
                render.cast(),
                viewport.cast(),
                location,
                rotation,
            )
            .map(|frame| frame.cast())
        }
    }

    unsafe fn app_cycles(&self) -> RuntimeResult<u32> {
        Ok(hp2_core::unix_clock::cycles())
    }

    unsafe fn audio_update(
        &self,
        audio: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::audio_owner::update(
                self.access,
                audio.cast(),
                frame.cast::<hp2_abi::engine::generated::FSceneNode>(),
            )
        }
    }

    unsafe fn render_pre_render(
        &self,
        render: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::render_owner::pre_render(
                self.access,
                render.cast(),
                frame.cast(),
            )
        }
    }

    unsafe fn console_pre_render(
        &self,
        console: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
        _viewport: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::console_owner::pre_render_from_viewport(
                self.access,
                console.cast(),
                frame.cast(),
                |frame| compute_render_size(self.access, frame.cast()),
            )
        }
    }

    unsafe fn canvas_update(
        &self,
        canvas: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::canvas_owner::update(self.access, canvas.cast(), frame.cast())
        }
    }

    unsafe fn canvas_set_render(
        &self,
        canvas: *mut std::ffi::c_void,
        render: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe { crate::canvas_owner::set_render(canvas.cast(), render) }
    }

    unsafe fn actor_event_pre_render(
        &self,
        actor: *mut std::ffi::c_void,
        canvas: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe { actor_canvas_event(self.access, actor.cast(), canvas.cast(), "PreRender") }
    }

    unsafe fn console_get_draw_world(
        &self,
        console: *mut std::ffi::c_void,
    ) -> RuntimeResult<i32> {
        unsafe { crate::console_owner::get_draw_world(console.cast()) }
    }

    unsafe fn render_draw_world(
        &self,
        render: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe { crate::render_owner::draw_world(self.access, render.cast(), frame.cast()) }
    }

    unsafe fn renderdev_end_flash(&self, device: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { crate::render_device_owner::end_flash(self.access, device.cast()) }
    }

    unsafe fn actor_event_post_render(
        &self,
        actor: *mut std::ffi::c_void,
        canvas: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe { actor_canvas_event(self.access, actor.cast(), canvas.cast(), "PostRender") }
    }

    unsafe fn viewport_ui_scale(&self, viewport: *mut std::ffi::c_void) -> RuntimeResult<f32> {
        unsafe { viewport_ui_scale(viewport.cast()) }
    }

    unsafe fn apply_console_ui_scale(
        &self,
        viewport: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe { apply_console_ui_scale(self.access, viewport.cast()) }
    }

    unsafe fn console_post_render(
        &self,
        console: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::console_owner::post_render(self.access, console.cast(), frame.cast())
        }
    }

    unsafe fn console_event_post_render(
        &self,
        console: *mut std::ffi::c_void,
        canvas: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::console_owner::event_post_render(
                self.access,
                console.cast(),
                canvas.cast(),
            )
        }
    }

    #[cfg(feature = "in-game-overlay")]
    unsafe fn in_game_overlay(&self, viewport: *mut std::ffi::c_void, canvas: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let Some((content, selected)) = crate::in_game_overlay::snapshot(viewport as usize) else { return Ok(()); };
        let canvas = canvas.cast::<hp2_abi::engine::generated::UCanvas>();
        if canvas.is_null() { return Err(RuntimeError::new("Overlay has no Canvas")); }
        unsafe {
            let font = (*canvas).small_font.cast::<hp2_abi::engine::generated::UFont>();
            if font.is_null() { return Ok(()); }
            with_overlay_canvas(canvas, || {
            (*canvas).color = [255, 255, 255, 255];
            let mut lines = vec![content.title];
            lines.extend(content.status);
            for (i, action) in content.actions.iter().enumerate() {
                lines.push(format!("{} {}{}", if i == selected { ">" } else { " " }, action.label, if action.enabled { "" } else { " (unavailable)" }));
            }
                for (i, line) in lines.iter().enumerate() {
                    (*canvas).cur_x = 24.0;
                    (*canvas).cur_y = 24.0 + i as f32 * 22.0;
                    let mut text: Vec<u16> = line.encode_utf16().collect();
                    text.push(0);
                    crate::canvas_owner::wrapped_printf(self.access, canvas, font, false, &text)?;
                }
                Ok(())
            })
        }
    }

    unsafe fn replay_replaying(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let replay = unsafe { std::ptr::addr_of!((*engine.cast::<UGameEngine>()).replay) };
        Ok(i32::from(unsafe { crate::replay_owner::replaying(replay) }))
    }

    unsafe fn replay_recording(&self, engine: *mut std::ffi::c_void) -> RuntimeResult<i32> {
        let replay = unsafe { std::ptr::addr_of!((*engine.cast::<UGameEngine>()).replay) };
        Ok(i32::from(unsafe { crate::replay_owner::recording(replay) }))
    }

    unsafe fn canvas_replay_text(
        &self,
        engine: *mut std::ffi::c_void,
        canvas: *mut std::ffi::c_void,
        recording: i32,
    ) -> RuntimeResult<()> {
        let game = engine.cast::<UGameEngine>();
        let replay = unsafe { std::ptr::addr_of!((*game).replay) };
        let frame = unsafe { crate::replay_owner::frame_num(replay) };
        let level = unsafe { (*game).g_level.cast::<hp2_abi::engine::generated::ULevel>() };
        if level.is_null() {
            return Err(RuntimeError::new("Canonical Draw replay status has no current level"));
        }
        let time_raw = unsafe { std::ptr::addr_of!((*level).time_seconds).read_unaligned() };
        let time = hp2_core::time::get_float(i64::from_ne_bytes(time_raw));
        unsafe {
            crate::canvas_owner::canvas_replay_text(
                self.access,
                canvas.cast(),
                recording != 0,
                frame,
                time,
            )
        }
    }

    unsafe fn audio_post_render(
        &self,
        audio: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        crate::audio_owner::post_render(
            self.access,
            audio.cast(),
            frame.cast::<hp2_abi::engine::generated::FSceneNode>(),
        )
    }

    unsafe fn canvas_strlen_fps(
        &self,
        canvas: *mut std::ffi::c_void,
        fps: i32,
        width: *mut i32,
        height: *mut i32,
    ) -> RuntimeResult<()> {
        let (measured_width, measured_height) =
            unsafe { crate::canvas_owner::canvas_strlen_fps(self.access, canvas.cast(), fps)? };
        unsafe {
            std::ptr::write_unaligned(width, measured_width);
            std::ptr::write_unaligned(height, measured_height);
        }
        Ok(())
    }

    unsafe fn canvas_printf_fps(
        &self,
        canvas: *mut std::ffi::c_void,
        fps: i32,
    ) -> RuntimeResult<()> {
        unsafe { crate::canvas_owner::canvas_printf_fps(self.access, canvas.cast(), fps) }
    }

    unsafe fn render_post_render(
        &self,
        render: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::render_owner::post_render(
                self.access,
                render.cast(),
                frame.cast(),
            )
        }
    }

    unsafe fn viewport_unlock(
        &self,
        viewport: *mut std::ffi::c_void,
        blit: i32,
    ) -> RuntimeResult<()> {
        unsafe { unlock_viewport(self.access, viewport.cast(), blit) }
    }

    unsafe fn render_finish_master_frame(
        &self,
        render: *mut std::ffi::c_void,
        frame: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        unsafe { finish_master_frame(self.access, render.cast(), frame.cast()) }
    }
}

/// Runs the canonical `UGameEngine::Draw` entry point.
///
/// The drawing flag is published before frame work and cleared on every return path.
///
/// # Safety
/// `game` and `viewport` must be live canonical Core records for this call.
pub unsafe fn draw(
    access: CoreAccess<'_>,
    game: *mut UGameEngine,
    viewport: *mut UViewport,
    blit: bool,
    hit_data: *mut u8,
    hit_size: *mut i32,
) -> RuntimeResult<()> {
    if !access.is_running() || unsafe { (*game).g_is_drawing } {
        return Ok(());
    }
    unsafe { (*game).g_is_drawing = true };
    let _drawing = DrawingFlag(game);
    // FPushMemTag maps to Core FMalloc::SetTag, whose source body is empty.
    let ops = CoreDrawOps { access };
    unsafe {
        crate::game_tick::draw::draw_with(
            &ops,
            game.cast(),
            viewport.cast(),
            i32::from(blit),
            hit_data,
            hit_size,
            c"UnGame.cpp".as_ptr(),
        )
    }
}

struct DrawingFlag(*mut UGameEngine);

impl Drop for DrawingFlag {
    fn drop(&mut self) {
        // SAFETY: the draw caller guarantees `game` remains live until this synchronous call returns.
        unsafe { (*self.0).g_is_drawing = false };
    }
}
