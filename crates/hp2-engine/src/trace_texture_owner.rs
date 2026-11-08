use hp2_abi::engine::generated::{AActor, FCheckResult, FBspSurf, FDecal, ULevel, UModel};
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::c_void, ptr};

pub(crate) unsafe fn trace_texture(access: CoreAccess<'_>, actor: *mut AActor, end: FVector, start: FVector, flags: *mut i32, decals: bool) -> RuntimeResult<*mut c_void> {
    struct Backend<'a> { access: CoreAccess<'a> }
    impl crate::actor::TraceTextureOps for Backend<'_> {
        type Error = RuntimeError;
        unsafe fn trace_texture_hit(&mut self, actor: *mut AActor, end: FVector, start: FVector) -> RuntimeResult<(*mut FBspSurf, FVector)> {
            let level = unsafe { ptr::addr_of!((*actor).x_level).read_unaligned().cast::<ULevel>() };
            let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
            let zero = FVector { x: 0.0, y: 0.0, z: 0.0 };
            // TRACE_Level selects the world BSP; movers are excluded, so MoverToLocal is None.
            let clear = unsafe { crate::level_trace_owner::single_line_check(self.access, level, &mut hit, actor, end, start, 0x04, zero, 0)? };
            if clear { return Ok((ptr::null_mut(), hit.location)); }
            let model_class = self.access.native_class("Engine.Model")?;
            if !hp2_core::object_model::is_a(hit.primitive, model_class.cast()) {
                return Ok((ptr::null_mut(), hit.location));
            }
            let model = hit.primitive.cast::<UModel>();
            let Some(surf) = (unsafe { bsp_surface_for_hit(model, &hit)? }) else {
                return Ok((ptr::null_mut(), hit.location));
            };
            Ok((surf, hit.location))
        }
        unsafe fn actor_level_model(&mut self, actor: *mut AActor) -> RuntimeResult<*mut UModel> {
            let level = unsafe { ptr::addr_of!((*actor).x_level).read_unaligned().cast::<ULevel>() };
            Ok(unsafe { ptr::addr_of!((*level).model).read_unaligned().cast() })
        }
        unsafe fn surface_texture(&mut self, surf: *mut FBspSurf) -> RuntimeResult<*mut c_void> { Ok(unsafe { ptr::addr_of!((*surf).texture).read_unaligned() }) }
        unsafe fn surface_flags(&mut self, surf: *mut FBspSurf) -> RuntimeResult<u32> { Ok(unsafe { ptr::addr_of!((*surf).poly_flags).read_unaligned() }) }
        unsafe fn texture_flags(&mut self, texture: *mut c_void) -> RuntimeResult<u32> { Ok(unsafe { ptr::addr_of!((*texture.cast::<hp2_abi::engine::generated::UTexture>()).poly_flags).read_unaligned() }) }
        unsafe fn decal_count(&mut self, surf: *mut FBspSurf) -> RuntimeResult<i32> { Ok(unsafe { ptr::addr_of!((*surf).decals).read_unaligned().num }) }
        unsafe fn surface_normal_index(&mut self, surf: *mut FBspSurf) -> RuntimeResult<i32> { Ok(unsafe { ptr::addr_of!((*surf).v_normal).read_unaligned() }) }
        unsafe fn surface_base_index(&mut self, surf: *mut FBspSurf) -> RuntimeResult<i32> { Ok(unsafe { ptr::addr_of!((*surf).p_base).read_unaligned() }) }
        unsafe fn model_vector(&mut self, model: *mut UModel, index: i32) -> RuntimeResult<*const c_void> {
            let array = unsafe { ptr::addr_of!((*model).vectors).read_unaligned() };
            unsafe { array_element::<FVector>(array.data, array.array_num, index).map(|p| p.cast_const().cast()) }
        }
        unsafe fn model_point(&mut self, model: *mut UModel, index: i32) -> RuntimeResult<*const c_void> {
            let array = unsafe { ptr::addr_of!((*model).points).read_unaligned() };
            unsafe { array_element::<FVector>(array.data, array.array_num, index).map(|p| p.cast_const().cast()) }
        }
        unsafe fn decal_at(&mut self, surf: *mut FBspSurf, index: i32) -> RuntimeResult<*mut FDecal> {
            let array = unsafe { ptr::addr_of!((*surf).decals).read_unaligned() };
            unsafe { array_element(array.data, array.num, index) }
        }
        unsafe fn decal_vertices(&mut self, decal: *mut FDecal) -> RuntimeResult<*const [FVector; 4]> {
            Ok(unsafe { ptr::addr_of!((*decal).vertices) })
        }
        unsafe fn decal_texture(&mut self, decal: *mut FDecal) -> RuntimeResult<*mut c_void> {
            let actor = unsafe { ptr::addr_of!((*decal).actor).read_unaligned().cast::<AActor>() };
            if actor.is_null() { return Ok(ptr::null_mut()); }
            Ok(unsafe { ptr::addr_of!((*actor).texture).read_unaligned() })
        }
    }
    unsafe { crate::actor::trace_texture_with_ops(&mut Backend { access }, actor, flags, end, start, decals) }
}

unsafe fn array_element<T>(data: *mut c_void, num: i32, index: i32) -> RuntimeResult<*mut T> {
    if index < 0 || num < 0 || index >= num || data.is_null() {
        return Err(RuntimeError::new("TraceTexture array index out of range"));
    }
    Ok(unsafe { data.cast::<T>().add(index as usize) })
}

unsafe fn bsp_surface_for_hit(model: *mut UModel, hit: &FCheckResult) -> RuntimeResult<Option<*mut FBspSurf>> {
    let nodes = unsafe { ptr::addr_of!((*model).nodes).read_unaligned() };
    let points = unsafe { ptr::addr_of!((*model).points).read_unaligned() };
    let verts = unsafe { ptr::addr_of!((*model).verts).read_unaligned() };
    let surfs = unsafe { ptr::addr_of!((*model).surfs).read_unaligned() };
    if hit.item < 0 || hit.item >= nodes.array_num || nodes.array_num < 0
        || (nodes.array_num > 0 && nodes.data.is_null())
        || points.array_num < 0 || (points.array_num > 0 && points.data.is_null())
        || verts.array_num < 0 || (verts.array_num > 0 && verts.data.is_null())
        || surfs.array_num < 0 || (surfs.array_num > 0 && surfs.data.is_null())
    { return Err(RuntimeError::new("TraceTexture malformed BSP model arrays or hit node")); }
    let arrays = crate::prim::BspArrays {
        nodes: if nodes.array_num == 0 { &[] } else { unsafe { std::slice::from_raw_parts(nodes.data.cast::<hp2_abi::render_scene::FBspNode>(), nodes.array_num as usize) } },
        points: if points.array_num == 0 { &[] } else { unsafe { std::slice::from_raw_parts(points.data.cast::<FVector>(), points.array_num as usize) } },
        verts: if verts.array_num == 0 { &[] } else { unsafe { std::slice::from_raw_parts(verts.data.cast::<hp2_abi::render_scene::FVert>(), verts.array_num as usize) } },
        surfs: if surfs.array_num == 0 { &[] } else { unsafe { std::slice::from_raw_parts(surfs.data.cast::<hp2_abi::render_scene::FBspSurf>(), surfs.array_num as usize) } },
    };
    let Some(index) = crate::prim::bsp_surf(Some(&arrays), hit.item, hit.location, hit.normal, 0.0, None) else { return Ok(None); };
    if index >= arrays.surfs.len() { return Err(RuntimeError::new("TraceTexture BSP surface index out of range")); }
    Ok(Some(unsafe { surfs.data.cast::<FBspSurf>().add(index) }))
}
