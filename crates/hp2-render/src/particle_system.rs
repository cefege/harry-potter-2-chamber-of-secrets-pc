//! Complete active UnParticleRn policy. Native leaves retain engine object
//! layouts, virtual calls and precisely scoped FTextureInfo/FMemMark storage.
use crate::particle::{self, FTransTexture, FTransform, FVF_OUT_ALL, ParticleFrame, madd};
use crate::particle_shard::{self, ParticleState};
use crate::particle_tube::{self, TubeParticle};
use crate::services::Hp2RenderServices;
use hp2_abi::engine::generated::UParticle as Particle;
use hp2_abi::render_scene::{FDynamicSprite, FSceneNode};
use hp2_abi::{FCoords, FPlane, FVector};
use std::ffi::c_void;
use std::mem::MaybeUninit;

/// Particle primitive selected by `AParticleFX::RenderPrimitive`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticlePrimitive {
    /// Standard particle system.
    Standard,
    /// Spark particle system.
    Spark,
    /// Liquid particle system.
    Liquid,
    /// Shard particle system.
    Shard,
    /// Tube particle system.
    Tube,
    /// An unrecognized native value; the policy reports the assertion as usual.
    Other,
}

impl From<usize> for ParticlePrimitive {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Standard,
            1 => Self::Spark,
            2 => Self::Liquid,
            3 => Self::Shard,
            4 => Self::Tube,
            _ => Self::Other,
        }
    }
}

/// Native draw path selected for one particle primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticleDrawKind {
    /// Default particle quad (native particle kind 1).
    Standard,
    /// Polygon-strip particle geometry (native particle kind 0).
    PolyStrip,
    /// Shell-only particle quad.
    Shell,
    /// Liquid particle geometry.
    Liquid,
    /// Shard particle geometry.
    Shard,
    /// Tube particle geometry.
    Tube,
}

impl ParticleDrawKind {
    fn native_kind(self) -> u32 {
        match self {
            Self::Standard => 1,
            Self::PolyStrip => 0,
            Self::Shell => 5,
            Self::Liquid => 2,
            Self::Shard => 3,
            Self::Tube => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleRenderView {
    pub frame: ParticleFrame,
    pub coords: FCoords,
    pub texture_u: i32,
    pub texture_v: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleRenderData {
    pub position: FVector,
    pub velocity: FVector,
    pub width: f32,
    pub length: f32,
    pub drip_timer: f32,
    pub spin: f32,
    pub spin_rate: f32,
    pub alpha: f32,
    pub priority_tag: i32,
    pub color: FPlane,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleStatistic {
    Updated,
    Seen,
    Rendered,
    Area,
    AreaRendered,
}

pub trait ParticleRenderOps: particle_shard::RenderMath {
    type Error;

    fn particle_primitive(
        &mut self,
        sprite: *mut FDynamicSprite,
    ) -> Result<ParticlePrimitive, Self::Error>;
    fn shell_only(&mut self, sprite: *mut FDynamicSprite) -> Result<bool, Self::Error>;
    fn invalid_particle_primitive(&mut self) -> Result<(), Self::Error>;
    fn render_dimensions(
        &mut self,
        frame: *mut FSceneNode,
        sprite: *mut FDynamicSprite,
        kind: ParticleDrawKind,
    ) -> Result<[f32; 3], Self::Error>;
    fn texture_available(&mut self, sprite: *mut FDynamicSprite) -> Result<bool, Self::Error>;
    fn particle_list_nonempty(&mut self, sprite: *mut FDynamicSprite)
    -> Result<bool, Self::Error>;
    fn resolve_texture(
        &mut self,
        frame: *mut FSceneNode,
        sprite: *mut FDynamicSprite,
    ) -> Result<(), Self::Error>;
    fn lock_texture(&mut self, frame: *mut FSceneNode) -> Result<(), Self::Error>;
    fn texture_flags(&mut self) -> Result<[u32; 2], Self::Error>;
    fn max_vertices(&mut self, frame: *mut FSceneNode) -> Result<i32, Self::Error>;
    fn mark_memory(&mut self) -> Result<(), Self::Error>;
    fn allocate_vertices(
        &mut self,
        count: usize,
    ) -> Result<std::ptr::NonNull<FTransTexture>, Self::Error>;
    fn allocate_vertex_pointers(
        &mut self,
        count: usize,
    ) -> Result<std::ptr::NonNull<*mut FTransTexture>, Self::Error>;
    fn allocate_indices(&mut self, count: usize) -> Result<std::ptr::NonNull<u16>, Self::Error>;
    fn is_unlit(&mut self) -> Result<bool, Self::Error>;
    fn set_white_light(&mut self) -> Result<(), Self::Error>;
    fn setup_lighting(&mut self, frame: *mut FSceneNode, sprite: *mut FDynamicSprite)
    -> Result<(), Self::Error>;
    fn light_particles(&mut self, frame: *mut FSceneNode, sprite: *mut FDynamicSprite)
    -> Result<(), Self::Error>;
    fn finish_lighting(&mut self) -> Result<(), Self::Error>;
    fn first_particle(&mut self, sprite: *mut FDynamicSprite) -> Result<*mut Particle, Self::Error>;
    fn next_particle(
        &mut self,
        sprite: *mut FDynamicSprite,
        particle: *mut Particle,
    ) -> Result<*mut Particle, Self::Error>;
    fn render_view(
        &mut self,
        frame: *mut FSceneNode,
        sprite: *mut FDynamicSprite,
    ) -> Result<ParticleRenderView, Self::Error>;
    fn particle_data(&mut self, particle: *mut Particle) -> Result<ParticleRenderData, Self::Error>;
    fn set_particle_alpha(
        &mut self,
        particle: *mut Particle,
        alpha: f32,
    ) -> Result<(), Self::Error>;
    fn color_state(&mut self) -> Result<ColorState, Self::Error>;
    fn draw_triangles(&mut self, draw: &Draw, final_draw: bool) -> Result<(), Self::Error>;
    fn allocate_clip_vertex(&mut self) -> Result<std::ptr::NonNull<FTransTexture>, Self::Error>;
    fn particle_stat(&mut self, statistic: ParticleStatistic, value: f32)
    -> Result<(), Self::Error>;
    fn pop_memory_mark(&mut self) -> Result<(), Self::Error>;
    fn unlock_texture(&mut self) -> Result<(), Self::Error>;
}

struct ParticleRenderState {
    kind: ParticleDrawKind,
    max_size: f32,
    area: f32,
    rendered: f32,
    center: FTransform,
    previous: Option<TubeParticle>,
    strip: FVector,
    edge: FVector,
    max_vertices: i32,
    max_indices: i32,
    flags: u32,
}

fn tube_particle(data: ParticleRenderData) -> TubeParticle {
    TubeParticle {
        x: data.position.x,
        y: data.position.y,
        z: data.position.z,
        width: data.width,
        length: data.length,
    }
}

fn particle_color(data: ParticleRenderData, state: &ColorState) -> FPlane {
    let mut color = FPlane {
        x: state.light.x * data.color.x,
        y: state.light.y * data.color.y,
        z: state.light.z * data.color.z,
        w: 1.0,
    };
    let alpha = data.alpha * state.opacity;
    if alpha < 1.0 {
        color.x *= alpha;
        color.y *= alpha;
        color.z *= alpha;
        color.w *= alpha;
        if state.style == 4 {
            let grey = (1.0 - alpha) * 0.5;
            color.x += grey;
            color.y += grey;
            color.z += grey;
            color.w += grey;
        }
    }
    color
}

fn setup_vertices_with_ops<O: ParticleRenderOps>(
    ops: &mut O,
    state: &mut ParticleRenderState,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
    vertices: *mut FTransTexture,
    particle: *mut Particle,
    next: *mut Particle,
) -> Result<u8, O::Error> {
    let view = ops.render_view(frame, sprite)?;
    let current = ops.particle_data(particle)?;
    match state.kind {
        ParticleDrawKind::Shard => Ok(particle_shard::shard_setup_verts(
            ops,
            &view.frame,
            &view.coords,
            view.texture_u,
            view.texture_v,
            current.position,
            current.spin,
            current.length,
            current.width,
            unsafe { &mut *vertices.cast::<[FTransTexture; 3]>() },
        )),
        ParticleDrawKind::Liquid => Ok(particle_shard::liquid_setup_verts(
            ops,
            &view.frame,
            &view.coords,
            view.texture_u,
            view.texture_v,
            &mut state.center,
            &ParticleState {
                position: current.position,
                velocity: current.velocity,
                length: current.length,
                width: current.width,
                drip_timer: current.drip_timer,
            },
            unsafe { &mut *vertices.cast::<[FTransTexture; 4]>() },
        )),
        ParticleDrawKind::PolyStrip | ParticleDrawKind::Tube => {
            let next = if next.is_null() {
                None
            } else {
                Some(ops.particle_data(next)?)
            };
            let current_tube = tube_particle(current);
            let next_tube = next.map(tube_particle);
            let clip = if state.kind == ParticleDrawKind::Tube {
                particle_tube::tritube_setup_verts(
                    ops,
                    &view.frame,
                    &view.coords,
                    view.texture_u,
                    view.texture_v,
                    state.previous.as_ref(),
                    &current_tube,
                    next_tube.as_ref(),
                    &mut state.strip,
                    unsafe { &mut *vertices.cast::<[FTransTexture; 12]>() },
                )
            } else {
                particle_tube::poly_strip_setup_verts(
                    ops,
                    &view.frame,
                    &view.coords,
                    view.texture_u,
                    view.texture_v,
                    state.previous.as_ref(),
                    &current_tube,
                    next_tube.as_ref(),
                    &mut state.strip,
                    &mut state.edge,
                    unsafe { &mut *vertices.cast::<[FTransTexture; 4]>() },
                )
            };
            state.previous = Some(current_tube);
            Ok(clip)
        }
        ParticleDrawKind::Shell => {
            let position = current.position;
            let rz = view.frame.proj_z / position.z;
            let dx = (current.width * 0.5) as i32 as f32;
            let dy = (current.length * 0.5) as i32 as f32;
            for (index, x, y, u, v) in [
                (0, position.x - dx, position.y + dy, 0.0, 1.0),
                (2, position.x + dx, position.y - dy, 1.0, 0.0),
                (1, position.x - dx, position.y - dy, 0.0, 0.0),
                (3, position.x + dx, position.y + dy, 1.0, 1.0),
            ] {
                unsafe {
                    particle::set_trans(
                        &mut *vertices.add(index),
                        &view.frame,
                        view.texture_u,
                        view.texture_v,
                        x,
                        y,
                        rz,
                        u,
                        v,
                    );
                }
            }
            Ok(particle::combine_clip_codes(unsafe {
                std::slice::from_raw_parts(vertices, 4)
            }))
        }
        ParticleDrawKind::Standard => setup_quad_with_ops(ops, state, view, current, vertices, particle),
    }
}

fn setup_quad_with_ops<O: ParticleRenderOps>(
    ops: &mut O,
    state: &mut ParticleRenderState,
    view: ParticleRenderView,
    current: ParticleRenderData,
    vertices: *mut FTransTexture,
    particle: *mut Particle,
) -> Result<u8, O::Error> {
    let t = FVector {
        x: current.position.x - view.coords.origin.x,
        y: current.position.y - view.coords.origin.y,
        z: current.position.z - view.coords.origin.z,
    };
    let dot = |axis: FVector| madd(t.z, axis.z, madd(t.x, axis.x, t.y * axis.y));
    state.center.point = FVector {
        x: dot(view.coords.x_axis),
        y: dot(view.coords.y_axis),
        z: dot(view.coords.z_axis),
    };
    if state.center.point.z < 1.0 {
        return Ok(FVF_OUT_ALL);
    }
    particle_shard::project(&mut state.center, &view.frame);
    let center = state.center;
    let dx = current.width * 0.5;
    let dy = current.length * 0.5;
    let tx = view.texture_u.wrapping_sub(1) as f32;
    let ty = view.texture_v.wrapping_sub(1) as f32;
    let clip = if current.spin_rate != 0.0 {
        let trig_table = hp2_core::math::trig_table();
        let (cos, sin) = (
            hp2_core::math::cos_float_inline(current.spin, trig_table),
            hp2_core::math::sin_float_inline(current.spin, trig_table),
        );
        let offsets = [
            (madd(-dx, cos, dy * sin), madd(-dx, sin, -(dy * cos)), 0.0, ty),
            (madd(dx, cos, dy * sin), madd(dx, sin, -(dy * cos)), tx, ty),
            (madd(dx, cos, -(dy * sin)), madd(dx, sin, dy * cos), tx, 0.0),
            (madd(-dx, cos, -(dy * sin)), madd(-dx, sin, dy * cos), 0.0, 0.0),
        ];
        for (index, (x, y, u, v)) in offsets.into_iter().enumerate() {
            unsafe { particle::set_trans_2d(&mut *vertices.add(index), &center, x, y, u, v) };
        }
        particle::compute_clip_codes_2d(
            &view.frame,
            &center.point,
            madd(cos.abs(), dx, sin.abs() * dy),
            madd(cos.abs(), dy, sin.abs() * dx),
        )
    } else {
        for (index, (x, y, u, v)) in [
            (-dx, -dy, 0.0, ty),
            (dx, -dy, tx, ty),
            (dx, dy, tx, 0.0),
            (-dx, dy, 0.0, 0.0),
        ]
        .into_iter()
        .enumerate()
        {
            unsafe { particle::set_trans_2d(&mut *vertices.add(index), &center, x, y, u, v) };
        }
        particle::compute_clip_codes_2d(&view.frame, &center.point, dx, dy)
    };
    if clip != FVF_OUT_ALL {
        let size = current.width * current.length * (center.rz * center.rz);
        state.area += size;
        let test =
            ((size * current.priority_tag.wrapping_add(1) as f32) as f64 * 0.25) as f32;
        if test > state.max_size {
            let new = (2.0 * state.max_size - test) / state.max_size;
            let alpha = if current.alpha < new { current.alpha } else { new };
            ops.set_particle_alpha(particle, alpha)?;
            if alpha <= 0.0 {
                ops.particle_stat(ParticleStatistic::Seen, 1.0)?;
                return Ok(FVF_OUT_ALL);
            }
        }
        state.rendered += size;
    }
    Ok(clip)
}

fn color_with_ops<O: ParticleRenderOps>(
    ops: &mut O,
    particle: *mut Particle,
) -> Result<FPlane, O::Error> {
    let state = ops.color_state()?;
    Ok(particle_color(ops.particle_data(particle)?, &state))
}

unsafe fn colors_with_ops<O: ParticleRenderOps>(
    ops: &mut O,
    kind: ParticleDrawKind,
    vertices: *mut FTransTexture,
    particle: *mut Particle,
    next: *mut Particle,
) -> Result<(), O::Error> {
    if kind == ParticleDrawKind::PolyStrip && !next.is_null() {
        let next_color = color_with_ops(ops, next)?;
        unsafe {
            (*vertices.add(1)).light = next_color;
            (*vertices.add(2)).light = next_color;
        }
        let current_color = color_with_ops(ops, particle)?;
        unsafe {
            (*vertices).light = current_color;
            (*vertices.add(3)).light = current_color;
        }
    } else if kind == ParticleDrawKind::Tube {
        let current_color = color_with_ops(ops, particle)?;
        if !next.is_null() {
            for index in [0, 3, 4, 7, 8, 11] {
                unsafe { (*vertices.add(index)).light = current_color };
            }
            let next_color = color_with_ops(ops, next)?;
            for index in [1, 2, 5, 6, 9, 10] {
                unsafe { (*vertices.add(index)).light = next_color };
            }
        } else {
            for index in 0..12 {
                unsafe { (*vertices.add(index)).light = current_color };
            }
        }
    } else {
        let current_color = color_with_ops(ops, particle)?;
        for index in 0..4 {
            unsafe { (*vertices.add(index)).light = current_color };
        }
    }
    Ok(())
}

fn draw_batch_with_ops<O: ParticleRenderOps>(
    ops: &mut O,
    state: &ParticleRenderState,
    vertices: *mut *mut FTransTexture,
    indices: *mut u16,
    vertex_count: i32,
    index_count: i32,
    final_draw: bool,
) -> Result<(), O::Error> {
    let draw = Draw {
        vertices,
        indices,
        vertex_count,
        index_count,
        flags: state.flags,
        valid: (vertex_count <= state.max_vertices && index_count <= state.max_indices) as u32,
    };
    ops.draw_triangles(&draw, final_draw)
}

fn render_marked_with_ops<O: ParticleRenderOps>(
    ops: &mut O,
    state: &mut ParticleRenderState,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
) -> Result<(), O::Error> {
    let num_polys = if state.kind == ParticleDrawKind::Tube { 3 } else { 1 };
    let num_verts = if state.kind == ParticleDrawKind::Shard { 3 } else { 4 };
    let all = ops.allocate_vertices(state.max_vertices as usize)?.as_ptr();
    let pointers = ops
        .allocate_vertex_pointers(state.max_vertices as usize)?
        .as_ptr();
    let indices = ops.allocate_indices(state.max_indices as usize)?.as_ptr();
    if ops.is_unlit()? {
        ops.set_white_light()?;
    } else {
        ops.setup_lighting(frame, sprite)?;
        ops.light_particles(frame, sprite)?;
        ops.finish_lighting()?;
    }
    let mut particle = ops.first_particle(sprite)?;
    let mut vertex_count = 0;
    let mut index_count = 0;
    while !particle.is_null() {
        ops.particle_stat(ParticleStatistic::Updated, 0.0)?;
        let next = ops.next_particle(sprite, particle)?;
        if vertex_count >= state.max_vertices - num_polys * num_verts * 3 {
            draw_batch_with_ops(
                ops,
                state,
                pointers,
                indices,
                vertex_count,
                index_count,
                false,
            )?;
            vertex_count = 0;
            index_count = 0;
        }
        let vertices = unsafe { all.offset(vertex_count as isize) };
        let clip = setup_vertices_with_ops(ops, state, frame, sprite, vertices, particle, next)?;
        if clip != FVF_OUT_ALL {
            unsafe { colors_with_ops(ops, state.kind, vertices, particle, next)? };
            for index in 0..num_verts * num_polys {
                unsafe { *pointers.offset((vertex_count + index) as isize) = vertices.add(index as usize) };
            }
            for _ in 0..num_polys {
                let count = if clip != 0 {
                    let view = ops.render_view(frame, sprite)?;
                    let mut allocate = || {
                        ops.allocate_clip_vertex().map(std::ptr::NonNull::as_ptr)
                    };
                    // SAFETY: pointers refers to initialized vertex pointers, while the
                    // allocator returns writable storage with the required lifetime.
                    unsafe {
                        crate::particle_clip::clip_polygon_checked(
                            &view.frame,
                            pointers.offset(vertex_count as isize),
                            num_verts,
                            clip,
                            &mut allocate,
                        )?
                    }
                } else {
                    num_verts
                };
                for triangle in 0..count - 2 {
                    unsafe {
                        *indices.offset(index_count as isize) = vertex_count as u16;
                        index_count += 1;
                        *indices.offset(index_count as isize) =
                            (vertex_count + triangle + 1) as u16;
                        index_count += 1;
                        *indices.offset(index_count as isize) =
                            (vertex_count + triangle + 2) as u16;
                        index_count += 1;
                    }
                }
                vertex_count += count.max(num_verts);
            }
            ops.particle_stat(ParticleStatistic::Seen, 0.0)?;
            ops.particle_stat(ParticleStatistic::Rendered, 0.0)?;
        }
        particle = next;
    }
    if index_count > 0 {
        draw_batch_with_ops(
            ops,
            state,
            pointers,
            indices,
            vertex_count,
            index_count,
            true,
        )?;
    }
    ops.particle_stat(ParticleStatistic::Area, state.area)?;
    ops.particle_stat(ParticleStatistic::AreaRendered, state.rendered)?;
    Ok(())
}

pub fn render_particle_pass_with_ops<O: ParticleRenderOps>(
    ops: &mut O,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
    kind: ParticleDrawKind,
) -> Result<(), O::Error> {
    let dimensions = ops.render_dimensions(frame, sprite, kind)?;
    let max_size = dimensions[0] * dimensions[1] * dimensions[2] * 0.5;
    if !ops.texture_available(sprite)? || !ops.particle_list_nonempty(sprite)? {
        return Ok(());
    }
    ops.resolve_texture(frame, sprite)?;
    ops.lock_texture(frame)?;
    let result = (|| {
        let flags = ops.texture_flags()?;
        let max_vertices = ops.max_vertices(frame)?;
        let mut state = ParticleRenderState {
            kind,
            max_size,
            area: 0.0,
            rendered: 0.0,
            center: FTransform {
                point: ZERO,
                flags: 0,
                screen_x: 0.0,
                screen_y: 0.0,
                int_y: 0,
                rz: 0.0,
            },
            previous: None,
            strip: ZERO,
            edge: ZERO,
            max_vertices,
            max_indices: max_vertices.wrapping_mul(3),
            flags: poly_flags(flags[0]) | flags[1],
        };
        ops.mark_memory()?;
        let marked = render_marked_with_ops(ops, &mut state, frame, sprite);
        let popped = ops.pop_memory_mark();
        marked?;
        popped
    })();
    let unlocked = ops.unlock_texture();
    result?;
    unlocked
}

/// Native operations used by the particle-system policy entrypoint.
///
/// Implementations perform engine-facing operations. Primitive selection,
/// shell routing, timing boundaries, and draw ordering remain in Rust.
pub trait ParticleSystemOps {
    /// Error returned by a native operation.
    type Error;

    /// Returns the configured particle density for this sprite's level.
    fn particle_density(&mut self, sprite: *mut FDynamicSprite) -> Result<bool, Self::Error>;

    /// Updates the sprite's particle actor; `false` skips rendering.
    fn update_particles(&mut self, sprite: *mut FDynamicSprite) -> Result<bool, Self::Error>;

    /// Starts particle-system elapsed-time accounting.
    fn begin_particle_timing(&mut self) -> Result<(), Self::Error>;

    /// Starts render-time accounting after a successful particle update.
    fn begin_particle_render_timing(&mut self) -> Result<(), Self::Error>;

    /// Finishes render-time accounting after all selected passes.
    fn finish_particle_render_timing(&mut self) -> Result<(), Self::Error>;

}
trait LegacyParticlePassOps {
    type Error;
    fn primitive(&mut self) -> Result<ParticlePrimitive, Self::Error>;
    fn shell_only(&mut self) -> Result<bool, Self::Error>;
    fn draw(&mut self, kind: ParticleDrawKind) -> Result<(), Self::Error>;
    fn invalid_primitive(&mut self) -> Result<(), Self::Error>;
}

trait ParticlePassExecutor {
    type Error;
    fn primitive(&mut self) -> Result<ParticlePrimitive, Self::Error>;
    fn shell_only(&mut self) -> Result<bool, Self::Error>;
    fn draw(&mut self, kind: ParticleDrawKind) -> Result<(), Self::Error>;
    fn invalid_primitive(&mut self) -> Result<(), Self::Error>;
    fn finish(&mut self) -> Result<(), Self::Error>;
}

struct LegacyPassExecutor<'a, O> {
    ops: &'a mut O,
}

impl<O> ParticlePassExecutor for LegacyPassExecutor<'_, O>
where
    O: LegacyParticlePassOps + ParticleSystemOps<Error = <O as LegacyParticlePassOps>::Error>,
{
    type Error = <O as LegacyParticlePassOps>::Error;
    fn primitive(&mut self) -> Result<ParticlePrimitive, Self::Error> {
        LegacyParticlePassOps::primitive(self.ops)
    }
    fn shell_only(&mut self) -> Result<bool, Self::Error> {
        LegacyParticlePassOps::shell_only(self.ops)
    }
    fn draw(&mut self, kind: ParticleDrawKind) -> Result<(), Self::Error> {
        LegacyParticlePassOps::draw(self.ops, kind)
    }
    fn invalid_primitive(&mut self) -> Result<(), Self::Error> {
        LegacyParticlePassOps::invalid_primitive(self.ops)
    }
    fn finish(&mut self) -> Result<(), Self::Error> {
        ParticleSystemOps::finish_particle_render_timing(self.ops)
    }
}

fn run_particle_passes_with_executor<E: ParticlePassExecutor>(
    ops: &mut E,
) -> Result<(), E::Error> {
    let primitive = ops.primitive()?;
    match primitive {
        ParticlePrimitive::Standard => {
            ops.draw(ParticleDrawKind::PolyStrip)?;
            ops.draw(ParticleDrawKind::Shard)?;
            ops.draw(ParticleDrawKind::Liquid)?;
            let kind = if ops.shell_only()? {
                ParticleDrawKind::Shell
            } else {
                ParticleDrawKind::Standard
            };
            ops.draw(kind)?;
        }
        ParticlePrimitive::Shard => {
            ops.draw(ParticleDrawKind::Shard)?;
            ops.draw(ParticleDrawKind::Liquid)?;
            let kind = if ops.shell_only()? {
                ParticleDrawKind::Shell
            } else {
                ParticleDrawKind::Standard
            };
            ops.draw(kind)?;
        }
        ParticlePrimitive::Liquid => {
            ops.draw(ParticleDrawKind::Liquid)?;
            let kind = if ops.shell_only()? {
                ParticleDrawKind::Shell
            } else {
                ParticleDrawKind::Standard
            };
            ops.draw(kind)?;
        }
        ParticlePrimitive::Spark => {
            let kind = if ops.shell_only()? {
                ParticleDrawKind::Shell
            } else {
                ParticleDrawKind::Standard
            };
            ops.draw(kind)?;
        }
        ParticlePrimitive::Tube => {
            ops.draw(ParticleDrawKind::Tube)?;
            ops.invalid_primitive()?;
        }
        ParticlePrimitive::Other => ops.invalid_primitive()?,
    }
    ops.finish()
}

fn run_particle_passes<O>(
    ops: &mut O,
    _frame: *mut FSceneNode,
    _particle: *mut Particle,
    _sprite: *mut FDynamicSprite,
) -> Result<(), <O as LegacyParticlePassOps>::Error>
where
    O: LegacyParticlePassOps
        + ParticleSystemOps<Error = <O as LegacyParticlePassOps>::Error>,
{
    let mut executor = LegacyPassExecutor { ops };
    run_particle_passes_with_executor(&mut executor)
}

struct CorePassExecutor<'a, O>
where
    O: ParticleSystemOps + ParticleRenderOps<Error = <O as ParticleSystemOps>::Error>,
{
    ops: &'a mut O,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
}

impl<O> ParticlePassExecutor for CorePassExecutor<'_, O>
where
    O: ParticleSystemOps + ParticleRenderOps<Error = <O as ParticleSystemOps>::Error>,
{
    type Error = <O as ParticleSystemOps>::Error;
    fn primitive(&mut self) -> Result<ParticlePrimitive, Self::Error> {
        ParticleRenderOps::particle_primitive(self.ops, self.sprite)
    }
    fn shell_only(&mut self) -> Result<bool, Self::Error> {
        ParticleRenderOps::shell_only(self.ops, self.sprite)
    }
    fn draw(&mut self, kind: ParticleDrawKind) -> Result<(), Self::Error> {
        render_particle_pass_with_ops(self.ops, self.frame, self.sprite, kind)
    }
    fn invalid_primitive(&mut self) -> Result<(), Self::Error> {
        ParticleRenderOps::invalid_particle_primitive(self.ops)
    }
    fn finish(&mut self) -> Result<(), Self::Error> {
        ParticleSystemOps::finish_particle_render_timing(self.ops)
    }
}

pub fn run_with_ops<O>(
    ops: &mut O,
    frame: *mut FSceneNode,
    _particle: *mut Particle,
    sprite: *mut FDynamicSprite,
) -> Result<(), <O as ParticleSystemOps>::Error>
where
    O: ParticleSystemOps + ParticleRenderOps<Error = <O as ParticleSystemOps>::Error>,
{
    if !ParticleSystemOps::particle_density(ops, sprite)? {
        return Ok(());
    }
    ParticleSystemOps::begin_particle_timing(ops)?;
    if !ParticleSystemOps::update_particles(ops, sprite)? {
        return Ok(());
    }
    ParticleSystemOps::begin_particle_render_timing(ops)?;
    let mut executor = CorePassExecutor { ops, frame, sprite };
    run_particle_passes_with_executor(&mut executor)
}


#[repr(C)]
pub struct Hp2ParticleSystemServices {
    pub leaf: unsafe extern "C" fn(*mut c_void, u32, usize, usize, *mut usize) -> i32,
    pub math: *const Hp2RenderServices,
    /// `Frame`. Always valid for the lifetime of a `run()` call, same as
    /// every other unchecked `Frame->` dereference in this file;
    /// `ParticleFrame`/`Coords` are `FSceneNode` mirror reads, never a
    /// per-particle leaf.
    pub frame: *const c_void,
}
/// `Hp2ParticleFrame(Frame)`: the same eight fields the native leaf used to
/// copy, read directly from the mirror instead.
///
/// # Safety
/// `frame` must point to a live `FSceneNode`.
unsafe fn particle_frame(frame: *const FSceneNode) -> ParticleFrame {
    let f = unsafe { &*frame };
    ParticleFrame {
        proj_z: f.proj.z,
        rproj_z: f.r_proj.z,
        fx15: f.fx15,
        fy15: f.fy15,
        prjxm: f.prj_xm,
        prjxp: f.prj_xp,
        prjym: f.prj_ym,
        prjyp: f.prj_yp,
    }
}
#[derive(Clone, Copy)]
struct View {
    frame: ParticleFrame,
    coords: FCoords,
    u: i32,
    v: i32,
}
/// Native leaf payload for `VIEW`: only the texture dimensions remain a
/// leaf; `Frame`/`Coords` are read from the mirror by the caller.
#[repr(C)]
struct TexSize {
    u: i32,
    v: i32,
}
#[repr(C)]
pub struct ColorState {
    pub light: FPlane,
    pub opacity: f32,
    pub style: u32,
}
#[repr(C)]
pub struct Draw {
    pub vertices: *mut *mut FTransTexture,
    pub indices: *mut u16,
    pub vertex_count: i32,
    pub index_count: i32,
    pub flags: u32,
    pub valid: u32,
}
// Leaf IDs are shared verbatim with native_adapter.inc. No leaf contains
// renderer decisions: stack scopes call back into run instead of implementing
// any part of the render loop.
const DENSITY: u32 = 0;
const UPDATE: u32 = 1;
const PRIMITIVE: u32 = 2;
const SHELL: u32 = 3;
const BEGIN: u32 = 4;
const TEXTURE: u32 = 5;
const SIZE: u32 = 6;
const GET_TEXTURE: u32 = 7;
const TEXTURE_SCOPE: u32 = 8;
const LOCK: u32 = 9;
const FLAGS: u32 = 10;
const MAX_VERTICES: u32 = 11;
const MARK_SCOPE: u32 = 12;
const ALLOC_VERTICES: u32 = 13;
const ALLOC_POINTERS: u32 = 14;
const ALLOC_INDICES: u32 = 15;
const UNLIT: u32 = 16;
const WHITE: u32 = 17;
const LIGHT_SETUP: u32 = 18;
const LIGHT: u32 = 19;
const LIGHT_FINISH: u32 = 20;
const START: u32 = 21;
const NEXT: u32 = 22;
const VIEW: u32 = 24;
const COLOR: u32 = 25;
const DRAW: u32 = 28;
const ALLOC_CLIP: u32 = 29;
const POP: u32 = 30;
const UNLOCK: u32 = 31;
const STAT: u32 = 32;
const CLOCK: u32 = 33;
const ASSERT: u32 = 34;
const FINISH_SCOPE: u32 = 35;

#[derive(Clone, Copy)]
struct Call {
    services: *const Hp2ParticleSystemServices,
    context: *mut c_void,
}
impl Call {
    /// `Frame`: an `FSceneNode` mirror, not a leaf.
    unsafe fn scene(self) -> *const FSceneNode {
        unsafe { (*self.services).frame.cast() }
    }
    unsafe fn leaf(self, op: u32, a: usize, b: usize) -> Result<usize, i32> {
        let mut out = 0;
        let status = unsafe { ((*self.services).leaf)(self.context, op, a, b, &mut out) };
        if status == 0 { Ok(out) } else { Err(status) }
    }
    unsafe fn read<T>(self, op: u32, arg: usize) -> Result<T, i32> {
        let mut out = MaybeUninit::<T>::uninit();
        unsafe {
            self.leaf(op, arg, out.as_mut_ptr() as usize)?;
            Ok(out.assume_init())
        }
    }
    unsafe fn allocate<T>(self, op: u32, count: usize) -> Result<*mut T, i32> {
        let pointer = unsafe { self.leaf(op, count, 0)? } as *mut T;
        // A misbehaving native error sink must not turn a failed allocation
        // into a null dereference or an assertion panic in the clip kernel.
        if pointer.is_null() {
            Err(1)
        } else {
            Ok(pointer)
        }
    }
}

struct AbiParticleSystemOps {
    call: Call,
}

impl ParticleSystemOps for AbiParticleSystemOps {
    type Error = i32;

    fn particle_density(&mut self, _sprite: *mut FDynamicSprite) -> Result<bool, Self::Error> {
        unsafe { self.call.leaf(DENSITY, 0, 0).map(|value| value != 0) }
    }

    fn update_particles(&mut self, _sprite: *mut FDynamicSprite) -> Result<bool, Self::Error> {
        unsafe { self.call.leaf(UPDATE, 0, 0).map(|value| value != 0) }
    }

    fn begin_particle_timing(&mut self) -> Result<(), Self::Error> {
        unsafe { self.call.leaf(CLOCK, 0, 0).map(|_| ()) }
    }

    fn begin_particle_render_timing(&mut self) -> Result<(), Self::Error> {
        unsafe { self.call.leaf(CLOCK, 2, 0).map(|_| ()) }
    }

    fn finish_particle_render_timing(&mut self) -> Result<(), Self::Error> {
        unsafe { self.call.leaf(CLOCK, 3, 0).map(|_| ()) }
    }

}

impl LegacyParticlePassOps for AbiParticleSystemOps {
    type Error = i32;

    fn primitive(&mut self) -> Result<ParticlePrimitive, Self::Error> {
        unsafe { self.call.leaf(PRIMITIVE, 0, 0).map(Into::into) }
    }
    fn shell_only(&mut self) -> Result<bool, Self::Error> {
        unsafe { self.call.leaf(SHELL, 0, 0).map(|value| value != 0) }
    }
    fn draw(&mut self, kind: ParticleDrawKind) -> Result<(), Self::Error> {
        unsafe { render(self.call, kind.native_kind()) }
    }
    fn invalid_primitive(&mut self) -> Result<(), Self::Error> {
        unsafe { self.call.leaf(ASSERT, 0, 0).map(|_| ()) }
    }
}
struct System {
    call: Call,
    kind: u32,
    max_size: f32,
    area: f32,
    rendered: f32,
    center: FTransform,
    previous: usize,
    strip: FVector,
    edge: FVector,
    max_vertices: i32,
    max_indices: i32,
    flags: u32,
}
const ZERO: FVector = FVector {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};
pub fn poly_flags(style: u32) -> u32 {
    0x00400000
        | match style {
            2 => 2,
            3 => 4,
            4 => 0x40,
            _ => 0,
        }
}
// Native particles leave unused fields untouched; never borrow the whole object.
unsafe fn tube(p: *const Particle) -> TubeParticle {
    let position = unsafe { (*p).position };
    TubeParticle {
        x: position.x,
        y: position.y,
        z: position.z,
        width: unsafe { (*p).width },
        length: unsafe { (*p).length },
    }
}
unsafe fn color(call: Call, p: usize) -> Result<FPlane, i32> {
    let state: ColorState = unsafe { call.read(COLOR, 0)? };
    let p = p as *const Particle;
    let color = unsafe { (*p).color };
    let mut result = FPlane {
        x: state.light.x * color.x,
        y: state.light.y * color.y,
        z: state.light.z * color.z,
        w: 1.0,
    };
    let alpha = unsafe { (*p).alpha } * state.opacity;
    if alpha < 1.0 {
        result.x *= alpha;
        result.y *= alpha;
        result.z *= alpha;
        result.w *= alpha;
        if state.style == 4 {
            let grey = (1.0 - alpha) * 0.5;
            result.x += grey;
            result.y += grey;
            result.z += grey;
            result.w += grey;
        }
    }
    Ok(result)
}
unsafe fn colors(s: *mut System, v: *mut FTransTexture, p: usize, next: usize) -> Result<(), i32> {
    let call = unsafe { (*s).call };
    let kind = unsafe { (*s).kind };
    if kind == 0 && next != 0 {
        let n = unsafe { color(call, next)? };
        unsafe {
            (*v.add(1)).light = n;
            (*v.add(2)).light = n;
        }
        let c = unsafe { color(call, p)? };
        unsafe {
            (*v).light = c;
            (*v.add(3)).light = c;
        }
    } else if kind == 4 {
        let c = unsafe { color(call, p)? };
        if next != 0 {
            for i in [0, 3, 4, 7, 8, 11] {
                unsafe {
                    (*v.add(i)).light = c;
                }
            }
            let n = unsafe { color(call, next)? };
            for i in [1, 2, 5, 6, 9, 10] {
                unsafe {
                    (*v.add(i)).light = n;
                }
            }
        } else {
            for i in 0..12 {
                unsafe {
                    (*v.add(i)).light = c;
                }
            }
        }
    } else {
        let c = unsafe { color(call, p)? };
        // The original base SetupColors writes four entries even for Shard.
        for i in 0..4 {
            unsafe {
                (*v.add(i)).light = c;
            }
        }
    }
    Ok(())
}
unsafe fn setup(
    s: *mut System,
    vertices: *mut FTransTexture,
    p: usize,
    next: usize,
) -> Result<u8, i32> {
    let call = unsafe { (*s).call };
    let tex: TexSize = unsafe { call.read(VIEW, 0)? };
    let view = View {
        frame: unsafe { particle_frame(call.scene()) },
        coords: unsafe { (*call.scene()).coords },
        u: tex.u,
        v: tex.v,
    };
    let q = p as *const Particle;
    let math = unsafe { &*(*call.services).math };
    let kind = unsafe { (*s).kind };
    if kind == 3 {
        return Ok(unsafe {
            particle_shard::shard_setup_verts(
                math,
                &view.frame,
                &view.coords,
                view.u,
                view.v,
                (*q).position,
                (*q).spin,
                (*q).length,
                (*q).width,
                &mut *vertices.cast(),
            )
        });
    }
    if kind == 2 {
        return Ok(unsafe {
            particle_shard::liquid_setup_verts(
                math,
                &view.frame,
                &view.coords,
                view.u,
                view.v,
                &mut (*s).center,
                &ParticleState {
                    position: (*q).position,
                    velocity: (*q).velocity,
                    length: (*q).length,
                    width: (*q).width,
                    drip_timer: (*q).drip_timer,
                },
                &mut *vertices.cast(),
            )
        });
    }
    if kind == 0 || kind == 4 {
        let previous = unsafe { (*s).previous };
        let prev = if previous != 0 {
            Some(unsafe { tube(previous as *const Particle) })
        } else {
            None
        };
        let next = if next != 0 {
            Some(unsafe { tube(next as *const Particle) })
        } else {
            None
        };
        let curr = unsafe { tube(q) };
        let clip = unsafe {
            if kind == 4 {
                particle_tube::tritube_setup_verts(
                    math,
                    &view.frame,
                    &view.coords,
                    view.u,
                    view.v,
                    prev.as_ref(),
                    &curr,
                    next.as_ref(),
                    &mut (*s).strip,
                    &mut *vertices.cast(),
                )
            } else {
                particle_tube::poly_strip_setup_verts(
                    math,
                    &view.frame,
                    &view.coords,
                    view.u,
                    view.v,
                    prev.as_ref(),
                    &curr,
                    next.as_ref(),
                    &mut (*s).strip,
                    &mut (*s).edge,
                    &mut *vertices.cast(),
                )
            }
        };
        unsafe {
            (*s).previous = p;
        }
        return Ok(clip);
    }
    if kind == 5 {
        let position = unsafe { (*q).position };
        let rz = view.frame.proj_z / position.z;
        let dx = (unsafe { (*q).width } * 0.5) as i32 as f32;
        let dy = (unsafe { (*q).length } * 0.5) as i32 as f32;
        for (i, x, y, u, v) in [
            (0, position.x - dx, position.y + dy, 0.0, 1.0),
            (2, position.x + dx, position.y - dy, 1.0, 0.0),
            (1, position.x - dx, position.y - dy, 0.0, 0.0),
            (3, position.x + dx, position.y + dy, 1.0, 1.0),
        ] {
            unsafe {
                particle::set_trans(
                    &mut *vertices.add(i),
                    &view.frame,
                    view.u,
                    view.v,
                    x,
                    y,
                    rz,
                    u,
                    v,
                );
            }
        }
        return Ok(particle::combine_clip_codes(unsafe {
            std::slice::from_raw_parts(vertices, 4)
        }));
    }
    let position = unsafe { (*q).position };
    let t = FVector {
        x: position.x - view.coords.origin.x,
        y: position.y - view.coords.origin.y,
        z: position.z - view.coords.origin.z,
    };
    let dot = |axis: FVector| madd(t.z, axis.z, madd(t.x, axis.x, t.y * axis.y));
    unsafe {
        (*s).center.point = FVector {
            x: dot(view.coords.x_axis),
            y: dot(view.coords.y_axis),
            z: dot(view.coords.z_axis),
        };
    }
    if unsafe { (*s).center.point.z } < 1.0 {
        return Ok(FVF_OUT_ALL);
    }
    unsafe {
        particle_shard::project(&mut (*s).center, &view.frame);
    }
    let center = unsafe { (*s).center };
    let dx = unsafe { (*q).width } * 0.5;
    let dy = unsafe { (*q).length } * 0.5;
    let tx = view.u.wrapping_sub(1) as f32;
    let ty = view.v.wrapping_sub(1) as f32;
    let clip;
    if unsafe { (*q).spin_rate } != 0.0 {
        let trig_table = hp2_core::math::trig_table();
        let spin = unsafe { (*q).spin };
        let (cos, sin) = (
            hp2_core::math::cos_float_inline(spin, trig_table),
            hp2_core::math::sin_float_inline(spin, trig_table),
        );
        let offsets = [
            (
                madd(-dx, cos, dy * sin),
                madd(-dx, sin, -(dy * cos)),
                0.0,
                ty,
            ),
            (madd(dx, cos, dy * sin), madd(dx, sin, -(dy * cos)), tx, ty),
            (madd(dx, cos, -(dy * sin)), madd(dx, sin, dy * cos), tx, 0.0),
            (
                madd(-dx, cos, -(dy * sin)),
                madd(-dx, sin, dy * cos),
                0.0,
                0.0,
            ),
        ];
        for (i, (x, y, u, v)) in offsets.into_iter().enumerate() {
            unsafe {
                particle::set_trans_2d(&mut *vertices.add(i), &center, x, y, u, v);
            }
        }
        clip = particle::compute_clip_codes_2d(
            &view.frame,
            &center.point,
            madd(cos.abs(), dx, sin.abs() * dy),
            madd(cos.abs(), dy, sin.abs() * dx),
        );
    } else {
        for (i, (x, y, u, v)) in [
            (-dx, -dy, 0.0, ty),
            (dx, -dy, tx, ty),
            (dx, dy, tx, 0.0),
            (-dx, dy, 0.0, 0.0),
        ]
        .into_iter()
        .enumerate()
        {
            unsafe {
                particle::set_trans_2d(&mut *vertices.add(i), &center, x, y, u, v);
            }
        }
        clip = particle::compute_clip_codes_2d(&view.frame, &center.point, dx, dy);
    }
    if clip != FVF_OUT_ALL {
        let size = unsafe { (*q).width * (*q).length } * (center.rz * center.rz);
        unsafe {
            (*s).area += size;
        }
        // 0.25 is a DOUBLE literal in the original expression.
        let test =
            ((size * unsafe { (*q).priority_tag }.wrapping_add(1) as f32) as f64 * 0.25) as f32;
        let max = unsafe { (*s).max_size };
        if test > max {
            let new = (2.0 * max - test) / max;
            let current_alpha = unsafe { (*q).alpha };
            let alpha = if current_alpha < new {
                current_alpha
            } else {
                new
            };
            unsafe {
                (*(p as *mut Particle)).alpha = alpha;
            }
            if alpha <= 0.0 {
                unsafe {
                    call.leaf(STAT, 1, 0)?;
                }
                return Ok(FVF_OUT_ALL);
            }
        }
        unsafe {
            (*s).rendered += size;
        }
    }
    Ok(clip)
}
unsafe fn render(call: Call, kind: u32) -> Result<(), i32> {
    let dimensions: [f32; 3] = unsafe { call.read(BEGIN, 0)? };
    let max = dimensions[0] * dimensions[1] * dimensions[2] * 0.5;
    let mut s = System {
        call,
        kind,
        max_size: max,
        area: 0.0,
        rendered: 0.0,
        center: FTransform {
            point: ZERO,
            flags: 0,
            screen_x: 0.0,
            screen_y: 0.0,
            int_y: 0,
            rz: 0.0,
        },
        previous: 0,
        strip: ZERO,
        edge: ZERO,
        max_vertices: 0,
        max_indices: 0,
        flags: 0,
    };
    if unsafe { call.leaf(TEXTURE, 0, 0)? } == 0 {
        return Ok(());
    }
    if unsafe { call.leaf(SIZE, 0, 0)? } == 0 {
        return Ok(());
    }
    unsafe {
        call.leaf(GET_TEXTURE, 0, 0)?;
        call.leaf(TEXTURE_SCOPE, &mut s as *mut System as usize, 0)?;
    }
    Ok(())
}
unsafe fn texture_scope(s: *mut System) -> Result<(), i32> {
    let call = unsafe { (*s).call };
    unsafe {
        call.leaf(LOCK, 0, 0)?;
    }
    let flags: [u32; 2] = unsafe { call.read(FLAGS, 0)? };
    unsafe {
        (*s).flags = poly_flags(flags[0]) | flags[1];
    }
    let max = unsafe { call.leaf(MAX_VERTICES, 0, 0)? } as i32;
    unsafe {
        (*s).max_vertices = max;
        (*s).max_indices = max.wrapping_mul(3);
        call.leaf(MARK_SCOPE, s as usize, 0)?;
    }
    Ok(())
}
unsafe fn marked_scope(s: *mut System) -> Result<(), i32> {
    let call = unsafe { (*s).call };
    let kind = unsafe { (*s).kind };
    let max = unsafe { (*s).max_vertices };
    let max_indices = unsafe { (*s).max_indices };
    let num_polys = if kind == 4 { 3 } else { 1 };
    let num_verts = if kind == 3 { 3 } else { 4 };
    let all = unsafe { call.allocate::<FTransTexture>(ALLOC_VERTICES, max as usize)? };
    let pointers = unsafe { call.allocate::<*mut FTransTexture>(ALLOC_POINTERS, max as usize)? };
    let indices = unsafe { call.allocate::<u16>(ALLOC_INDICES, max_indices as usize)? };
    if unsafe { call.leaf(UNLIT, 0, 0)? } != 0 {
        unsafe {
            call.leaf(WHITE, 0, 0)?;
        }
    } else {
        unsafe {
            call.leaf(LIGHT_SETUP, 0, 0)?;
            call.leaf(LIGHT, 0, 0)?;
            call.leaf(LIGHT_FINISH, 0, 0)?;
        }
    }
    let mut p = unsafe { call.leaf(START, 0, 0)? };
    let mut vc = 0i32;
    let mut ic = 0i32;
    while p != 0 {
        unsafe {
            call.leaf(STAT, 0, 0)?;
        }
        let next = unsafe { call.leaf(NEXT, 0, 0)? };
        if vc >= max - num_polys * num_verts * 3 {
            unsafe {
                draw(s, pointers, indices, vc, ic, false)?;
            }
            vc = 0;
            ic = 0;
        }
        let vertices = unsafe { all.offset(vc as isize) };
        let clip = unsafe { setup(s, vertices, p, next)? };
        if clip != FVF_OUT_ALL {
            unsafe {
                colors(s, vertices, p, next)?;
            }
            for i in 0..num_verts * num_polys {
                unsafe {
                    *pointers.offset((vc + i) as isize) = vertices.offset(i as isize);
                }
            }
            for _ in 0..num_polys {
                let count = if clip != 0 {
                    let frame: ParticleFrame = unsafe { particle_frame(call.scene()) };
                    let mut alloc = || unsafe { call.allocate::<FTransTexture>(ALLOC_CLIP, 1) };
                    unsafe {
                        crate::particle_clip::clip_polygon_checked(
                            &frame,
                            pointers.offset(vc as isize),
                            num_verts,
                            clip,
                            &mut alloc,
                        )?
                    }
                } else {
                    num_verts
                };
                for t in 0..count - 2 {
                    unsafe {
                        *indices.offset(ic as isize) = vc as u16;
                        ic += 1;
                        *indices.offset(ic as isize) = (vc + t + 1) as u16;
                        ic += 1;
                        *indices.offset(ic as isize) = (vc + t + 2) as u16;
                        ic += 1;
                    }
                }
                vc += count.max(num_verts);
            }
            unsafe {
                call.leaf(STAT, 1, 0)?;
                call.leaf(STAT, 2, 0)?;
            }
        }
        p = next;
    }
    let mut finish = Finish {
        system: s,
        pointers,
        indices,
        vc,
        ic,
    };
    unsafe {
        call.leaf(FINISH_SCOPE, &mut finish as *mut Finish as usize, 0)?;
    }
    Ok(())
}
struct Finish {
    system: *mut System,
    pointers: *mut *mut FTransTexture,
    indices: *mut u16,
    vc: i32,
    ic: i32,
}
unsafe fn finish_scope(f: *mut Finish) -> Result<(), i32> {
    let s = unsafe { (*f).system };
    let call = unsafe { (*s).call };
    if unsafe { (*f).ic } > 0 {
        unsafe {
            draw(s, (*f).pointers, (*f).indices, (*f).vc, (*f).ic, true)?;
        }
    }
    unsafe {
        call.leaf(STAT, 3, (*s).area.to_bits() as usize)?;
        call.leaf(STAT, 4, (*s).rendered.to_bits() as usize)?;
        call.leaf(POP, 0, 0)?;
        call.leaf(UNLOCK, 0, 0)?;
    }
    Ok(())
}
unsafe fn draw(
    s: *mut System,
    v: *mut *mut FTransTexture,
    i: *mut u16,
    vc: i32,
    ic: i32,
    final_draw: bool,
) -> Result<(), i32> {
    let call = unsafe { (*s).call };
    let d = Draw {
        vertices: v,
        indices: i,
        vertex_count: vc,
        index_count: ic,
        flags: unsafe { (*s).flags },
        valid: unsafe { (vc <= (*s).max_vertices && ic <= (*s).max_indices) as u32 },
    };
    unsafe {
        call.leaf(DRAW, &d as *const Draw as usize, final_draw as usize)?;
    }
    Ok(())
}
/// # Safety
/// Native context and services remain live throughout nested continuations.
/// `entry` 1/2 must receive the opaque System pointer supplied by this module.
/// Native callbacks catch all C++ exceptions and report nonzero immediately.
pub unsafe fn run(
    services: *const Hp2ParticleSystemServices,
    context: *mut c_void,
    entry: u32,
    arg: usize,
    out: *mut usize,
) -> i32 {
    let call = Call { services, context };
    let result = (|| -> Result<(), i32> {
        unsafe {
            match entry {
                0 => {
                    if call.leaf(DENSITY, 0, 0)? == 0 {
                        return Ok(());
                    }
                    call.leaf(CLOCK, 0, 0)?;
                }
                3 => {
                    if call.leaf(UPDATE, 0, 0)? == 0 {
                        return Ok(());
                    }
                    call.leaf(CLOCK, 2, 0)?;
                    let mut ops = AbiParticleSystemOps { call };
                    run_particle_passes(
                        &mut ops,
                        unsafe { call.scene() }.cast_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )?;
                }
                1 => texture_scope(arg as *mut System)?,
                2 => marked_scope(arg as *mut System)?,
                4 => finish_scope(arg as *mut Finish)?,
                _ => return Err(1),
            }
            if !out.is_null() {
                *out = 0;
            }
            Ok(())
        }
    })();
    match result {
        Ok(()) => 0,
        Err(status) => status,
    }
}
