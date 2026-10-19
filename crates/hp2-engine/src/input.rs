//! Port of `HarryPotter2/Unreal/Engine/Src/UnIn.cpp` (`UInput`): command
//! execution, button and axis binding lookup, key processing, per-frame axis
//! scaling, input reset, and the key-state read behind
//! `AActor::execGetCurrentKeyState`.
//!
//! The C++ shells in `UnIn.cpp` keep the UObject boilerplate (`IMPLEMENT_CLASS`,
//! `StaticInitInput`, constructor, `StaticConstructor`, `Serialize`), the
//! `guard`/`unguard` wrappers, and the `P_GET_BYTE`/`P_FINISH`/`Result`
//! plumbing of `execGetCurrentKeyState`.
//!
//! Engine state and Core text helpers go through [`Hp2InputServices`], filled by
//! `Hp2InputServices.cpp`. `ParseLine`, `ParseToken`, `ParseCommand`, `Parse`,
//! `appAtoi`, `appStrlen`, `appSprintf`, `FName`, `FString`, and the `GCache`
//! calls are all reached through the table.
//!
//! `TCHAR` is `wchar_t` = 4 bytes on this build (`HP2_HOST_WCHAR_TCHAR`), so
//! text buffers are `u32` units throughout.

use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use hp2_abi::engine::generated::UInput;

/// `IK_MAX` (`EngineClasses.h`): number of key codes.
pub const IK_MAX: i32 = 255;
/// `ALIAS_MAX` (`UnIn.h`): number of alias slots.
const ALIAS_MAX: i32 = 40;
/// `NAME_None` is index 0. `Hp2InputServices.cpp` asserts this at compile time.
const NAME_NONE: i32 = 0;
/// `ARRAY_COUNT(Line)` and `ARRAY_COUNT(Temp)` in `UnIn.cpp`.
const TEXT_BUF_LEN: i32 = 256;

/// `EInputAction` (`EngineClasses.h:391-397`).
const IST_NONE: i32 = 0;
const IST_PRESS: i32 = 1;
const IST_HOLD: i32 = 2;
const IST_RELEASE: i32 = 3;
const IST_AXIS: i32 = 4;

/// `CPF_Input` (`UnObjBas.h:97`).
const CPF_INPUT: u32 = 0x0000_0004;

/// `class_nth_property` kinds: `TFieldIterator<UProperty>`,
/// `TFieldIterator<UByteProperty>`, `TFieldIterator<UFloatProperty>`.
const KIND_PROPERTY: i32 = 0;
const KIND_BYTE: i32 = 1;
const KIND_FLOAT: i32 = 2;

/// `static UBOOL InAlias` inside `UInput::Exec`. Process-wide, as in C++.
static IN_ALIAS: AtomicBool = AtomicBool::new(false);

/// Builds a NUL-terminated `TCHAR` (`u32`) array at compile time.
const fn wide<const N: usize>(s: &str) -> [u32; N] {
    let b = s.as_bytes();
    assert!(b.len() < N);
    let mut out = [0u32; N];
    let mut i = 0;
    while i < b.len() {
        assert!(b[i] < 0x80);
        out[i] = b[i] as u32;
        i += 1;
    }
    out
}

/// `TEXT("...")`: a pointer to a static NUL-terminated `TCHAR` string.
macro_rules! w {
    ($s:literal) => {{
        const N: usize = $s.len() + 1;
        static WIDE: [u32; N] = wide::<N>($s);
        WIDE.as_ptr()
    }};
}

/// Engine services used by the input group. Filled in by
/// `Hp2InputServices.cpp`. `*const u32` arguments are NUL-terminated `TCHAR`
/// strings.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2InputServices {
    /// `UViewport::Actor`.
    pub viewport_actor: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `UViewport::Input`.
    pub viewport_input: unsafe extern "C" fn(viewport: *mut c_void) -> *mut c_void,
    /// `Viewport->UpdateInput(Reset)`.
    pub viewport_update_input: unsafe extern "C" fn(viewport: *mut c_void, reset: i32),
    /// `Viewport->Exec(Cmd, Ar)`; the `UBOOL` result is unused by `UnIn.cpp`.
    pub viewport_exec:
        unsafe extern "C" fn(viewport: *mut c_void, cmd: *const u32, ar: *mut c_void),
    /// `debugf(NAME_Init, "Input system initialized for %s", Viewport->GetName())`.
    pub log_initialized: unsafe extern "C" fn(viewport: *mut c_void),
    /// `Cast<UViewport>(Cast<APlayerPawn>(actor)->Player)`, or null.
    pub actor_viewport: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `UObject::GetClass()`.
    pub object_class: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    /// `GIsRunning`. Returns `UBOOL`.
    pub is_running: unsafe extern "C" fn() -> i32,
    /// `GLog`, as `FOutputDevice*`.
    pub glog: unsafe extern "C" fn() -> *mut c_void,
    /// `appFailAssert(expr, file, line)` for a failed `check`. The file is the
    /// `UnIn.cpp` path fixed in the adapter.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32),
    /// `Ar.Log(text)`.
    pub output_log: unsafe extern "C" fn(ar: *mut c_void, text: *const u32),
    /// `Ar.Logf(text)` for literal text with no arguments.
    pub output_logf: unsafe extern "C" fn(ar: *mut c_void, text: *const u32),
    /// `ParseLine(stream, out, max)`. Returns `UBOOL`.
    pub parse_line: unsafe extern "C" fn(stream: *mut *const u32, out: *mut u32, max: i32) -> i32,
    /// `ParseCommand(stream, match)`. Returns `UBOOL`.
    pub parse_command: unsafe extern "C" fn(stream: *mut *const u32, m: *const u32) -> i32,
    /// `ParseToken(stream, out, max, use_escape)`. Returns `UBOOL`.
    pub parse_token: unsafe extern "C" fn(
        stream: *mut *const u32,
        out: *mut u32,
        max: i32,
        use_escape: i32,
    ) -> i32,
    /// `Parse(stream, match, FLOAT& out)`. Returns `UBOOL`.
    pub parse_float: unsafe extern "C" fn(stream: *const u32, m: *const u32, out: *mut f32) -> i32,
    /// `appAtoi(s)`.
    pub app_atoi: unsafe extern "C" fn(s: *const u32) -> i32,
    /// `appStrlen(s)`.
    pub app_strlen: unsafe extern "C" fn(s: *const u32) -> i32,
    /// `appSprintf(dest, TEXT("IK_%s"), name)`.
    pub app_sprintf_ik: unsafe extern "C" fn(dest: *mut u32, name: *const u32),
    /// `FName(text, FNAME_Find).GetIndex()`. Returns `NAME_None` (0) when absent.
    pub fname_find: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `Aliases[index].Alias.GetIndex()`.
    pub alias_name_index: unsafe extern "C" fn(obj: *mut c_void, index: i32) -> i32,
    /// `*Aliases[index].Command`.
    pub alias_command: unsafe extern "C" fn(obj: *mut c_void, index: i32) -> *const u32,
    /// `Bindings[key].Len()`.
    pub binding_len: unsafe extern "C" fn(obj: *mut c_void, key: i32) -> i32,
    /// `*Bindings[key]`.
    pub binding_chars: unsafe extern "C" fn(obj: *mut c_void, key: i32) -> *const u32,
    /// `FString::Printf(TEXT("%s Speed=%f"), *Bindings[key], length)` on the heap.
    /// Released with `fstring_free`.
    pub direct_axis_command:
        unsafe extern "C" fn(obj: *mut c_void, key: i32, length: f32) -> *mut c_void,
    /// `*FString` of a handle from `direct_axis_command`.
    pub fstring_chars: unsafe extern "C" fn(handle: *mut c_void) -> *const u32,
    /// Deletes a handle from `direct_axis_command`.
    pub fstring_free: unsafe extern "C" fn(handle: *mut c_void),
    /// `*InputKeys->Names(key)`.
    pub key_name_chars: unsafe extern "C" fn(input_keys: *mut c_void, key: i32) -> *const u32,
    /// `FName N(text, FNAME_Find); if (N != NAME_None) return InputKeys->Names.FindItem(N, *out); return 0`.
    /// `FName` has no public index constructor, so the lookup stays in the adapter. Returns `UBOOL`.
    pub key_name_find_item:
        unsafe extern "C" fn(input_keys: *mut c_void, text: *const u32, out: *mut i32) -> i32,
    /// `n`-th property of `TFieldIterator<...>(klass)` for `kind`. Null past the end.
    pub class_nth_property:
        unsafe extern "C" fn(klass: *mut c_void, kind: i32, n: i32) -> *mut c_void,
    /// `UProperty::PropertyFlags`.
    pub property_flags: unsafe extern "C" fn(prop: *mut c_void) -> u32,
    /// `UProperty::GetFName().GetIndex()`.
    pub property_fname_index: unsafe extern "C" fn(prop: *mut c_void) -> i32,
    /// `UProperty::Offset`.
    pub property_offset: unsafe extern "C" fn(prop: *mut c_void) -> i32,
    /// `Cast<UByteProperty>(prop) != NULL`. Returns `UBOOL`.
    pub property_is_byte: unsafe extern "C" fn(prop: *mut c_void) -> i32,
    /// `Cast<UFloatProperty>(prop) != NULL`. Returns `UBOOL`.
    pub property_is_float: unsafe extern "C" fn(prop: *mut c_void) -> i32,
    /// `GCache.Get(id(klass), item)`. Returns null on a miss.
    pub cache_get: unsafe extern "C" fn(klass: *mut c_void, item: *mut *mut c_void) -> *mut c_void,
    /// `GCache.Create(id(klass), item, appCheckedIntSize(size))`. Returns the block.
    pub cache_create: unsafe extern "C" fn(
        klass: *mut c_void,
        item: *mut *mut c_void,
        size: usize,
    ) -> *mut c_void,
    /// `item->Unlock()`.
    pub cache_unlock: unsafe extern "C" fn(item: *mut c_void),
}

/// `FInputVarCache` (`UnIn.cpp:15-38`): `INT Count; UProperty* Properties[]`.
/// The block lives in `GCache`; Rust fills it and C++ only allocates it. Its
/// size is `size_of::<InputVarCache>()` plus `Count` pointers, matching the C++
/// `sizeof(FInputVarCache) + Count*sizeof(UProperty*)` (8 + 8*Count on LP64).
#[repr(C)]
struct InputVarCache {
    count: i32,
    properties: [*mut c_void; 0],
}

/// `FInputVarCache::Get` (`UnIn.cpp:15-38`). Returns the cache block with
/// `item` as `GCache.Get` leaves it. On a miss, counts the `CPF_Input`
/// properties, creates the block, and fills `Properties` in iteration order.
///
/// # Safety
/// `klass` must be a live `UClass`; `item` must be a valid out-slot.
unsafe fn input_var_cache_get(
    svc: &Hp2InputServices,
    klass: *mut c_void,
    item: *mut *mut c_void,
) -> *mut InputVarCache {
    unsafe {
        let mut result = (svc.cache_get)(klass, item);
        if result.is_null() {
            let mut count: usize = 0;
            let mut n = 0;
            loop {
                let prop = (svc.class_nth_property)(klass, KIND_PROPERTY, n);
                if prop.is_null() {
                    break;
                }
                if (svc.property_flags)(prop) & CPF_INPUT != 0 {
                    count += 1;
                }
                n += 1;
            }
            result = (svc.cache_create)(
                klass,
                item,
                size_of::<InputVarCache>() + count * size_of::<*mut c_void>(),
            );
            let cache = result.cast::<InputVarCache>();
            (*cache).count = count as i32;
            let props = ptr::addr_of_mut!((*cache).properties).cast::<*mut c_void>();
            let mut temp = 0usize;
            n = 0;
            loop {
                let prop = (svc.class_nth_property)(klass, KIND_PROPERTY, n);
                if prop.is_null() {
                    break;
                }
                if (svc.property_flags)(prop) & CPF_INPUT != 0 {
                    props.add(temp).write(prop);
                    temp += 1;
                }
                n += 1;
            }
        }
        result.cast::<InputVarCache>()
    }
}

/// `Result->Count` of a cache block.
///
/// # Safety
/// `cache` must be a live block from [`input_var_cache_get`].
unsafe fn cache_count(cache: *mut InputVarCache) -> i32 {
    unsafe { (*cache).count }
}

/// `Result->Properties[i]` of a cache block.
///
/// # Safety
/// `cache` must be live and `0 <= i < Count`.
unsafe fn cache_property(cache: *mut InputVarCache, i: i32) -> *mut c_void {
    unsafe {
        ptr::addr_of!((*cache).properties)
            .cast::<*mut c_void>()
            .offset(isize::try_from(i).unwrap_or(0))
            .read()
    }
}

/// `check(expr)` (`UnFile.h:170`): reports a failure and continues, as the
/// original does.
#[inline]
unsafe fn check(svc: &Hp2InputServices, ok: bool, expr: &CStr, line: i32) {
    if !ok {
        unsafe { (svc.check_failed)(expr.as_ptr(), line) };
    }
}

/// `SetInputAction(NewAction, NewDelta)` (`UnIn.h`).
///
/// # Safety
/// `this` must point to a live `UInput`.
unsafe fn set_input_action(this: *mut UInput, new_action: i32, new_delta: f32) {
    unsafe {
        (*this).action = new_action;
        (*this).delta = new_delta;
    }
}

/// `UInput::FindButtonName` (`UnIn.cpp:121-143`). Returns the `BYTE` property
/// address, or null.
///
/// # Safety
/// `this` must be a live `UInput`; `actor` a live `AActor`; `name` a
/// NUL-terminated `TCHAR` string.
pub unsafe fn find_button_name(
    svc: &Hp2InputServices,
    this: *mut UInput,
    actor: *mut c_void,
    name: *const u32,
) -> *mut u8 {
    unsafe {
        check(svc, !(*this).viewport.is_null(), c"Viewport", 124);
        check(svc, !actor.is_null(), c"Actor", 125);
        let button = (svc.fname_find)(name);
        if button != NAME_NONE {
            let mut item: *mut c_void = ptr::null_mut();
            let cache = input_var_cache_get(svc, (svc.object_class)(actor), &raw mut item);
            let count = cache_count(cache);
            let mut i = 0;
            while i < count {
                let prop = cache_property(cache, i);
                if (svc.property_fname_index)(prop) == button && (svc.property_is_byte)(prop) != 0 {
                    break;
                }
                i += 1;
            }
            (svc.cache_unlock)(item);
            if i < count {
                let prop = cache_property(cache, i);
                return actor
                    .cast::<u8>()
                    .offset(isize::try_from((svc.property_offset)(prop)).unwrap_or(0));
            }
        }
        ptr::null_mut()
    }
}

/// `UInput::FindAxisName` (`UnIn.cpp:148-170`). Returns the `FLOAT` property
/// address, or null.
///
/// # Safety
/// Same as [`find_button_name`].
pub unsafe fn find_axis_name(
    svc: &Hp2InputServices,
    this: *mut UInput,
    actor: *mut c_void,
    name: *const u32,
) -> *mut f32 {
    unsafe {
        check(svc, !(*this).viewport.is_null(), c"Viewport", 151);
        check(svc, !actor.is_null(), c"Actor", 152);
        let axis = (svc.fname_find)(name);
        if axis != NAME_NONE {
            let mut item: *mut c_void = ptr::null_mut();
            let cache = input_var_cache_get(svc, (svc.object_class)(actor), &raw mut item);
            let count = cache_count(cache);
            let mut i = 0;
            while i < count {
                let prop = cache_property(cache, i);
                if (svc.property_fname_index)(prop) == axis && (svc.property_is_float)(prop) != 0 {
                    break;
                }
                i += 1;
            }
            (svc.cache_unlock)(item);
            if i < count {
                let prop = cache_property(cache, i);
                return actor
                    .cast::<u8>()
                    .offset(isize::try_from((svc.property_offset)(prop)).unwrap_or(0))
                    .cast::<f32>();
            }
        }
        ptr::null_mut()
    }
}

/// `UInput::ExecInputCommands` (`UnIn.cpp:175-188`). Each line is parsed into
/// `Line` (256 units). `ParseCommand` on `OnRelease` advances the line cursor,
/// and the advanced cursor is what runs, exactly as in the C++.
///
/// # Safety
/// `this` must be a live `UInput`; `cmd` a NUL-terminated `TCHAR` string; `ar`
/// a live `FOutputDevice`.
pub unsafe fn exec_input_commands(
    svc: &Hp2InputServices,
    this: *mut UInput,
    mut cmd: *const u32,
    ar: *mut c_void,
) {
    unsafe {
        let mut line = [0u32; TEXT_BUF_LEN as usize];
        while (svc.parse_line)(&raw mut cmd, line.as_mut_ptr(), TEXT_BUF_LEN) != 0 {
            let mut str_ptr: *const u32 = line.as_ptr();
            let action = (*this).action;
            if action == IST_PRESS
                || (action == IST_RELEASE
                    && (svc.parse_command)(&raw mut str_ptr, w!("OnRelease")) != 0)
            {
                (svc.viewport_exec)((*this).viewport, str_ptr, ar);
            } else {
                exec(svc, this, str_ptr, ar);
            }
        }
    }
}

/// `UInput::Init` (`UnIn.cpp:193-205`).
///
/// # Safety
/// `this` must be a live `UInput`; `viewport` a live `UViewport`.
pub unsafe fn init(svc: &Hp2InputServices, this: *mut UInput, viewport: *mut c_void) {
    unsafe {
        (*this).viewport = viewport;
        reset_input(svc, this);
        (svc.log_initialized)(viewport);
    }
}

/// `UInput::Exec` (`UnIn.cpp:214-330`). Returns `UBOOL`.
///
/// Branch order is BUTTON, PULSE, TOGGLE, AXIS, KEYNAME, KEYBINDING, then the
/// alias table. The inner `guard(ExecAlias)` label is not reproduced; the
/// shell's `guard(UInput::Exec)` covers the same failure path.
///
/// # Safety
/// `this` must be a live `UInput`; `s` a NUL-terminated `TCHAR` string; `ar`
/// a live `FOutputDevice`.
pub unsafe fn exec(
    svc: &Hp2InputServices,
    this: *mut UInput,
    mut s: *const u32,
    ar: *mut c_void,
) -> i32 {
    unsafe {
        let mut temp = [0u32; TEXT_BUF_LEN as usize];
        let temp_ptr = temp.as_mut_ptr();

        if (svc.parse_command)(&raw mut s, w!("BUTTON")) != 0 {
            let actor = (svc.viewport_actor)((*this).viewport);
            let found =
                !actor.is_null() && (svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0) != 0;
            let button = if found {
                find_button_name(svc, this, actor, temp_ptr)
            } else {
                ptr::null_mut()
            };
            if found && !button.is_null() {
                let action = (*this).action;
                if action == IST_PRESS {
                    *button = 1;
                } else if action == IST_RELEASE && *button != 0 {
                    *button = 0;
                }
            } else {
                (svc.output_log)(ar, w!("Bad Button command"));
            }
            return 1;
        } else if (svc.parse_command)(&raw mut s, w!("PULSE")) != 0 {
            let actor = (svc.viewport_actor)((*this).viewport);
            let found =
                !actor.is_null() && (svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0) != 0;
            let button = if found {
                find_button_name(svc, this, actor, temp_ptr)
            } else {
                ptr::null_mut()
            };
            if found && !button.is_null() {
                if (*this).action == IST_PRESS {
                    *button = 1;
                }
            } else {
                (svc.output_log)(ar, w!("Bad Button command"));
            }
            return 1;
        } else if (svc.parse_command)(&raw mut s, w!("TOGGLE")) != 0 {
            let actor = (svc.viewport_actor)((*this).viewport);
            let found =
                !actor.is_null() && (svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0) != 0;
            let button = if found {
                find_button_name(svc, this, actor, temp_ptr)
            } else {
                ptr::null_mut()
            };
            if found && !button.is_null() {
                if (*this).action == IST_PRESS {
                    *button ^= 0x80;
                }
            } else {
                (svc.output_log)(ar, w!("Bad Toggle command"));
            }
            return 1;
        } else if (svc.parse_command)(&raw mut s, w!("AXIS")) != 0 {
            let actor = (svc.viewport_actor)((*this).viewport);
            let found =
                !actor.is_null() && (svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0) != 0;
            let axis = if found {
                find_axis_name(svc, this, actor, temp_ptr)
            } else {
                ptr::null_mut()
            };
            if found && !axis.is_null() {
                let mut speed: f32 = 1.0;
                let mut speed_base: f32 = 0.0;
                (svc.parse_float)(s, w!("SPEED="), &raw mut speed);
                (svc.parse_float)(s, w!("SPEEDBASE="), &raw mut speed_base);
                let delta = (*this).delta;
                if speed_base > 0.0 {
                    // `*Axis += Delta * SpeedBase * Speed` (contracted: fma of Delta*SpeedBase, Speed).
                    *axis = (delta * speed_base).mul_add(speed, *axis);
                } else if (*this).action == IST_AXIS {
                    // `*Axis += 0.01f * Delta * Speed` (contracted: fma of 0.01*Delta, Speed).
                    *axis = (0.01_f32 * delta).mul_add(speed, *axis);
                } else if (*this).action == IST_HOLD {
                    // `*Axis += Delta * Speed` (contracted: fma of Delta, Speed).
                    *axis = delta.mul_add(speed, *axis);
                }
            } else {
                (svc.output_logf)(ar, w!("Bad Axis command"));
            }
            return 1;
        } else if (svc.parse_command)(&raw mut s, w!("KEYNAME")) != 0 {
            let key_no = (svc.app_atoi)(s);
            (svc.output_log)(ar, get_key_name(svc, this, key_no));
            return 1;
        } else if (svc.parse_command)(&raw mut s, w!("KEYBINDING")) != 0 {
            let mut i_key: i32 = 0;
            if find_key_name(svc, this, s, &raw mut i_key) != 0 {
                (svc.output_log)(ar, (svc.binding_chars)(this.cast(), i_key));
            }
            return 1;
        } else if !IN_ALIAS.load(Ordering::Relaxed)
            && (svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0) != 0
        {
            let name = (svc.fname_find)(temp_ptr);
            if name != NAME_NONE {
                for i in 0..ALIAS_MAX {
                    if (svc.alias_name_index)(this.cast(), i) == name {
                        IN_ALIAS.store(true, Ordering::Relaxed);
                        exec_input_commands(svc, this, (svc.alias_command)(this.cast(), i), ar);
                        IN_ALIAS.store(false, Ordering::Relaxed);
                        return 1;
                    }
                }
            }
        }
        0
    }
}

/// `UInput::PreProcess` (`UnIn.cpp:339-358`). Returns `UBOOL`. No services.
///
/// # Safety
/// `this` must be a live `UInput`; `key` must be a valid index of `KeyDownTable`.
pub unsafe fn pre_process(this: *mut UInput, key: i32, state: i32) -> i32 {
    unsafe {
        let slot = (*this).key_down_table.as_mut_ptr().offset(key as isize);
        match state {
            IST_PRESS => {
                if *slot != 0 {
                    return 0;
                }
                *slot = 1;
                1
            }
            IST_RELEASE => {
                if *slot == 0 {
                    return 0;
                }
                *slot = 0;
                1
            }
            _ => 1,
        }
    }
}

/// `UInput::Process` (`UnIn.cpp:363-379`). Returns `UBOOL`.
///
/// # Safety
/// `this` must be a live `UInput`; `ar` a live `FOutputDevice`.
pub unsafe fn process(
    svc: &Hp2InputServices,
    this: *mut UInput,
    ar: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    unsafe {
        check(
            svc,
            (0..IK_MAX).contains(&key),
            c"iKey>=0&&iKey<IK_MAX",
            366,
        );
        if (svc.binding_len)(this.cast(), key) != 0 {
            set_input_action(this, state, delta);
            exec_input_commands(svc, this, (svc.binding_chars)(this.cast(), key), ar);
            set_input_action(this, IST_NONE, 0.0);
            return 1;
        }
        0
    }
}

/// `UInput::DirectAxis` (`UnIn.cpp:384-390`).
///
/// # Safety
/// `this` must be a live `UInput`.
pub unsafe fn direct_axis(
    svc: &Hp2InputServices,
    this: *mut UInput,
    key: i32,
    length: f32,
    delta: f32,
) {
    unsafe {
        // `FString Command = FString::Printf(...)` comes first in the C++.
        let command = (svc.direct_axis_command)(this.cast(), key, length);
        set_input_action(this, IST_HOLD, delta);
        exec_input_commands(svc, this, (svc.fstring_chars)(command), (svc.glog)());
        set_input_action(this, IST_NONE, 0.0);
        (svc.fstring_free)(command);
    }
}

/// `UInput::ReadInput` (`UnIn.cpp:399-424`).
///
/// # Safety
/// `this` must be a live `UInput` with a live `Viewport` and `Actor`.
pub unsafe fn read_input(svc: &Hp2InputServices, this: *mut UInput, delta_seconds: f32) {
    unsafe {
        if (svc.is_running)() == 0 {
            return;
        }
        let mut item: *mut c_void = ptr::null_mut();
        let cache = input_var_cache_get(
            svc,
            (svc.object_class)((svc.viewport_actor)((*this).viewport)),
            &raw mut item,
        );
        if delta_seconds != -1.0 {
            for i in 0..IK_MAX {
                if (*this).key_down_table[i as usize] != 0 {
                    process(svc, this, (svc.glog)(), i, IST_HOLD, delta_seconds);
                }
            }
        }
        let scale = if delta_seconds != -1.0 {
            20.0_f32 / delta_seconds
        } else {
            0.0
        };
        let mut i = 0;
        while i < cache_count(cache) {
            let prop = cache_property(cache, i);
            if (svc.property_is_float)(prop) != 0 {
                let offset = (svc.property_offset)(prop);
                let axis = (svc.viewport_actor)((*this).viewport)
                    .cast::<u8>()
                    .offset(isize::try_from(offset).unwrap_or(0))
                    .cast::<f32>();
                *axis *= scale;
            }
            i += 1;
        }
        (svc.cache_unlock)(item);
    }
}

/// `UInput::ResetInput` (`UnIn.cpp:433-459`).
///
/// # Safety
/// `this` must be a live `UInput` with a live `Viewport` and `Actor`.
pub unsafe fn reset_input(svc: &Hp2InputServices, this: *mut UInput) {
    unsafe {
        let vp = (*this).viewport;
        check(svc, !vp.is_null(), c"Viewport", 436);
        (*this).key_down_table.fill(0);

        let klass_b = (svc.object_class)((svc.viewport_actor)(vp));
        let mut n = 0;
        loop {
            let prop = (svc.class_nth_property)(klass_b, KIND_BYTE, n);
            if prop.is_null() {
                break;
            }
            if (svc.property_flags)(prop) & CPF_INPUT != 0 {
                let offset = (svc.property_offset)(prop);
                let p = (svc.viewport_actor)(vp)
                    .cast::<u8>()
                    .offset(isize::try_from(offset).unwrap_or(0));
                *p = 0;
            }
            n += 1;
        }

        let klass_f = (svc.object_class)((svc.viewport_actor)(vp));
        n = 0;
        loop {
            let prop = (svc.class_nth_property)(klass_f, KIND_FLOAT, n);
            if prop.is_null() {
                break;
            }
            if (svc.property_flags)(prop) & CPF_INPUT != 0 {
                let offset = (svc.property_offset)(prop);
                let p = (svc.viewport_actor)(vp)
                    .cast::<u8>()
                    .offset(isize::try_from(offset).unwrap_or(0))
                    .cast::<f32>();
                *p = 0.0;
            }
            n += 1;
        }

        set_input_action(this, IST_NONE, 0.0);
        (svc.viewport_update_input)(vp, 1);
    }
}

/// `UInput::GetKeyName` (`UnIn.cpp:492-500`). Returns the name after the
/// `IK_` prefix, or an empty string.
///
/// # Safety
/// `this` must be a live `UInput`.
pub unsafe fn get_key_name(svc: &Hp2InputServices, this: *mut UInput, key: i32) -> *const u32 {
    unsafe {
        if (0..IK_MAX).contains(&key) {
            let name = (svc.key_name_chars)((*this).input_keys, key);
            if (svc.app_strlen)(name) > 3 {
                return name.add(3);
            }
        }
        w!("")
    }
}

/// `UInput::FindKeyName` (`UnIn.cpp:505-515`). Returns `UBOOL`; on success
/// writes the key code to `i_key`.
///
/// Divergence: `appSprintf` writes into a fixed 256-unit buffer with no bound
/// check, as in C++. A key name of 254 units or more overflows that buffer.
///
/// # Safety
/// `this` must be a live `UInput`; `key_name` a NUL-terminated `TCHAR` string;
/// `i_key` a valid out-pointer.
pub unsafe fn find_key_name(
    svc: &Hp2InputServices,
    this: *mut UInput,
    key_name: *const u32,
    i_key: *mut i32,
) -> i32 {
    unsafe {
        let mut temp = [0u32; TEXT_BUF_LEN as usize];
        (svc.app_sprintf_ik)(temp.as_mut_ptr(), key_name);
        (svc.key_name_find_item)((*this).input_keys, temp.as_ptr(), i_key)
    }
}

/// `AActor::execGetCurrentKeyState` body (`UnIn.cpp:465-483`): returns the
/// `UBOOL` the shell copies into `Result`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn current_key_state(svc: &Hp2InputServices, actor: *mut c_void, key: u8) -> i32 {
    unsafe {
        let mut result = 0;
        if i32::from(key) < IK_MAX {
            let viewport = (svc.actor_viewport)(actor);
            if !viewport.is_null() {
                let input = (svc.viewport_input)(viewport);
                if !input.is_null() {
                    result = i32::from(key_down(input.cast::<UInput>(), i32::from(key)) != 0);
                }
            }
        }
        result
    }
}

/// `UInput::KeyDown(int)` (`UInput.h`): `KeyDownTable[Clamp(i, 0, IK_MAX-1)]`.
///
/// # Safety
/// `this` must be a live `UInput`.
unsafe fn key_down(this: *mut UInput, i: i32) -> u8 {
    unsafe { (*this).key_down_table[i.clamp(0, IK_MAX - 1) as usize] }
}
