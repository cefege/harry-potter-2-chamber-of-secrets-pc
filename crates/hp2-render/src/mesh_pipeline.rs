//! Ordinary UMesh draw policy. Native leaves are virtual calls and SDK field views.
use crate::mesh_lod_pipeline::{LodAllocation, LodArrayView, LodLightVertex, LodTexture};
use crate::{mesh, mesh_shared as shared};
use core::{
    ffi::c_void,
    ptr::{addr_of_mut, null_mut},
};
use hp2_abi::engine::generated::{AActor, AZoneInfo, FMemStack, UTexture};
use hp2_abi::render_particle::FTransTexture;
use hp2_abi::render_span::FMemMark;
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::{math, mem};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MeshTriangle {
    pub vertices: [u16; 3],
    pub uv: [[u8; 2]; 3],
    pub flags: u32,
    pub texture: i32,
}
#[repr(C)]
pub struct MeshConnect {
    pub count: i32,
    pub offset: i32,
}
#[repr(C)]
pub struct MeshWireDraw {
    pub color: FPlane,
    pub flags: u32,
    pub a: FVector,
    pub b: FVector,
}
#[repr(C)]
pub struct MeshDrawLive {
    pub selected: i32,
    pub wire: i32,
    pub span_based: i32,
    pub texture_count: i32,
    pub world_lod: i32,
    pub recommended_lod: i32,
    pub rust_light_manager: i32,
}
#[repr(C)]
pub struct MeshDrawInput {
    pub shared: shared::MeshSharedInput,
    pub context: *mut c_void,
    pub service: unsafe extern "C" fn(*mut c_void, u32, i32, *mut c_void) -> i32,
    pub live: *const MeshDrawLive,
    pub error_stage: *mut i32,
    pub span: *const *mut c_void,
    pub frame_verts: *const i32,
    pub triangles: LodArrayView,
    pub connects: LodArrayView,
    pub links: LodArrayView,
    pub not_weapon: i32,
    pub editor: *const i32,
    pub scale_glow: *const f32,
    pub ambient_glow: *const u8,
    pub fatness: *const u8,
    pub mirror: *const f32,
    pub flags: u32,
    pub render_services: *const crate::services::Hp2RenderServices,
    pub light_count_stat: *mut i32,
    /// `Owner`. Read-only mirror access (`AActor.texture`/`.region.zone`/
    /// `.level`) for environment-map lookup; never a live-owner virtual call.
    pub owner: *const c_void,
}

/// Callback-free inputs for ordinary mesh drawing.
pub struct MeshDrawState {
    pub shared: shared::MeshSharedGeometry,
    pub memory: *mut c_void,
    pub memory_error: *mut i32,
    pub alignment: i32,
    pub slow_checks: bool,
    pub live: *const MeshDrawLive,
    pub error_stage: *mut i32,
    pub frame_verts: *const i32,
    pub triangles: LodArrayView,
    pub connects: LodArrayView,
    pub links: LodArrayView,
    pub not_weapon: i32,
    pub editor: *const i32,
    pub scale_glow: *const f32,
    pub ambient_glow: *const u8,
    pub fatness: *const u8,
    pub mirror: *const f32,
    pub flags: u32,
    pub light_count_stat: *mut i32,
    pub owner: *const AActor,
}

/// Environment-map values read from the actor's current owner chain.
#[derive(Clone, Copy)]
pub struct MeshEnvironmentMaps {
    pub texture: *mut UTexture,
    pub zone: *mut UTexture,
    pub level: *mut UTexture,
}

/// Typed Native/Core operations used by ordinary mesh draw policy.
pub trait MeshDrawOps: shared::MeshSharedOps {
    unsafe fn memory_mark(&mut self, memory: *mut c_void) -> Result<FMemMark, Self::Error>;
    unsafe fn memory_pop(&mut self, mark: &FMemMark);
    fn begin_mesh(&mut self) -> Result<(), Self::Error>;
    fn transform_samples(
        &mut self,
        wire: bool,
        samples: *mut FTransTexture,
        count: i32,
    ) -> Result<(), Self::Error>;
    fn samples_ready(&mut self, count: i32) -> Result<(), Self::Error>;
    fn draw_wire(&mut self, line: &mut MeshWireDraw) -> Result<(), Self::Error>;
    fn finish_wire(&mut self) -> Result<(), Self::Error>;
    fn initialize_lighting(&mut self) -> Result<(), Self::Error>;
    fn visibility_ready(&mut self) -> Result<(), Self::Error>;
    fn begin_textures(&mut self) -> Result<(), Self::Error>;
    fn texture_info(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), Self::Error>;
    fn lock_texture(&mut self, index: i32) -> Result<(), Self::Error>;
    fn texture_lod(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), Self::Error>;
    fn resolve_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), Self::Error>;
    fn bind_environment(&mut self, environment: &mut *mut UTexture) -> Result<(), Self::Error>;
    fn environment_flags(&mut self, flags: &mut u32) -> Result<(), Self::Error>;
    fn environment_maps(&mut self, owner: *const AActor) -> Result<MeshEnvironmentMaps, Self::Error>;
    fn light_vertex(&mut self, vertex: &mut LodLightVertex) -> Result<(), Self::Error>;
    fn light_and_fog(
        &mut self,
        vertex: &mut crate::light::FogVertex,
        flags: u32,
    ) -> bool;
    fn begin_polygons(&mut self, visible: i32) -> Result<(), Self::Error>;
    fn finish_polygons(&mut self) -> Result<(), Self::Error>;
    fn unlock_texture(&mut self, index: i32) -> Result<(), Self::Error>;
    fn end_mesh(&mut self) -> Result<(), Self::Error>;
    fn texture_transition_complete(&mut self) -> Result<(), Self::Error>;
    fn unbind_environment(&mut self, environment: &mut *mut UTexture) -> Result<(), Self::Error>;
}
unsafe fn call<T>(i: &MeshDrawInput, op: u32, n: i32, p: *mut T) -> Result<(), ()> {
    if unsafe { (i.service)(i.context, op, n, p.cast()) } != 0 {
        Ok(())
    } else {
        Err(())
    }
}
unsafe fn event(i: &MeshDrawInput, op: u32, n: i32) -> Result<(), ()> {
    unsafe { call(i, op, n, null_mut::<u8>()) }
}
fn v(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}
fn add(a: FVector, b: FVector) -> FVector {
    v(a.x + b.x, a.y + b.y, a.z + b.z)
}
fn sub(a: FVector, b: FVector) -> FVector {
    v(a.x - b.x, a.y - b.y, a.z - b.z)
}
fn scale(a: FVector, b: f32) -> FVector {
    v(a.x * b, a.y * b, a.z * b)
}
fn xyz(a: FPlane) -> FVector {
    v(a.x, a.y, a.z)
}
fn plane(a: FVector, w: f32) -> FPlane {
    FPlane {
        x: a.x,
        y: a.y,
        z: a.z,
        w,
    }
}
unsafe fn allocate<O: MeshDrawOps, T>(
    state: &MeshDrawState,
    ops: &mut O,
    count: usize,
) -> Result<*mut T, O::Error> {
    let shared_state = shared::MeshSharedState {
        geometry: state.shared,
        memory: state.memory,
        memory_error: state.memory_error,
        alignment: state.alignment,
        slow_checks: state.slow_checks,
    };
    unsafe { shared::allocate_with_ops(&shared_state, ops, count) }
}

pub unsafe fn draw_with_ops<O: MeshDrawOps>(
    i: &MeshDrawState,
    ops: &mut O,
) -> Result<(), O::Error> {
    unsafe {
        let shared_state = shared::MeshSharedState {
            geometry: i.shared,
            memory: i.memory,
            memory_error: i.memory_error,
            alignment: i.alignment,
            slow_checks: i.slow_checks,
        };
        let s = shared::storage();
        let mark = ops.memory_mark(i.memory)?;
        let mut flags = i.flags | 0x4000;
        *i.error_stage = 1;
        ops.begin_mesh()?;
        let samples = allocate::<O, FTransTexture>(i, ops, *i.frame_verts as usize)?;
        let wire = (*i.live).wire != 0;
        ops.transform_samples(
            wire,
            samples,
            *i.frame_verts,
        )?;
        *i.error_stage = 2;
        let mut out = 0x3cu8;
        let mut n = 0;
        while n < *i.frame_verts {
            let p = samples.add(n as usize);
            (*p).light.x = -1.;
            (*p).transform.flags = shared::outcode_geometry((*p).transform.point, &i.shared);
            out &= (*p).transform.flags;
            n += 1;
        }
        ops.samples_ready(*i.frame_verts)?;
        if wire {
            *i.error_stage = 3;
            let color = if (*i.live).selected != 0 {
                FPlane {
                    x: 0.2,
                    y: 0.8,
                    z: 0.1,
                    w: 0.,
                }
            } else {
                FPlane {
                    x: 0.6,
                    y: 0.4,
                    z: 0.1,
                    w: 0.,
                }
            };
            let mut n = 0;
            while n < i.triangles.len() {
                let tri = i.triangles.at::<MeshTriangle>(n);
                let mut p = samples.add((*tri).vertices[2] as usize);
                for j in 0..3 {
                    let q = samples.add((*tri).vertices[j] as usize);
                    if (*tri).flags & 0x100 != 0 || (*p).transform.point.x >= (*q).transform.point.x
                    {
                        let mut line = MeshWireDraw {
                            color,
                            flags: 2,
                            a: (*p).transform.point,
                            b: (*q).transform.point,
                        };
                        ops.draw_wire(&mut line)?;
                    }
                    p = q;
                }
                n += 1;
            }
            ops.finish_wire()?;
            ops.memory_pop(&mark);
            return Ok(());
        }
        let unlit = mesh::unlit(*i.scale_glow, *i.ambient_glow);
        (*s).unlit_color = v(unlit, unlit, unlit);
        if *i.editor != 0 && flags & 0x2000000 != 0 {
            (*s).unlit_color = add(scale((*s).unlit_color, 0.4), v(0., 0.6, 0.));
        }
        shared::HasSpecialCoords = 0;
        let mut visible = 0;
        let mut tris = null_mut::<(*const MeshTriangle, i32)>();
        let mut normals = null_mut::<FVector>();
        if out == 0 {
            *i.error_stage = 4;
            tris = allocate::<O, (*const MeshTriangle, i32)>(i, ops, i.triangles.len())?;
            normals = allocate::<O, FVector>(i, ops, i.triangles.len())?;
            ops.initialize_lighting()?;
            let mut n = 0;
            while n < i.triangles.len() {
                let tri = i.triangles.at::<MeshTriangle>(n);
                let a = samples.add((*tri).vertices[0] as usize);
                let b = samples.add((*tri).vertices[1] as usize);
                let c = samples.add((*tri).vertices[2] as usize);
                let poly = flags | (*tri).flags;
                let normal = mesh::tri_normal(
                    (*a).transform.point,
                    (*b).transform.point,
                    (*c).transform.point,
                );
                *normals.add(n) = normal;
                if (*a).transform.flags & (*b).transform.flags & (*c).transform.flags == 0
                    && (poly & (0x100 | 0x4000 | 1) != 0x4000
                        || *i.mirror * math::vector_dot((*a).transform.point, normal) < 0.)
                {
                    let key = if i.not_weapon != 0 {
                        mesh::app_round(
                            ((*a).transform.point.z + (*b).transform.point.z)
                                + (*c).transform.point.z,
                        )
                    } else {
                        mesh::depth_key_distance(
                            (*a).transform.point,
                            (*b).transform.point,
                            (*c).transform.point,
                            v(0., -8., 0.),
                        )
                    };
                    *tris.add(visible) = (tri, key);
                    visible += 1;
                }
                n += 1;
            }
            ops.visibility_ready()?;
        }
        if visible != 0 {
            *i.error_stage = 5;
            let fatten = *i.fatness != 128;
            let fat = mesh::fatness_offset(*i.fatness);
            if (*i.live).span_based != 0 {
                crate::mesh_lod_pipeline::sort_faces(core::slice::from_raw_parts_mut(
                    tris, visible,
                ));
            }
            *i.error_stage = 6;
            ops.begin_textures()?;
            let lod = ((*i.live).world_lod - 1).max((*i.live).recommended_lod);
            let mut environment = null_mut::<UTexture>();
            let mut n = 0;
            while n < (*i.live).texture_count {
                let mut t = LodTexture {
                    object: null_mut(),
                    info: null_mut(),
                    u_scale: 0.,
                    v_scale: 0.,
                    lod: 0,
                    u_size: 0,
                    v_size: 0,
                };
                ops.texture_info(n, &mut t)?;
                if !(*s).textures[n as usize].is_null() {
                    ops.lock_texture(0)?;
                    ops.texture_lod(n, &mut t)?;
                    t.lod = lod.min(t.lod);
                    ops.resolve_texture(n, &mut t)?;
                    environment = (*s).textures[n as usize].cast();
                    ops.texture_transition_complete()?;
                }
                n += 1;
            }
            let maps = ops.environment_maps(i.owner)?;
            for e in [maps.texture, maps.zone, maps.level] {
                if !e.is_null() {
                    environment = e;
                    break;
                }
            }
            // This legacy return intentionally neither pops memory nor unlocks textures.
            if environment.is_null() {
                return Ok(());
            }
            ops.lock_texture(0)?;
            ops.bind_environment(&mut environment)?;
            ops.texture_transition_complete()?;
            *i.error_stage = 5;
            ops.environment_flags(&mut flags)?;
            *i.error_stage = 7;
            for n in 0..visible {
                let tri = (*tris.add(n)).0;
                for j in 0..3 {
                    let id = (*tri).vertices[j] as usize;
                    let p = samples.add(id);
                    if (*p).light.x == -1. {
                        let mut normal = v(0., 0., 0.);
                        let connect = i.connects.at::<MeshConnect>(id);
                        let mut k = 0;
                        while k < (*connect).count {
                            normal = add(
                                normal,
                                *normals
                                    .add(*i.links.at::<i32>(((*connect).offset + k) as usize)
                                        as usize),
                            );
                            k += 1;
                        }
                        normal = scale(normal, mesh::div_sqrt_approx(mesh::size_squared(normal)));
                        (*p).normal = plane(normal, math::vector_dot((*p).transform.point, normal));
                        if fatten {
                            (*p).transform.point = add((*p).transform.point, scale(normal, fat));
                            (*p).transform.flags =
                                shared::outcode_geometry((*p).transform.point, &i.shared);
                        }
                        // Identity is live: a foreign callback can replace GLightManager.
                        if (*i.live).rust_light_manager != 0 {
                            let mut vertex = crate::light::FogVertex {
                                point: (*p).transform.point,
                                normal: xyz((*p).normal),
                                light: [0.; 4],
                                fog: [0.; 4],
                            };
                            if ops.light_and_fog(&mut vertex, flags)
                                && !i.light_count_stat.is_null()
                            {
                                *i.light_count_stat = (*i.light_count_stat)
                                    .wrapping_add(crate::light_manager::light_count() as i32);
                            }
                            (*p).light = FPlane {
                                x: vertex.light[0],
                                y: vertex.light[1],
                                z: vertex.light[2],
                                w: vertex.light[3],
                            };
                            (*p).fog = FPlane {
                                x: vertex.fog[0],
                                y: vertex.fog[1],
                                z: vertex.fog[2],
                                w: vertex.fog[3],
                            };
                        } else {
                            let mut vertex = LodLightVertex { sample: p, flags };
                            ops.light_vertex(&mut vertex)?;
                        }
                        if (*p).transform.flags == 0 {
                            shared::project_geometry(p, &i.shared);
                        }
                    }
                }
            }
            *i.error_stage = 8;
            ops.begin_polygons(visible as i32)?;
            shared::timed_init_with_ops(ops, samples, *i.frame_verts)?;
            for n in 0..visible {
                let tri = (*tris.add(n)).0;
                if (*tri).flags & 1 == 0 {
                    let poly = (*tri).flags | flags;
                    let index = (*tri).texture as usize;
                    let info = if !(*s).textures[index].is_null() && poly & 0x10 == 0 {
                        addr_of_mut!((*s).texture_info)
                            .cast::<shared::MeshTextureInfo>()
                            .add(index)
                    } else {
                        addr_of_mut!((*s).environment_info)
                    };
                    (*s).u_scale = (*info).u_scale * (*info).u_size as f32 / 256.;
                    (*s).v_scale = (*info).v_scale * (*info).v_size as f32 / 256.;
                    let mut pts = [null_mut(); 6];
                    for j in 0..3 {
                        let p = samples.add((*tri).vertices[j] as usize);
                        pts[j] = p;
                        (*p).u = (*tri).uv[j][0] as f32 * (*s).u_scale;
                        (*p).v = (*tri).uv[j][1] as f32 * (*s).v_scale;
                    }
                    if *i.mirror == -1. {
                        pts.swap(0, 2);
                    }
                    *i.error_stage = 9;
                    shared::subsurface_with_ops(&shared_state, ops, info.cast(), pts.as_mut_ptr(), poly)?;
                    *i.error_stage = 8;
                } else {
                    let a = (*samples.add((*tri).vertices[0] as usize)).transform.point;
                    let b = (*samples.add((*tri).vertices[1] as usize)).transform.point;
                    let c = (*samples.add((*tri).vertices[2] as usize)).transform.point;
                    let x = math::vector_safe_normal(sub(b, a));
                    let y = math::vector_safe_normal(mesh::cross(x, sub(a, c)));
                    let z = mesh::cross(y, x);
                    let coords = FCoords {
                        origin: v(0., 0., 0.),
                        x_axis: x,
                        y_axis: y,
                        z_axis: z,
                    };
                    shared::SpecialCoords = math::coords_mul(
                        math::coords_mul_point(math::unit_coords(), scale(add(a, c), 0.5)),
                        coords,
                    );
                    shared::HasSpecialCoords = 1;
                }
            }
            shared::timed_flush_with_ops(ops)?;
            ops.finish_polygons()?;
            *i.error_stage = 5;
            ops.lock_texture(0)?;
            let mut n = 0;
            while n < (*i.live).texture_count {
                if !(*s).textures[n as usize].is_null() {
                    ops.unlock_texture(n)?;
                }
                n += 1;
            }
            ops.unbind_environment(&mut environment)?;
            ops.texture_transition_complete()?;
        }
        *i.error_stage = 0;
        ops.end_mesh()?;
        ops.memory_pop(&mark);
        Ok(())
    }
}

struct LegacyMeshDrawOps<'a>(&'a MeshDrawInput);

impl shared::MeshSharedOps for LegacyMeshDrawOps<'_> {
    type Error = ();

    unsafe fn allocate_bytes(
        &mut self,
        memory: *mut c_void,
        bytes: usize,
        alignment: i32,
        slow_checks: bool,
        memory_error: *mut i32,
    ) -> Result<*mut c_void, ()> {
        unsafe {
            match mem::push_bytes(
                &mut *memory.cast::<FMemStack>(),
                bytes as i32,
                alignment,
                slow_checks,
                c"UnMem.h",
                &*self.0.shared.memory_services,
            ) {
                Ok(ptr) => Ok(ptr.cast()),
                Err(frame) => {
                    *memory_error = frame;
                    Err(())
                }
            }
        }
    }

    fn batch_capacity(&mut self) -> Result<usize, ()> {
        let mut capacity = 0i32;
        unsafe { call(self.0, 0, 0, &mut capacity)? };
        Ok(capacity as usize)
    }

    fn draw_triangles(&mut self, batch: &mut shared::MeshBatchDraw) -> Result<(), ()> {
        unsafe { call(self.0, 1, 0, batch) }
    }

    fn subsurface_begin(&mut self) -> Result<(), ()> { unsafe { event(self.0, 2, 0) } }
    fn surface_ready(&mut self) -> Result<(), ()> { unsafe { event(self.0, 4, 0) } }
    fn surface_timer_begin(&mut self) -> Result<(), ()> { unsafe { event(self.0, 3, 0) } }
    fn surface_timer_end(&mut self) -> Result<(), ()> { unsafe { event(self.0, 5, 0) } }
    fn check_allocation_size(&mut self, bytes: &mut usize) -> Result<(), ()> {
        unsafe { call(self.0, 6, 0, bytes) }
    }
}

impl MeshDrawOps for LegacyMeshDrawOps<'_> {

    unsafe fn memory_mark(&mut self, memory: *mut c_void) -> Result<FMemMark, ()> {
        let mut mark = core::mem::MaybeUninit::<FMemMark>::uninit();
        unsafe {
            mem::mark_init(mark.as_mut_ptr(), memory.cast::<FMemStack>());
            Ok(mark.assume_init())
        }
    }

    unsafe fn memory_pop(&mut self, mark: &FMemMark) {
        unsafe { mem::mark_pop(mark) }
    }

    fn begin_mesh(&mut self) -> Result<(), ()> { unsafe { event(self.0, 1, 0) } }
    fn transform_samples(&mut self, wire: bool, samples: *mut FTransTexture, count: i32) -> Result<(), ()> {
        let mut allocation = LodAllocation { count, data: samples };
        unsafe { call(self.0, 2, i32::from(wire), &mut allocation) }
    }
    fn samples_ready(&mut self, count: i32) -> Result<(), ()> { unsafe { event(self.0, 3, count) } }
    fn draw_wire(&mut self, line: &mut MeshWireDraw) -> Result<(), ()> { unsafe { call(self.0, 4, 0, line) } }
    fn finish_wire(&mut self) -> Result<(), ()> { unsafe { event(self.0, 5, 0) } }
    fn initialize_lighting(&mut self) -> Result<(), ()> { unsafe { event(self.0, 6, 0) } }
    fn visibility_ready(&mut self) -> Result<(), ()> { unsafe { event(self.0, 7, 0) } }
    fn begin_textures(&mut self) -> Result<(), ()> { unsafe { event(self.0, 8, 0) } }
    fn texture_info(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        unsafe { call(self.0, 9, index, texture) }
    }
    fn lock_texture(&mut self, index: i32) -> Result<(), ()> { unsafe { event(self.0, 10, index) } }
    fn texture_lod(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        unsafe { call(self.0, 11, index, texture) }
    }
    fn resolve_texture(&mut self, index: i32, texture: &mut LodTexture) -> Result<(), ()> {
        unsafe { call(self.0, 12, index, texture) }
    }
    fn bind_environment(&mut self, environment: &mut *mut UTexture) -> Result<(), ()> {
        unsafe { call(self.0, 14, 0, environment) }
    }
    fn environment_flags(&mut self, flags: &mut u32) -> Result<(), ()> { unsafe { call(self.0, 15, 0, flags) } }
    fn environment_maps(&mut self, owner: *const AActor) -> Result<MeshEnvironmentMaps, ()> {
        unsafe {
            let zone = (*owner).region.zone.cast::<AZoneInfo>();
            let level = (*owner).level.cast::<AZoneInfo>();
            Ok(MeshEnvironmentMaps {
                texture: (*owner).texture.cast(),
                zone: if zone.is_null() { null_mut() } else { (*zone).environment_map.cast() },
                level: (*level).environment_map.cast(),
            })
        }
    }
    fn light_vertex(&mut self, vertex: &mut LodLightVertex) -> Result<(), ()> {
        unsafe { call(self.0, 17, 0, vertex) }
    }
    fn light_and_fog(&mut self, vertex: &mut crate::light::FogVertex, flags: u32) -> bool {
        unsafe {
            crate::light_manager::light_and_fog(&*self.0.render_services, vertex, flags)
        }
    }
    fn begin_polygons(&mut self, visible: i32) -> Result<(), ()> { unsafe { event(self.0, 18, visible) } }
    fn finish_polygons(&mut self) -> Result<(), ()> { unsafe { event(self.0, 19, 0) } }
    fn unlock_texture(&mut self, index: i32) -> Result<(), ()> { unsafe { event(self.0, 20, index) } }
    fn unbind_environment(&mut self, environment: &mut *mut UTexture) -> Result<(), ()> {
        unsafe { call(self.0, 21, 0, environment) }
    }
    fn end_mesh(&mut self) -> Result<(), ()> { unsafe { event(self.0, 22, 0) } }
    fn texture_transition_complete(&mut self) -> Result<(), ()> { unsafe { event(self.0, 23, 0) } }
}

/// Runs mesh drawing through the existing Native ABI adapter.
///
/// # Safety
/// Input pointers and callback behavior must satisfy the Native mesh draw contract.
pub unsafe fn draw(input: &MeshDrawInput) -> Result<(), ()> {
    let mut shared = input.shared.geometry();
    shared.span = *input.span;
    let state = MeshDrawState {
        shared,
        memory: input.shared.memory,
        memory_error: input.shared.memory_error,
        alignment: input.shared.alignment,
        slow_checks: input.shared.slow_checks != 0,
        live: input.live,
        error_stage: input.error_stage,
        frame_verts: input.frame_verts,
        triangles: input.triangles,
        connects: input.connects,
        links: input.links,
        not_weapon: input.not_weapon,
        editor: input.editor,
        scale_glow: input.scale_glow,
        ambient_glow: input.ambient_glow,
        fatness: input.fatness,
        mirror: input.mirror,
        flags: input.flags,
        light_count_stat: input.light_count_stat,
        owner: input.owner.cast(),
    };
    unsafe { draw_with_ops(&state, &mut LegacyMeshDrawOps(input)) }
}
