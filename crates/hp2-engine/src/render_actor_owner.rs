//! Canonical actor-side renderer operations.
use hp2_abi::engine::generated::{FSceneNode, UCanvas, UTexture};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::ptr;
#[path = "render_actor_sprite_owner.rs"]
mod sprite;
#[path = "render_actor_dynamic_owner.rs"]
mod dynamic;
#[path = "render_actor_mesh_owner.rs"]
pub(super) mod mesh;
pub(crate) use mesh::set_high_detail_models;

/// Implements `URender::DrawIcon`'s Canvas submission for a scene-pipeline Icon packet.
///
/// # Safety
/// The frame and texture must be live canonical objects in the current draw graph.
pub(super) unsafe fn draw_icon(
    access: CoreAccess<'_>,
    _render: *mut super::URender,
    frame: *mut FSceneNode,
    packet: &mut hp2_render::scene_pipeline::draw::Hp2ScenePipelineDrawPacket,
) -> RuntimeResult<()> {
    if frame.is_null() || packet.p[0].is_null() {
        return Err(RuntimeError::new("Invalid canonical renderer Icon arguments"));
    }
    let (canvas, texture) = unsafe {
        let viewport = ptr::addr_of!((*frame).viewport).read_unaligned();
        if viewport.is_null() {
            return Err(RuntimeError::new("Canonical renderer Icon frame has no viewport"));
        }
        (
            ptr::addr_of!((*viewport.cast::<hp2_abi::engine::generated::UViewport>()).canvas)
                .read_unaligned()
                .cast::<UCanvas>(),
            packet.p[0].cast::<UTexture>(),
        )
    };
    let args = crate::canvas_text::Hp2CanvasPatternArgs {
        tex: texture.cast(),
        x: packet.f[0],
        y: packet.f[1],
        xl: packet.f[2],
        yl: packet.f[2],
        org_x: 0.0,
        org_y: 0.0,
        scale: 1.0,
        span: ptr::null_mut(),
        z: 1.0,
        color: crate::canvas_text::Hp2Plane {
            x: packet.f[3],
            y: packet.f[4],
            z: packet.f[5],
            w: 0.0,
        },
        fog: crate::canvas_text::Hp2Plane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 },
        flags: crate::canvas_text::PF_TWO_SIDED | crate::canvas_text::PF_TRANSLUCENT,
    };
    unsafe { crate::canvas_owner::draw_icon(access, canvas, &args) }
}

/// Draws one actor's dynamic sprite through the typed shared actor policy.
///
/// # Safety
/// `render`, `frame`, and `sprite` must be live records in the current Engine graph.
pub(super) unsafe fn draw_actor_sprite(
    access: CoreAccess<'_>,
    render: *mut super::URender,
    frame: *mut FSceneNode,
    sprite: *mut hp2_abi::render_scene::FDynamicSprite,
) -> RuntimeResult<()> {
    unsafe { sprite::draw_actor_sprite(access, render, frame, sprite) }
}

/// Draws a standalone actor using the source dynamic-item path.
///
/// # Safety
/// `render`, `frame`, and `actor` must be live records in the current Engine graph.
pub(super) unsafe fn draw_actor(
    access: CoreAccess<'_>,
    render: *mut super::URender,
    frame: *mut FSceneNode,
    actor: *mut hp2_abi::engine::generated::AActor,
) -> RuntimeResult<()> {
    unsafe { dynamic::draw_actor(access, render, frame, actor) }
}

/// Initializes dynamic actor lighting for the active scene frame and selected view actor.
///
/// # Safety
/// `render` and `frame` must be live; `actor` may be null as selected by scene policy.
pub(super) unsafe fn setup_dynamics(
    access: CoreAccess<'_>,
    render: *mut super::URender,
    frame: *mut FSceneNode,
    actor: *mut hp2_abi::engine::generated::AActor,
) -> RuntimeResult<()> {
    unsafe { dynamic::setup_dynamics(access, render, frame, actor) }
}

/// Filters one published dynamic item against a BSP node.
///
/// # Safety
/// `render`, `frame`, and `item` must be live in the current scene graph.
pub(super) unsafe fn filter(
    access: CoreAccess<'_>,
    render: *mut super::URender,
    frame: *mut FSceneNode,
    item: *mut hp2_abi::render_scene::FDynamicItem,
    node_index: i32,
    outside: i32,
) -> RuntimeResult<()> {
    unsafe { dynamic::filter(access, render, frame, item, node_index, outside) }
}

/// Clips and submits a dynamic item against its active span and volumetric-light lists.
///
/// # Safety
/// All pointers must belong to the active scene graph and remain live for the call.
pub(super) unsafe fn pre_render(
    access: CoreAccess<'_>,
    render: *mut super::URender,
    frame: *mut FSceneNode,
    item: *mut hp2_abi::render_scene::FDynamicItem,
    span: *mut hp2_abi::render_span::FSpanBuffer,
    first_volumetric: *mut *mut hp2_abi::render_scene::FVolActorLink,
    leaf_lights: *mut *mut hp2_abi::render_scene::FVolActorLink,
) -> RuntimeResult<()> {
    unsafe {
        dynamic::pre_render(
            access,
            render,
            frame,
            item,
            span,
            first_volumetric,
            leaf_lights,
        )
    }
}
