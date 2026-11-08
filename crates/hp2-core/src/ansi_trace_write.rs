//! Rand-trace JSON writers: stage-2 port of `appWriteRandTraceJsonString`,
//! `appWriteStartupRandTrace`, `appWriteRandTraceNullableString`, and
//! `appWriteRandTrace` (`HarryPotter2/Unreal/Core/Src/UnAnsi.cpp`, git 4f69478,
//! lines 781-1141). Output bytes and the temp-file/rename protocol match the oracle.
//!
//! Divergences, each unreachable or undefined behaviour in the oracle:
//! - The oracle's JSON-string writer dereferences its argument without a NULL check.
//!   Here the JSON-string writer takes a slice and the nullable writer takes an
//!   `Option`, which writes `null` for `None`. The only argument that can be NULL in
//!   the oracle is `EarlyReturnBranch`, written only when `HasFinalResult` is set, and
//!   `appGetGameInfoRelevanceReturnBranch` never returns NULL there.
//! - When `fclose` fails in the startup writer, the oracle calls `fclose` a second
//!   time on the closed `FILE*` (undefined behaviour). This port closes exactly once.
//! - `snprintf(Tmp, PATH_MAX+5, "%s.tmp", path)` becomes a byte copy with the same
//!   length check (`len + 4 < PATH_MAX + 5`). The bytes are identical.
//! - C `%s` stops at the first NUL. TraceCore strips NULs from strings; `c_bytes`
//!   applies the same stop so the output matches even if a NUL slips in.
//! - Raw `%s` fields (startup phase, lifecycle boundary, seed metadata) are written
//!   with `fwrite`; the bytes are the same as `fprintf("%s")`.

use std::ffi::{CStr, c_char, c_int, c_void};

use crate::ansi_trace::{
    FunctionEvent, PATH_MAX, PhaseEvent, PreBeginEvent, RAND_MAX, RelevanceEvent, Sample,
    TickBoundary, TraceState,
};

/// Engine `INDEX_NONE` (`Core/Inc/Core.h:104`).
const INDEX_NONE: i32 = -1;

/// C `EOF`.
const EOF: c_int = -1;

/// `FILE` is opaque here; only pointers cross the libc boundary.
type File = c_void;

unsafe extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut File;
    fn fclose(stream: *mut File) -> c_int;
    fn fflush(stream: *mut File) -> c_int;
    fn fputs(s: *const c_char, stream: *mut File) -> c_int;
    fn fputc(c: c_int, stream: *mut File) -> c_int;
    fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut File) -> usize;
    fn fprintf(stream: *mut File, format: *const c_char, ...) -> c_int;
    fn rename(old: *const c_char, new: *const c_char) -> c_int;
    fn remove(path: *const c_char) -> c_int;
}

/// Writes `GAppRandTraceStartupPhasePath` (`appWriteStartupRandTrace`).
pub fn write_startup_trace(st: &TraceState) -> bool {
    let Some(tmp) = tmp_path(&st.startup_phase_path) else {
        return false;
    };
    let Some(file) = open_tmp(&tmp) else {
        return false;
    };
    let ok = write_startup_body(Out(file), st);
    // Oracle: fclose runs on both paths; the error path ignores its result.
    let close_ok = unsafe { fclose(file) } == 0;
    if !(ok && close_ok) {
        unsafe { remove(cstr(&tmp)) };
        return false;
    }
    let dest = c_path(&st.startup_phase_path);
    unsafe { rename(cstr(&tmp), cstr(&dest)) == 0 }
}

/// Writes `GAppRandTracePath` (`appWriteRandTrace`).
pub fn write_rand_trace(st: &TraceState) -> bool {
    let Some(tmp) = tmp_path(&st.path) else {
        return false;
    };
    let Some(file) = open_tmp(&tmp) else {
        return false;
    };
    let mut ok = write_rand_trace_body(Out(file), st);
    ok = ok && unsafe { fflush(file) } == 0;
    let close_ok = unsafe { fclose(file) } == 0;
    let dest = c_path(&st.path);
    if !ok || !close_ok || unsafe { rename(cstr(&tmp), cstr(&dest)) } != 0 {
        unsafe { remove(cstr(&tmp)) };
        return false;
    }
    true
}

/// C string view: bytes before the first NUL (what `%s` and the JSON loop see).
fn c_bytes(value: &[u8]) -> &[u8] {
    match value.iter().position(|&b| b == 0) {
        Some(end) => &value[..end],
        None => value,
    }
}

/// NUL-terminated copy of `path`.
fn c_path(path: &[u8]) -> Vec<u8> {
    let mut out = c_bytes(path).to_vec();
    out.push(0);
    out
}

/// NUL-terminated `"<path>.tmp"`, or `None` when the oracle's `snprintf` would truncate.
fn tmp_path(path: &[u8]) -> Option<Vec<u8>> {
    let path = c_bytes(path);
    // Oracle: TemporaryLength = len + 4 must be < sizeof(TemporaryPath) = PATH_MAX + 5.
    if path.len() + 4 >= PATH_MAX + 5 {
        return None;
    }
    let mut out = Vec::with_capacity(path.len() + 5);
    out.extend_from_slice(path);
    out.extend_from_slice(b".tmp\0");
    Some(out)
}

fn cstr(path: &[u8]) -> *const c_char {
    path.as_ptr().cast()
}

fn open_tmp(tmp: &[u8]) -> Option<*mut File> {
    // SAFETY: `tmp` is NUL-terminated; the mode is a literal.
    let file = unsafe { fopen(cstr(tmp), c"wb".as_ptr()) };
    (!file.is_null()).then_some(file)
}

/// One open `FILE*` for the duration of a writer. Every method returns the oracle's
/// success predicate for the matching libc call, so `&&` chains short-circuit exactly
/// where the oracle's `WriteSucceeded` flag did.
#[derive(Clone, Copy)]
struct Out(*mut File);

impl Out {
    fn lit(self, s: &CStr) -> bool {
        // SAFETY: `s` is NUL-terminated; the stream is open for the writer's lifetime.
        unsafe { fputs(s.as_ptr(), self.0) != EOF }
    }

    fn raw(self, s: &[u8]) -> bool {
        let s = c_bytes(s);
        // SAFETY: `s` is valid for `s.len()` bytes.
        unsafe { fwrite(s.as_ptr().cast(), 1, s.len(), self.0) == s.len() }
    }

    fn byte(self, b: u8) -> bool {
        // SAFETY: the stream is open for the writer's lifetime.
        unsafe { fputc(c_int::from(b), self.0) != EOF }
    }

    fn num_u64(self, v: u64) -> bool {
        // SAFETY: `%llu` takes an `unsigned long long`, which is `u64` here.
        unsafe { fprintf(self.0, c"%llu".as_ptr(), v) >= 0 }
    }

    fn num_usize(self, v: usize) -> bool {
        // SAFETY: `%lu` takes an `unsigned long`, which is `usize` on LP64.
        unsafe { fprintf(self.0, c"%lu".as_ptr(), v) >= 0 }
    }

    fn num_i32(self, v: i32) -> bool {
        // SAFETY: `%d` takes an `int`.
        unsafe { fprintf(self.0, c"%d".as_ptr(), v) >= 0 }
    }

    fn num_u32(self, v: u32) -> bool {
        // SAFETY: `%u` takes an `unsigned int`.
        unsafe { fprintf(self.0, c"%u".as_ptr(), v) >= 0 }
    }

    fn boolean(self, v: bool) -> bool {
        self.lit(if v { c"true" } else { c"false" })
    }

    /// Oracle `has ? write(...) : fputs("null")`.
    fn if_has(self, has: bool, value: impl FnOnce(Out) -> bool) -> bool {
        if has { value(self) } else { self.lit(c"null") }
    }

    /// Oracle `%s` separator: `Index ? "," : ""`.
    fn sep(self, index: usize) -> bool {
        index == 0 || self.byte(b',')
    }

    /// Oracle `appWriteRandTraceJsonString`, for a non-NULL value.
    fn json(self, value: &[u8]) -> bool {
        let value = c_bytes(value);
        if !self.byte(b'"') {
            return false;
        }
        for &b in value {
            let written = match b {
                b'"' => self.lit(c"\\\""),
                b'\\' => self.lit(c"\\\\"),
                0x08 => self.lit(c"\\b"),
                0x0c => self.lit(c"\\f"),
                b'\n' => self.lit(c"\\n"),
                b'\r' => self.lit(c"\\r"),
                b'\t' => self.lit(c"\\t"),
                // Control characters below 0x20 use `%04x`, as the oracle does.
                0x00..=0x1f => unsafe { fprintf(self.0, c"\\u%04x".as_ptr(), u32::from(b)) >= 0 },
                _ => self.byte(b),
            };
            if !written {
                return false;
            }
        }
        self.byte(b'"')
    }

    /// Oracle `appWriteRandTraceNullableString`: `None` is C NULL and writes `null`.
    fn nullable(self, value: Option<&[u8]>) -> bool {
        match value {
            Some(value) => self.json(value),
            None => self.lit(c"null"),
        }
    }
}

fn seed_metadata(st: &TraceState) -> Vec<u8> {
    if st.replay_seeded {
        b",\"seed\":1,\"start_reason\":\"replay\"".to_vec()
    } else if st.diagnostic_seeded {
        format!(
            ",\"seed\":{},\"requested_seed\":{},\"start_reason\":\"diagnostic\"",
            st.diagnostic_seed, st.diagnostic_requested_seed
        )
        .into_bytes()
    } else {
        Vec::new()
    }
}

fn write_startup_body(o: Out, st: &TraceState) -> bool {
    o.lit(c"{\"version\":2,\"enabled\":true,\"phases\":[")
        && st
            .startup_phases
            .iter()
            .take(st.startup_phase_count)
            .enumerate()
            .all(|(i, e)| write_phase(o, i, e))
        && o.lit(c"]}\n")
}

fn write_phase(o: Out, index: usize, e: &PhaseEvent) -> bool {
    o.sep(index)
        && o.lit(c"{\"phase\":\"")
        && o.raw(&e.phase)
        && o.lit(c"\",\"map_token\":")
        && o.json(&e.map_token)
        && o.lit(c",\"map_url\":")
        && o.json(&e.map_url)
        && o.lit(c",\"load_ordinal\":")
        && o.num_u64(e.load_ordinal)
        && o.lit(c",\"lifecycle_boundary\":\"")
        && o.raw(&e.lifecycle_boundary)
        && o.lit(c"\",\"ordinal_before\":")
        && o.num_u64(e.ordinal_before)
        && o.lit(c",\"ordinal_after\":")
        && o.num_u64(e.ordinal_after)
        // Unsigned subtraction wraps in the oracle as well.
        && o.lit(c",\"delta\":")
        && o.num_u64(e.ordinal_after.wrapping_sub(e.ordinal_before))
        && o.byte(b'}')
}

fn write_rand_trace_body(o: Out, st: &TraceState) -> bool {
    write_header(o, st)
        && write_outputs(o, st)
        && o.lit(c"],\"tick_boundaries\":[")
        && write_tick_boundaries(o, st)
        && o.lit(c"],\"samples\":[")
        && write_samples(o, st)
        && o.lit(c"],\"function_events\":[")
        && write_function_events(o, st)
        && o.lit(c"],\"relevance_events\":[")
        && write_relevance_events(o, st)
        && o.lit(c"],\"prebegin_events\":[")
        && write_prebegin_events(o, st)
        && o.lit(c"]}\n")
}

fn write_header(o: Out, st: &TraceState) -> bool {
    o.lit(c"{\"version\":2,\"enabled\":true,\"rand_max\":")
        && o.num_i32(RAND_MAX)
        && o.lit(c",\"count\":")
        && o.num_usize(st.count)
        && o.lit(c",\"call_count\":")
        && o.num_u64(st.call_count)
        && o.lit(c",\"limit\":")
        && o.num_usize(st.limit)
        && o.lit(c",\"capture_available\":")
        && o.boolean(st.storage_available)
        && o.lit(c",\"truncated\":")
        && o.boolean(st.truncated)
        && o.raw(&seed_metadata(st))
        && o.lit(c",\"outputs\":[")
}

fn write_outputs(o: Out, st: &TraceState) -> bool {
    st.samples.iter().take(st.count).enumerate().all(|(i, s)| {
        o.sep(i)
            && o.lit(c"{\"ordinal\":")
            && o.num_u64(s.ordinal)
            && o.lit(c",\"raw\":")
            && o.num_i32(s.value)
            && o.byte(b'}')
    })
}

fn write_tick_boundaries(o: Out, st: &TraceState) -> bool {
    st.tick_boundaries
        .iter()
        .take(st.tick_boundary_count)
        .enumerate()
        .all(|(i, b)| write_tick_boundary(o, i, b))
}

fn write_tick_boundary(o: Out, index: usize, b: &TickBoundary) -> bool {
    o.sep(index)
        && o.lit(c"{\"index\":")
        && o.num_u64(b.index)
        && o.lit(c",\"phase\":")
        && o.json(&b.phase)
        && o.byte(b'}')
}

fn write_samples(o: Out, st: &TraceState) -> bool {
    st.samples
        .iter()
        .take(st.count)
        .enumerate()
        .all(|(i, s)| write_sample(o, i, s))
}

fn write_sample(o: Out, index: usize, s: &Sample) -> bool {
    o.sep(index)
        && o.lit(c"{\"ordinal\":")
        && o.num_u64(s.ordinal)
        && o.lit(c",\"raw\":")
        && o.num_i32(s.value)
        && o.lit(c",\"tick\":{\"index\":")
        && o.num_u64(s.tick_index)
        && o.lit(c",\"phase\":")
        && o.json(&s.phase)
        && o.lit(c"},\"consumer\":")
        && o.if_has(s.has_consumer, |x| write_native_consumer(x, s))
        && o.lit(c",\"outer_consumer\":")
        && o.if_has(s.has_outer_consumer, |x| write_outer_consumer(x, s))
        && o.byte(b'}')
}

fn write_native_consumer(o: Out, s: &Sample) -> bool {
    o.lit(c"{\"kind\":\"native\",\"actor\":")
        && o.nullable(s.actor.as_deref())
        && o.lit(c",\"class\":")
        && o.nullable(s.class.as_deref())
        && o.lit(c",\"function\":")
        && o.nullable(s.function.as_deref())
        && o.lit(c",\"native\":")
        && o.nullable(s.native.as_deref())
        && o.lit(c",\"native_slot\":")
        && o.num_i32(s.native_slot)
        && o.lit(c",\"callsite_offset\":")
        && o.num_i32(s.callsite_offset)
        && o.byte(b'}')
}

fn write_outer_consumer(o: Out, s: &Sample) -> bool {
    o.lit(c"{\"kind\":\"function\",\"actor\":")
        && o.nullable(s.outer_actor.as_deref())
        && o.lit(c",\"class\":")
        && o.nullable(s.outer_class.as_deref())
        && o.lit(c",\"function\":")
        && o.nullable(s.outer_function.as_deref())
        && o.lit(c",\"native\":null,\"native_slot\":null,\"callsite_offset\":null}")
}

fn write_function_events(o: Out, st: &TraceState) -> bool {
    st.function_events
        .iter()
        .take(st.function_event_count)
        .enumerate()
        .all(|(i, e)| write_function_event(o, i, e))
}

fn write_function_event(o: Out, index: usize, e: &FunctionEvent) -> bool {
    o.sep(index)
        && o.lit(c"{\"call_id\":")
        && o.num_u64(e.call_id)
        && o.lit(c",\"function\":")
        && o.nullable(e.function.as_deref())
        && o.lit(c",\"receiver\":")
        && o.nullable(e.receiver.as_deref())
        && o.lit(c",\"actor\":")
        && o.nullable(e.actor.as_deref())
        && o.lit(c",\"ordinal_at_entry\":")
        && o.num_u64(e.entry_ordinal)
        && o.lit(c",\"ordinal_at_exit\":")
        && o.if_has(e.has_exit, |x| x.num_u64(e.exit_ordinal))
        && o.lit(c",\"root_context\":{\"actor\":")
        && o.nullable(e.root_actor.as_deref())
        && o.lit(c",\"class\":")
        && o.nullable(e.root_class.as_deref())
        && o.lit(c",\"function\":")
        && o.nullable(e.root_function.as_deref())
        && o.lit(c"}}")
}

fn write_relevance_events(o: Out, st: &TraceState) -> bool {
    st.relevance_events
        .iter()
        .take(st.relevance_event_count)
        .enumerate()
        .all(|(i, e)| write_relevance_event(o, i, e))
}

fn write_relevance_event(o: Out, index: usize, e: &RelevanceEvent) -> bool {
    o.sep(index)
        && o.lit(c"{\"function\":")
        && o.nullable(e.function.as_deref())
        && o.lit(c",\"receiver\":")
        && o.nullable(e.receiver.as_deref())
        && o.lit(c",\"other\":{\"actor\":")
        && o.nullable(e.other.as_deref())
        && o.lit(c",\"class\":")
        && o.nullable(e.other_class.as_deref())
        && o.lit(c"},\"base_mutator\":{\"path\":")
        && o.nullable(e.base_mutator.as_deref())
        && o.lit(c",\"class\":")
        && o.nullable(e.base_mutator_class.as_deref())
        && o.lit(c"},\"ordinal_at_entry\":")
        && o.num_u64(e.entry_ordinal)
        && o.lit(c",\"ordinal_at_exit\":")
        && o.if_has(e.has_exit, |x| x.num_u64(e.exit_ordinal))
        && o.lit(c",\"always_keep\":")
        && o.if_has(e.has_always_keep_result, |x| {
            x.boolean(e.always_keep_result)
        })
        && o.lit(c",\"is_relevant\":")
        && o.if_has(e.has_is_relevant_result, |x| x.boolean(e.is_relevant_result))
        && o.lit(c",\"b_super_relevant\":")
        && o.if_has(e.has_super_relevant, |x| {
            x.num_u32(u32::from(e.super_relevant))
        })
        && o.lit(c",\"return_source_line\":")
        && o.if_has(e.return_source_line != INDEX_NONE, |x| {
            x.num_i32(e.return_source_line)
        })
        && o.lit(c",\"early_return_branch\":")
        // The oracle passes this to the JSON-string writer under `HasFinalResult`, where
        // it is never NULL (`appGetGameInfoRelevanceReturnBranch` returns a static
        // string; see module docs). `None` is therefore unreachable here; `nullable`
        // writes `null` only as a defensive fallback.
        && o.if_has(e.has_final_result, |x| {
            x.nullable(e.early_return_branch)
        })
        && o.lit(c",\"result\":")
        && o.if_has(e.has_final_result, |x| x.boolean(e.final_result))
        && o.byte(b'}')
}

fn write_prebegin_events(o: Out, st: &TraceState) -> bool {
    st.prebegin_events
        .iter()
        .take(st.prebegin_event_count)
        .enumerate()
        .all(|(i, e)| write_prebegin_event(o, i, e))
}

fn write_prebegin_event(o: Out, index: usize, e: &PreBeginEvent) -> bool {
    o.sep(index)
        && o.lit(c"{\"actor\":")
        && o.nullable(e.actor.as_deref())
        && o.lit(c",\"function\":")
        && o.nullable(e.function.as_deref())
        && o.lit(c",\"ordinal_at_entry\":")
        && o.num_u64(e.entry_ordinal)
        && o.lit(c",\"ordinal_at_exit\":")
        && o.if_has(e.has_exit, |x| x.num_u64(e.exit_ordinal))
        && o.lit(c",\"b_game_relevant\":")
        && o.boolean(e.game_relevant)
        && o.lit(c",\"b_relevant_in_software_renderer\":")
        && o.boolean(e.relevant_in_software_renderer)
        && o.lit(c",\"b_delete_me_at_entry\":")
        && o.boolean(e.delete_me_at_entry)
        && o.lit(c",\"b_delete_me_at_exit\":")
        && o.boolean(e.delete_me_at_exit)
        && o.lit(c",\"is_software_rendering\":")
        && o.if_has(e.has_software_rendering, |x| {
            x.boolean(e.is_software_rendering)
        })
        && o.lit(c",\"net_mode\":")
        && o.if_has(e.has_net_mode, |x| x.num_i32(e.net_mode))
        && o.lit(c",\"level_game\":")
        && o.if_has(e.has_level_game, |x| x.nullable(e.level_game.as_deref()))
        && o.lit(c",\"relevance_result\":")
        && o.if_has(e.has_relevance_result, |x| x.boolean(e.relevance_result))
        && o.lit(c",\"relevance_ordinal_at_entry\":")
        && o.if_has(e.has_relevance_ordinals, |x| {
            x.num_u64(e.relevance_entry_ordinal)
        })
        && o.lit(c",\"relevance_ordinal_at_exit\":")
        && o.if_has(e.has_relevance_ordinals, |x| {
            x.num_u64(e.relevance_exit_ordinal)
        })
        && o.lit(c",\"relevance_app_rand_consumed\":")
        && o.if_has(e.has_relevance_ordinals, |x| {
            x.boolean(e.relevance_app_rand_consumed)
        })
        && o.lit(c",\"relevance_will_dispatch\":")
        && o.boolean(e.relevance_will_dispatch)
        && o.lit(c",\"relevance_dispatched\":")
        && o.boolean(e.relevance_dispatched)
        && o.lit(c",\"relevance_completed\":")
        && o.boolean(e.relevance_completed)
        && o.byte(b'}')
}
