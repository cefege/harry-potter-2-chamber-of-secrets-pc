//! Shared engine reflection/object-graph leaf table used by the trace
//! reporters in this directory (`shadow_admission`, `actor_transition_ledger`,
//! `creature_generator`, `global_tick`, `actor_slot_dump`). Everything these
//! reporters read through `GetPathName`, `GetName`, `GetClass`, `FindField`,
//! property offsets/masks, `FName` text and `ULevel::Actors` indexing goes
//! through [`Hp2TraceReporterServices`], filled in once by
//! `HP2TraceReporterServices.cpp`. Mirrors the established
//! `crate::reporters::*`/`hp2_engine::pawn_native_input` pattern: the C++
//! side only resolves pointers/offsets/NUL-terminated ANSI text into a
//! process-static buffer (valid until the next call on the same leaf, single
//! game thread only); every byte copy, JSON shape and state transition is
//! Rust's.
//!
//! `FireTextureTraceReporter`'s services are separate
//! ([`super::fire_texture`]): that reporter drives real engine virtual calls
//! (`Lock`/`Update`/`Tick`/`ConstantTimeTick`/`Unlock`) rather than reading
//! reflection data, so it is published and consumed independently.
//!
//! Also hosts [`write_report`], the `fopen(tmp)`/`fwrite`/`fclose`/`rename`
//! atomic-publish sequence every reporter's `Flush()` shares (distinct from
//! `crate::atomic`'s `stage`/`publish`, which uses a random-nonce temp name
//! and `&str` paths; these reporters need the exact `<path>.tmp` suffix and
//! raw-byte paths the original C++ uses).

use std::ffi::{CStr, c_char, c_void};
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

/// Engine reflection services shared by the five object-graph trace
/// reporters. Every pointer-returning leaf returns null on a missing/absent
/// result exactly like the native `FindField`/`Cast`/field read it replaces.
/// ANSI text leaves return a pointer into a process-static buffer owned by
/// the C++ side, live only until the next call to that same leaf on the
/// single game thread; callers copy it into an owned `String` immediately.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TraceReporterServices {
    /// `Text(Object->GetPathName())` for any `UObject*` (actors, classes,
    /// decals, functions, ...).
    pub object_path_ansi: unsafe extern "C" fn(obj: *mut c_void) -> *const c_char,
    /// `Text(Object->GetName())`.
    pub object_name_ansi: unsafe extern "C" fn(obj: *mut c_void) -> *const c_char,
    /// `UObject::GetClass()`.
    pub object_class: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    /// `Name == NAME_None ? "" : Text(*Name)` for an `FName`'s raw index.
    pub fname_ansi_or_empty: unsafe extern "C" fn(index: i32) -> *const c_char,
    /// `Actor->Tag.GetIndex()` (the raw `FName` index to pass to
    /// `fname_ansi_or_empty`).
    pub actor_tag_fname: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Actor->Shadow` (an `ADecal*`).
    pub actor_shadow: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `Actor->XLevel`.
    pub actor_x_level: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `Actor->Owner`.
    pub actor_owner: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `Actor->bDeleteMe`. Returns `UBOOL`.
    pub actor_delete_marked: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Actor->IsPendingKill()`. Returns `UBOOL`.
    pub actor_pending_kill: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `(unsigned long long)Actor->bTicked`.
    pub actor_ticked: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Actor->bGameRelevant`. Returns `UBOOL`.
    pub actor_game_relevant: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `(INT)Actor->Role`.
    pub actor_role: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `(INT)Actor->RemoteRole`.
    pub actor_remote_role: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    /// `Level->Actors.Num()`.
    pub level_actor_count: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `Level->Actors(Slot)`.
    pub level_actor_at: unsafe extern "C" fn(level: *mut c_void, slot: i32) -> *mut c_void,
    /// `Level->iFirstDynamicActor`.
    pub level_first_dynamic_actor: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `Level->iFirstNetRelevantActor`.
    pub level_first_net_relevant_actor: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `Cast<UGameEngine>(Engine) ? GameEngine->GLevel : NULL`.
    pub game_engine_level: unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void,
    /// `UClass::GetSuperClass()`.
    pub class_super: unsafe extern "C" fn(klass: *mut c_void) -> *mut c_void,
    /// `UClass::GetDefaultActor()`.
    pub class_default_actor: unsafe extern "C" fn(klass: *mut c_void) -> *mut c_void,
    /// `Cast<UClass>(Object)`, null when `Object` is not a `UClass`.
    pub cast_to_class: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    /// `Cast<AActor>(Object)`, null when `Object` is not an `AActor`.
    pub cast_to_actor: unsafe extern "C" fn(obj: *mut c_void) -> *mut c_void,
    /// `FindField<UProperty>(Class, Name)` (untyped).
    pub find_property:
        unsafe extern "C" fn(klass: *mut c_void, name: *const c_char) -> *mut c_void,
    /// `Cast<UBoolProperty>(FindField<UProperty>(Class, Name))`.
    pub find_bool_property:
        unsafe extern "C" fn(klass: *mut c_void, name: *const c_char) -> *mut c_void,
    /// `Cast<UIntProperty>(FindField<UProperty>(Class, Name))`.
    pub find_int_property:
        unsafe extern "C" fn(klass: *mut c_void, name: *const c_char) -> *mut c_void,
    /// `Cast<UFloatProperty>(FindField<UProperty>(Class, Name))`.
    pub find_float_property:
        unsafe extern "C" fn(klass: *mut c_void, name: *const c_char) -> *mut c_void,
    /// `UProperty::Offset`.
    pub property_offset: unsafe extern "C" fn(prop: *mut c_void) -> i32,
    /// `UProperty::ArrayDim`.
    pub property_array_dim: unsafe extern "C" fn(prop: *mut c_void) -> i32,
    /// `UProperty::ElementSize`.
    pub property_element_size: unsafe extern "C" fn(prop: *mut c_void) -> i32,
    /// `UBoolProperty::BitMask`.
    pub bool_property_bitmask: unsafe extern "C" fn(prop: *mut c_void) -> u32,
    /// `UProperty::GetOuter()`.
    pub property_outer: unsafe extern "C" fn(prop: *mut c_void) -> *mut c_void,
    /// `appGetRandTraceOrdinal()`.
    pub rand_trace_ordinal: unsafe extern "C" fn() -> u64,
}

static mut SERVICES: Option<Hp2TraceReporterServices> = None;

/// Publish the process-wide reflection services table. Called once from
/// `HP2TraceReporterServices.cpp`'s static initializer, mirroring
/// `hp2_engine::route::publish_services`.
///
/// # Safety
/// `value` is the real, immutable services table; publication happens once,
/// before any trace reporter is constructed, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2TraceReporterServices) {
    unsafe { (&raw mut SERVICES).write(Some(value)) }
}

/// The table stored by [`publish_services`]; `Err(1)` before publication.
///
/// # Safety
/// No concurrent [`publish_services`] call.
pub unsafe fn published_services() -> Result<Hp2TraceReporterServices, i32> {
    unsafe { (&raw const SERVICES).read().ok_or(1) }
}

/// Copy a live NUL-terminated ANSI leaf result into owned bytes. Empty when
/// `ptr` is null, matching every `Text()`/`*Ansi ? Ansi : ""` helper in the
/// ported C++ reporters. Kept as raw bytes, not `String`: `appToAnsi` is a
/// lossy single-byte charset conversion, and JSON-escaping must reproduce
/// those exact bytes, not a UTF-8 substitution.
///
/// # Safety
/// `ptr` is either null or a live NUL-terminated C string for the duration
/// of this call (the leaf's "static buffer until next call" contract).
pub(crate) unsafe fn ansi(ptr: *const c_char) -> Vec<u8> {
    if ptr.is_null() {
        Vec::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec()
    }
}

/// `Object ? Text(Object->GetPathName()) : ""`.
///
/// # Safety
/// `svc` is published; `obj` is null or a live `UObject*`.
pub(crate) unsafe fn object_path(svc: &Hp2TraceReporterServices, obj: *mut c_void) -> Vec<u8> {
    if obj.is_null() {
        Vec::new()
    } else {
        unsafe { ansi((svc.object_path_ansi)(obj)) }
    }
}

/// `Object ? Text(Object->GetName()) : ""`.
///
/// # Safety
/// `svc` is published; `obj` is null or a live `UObject*`.
pub(crate) unsafe fn object_name(svc: &Hp2TraceReporterServices, obj: *mut c_void) -> Vec<u8> {
    if obj.is_null() {
        Vec::new()
    } else {
        unsafe { ansi((svc.object_name_ansi)(obj)) }
    }
}

/// `Escape()`: backslash-escapes `\`, `"`, `\n`, `\r`, `\t`; every other
/// byte (including non-ASCII) passes through unchanged, matching the
/// per-byte `switch` in every ported reporter's `Escape` helper.
pub(crate) fn escape_json(value: &[u8], out: &mut Vec<u8>) {
    for &byte in value {
        match byte {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            other => out.push(other),
        }
    }
}

/// `Value.empty() ? "null" : "\"" + Escape(Value) + "\""`.
pub(crate) fn push_json_string_or_null(value: &[u8], out: &mut Vec<u8>) {
    if value.is_empty() {
        out.extend_from_slice(b"null");
    } else {
        out.push(b'"');
        escape_json(value, out);
        out.push(b'"');
    }
}

/// `"\"" + Escape(Value) + "\""` (unconditional, for fields that are always
/// quoted strings even when empty).
pub(crate) fn push_json_string(value: &[u8], out: &mut Vec<u8>) {
    out.push(b'"');
    escape_json(value, out);
    out.push(b'"');
}

/// `Value ? "true" : "false"`.
pub(crate) fn push_json_bool(value: bool, out: &mut Vec<u8>) {
    out.extend_from_slice(if value { b"true" } else { b"false" });
}

/// `*(T*)((BYTE*)Object+Field->Offset)` for a 4-byte scalar/pointer field at
/// `offset` from `obj`. `obj` and `offset` come from a live property lookup
/// on the exact same object, so the read is in-bounds.
///
/// # Safety
/// `obj` is a live object pointer whose class actually has a property at
/// `offset` with a 4-byte representation (pointer, `INT`, `FLOAT`, or the
/// `DWORD` word backing a `UBoolProperty`'s bitfield).
pub(crate) unsafe fn read_u32_at(obj: *mut c_void, offset: i32) -> u32 {
    unsafe { (obj.byte_add(offset as usize) as *const u32).read_unaligned() }
}

/// `*(UObject**)((BYTE*)Object+Field->Offset)`.
///
/// # Safety
/// Same as [`read_u32_at`].
pub(crate) unsafe fn read_ptr_at(obj: *mut c_void, offset: i32) -> *mut c_void {
    unsafe { (obj.byte_add(offset as usize) as *const *mut c_void).read_unaligned() }
}

/// `*(FLOAT*)((BYTE*)Object+Field->Offset)`.
///
/// # Safety
/// Same as [`read_u32_at`].
pub(crate) unsafe fn read_f32_at(obj: *mut c_void, offset: i32) -> f32 {
    unsafe { (obj.byte_add(offset as usize) as *const f32).read_unaligned() }
}

/// `*(INT*)((BYTE*)Object+Field->Offset)`.
///
/// # Safety
/// Same as [`read_u32_at`].
pub(crate) unsafe fn read_i32_at(obj: *mut c_void, offset: i32) -> i32 {
    unsafe { read_u32_at(obj, offset) as i32 }
}

/// `std::to_string(FLOAT)`: equivalent to `sprintf(buf, "%f", value)`,
/// fixed-point with exactly 6 digits after the decimal point. Rust's `{}`
/// `f32` `Display` uses the shortest round-tripping representation instead
/// (e.g. `"1.5"`, not `"1.500000"`), so every ported `std::to_string(FLOAT)`
/// call site must go through this, not a bare `{}`/`.to_string()`.
pub(crate) fn push_cpp_float(value: f32, out: &mut Vec<u8>) {
    if value.is_nan() {
        out.extend_from_slice(if value.is_sign_negative() {
            b"-nan"
        } else {
            b"nan"
        });
    } else if value.is_infinite() {
        out.extend_from_slice(if value.is_sign_negative() {
            b"-inf"
        } else {
            b"inf"
        });
    } else {
        out.extend_from_slice(format!("{value:.6}").as_bytes());
    }
}

/// `UClass::IsChildOf`: walks `GetSuperClass()` starting at `class` itself,
/// looking for `base`.
///
/// # Safety
/// `svc` is published; `class` and `base` are null or live `UClass*`.
pub(crate) unsafe fn class_is_child_of(
    svc: &Hp2TraceReporterServices,
    class: *mut c_void,
    base: *mut c_void,
) -> bool {
    let mut current = class;
    while !current.is_null() {
        if current == base {
            return true;
        }
        current = unsafe { (svc.class_super)(current) };
    }
    false
}

/// Why [`write_report`] failed, selecting one of the two
/// `<TAG> report_error=<reason> path=<path>` stderr lines every reporter's
/// `Flush()` prints.
#[derive(Debug)]
pub(crate) enum ReportWriteError {
    /// `fopen(tmp, "wb")` failed: `report_error=unwritable`.
    Unwritable,
    /// The write, close, or rename failed after the tmp file was created
    /// (which is then removed): `report_error=write_failed`.
    WriteFailed,
}

/// `FILE* Report = fopen((ReportPath+".tmp").c_str(), "wb"); fwrite(...);
/// fclose(...); rename(tmp, ReportPath)`, exactly as every reporter's
/// `Flush()` does it: a failed `rename`/short write removes the tmp file
/// first.
pub(crate) fn write_report(report_path: &[u8], document: &[u8]) -> Result<(), ReportWriteError> {
    let mut tmp_path = report_path.to_vec();
    tmp_path.extend_from_slice(b".tmp");
    let tmp = std::ffi::OsStr::from_bytes(&tmp_path);
    let target = std::ffi::OsStr::from_bytes(report_path);
    let mut file = match fs::File::create(tmp) {
        Ok(file) => file,
        Err(_) => return Err(ReportWriteError::Unwritable),
    };
    let write_result = file.write_all(document);
    drop(file);
    let result = write_result.and_then(|()| fs::rename(tmp, target));
    if result.is_err() {
        let _ = fs::remove_file(tmp);
        return Err(ReportWriteError::WriteFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ground truth: every ported reporter's `Escape()`, e.g.
    // `ShadowAdmissionTraceReporter::Escape` (SDLLaunch.cpp, deleted, formerly
    // ~3891-3904): `case '\\': Escaped += "\\\\"; break; case '"': Escaped +=
    // "\\\""; break; case '\n': Escaped += "\\n"; break; case '\r': Escaped +=
    // "\\r"; break; case '\t': Escaped += "\\t"; break; default: Escaped +=
    // *It; break;` — a per-byte switch, so non-ASCII bytes pass through
    // unchanged rather than being substituted/rejected.
    #[test]
    fn escape_json_matches_cpp_escape_switch() {
        let mut out = Vec::new();
        escape_json(b"a\\b\"c\nd\re\tf", &mut out);
        assert_eq!(out, b"a\\\\b\\\"c\\nd\\re\\tf");
    }

    #[test]
    fn escape_json_passes_non_ascii_bytes_through_unchanged() {
        let mut out = Vec::new();
        escape_json(b"caf\xc3\xa9", &mut out);
        assert_eq!(out, b"caf\xc3\xa9");
    }

    #[test]
    fn escape_json_of_empty_input_is_empty() {
        let mut out = Vec::new();
        escape_json(b"", &mut out);
        assert!(out.is_empty());
    }

    // Ground truth: `JsonStringOrNull`, e.g.
    // `ActorTransitionLedger::JsonStringOrNull` (SDLLaunch.cpp, deleted,
    // formerly ~4119-4121): `Value.empty() ? "null" : "\"" + Escape(Value) +
    // "\""`.
    #[test]
    fn push_json_string_or_null_of_empty_is_null() {
        let mut out = Vec::new();
        push_json_string_or_null(b"", &mut out);
        assert_eq!(out, b"null");
    }

    #[test]
    fn push_json_string_or_null_of_non_empty_is_quoted_and_escaped() {
        let mut out = Vec::new();
        push_json_string_or_null(b"a\"b", &mut out);
        assert_eq!(out, b"\"a\\\"b\"");
    }

    // Ground truth: `ShadowAdmissionTraceReporter::SerializeEvent`'s
    // `Sample.CandidateStatus` field (SDLLaunch.cpp, deleted, formerly
    // ~3930): `",\"candidate_status\":\"" + Escape(Sample.CandidateStatus) +
    // "\""` — always quoted, even when empty (unlike `JsonStringOrNull`
    // fields).
    #[test]
    fn push_json_string_is_always_quoted_even_when_empty() {
        let mut out = Vec::new();
        push_json_string(b"", &mut out);
        assert_eq!(out, b"\"\"");
    }

    #[test]
    fn push_json_bool_matches_cpp_ternary() {
        let mut out = Vec::new();
        push_json_bool(true, &mut out);
        push_json_bool(false, &mut out);
        assert_eq!(out, b"truefalse");
    }

    // Ground truth: `std::to_string(FLOAT)` (libstdc++/libc++) is
    // `sprintf("%f", value)`: fixed-point, exactly 6 digits after the point.
    #[test]
    fn push_cpp_float_formats_fixed_point_with_six_digits() {
        let mut out = Vec::new();
        push_cpp_float(1.5_f32, &mut out);
        assert_eq!(out, b"1.500000");
        out.clear();
        push_cpp_float(0.0_f32, &mut out);
        assert_eq!(out, b"0.000000");
        out.clear();
        push_cpp_float(-2.25_f32, &mut out);
        assert_eq!(out, b"-2.250000");
    }

    #[test]
    fn push_cpp_float_formats_nan_and_infinity_like_glibc_printf() {
        let mut out = Vec::new();
        push_cpp_float(f32::NAN, &mut out);
        assert_eq!(out, b"nan");
        out.clear();
        push_cpp_float(-f32::NAN, &mut out);
        assert_eq!(out, b"-nan");
        out.clear();
        push_cpp_float(f32::INFINITY, &mut out);
        assert_eq!(out, b"inf");
        out.clear();
        push_cpp_float(f32::NEG_INFINITY, &mut out);
        assert_eq!(out, b"-inf");
    }

    // Ground truth: every reporter's `Flush()`, e.g.
    // `ShadowAdmissionTraceReporter::Flush` (SDLLaunch.cpp, deleted, formerly
    // ~3840-3853): `fopen((ReportPath+".tmp").c_str(), "wb")`, `fwrite`,
    // `fclose`, `rename(tmp, ReportPath)`; a failed rename/short write
    // removes the tmp file.
    #[test]
    fn write_report_writes_atomically_via_tmp_and_rename() {
        let dir = std::env::temp_dir().join(format!(
            "hp2_trace_services_test_{}_{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("report.json");
        let target_bytes = target.as_os_str().as_bytes().to_vec();
        write_report(&target_bytes, b"{\"ok\":true}\n").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"{\"ok\":true}\n");
        assert!(!dir.join("report.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_report_fails_unwritable_when_the_directory_does_not_exist() {
        let path = std::env::temp_dir().join(format!(
            "hp2_trace_services_missing_dir_{}_{}",
            std::process::id(),
            line!()
        ));
        let bad = path.join("nested").join("report.json");
        let bad_bytes = bad.as_os_str().as_bytes().to_vec();
        assert!(matches!(
            write_report(&bad_bytes, b"data"),
            Err(ReportWriteError::Unwritable)
        ));
    }
}
