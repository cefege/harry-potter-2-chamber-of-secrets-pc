//! Shared mesh clipping, indexed batching and process-wide renderer state.
//! SDK callbacks receive raw views only: no borrow of the singleton survives a call.
use core::{
    ffi::c_void,
    ptr::{addr_of_mut, null_mut},
};
use hp2_abi::engine::generated::FMemStack;
use hp2_abi::render_particle::FTransTexture;
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::{
    math,
    mem::{self, Hp2MemServices},
};

#[repr(C)]
pub struct MeshTextureInfo {
    pub texture: *mut c_void,
    pub cache_id: u64,
    pub palette_cache_id: u64,
    pub pan: FVector,
    pub max_color: *mut c_void,
    pub format: i32,
    pub u_scale: f32,
    pub v_scale: f32,
    pub u_size: i32,
    pub v_size: i32,
    pub u_clamp: i32,
    pub v_clamp: i32,
    pub num_mips: i32,
    pub lod: i32,
    pub palette: *mut c_void,
    pub bits: u32,
    pub mips: [*mut c_void; 12],
}
#[repr(C)]
pub struct MeshSharedStorage {
    pub u_scale: f32,
    pub v_scale: f32,
    pub textures: [*mut c_void; 16],
    pub texture_info: [MeshTextureInfo; 16],
    pub environment_info: MeshTextureInfo,
    pub unlit_color: FVector,
}
static mut STORAGE: MeshSharedStorage = unsafe { core::mem::zeroed() };
// These are the SDK's original data symbols, not mirrored publication copies.
pub static mut HasSpecialCoords: i32 = 0;
pub static mut SpecialCoords: FCoords = unsafe { core::mem::zeroed() };
pub fn storage() -> *mut MeshSharedStorage {
    addr_of_mut!(STORAGE)
}
pub unsafe fn texture(index: i32) -> crate::mesh_lod_pipeline::LodTexture {
    unsafe {
        let s = storage();
        let object = if index >= 0 {
            (*s).textures[index as usize].cast::<hp2_abi::engine::generated::UTexture>()
        } else {
            null_mut()
        };
        let info = if index >= 0 {
            addr_of_mut!((*s).texture_info)
                .cast::<MeshTextureInfo>()
                .add(index as usize)
                .cast::<hp2_abi::engine::generated::FTextureInfo>()
        } else {
            addr_of_mut!((*s).environment_info).cast::<hp2_abi::engine::generated::FTextureInfo>()
        };
        crate::mesh_lod_pipeline::LodTexture {
            object,
            info,
            u_scale: (*info).u_scale,
            v_scale: (*info).v_scale,
            lod: 0,
            u_size: (*info).u_size,
            v_size: (*info).v_size,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MeshSharedInput {
    pub context: *mut c_void,
    pub service: unsafe extern "C" fn(*mut c_void, u32, *mut c_void) -> i32,
    pub frame: *mut c_void,
    pub span: *mut c_void,
    pub memory: *mut c_void,
    pub memory_services: *const Hp2MemServices,
    pub memory_error: *mut i32,
    pub slow_checks: i32,
    pub alignment: i32,
    pub clip: [*const f32; 4],
    pub projection: *const f32,
    pub screen: [*const f32; 4], // FX15,FY15,FX,FY
    pub near_clip: *const FPlane,
    pub uncoords: *const FCoords,
}

/// Callback-free geometry and batch identity consumed by mesh draw policy.
#[derive(Clone, Copy)]
pub struct MeshSharedGeometry {
    pub frame: *mut c_void,
    pub span: *mut c_void,
    pub clip: [*const f32; 4],
    pub projection: *const f32,
    pub screen: [*const f32; 4],
    pub near_clip: *const FPlane,
    pub uncoords: *const FCoords,
}

impl MeshSharedInput {
    /// Returns the callback-free fields used by shared geometry calculations.
    pub fn geometry(&self) -> MeshSharedGeometry {
        MeshSharedGeometry {
            frame: self.frame,
            span: self.span,
            clip: self.clip,
            projection: self.projection,
            screen: self.screen,
            near_clip: self.near_clip,
            uncoords: self.uncoords,
        }
    }
}

/// Host leaves required by the shared mesh allocation, clipping, and batch policy.
pub trait MeshSharedOps {
    type Error;

    unsafe fn allocate_bytes(
        &mut self,
        memory: *mut c_void,
        bytes: usize,
        alignment: i32,
        slow_checks: bool,
        memory_error: *mut i32,
    ) -> Result<*mut c_void, Self::Error>;
    fn batch_capacity(&mut self) -> Result<usize, Self::Error>;
    fn draw_triangles(&mut self, batch: &mut MeshBatchDraw) -> Result<(), Self::Error>;
    fn subsurface_begin(&mut self) -> Result<(), Self::Error>;
    fn surface_ready(&mut self) -> Result<(), Self::Error>;
    fn surface_timer_begin(&mut self) -> Result<(), Self::Error>;
    fn surface_timer_end(&mut self) -> Result<(), Self::Error>;
    fn check_allocation_size(&mut self, bytes: &mut usize) -> Result<(), Self::Error>;
}

/// Allocation state passed to callback-free shared mesh policy.
#[derive(Clone, Copy)]
pub struct MeshSharedState {
    pub geometry: MeshSharedGeometry,
    pub memory: *mut c_void,
    pub memory_error: *mut i32,
    pub alignment: i32,
    pub slow_checks: bool,
}
#[repr(C)]
pub struct MeshBatchDraw {
    pub frame: *mut c_void,
    pub span: *mut c_void,
    pub texture: *mut c_void,
    pub vertices: *mut *mut FTransTexture,
    pub vertex_count: i32,
    pub indices: *mut u16,
    pub index_count: i32,
    pub flags: u32,
}
struct Batch {
    vertices: Vec<core::mem::MaybeUninit<*mut FTransTexture>>,
    remap: Vec<u16>,
    indices: Vec<core::mem::MaybeUninit<u16>>,
    base: *mut FTransTexture,
    count: usize,
    index_count: usize,
    frame: *mut c_void,
    span: *mut c_void,
    texture: *mut c_void,
    flags: u32,
}
static mut BATCH: Batch = Batch {
    vertices: Vec::new(),
    remap: Vec::new(),
    indices: Vec::new(),
    base: null_mut(),
    count: 0,
    index_count: 0,
    frame: null_mut(),
    span: null_mut(),
    texture: null_mut(),
    flags: 0,
};
unsafe fn service<T>(i: &MeshSharedInput, op: u32, value: *mut T) -> Result<(), ()> {
    if unsafe { (i.service)(i.context, op, value.cast()) } != 0 {
        Ok(())
    } else {
        Err(())
    }
}
struct LegacyMeshSharedOps<'a>(&'a MeshSharedInput);

impl MeshSharedOps for LegacyMeshSharedOps<'_> {
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
                &*self.0.memory_services,
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
        unsafe { service(self.0, 0, &mut capacity)? };
        Ok(capacity as usize)
    }

    fn draw_triangles(&mut self, draw: &mut MeshBatchDraw) -> Result<(), ()> {
        unsafe { service(self.0, 1, draw) }
    }

    fn subsurface_begin(&mut self) -> Result<(), ()> { unsafe { service(self.0, 2, null_mut::<u8>()) } }
    fn surface_ready(&mut self) -> Result<(), ()> { unsafe { service(self.0, 4, null_mut::<u8>()) } }
    fn surface_timer_begin(&mut self) -> Result<(), ()> { unsafe { service(self.0, 3, null_mut::<u8>()) } }
    fn surface_timer_end(&mut self) -> Result<(), ()> { unsafe { service(self.0, 5, null_mut::<u8>()) } }
    fn check_allocation_size(&mut self, bytes: &mut usize) -> Result<(), ()> {
        unsafe { service(self.0, 6, bytes) }
    }
}
pub unsafe fn allocate<T>(i: &MeshSharedInput, count: usize) -> Result<*mut T, ()> {
    let mut ops = LegacyMeshSharedOps(i);
    let state = MeshSharedState {
        geometry: i.geometry(),
        memory: i.memory,
        memory_error: i.memory_error,
        alignment: i.alignment,
        slow_checks: i.slow_checks != 0,
    };
    unsafe { allocate_with_ops(&state, &mut ops, count) }
}

/// Allocates mesh storage through typed host memory operations.
pub unsafe fn allocate_with_ops<T, O: MeshSharedOps>(
    state: &MeshSharedState,
    ops: &mut O,
    count: usize,
) -> Result<*mut T, O::Error> {
    let mut bytes = size_of::<T>().wrapping_mul(count);
    unsafe {
        if bytes > i32::MAX as usize {
            *state.memory_error = 3;
            ops.check_allocation_size(&mut bytes)?;
            *state.memory_error = 0;
        }
        ops.allocate_bytes(
            state.memory,
            bytes,
            state.alignment,
            state.slow_checks,
            state.memory_error,
        )
        .map(|ptr| ptr.cast())
    }
}
pub unsafe fn init(base: *mut FTransTexture, count: i32) {
    unsafe {
        let b = addr_of_mut!(BATCH);
        (*b).base = base;
        (*b).remap.clear();
        (*b).remap.resize(count as usize, u16::MAX);
    }
}
/// Flushes pending mesh batches through typed host leaves.
pub unsafe fn flush_with_ops<O: MeshSharedOps>(ops: &mut O) -> Result<(), O::Error> {
    unsafe {
        let batch = addr_of_mut!(BATCH);
        if (*batch).count != 0 {
            let mut draw = MeshBatchDraw {
                frame: (*batch).frame,
                span: (*batch).span,
                texture: (*batch).texture,
                vertices: (*batch).vertices.as_mut_ptr().cast(),
                vertex_count: (*batch).count as i32,
                indices: (*batch).indices.as_mut_ptr().cast(),
                index_count: (*batch).index_count as i32,
                flags: (*batch).flags,
            };
            ops.draw_triangles(&mut draw)?;
            (*batch).count = 0;
            (*batch).index_count = 0;
            (*batch).remap.fill(u16::MAX);
        }
        Ok(())
    }
}

pub unsafe fn flush(i: &MeshSharedInput) -> Result<(), ()> {
    unsafe { flush_with_ops(&mut LegacyMeshSharedOps(i)) }
}
unsafe fn process_vertex(p: *mut FTransTexture) -> u16 {
    unsafe {
        let b = addr_of_mut!(BATCH);
        if p.addr() >= (*b).base.addr() {
            let bytes = p.addr() - (*b).base.addr();
            if bytes % size_of::<FTransTexture>() == 0 {
                let id = bytes / size_of::<FTransTexture>();
                if id < (*b).remap.len() {
                    let slot = (*b).remap.as_mut_ptr().add(id);
                    if *slot == u16::MAX {
                        *slot = (*b).count as u16;
                        *(*b)
                            .vertices
                            .as_mut_ptr()
                            .cast::<*mut FTransTexture>()
                            .add((*b).count) = p;
                        (*b).count += 1;
                    }
                    return *slot;
                }
            }
        }
        let id = (*b).count;
        *(*b)
            .vertices
            .as_mut_ptr()
            .cast::<*mut FTransTexture>()
            .add(id) = p;
        (*b).count += 1;
        id as u16
    }
}
pub unsafe fn polygon_with_ops<O: MeshSharedOps>(
    geometry: &MeshSharedGeometry,
    ops: &mut O,
    texture: *mut c_void,
    flags: u32,
    points: *const *mut FTransTexture,
    count: usize,
) -> Result<(), O::Error> {
    unsafe {
        let batch = addr_of_mut!(BATCH);
        if geometry.frame != (*batch).frame
            || geometry.span != (*batch).span
            || texture != (*batch).texture
            || flags != (*batch).flags
        {
            flush_with_ops(ops)?;
            if (*batch).vertices.is_empty() {
                let capacity = ops.batch_capacity()?;
                (*batch).vertices.clear();
                (*batch).vertices.reserve_exact(capacity);
                (*batch).vertices.set_len(capacity);
                (*batch).indices.clear();
                (*batch).indices.reserve_exact(capacity * 6);
                (*batch).indices.set_len(capacity * 6);
            }
            (*batch).frame = geometry.frame;
            (*batch).span = geometry.span;
            (*batch).texture = texture;
            (*batch).flags = flags;
        }
        if (*batch).count + count > (*batch).vertices.len() {
            flush_with_ops(ops)?;
        }
        for n in 0..3 {
            let id = process_vertex(*points.add(n));
            *(*batch)
                .indices
                .as_mut_ptr()
                .cast::<u16>()
                .add((*batch).index_count) = id;
            (*batch).index_count += 1;
        }
        for n in 3..count {
            let at = (*batch).index_count;
            let indices = (*batch).indices.as_mut_ptr().cast::<u16>();
            *indices.add(at) = *indices.add(at - 3);
            *indices.add(at + 1) = *indices.add(at - 1);
            *indices.add(at + 2) = process_vertex(*points.add(n));
            (*batch).index_count += 3;
        }
        Ok(())
    }
}

pub unsafe fn polygon(
    i: &MeshSharedInput,
    texture: *mut c_void,
    flags: u32,
    points: *const *mut FTransTexture,
    count: usize,
) -> Result<(), ()> {
    unsafe {
        polygon_with_ops(&i.geometry(), &mut LegacyMeshSharedOps(i), texture, flags, points, count)
    }
}
pub unsafe fn project_geometry(p: *mut FTransTexture, geometry: &MeshSharedGeometry) {
    unsafe {
        (*p).transform.rz = *geometry.projection / (*p).transform.point.z;
        (*p).transform.screen_x = (*p)
            .transform
            .point
            .x
            .mul_add((*p).transform.rz, *geometry.screen[0]);
        (*p).transform.screen_y = (*p)
            .transform
            .point
            .y
            .mul_add((*p).transform.rz, *geometry.screen[1]);
        (*p).transform.int_y = (*p).transform.screen_y.floor() as i32;
    }
}
pub unsafe fn project(p: *mut FTransTexture, i: &MeshSharedInput) {
    unsafe { project_geometry(p, &i.geometry()) }
}
pub unsafe fn outcode_geometry(p: FVector, geometry: &MeshSharedGeometry) -> u8 {
    unsafe {
        (u8::from((*geometry.clip[0]).mul_add(p.z, p.x) < 0.) << 2)
            | (u8::from((*geometry.clip[1]).mul_add(p.z, -p.x) < 0.) << 3)
            | (u8::from((*geometry.clip[2]).mul_add(p.z, p.y) < 0.) << 4)
            | (u8::from((*geometry.clip[3]).mul_add(p.z, -p.y) < 0.) << 5)
    }
}
pub unsafe fn outcode(p: FVector, i: &MeshSharedInput) -> u8 {
    unsafe { outcode_geometry(p, &i.geometry()) }
}
fn interpolate(a: f32, b: f32, t: f32) -> f32 {
    // Native overloaded vector operators round multiply before addition.
    a + (b - a) * t
}
unsafe fn clip_with_ops<O: MeshSharedOps>(
    state: &MeshSharedState,
    ops: &mut O,
    dest: *mut *mut FTransTexture,
    src: *const *mut FTransTexture,
    count: usize,
    dots: &[f32; 32],
) -> Result<usize, O::Error> {
    unsafe {
        let mut output = 0;
        let mut j = count - 1;
        for n in 0..count {
            if dots[j] >= 0. {
                *dest.add(output) = *src.add(j);
                output += 1;
            }
            if dots[j] * dots[n] < 0. {
                let p = allocate_with_ops::<FTransTexture, O>(state, ops, 1)?;
                *dest.add(output) = p;
                let a = *src.add(j);
                let b = *src.add(n);
                let t = dots[j] / (dots[j] - dots[n]);
                // Normal and outcode are intentionally not initialized by native interpolation.
                (*p).transform.point = FVector {
                    x: interpolate((*a).transform.point.x, (*b).transform.point.x, t),
                    y: interpolate((*a).transform.point.y, (*b).transform.point.y, t),
                    z: interpolate((*a).transform.point.z, (*b).transform.point.z, t),
                };
                let pa = addr_of_mut!((*p).light).cast::<f32>();
                let aa = addr_of_mut!((*a).light).cast::<f32>();
                let ba = addr_of_mut!((*b).light).cast::<f32>();
                for k in 0..8 {
                    *pa.add(k) = interpolate(*aa.add(k), *ba.add(k), t);
                }
                (*p).u = interpolate((*a).u, (*b).u, t);
                (*p).v = interpolate((*a).v, (*b).v, t);
                project_geometry(p, &state.geometry);
                output += 1;
            }
            j = n;
        }
        Ok(output)
    }
}
fn clip_zero(value: f32, range: f32) -> f32 {
    if value.to_bits() >= 0x80000000 {
        0.
    } else if value.to_bits() > range.to_bits() {
        range
    } else {
        value
    }
}
pub unsafe fn subsurface_with_ops<O: MeshSharedOps>(
    state: &MeshSharedState,
    ops: &mut O,
    texture: *mut c_void,
    points: *mut *mut FTransTexture,
    flags: u32,
) -> Result<(), O::Error> {
    unsafe {
        let storage = storage();
        if flags & 0x10 != 0 {
            for n in 0..3 {
                let p = *points.add(n);
                let normal = FVector {
                    x: (*p).normal.x,
                    y: (*p).normal.y,
                    z: (*p).normal.z,
                };
                let t = math::vector_transform(
                    math::vector_mirror_by_vector(
                        math::vector_unsafe_normal((*p).transform.point),
                        normal,
                    ),
                    *state.geometry.uncoords,
                );
                let z = if t.z >= 0. { t.z } else { 0. };
                let light = (z as f64).powf(0.25) as f32;
                (*p).light = FPlane { x: light, y: light, z: light, w: 0. };
                (*p).u = (t.x + 1.) * 0.5 * 256. * (*storage).u_scale;
                (*p).v = (t.y + 1.) * 0.5 * 256. * (*storage).v_scale;
            }
        }
        if flags & 0x400000 != 0 {
            for n in 0..3 {
                let p = *points.add(n);
                let c = (*storage).unlit_color;
                (*p).light = FPlane { x: c.x, y: c.y, z: c.z, w: 0. };
            }
        }
        ops.subsurface_begin()?;
        if (**points).transform.flags
            & (**points.add(1)).transform.flags
            & (**points.add(2)).transform.flags
            != 0
        {
            ops.surface_ready()?;
            return Ok(());
        }
        if flags & 0x100 != 0
            && math::triple(
                (**points).transform.point,
                (**points.add(1)).transform.point,
                (**points.add(2)).transform.point,
            ) <= 0.
        {
            core::ptr::swap(points, points.add(2));
        }
        let codes = (**points).transform.flags
            | (**points.add(1)).transform.flags
            | (**points.add(2)).transform.flags;
        let mut clipped = [[null_mut(); 8]; 5];
        let mut src = points as *const *mut FTransTexture;
        let mut count = 3;
        let mut dots = [0.; 32];
        for plane in 0..5 {
            if plane < 4 && codes & (4 << plane) == 0 {
                continue;
            }
            if plane == 4 && (*state.geometry.near_clip).w == 0. {
                continue;
            }
            let mut outside = false;
            for n in 0..count {
                let p = (**src.add(n)).transform.point;
                dots[n] = if plane == 4 {
                    math::plane_dot(*state.geometry.near_clip, p)
                } else {
                    (*state.geometry.clip[plane]).mul_add(
                        p.z,
                        match plane {
                            0 => p.x,
                            1 => -p.x,
                            2 => p.y,
                            _ => -p.y,
                        },
                    )
                };
                outside |= dots[n] < 0.;
            }
            if plane == 4 && !outside {
                continue;
            }
            count = clip_with_ops(state, ops, clipped[plane].as_mut_ptr(), src, count, &dots)?;
            if count == 0 {
                ops.surface_ready()?;
                return Ok(());
            }
            src = clipped[plane].as_ptr();
        }
        for n in 0..count {
            let p = *src.add(n);
            (*p).transform.screen_x =
                clip_zero((*p).transform.screen_x, *state.geometry.screen[2]);
            (*p).transform.screen_y =
                clip_zero((*p).transform.screen_y, *state.geometry.screen[3]);
        }
        ops.surface_ready()?;
        ops.surface_timer_begin()?;
        polygon_with_ops(&state.geometry, ops, texture, flags, src, count)?;
        ops.surface_timer_end()
    }
}

pub unsafe fn subsurface(
    i: &MeshSharedInput,
    texture: *mut c_void,
    points: *mut *mut FTransTexture,
    flags: u32,
) -> Result<(), ()> {
    let state = MeshSharedState {
        geometry: i.geometry(),
        memory: i.memory,
        memory_error: i.memory_error,
        alignment: i.alignment,
        slow_checks: i.slow_checks != 0,
    };
    unsafe { subsurface_with_ops(&state, &mut LegacyMeshSharedOps(i), texture, points, flags) }
}

pub unsafe fn timed_init_with_ops<O: MeshSharedOps>(
    ops: &mut O,
    base: *mut FTransTexture,
    count: i32,
) -> Result<(), O::Error> {
    ops.surface_timer_begin()?;
    unsafe { init(base, count) };
    ops.surface_timer_end()
}

pub unsafe fn timed_flush_with_ops<O: MeshSharedOps>(
    ops: &mut O,
) -> Result<(), O::Error> {
    ops.surface_timer_begin()?;
    unsafe { flush_with_ops(ops)? };
    ops.surface_timer_end()
}

pub unsafe fn timed_init(
    i: &MeshSharedInput,
    base: *mut FTransTexture,
    count: i32,
) -> Result<(), ()> {
    unsafe { timed_init_with_ops(&mut LegacyMeshSharedOps(i), base, count) }
}

pub unsafe fn timed_flush(i: &MeshSharedInput) -> Result<(), ()> {
    unsafe { timed_flush_with_ops(&mut LegacyMeshSharedOps(i)) }
}
/// Diagnostic access used by existing native consumer probes, never alternate storage.
pub unsafe fn pending() -> (i32, i32) {
    unsafe {
        let b = addr_of_mut!(BATCH);
        ((*b).count as i32, (*b).index_count as i32)
    }
}
pub unsafe fn discard_pending() {
    unsafe {
        let b = addr_of_mut!(BATCH);
        (*b).count = 0;
        (*b).index_count = 0;
    }
}
