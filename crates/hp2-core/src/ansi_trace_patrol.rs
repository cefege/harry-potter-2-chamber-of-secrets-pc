//! Patrol-state and patrol-point tracing: the port of the "Selected patrol-state
//! tracing" and "Selected patrol-point iterator tracing" sections of
//! `UnAnsi.cpp` (oracle 1144-1439).
//!
//! Each selected record appends one JSON line to the file named by
//! `HP2_PATROL_STATE_TRACE` or `HP2_PATROL_POINT_TRACE`. Each file is truncated
//! once per process, on the first call that reaches its init point, and then
//! opened `"ab"` for every record.
//!
//! Quirks kept on purpose:
//! - `appToAnsi` results are copied at once (row 13). The oracle reads the point
//!   iterator's `FirstPatrolPoint` and `Tag` text later, from its 128-slot ring. No
//!   slot is reused within one record, so the copy writes the same bytes.
//! - The state-trace `State` and `StateOwner` filters, and the point-trace
//!   `Actor` filter, are optional. An unset variable is the oracle's empty
//!   buffer, and an over-long value is treated as unset.
//! - A service raise inside a write chain returns with the trace `FILE*` still
//!   open, as the oracle does when a C++ exception unwinds past its `fclose`. The
//!   lock is released, and stdio flushes the buffered bytes at process exit.
//!
//! Engine pointers are not null-checked where the oracle does not check them.
//! The oracle dereferences them unguarded at those sites, so a NULL there is UB
//! in both versions.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::null;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use hp2_abi::engine::generated::FName;

use crate::ansi::{self, Tchar, Threw};
use crate::ansi_trace::{Hp2AnsiTraceServices, PATH_MAX, status};

/// `TCHAR[1024]`: `ExportedValue`, `RequestorFirstPatrolPoint`, and `YieldedTag`.
const TEXT_CAPACITY: usize = 1024;
/// `EOF`.
const EOF: c_int = -1;
/// Services row 5 kind for `UProperty`.
const KIND_UPROPERTY: i32 = 0;
/// Services row 6 kind for `UState`.
const KIND_USTATE: i32 = 1;

/// A NUL-terminated `TEXT("...")` literal, sized for every literal in this file.
type Literal = [Tchar; 32];

/// Builds a [`Literal`] from ASCII text at compile time.
const fn literal(text: &str) -> Literal {
    let bytes = text.as_bytes();
    assert!(bytes.len() < 32, "literal longer than its buffer");
    let mut out = [0; 32];
    let mut i = 0;
    while i < bytes.len() {
        out[i] = bytes[i] as Tchar;
        i += 1;
    }
    out
}

const PATROL: Literal = literal("patrol");
const STATE_IDLE: Literal = literal("stateIdle");
const PATROL_POINT: Literal = literal("PatrolPoint");
const NAVIGATION_POINT: Literal = literal("NavigationPoint");
const FIRST_PATROL_POINT: Literal = literal("FirstPatrolPoint_ObjectName");
const FIRST_PATROL_POINT_LOWER: Literal = literal("firstPatrolPointObjectName");
const TAG: Literal = literal("Tag");
const UNAVAILABLE: Literal = literal("<unavailable>");

/// Opaque libc `FILE`.
type File = c_void;

unsafe extern "C" {
    fn getenv(name: *const c_char) -> *mut c_char;
    fn strcmp(lhs: *const c_char, rhs: *const c_char) -> c_int;
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut File;
    fn fclose(stream: *mut File) -> c_int;
    fn fflush(stream: *mut File) -> c_int;
    fn fputs(text: *const c_char, stream: *mut File) -> c_int;
    fn fputc(ch: c_int, stream: *mut File) -> c_int;
    fn fprintf(stream: *mut File, format: *const c_char, ...) -> c_int;
}

/// Path and filters of `HP2_PATROL_STATE_TRACE*`. A `None` filter is the
/// oracle's empty buffer.
struct StateTrace {
    path: CString,
    state: Option<CString>,
    actor: Option<CString>,
    state_owner: Option<CString>,
}

/// Path and actor filter of `HP2_PATROL_POINT_TRACE*`.
struct PointTrace {
    path: CString,
    actor: Option<CString>,
}

static STATE_TRACE: OnceLock<Option<StateTrace>> = OnceLock::new();
static POINT_TRACE: OnceLock<Option<PointTrace>> = OnceLock::new();
static STATE_WRITE: Mutex<()> = Mutex::new(());
static POINT_WRITE: Mutex<()> = Mutex::new(());

/// Oracle `appGetPatrolStateTraceState` init. `None` means tracing is disabled.
fn init_state_trace() -> Option<StateTrace> {
    let path = env_value(c"HP2_PATROL_STATE_TRACE")?;
    let trace = StateTrace {
        path,
        state: env_value(c"HP2_PATROL_STATE_TRACE_STATE"),
        actor: env_value(c"HP2_PATROL_STATE_TRACE_ACTOR"),
        state_owner: env_value(c"HP2_PATROL_STATE_TRACE_STATE_OWNER"),
    };
    // The oracle truncates the trace here and enables tracing only if that open succeeds.
    let file = open(&trace.path, c"wb")?;
    finish(file, false);
    Some(trace)
}

/// Oracle `appInitPatrolPointTrace`. `None` means tracing is disabled.
fn init_point_trace() -> Option<PointTrace> {
    let path = env_value(c"HP2_PATROL_POINT_TRACE")?;
    let trace = PointTrace {
        path,
        actor: env_value(c"HP2_PATROL_POINT_TRACE_ACTOR"),
    };
    let file = open(&trace.path, c"wb")?;
    finish(file, false);
    Some(trace)
}

fn state_trace() -> Option<&'static StateTrace> {
    STATE_TRACE.get_or_init(init_state_trace).as_ref()
}

fn point_trace() -> Option<&'static PointTrace> {
    POINT_TRACE.get_or_init(init_point_trace).as_ref()
}

/// Oracle copy rule: a variable is kept only when it is non-empty and shorter than `PATH_MAX`.
fn env_value(name: &CStr) -> Option<CString> {
    // SAFETY: `name` is NUL-terminated. `getenv` returns NULL or a string valid until the
    // environment changes, and it is copied before anything else runs.
    let value = unsafe { getenv(name.as_ptr()) };
    if value.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(value) }.to_bytes();
    if bytes.is_empty() || bytes.len() >= PATH_MAX {
        return None;
    }
    CString::new(bytes).ok()
}

/// `fopen(path, mode)`. `None` where the oracle sees NULL.
fn open(path: &CStr, mode: &CStr) -> Option<Out> {
    // SAFETY: both arguments are NUL-terminated.
    let file = unsafe { fopen(path.as_ptr(), mode.as_ptr()) };
    (!file.is_null()).then_some(Out(file))
}

/// Oracle tail: `if (Wrote) fflush(Trace); fclose(Trace);`.
fn finish(out: Out, wrote: bool) {
    // SAFETY: `out` is the live `FILE*` from `open`, and it is closed exactly once here.
    unsafe {
        if wrote {
            fflush(out.0);
        }
        fclose(out.0);
    }
}

/// Oracle `std::lock_guard` over a trace mutex. The oracle has no poisoning, so a
/// poisoned lock is still used.
fn lock(mutex: &'static Mutex<()>) -> MutexGuard<'static, ()> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Writes to the trace `FILE*`. Each method returns the oracle's success test for its libc call.
#[derive(Clone, Copy)]
struct Out(*mut File);

impl Out {
    /// `fputs(text) != EOF`.
    fn lit(self, text: &CStr) -> bool {
        // SAFETY: `self.0` is the open `FILE*`, and `text` is NUL-terminated.
        unsafe { fputs(text.as_ptr(), self.0) != EOF }
    }

    /// `fputc(byte) != EOF`.
    fn byte(self, byte: u8) -> bool {
        // SAFETY: `self.0` is the open `FILE*`.
        unsafe { fputc(c_int::from(byte), self.0) != EOF }
    }

    /// `fprintf("\\u%04x", byte) >= 0`.
    fn unicode_escape(self, byte: u8) -> bool {
        // SAFETY: the format takes one `unsigned int`, which `u32` matches.
        unsafe { fprintf(self.0, c"\\u%04x".as_ptr(), u32::from(byte)) >= 0 }
    }

    /// `fprintf(",\"opcode_offset\":%d,\"access\":", opcode) >= 0`.
    fn opcode_and_access(self, opcode_offset: i32) -> bool {
        // SAFETY: the format takes one `int`.
        unsafe {
            fprintf(
                self.0,
                c",\"opcode_offset\":%d,\"access\":".as_ptr(),
                opcode_offset,
            ) >= 0
        }
    }

    /// The branch record tail, with `true`/`false` strings for the condition and taken fields.
    fn branch(
        self,
        opcode_offset: i32,
        target_offset: i32,
        next_offset: i32,
        condition: bool,
    ) -> bool {
        let (condition_text, taken_text) = if condition {
            (c"true", c"false")
        } else {
            (c"false", c"true")
        };
        // SAFETY: the format takes three `int`s and two `char*`s.
        unsafe {
            fprintf(
                self.0,
                c",\"opcode_offset\":%d,\"target_offset\":%d,\"next_offset\":%d,\"condition\":%s,\"taken\":%s}\n"
                    .as_ptr(),
                opcode_offset,
                target_offset,
                next_offset,
                condition_text.as_ptr(),
                taken_text.as_ptr(),
            ) >= 0
        }
    }

    /// `fprintf(",\"yielded\":{\"order\":%d,\"slot\":%d,\"path\":", order, slot) >= 0`.
    fn yielded_head(self, order: i32, slot: i32) -> bool {
        // SAFETY: the format takes two `int`s.
        unsafe {
            fprintf(
                self.0,
                c",\"yielded\":{\"order\":%d,\"slot\":%d,\"path\":".as_ptr(),
                order,
                slot,
            ) >= 0
        }
    }
}

/// Oracle `appWritePatrolStateTraceString`. NULL writes `null`. Otherwise the value is a
/// quoted JSON string: `"`, `\`, `\b`, `\f`, `\n`, `\r`, and `\t` are escaped, other bytes
/// below 0x20 become `\u%04x`, and the rest are written verbatim.
fn write_string(out: Out, value: *const c_char) -> bool {
    if value.is_null() {
        return out.lit(c"null");
    }
    // SAFETY: non-NULL values come from `appToAnsi` or a config copy, and both are NUL-terminated.
    let bytes = unsafe { CStr::from_ptr(value) }.to_bytes();
    if !out.byte(b'"') {
        return false;
    }
    for &ch in bytes {
        let written = match ch {
            b'"' => out.lit(c"\\\""),
            b'\\' => out.lit(c"\\\\"),
            0x08 => out.lit(c"\\b"),
            0x0c => out.lit(c"\\f"),
            b'\n' => out.lit(c"\\n"),
            b'\r' => out.lit(c"\\r"),
            b'\t' => out.lit(c"\\t"),
            0x00..=0x1f => out.unicode_escape(ch),
            _ => out.byte(ch),
        };
        if !written {
            return false;
        }
    }
    out.byte(b'"')
}

/// Engine handles from the services table (contract §3). Raw reads are documented at the
/// service that performs them. Methods do not check for NULL unless the oracle does.
#[derive(Clone, Copy)]
struct Svc<'a>(&'a Hp2AnsiTraceServices);

impl Svc<'_> {
    /// Row 1, `GetClass()`.
    fn class(self, object: *const c_void) -> *const c_void {
        // SAFETY: row 1 takes a live object.
        unsafe { (self.0.obj_class)(object as _) as *const c_void }
    }

    /// Row 2, `GetOuter()`.
    fn outer(self, object: *const c_void) -> *const c_void {
        // SAFETY: row 2 takes a live object.
        unsafe { (self.0.obj_outer)(object as _) as *const c_void }
    }

    /// Row 3, `GetName()`.
    fn name(self, object: *const c_void) -> *const Tchar {
        // SAFETY: row 3 takes a live object.
        unsafe { (self.0.obj_name)(object as _).cast() }
    }

    /// Row 4, `GetPathName()`. A hook row: the harness prints it. Raises through `Threw`.
    fn path(self, object: *const c_void) -> Result<*const Tchar, Threw> {
        let mut text: *const c_void = null();
        // SAFETY: row 4 takes a live object and writes one `const TCHAR*` through `text`.
        status(unsafe { (self.0.obj_path_name)(object as _, &mut text) })?;
        Ok(text.cast())
    }

    /// Row 5, `FindField<T>(owner, name)`.
    fn find_field(self, owner: *const c_void, name: *const Tchar, kind: i32) -> *const c_void {
        // SAFETY: `owner` is a UStruct and `name` is NUL-terminated.
        unsafe { (self.0.find_field)(owner as _, name.cast(), kind) as *const c_void }
    }

    /// Row 6, `Cast<T>` test. NULL yields `false`.
    fn is_a(self, object: *const c_void, kind: i32) -> bool {
        // SAFETY: row 6 accepts NULL and returns 0 for it.
        unsafe { (self.0.is_a)(object as _, kind) != 0 }
    }

    /// Row 7, `UProperty::Offset`.
    fn prop_offset(self, property: *const c_void) -> i32 {
        // SAFETY: `property` is a live UProperty.
        unsafe { (self.0.prop_offset)(property as _) }
    }

    /// Row 9, `ExportText(0, buf, data, data, PPF_Delimited)`. A hook row. Returns the
    /// oracle's test result. Raises through `Threw`.
    fn export_text(
        self,
        property: *const c_void,
        buffer: *mut Tchar,
        count: i32,
        data: *const c_void,
    ) -> Result<bool, Threw> {
        let mut written: i32 = 0;
        // SAFETY: `buffer` has room for `count` characters, and `data` is the value the oracle
        // passes. The harness writes the test result through `written`.
        status(unsafe {
            (self.0.prop_export_text)(property as _, buffer.cast(), count, data as _, &mut written)
        })?;
        Ok(written != 0)
    }

    /// Row 10, `FFrame::Node`.
    fn frame_node(self, frame: *const c_void) -> *const c_void {
        // SAFETY: `frame` is the live FFrame reference.
        unsafe { (self.0.frame_node)(frame as _) as *const c_void }
    }

    /// Row 13, `appToAnsi`. Returns a ring slot to copy at once. A NULL input gives NULL. A hook
    /// row. Raises through `Threw`.
    fn to_ansi(self, text: *const Tchar) -> Result<*const c_char, Threw> {
        let mut ansi: *const c_char = null();
        // SAFETY: `text` is NUL-terminated or NULL, and row 13 writes one `const ANSICHAR*`
        // through `ansi`.
        status(unsafe { (self.0.to_ansi)(text.cast(), &mut ansi) })?;
        Ok(ansi)
    }

    /// Row 15, `FName::operator*`. Returns the name text.
    fn fname_text(self, name: *const c_void) -> *const Tchar {
        // SAFETY: `name` is a live `FName`.
        unsafe { (self.0.fname_text)(name as _).cast() }
    }

    /// `appToAnsi(obj->GetPathName())`.
    fn path_ansi(self, object: *const c_void) -> Result<*const c_char, Threw> {
        self.to_ansi(self.path(object)?)
    }

    /// `appToAnsi(obj->GetName())`.
    fn name_ansi(self, object: *const c_void) -> Result<*const c_char, Threw> {
        self.to_ansi(self.name(object))
    }

    /// `obj ? appToAnsi(obj->GetPathName()) : NULL`.
    fn path_or_null(self, object: *const c_void) -> Result<*const c_char, Threw> {
        if object.is_null() {
            Ok(null())
        } else {
            self.path_ansi(object)
        }
    }
}

/// `appStricmp(name, literal) == 0`.
fn tchar_eq_ci(name: *const Tchar, literal: &Literal) -> bool {
    // SAFETY: both arguments are NUL-terminated.
    unsafe { ansi::stricmp(name, literal.as_ptr()) == 0 }
}

/// `strcmp(wanted, text) == 0`.
fn ansi_eq(wanted: &CString, text: *const c_char) -> bool {
    // SAFETY: `text` is a NUL-terminated `appToAnsi` result.
    unsafe { strcmp(wanted.as_ptr(), text) == 0 }
}

/// Oracle `appWritePatrolStateTracePrefix`.
fn write_prefix(
    svc: Svc<'_>,
    out: Out,
    kind: &CStr,
    object: *const c_void,
    state: *const c_void,
) -> Result<bool, Threw> {
    // Oracle: `State ? State->GetOuter() : NULL`, read before the first write.
    let state_owner = if state.is_null() {
        null()
    } else {
        svc.outer(state)
    };
    Ok(out.lit(c"{\"version\":1,\"kind\":")
        && write_string(out, kind.as_ptr())
        && out.lit(c",\"actor\":")
        && write_string(out, svc.path_or_null(object)?)
        && out.lit(c",\"class\":")
        && write_string(
            out,
            if object.is_null() {
                null()
            } else {
                svc.path_ansi(svc.class(object))?
            },
        )
        && out.lit(c",\"state\":")
        && write_string(out, svc.path_or_null(state)?)
        && out.lit(c",\"state_owner\":")
        && write_string(out, svc.path_or_null(state_owner)?))
}

/// Oracle `appGetPatrolStateTraceState`. `Cast<UState>(Stack.Node)`, then the state-trace
/// filters. `Some` holds the trace and the state when the record is selected.
fn select_state(
    svc: Svc<'_>,
    stack: *const c_void,
    object: *const c_void,
) -> Result<Option<(&'static StateTrace, *const c_void)>, Threw> {
    let node = svc.frame_node(stack);
    let state = if svc.is_a(node, KIND_USTATE) {
        node
    } else {
        null()
    };
    if object.is_null() || state.is_null() {
        return Ok(None);
    }
    let Some(trace) = state_trace() else {
        return Ok(None);
    };

    let state_name = svc.name(state);
    let excluded = match &trace.state {
        Some(wanted) => {
            !ansi_eq(wanted, svc.name_ansi(state)?) && !ansi_eq(wanted, svc.path_ansi(state)?)
        }
        None => !tchar_eq_ci(state_name, &PATROL) && !tchar_eq_ci(state_name, &STATE_IDLE),
    };
    if excluded {
        return Ok(None);
    }
    if let Some(wanted) = &trace.actor
        && !ansi_eq(wanted, svc.name_ansi(object)?)
        && !ansi_eq(wanted, svc.path_ansi(object)?)
    {
        return Ok(None);
    }

    let owner = svc.outer(state);
    let owner_selected = match &trace.state_owner {
        None => true,
        Some(wanted) => {
            !owner.is_null()
                && (ansi_eq(wanted, svc.name_ansi(owner)?)
                    || ansi_eq(wanted, svc.path_ansi(owner)?))
        }
    };
    Ok(owner_selected.then_some((trace, state)))
}

/// Oracle `appRecordPatrolStateTraceProperty`.
///
/// # Safety
/// `services` is the live services table. `stack` is the live FFrame. `object`, `property`,
/// and `value` are engine pointers as the oracle receives them. `access` is NULL or NUL-terminated.
pub unsafe fn record_state_property(
    services: &Hp2AnsiTraceServices,
    stack: *const c_void,
    object: *const c_void,
    property: *const c_void,
    value: *const c_void,
    opcode_offset: i32,
    access: *const c_char,
) -> Result<(), Threw> {
    let svc = Svc(services);
    if property.is_null() || value.is_null() {
        return Ok(());
    }
    let Some((trace, state)) = select_state(svc, stack, object)? else {
        return Ok(());
    };

    // Zeroed, so `ExportedValue[0] == 0` as the oracle sets it.
    let mut exported = [0 as Tchar; TEXT_CAPACITY];
    if !svc.export_text(property, exported.as_mut_ptr(), TEXT_CAPACITY as i32, value)? {
        // SAFETY: `exported` has room for `UNAVAILABLE` (13 characters and a NUL).
        unsafe { ansi::strcpy(exported.as_mut_ptr(), UNAVAILABLE.as_ptr()) };
    }

    let _write = lock(&STATE_WRITE);
    let Some(out) = open(&trace.path, c"ab") else {
        return Ok(());
    };
    let wrote = write_prefix(svc, out, c"property", object, state)?
        && out.opcode_and_access(opcode_offset)
        && write_string(out, access)
        && out.lit(c",\"property\":")
        && write_string(out, svc.name_ansi(property)?)
        && out.lit(c",\"property_owner\":")
        && write_string(out, svc.path_or_null(svc.outer(property))?)
        && out.lit(c",\"property_type\":")
        && write_string(out, svc.name_ansi(svc.class(property))?)
        && out.lit(c",\"value\":")
        && write_string(out, svc.to_ansi(exported.as_ptr())?)
        && out.lit(c"}\n");
    finish(out, wrote);
    Ok(())
}

/// Oracle `appRecordPatrolStateTraceBranch`.
///
/// # Safety
/// Same as [`record_state_property`] for `services`, `stack`, and `object`.
pub unsafe fn record_state_branch(
    services: &Hp2AnsiTraceServices,
    stack: *const c_void,
    object: *const c_void,
    opcode_offset: i32,
    target_offset: i32,
    next_offset: i32,
    condition: i32,
) -> Result<(), Threw> {
    let svc = Svc(services);
    let Some((trace, state)) = select_state(svc, stack, object)? else {
        return Ok(());
    };

    let _write = lock(&STATE_WRITE);
    let Some(out) = open(&trace.path, c"ab") else {
        return Ok(());
    };
    let wrote = write_prefix(svc, out, c"jump_if_not", object, state)?
        && out.branch(opcode_offset, target_offset, next_offset, condition != 0);
    finish(out, wrote);
    Ok(())
}

/// Oracle `appPatrolPointTraceSelected`.
fn selected_point(svc: Svc<'_>, trace: &PointTrace, object: *const c_void) -> Result<bool, Threw> {
    match &trace.actor {
        None => Ok(true),
        Some(wanted) => {
            Ok(ansi_eq(wanted, svc.name_ansi(object)?) || ansi_eq(wanted, svc.path_ansi(object)?))
        }
    }
}

/// Oracle `appPatrolPointTraceProperty`. The `appToAnsi` result is copied at once. `None` is the
/// oracle's NULL.
fn point_property_text(
    svc: Svc<'_>,
    object: *const c_void,
    name: &Literal,
    buffer: &mut [Tchar; TEXT_CAPACITY],
) -> Result<Option<CString>, Threw> {
    let property = if object.is_null() {
        null()
    } else {
        svc.find_field(svc.class(object), name.as_ptr(), KIND_UPROPERTY)
    };
    if property.is_null() {
        return Ok(None);
    }
    // Oracle: `(BYTE*)Object + Property->Offset`, passed as both data and defaults.
    let data = object
        .cast::<u8>()
        .wrapping_offset(svc.prop_offset(property) as isize);
    if !svc.export_text(
        property,
        buffer.as_mut_ptr(),
        TEXT_CAPACITY as i32,
        data.cast(),
    )? {
        return Ok(None);
    }
    let text = svc.to_ansi(buffer.as_ptr())?;
    if text.is_null() {
        return Ok(None);
    }
    // SAFETY: `text` is a NUL-terminated `appToAnsi` result. It is copied before any other conversion.
    Ok(Some(unsafe { CStr::from_ptr(text) }.to_owned()))
}

/// The point recorder needs object/text operations only, not a published native table.
trait PointIteratorOps {
    type Error;
    fn class(&self, object: *const c_void) -> *const c_void;
    fn selected(&self, trace: &PointTrace, object: *const c_void) -> Result<bool, Self::Error>;
    fn requested_point_class(&self, class: *const c_void) -> Result<bool, Self::Error>;
    fn property_text(&self, object: *const c_void, name: &Literal, buffer: &mut [Tchar; TEXT_CAPACITY]) -> Result<Option<CString>, Self::Error>;
    fn path_ansi(&self, object: *const c_void) -> Result<*const c_char, Self::Error>;
    fn name_ansi(&self, object: *const c_void) -> Result<*const c_char, Self::Error>;
    fn tag_ansi(&self, tag: *const c_void) -> Result<*const c_char, Self::Error>;
}

impl PointIteratorOps for Svc<'_> {
    type Error = Threw;
    fn class(&self, object: *const c_void) -> *const c_void { (*self).class(object) }
    fn selected(&self, trace: &PointTrace, object: *const c_void) -> Result<bool, Threw> {
        selected_point(*self, trace, object)
    }
    fn requested_point_class(&self, class: *const c_void) -> Result<bool, Threw> {
        let name = (*self).name(class);
        Ok(tchar_eq_ci(name, &PATROL_POINT) || tchar_eq_ci(name, &NAVIGATION_POINT))
    }
    fn property_text(&self, object: *const c_void, name: &Literal, buffer: &mut [Tchar; TEXT_CAPACITY]) -> Result<Option<CString>, Threw> {
        point_property_text(*self, object, name, buffer)
    }
    fn path_ansi(&self, object: *const c_void) -> Result<*const c_char, Threw> { (*self).path_ansi(object) }
    fn name_ansi(&self, object: *const c_void) -> Result<*const c_char, Threw> { (*self).name_ansi(object) }
    fn tag_ansi(&self, tag: *const c_void) -> Result<*const c_char, Threw> {
        (*self).to_ansi((*self).fname_text(tag))
    }
}

struct DirectPointOps<'a>(crate::core_access::CoreAccess<'a>);

impl DirectPointOps<'_> {
    fn name_text(&self, object: *const c_void) -> crate::core_access::RuntimeResult<Vec<u32>> {
        let object = object.cast::<hp2_abi::engine::generated::UObject>();
        let name = i32::from_ne_bytes(unsafe { std::ptr::addr_of!((*object).name).read() });
        self.0.name_text(name)
    }
    fn to_ansi(&self, mut units: Vec<u32>) -> *const c_char {
        units.push(0);
        // The shared ANSI ring copies these terminated code units synchronously.
        unsafe { ansi::to_ansi_string(units.as_ptr().cast()).cast() }
    }
}

impl PointIteratorOps for DirectPointOps<'_> {
    type Error = crate::core_access::RuntimeError;
    fn class(&self, object: *const c_void) -> *const c_void {
        unsafe { (*object.cast::<hp2_abi::engine::generated::UObject>()).class.cast() }
    }
    fn selected(&self, trace: &PointTrace, object: *const c_void) -> Result<bool, Self::Error> {
        match &trace.actor {
            None => Ok(true),
            Some(wanted) => Ok(ansi_eq(wanted, self.name_ansi(object)?) || ansi_eq(wanted, self.path_ansi(object)?)),
        }
    }
    fn requested_point_class(&self, class: *const c_void) -> Result<bool, Self::Error> {
        let mut name = self.name_text(class)?;
        name.push(0);
        Ok(tchar_eq_ci(name.as_ptr().cast(), &PATROL_POINT) || tchar_eq_ci(name.as_ptr().cast(), &NAVIGATION_POINT))
    }
    fn property_text(&self, object: *const c_void, name: &Literal, _: &mut [Tchar; TEXT_CAPACITY]) -> Result<Option<CString>, Self::Error> {
        use hp2_abi::engine::generated::{UObject, UProperty};
        if object.is_null() { return Ok(None); }
        let length = name.iter().position(|&unit| unit == 0).unwrap_or(name.len());
        let name = self.0.find_name(crate::native_text::code_units(&name[..length]))?;
        let class = self.0.native_class("Core.Property")?;
        let property = unsafe { self.0.find_field(self.class(object).cast_mut().cast(), name, class) }.cast::<UProperty>();
        if property.is_null() { return Ok(None); }
        // Preserve the shipping trace leaf's already-offset Data/Delta, even though
        // full ExportText applies Offset again. Do not silently switch to ExportTextItem.
        let data = unsafe { self.0.property_address(object.cast_mut().cast::<UObject>(), property)? };
        let Some(text) = (unsafe { self.0.property_export_text_optional_at(property, data, data, 0, 2)? }) else {
            return Ok(None);
        };
        let ansi = self.to_ansi(text);
        if ansi.is_null() { return Ok(None); }
        // Retained first-point and tag text must survive subsequent ANSI ring conversions.
        Ok(Some(unsafe { CStr::from_ptr(ansi) }.to_owned()))
    }
    fn path_ansi(&self, object: *const c_void) -> Result<*const c_char, Self::Error> {
        let path = crate::core_reflection::object_path_text_with(object.cast_mut().cast(), &|name| self.0.name_text(name))?;
        Ok(self.to_ansi(path))
    }
    fn name_ansi(&self, object: *const c_void) -> Result<*const c_char, Self::Error> {
        Ok(self.to_ansi(self.name_text(object)?))
    }
    fn tag_ansi(&self, tag: *const c_void) -> Result<*const c_char, Self::Error> {
        let name = unsafe { (*tag.cast::<FName>()).index };
        Ok(self.to_ansi(self.0.name_text(name)?))
    }
}

/// Oracle `appRecordPatrolPointIterator`.
///
/// `match_tag` points to the `FName` the oracle receives as `const FName&`.
///
/// # Safety
/// `services` is the live services table. The object arguments are engine pointers as the
/// oracle receives them. `match_tag` is a live `FName`.
pub unsafe fn record_point_iterator(
    services: &Hp2AnsiTraceServices,
    requestor: *const c_void,
    requested_class: *const c_void,
    match_tag: *const c_void,
    slot: i32,
    yielded: *const c_void,
    order: i32,
) -> Result<(), Threw> {
    record_point_iterator_with(&Svc(services), requestor, requested_class, match_tag, slot, yielded, order)
}

/// Records source patrol iteration directly from the canonical Core graph.
///
/// # Safety
/// Every non-null object belongs to `access` and remains live for synchronous reflection.
pub unsafe fn record_point_iterator_owned(
    access: crate::core_access::CoreAccess<'_>,
    requestor: *mut hp2_abi::engine::generated::UObject,
    requested_class: *mut hp2_abi::engine::generated::UClass,
    match_tag: i32, slot: i32,
    yielded: *mut hp2_abi::engine::generated::UObject, order: i32,
) -> crate::core_access::RuntimeResult<()> {
    let tag = FName { index: match_tag };
    record_point_iterator_with(
        &DirectPointOps(access), requestor.cast(), requested_class.cast(),
        (&raw const tag).cast(), slot, yielded.cast(), order,
    )
}

fn record_point_iterator_with<O: PointIteratorOps>(
    svc: &O, requestor: *const c_void, requested_class: *const c_void,
    match_tag: *const c_void, slot: i32, yielded: *const c_void, order: i32,
) -> Result<(), O::Error> {
    let Some(trace) = point_trace() else { return Ok(()); };
    if requestor.is_null() || requested_class.is_null() || yielded.is_null()
        || !svc.selected(trace, requestor)? {
        return Ok(());
    }
    if !svc.requested_point_class(requested_class)? { return Ok(()); }
    let mut requestor_first = [0 as Tchar; TEXT_CAPACITY];
    let mut first_patrol_point = svc.property_text(requestor, &FIRST_PATROL_POINT, &mut requestor_first)?;
    if first_patrol_point.is_none() {
        first_patrol_point = svc.property_text(requestor, &FIRST_PATROL_POINT_LOWER, &mut requestor_first)?;
    }
    let mut yielded_tag = [0 as Tchar; TEXT_CAPACITY];
    let tag = svc.property_text(yielded, &TAG, &mut yielded_tag)?;
    let _write = lock(&POINT_WRITE);
    let Some(out) = open(&trace.path, c"ab") else { return Ok(()); };
    let match_is_none = unsafe { (*match_tag.cast::<FName>()).index } == 0;
    let first_text = first_patrol_point.as_ref().map_or(null(), |s| s.as_ptr());
    let tag_text = tag.as_ref().map_or(null(), |s| s.as_ptr());
    let wrote = out.lit(c"{\"version\":1,\"kind\":\"all_actors\",\"actor\":")
        && write_string(out, svc.path_ansi(requestor)?)
        && out.lit(c",\"class\":")
        && write_string(out, svc.path_ansi(svc.class(requestor))?)
        && out.lit(c",\"requested_class\":")
        && write_string(out, svc.path_ansi(requested_class)?)
        && out.lit(c",\"match_tag\":")
        && write_string(out, if match_is_none { null() } else { svc.tag_ansi(match_tag)? })
        && out.lit(c",\"firstPatrolPointObjectName\":")
        && write_string(out, first_text)
        && out.yielded_head(order, slot)
        && write_string(out, svc.path_ansi(yielded)?)
        && out.lit(c",\"name\":")
        && write_string(out, svc.name_ansi(yielded)?)
        && out.lit(c",\"tag\":")
        && write_string(out, tag_text)
        && out.lit(c"}}\n");
    finish(out, wrote);
    Ok(())
}
