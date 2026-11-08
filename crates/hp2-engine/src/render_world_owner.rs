//! Canonical Core hosts for the complete renderer world and occlusion pipeline.

use crate::render_owner::{self, CoreSceneStackOps, RenderGraphState};
use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, FSceneNode, UCanvas, UClient, UEngine, UObject, UViewport, URender};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_render::scene_pipeline::helper_frames::{self, Hp2ScenePipelineHelperActor, WorldFrames, WorldOps};
use std::{cell::Cell, ffi::c_void, ptr};

const REN_DYN_LIGHT: i32 = hp2_abi::engine::generated::ENGINE_REN_DYN_LIGHT;
const SHOW_ACTORS: u32 = 0x8;

/// Runs the complete source-ordered `URender::DrawWorld` orchestration.
///
/// # Safety
/// The renderer and scene frame must be live canonical records for the duration of the call.
pub(crate) unsafe fn draw_world(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Null canonical DrawWorld input"));
    }
    let graph = render_owner::state(access)?;
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("DrawWorld renderer is not the published canonical renderer"));
    }
    let token = hp2_render::dynamic_items::begin();
    let result = unsafe { draw_world_in_scope(access, &graph, render, frame) };
    let ended = hp2_render::dynamic_items::end(token);
    match result {
        Err(error) => Err(error),
        Ok(()) if ended => Ok(()),
        Ok(()) => Err(RuntimeError::new("Canonical DrawWorld dynamic scope did not close")),
    }
}

unsafe fn draw_world_in_scope(
    access: CoreAccess<'_>,
    graph: &RenderGraphState,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    let stacks = CoreSceneStackOps::new(access)?;
    let render_state = graph.render_state_ptr();
    let frames = WorldFrames {
        stacks: stacks.stack_ptrs(),
        frame_stamp: unsafe { ptr::addr_of_mut!((*render_state).gframe_stamp) },
        mover_rejects: graph.mover_zone_rejects.as_ptr(),
        editor: access.process_mode().is_editor,
        show_actors: SHOW_ACTORS,
        dyn_light: REN_DYN_LIGHT,
    };
    let mut ops = CoreWorldOps {
        access,
        graph,
        render,
        overlay_flag_previous: None,
    };
    let result = unsafe { helper_frames::world_with(&frames, &stacks, &mut ops, frame.cast()) };
    if let Some(error) = stacks.take_error() {
        if result.is_ok() {
            return Err(error);
        }
    }
    result
}

/// Native occlusion is a renderer-hosted operation, not part of DrawFrame.
///
/// # Safety
/// The renderer and scene frame must be live canonical records for the duration of the call.
pub(super) unsafe fn occlude_frame(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() {
        return Err(RuntimeError::new("Null canonical OccludeFrame input"));
    }
    let graph = render_owner::state(access)?;
    if graph.render()?.as_ptr() != render {
        return Err(RuntimeError::new("OccludeFrame renderer is not the published canonical renderer"));
    }
    let stack_ops = CoreSceneStackOps::new(access)?;
    occlude_frame_core(access, &graph, &stack_ops, render, frame)
}

unsafe fn occlude_frame_core(
    access: CoreAccess<'_>,
    graph: &RenderGraphState,
    stacks: &CoreSceneStackOps<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
) -> RuntimeResult<()> {
    let _ = (graph, stacks);
    unsafe { render_owner::occlusion::occlude_frame(access, render, frame) }
}

struct CoreWorldOps<'a> {
    access: CoreAccess<'a>,
    graph: &'a RenderGraphState,
    render: *mut URender,
    overlay_flag_previous: Option<bool>,
}

impl WorldOps for CoreWorldOps<'_> {
    type Error = RuntimeError;

    unsafe fn actor(
        &mut self,
        viewport: *mut c_void,
    ) -> RuntimeResult<Hp2ScenePipelineHelperActor> {
        if viewport.is_null() {
            return Err(RuntimeError::new("DrawWorld viewport is null"));
        }
        let viewport = viewport.cast::<UViewport>();
        let actor = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned().cast::<APlayerPawn>() };
        if actor.is_null() {
            return Err(RuntimeError::new("DrawWorld viewport has no actor"));
        }
        let canvas = unsafe { ptr::addr_of!((*viewport).canvas).read_unaligned().cast::<UCanvas>() };
        let target = unsafe { ptr::addr_of!((*actor).view_target).read_unaligned().cast::<c_void>() };
        let show_flags = unsafe { ptr::addr_of!((*actor).show_flags).read_unaligned() };
        let actor_base = unsafe { ptr::addr_of!((*actor).base.base).cast::<AActor>().cast_mut() };
        let level = unsafe { ptr::addr_of!((*actor_base).level).read_unaligned().cast::<ALevelInfo>() };
        if level.is_null() {
            return Err(RuntimeError::new("DrawWorld actor has no level info"));
        }
        Ok(Hp2ScenePipelineHelperActor {
            actor: actor_base.cast(),
            target,
            canvas: canvas.cast(),
            admin: i32::from(unsafe { (*actor).b_admin() }),
            no_cheating: i32::from(unsafe { (*level).b_no_cheating() }),
            behind: i32::from(unsafe { (*actor).base.b_behind_view() }),
            show_flags: show_flags as u32,
        })
    }

    unsafe fn render_mode(
        &mut self,
        viewport: *mut c_void,
        mode: i32,
        lighting: i32,
    ) -> RuntimeResult<()> {
        if viewport.is_null() {
            return Err(RuntimeError::new("DrawWorld render-mode viewport is null"));
        }
        let viewport = viewport.cast::<UViewport>();
        let actor = unsafe { ptr::addr_of_mut!((*viewport).actor).read_unaligned().cast::<APlayerPawn>() };
        if actor.is_null() {
            return Err(RuntimeError::new("DrawWorld viewport has no actor for render mode"));
        }
        unsafe { ptr::addr_of_mut!((*actor).rend_map).write_unaligned(mode) };
        let client = unsafe { ptr::addr_of!((*viewport).outer).read_unaligned().cast::<UClient>() };
        if client.is_null() {
            return Err(RuntimeError::new("DrawWorld viewport has no client for render mode"));
        }
        unsafe { (*client).set_no_lighting(lighting != 0) };
        Ok(())
    }

    unsafe fn audio(&mut self) -> RuntimeResult<*mut c_void> {
        let engine = unsafe { ptr::addr_of!((*self.render).base.engine).read_unaligned().cast::<UEngine>() };
        if engine.is_null() {
            return Ok(ptr::null_mut());
        }
        Ok(unsafe { ptr::addr_of!((*engine).audio).read_unaligned().cast() })
    }

    unsafe fn render_audio(
        &mut self,
        audio: *mut c_void,
        _frame: *mut hp2_abi::render_scene::FSceneNode,
    ) -> RuntimeResult<()> {
        // UALAudioSubsystem::RenderAudioGeometry is empty in the source implementation.
        let _ = audio;
        Ok(())
    }

    unsafe fn occlude_frame(&mut self, frame: *mut hp2_abi::render_scene::FSceneNode) -> RuntimeResult<()> {
        unsafe { occlude_frame(self.access, self.render, frame.cast()) }
    }

    unsafe fn draw_frame(&mut self, frame: *mut hp2_abi::render_scene::FSceneNode) -> RuntimeResult<()> {
        unsafe { super::scene_draw::draw_frame(self.access, self.render, frame.cast()) }
    }

    unsafe fn mover_debug_enabled(&mut self) -> RuntimeResult<bool> {
        Ok(self.graph.mover_debug_enabled())
    }

    unsafe fn mover_report(&mut self, stamp: i32, rejects: i32) -> RuntimeResult<()> {
        let message = format!("Mover zone rejects: {rejects} (frame {stamp})");
        let wide: Vec<u32> = message.chars().map(u32::from).collect();
        render_owner::emit_renderer_log(self.access, "Log", &wide)
    }

    unsafe fn overlays(&mut self, actor: *mut c_void, canvas: *mut c_void) -> RuntimeResult<()> {
        if actor.is_null() {
            return Err(RuntimeError::new("DrawWorld overlay actor is null"));
        }
        let event_name = self.access.intern_str("RenderOverlays", FindName::Find)?;
        let object = actor.cast::<UObject>();
        let function = unsafe { self.access.find_object_function_by_name(object, event_name, false)? };
        if function.is_null() {
            return Err(RuntimeError::new("Canonical RenderOverlays event is missing"));
        }
        #[repr(C)]
        struct Params { canvas: *mut UCanvas }
        let mut params = Params { canvas: canvas.cast() };
        unsafe {
            crate::actor_owner::process_event(
                self.access,
                actor.cast::<AActor>(),
                function,
                (&mut params as *mut Params).cast(),
                ptr::null_mut(),
            )
    }
    }

    unsafe fn overlays_begin(&mut self) -> RuntimeResult<()> {
        let flags = unsafe { hp2_core::runtime_globals::ugly_hack_flags() };
        self.overlay_flag_previous = Some(flags & 1 != 0);
        unsafe { hp2_core::runtime_globals::set_ugly_hack_flags(flags | 1) };
        Ok(())
    }

    unsafe fn overlays_end(&mut self) -> RuntimeResult<()> {
        if let Some(previous) = self.overlay_flag_previous.take() {
            let flags = unsafe { hp2_core::runtime_globals::ugly_hack_flags() };
            let flags = if previous { flags | 1 } else { flags & !1 };
            unsafe { hp2_core::runtime_globals::set_ugly_hack_flags(flags) };
        }
        Ok(())
    }
}
