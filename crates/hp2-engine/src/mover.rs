//! Port of `Engine/Src/UnMover.cpp` (`AMover`). Rust owns the class table
//! (`StaticConstructor`, the property list in original order) and the mover
//! bodies. C++ keeps the UObject plumbing (UProperty/UStruct/UEnum creation,
//! the collision hash and the brush tracker). Rust reaches that plumbing only
//! through `Hp2MoverServices`, which the C++ side fills once in
//! `Hp2MoverServices.cpp`. The C++ shells keep the base-class calls
//! (`ABrush::Spawned()` and so on) and the `guard`/`unguard` frames.
//!
//! Divergences from the C++ (documented, not claimed as matching):
//! - `KeyPos[KeyNum]` / `KeyRot[KeyNum]` with an out-of-range `KeyNum` is
//!   undefined behaviour in C++; Rust indexing panics, which the FFI guard
//!   catches.
//! - `GetLevel()` is dereferenced unchecked in C++; Rust asserts non-null.
//! - `FRotator` component arithmetic is plain signed int in C++ (overflow is
//!   UB); Rust uses wrapping ops, which is what the shipped arm64 code does.

use std::ffi::{CStr, c_char, c_void};
use std::mem::{offset_of, size_of};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AMover, FRotator, ULevel};

/// `sizeof(BITFIELD)`: `BITFIELD` is `std::uint32_t` (UnGnuG.h).
const BITFIELD_SIZE: i32 = 4;
/// `ARRAY_COUNT(KeyPos)`.
const KEY_COUNT: i32 = 8;

use hp2_core::core_class_specs::{
    StartupProperty, StartupPropertyKind, StartupScalarKind, StartupStructField,
};

const VECTOR_FIELDS: &[StartupStructField] = &[
    StartupStructField { name: "X", kind: StartupScalarKind::Float, offset: offset_of!(FVector, x) },
    StartupStructField { name: "Y", kind: StartupScalarKind::Float, offset: offset_of!(FVector, y) },
    StartupStructField { name: "Z", kind: StartupScalarKind::Float, offset: offset_of!(FVector, z) },
];
const ROTATOR_FIELDS: &[StartupStructField] = &[
    StartupStructField { name: "Pitch", kind: StartupScalarKind::Int, offset: offset_of!(FRotator, pitch) },
    StartupStructField { name: "Yaw", kind: StartupScalarKind::Int, offset: offset_of!(FRotator, yaw) },
    StartupStructField { name: "Roll", kind: StartupScalarKind::Int, offset: offset_of!(FRotator, roll) },
];
const ENCROACH_VALUES: &[&str] = &[
    "ME_StopWhenEncroach", "ME_ReturnWhenEncroach", "ME_CrushWhenEncroach",
    "ME_IgnoreWhenEncroach", "ME_ReallyStopWhenEncroach", "ME_ReallyReturnWhenEncroach",
    "ME_ReallyCrushWhenEncroach", "ME_ReallyIgnoreWhenEncroach",
];
const GLIDE_VALUES: &[&str] = &["MV_MoveByTime", "MV_GlideByTime", "MV_SpringByTime"];
const BUMP_VALUES: &[&str] = &["BT_PlayerBump", "BT_PawnBump", "BT_AnyBump"];

const fn mover_property(name: &'static str, kind: StartupPropertyKind, offset: usize, array_dim: usize) -> StartupProperty {
    StartupProperty {
        name, kind, category: "Mover", property_flags: 0, object_flags: 4,
        inner_name: None, offset, array_dim,
    }
}

/// The single declarative property source consumed by canonical metadata and
/// the legacy StaticConstructor adapter.
pub(crate) const MOVER_PROPERTIES: &[StartupProperty] = &[
    mover_property("MoverEncroachType", StartupPropertyKind::ByteEnum { name: "EMoverEncroachType", values: ENCROACH_VALUES }, offset_of!(AMover, mover_encroach_type), 1),
    mover_property("MoverGlideType", StartupPropertyKind::ByteEnum { name: "EMoverGlideType", values: GLIDE_VALUES }, offset_of!(AMover, mover_glide_type), 1),
    mover_property("BumpType", StartupPropertyKind::ByteEnum { name: "EBumpType", values: BUMP_VALUES }, offset_of!(AMover, bump_type), 1),
    mover_property("KeyNum", StartupPropertyKind::Byte, offset_of!(AMover, key_num), 1),
    mover_property("PrevKeyNum", StartupPropertyKind::Byte, offset_of!(AMover, prev_key_num), 1),
    mover_property("NumKeys", StartupPropertyKind::Byte, offset_of!(AMover, num_keys), 1),
    mover_property("WorldRaytraceKey", StartupPropertyKind::Byte, offset_of!(AMover, world_raytrace_key), 1),
    mover_property("BrushRaytraceKey", StartupPropertyKind::Byte, offset_of!(AMover, brush_raytrace_key), 1),
    mover_property("MoveTimes", StartupPropertyKind::Float, offset_of!(AMover, move_times), 8),
    mover_property("MoveTime", StartupPropertyKind::Float, offset_of!(AMover, move_time), 1),
    mover_property("StayOpenTime", StartupPropertyKind::Float, offset_of!(AMover, stay_open_time), 1),
    mover_property("OtherTime", StartupPropertyKind::Float, offset_of!(AMover, other_time), 1),
    mover_property("EncroachDamage", StartupPropertyKind::Int, offset_of!(AMover, encroach_damage), 1),
    mover_property("bKeepRotationDirection", StartupPropertyKind::Bool { bit_mask: 1 }, offset_of!(AMover, player_bump_event) - 4, 1),
    mover_property("bTriggerOnceOnly", StartupPropertyKind::Bool { bit_mask: 2 }, offset_of!(AMover, player_bump_event) - 4, 1),
    mover_property("bSlave", StartupPropertyKind::Bool { bit_mask: 4 }, offset_of!(AMover, player_bump_event) - 4, 1),
    mover_property("bUseTriggered", StartupPropertyKind::Bool { bit_mask: 8 }, offset_of!(AMover, player_bump_event) - 4, 1),
    mover_property("bDamageTriggered", StartupPropertyKind::Bool { bit_mask: 16 }, offset_of!(AMover, player_bump_event) - 4, 1),
    mover_property("bDynamicLightMover", StartupPropertyKind::Bool { bit_mask: 32 }, offset_of!(AMover, player_bump_event) - 4, 1),
    mover_property("PlayerBumpEvent", StartupPropertyKind::Name, offset_of!(AMover, player_bump_event), 1),
    mover_property("BumpEvent", StartupPropertyKind::Name, offset_of!(AMover, bump_event), 1),
    mover_property("SavedTrigger", StartupPropertyKind::Object { class: "Engine.Actor" }, offset_of!(AMover, saved_trigger), 1),
    mover_property("DamageThreshold", StartupPropertyKind::Float, offset_of!(AMover, damage_threshold), 1),
    mover_property("numTriggerEvents", StartupPropertyKind::Int, offset_of!(AMover, num_trigger_events), 1),
    mover_property("Leader", StartupPropertyKind::Object { class: "Engine.Mover" }, offset_of!(AMover, leader), 1),
    mover_property("Follower", StartupPropertyKind::Object { class: "Engine.Mover" }, offset_of!(AMover, follower), 1),
    mover_property("ReturnGroup", StartupPropertyKind::Name, offset_of!(AMover, return_group), 1),
    mover_property("DelayTime", StartupPropertyKind::Float, offset_of!(AMover, delay_time), 1),
    mover_property("AttachTag", StartupPropertyKind::Name, offset_of!(AMover, attach_tag), 1),
    mover_property("OpeningSound", StartupPropertyKind::Object { class: "Engine.Sound" }, offset_of!(AMover, opening_sound), 1),
    mover_property("OpenedSound", StartupPropertyKind::Object { class: "Engine.Sound" }, offset_of!(AMover, opened_sound), 1),
    mover_property("ClosingSound", StartupPropertyKind::Object { class: "Engine.Sound" }, offset_of!(AMover, closing_sound), 1),
    mover_property("ClosedSound", StartupPropertyKind::Object { class: "Engine.Sound" }, offset_of!(AMover, closed_sound), 1),
    mover_property("MoveAmbientSound", StartupPropertyKind::Object { class: "Engine.Sound" }, offset_of!(AMover, move_ambient_sound), 1),
    mover_property("FailSound", StartupPropertyKind::Object { class: "Engine.Sound" }, offset_of!(AMover, fail_sound), 1),
    mover_property("MoverRadius", StartupPropertyKind::Float, offset_of!(AMover, mover_radius), 1),
    mover_property("MoverVolume", StartupPropertyKind::Byte, offset_of!(AMover, mover_volume), 1),
    mover_property("MoverPitch", StartupPropertyKind::Byte, offset_of!(AMover, mover_pitch), 1),
    mover_property("KeyPos", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, key_pos), 8),
    mover_property("KeyRot", StartupPropertyKind::Struct { name: "Rotator", fields: ROTATOR_FIELDS }, offset_of!(AMover, key_rot), 8),
    mover_property("BasePos", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, base_pos), 1),
    mover_property("OldPos", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, old_pos), 1),
    mover_property("OldPrePivot", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, old_pre_pivot), 1),
    mover_property("SavedPos", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, saved_pos), 1),
    mover_property("BaseRot", StartupPropertyKind::Struct { name: "Rotator", fields: ROTATOR_FIELDS }, offset_of!(AMover, base_rot), 1),
    mover_property("OldRot", StartupPropertyKind::Struct { name: "Rotator", fields: ROTATOR_FIELDS }, offset_of!(AMover, old_rot), 1),
    mover_property("SavedRot", StartupPropertyKind::Struct { name: "Rotator", fields: ROTATOR_FIELDS }, offset_of!(AMover, saved_rot), 1),
    mover_property("PhysAlpha", StartupPropertyKind::Float, offset_of!(AMover, phys_alpha), 1),
    mover_property("PhysRate", StartupPropertyKind::Float, offset_of!(AMover, phys_rate), 1),
    mover_property("MyMarker", StartupPropertyKind::Object { class: "Engine.NavigationPoint" }, offset_of!(AMover, my_marker), 1),
    mover_property("TriggerActor", StartupPropertyKind::Object { class: "Engine.Actor" }, offset_of!(AMover, trigger_actor), 1),
    mover_property("TriggerActor2", StartupPropertyKind::Object { class: "Engine.Actor" }, offset_of!(AMover, trigger_actor2), 1),
    mover_property("WaitingPawn", StartupPropertyKind::Object { class: "Engine.Pawn" }, offset_of!(AMover, waiting_pawn), 1),
    mover_property("bOpening", StartupPropertyKind::Bool { bit_mask: 1 }, offset_of!(AMover, recommended_trigger) - 4, 1),
    mover_property("bDelaying", StartupPropertyKind::Bool { bit_mask: 2 }, offset_of!(AMover, recommended_trigger) - 4, 1),
    mover_property("bClientPause", StartupPropertyKind::Bool { bit_mask: 4 }, offset_of!(AMover, recommended_trigger) - 4, 1),
    mover_property("bPlayerOnly", StartupPropertyKind::Bool { bit_mask: 8 }, offset_of!(AMover, recommended_trigger) - 4, 1),
    mover_property("RecommendedTrigger", StartupPropertyKind::Object { class: "Engine.Trigger" }, offset_of!(AMover, recommended_trigger), 1),
    mover_property("SimOldPos", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, sim_old_pos), 1),
    mover_property("SimOldRotPitch", StartupPropertyKind::Int, offset_of!(AMover, sim_old_rot_pitch), 1),
    mover_property("SimOldRotYaw", StartupPropertyKind::Int, offset_of!(AMover, sim_old_rot_yaw), 1),
    mover_property("SimOldRotRoll", StartupPropertyKind::Int, offset_of!(AMover, sim_old_rot_roll), 1),
    mover_property("SimInterpolate", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, sim_interpolate), 1),
    mover_property("RealPosition", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, real_position), 1),
    mover_property("RealRotation", StartupPropertyKind::Struct { name: "Rotator", fields: ROTATOR_FIELDS }, offset_of!(AMover, real_rotation), 1),
    mover_property("ClientUpdate", StartupPropertyKind::Int, offset_of!(AMover, client_update), 1),
    mover_property("bCorralMover", StartupPropertyKind::Bool { bit_mask: 1 }, offset_of!(AMover, mover_spring_time) - 4, 1),
    mover_property("bCorraledFlag", StartupPropertyKind::Bool { bit_mask: 2 }, offset_of!(AMover, mover_spring_time) - 4, 1),
    mover_property("MoverSpringTime", StartupPropertyKind::Float, offset_of!(AMover, mover_spring_time), 1),
    mover_property("MoverMaxAmplitude", StartupPropertyKind::Float, offset_of!(AMover, mover_max_amplitude), 1),
    mover_property("MoverFluctuations", StartupPropertyKind::Byte, offset_of!(AMover, mover_fluctuations), 1),
    mover_property("HitPosition", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, hit_position), 1),
    mover_property("HitNormal", StartupPropertyKind::Struct { name: "Vector", fields: VECTOR_FIELDS }, offset_of!(AMover, hit_normal), 1),
];


/// Context-dependent engine effects used by shared Mover policy.
pub(crate) trait MoverOps {
    fn hash_remove(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()>;
    fn hash_add(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()>;
    fn tracker_update(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()>;
    fn tracker_flush(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()>;
    fn renumber_brush_polys(&mut self, brush: *mut c_void) -> hp2_core::core_access::RuntimeResult<()>;
}

/// Runs post-load reset policy and its injected native polygon repair.
///
/// # Safety
/// `mover` must be a live canonical `AMover`; the ops implementation must
/// validate any external objects before dereferencing them.
pub(crate) unsafe fn post_load_with_ops(
    mover: *mut AMover, ops: &mut impl MoverOps,
) -> hp2_core::core_access::RuntimeResult<()> {
    if mover.is_null() { return Err(hp2_core::core_access::RuntimeError::new("Null Mover in PostLoad")); }
    unsafe { reset_after_post_load(&mut *mover); }
    let brush = unsafe { (*mover).base.brush };
    ops.renumber_brush_polys(brush)
}

/// Runs world-raytrace policy without holding a Rust object borrow over callbacks.
///
/// # Safety
/// `mover` is a live canonical record for the duration of synchronous callbacks.
pub(crate) unsafe fn set_world_raytrace_key_with_ops(
    mover: *mut AMover, ops: &mut impl MoverOps,
) -> hp2_core::core_access::RuntimeResult<()> {
    if mover.is_null() { return Err(hp2_core::core_access::RuntimeError::new("Null Mover in SetWorldRaytraceKey")); }
    if !clamp_world_raytrace_key(unsafe { &mut *mover }) {
        return ops.tracker_flush(mover);
    }
    let collides = unsafe { (*mover).base.b_collide_actors() };
    if collides { ops.hash_remove(mover)?; }
    apply_world_raytrace_pose(unsafe { &mut *mover });
    if collides { ops.hash_add(mover)?; }
    ops.tracker_update(mover)
}

/// Runs brush-raytrace policy without holding a Rust object borrow over callbacks.
///
/// # Safety
/// `mover` is a live canonical record for the duration of synchronous callbacks.
pub(crate) unsafe fn set_brush_raytrace_key_with_ops(
    mover: *mut AMover, ops: &mut impl MoverOps,
) -> hp2_core::core_access::RuntimeResult<()> {
    if mover.is_null() { return Err(hp2_core::core_access::RuntimeError::new("Null Mover in SetBrushRaytraceKey")); }
    let collides = unsafe { (*mover).base.b_collide_actors() };
    if collides { ops.hash_remove(mover)?; }
    apply_brush_raytrace_pose(unsafe { &mut *mover });
    if collides { ops.hash_add(mover)?; }
    ops.tracker_update(mover)
}

/// Runs post-raytrace restoration using injected hash and tracker operations.
///
/// # Safety
/// `mover` is a live canonical record for the duration of synchronous callbacks.
pub(crate) unsafe fn post_raytrace_with_ops(
    mover: *mut AMover, ops: &mut impl MoverOps,
) -> hp2_core::core_access::RuntimeResult<()> {
    if mover.is_null() { return Err(hp2_core::core_access::RuntimeError::new("Null Mover in PostRaytrace")); }
    let collides = unsafe { (*mover).base.b_collide_actors() };
    if collides { ops.hash_remove(mover)?; }
    restore_raytrace_pose(unsafe { &mut *mover });
    if collides { ops.hash_add(mover)?; }
    clear_raytrace_transform(unsafe { &mut *mover });
    ops.tracker_update(mover)
}
const CAT_MOVER: &CStr = c"Mover";
const CAT_EMPTY: &CStr = c"";

/// Matches the `Hp2MoverPropKind` enum in `Hp2MoverServices.h`.
#[repr(i32)]
#[derive(Clone, Copy)]
pub enum PropKind {
    Byte = 0,
    Float = 1,
    Int = 2,
    Name = 3,
    Object = 4,
    Struct = 5,
    Bool = 6,
}

/// Matches the `Hp2MoverClassRef` enum in `Hp2MoverServices.h`.
#[repr(i32)]
#[derive(Clone, Copy)]
pub enum ClassRef {
    Mover = 0,
    Actor = 1,
    Sound = 2,
    NavigationPoint = 3,
    Pawn = 4,
    Trigger = 5,
}

/// C++ object-model thunks. Field order is ABI: mirror it in
/// `Engine/Inc/Hp2MoverServices.h`. Every thunk is a plain C++ call on
/// UObject/UProperty/FCollisionHashBase/FMovingBrushTrackerBase.
#[repr(C)]
pub struct Hp2MoverServices {
    /// `FindObject<UStruct>(UObject::StaticClass(), name)`.
    pub find_struct: unsafe extern "C" fn(name: *const c_char) -> *mut c_void,
    /// `new(UObject::StaticClass(),name,RF_Public)UStruct(NULL)` + `SetPropertiesSize(size)`.
    pub new_struct: unsafe extern "C" fn(name: *const c_char, size: i32) -> *mut c_void,
    /// `FArchive ArDummy; Struct->Link(ArDummy,0)`.
    pub link_struct: unsafe extern "C" fn(st: *mut c_void),
    /// `new(GetClass(),name)UEnum(NULL)` (GetClass() is AMover's class).
    pub new_enum: unsafe extern "C" fn(name: *const c_char) -> *mut c_void,
    /// `new(E->Names)FName(name)`.
    pub enum_add_name: unsafe extern "C" fn(e: *mut c_void, name: *const c_char),
    /// Class pointer for an `Hp2MoverClassRef` id.
    pub class_ref: unsafe extern "C" fn(id: i32) -> *mut c_void,
    /// `new(owner,name,RF_Public)<kind>Property(EC_CppProperty,offset,category,0[,ref])`,
    /// then `ArrayDim = array_dim`. Bool kind sets `BitMask = bit_mask`.
    /// `ref_obj` is the UEnum (Byte), UClass (Object) or UStruct (Struct).
    pub add_property: unsafe extern "C" fn(
        owner: *mut c_void,
        kind: i32,
        name: *const c_char,
        category: *const c_char,
        offset: i32,
        array_dim: i32,
        bit_mask: u32,
        ref_obj: *mut c_void,
    ),
    /// `((FCollisionHashBase*)hash)->AddActor(actor)`.
    pub hash_add: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void),
    /// `((FCollisionHashBase*)hash)->RemoveActor(actor)` (bThorough = false).
    pub hash_remove: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void),
    /// `((FMovingBrushTrackerBase*)tracker)->Update(actor)`.
    pub brush_tracker_update: unsafe extern "C" fn(tracker: *mut c_void, actor: *mut c_void),
    /// `((FMovingBrushTrackerBase*)tracker)->Flush(actor)`.
    pub brush_tracker_flush: unsafe extern "C" fn(tracker: *mut c_void, actor: *mut c_void),
    /// `if (brush && brush->Polys) for i: Polys->Element(i).iLink = i`.
    pub renumber_brush_polys: unsafe extern "C" fn(brush: *mut c_void),
}

unsafe fn sub_struct(
    s: &Hp2MoverServices,
    name: &CStr,
    size: usize,
    fields: &[(&CStr, i32)],
    kind: PropKind,
) -> *mut c_void {
    unsafe {
        let found = (s.find_struct)(name.as_ptr());
        if !found.is_null() {
            return found;
        }
        let st = (s.new_struct)(name.as_ptr(), size as i32);
        for (field, offset) in fields {
            (s.add_property)(
                st,
                kind as i32,
                field.as_ptr(),
                CAT_EMPTY.as_ptr(),
                *offset,
                1,
                0,
                ptr::null_mut(),
            );
        }
        (s.link_struct)(st);
        st
    }
}

unsafe fn new_enum(s: &Hp2MoverServices, name: &CStr, names: &[&CStr]) -> *mut c_void {
    unsafe {
        let e = (s.new_enum)(name.as_ptr());
        for n in names {
            (s.enum_add_name)(e, n.as_ptr());
        }
        e
    }
}

/// `REGISTER_MOVER_PROPERTY` / `_ARRAY_` / `_OBJECT` / `_STRUCT` / `_STRUCT_ARRAY`.
unsafe fn prop(
    s: &Hp2MoverServices,
    owner: *mut c_void,
    kind: PropKind,
    name: &CStr,
    offset: i32,
    array_dim: i32,
    ref_obj: *mut c_void,
) {
    unsafe {
        (s.add_property)(
            owner,
            kind as i32,
            name.as_ptr(),
            CAT_MOVER.as_ptr(),
            offset,
            array_dim,
            0,
            ref_obj,
        );
    }
}

/// `REGISTER_MOVER_BOOL(Member, StorageOffset, Mask)`.
unsafe fn bool_prop(
    s: &Hp2MoverServices,
    owner: *mut c_void,
    name: &CStr,
    storage: i32,
    mask: u32,
) {
    unsafe {
        (s.add_property)(
            owner,
            PropKind::Bool as i32,
            name.as_ptr(),
            CAT_MOVER.as_ptr(),
            storage,
            1,
            mask,
            ptr::null_mut(),
        );
    }
}

/// `AMover::StaticConstructor()`: builds the Vector/Rotator structs, the three
/// enums and every mover property, in the original order.
///
/// # Safety
/// Must be called from `AMover::StaticConstructor` with a valid services table.
pub unsafe fn static_constructor(s: &Hp2MoverServices) {
    unsafe {
        const VECTOR_FIELDS: &[(&CStr, i32)] = &[
            (c"X", offset_of!(FVector, x) as i32),
            (c"Y", offset_of!(FVector, y) as i32),
            (c"Z", offset_of!(FVector, z) as i32),
        ];
        const ROTATOR_FIELDS: &[(&CStr, i32)] = &[
            (c"Pitch", offset_of!(FRotator, pitch) as i32),
            (c"Yaw", offset_of!(FRotator, yaw) as i32),
            (c"Roll", offset_of!(FRotator, roll) as i32),
        ];
        let vector = sub_struct(s, c"Vector", size_of::<FVector>(), VECTOR_FIELDS, PropKind::Float);
        let rotator = sub_struct(s, c"Rotator", size_of::<FRotator>(), ROTATOR_FIELDS, PropKind::Int);
        let encroach = new_enum(s, c"EMoverEncroachType", &[
            c"ME_StopWhenEncroach", c"ME_ReturnWhenEncroach", c"ME_CrushWhenEncroach",
            c"ME_IgnoreWhenEncroach", c"ME_ReallyStopWhenEncroach", c"ME_ReallyReturnWhenEncroach",
            c"ME_ReallyCrushWhenEncroach", c"ME_ReallyIgnoreWhenEncroach",
        ]);
        let glide = new_enum(s, c"EMoverGlideType", &[c"MV_MoveByTime", c"MV_GlideByTime", c"MV_SpringByTime"]);
        let bump = new_enum(s, c"EBumpType", &[c"BT_PlayerBump", c"BT_PawnBump", c"BT_AnyBump"]);
        let class = (s.class_ref)(ClassRef::Mover as i32);
        let actor = (s.class_ref)(ClassRef::Actor as i32);
        let sound = (s.class_ref)(ClassRef::Sound as i32);
        let nav = (s.class_ref)(ClassRef::NavigationPoint as i32);
        let pawn = (s.class_ref)(ClassRef::Pawn as i32);
        let trigger = (s.class_ref)(ClassRef::Trigger as i32);
        for descriptor in MOVER_PROPERTIES {
            let mut name_storage = [0u8; 64];
            let name = stack_cstr(descriptor.name, &mut name_storage);
            let (kind, reference) = match descriptor.kind {
                StartupPropertyKind::Byte => (PropKind::Byte, ptr::null_mut()),
                StartupPropertyKind::ByteEnum { name: "EMoverEncroachType", .. } => (PropKind::Byte, encroach),
                StartupPropertyKind::ByteEnum { name: "EMoverGlideType", .. } => (PropKind::Byte, glide),
                StartupPropertyKind::ByteEnum { name: "EBumpType", .. } => (PropKind::Byte, bump),
                StartupPropertyKind::Int => (PropKind::Int, ptr::null_mut()),
                StartupPropertyKind::Float => (PropKind::Float, ptr::null_mut()),
                StartupPropertyKind::Bool { .. } => (PropKind::Bool, ptr::null_mut()),
                StartupPropertyKind::Name => (PropKind::Name, ptr::null_mut()),
                StartupPropertyKind::Object { class: "Engine.Actor" } => (PropKind::Object, actor),
                StartupPropertyKind::Object { class: "Engine.Mover" } => (PropKind::Object, class),
                StartupPropertyKind::Object { class: "Engine.Sound" } => (PropKind::Object, sound),
                StartupPropertyKind::Object { class: "Engine.NavigationPoint" } => (PropKind::Object, nav),
                StartupPropertyKind::Object { class: "Engine.Pawn" } => (PropKind::Object, pawn),
                StartupPropertyKind::Object { class: "Engine.Trigger" } => (PropKind::Object, trigger),
                StartupPropertyKind::Struct { name: "Vector", .. } => (PropKind::Struct, vector),
                StartupPropertyKind::Struct { name: "Rotator", .. } => (PropKind::Struct, rotator),
                _ => panic!("unsupported Mover startup property descriptor"),
            };
            if let StartupPropertyKind::Bool { bit_mask } = descriptor.kind {
                bool_prop(s, class, name, descriptor.offset as i32, bit_mask);
            } else {
                prop(s, class, kind, name, descriptor.offset as i32, descriptor.array_dim as i32, reference);
            }
        }
    }
}

fn stack_cstr<'a>(text: &str, storage: &'a mut [u8; 64]) -> &'a CStr {
    assert!(text.len() < storage.len(), "Mover property name exceeds stack C string buffer");
    storage[..text.len()].copy_from_slice(text.as_bytes());
    storage[text.len()] = 0;
    unsafe { CStr::from_bytes_with_nul_unchecked(&storage[..=text.len()]) }
}

/// `Clamp((INT)Key, (INT)0, (INT)ARRAY_COUNT(KeyPos)-1)` (UnCore `Clamp`: `X<Min?Min:X<Max?X:Max`).
fn clamp_key(key: u8) -> u8 {
    let x = key as i32;
    let max = KEY_COUNT - 1;
    (if x < 0 {
        0
    } else if x < max {
        x
    } else {
        max
    }) as u8
}

fn v_add(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn v_sub(a: FVector, b: FVector) -> FVector {
    FVector {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn r_add(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_add(b.pitch),
        yaw: a.yaw.wrapping_add(b.yaw),
        roll: a.roll.wrapping_add(b.roll),
    }
}

fn r_sub(a: FRotator, b: FRotator) -> FRotator {
    FRotator {
        pitch: a.pitch.wrapping_sub(b.pitch),
        yaw: a.yaw.wrapping_sub(b.yaw),
        roll: a.roll.wrapping_sub(b.roll),
    }
}

struct LegacyMoverOps<'a>(&'a Hp2MoverServices);

impl MoverOps for LegacyMoverOps<'_> {
    fn hash_remove(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()> {
        unsafe {
            let level = (*mover).base.x_level.cast::<ULevel>();
            if level.is_null() { return Err(hp2_core::core_access::RuntimeError::new("AMover::GetLevel() is null")); }
            let hash = (*level).hash;
            if !hash.is_null() { (self.0.hash_remove)(hash, mover.cast()); }
        }
        Ok(())
    }

    fn hash_add(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()> {
        unsafe {
            let level = (*mover).base.x_level.cast::<ULevel>();
            if level.is_null() { return Err(hp2_core::core_access::RuntimeError::new("AMover::GetLevel() is null")); }
            let hash = (*level).hash;
            if !hash.is_null() { (self.0.hash_add)(hash, mover.cast()); }
        }
        Ok(())
    }

    fn tracker_update(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()> {
        unsafe {
            let level = (*mover).base.x_level.cast::<ULevel>();
            if level.is_null() { return Err(hp2_core::core_access::RuntimeError::new("AMover::GetLevel() is null")); }
            let tracker = (*level).brush_tracker;
            if !tracker.is_null() { (self.0.brush_tracker_update)(tracker, mover.cast()); }
        }
        Ok(())
    }

    fn tracker_flush(&mut self, mover: *mut AMover) -> hp2_core::core_access::RuntimeResult<()> {
        unsafe {
            let level = (*mover).base.x_level.cast::<ULevel>();
            if level.is_null() { return Err(hp2_core::core_access::RuntimeError::new("AMover::GetLevel() is null")); }
            let tracker = (*level).brush_tracker;
            if !tracker.is_null() { (self.0.brush_tracker_flush)(tracker, mover.cast()); }
        }
        Ok(())
    }

    fn renumber_brush_polys(&mut self, brush: *mut c_void) -> hp2_core::core_access::RuntimeResult<()> {
        unsafe { (self.0.renumber_brush_polys)(brush); }
        Ok(())
    }
}
/// `AMover::Spawned()` body (after `ABrush::Spawned()`).
pub fn spawned(mv: &mut AMover) {
    mv.base_pos = mv.base.location;
    mv.base_rot = mv.base.rotation;
}

/// `AMover::PostLoad()` body (after `AActor::PostLoad()`).
///
/// # Safety
/// `mv` must be a live `AMover` whose `XLevel` (if any) is valid.
pub(crate) fn reset_after_post_load(mv: &mut AMover) {
    mv.saved_pos = FVector { x: -12345.0, y: -12345.0, z: -12345.0 };
    mv.saved_rot = FRotator { pitch: 123, yaw: 456, roll: 789 };
}

pub unsafe fn post_load(mv: &mut AMover, s: &Hp2MoverServices) {
    let mut ops = LegacyMoverOps(s);
    unsafe { post_load_with_ops(ptr::from_mut(mv), &mut ops) }
        .expect("legacy mover post-load has a valid brush object");
}

/// `AMover::PostEditMove()` body (after `ABrush::PostEditMove()`).
pub fn post_edit_move(mv: &mut AMover) {
    if mv.key_num == 0 {
        // Changing location.
        mv.base_pos = v_sub(mv.base.location, mv.old_pos);
        mv.base_rot = r_sub(mv.base.rotation, mv.old_rot);
    } else {
        let k = mv.key_num as usize;
        // Changing displacement of KeyPos[KeyNum] relative to KeyPos[0].
        mv.key_pos[k] = v_sub(mv.base.location, v_add(mv.base_pos, mv.key_pos[0]));
        mv.key_rot[k] = r_sub(mv.base.rotation, r_add(mv.base_rot, mv.key_rot[0]));
        // Update Old.
        mv.old_pos = mv.key_pos[k];
        mv.old_rot = mv.key_rot[k];
    }
    mv.base.location = v_add(mv.base_pos, mv.key_pos[mv.key_num as usize]);
}

/// `AMover::PostEditChange()` body (after `ABrush::PostEditChange()`).
pub fn post_edit_change(mv: &mut AMover) {
    // Validate KeyNum.
    mv.key_num = clamp_key(mv.key_num);

    // Update BasePos.
    mv.base_pos = v_sub(mv.base.location, mv.old_pos);
    mv.base_rot = r_sub(mv.base.rotation, mv.old_rot);

    // Update Old.
    mv.old_pos = mv.key_pos[mv.key_num as usize];
    mv.old_rot = mv.key_rot[mv.key_num as usize];

    // Update Location.
    mv.base.location = v_add(mv.base_pos, mv.old_pos);
    mv.base.rotation = r_add(mv.base_rot, mv.old_rot);

    post_edit_move(mv);
}

/// `AMover::PreRaytrace()` body (after `ABrush::PreRaytrace()`).
pub fn pre_raytrace(mv: &mut AMover) {
    // Place this brush in position to raytrace the world.
    mv.saved_pos = FVector {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    mv.saved_rot = FRotator {
        pitch: 0,
        yaw: 0,
        roll: 0,
    };
}

/// Clamps the world raytrace key; `false` means the tracker must be flushed.
pub(crate) fn clamp_world_raytrace_key(mv: &mut AMover) -> bool {
    if mv.world_raytrace_key == 255 { return false; }
    mv.world_raytrace_key = clamp_key(mv.world_raytrace_key);
    true
}

/// Applies the already-clamped world raytrace pose.
pub(crate) fn apply_world_raytrace_pose(mv: &mut AMover) {
    let k = mv.world_raytrace_key as usize;
    mv.base.location = v_add(mv.base_pos, mv.key_pos[k]);
    mv.base.rotation = r_add(mv.base_rot, mv.key_rot[k]);
}

/// Applies the pure keyed brush-raytrace pose.
pub(crate) fn apply_brush_raytrace_pose(mv: &mut AMover) {
    mv.brush_raytrace_key = clamp_key(mv.brush_raytrace_key);
    let k = mv.brush_raytrace_key as usize;
    mv.base.location = v_add(mv.base_pos, mv.key_pos[k]);
    mv.base.rotation = r_add(mv.base_rot, mv.key_rot[k]);
}

/// Restores the mover's current key pose before collision re-insertion.
pub(crate) fn restore_raytrace_pose(mv: &mut AMover) {
    let k = mv.key_num as usize;
    mv.base.location = v_add(mv.base_pos, mv.key_pos[k]);
    mv.base.rotation = r_add(mv.base_rot, mv.key_rot[k]);
}

/// Clears the temporary raytrace transform after hash re-insertion.
pub(crate) fn clear_raytrace_transform(mv: &mut AMover) {
    mv.saved_pos = FVector { x: 0.0, y: 0.0, z: 0.0 };
    mv.saved_rot = FRotator { pitch: 0, yaw: 0, roll: 0 };
}

/// `AMover::SetWorldRaytraceKey()` body.
///
/// # Safety
/// `mv`'s `XLevel` must be valid and its hash/tracker pointers must be live when non-null.
pub unsafe fn set_world_raytrace_key(mv: &mut AMover, s: &Hp2MoverServices) {
    let mut ops = LegacyMoverOps(s);
    unsafe { set_world_raytrace_key_with_ops(ptr::from_mut(mv), &mut ops) }
        .expect("legacy mover has a valid level for world raytracing");
}

/// `AMover::SetBrushRaytraceKey()` body.
///
/// # Safety
/// Same contract as [`set_world_raytrace_key`].
pub unsafe fn set_brush_raytrace_key(mv: &mut AMover, s: &Hp2MoverServices) {
    let mut ops = LegacyMoverOps(s);
    unsafe { set_brush_raytrace_key_with_ops(ptr::from_mut(mv), &mut ops) }
        .expect("legacy mover has a valid level for brush raytracing");
}

/// `AMover::PostRaytrace()` body (after `ABrush::PostRaytrace()`).
///
/// # Safety
/// Same contract as [`set_world_raytrace_key`].
pub unsafe fn post_raytrace(mv: &mut AMover, s: &Hp2MoverServices) {
    let mut ops = LegacyMoverOps(s);
    unsafe { post_raytrace_with_ops(ptr::from_mut(mv), &mut ops) }
        .expect("legacy mover has a valid level after raytracing");
}
