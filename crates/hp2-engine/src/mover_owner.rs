//! Canonical Rust ownership and lifecycle for Engine's `AMover`.

use crate::{actor_owner, dynbsp_owner, level_owner, mover};
use hp2_abi::engine::generated::{
    AActor, AMover, FPoly, ULevel, UModel, UObject, UPolys,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, ptr};

const CLASS_NATIVE_REPLICATION: u32 = 0x0000_0800;
const CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const OBJECT: ClassOps = NativeRecordKind::Object.operations();

/// Canonical Mover class declaration, laid out over its physical `AMover` mirror.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Mover",
    package: "Engine",
    super_name: Some("Brush"),
    within: "Object",
    flags: CLASS_NATIVE_REPLICATION,
    object_flags: CLASS_OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<AMover>()),
    operations: Some(MOVER_OPERATIONS),
    properties: mover::MOVER_PROPERTIES,
    defaults: &[],
}];

const MOVER_OPERATIONS: ClassOps = {
    let mut operations = actor_owner::ACTOR_OPERATIONS;
    operations.construct = OBJECT.construct;
    operations.post_load = post_load;
    operations.post_edit_change = post_edit_change_class;
    operations
};

/// Applies `AActor::PostLoad`, then Mover refresh sentinels and brush-poly links.
///
/// # Safety
/// `object` is a live canonical `AMover`; its brush and polygon array are canonical.
unsafe fn post_load(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (actor_owner::ACTOR_OPERATIONS.post_load)(access, object)?; }
    let mut ops = CanonicalMoverOps { access, level: None };
    unsafe { mover::post_load_with_ops(object.cast::<AMover>(), &mut ops) }
}

/// Applies Actor's edit notification and then Mover's keyframe correction policy.
///
/// # Safety
/// `object` is a live canonical `AMover` on the engine thread.
unsafe fn post_edit_change_class(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (actor_owner::ACTOR_OPERATIONS.post_edit_change)(access, object)?; }
    unsafe { mover::post_edit_change(&mut *object.cast::<AMover>()); }
    Ok(())
}

/// Dispatches canonical edit notification and Mover keyframe correction.
///
/// # Safety
/// `mover` is a live canonical `AMover`; its class remains valid during dispatch.
pub unsafe fn post_edit_change(access: CoreAccess<'_>, mover: *mut AMover) -> RuntimeResult<()> {
    if mover.is_null() { return Err(RuntimeError::new("Null Mover in PostEditChange")); }
    unsafe { access.post_edit_change(mover.cast()) }
}

struct CanonicalMoverOps<'a> { access: CoreAccess<'a>, level: Option<*mut ULevel> }

impl mover::MoverOps for CanonicalMoverOps<'_> {
    fn hash_remove(&mut self, mover: *mut AMover) -> RuntimeResult<()> {
        let level = self.level.ok_or_else(|| RuntimeError::new("Mover has no XLevel"))?;
        if unsafe { !(*level).hash.is_null() } {
            unsafe { level_owner::hash_remove(self.access, level, mover.cast::<AActor>(), false)?; }
        }
        Ok(())
    }

    fn hash_add(&mut self, mover: *mut AMover) -> RuntimeResult<()> {
        let level = self.level.ok_or_else(|| RuntimeError::new("Mover has no XLevel"))?;
        if unsafe { !(*level).hash.is_null() } {
            unsafe { level_owner::hash_add(self.access, level, mover.cast::<AActor>())?; }
        }
        Ok(())
    }

    fn tracker_update(&mut self, mover: *mut AMover) -> RuntimeResult<()> {
        let level = self.level.ok_or_else(|| RuntimeError::new("Mover has no XLevel"))?;
        if unsafe { !(*level).brush_tracker.is_null() } {
            unsafe { dynbsp_owner::update(self.access, level, mover.cast::<AActor>())?; }
        }
        Ok(())
    }

    fn tracker_flush(&mut self, mover: *mut AMover) -> RuntimeResult<()> {
        let level = self.level.ok_or_else(|| RuntimeError::new("Mover has no XLevel"))?;
        if unsafe { !(*level).brush_tracker.is_null() } {
            unsafe { dynbsp_owner::flush(self.access, level, mover.cast::<AActor>())?; }
        }
        Ok(())
    }

    fn renumber_brush_polys(&mut self, brush: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { renumber_brush_polys(brush.cast::<UModel>()) }
    }
}

/// Rebuilds the native polygon link indices without invoking C++ UObject callbacks.
///
/// # Safety
/// `model` is null or a live canonical `UModel` whose polygon array is valid.
unsafe fn renumber_brush_polys(model: *mut UModel) -> RuntimeResult<()> {
    if model.is_null() { return Ok(()); }
    let polys = unsafe { (*model).polys.cast::<UPolys>() };
    if polys.is_null() { return Ok(()); }
    let elements = unsafe { ptr::addr_of!((*polys).element).read_unaligned() };
    if elements.array_num < 0 || (elements.array_num > 0 && elements.data.is_null()) {
        return Err(RuntimeError::new("Invalid Mover brush polygon array"));
    }
    for index in 0..elements.array_num as usize {
        let polygon = unsafe { elements.data.cast::<FPoly>().add(index) };
        unsafe { ptr::addr_of_mut!((*polygon).i_link).write_unaligned(index as i32); }
    }
    Ok(())
}

/// Runs canonical AActor Spawned validation, then Mover's keyed-position policy.
///
/// # Safety
/// `mover` is a live canonical `AMover` with initialized level and Engine references.
pub(crate) unsafe fn spawned(access: CoreAccess<'_>, mover: *mut AMover) -> RuntimeResult<()> {
    if mover.is_null() { return Err(RuntimeError::new("Null Mover in Spawned")); }
    unsafe { actor_owner::spawned(access, mover.cast::<AActor>())?; }
    unsafe { mover::spawned(&mut *mover); }
    Ok(())
}

/// Runs Mover's `PostEditMove` policy after Brush's editor policy.
///
/// # Safety
/// `mover` is an exclusively mutable live canonical `AMover`.
pub unsafe fn post_edit_move(mover: *mut AMover) -> RuntimeResult<()> {
    if mover.is_null() { return Err(RuntimeError::new("Null Mover in PostEditMove")); }
    unsafe { mover::post_edit_move(&mut *mover); }
    Ok(())
}

/// Places the mover in its pre-raytrace origin pose after Brush preparation.
///
/// # Safety
/// `mover` is an exclusively mutable live canonical `AMover`.
pub unsafe fn pre_raytrace(mover: *mut AMover) -> RuntimeResult<()> {
    if mover.is_null() { return Err(RuntimeError::new("Null Mover in PreRaytrace")); }
    unsafe { mover::pre_raytrace(&mut *mover); }
    Ok(())
}

unsafe fn mover_level(mover: *mut AMover) -> RuntimeResult<*mut ULevel> {
    if mover.is_null() { return Err(RuntimeError::new("Null Mover")); }
    let level = unsafe { (*mover).base.x_level.cast::<ULevel>() };
    if level.is_null() { return Err(RuntimeError::new("Mover has no XLevel")); }
    Ok(level)
}

/// Applies the mover's world-raytrace key through canonical hash and tracker owners.
///
/// # Safety
/// `mover` is an exclusively mutable live canonical `AMover` with a valid `XLevel`.
pub unsafe fn set_world_raytrace_key(access: CoreAccess<'_>, mover: *mut AMover) -> RuntimeResult<()> {
    let level = unsafe { mover_level(mover)? };
    let mut ops = CanonicalMoverOps { access, level: Some(level) };
    unsafe { mover::set_world_raytrace_key_with_ops(mover, &mut ops) }
}

/// Applies the mover's brush-raytrace key through canonical hash and tracker owners.
///
/// # Safety
/// `mover` is an exclusively mutable live canonical `AMover` with a valid `XLevel`.
pub unsafe fn set_brush_raytrace_key(access: CoreAccess<'_>, mover: *mut AMover) -> RuntimeResult<()> {
    let level = unsafe { mover_level(mover)? };
    let mut ops = CanonicalMoverOps { access, level: Some(level) };
    unsafe { mover::set_brush_raytrace_key_with_ops(mover, &mut ops) }
}

/// Restores the mover after world raytracing using canonical hash and tracker owners.
///
/// # Safety
/// `mover` is an exclusively mutable live canonical `AMover` with a valid `XLevel`.
pub unsafe fn post_raytrace(access: CoreAccess<'_>, mover: *mut AMover) -> RuntimeResult<()> {
    let level = unsafe { mover_level(mover)? };
    let mut ops = CanonicalMoverOps { access, level: Some(level) };
    unsafe { mover::post_raytrace_with_ops(mover, &mut ops) }
}
