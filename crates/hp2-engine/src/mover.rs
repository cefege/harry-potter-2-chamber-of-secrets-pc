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
        let vector = sub_struct(
            s,
            c"Vector",
            size_of::<FVector>(),
            &[
                (c"X", offset_of!(FVector, x) as i32),
                (c"Y", offset_of!(FVector, y) as i32),
                (c"Z", offset_of!(FVector, z) as i32),
            ],
            PropKind::Float,
        );
        let rotator = sub_struct(
            s,
            c"Rotator",
            size_of::<FRotator>(),
            &[
                (c"Pitch", offset_of!(FRotator, pitch) as i32),
                (c"Yaw", offset_of!(FRotator, yaw) as i32),
                (c"Roll", offset_of!(FRotator, roll) as i32),
            ],
            PropKind::Int,
        );

        let encroach = new_enum(
            s,
            c"EMoverEncroachType",
            &[
                c"ME_StopWhenEncroach",
                c"ME_ReturnWhenEncroach",
                c"ME_CrushWhenEncroach",
                c"ME_IgnoreWhenEncroach",
                c"ME_ReallyStopWhenEncroach",
                c"ME_ReallyReturnWhenEncroach",
                c"ME_ReallyCrushWhenEncroach",
                c"ME_ReallyIgnoreWhenEncroach",
            ],
        );
        let glide = new_enum(
            s,
            c"EMoverGlideType",
            &[c"MV_MoveByTime", c"MV_GlideByTime", c"MV_SpringByTime"],
        );
        let bump = new_enum(
            s,
            c"EBumpType",
            &[c"BT_PlayerBump", c"BT_PawnBump", c"BT_AnyBump"],
        );

        let class = (s.class_ref)(ClassRef::Mover as i32);
        let actor = (s.class_ref)(ClassRef::Actor as i32);
        let sound = (s.class_ref)(ClassRef::Sound as i32);
        let nav = (s.class_ref)(ClassRef::NavigationPoint as i32);
        let pawn = (s.class_ref)(ClassRef::Pawn as i32);
        let trigger = (s.class_ref)(ClassRef::Trigger as i32);

        // Enum-typed bytes. The explicit names are the C++ member names (no `#M` form).
        (s.add_property)(
            class,
            PropKind::Byte as i32,
            c"MoverEncroachType".as_ptr(),
            CAT_MOVER.as_ptr(),
            offset_of!(AMover, mover_encroach_type) as i32,
            1,
            0,
            encroach,
        );
        (s.add_property)(
            class,
            PropKind::Byte as i32,
            c"MoverGlideType".as_ptr(),
            CAT_MOVER.as_ptr(),
            offset_of!(AMover, mover_glide_type) as i32,
            1,
            0,
            glide,
        );
        (s.add_property)(
            class,
            PropKind::Byte as i32,
            c"BumpType".as_ptr(),
            CAT_MOVER.as_ptr(),
            offset_of!(AMover, bump_type) as i32,
            1,
            0,
            bump,
        );

        let null = ptr::null_mut();
        prop(
            s,
            class,
            PropKind::Byte,
            c"KeyNum",
            offset_of!(AMover, key_num) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Byte,
            c"PrevKeyNum",
            offset_of!(AMover, prev_key_num) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Byte,
            c"NumKeys",
            offset_of!(AMover, num_keys) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Byte,
            c"WorldRaytraceKey",
            offset_of!(AMover, world_raytrace_key) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Byte,
            c"BrushRaytraceKey",
            offset_of!(AMover, brush_raytrace_key) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"MoveTimes",
            offset_of!(AMover, move_times) as i32,
            8,
            null,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"MoveTime",
            offset_of!(AMover, move_time) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"StayOpenTime",
            offset_of!(AMover, stay_open_time) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"OtherTime",
            offset_of!(AMover, other_time) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Int,
            c"EncroachDamage",
            offset_of!(AMover, encroach_damage) as i32,
            1,
            null,
        );

        // Bool storage is the dword in front of PlayerBumpEvent (sizeof(BITFIELD) below it).
        let state_bool = offset_of!(AMover, player_bump_event) as i32 - BITFIELD_SIZE;
        bool_prop(s, class, c"bKeepRotationDirection", state_bool, 1);
        bool_prop(s, class, c"bTriggerOnceOnly", state_bool, 2);
        bool_prop(s, class, c"bSlave", state_bool, 4);
        bool_prop(s, class, c"bUseTriggered", state_bool, 8);
        bool_prop(s, class, c"bDamageTriggered", state_bool, 16);
        bool_prop(s, class, c"bDynamicLightMover", state_bool, 32);

        prop(
            s,
            class,
            PropKind::Name,
            c"PlayerBumpEvent",
            offset_of!(AMover, player_bump_event) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Name,
            c"BumpEvent",
            offset_of!(AMover, bump_event) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"SavedTrigger",
            offset_of!(AMover, saved_trigger) as i32,
            1,
            actor,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"DamageThreshold",
            offset_of!(AMover, damage_threshold) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Int,
            c"numTriggerEvents",
            offset_of!(AMover, num_trigger_events) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"Leader",
            offset_of!(AMover, leader) as i32,
            1,
            class,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"Follower",
            offset_of!(AMover, follower) as i32,
            1,
            class,
        );
        prop(
            s,
            class,
            PropKind::Name,
            c"ReturnGroup",
            offset_of!(AMover, return_group) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"DelayTime",
            offset_of!(AMover, delay_time) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Name,
            c"AttachTag",
            offset_of!(AMover, attach_tag) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"OpeningSound",
            offset_of!(AMover, opening_sound) as i32,
            1,
            sound,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"OpenedSound",
            offset_of!(AMover, opened_sound) as i32,
            1,
            sound,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"ClosingSound",
            offset_of!(AMover, closing_sound) as i32,
            1,
            sound,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"ClosedSound",
            offset_of!(AMover, closed_sound) as i32,
            1,
            sound,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"MoveAmbientSound",
            offset_of!(AMover, move_ambient_sound) as i32,
            1,
            sound,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"FailSound",
            offset_of!(AMover, fail_sound) as i32,
            1,
            sound,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"MoverRadius",
            offset_of!(AMover, mover_radius) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Byte,
            c"MoverVolume",
            offset_of!(AMover, mover_volume) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Byte,
            c"MoverPitch",
            offset_of!(AMover, mover_pitch) as i32,
            1,
            null,
        );

        prop(
            s,
            class,
            PropKind::Struct,
            c"KeyPos",
            offset_of!(AMover, key_pos) as i32,
            KEY_COUNT,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"KeyRot",
            offset_of!(AMover, key_rot) as i32,
            KEY_COUNT,
            rotator,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"BasePos",
            offset_of!(AMover, base_pos) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"OldPos",
            offset_of!(AMover, old_pos) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"OldPrePivot",
            offset_of!(AMover, old_pre_pivot) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"SavedPos",
            offset_of!(AMover, saved_pos) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"BaseRot",
            offset_of!(AMover, base_rot) as i32,
            1,
            rotator,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"OldRot",
            offset_of!(AMover, old_rot) as i32,
            1,
            rotator,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"SavedRot",
            offset_of!(AMover, saved_rot) as i32,
            1,
            rotator,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"PhysAlpha",
            offset_of!(AMover, phys_alpha) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"PhysRate",
            offset_of!(AMover, phys_rate) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"MyMarker",
            offset_of!(AMover, my_marker) as i32,
            1,
            nav,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"TriggerActor",
            offset_of!(AMover, trigger_actor) as i32,
            1,
            actor,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"TriggerActor2",
            offset_of!(AMover, trigger_actor2) as i32,
            1,
            actor,
        );
        prop(
            s,
            class,
            PropKind::Object,
            c"WaitingPawn",
            offset_of!(AMover, waiting_pawn) as i32,
            1,
            pawn,
        );

        // Motion bools sit in front of RecommendedTrigger.
        let motion_bool = offset_of!(AMover, recommended_trigger) as i32 - BITFIELD_SIZE;
        bool_prop(s, class, c"bOpening", motion_bool, 1);
        bool_prop(s, class, c"bDelaying", motion_bool, 2);
        bool_prop(s, class, c"bClientPause", motion_bool, 4);
        bool_prop(s, class, c"bPlayerOnly", motion_bool, 8);

        prop(
            s,
            class,
            PropKind::Object,
            c"RecommendedTrigger",
            offset_of!(AMover, recommended_trigger) as i32,
            1,
            trigger,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"SimOldPos",
            offset_of!(AMover, sim_old_pos) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Int,
            c"SimOldRotPitch",
            offset_of!(AMover, sim_old_rot_pitch) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Int,
            c"SimOldRotYaw",
            offset_of!(AMover, sim_old_rot_yaw) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Int,
            c"SimOldRotRoll",
            offset_of!(AMover, sim_old_rot_roll) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"SimInterpolate",
            offset_of!(AMover, sim_interpolate) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"RealPosition",
            offset_of!(AMover, real_position) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"RealRotation",
            offset_of!(AMover, real_rotation) as i32,
            1,
            rotator,
        );
        prop(
            s,
            class,
            PropKind::Int,
            c"ClientUpdate",
            offset_of!(AMover, client_update) as i32,
            1,
            null,
        );

        // Corral bools sit in front of MoverSpringTime.
        let corral_bool = offset_of!(AMover, mover_spring_time) as i32 - BITFIELD_SIZE;
        bool_prop(s, class, c"bCorralMover", corral_bool, 1);
        bool_prop(s, class, c"bCorraledFlag", corral_bool, 2);

        prop(
            s,
            class,
            PropKind::Float,
            c"MoverSpringTime",
            offset_of!(AMover, mover_spring_time) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Float,
            c"MoverMaxAmplitude",
            offset_of!(AMover, mover_max_amplitude) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Byte,
            c"MoverFluctuations",
            offset_of!(AMover, mover_fluctuations) as i32,
            1,
            null,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"HitPosition",
            offset_of!(AMover, hit_position) as i32,
            1,
            vector,
        );
        prop(
            s,
            class,
            PropKind::Struct,
            c"HitNormal",
            offset_of!(AMover, hit_normal) as i32,
            1,
            vector,
        );
    }
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

/// `GetLevel()` is `XLevel`.
fn level(mv: &AMover) -> *mut ULevel {
    let lvl = mv.base.x_level as *mut ULevel;
    assert!(!lvl.is_null(), "AMover::GetLevel() is null");
    lvl
}

fn as_actor(mv: &AMover) -> *mut c_void {
    ptr::from_ref(mv).cast_mut().cast()
}

/// `if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->RemoveActor( this );`
unsafe fn unhash(mv: &AMover, s: &Hp2MoverServices) {
    unsafe {
        let lvl = level(mv);
        if mv.base.b_collide_actors() && !(*lvl).hash.is_null() {
            (s.hash_remove)((*lvl).hash, as_actor(mv));
        }
    }
}

/// `if( bCollideActors && GetLevel()->Hash ) GetLevel()->Hash->AddActor( this );`
unsafe fn rehash(mv: &AMover, s: &Hp2MoverServices) {
    unsafe {
        let lvl = level(mv);
        if mv.base.b_collide_actors() && !(*lvl).hash.is_null() {
            (s.hash_add)((*lvl).hash, as_actor(mv));
        }
    }
}

/// `if( GetLevel()->BrushTracker ) GetLevel()->BrushTracker->Update( this );`
unsafe fn tracker_update(mv: &AMover, s: &Hp2MoverServices) {
    unsafe {
        let tracker = (*level(mv)).brush_tracker;
        if !tracker.is_null() {
            (s.brush_tracker_update)(tracker, as_actor(mv));
        }
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
pub unsafe fn post_load(mv: &mut AMover, s: &Hp2MoverServices) {
    unsafe {
        // For refresh.
        mv.saved_pos = FVector {
            x: -12345.0,
            y: -12345.0,
            z: -12345.0,
        };
        mv.saved_rot = FRotator {
            pitch: 123,
            yaw: 456,
            roll: 789,
        };
        // Fix brush poly iLinks which were broken.
        (s.renumber_brush_polys)(mv.base.brush);
    }
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

/// `AMover::SetWorldRaytraceKey()` body.
///
/// # Safety
/// `mv`'s `XLevel` must be valid and its hash/tracker pointers must be live when non-null.
pub unsafe fn set_world_raytrace_key(mv: &mut AMover, s: &Hp2MoverServices) {
    unsafe {
        if mv.world_raytrace_key != 255 {
            mv.world_raytrace_key = clamp_key(mv.world_raytrace_key);
            unhash(mv, s);
            let k = mv.world_raytrace_key as usize;
            mv.base.location = v_add(mv.base_pos, mv.key_pos[k]);
            mv.base.rotation = r_add(mv.base_rot, mv.key_rot[k]);
            rehash(mv, s);
            tracker_update(mv, s);
        } else {
            let tracker = (*level(mv)).brush_tracker;
            if !tracker.is_null() {
                (s.brush_tracker_flush)(tracker, as_actor(mv));
            }
        }
    }
}

/// `AMover::SetBrushRaytraceKey()` body.
///
/// # Safety
/// Same contract as [`set_world_raytrace_key`].
pub unsafe fn set_brush_raytrace_key(mv: &mut AMover, s: &Hp2MoverServices) {
    unsafe {
        mv.brush_raytrace_key = clamp_key(mv.brush_raytrace_key);
        unhash(mv, s);
        let k = mv.brush_raytrace_key as usize;
        mv.base.location = v_add(mv.base_pos, mv.key_pos[k]);
        mv.base.rotation = r_add(mv.base_rot, mv.key_rot[k]);
        rehash(mv, s);
        tracker_update(mv, s);
    }
}

/// `AMover::PostRaytrace()` body (after `ABrush::PostRaytrace()`).
///
/// # Safety
/// Same contract as [`set_world_raytrace_key`].
pub unsafe fn post_raytrace(mv: &mut AMover, s: &Hp2MoverServices) {
    unsafe {
        unhash(mv, s);
        let k = mv.key_num as usize;
        mv.base.location = v_add(mv.base_pos, mv.key_pos[k]);
        mv.base.rotation = r_add(mv.base_rot, mv.key_rot[k]);
        rehash(mv, s);
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
        tracker_update(mv, s);
    }
}
