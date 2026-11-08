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
//! calls are all reached through the checked table. Native status is propagated
//! immediately; a successful value is read only after its output slot is written.
//!
//! `TCHAR` is `wchar_t` = 4 bytes on this build (`HP2_HOST_WCHAR_TCHAR`), so
//! text buffers are `u32` units throughout.

use std::ffi::{CStr, c_char, c_void};
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::camera::{checked_unit, checked_value};
use hp2_abi::engine::generated::{UInput, UObject, UProperty, UViewport};

type Res<T> = Result<T, i32>;

/// `IK_MAX` (`EngineClasses.h`): number of key codes.
pub const IK_MAX: i32 = 255;
/// `ALIAS_MAX` (`UnIn.h`): number of alias slots.
const ALIAS_MAX: i32 = 40;
/// `NAME_None` is index 0. `Hp2InputServices.cpp` asserts this at compile time.
const NAME_NONE: i32 = 0;
/// `ARRAY_COUNT(Line)` and `ARRAY_COUNT(Temp)` in `UnIn.cpp`.
const TEXT_BUF_LEN: i32 = 256;

/// `EInputAction` (`EngineClasses.h:391-397`).
pub(crate) const IST_NONE: i32 = 0;
pub(crate) const IST_PRESS: i32 = 1;
pub(crate) const IST_HOLD: i32 = 2;
pub(crate) const IST_RELEASE: i32 = 3;
pub(crate) const IST_AXIS: i32 = 4;

/// `CPF_Input` (`UnObjBas.h:97`).
const CPF_INPUT: u32 = 0x0000_0004;

/// Native iterator kinds: `TFieldIterator<UProperty>`,
/// `TFieldIterator<UByteProperty>`, `TFieldIterator<UFloatProperty>`.
const KIND_PROPERTY: i32 = 0;
const KIND_BYTE: i32 = 1;
const KIND_FLOAT: i32 = 2;

/// `static UBOOL InAlias` inside `UInput::Exec`. Process-wide, as in C++.
static IN_ALIAS: AtomicBool = AtomicBool::new(false);

pub(crate) fn in_alias() -> bool {
    IN_ALIAS.load(Ordering::Relaxed)
}

pub(crate) fn with_alias_scope<T, E>(execute: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    // Match native normal-return-only reset; command failures propagate to the caller.
    IN_ALIAS.store(true, Ordering::Relaxed);
    let value = execute()?;
    IN_ALIAS.store(false, Ordering::Relaxed);
    Ok(value)
}

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
    /// `Viewport->UpdateInput(Reset)`.
    pub viewport_update_input: unsafe extern "C" fn(viewport: *mut c_void, reset: i32) -> i32,
    /// `Viewport->Exec(Cmd, Ar)`; the `UBOOL` result is unused by `UnIn.cpp`.
    pub viewport_exec:
        unsafe extern "C" fn(viewport: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    /// `debugf(NAME_Init, "Input system initialized for %s", Viewport->GetName())`.
    pub log_initialized: unsafe extern "C" fn(viewport: *mut c_void) -> i32,
    /// `Cast<UViewport>(Cast<APlayerPawn>(actor)->Player)`, or null.
    pub actor_viewport: unsafe extern "C" fn(actor: *mut c_void, out: *mut *mut c_void) -> i32,
    /// `GIsRunning`. Returns `UBOOL`.
    pub is_running: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `GLog`, as `FOutputDevice*`.
    pub glog: unsafe extern "C" fn(out: *mut *mut c_void) -> i32,
    /// `appFailAssert(expr, file, line)` for a failed `check`. The file is the
    /// `UnIn.cpp` path fixed in the adapter.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    /// `Ar.Log(text)`.
    pub output_log: unsafe extern "C" fn(ar: *mut c_void, text: *const u32) -> i32,
    /// `Ar.Logf(text)` for literal text with no arguments.
    pub output_logf: unsafe extern "C" fn(ar: *mut c_void, text: *const u32) -> i32,
    /// `ParseLine(stream, out, max)`. Returns `UBOOL`.
    pub parse_line: unsafe extern "C" fn(
        stream: *mut *const u32,
        out: *mut u32,
        max: i32,
        result_out: *mut i32,
    ) -> i32,
    /// `ParseCommand(stream, match)`. Returns `UBOOL`.
    pub parse_command:
        unsafe extern "C" fn(stream: *mut *const u32, m: *const u32, out: *mut i32) -> i32,
    /// `ParseToken(stream, out, max, use_escape)`. Returns `UBOOL`.
    pub parse_token: unsafe extern "C" fn(
        stream: *mut *const u32,
        out: *mut u32,
        max: i32,
        use_escape: i32,
        result_out: *mut i32,
    ) -> i32,
    /// `Parse(stream, match, FLOAT& out)`. Returns `UBOOL`.
    pub parse_float: unsafe extern "C" fn(
        stream: *const u32,
        m: *const u32,
        out: *mut f32,
        result_out: *mut i32,
    ) -> i32,
    /// `appAtoi(s)`.
    pub app_atoi: unsafe extern "C" fn(s: *const u32, out: *mut i32) -> i32,
    /// `appStrlen(s)`.
    pub app_strlen: unsafe extern "C" fn(s: *const u32, out: *mut i32) -> i32,
    /// `appSprintf(dest, TEXT("IK_%s"), name)`.
    pub app_sprintf_ik: unsafe extern "C" fn(dest: *mut u32, name: *const u32) -> i32,
    /// `FName(text, FNAME_Find).GetIndex()`. Returns `NAME_None` (0) when absent.
    pub fname_find: unsafe extern "C" fn(text: *const u32, out: *mut i32) -> i32,
    /// `Aliases[index].Alias.GetIndex()`.
    pub alias_name_index: unsafe extern "C" fn(obj: *mut c_void, index: i32, out: *mut i32) -> i32,
    /// `*Aliases[index].Command`.
    pub alias_command:
        unsafe extern "C" fn(obj: *mut c_void, index: i32, out: *mut *const u32) -> i32,
    /// `Bindings[key].Len()`.
    pub binding_len: unsafe extern "C" fn(obj: *mut c_void, key: i32, out: *mut i32) -> i32,
    /// `*Bindings[key]`.
    pub binding_chars:
        unsafe extern "C" fn(obj: *mut c_void, key: i32, out: *mut *const u32) -> i32,
    /// Native FString lifetime spanning the checked Rust axis continuation.
    pub direct_axis_scope: unsafe extern "C" fn(
        obj: *mut c_void,
        key: i32,
        length: f32,
        context: *mut c_void,
        run: unsafe extern "C" fn(*mut c_void, *const u32) -> i32,
    ) -> i32,
    /// `*InputKeys->Names(key)`.
    pub key_name_chars:
        unsafe extern "C" fn(input_keys: *mut c_void, key: i32, out: *mut *const u32) -> i32,
    /// `FName N(text, FNAME_Find); if (N != NAME_None) return InputKeys->Names.FindItem(N, *out); return 0`.
    /// `FName` has no public index constructor, so the lookup stays in the adapter. Returns `UBOOL`.
    pub key_name_find_item: unsafe extern "C" fn(
        input_keys: *mut c_void,
        text: *const u32,
        out: *mut i32,
        result_out: *mut i32,
    ) -> i32,
    /// Native TFieldIterator lifetime; `run` may reenter the public input API.
    pub property_scope: unsafe extern "C" fn(
        klass: *mut c_void,
        kind: i32,
        context: *mut c_void,
        run: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    ) -> i32,
    pub property_current: unsafe extern "C" fn(iterator: *mut c_void, out: *mut *mut c_void) -> i32,
    pub property_next: unsafe extern "C" fn(iterator: *mut c_void) -> i32,
    /// `Cast<UByteProperty>(prop) != NULL`. Returns `UBOOL`.
    pub property_is_byte: unsafe extern "C" fn(prop: *mut c_void, out: *mut i32) -> i32,
    /// `Cast<UFloatProperty>(prop) != NULL`. Returns `UBOOL`.
    pub property_is_float: unsafe extern "C" fn(prop: *mut c_void, out: *mut i32) -> i32,
    /// `GCache.Get(id(klass), item)`. Returns null on a miss.
    pub cache_get: unsafe extern "C" fn(
        klass: *mut c_void,
        item: *mut *mut c_void,
        out: *mut *mut c_void,
    ) -> i32,
    /// `GCache.Create(id(klass), item, appCheckedIntSize(size))`. Returns the block.
    pub cache_create: unsafe extern "C" fn(
        klass: *mut c_void,
        item: *mut *mut c_void,
        size: usize,
        out: *mut *mut c_void,
    ) -> i32,
    /// `item->Unlock()`.
    pub cache_unlock: unsafe extern "C" fn(item: *mut c_void) -> i32,
    /// Virtual `UInput::Process`, preserving dispatch to derived input classes.
    pub input_process: unsafe extern "C" fn(
        input: *mut c_void,
        ar: *mut c_void,
        key: i32,
        action: i32,
        delta: f32,
        out: *mut i32,
    ) -> i32,
    pub input_exec: unsafe extern "C" fn(
        obj: *mut c_void,
        cmd: *const u32,
        ar: *mut c_void,
        out: *mut i32,
    ) -> i32,
    pub input_exec_commands:
        unsafe extern "C" fn(obj: *mut c_void, cmd: *const u32, ar: *mut c_void) -> i32,
    pub input_reset: unsafe extern "C" fn(obj: *mut c_void) -> i32,
    pub input_find_button: unsafe extern "C" fn(
        obj: *mut c_void,
        actor: *mut c_void,
        name: *const u32,
        out: *mut *mut u8,
    ) -> i32,
    pub input_find_axis: unsafe extern "C" fn(
        obj: *mut c_void,
        actor: *mut c_void,
        name: *const u32,
        out: *mut *mut f32,
    ) -> i32,
    pub input_key_name:
        unsafe extern "C" fn(obj: *mut c_void, key: i32, out: *mut *const u32) -> i32,
    pub input_find_key: unsafe extern "C" fn(
        obj: *mut c_void,
        name: *const u32,
        key: *mut i32,
        out: *mut i32,
    ) -> i32,
    /// The original inner guard(ExecAlias), including the non-restored failure state.
    pub alias_scope: unsafe extern "C" fn(
        context: *mut c_void,
        run: unsafe extern "C" fn(*mut c_void) -> i32,
    ) -> i32,
}

static mut INPUT_SERVICES: Option<Hp2InputServices> = None;

/// # Safety
/// `value` is the real, immutable `GHp2InputServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2InputServices) {
    unsafe { (&raw mut INPUT_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Res<Hp2InputServices> {
    unsafe { (&raw const INPUT_SERVICES).read().ok_or(1) }
}

unsafe fn for_each_property<F: FnMut(*mut c_void) -> Res<()>>(
    svc: &Hp2InputServices,
    klass: *mut c_void,
    kind: i32,
    mut visit: F,
) -> Res<()> {
    struct Context<F> {
        svc: *const Hp2InputServices,
        visit: *mut F,
    }
    unsafe extern "C" fn run<F: FnMut(*mut c_void) -> Res<()>>(
        context: *mut c_void,
        iterator: *mut c_void,
    ) -> i32 {
        unsafe {
            let context = context.cast::<Context<F>>();
            let svc = &*(*context).svc;
            let result = (|| -> Res<()> {
                loop {
                    let property = checked_value!((svc.property_current)(iterator));
                    if property.is_null() {
                        break;
                    }
                    (&mut *(*context).visit)(property)?;
                    checked_unit!((svc.property_next)(iterator));
                }
                Ok(())
            })();
            result.map_or_else(|status| -status, |()| 0)
        }
    }
    let mut context = Context {
        svc,
        visit: &raw mut visit,
    };
    unsafe {
        checked_unit!((svc.property_scope)(
            klass,
            kind,
            (&raw mut context).cast(),
            run::<F>
        ));
    }
    Ok(())
}

/// `FInputVarCache` (`UnIn.cpp:15-38`): `INT Count; UProperty* Properties[]`.
/// The block lives in `GCache`; Rust fills it and C++ only allocates it. Its
/// size is `size_of::<InputVarCache>()` plus `Count` pointers, matching the C++
/// `sizeof(FInputVarCache) + Count*sizeof(UProperty*)` (8 + 8*Count on LP64).
#[repr(C)]
pub(crate) struct InputVarCache {
    pub(crate) count: i32,
    pub(crate) properties: [*mut c_void; 0],
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
) -> Res<*mut InputVarCache> {
    unsafe {
        let mut result = checked_value!((svc.cache_get)(klass, item));
        if result.is_null() {
            let mut count = 0usize;
            for_each_property(svc, klass, KIND_PROPERTY, |prop| {
                if (*(prop as *mut UProperty)).property_flags & CPF_INPUT != 0 {
                    count += 1;
                }
                Ok(())
            })?;
            result = checked_value!((svc.cache_create)(
                klass,
                item,
                size_of::<InputVarCache>() + count * size_of::<*mut c_void>()
            ));
            let cache = result.cast::<InputVarCache>();
            (*cache).count = count as i32;
            let props = ptr::addr_of_mut!((*cache).properties).cast::<*mut c_void>();
            let mut index = 0;
            for_each_property(svc, klass, KIND_PROPERTY, |prop| {
                if (*(prop as *mut UProperty)).property_flags & CPF_INPUT != 0 {
                    props.add(index).write(prop);
                    index += 1;
                }
                Ok(())
            })?;
        }
        Ok(result.cast())
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

/// `check(expr)` (`UnFile.h:170`): the native leaf respects DO_CHECK and
/// captures a thrown assertion before Rust resumes.
#[inline]
unsafe fn check(svc: &Hp2InputServices, ok: bool, expr: &CStr, line: i32) -> Res<()> {
    if !ok {
        unsafe { checked_unit!((svc.check_failed)(expr.as_ptr(), line)) };
    }

    Ok(())
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
) -> Res<*mut u8> {
    Ok(unsafe {
        check(svc, !(*this).viewport.is_null(), c"Viewport", 124)?;
        check(svc, !actor.is_null(), c"Actor", 125)?;
        let button = checked_value!((svc.fname_find)(name));
        if button != NAME_NONE {
            let mut item: *mut c_void = ptr::null_mut();
            let cache = input_var_cache_get(
                svc,
                (*(actor as *mut UObject)).class,
                &raw mut item,
            )?;
            let count = cache_count(cache);
            let mut i = 0;
            while i < count {
                let prop = cache_property(cache, i);
                if i32::from_ne_bytes((*(prop as *mut UProperty)).name) == button
                    && checked_value!((svc.property_is_byte)(prop)) != 0
                {
                    break;
                }
                i += 1;
            }
            checked_unit!((svc.cache_unlock)(item));
            if i < count {
                let prop = cache_property(cache, i);
                return Ok(actor.cast::<u8>().offset(
                    isize::try_from((*(prop as *mut UProperty)).offset).unwrap_or(0),
                ));
            }
        }
        ptr::null_mut()
    })
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
) -> Res<*mut f32> {
    Ok(unsafe {
        check(svc, !(*this).viewport.is_null(), c"Viewport", 151)?;
        check(svc, !actor.is_null(), c"Actor", 152)?;
        let axis = checked_value!((svc.fname_find)(name));
        if axis != NAME_NONE {
            let mut item: *mut c_void = ptr::null_mut();
            let cache = input_var_cache_get(
                svc,
                (*(actor as *mut UObject)).class,
                &raw mut item,
            )?;
            let count = cache_count(cache);
            let mut i = 0;
            while i < count {
                let prop = cache_property(cache, i);
                if i32::from_ne_bytes((*(prop as *mut UProperty)).name) == axis
                    && checked_value!((svc.property_is_float)(prop)) != 0
                {
                    break;
                }
                i += 1;
            }
            checked_unit!((svc.cache_unlock)(item));
            if i < count {
                let prop = cache_property(cache, i);
                return Ok(actor
                    .cast::<u8>()
                    .offset(
                        isize::try_from((*(prop as *mut UProperty)).offset).unwrap_or(0),
                    )
                    .cast::<f32>());
            }
        }
        ptr::null_mut()
    })
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
    cmd: *const u32,
    ar: *mut c_void,
) -> Res<()> {
    unsafe {
        exec_input_commands_with(
            this,
            cmd,
            |cursor, line, len| {
                Ok(checked_value!((svc.parse_line)(cursor.cast(), line.cast(), len)) != 0)
            },
            |cursor, token| {
                Ok(checked_value!((svc.parse_command)(cursor.cast(), token.cast())) != 0)
            },
            |viewport, line| {
                checked_unit!((svc.viewport_exec)(viewport.cast(), line, ar));
                Ok(())
            },
            |input, line| {
                let _ = checked_value!((svc.input_exec)(input.cast(), line, ar));
                Ok(())
            },
        )
    }
}

/// Shared `ExecInputCommands` algorithm for native and canonical input backends.
///
/// # Safety
/// `this` must be a live `UInput`; `cmd` must be NUL-terminated.
pub unsafe fn exec_input_commands_with<E>(
    this: *mut UInput,
    mut cmd: *const u32,
    mut parse_line: impl FnMut(*mut *const u32, *mut u32, i32) -> Result<bool, E>,
    mut parse_command: impl FnMut(*mut *const u32, *const u32) -> Result<bool, E>,
    mut viewport_exec: impl FnMut(*mut c_void, *const u32) -> Result<(), E>,
    mut input_exec: impl FnMut(*mut UInput, *const u32) -> Result<(), E>,
) -> Result<(), E> {
    let mut line = [0u32; TEXT_BUF_LEN as usize];
    while parse_line(&mut cmd, line.as_mut_ptr(), TEXT_BUF_LEN)? {
        let mut cursor = line.as_ptr();
        let action = unsafe { (*this).action };
        if action == IST_PRESS
            || (action == IST_RELEASE && parse_command(&mut cursor, w!("OnRelease"))?)
        {
            let viewport = unsafe { (*this).viewport };
            viewport_exec(viewport, cursor)?;
        } else {
            input_exec(this, cursor)?;
        }
    }
    Ok(())
}

/// `UInput::Init` (`UnIn.cpp:193-205`).
///
/// # Safety
/// `this` must be a live `UInput`; `viewport` a live `UViewport`.
pub unsafe fn init(svc: &Hp2InputServices, this: *mut UInput, viewport: *mut c_void) -> Res<()> {
    unsafe {
        (*this).viewport = viewport;
        checked_unit!((svc.input_reset)(this.cast()));
        checked_unit!((svc.log_initialized)((*this).viewport));
    }

    Ok(())
}

/// `UInput::Exec` (`UnIn.cpp:214-330`). Returns `UBOOL`.
///
/// Branch order is BUTTON, PULSE, TOGGLE, AXIS, KEYNAME, KEYBINDING, then the
/// alias table. The inner native `guard(ExecAlias)` spans a checked continuation.
///
/// # Safety
/// `this` must be a live `UInput`; `s` a NUL-terminated `TCHAR` string; `ar`
/// a live `FOutputDevice`.
pub unsafe fn exec(
    svc: &Hp2InputServices,
    this: *mut UInput,
    mut s: *const u32,
    ar: *mut c_void,
) -> Res<i32> {
    Ok(unsafe {
        let mut temp = [0u32; TEXT_BUF_LEN as usize];
        let temp_ptr = temp.as_mut_ptr();

        if checked_value!((svc.parse_command)(&raw mut s, w!("BUTTON"))) != 0 {
            let actor = (*((*this).viewport as *mut UViewport)).actor;
            let found = !actor.is_null()
                && checked_value!((svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0)) != 0;
            let button = if found {
                checked_value!((svc.input_find_button)(
                    this.cast(),
                    (*((*this).viewport as *mut UViewport)).actor,
                    temp_ptr
                ))
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
                checked_unit!((svc.output_log)(ar, w!("Bad Button command")));
            }
            return Ok(1);
        } else if checked_value!((svc.parse_command)(&raw mut s, w!("PULSE"))) != 0 {
            let actor = (*((*this).viewport as *mut UViewport)).actor;
            let found = !actor.is_null()
                && checked_value!((svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0)) != 0;
            let button = if found {
                checked_value!((svc.input_find_button)(
                    this.cast(),
                    (*((*this).viewport as *mut UViewport)).actor,
                    temp_ptr
                ))
            } else {
                ptr::null_mut()
            };
            if found && !button.is_null() {
                if (*this).action == IST_PRESS {
                    *button = 1;
                }
            } else {
                checked_unit!((svc.output_log)(ar, w!("Bad Button command")));
            }
            return Ok(1);
        } else if checked_value!((svc.parse_command)(&raw mut s, w!("TOGGLE"))) != 0 {
            let actor = (*((*this).viewport as *mut UViewport)).actor;
            let found = !actor.is_null()
                && checked_value!((svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0)) != 0;
            let button = if found {
                checked_value!((svc.input_find_button)(
                    this.cast(),
                    (*((*this).viewport as *mut UViewport)).actor,
                    temp_ptr
                ))
            } else {
                ptr::null_mut()
            };
            if found && !button.is_null() {
                if (*this).action == IST_PRESS {
                    *button ^= 0x80;
                }
            } else {
                checked_unit!((svc.output_log)(ar, w!("Bad Toggle command")));
            }
            return Ok(1);
        } else if checked_value!((svc.parse_command)(&raw mut s, w!("AXIS"))) != 0 {
            let actor = (*((*this).viewport as *mut UViewport)).actor;
            let found = !actor.is_null()
                && checked_value!((svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0)) != 0;
            let axis = if found {
                checked_value!((svc.input_find_axis)(
                    this.cast(),
                    (*((*this).viewport as *mut UViewport)).actor,
                    temp_ptr
                ))
            } else {
                ptr::null_mut()
            };
            if found && !axis.is_null() {
                let mut speed: f32 = 1.0;
                let mut speed_base: f32 = 0.0;
                checked_value!((svc.parse_float)(s, w!("SPEED="), &raw mut speed));
                checked_value!((svc.parse_float)(s, w!("SPEEDBASE="), &raw mut speed_base));
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
                checked_unit!((svc.output_logf)(ar, w!("Bad Axis command")));
            }
            return Ok(1);
        } else if checked_value!((svc.parse_command)(&raw mut s, w!("KEYNAME"))) != 0 {
            let key_no = checked_value!((svc.app_atoi)(s));
            checked_unit!((svc.output_log)(
                ar,
                checked_value!((svc.input_key_name)(this.cast(), key_no))
            ));
            return Ok(1);
        } else if checked_value!((svc.parse_command)(&raw mut s, w!("KEYBINDING"))) != 0 {
            let mut i_key: i32 = 0;
            if checked_value!((svc.input_find_key)(this.cast(), s, &raw mut i_key)) != 0 {
                checked_unit!((svc.output_log)(
                    ar,
                    checked_value!((svc.binding_chars)(this.cast(), i_key))
                ));
            }
            return Ok(1);
        } else if !in_alias()
            && checked_value!((svc.parse_token)(&raw mut s, temp_ptr, TEXT_BUF_LEN, 0)) != 0
        {
            let name = checked_value!((svc.fname_find)(temp_ptr));
            if name != NAME_NONE {
                for i in 0..ALIAS_MAX {
                    if checked_value!((svc.alias_name_index)(this.cast(), i)) == name {
                        exec_alias(svc, this, i, ar)?;
                        return Ok(1);
                    }
                }
            }
        }
        0
    })
}

unsafe fn exec_alias(
    svc: &Hp2InputServices,
    this: *mut UInput,
    index: i32,
    ar: *mut c_void,
) -> Res<()> {
    struct Context {
        svc: *const Hp2InputServices,
        input: *mut UInput,
        index: i32,
        ar: *mut c_void,
    }
    unsafe extern "C" fn run(context: *mut c_void) -> i32 {
        unsafe {
            let c = context.cast::<Context>();
            let svc = &*(*c).svc;
            let result = with_alias_scope(|| -> Res<()> {
                let command = checked_value!((svc.alias_command)((*c).input.cast(), (*c).index));
                checked_unit!((svc.input_exec_commands)(
                    (*c).input.cast(),
                    command,
                    (*c).ar
                ));
                Ok(())
            });
            result.map_or_else(|status| -status, |()| 0)
        }
    }
    let mut context = Context {
        svc,
        input: this,
        index,
        ar,
    };
    unsafe {
        checked_unit!((svc.alias_scope)((&raw mut context).cast(), run));
    }
    Ok(())
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

/// Shared `UInput::Process` binding-action sequence for native and Core owners.
///
/// # Safety
/// `this` must be a live `UInput`.
pub unsafe fn process_binding_with<E>(
    this: *mut UInput,
    state: i32,
    delta: f32,
    has_binding: bool,
    execute: impl FnOnce() -> Result<(), E>,
) -> Result<i32, E> {
    if !has_binding { return Ok(0); }
    unsafe { set_input_action(this, state, delta); }
    execute()?;
    unsafe { set_input_action(this, IST_NONE, 0.0); }
    Ok(1)
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
) -> Res<i32> {
    unsafe {
        check(svc, (0..IK_MAX).contains(&key), c"iKey>=0&&iKey<IK_MAX", 366)?;
        process_binding_with(
            this,
            state,
            delta,
            checked_value!((svc.binding_len)(this.cast(), key)) != 0,
            || {
                checked_unit!((svc.input_exec_commands)(
                    this.cast(),
                    checked_value!((svc.binding_chars)(this.cast(), key)),
                    ar
                ));
                Ok(())
            },
        )
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
) -> Res<()> {
    struct AxisContext {
        svc: *const Hp2InputServices,
        input: *mut UInput,
        delta: f32,
    }
    unsafe extern "C" fn run(context: *mut c_void, command: *const u32) -> i32 {
        unsafe {
            let context = context.cast::<AxisContext>();
            let svc = &*(*context).svc;
            let this = (*context).input;
            let result = (|| -> Res<()> {
                set_input_action(this, IST_HOLD, (*context).delta);
                checked_unit!((svc.input_exec_commands)(
                    this.cast(),
                    command,
                    checked_value!((svc.glog)())
                ));
                set_input_action(this, IST_NONE, 0.0);
                Ok(())
            })();
            result.map_or_else(|status| -status, |()| 0)
        }
    }
    let mut context = AxisContext {
        svc,
        input: this,
        delta,
    };
    unsafe {
        checked_unit!((svc.direct_axis_scope)(
            this.cast(),
            key,
            length,
            (&raw mut context).cast(),
            run
        ));
    }
    Ok(())
}

/// `UInput::ReadInput` (`UnIn.cpp:399-424`).
///
/// # Safety
/// `this` must be a live `UInput` with a live `Viewport` and `Actor`.
pub unsafe fn read_input(svc: &Hp2InputServices, this: *mut UInput, delta_seconds: f32) -> Res<()> {
    unsafe {
        if checked_value!((svc.is_running)()) == 0 {
            return Ok(());
        }
        let mut item: *mut c_void = ptr::null_mut();
        let cache = input_var_cache_get(
            svc,
            (*((*((*this).viewport as *mut UViewport)).actor as *mut UObject)).class,
            &raw mut item,
        )?;
        if delta_seconds != -1.0 {
            for i in 0..IK_MAX {
                if (*this).key_down_table[i as usize] != 0 {
                    checked_value!((svc.input_process)(
                        this.cast(),
                        checked_value!((svc.glog)()),
                        i,
                        IST_HOLD,
                        delta_seconds
                    ));
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
            if checked_value!((svc.property_is_float)(prop)) != 0 {
                let offset = (*(prop as *mut UProperty)).offset;
                let axis = (*((*this).viewport as *mut UViewport)).actor
                    .cast::<u8>()
                    .offset(isize::try_from(offset).unwrap_or(0))
                    .cast::<f32>();
                *axis *= scale;
            }
            i += 1;
        }
        checked_unit!((svc.cache_unlock)(item));
    }

    Ok(())
}

/// `UInput::ResetInput` (`UnIn.cpp:433-459`).
///
/// # Safety
/// `this` must be a live `UInput` with a live `Viewport` and `Actor`.
pub unsafe fn reset_input(svc: &Hp2InputServices, this: *mut UInput) -> Res<()> {
    unsafe {
        check(svc, !(*this).viewport.is_null(), c"Viewport", 436)?;
        reset_input_with(
            this,
            |_viewport, actor, kind| {
                let klass = (*actor.cast::<UObject>()).class;
                for_each_property(svc, klass, kind, |prop| {
                    let property = &*prop.cast::<UProperty>();
                    if property.property_flags & CPF_INPUT != 0 {
                        let value = actor.cast::<u8>().offset(property.offset as isize);
                        if kind == KIND_BYTE {
                            value.write(0);
                        } else {
                            value.cast::<f32>().write(0.0);
                        }
                    }
                    Ok(())
                })?;
                Ok(())
            },
            |viewport, reset| {
                checked_unit!((svc.viewport_update_input)(viewport, reset));
                Ok(())
            },
        )
    }
}

/// Shared ordering for canonical and native adapters of `UInput::ResetInput`.
///
/// # Safety
/// `this` must be a live `UInput`; callbacks must access only live objects and
/// implement property reset and viewport update for that same owner.
pub unsafe fn reset_input_with<E>(
    this: *mut UInput,
    mut reset_properties: impl FnMut(*mut c_void, *mut c_void, i32) -> Result<(), E>,
    mut update_input: impl FnMut(*mut c_void, i32) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let viewport = (*this).viewport;
        let actor = (*(viewport.cast::<UViewport>())).actor;
        (*this).key_down_table.fill(0);
        reset_properties(viewport, actor, KIND_BYTE)?;
        reset_properties(viewport, actor, KIND_FLOAT)?;
        set_input_action(this, IST_NONE, 0.0);
        update_input(viewport, 1)?;
    }
    Ok(())
}

/// `UInput::GetKeyName` (`UnIn.cpp:492-500`). Returns the name after the
/// `IK_` prefix, or an empty string.
///
/// # Safety
/// `this` must be a live `UInput`.
pub unsafe fn get_key_name(svc: &Hp2InputServices, this: *mut UInput, key: i32) -> Res<*const u32> {
    Ok(unsafe {
        if (0..IK_MAX).contains(&key) {
            let name = checked_value!((svc.key_name_chars)((*this).input_keys, key));
            if checked_value!((svc.app_strlen)(name)) > 3 {
                return Ok(name.add(3));
            }
        }
        w!("")
    })
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
) -> Res<i32> {
    Ok(unsafe {
        let mut temp = [0u32; TEXT_BUF_LEN as usize];
        checked_unit!((svc.app_sprintf_ik)(temp.as_mut_ptr(), key_name));
        checked_value!((svc.key_name_find_item)(
            (*this).input_keys,
            temp.as_ptr(),
            i_key
        ))
    })
}

/// `AActor::execGetCurrentKeyState` body (`UnIn.cpp:465-483`): returns the
/// `UBOOL` the shell copies into `Result`.
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn current_key_state(svc: &Hp2InputServices, actor: *mut c_void, key: u8) -> Res<i32> {
    unsafe { current_key_state_with(key, || Ok(checked_value!((svc.actor_viewport)(actor)).cast())) }
}

/// Shares the key bound, viewport/input gates and normalized boolean read.
///
/// # Safety
/// The provider returns a live canonical or native viewport for the receiver.
pub(crate) unsafe fn current_key_state_with<E>(
    key: u8, viewport: impl FnOnce() -> Result<*mut UViewport, E>,
) -> Result<i32, E> {
    unsafe {
        if i32::from(key) >= IK_MAX { return Ok(0); }
        let viewport = viewport()?;
        if viewport.is_null() { return Ok(0); }
        let input = (*viewport).input;
        Ok(if input.is_null() { 0 } else { i32::from(key_down(input.cast(), i32::from(key)) != 0) })
    }
}

/// `UInput::KeyDown(int)` (`UInput.h`): `KeyDownTable[Clamp(i, 0, IK_MAX-1)]`.
///
/// # Safety
/// `this` must be a live `UInput`.
unsafe fn key_down(this: *mut UInput, i: i32) -> u8 {
    unsafe { (*this).key_down_table[i.clamp(0, IK_MAX - 1) as usize] }
}
