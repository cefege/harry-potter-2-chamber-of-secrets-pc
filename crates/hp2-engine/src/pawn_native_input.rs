//! Port of the modern third-person input group of `Engine/Src/UnPawn.cpp`:
//! `APlayerPawn::BeginModernThirdPersonInput`, `FinishModernThirdPersonInput`,
//! `ApplyModernThirdPersonMovement` and their anonymous-namespace helpers
//! (`GetModernHarryProperties`, the auto-center override, the analog
//! expansion rule, and the `HGame.Harry` contract check).
//!
//! Engine reflection and object-graph reads that the C++ does through
//! `FindField`, `GetClass`, `IsA`, `GetPathName`, `GetStateFrame` and `debugf`
//! go through [`Hp2PawnNativeInputServices`], filled in by
//! `Hp2PawnNativeInputServices.cpp`. Harry bool and object reads and writes
//! use the offsets and masks those services report, exactly as the C++ did.
//! The C++ shells keep the `guard`/`unguard` wrappers.
//!
//! FP contraction: each `mul_add` marks a site the original object file fuses
//! (`fmadd`/`fmsub`), verified with `objdump -dl` against the 32dcfda build.
//! Every other product is a separate rounded multiply.
//!
//! Process state has the native game-thread-only lifetime. Raw field accesses
//! preserve partial cache updates and live reentrant reads; no lock or Rust
//! reference to that state survives a native callback.

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::engine::generated::{AActor, APlayerPawn, FRotator};
use hp2_abi::{FCoords, FVector};

use crate::coords::{self, Trig};
use crate::pawn::{PawnResult, checked};

macro_rules! native_value {
    ($service:expr, $($arg:expr),* $(,)?) => {{
        let mut value = std::mem::MaybeUninit::uninit();
        checked(($service)($($arg,)* value.as_mut_ptr()))?;
        value.assume_init()
    }};
}

/// `PI` (UnMath.h:30) as a single-precision constant.
const PI_F32: f32 = std::f32::consts::PI;

/// `(FLOAT)MAXWORD` (`MAXWORD` is `0xffff`).
const MAXWORD_F32: f32 = 65535.0;

/// Harry class path compared against `UClass::GetPathName()`.
const HARRY_CLASS_PATH: &core::ffi::CStr = c"HGame.Harry";

/// Engine services used by the modern input group. Filled in by
/// `Hp2PawnNativeInputServices.cpp`. Names are NUL-terminated ASCII; native
/// adapters widen them into invocation-local `TCHAR` storage. Except for the
/// two explicitly nonthrowing math leaves, zero status succeeds and return
/// values are published through the final out pointer only on success.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PawnNativeInputServices {
    /// `FindField<UBoolProperty>(class, name)`. Null when the field is missing.
    pub find_bool_property:
        unsafe extern "C" fn(klass: *mut c_void, name: *const c_char, out: *mut *mut c_void) -> i32,
    /// `FindField<UObjectProperty>(class, name)`. Null when the field is missing.
    pub find_object_property:
        unsafe extern "C" fn(klass: *mut c_void, name: *const c_char, out: *mut *mut c_void) -> i32,
    /// `UProperty::Offset` of a bool or object property.
    pub property_offset: unsafe extern "C" fn(prop: *mut c_void, out: *mut i32) -> i32,
    /// `UBoolProperty::BitMask`.
    pub bool_property_mask: unsafe extern "C" fn(prop: *mut c_void, out: *mut u32) -> i32,
    /// `UObject::GetClass()`.
    pub object_class: unsafe extern "C" fn(obj: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UClass::GetSuperClass()`.
    pub class_super: unsafe extern "C" fn(klass: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UObject::IsA(class)`. Returns `UBOOL`.
    pub is_a: unsafe extern "C" fn(obj: *mut c_void, klass: *mut c_void, out: *mut i32) -> i32,
    /// `appStricmp(class->GetPathName(), name) == 0`. Returns `UBOOL`.
    pub class_path_equals:
        unsafe extern "C" fn(klass: *mut c_void, path: *const c_char, out: *mut i32) -> i32,
    /// `obj->GetStateFrame()->StateNode`. Null when `obj` is null or has no state frame.
    pub state_node: unsafe extern "C" fn(obj: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `UState::GetSuperState()`.
    pub state_super: unsafe extern "C" fn(state: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `state->GetFName() == FName(name, FNAME_Find)`. Returns `UBOOL`.
    pub state_name_equals:
        unsafe extern "C" fn(state: *mut c_void, name: *const c_char, out: *mut i32) -> i32,
    /// `debugf(NAME_Warning, TEXT("Modern third-person controls disabled: HGame.Harry
    /// is missing required property %s"), name)`.
    pub log_missing_property: unsafe extern "C" fn(name: *const c_char) -> i32,
    /// `GMath.SinTab(angle)`: explicitly nonthrowing, no native reentry.
    pub sin_tab: unsafe extern "C" fn(angle: i32) -> f32,
    /// `GMath.CosTab(angle)`: explicitly nonthrowing, no native reentry.
    pub cos_tab: unsafe extern "C" fn(angle: i32) -> f32,
}

/// `FModernHarryProperties` (the cached Harry class and its resolved fields).
#[derive(Clone, Copy)]
struct HarryProperties {
    harry_class: *mut c_void,
    resolved: bool,
    valid: bool,
    screen_relative_movement: *mut c_void,
    locked_on_target: *mut c_void,
    fixed_face_direction: *mut c_void,
    in_dueling_mode: *mut c_void,
    reverse_input: *mut c_void,
    keep_stationary: *mut c_void,
    lock_out_forward: *mut c_void,
    lock_out_backward: *mut c_void,
    lock_out_strafe_left: *mut c_void,
    lock_out_strafe_right: *mut c_void,
    is_aiming: *mut c_void,
    e3_demo_lockout: *mut c_void,
    auto_center_camera: *mut c_void,
    boss_target: *mut c_void,
}

impl HarryProperties {
    const ZERO: Self = Self {
        harry_class: ptr::null_mut(),
        resolved: false,
        valid: false,
        screen_relative_movement: ptr::null_mut(),
        locked_on_target: ptr::null_mut(),
        fixed_face_direction: ptr::null_mut(),
        in_dueling_mode: ptr::null_mut(),
        reverse_input: ptr::null_mut(),
        keep_stationary: ptr::null_mut(),
        lock_out_forward: ptr::null_mut(),
        lock_out_backward: ptr::null_mut(),
        lock_out_strafe_left: ptr::null_mut(),
        lock_out_strafe_right: ptr::null_mut(),
        is_aiming: ptr::null_mut(),
        e3_demo_lockout: ptr::null_mut(),
        auto_center_camera: ptr::null_mut(),
        boss_target: ptr::null_mut(),
    };
}

/// `FModernAutoCenterOverride`.
#[derive(Clone, Copy)]
struct AutoCenterOverride {
    pawn: *mut c_void,
    property: *mut c_void,
    saved_value: bool,
}

/// Process-wide state (`GModernHarryProperties`, `GReportedModernHarryContractFailure`,
/// `GModernAutoCenterOverride`).
struct ModernState {
    props: HarryProperties,
    reported_failure: bool,
    auto_center: AutoCenterOverride,
}

// SAFETY: like the native globals, accessed exclusively on the engine game
// thread. Reentry is allowed; access through raw pointers never borrows state.
struct GameThreadState(UnsafeCell<ModernState>);
unsafe impl Sync for GameThreadState {}

static STATE: GameThreadState = GameThreadState(UnsafeCell::new(ModernState {
    props: HarryProperties::ZERO,
    reported_failure: false,
    auto_center: AutoCenterOverride {
        pawn: ptr::null_mut(),
        property: ptr::null_mut(),
        saved_value: false,
    },
}));

/// `ReadModernHarryBool`: `(*(const BITFIELD*)(P + Offset) & BitMask) != 0`.
unsafe fn read_harry_bool(
    svc: &Hp2PawnNativeInputServices,
    pawn: *const c_void,
    prop: *mut c_void,
) -> PawnResult<bool> {
    unsafe {
        let offset = native_value!(svc.property_offset, prop) as isize;
        let mask = native_value!(svc.bool_property_mask, prop);
        let storage = (pawn as *const u8)
            .offset(offset)
            .cast::<u32>()
            .read_unaligned();
        Ok(storage & mask != 0)
    }
}

/// `WriteModernHarryBool`: sets or clears the `BitMask` bit of the `BITFIELD` at `Offset`.
unsafe fn write_harry_bool(
    svc: &Hp2PawnNativeInputServices,
    pawn: *mut c_void,
    prop: *mut c_void,
    value: bool,
) -> PawnResult<()> {
    unsafe {
        let offset = native_value!(svc.property_offset, prop) as isize;
        let mask = native_value!(svc.bool_property_mask, prop);
        let slot = (pawn as *mut u8).offset(offset).cast::<u32>();
        let storage = slot.read_unaligned();
        slot.write_unaligned(if value {
            storage | mask
        } else {
            storage & !mask
        });
        Ok(())
    }
}

/// `ReadModernHarryObject`: the `UObject*` stored at `Offset`.
unsafe fn read_harry_object(
    svc: &Hp2PawnNativeInputServices,
    pawn: *const c_void,
    prop: *mut c_void,
) -> PawnResult<*mut c_void> {
    unsafe {
        let offset = native_value!(svc.property_offset, prop) as isize;
        Ok((pawn as *const u8)
            .offset(offset)
            .cast::<*mut c_void>()
            .read_unaligned())
    }
}

/// `RestoreModernAutoCenter`: writes the saved value back if this pawn owns the override.
unsafe fn restore_auto_center(
    svc: &Hp2PawnNativeInputServices,
    st: *mut ModernState,
    pawn: *mut c_void,
) -> PawnResult<()> {
    unsafe {
        if (*st).auto_center.pawn != pawn {
            return Ok(());
        }
        write_harry_bool(
            svc,
            pawn,
            (*st).auto_center.property,
            (*st).auto_center.saved_value,
        )?;
        (*st).auto_center = AutoCenterOverride {
            pawn: ptr::null_mut(),
            property: ptr::null_mut(),
            saved_value: false,
        };
        Ok(())
    }
}

/// `SuppressModernAutoCenter`: records the override and clears `bAutoCenterCamera` while it is set.
unsafe fn suppress_auto_center(
    svc: &Hp2PawnNativeInputServices,
    st: *mut ModernState,
    pawn: *mut c_void,
    props: *const HarryProperties,
) -> PawnResult<()> {
    unsafe {
        if !read_harry_bool(svc, pawn, (*props).auto_center_camera)? {
            return Ok(());
        }
        (*st).auto_center = AutoCenterOverride {
            pawn,
            property: (*props).auto_center_camera,
            saved_value: true,
        };
        write_harry_bool(svc, pawn, (*props).auto_center_camera, false)?;
        Ok(())
    }
}

/// `ExpandModernAnalogMovement`. The comparison is made in the pre-divide domain.
pub fn expand_analog_movement(
    value: f32,
    delta_seconds: f32,
    max_analog_frame_contribution: f32,
    scale: f32,
) -> f32 {
    if delta_seconds <= 0.0
        || (value * delta_seconds).abs() > max_analog_frame_contribution + 0.0001
    {
        return value;
    }
    value * delta_seconds * scale
}

/// `IsInScriptState`: walks the object's state chain looking for `name`.
unsafe fn in_script_state(
    svc: &Hp2PawnNativeInputServices,
    obj: *mut c_void,
    name: &core::ffi::CStr,
) -> PawnResult<bool> {
    unsafe {
        let mut state = native_value!(svc.state_node, obj);
        while !state.is_null() {
            if native_value!(svc.state_name_equals, state, name.as_ptr()) != 0 {
                return Ok(true);
            }
            state = native_value!(svc.state_super, state);
        }
    }
    Ok(false)
}

/// `GetModernHarryProperties`: caches the `HGame.Harry` class and its fields.
/// Returns `None` when the pawn is not a Harry or the contract check failed.
unsafe fn harry_properties(
    svc: &Hp2PawnNativeInputServices,
    st: *mut ModernState,
    pawn: *mut c_void,
) -> PawnResult<Option<*const HarryProperties>> {
    unsafe {
        let mut harry_class = (*st).props.harry_class;
        if harry_class.is_null() || native_value!(svc.is_a, pawn, harry_class) == 0 {
            harry_class = ptr::null_mut();
            let mut candidate = native_value!(svc.object_class, pawn);
            while !candidate.is_null() {
                if native_value!(svc.class_path_equals, candidate, HARRY_CLASS_PATH.as_ptr()) != 0 {
                    harry_class = candidate;
                    break;
                }
                candidate = native_value!(svc.class_super, candidate);
            }
            if harry_class.is_null() {
                return Ok(None);
            }
        }
        if (*st).props.harry_class != harry_class {
            (*st).props = HarryProperties::ZERO;
            (*st).props.harry_class = harry_class;
        }

        if !(*st).props.resolved {
            (*st).props.resolved = true;
            let mut missing: Option<&'static core::ffi::CStr> = None;
            macro_rules! bool_prop {
                ($name:expr) => {{
                    let prop = native_value!(svc.find_bool_property, harry_class, $name.as_ptr());
                    if prop.is_null() && missing.is_none() {
                        missing = Some($name);
                    }
                    prop
                }};
            }
            (*st).props.screen_relative_movement = bool_prop!(c"bScreenRelativeMovement");
            (*st).props.locked_on_target = bool_prop!(c"bLockedOnTarget");
            (*st).props.fixed_face_direction = bool_prop!(c"bFixedFaceDirection");
            (*st).props.in_dueling_mode = bool_prop!(c"bInDuelingMode");
            (*st).props.reverse_input = bool_prop!(c"bReverseInput");
            (*st).props.keep_stationary = bool_prop!(c"bKeepStationary");
            (*st).props.lock_out_forward = bool_prop!(c"bLockOutForward");
            (*st).props.lock_out_backward = bool_prop!(c"bLockOutBackward");
            (*st).props.lock_out_strafe_left = bool_prop!(c"bLockOutStrafeLeft");
            (*st).props.lock_out_strafe_right = bool_prop!(c"bLockOutStrafeRight");
            (*st).props.is_aiming = bool_prop!(c"bIsAiming");
            (*st).props.e3_demo_lockout = bool_prop!(c"bE3DemoLockout");
            (*st).props.auto_center_camera = bool_prop!(c"bAutoCenterCamera");
            let boss = native_value!(
                svc.find_object_property,
                harry_class,
                c"BossTarget".as_ptr()
            );
            if boss.is_null() && missing.is_none() {
                missing = Some(c"BossTarget");
            }
            (*st).props.boss_target = boss;

            (*st).props.valid = missing.is_none();
            if let Some(name) = missing
                && !(*st).reported_failure
            {
                checked((svc.log_missing_property)(name.as_ptr()))?;
                (*st).reported_failure = true;
            }
        }

        Ok((*st).props.valid.then_some(ptr::addr_of!((*st).props)))
    }
}

/// `IsModernThirdPersonContext`.
unsafe fn is_modern_context(
    svc: &Hp2PawnNativeInputServices,
    pawn: *mut APlayerPawn,
    props: *const HarryProperties,
) -> PawnResult<bool> {
    unsafe {
        let raw = pawn.cast::<c_void>();
        Ok(in_script_state(svc, raw, c"PlayerWalking")?
            && in_script_state(svc, (*pawn).view_target, c"StateStandardCam")?
            && !(*pawn).b_show_menu()
            && !(*pawn).b_is_captured()
            && !read_harry_bool(svc, raw, (*props).keep_stationary)?
            && !read_harry_bool(svc, raw, (*props).e3_demo_lockout)?
            && read_harry_object(svc, raw, (*props).boss_target)?.is_null()
            && !read_harry_bool(svc, raw, (*props).in_dueling_mode)?
            && !read_harry_bool(svc, raw, (*props).locked_on_target)?
            && !read_harry_bool(svc, raw, (*props).fixed_face_direction)?
            && !read_harry_bool(svc, raw, (*props).reverse_input)?)
    }
}

/// `APlayerPawn::BeginModernThirdPersonInput`. Four raw reference arguments
/// may alias each other or pawn fields. Assignments occur in native order,
/// including partial mutations before a later service failure.
///
/// # Safety
/// `pawn` and all four argument pointers must be valid on the game thread.
pub unsafe fn begin_modern_third_person_input(
    svc: &Hp2PawnNativeInputServices,
    pawn: *mut APlayerPawn,
    raw_base_x: *mut f32,
    raw_strafe: *mut f32,
    raw_camera_yaw: *mut f32,
    raw_camera_pitch: *mut f32,
) -> PawnResult<bool> {
    unsafe {
        if !(*pawn).b_modern_third_person_controls() {
            return Ok(false);
        }

        *raw_base_x = (*pawn).a_base_x;
        *raw_strafe = (*pawn).a_strafe;
        *raw_camera_yaw = (*pawn).a_turn;
        *raw_camera_pitch = (*pawn).a_look_up;

        let st = STATE.0.get();
        let raw_pawn = pawn.cast::<c_void>();
        let Some(props) = harry_properties(svc, st, raw_pawn)? else {
            return Ok(false);
        };
        write_harry_bool(svc, raw_pawn, (*props).screen_relative_movement, false)?;
        if !is_modern_context(svc, pawn, props)? {
            return Ok(false);
        }

        (*pawn).a_base_x = 0.0;
        (*pawn).a_turn = 0.0;
        (*pawn).a_look_up = 0.0;
        Ok(true)
    }
}

/// `APlayerPawn::FinishModernThirdPersonInput`.
///
/// # Safety
/// `pawn` must point to a live `APlayerPawn` on the game thread.
#[allow(clippy::too_many_arguments)]
pub unsafe fn finish_modern_third_person_input(
    svc: &Hp2PawnNativeInputServices,
    pawn: *mut APlayerPawn,
    active: bool,
    raw_base_x: f32,
    raw_strafe: f32,
    raw_camera_yaw: f32,
    raw_camera_pitch: f32,
    delta_seconds: f32,
) -> PawnResult<()> {
    unsafe {
        if !active || !(*pawn).b_modern_third_person_controls() {
            return Ok(());
        }

        let st = STATE.0.get();
        let raw_pawn = pawn.cast::<c_void>();
        let Some(props) = harry_properties(svc, st, raw_pawn)? else {
            return Ok(());
        };

        write_harry_bool(svc, raw_pawn, (*props).screen_relative_movement, false)?;
        if read_harry_bool(svc, raw_pawn, (*props).keep_stationary)?
            || !is_modern_context(svc, pawn, props)?
        {
            (*pawn).a_forward = 0.0;
            (*pawn).a_strafe = 0.0;
            (*pawn).a_side_move = 0.0;
            return Ok(());
        }

        let camera_scale = (*pawn).desired_fov * 0.01111;
        (*pawn).smooth_mouse_x = raw_camera_yaw.mul_add(camera_scale, (*pawn).smooth_mouse_x);
        (*pawn).smooth_mouse_y = raw_camera_pitch.mul_add(camera_scale, (*pawn).smooth_mouse_y);

        // JoyX Speed=1 -> RotLeft/Right Speed=200.
        let base_x = expand_analog_movement(raw_base_x, delta_seconds, 0.2, 20000.0);
        // JoyY Speed=2 -> MoveForward Speed=300.
        (*pawn).a_forward = expand_analog_movement((*pawn).a_forward, delta_seconds, 0.4, 15000.0);
        let mut effective_side = 1.2_f32.mul_add(base_x, 0.8 * raw_strafe);
        if (effective_side < 0.0 && read_harry_bool(svc, raw_pawn, (*props).lock_out_strafe_left)?)
            || (effective_side > 0.0
                && read_harry_bool(svc, raw_pawn, (*props).lock_out_strafe_right)?)
        {
            effective_side = 0.0;
        }
        if ((*pawn).a_forward > 0.0 && read_harry_bool(svc, raw_pawn, (*props).lock_out_forward)?)
            || ((*pawn).a_forward < 0.0
                && read_harry_bool(svc, raw_pawn, (*props).lock_out_backward)?)
        {
            (*pawn).a_forward = 0.0;
        }

        (*pawn).a_side_move = effective_side;
        (*pawn).a_strafe = 1.25 * effective_side;
        suppress_auto_center(svc, st, raw_pawn, props)?;
        write_harry_bool(svc, raw_pawn, (*props).screen_relative_movement, true)?;
        Ok(())
    }
}

/// `FVector::Rotation()` (UnMath.cpp:78). `appAtan2`/`appSqrt` are the libm
/// `atan2`/`sqrt` on `DOUBLE`. The `X*X+Y*Y` sum is evaluated in `FLOAT` with `X*X` fused (UnMath.cpp:86).
pub fn vector_rotation(v: FVector) -> FRotator {
    let denom = f64::from(2.0_f32 * PI_F32);
    let yaw = ((f64::from(v.y)).atan2(f64::from(v.x)) * f64::from(MAXWORD_F32) / denom) as i32;
    let planar = v.x.mul_add(v.x, v.y * v.y);
    let pitch =
        (f64::from(v.z).atan2(f64::from(planar).sqrt()) * f64::from(MAXWORD_F32) / denom) as i32;
    FRotator {
        pitch,
        yaw,
        roll: 0,
    }
}

/// `APlayerPawn::ApplyModernThirdPersonMovement`.
///
/// # Safety
/// `pawn` must point to a live `APlayerPawn` on the game thread.
pub unsafe fn apply_modern_third_person_movement(
    svc: &Hp2PawnNativeInputServices,
    pawn: *mut APlayerPawn,
) -> PawnResult<()> {
    unsafe {
        let raw_pawn = pawn.cast::<c_void>();
        let st = STATE.0.get();
        restore_auto_center(svc, st, raw_pawn)?;

        if !(*pawn).b_modern_third_person_controls() {
            return Ok(());
        }
        let Some(props) = harry_properties(svc, st, raw_pawn)? else {
            return Ok(());
        };

        let was_active = read_harry_bool(svc, raw_pawn, (*props).screen_relative_movement)?;
        write_harry_bool(svc, raw_pawn, (*props).screen_relative_movement, false)?;
        if !was_active || !is_modern_context(svc, pawn, props)? {
            return Ok(());
        }
        write_harry_bool(svc, raw_pawn, (*props).screen_relative_movement, true)?;

        let sin = |angle: i32| (svc.sin_tab)(angle);
        let cos = |angle: i32| (svc.cos_tab)(angle);
        let trig = Trig {
            sin: &sin,
            cos: &cos,
        };
        let view_target = (*pawn).view_target.cast::<AActor>();
        let mut camera_axes: FCoords = coords::unit_coords();
        coords::div_rot(
            &mut camera_axes,
            FRotator {
                pitch: 0,
                yaw: (*view_target).rotation.yaw,
                roll: 0,
            },
            &trig,
        );

        let forward = (*pawn).a_forward;
        let side = (*pawn).a_side_move;
        let x_axis = camera_axes.x_axis;
        let y_axis = camera_axes.y_axis;
        let acceleration = FVector {
            x: forward * x_axis.x + side * y_axis.x,
            y: forward * x_axis.y + side * y_axis.y,
            z: 0.0,
        };
        (*pawn).base.base.acceleration = acceleration;

        if !read_harry_bool(svc, raw_pawn, (*props).is_aiming)? {
            let acc = (*pawn).base.base.acceleration;
            let vel = (*pawn).base.base.velocity;
            let yaw = if acc.x != 0.0 || acc.y != 0.0 {
                vector_rotation(acc).yaw
            } else if vel.x != 0.0 || vel.y != 0.0 {
                vector_rotation(FVector {
                    x: vel.x,
                    y: vel.y,
                    z: 0.0,
                })
                .yaw
            } else {
                (*pawn).base.base.rotation.yaw
            };
            (*pawn).base.base.desired_rotation.yaw = yaw;
        }
        Ok(())
    }
}
