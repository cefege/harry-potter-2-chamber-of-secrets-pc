//! Canonical Rust ClassOps for Engine's AActor record.

use crate::{actor, prim};
use hp2_abi::engine::generated::{
    self, AActor, ALevelInfo, FFrame, FStateFrame, UBoxPrim, UEngine, UFunction, ULevel, UModel, UObject,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{
    NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind,
};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, ffi::c_void, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const RF_PUBLIC: u32 = 4;
const RF_TRANSACTIONAL: u32 = 0x0000_0001;
const CPF_EDIT: u32 = 0x0000_0001;
const COLLISION_FLAGS_OFFSET: usize = std::mem::offset_of!(AActor, light_type) - 4;
const COLLISION_PROPERTIES: &[StartupProperty] = &[
    StartupProperty {
        name: "bAlignBottom",
        kind: StartupPropertyKind::Bool { bit_mask: 64 },
        category: "Collision",
        property_flags: CPF_EDIT,
        object_flags: RF_PUBLIC,
        inner_name: None,
        offset: COLLISION_FLAGS_OFFSET,
        array_dim: 1,
    },
    StartupProperty {
        name: "bAlignBottomAlways",
        kind: StartupPropertyKind::Bool { bit_mask: 128 },
        category: "Collision",
        property_flags: CPF_EDIT,
        object_flags: RF_PUBLIC,
        inner_name: None,
        offset: COLLISION_FLAGS_OFFSET,
        array_dim: 1,
    },
    StartupProperty {
        name: "bBlockCamera",
        kind: StartupPropertyKind::Bool { bit_mask: 256 },
        category: "Collision",
        property_flags: CPF_EDIT,
        object_flags: RF_PUBLIC,
        inner_name: None,
        offset: COLLISION_FLAGS_OFFSET,
        array_dim: 1,
    },
];

/// Engine's canonical Actor class declaration, to be registered after its Core superclass.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Actor",
    package: "Engine",
    super_name: Some("Object"),
    within: "Object",
    flags: generated::ACTOR_CLASS_FLAGS,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<AActor>()),
    operations: Some(ACTOR_OPERATIONS),
    properties: COLLISION_PROPERTIES,
    defaults: &[],
},
// These native subclasses add script-backed fields but no object-lifecycle
// overrides or native static constructors. Their package metadata supplies the
// reflected properties; object operations are inherited from Actor.
NativeClassSpec {
    short_name: "Decoration", package: "Engine", super_name: Some("Actor"), within: "Object",
    flags: 0, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<generated::ADecoration>()),
    operations: Some(ACTOR_OPERATIONS), properties: &[], defaults: &[],
},
NativeClassSpec {
    short_name: "Projectile", package: "Engine", super_name: Some("Actor"), within: "Object",
    flags: 0, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<generated::AProjectile>()),
    operations: Some(ACTOR_OPERATIONS), properties: &[], defaults: &[],
},
NativeClassSpec {
    short_name: "Inventory", package: "Engine", super_name: Some("Actor"), within: "Object",
    flags: generated::ACTOR_CLASS_FLAGS, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<generated::AInventory>()),
    operations: Some(ACTOR_OPERATIONS), properties: &[], defaults: &[],
},
NativeClassSpec {
    short_name: "Weapon", package: "Engine", super_name: Some("Inventory"), within: "Object",
    flags: generated::ACTOR_CLASS_FLAGS, object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<generated::AWeapon>()),
    operations: Some(ACTOR_OPERATIONS), properties: &[], defaults: &[],
}];

pub(crate) const ACTOR_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.destroy = destroy_actor;
    operations.post_load = post_load_actor;
    operations.initialize_execution = initialize_actor_execution;
    operations.post_edit_change = post_edit_change_actor;
    operations.execute_event = execute_actor_event;
    operations.is_pending_kill = pending_kill_actor;
    operations.process_remote_function = process_remote_function_actor;
    operations
};

unsafe fn pending_kill_actor(
    _access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<bool> {
    if object.is_null() {
        return Err(RuntimeError::new("Null Actor in IsPendingKill"));
    }
    Ok(unsafe { actor::is_pending_kill(object.cast::<AActor>()) })
}

unsafe fn process_remote_function_actor(
    access: CoreAccess<'_>,
    object: *mut UObject,
    function: *mut UFunction,
    parameters: *mut c_void,
    stack: *mut FFrame,
) -> RuntimeResult<bool> {
    unsafe { crate::actor_rpc_owner::process_remote(access, object.cast(), function, parameters, stack) }
}

/// Tears down Actor-owned resources in native order, then delegates to UObject.
///
/// # Safety
/// `object` must be a live canonical AActor; all stored references must be canonical objects.
unsafe fn destroy_actor(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let render_interface = unsafe { (*actor).render_interface.cast::<UObject>() };
    if !render_interface.is_null() {
        access.remove_root(render_interface);
        unsafe { access.conditional_destroy(render_interface)?; }
        unsafe { access.delete_object(render_interface)?; }
        unsafe { (*actor).render_interface = ptr::null_mut(); }
    }

    // Match the native for-array loop: preserve the header and re-read both
    // Num and Data after each potentially reentrant conditional destruction.
    let mut index = 0;
    loop {
        let (num, data) = unsafe { ((*actor).aux_anims.num, (*actor).aux_anims.data) };
        if num < 0 || (num > 0 && data.is_null()) {
            return Err(RuntimeError::new("Invalid Actor auxiliary animation array"));
        }
        if index >= num { break; }
        let animation = unsafe { *data.cast::<*mut UObject>().add(index as usize) };
        if !animation.is_null() {
            unsafe { access.conditional_destroy(animation)?; }
        }
        index += 1;
    }
    // Core's reflected constructor-link exit releases AuxAnims storage and the
    // implicit FStringNoInit members (ExcludeGameStates, CutName,
    // CutErrorString, sCutNotifyCue). Only the array elements' native
    // conditional-destroy policy is handled above; freeing those values here
    // as well would double-destroy them.
    unsafe { (OBJECT.destroy)(access, object) }
}

/// Executes AActor's genuine InitExecution sequence and enforces its world invariants.
///
/// # Safety
/// `object` must be a live canonical AActor already published in CoreOwner.
unsafe fn initialize_actor_execution(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: Core dispatch selected this canonical object and this is its UObject base operation.
    unsafe { (OBJECT.initialize_execution)(access, object)?; }
    let actor = object.cast::<AActor>();
    // SAFETY: Native InitExecution checks these fields immediately after UObject initialization.
    let (frame, level, x_level) = unsafe {
        ((*object).state_frame.cast::<FStateFrame>(), (*actor).level, (*actor).x_level.cast::<ULevel>())
    };
    if frame.is_null() { return Err(RuntimeError::new("Actor execution frame was not initialized")); }
    if unsafe { (*frame).object } != object.cast() { return Err(RuntimeError::new("Actor execution frame refers to another object")); }
    if x_level.is_null() { return Err(RuntimeError::new("Actor has no XLevel")); }
    let actors = unsafe { ptr::addr_of!((*x_level).base.actors).read_unaligned() };
    if actors.array_num <= 0 || actors.data.is_null() {
        return Err(RuntimeError::new("Actor XLevel has no level-info actor"));
    }
    // SAFETY: compiler-probed TTransArray data begins with the AActor* array.
    let first_actor = unsafe { *actors.data.cast::<*mut c_void>() };
    if first_actor.is_null() { return Err(RuntimeError::new("Actor XLevel actor zero is null")); }
    if first_actor != level { return Err(RuntimeError::new("Actor Level does not match XLevel actor zero")); }
    if level.is_null() { return Err(RuntimeError::new("Actor has no LevelInfo")); }
    crate::live_trace_owner::post_init_execution(access, actor);
    Ok(())
}


/// Performs AActor PostLoad localization, brush transaction marking, and shape validation.
unsafe fn post_load_actor(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: native AActor::PostLoad starts with UObject::PostLoad.
    unsafe { (OBJECT.post_load)(access, object)?; }
    unsafe { access.load_localized(object)?; }
    let actor = object.cast::<AActor>();
    let brush = unsafe { (*actor).brush.cast::<UModel>() };
    if !brush.is_null() {
        unsafe { (*brush.cast::<UObject>()).object_flags |= RF_TRANSACTIONAL; }
        let polys = unsafe { (*brush).polys.cast::<UObject>() };
        if !polys.is_null() { unsafe { (*polys).object_flags |= RF_TRANSACTIONAL; } }
    }
    if unsafe { (*actor).collide_type } == actor::CT_SHAPE
        && (unsafe { !(*actor).brush.is_null() } || unsafe { !(*actor).mesh.is_null() })
    {
        let primitive = unsafe { get_primitive(actor)? };
        unsafe { validate_actor_for_primitive(access, primitive, actor)?; }
    }
    Ok(())
}

/// Applies the native AActor begin-play gate before UObject event execution.
unsafe fn execute_actor_event(
    access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction,
    parameters: *mut c_void, result: *mut c_void,
) -> RuntimeResult<()> {
    let actor = object.cast::<AActor>();
    let level = unsafe { (*actor).level.cast::<ALevelInfo>() };
    if level.is_null() { return Err(RuntimeError::new("Actor has no LevelInfo during event dispatch")); }
    if unsafe { (*level).b_begun_play() } {
        unsafe { (OBJECT.execute_event)(access, object, function, parameters, result)?; }
    }
    Ok(())
}

/// Applies UObject's edit notification followed by AActor's editor light invalidation.
unsafe fn post_edit_change_actor(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.post_edit_change)(access, object)?; }
    if access.process_mode().is_editor {
        unsafe { (*object.cast::<AActor>()).set_b_light_changed(true); }
    }
    Ok(())
}

/// Executes AActor's Spawned validation through canonical primitive ownership.
///
/// # Safety
/// `actor` is a live canonical `AActor` whose level and engine references are initialized.
pub(crate) unsafe fn spawned(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<()> {
    if actor.is_null() { return Err(RuntimeError::new("Null Actor in Spawned")); }
    if unsafe { (*actor).collide_type } == actor::CT_SHAPE {
        let primitive = unsafe { get_primitive(actor)? };
        unsafe { validate_actor_for_primitive(access, primitive, actor)?; }
    }
    Ok(())
}

/// Resolves AActor::GetPrimitive using the Actor's real mesh, brush, and ULevel.Engine fields.
///
/// # Safety
/// `actor` must be a live canonical `AActor` with canonical `XLevel` and Engine references.
pub(crate) unsafe fn get_primitive(actor: *mut AActor) -> RuntimeResult<*mut UObject> {
    unsafe {
        actor::get_primitive_with(actor, |level, slot| {
            let level = level.cast::<ULevel>();
            if level.is_null() { return Err(RuntimeError::new("Actor has no XLevel for primitive lookup")); }
            let engine = (*level).base.engine.cast::<UEngine>();
            if engine.is_null() { return Err(RuntimeError::new("Actor XLevel has no Engine for primitive lookup")); }
            let primitive = match slot {
                actor::CT_ORIENTED_CYLINDER => (*engine).o_cylinder,
                actor::CT_BOX => (*engine).r#box,
                actor::CT_ALIGNED_OVAL_CYLINDER => (*engine).oval_cylinder,
                actor::CT_ORIENTED_OVAL_CYLINDER => (*engine).o_oval_cylinder,
                _ => (*engine).cylinder,
            };
            if primitive.is_null() { return Err(RuntimeError::new("Engine primitive is not initialized")); }
            Ok(primitive)
        }).map(|primitive| primitive.cast())
    }
}

/// Performs virtual ValidateActor dispatch by canonical class hierarchy, never by native vptr.
unsafe fn validate_actor_for_primitive(
    access: CoreAccess<'_>, primitive: *mut UObject, actor: *mut AActor,
) -> RuntimeResult<()> {
    if primitive.is_null() { return Err(RuntimeError::new("Actor primitive is null")); }
    let box_prim_class = access.native_class("Engine.BoxPrim")?;
    if !unsafe { hp2_core::object_model::is_a(primitive.cast(), box_prim_class.cast()) } {
        return Ok(());
    }
    let bounds = unsafe { &(*primitive.cast::<UBoxPrim>()).base.base.bounding_box };
    let aabb = prim::Aabb { min: bounds.min, max: bounds.max, valid: bounds.is_valid != 0 };
    let (radius, height) = prim::boxprim_validate(&aabb);
    unsafe {
        (*actor).collision_radius = radius;
        (*actor).collision_width = radius;
        (*actor).collision_height = height;
    }
    Ok(())
}

/// Invokes AActor's ProcessEvent override over a canonical actor.
///
/// # Safety
/// Actor/Level/function and the function's parameter/result storage are live.
/// The event may reenter the owner; no state borrow may span this call.
pub(crate) unsafe fn process_event(
    access: CoreAccess<'_>, actor: *mut AActor, function: *mut UFunction,
    parameters: *mut c_void, result: *mut c_void,
) -> RuntimeResult<()> {
    let level = unsafe { (*actor).level.cast::<ALevelInfo>() };
    actor::process_event_with(unsafe { (*level).b_begun_play() }, || unsafe {
        access.execute_event(actor.cast(), function, parameters, result)
    })
}
