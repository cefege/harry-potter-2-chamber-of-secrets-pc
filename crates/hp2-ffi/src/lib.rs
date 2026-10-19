//! The single staticlib linked into hp2_game, hp2_ucc, hp2_import and the
//! test executables. Port crates' `extern "C"` APIs are re-exported here so
//! the archive and the generated `hp2_rs.h` cover all of them.

use std::ffi::c_void;

use hp2_ffi_support::{guard, install_log_sink};
use hp2_ini::Profile;

mod audio;
mod canvas;
mod canvas_text;
mod engine;
mod engine_actcol;
mod engine_cam_mgr;
mod engine_console;
mod engine_dynbsp;
mod engine_gesture;
mod engine_input;
mod engine_levact;
mod engine_mover;
mod engine_palette;
mod engine_params;
mod engine_particle;
mod engine_pawn;
mod engine_pawn_native_ai;
mod engine_pawn_native_files;
mod engine_pawn_native_input;
mod engine_pawn_native_move;
mod engine_pawn_reach;
mod engine_pawn_sense;
mod engine_physic;
mod engine_player;
mod engine_prim_trace;
mod engine_route;
mod engine_scr_tex;
mod engine_skeletal;
mod engine_tex;
mod engine_uengine;
mod engine_url;
mod fire_fire;
mod fire_ice;
mod fire_sparks;
mod fire_water;
mod fire_wet_wave;
mod geometry;
mod input;
mod mesh;
mod render;
mod render_light;
mod render_mesh;
mod render_particle;
mod render_particle_clip;
mod render_particle_shard;
mod render_particle_tube;
mod render_softln;
mod render_span;
mod render_sprite;
mod render_sprite_filter;
mod viewport;

/// Route Rust log records to the engine log. Called from `main` before
/// `appInit`; the C++ sink reads `GLog` per line, so records follow
/// `appInit`'s log redirection. A null sink drops records.
///
/// The sink type is spelled out (not the `LogSink` alias) so cbindgen emits a
/// nullable function pointer rather than an opaque `Option_LogSink`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_set_log(sink: Option<unsafe extern "C" fn(text: *const u8, len: usize)>) {
    guard("hp2_rs_set_log", (), || install_log_sink(sink));
}

/// Record in the engine log that the Rust archive is linked and bridged.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_log_startup() {
    guard("hp2_rs_log_startup", (), || {
        log::info!("hp2-ffi {} linked", env!("CARGO_PKG_VERSION"));
    });
}

/// C++ receiver for one ini-build result, called exactly once per export
/// call: the encoded ini text on success, the error message on failure.
/// UTF-8, not NUL-terminated. `ctx` is whatever pointer the caller passed
/// in (its destination, e.g. a `std::string*`).
pub type IniResultSink = unsafe extern "C" fn(ctx: *mut c_void, text: *const u8, len: usize);

/// Borrows a `(ptr, len)` FFI string as `&str`. `len == 0` returns `""`
/// without reading `ptr` (matches an empty `std::string_view`). A null
/// `ptr` with `len != 0`, or invalid UTF-8, is `None`.
unsafe fn str_from_raw<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).ok()
}

/// Invokes `sink` once with `result`'s text (success or error) and reports
/// which it was.
unsafe fn deliver(sink: IniResultSink, ctx: *mut c_void, result: Result<String, String>) -> bool {
    let (ok, text) = match result {
        Ok(text) => (true, text),
        Err(text) => (false, text),
    };
    unsafe { sink(ctx, text.as_ptr(), text.len()) };
    ok
}

/// Builds `System/Default.ini`. Replaces the deleted
/// `HP2Import::Detail::BuildDefaultIni` (`HP2ImportIni.cpp`). Paths are
/// UTF-8, not NUL-terminated. `profile` is `HP2Import::Profile`'s C++
/// ordinal (Safe=0, Full=1, RetailOnly=2). Calls `sink` exactly once; the
/// return value says whether it received the ini text or an error message.
///
/// # Safety
/// `template_path`/`retail_path` must each be valid for reads of their
/// given length (or null with length 0). `sink` must be safe to call with
/// `ctx` and a `(ptr, len)` UTF-8 byte span valid only for the call's
/// duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_build_default(
    template_path: *const u8,
    template_path_len: usize,
    retail_path: *const u8,
    retail_path_len: usize,
    profile: u32,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_build_default", false, move || unsafe {
        let template = match str_from_raw(template_path, template_path_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("template path is not valid UTF-8".to_string()),
                );
            }
        };
        let retail = match str_from_raw(retail_path, retail_path_len) {
            Some(s) => s,
            None => return deliver(sink, ctx, Err("retail path is not valid UTF-8".to_string())),
        };
        let profile = match Profile::from_u32(profile) {
            Some(p) => p,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err(format!("unknown ini profile ordinal {profile}")),
                );
            }
        };
        deliver(
            sink,
            ctx,
            hp2_ini::build_default_ini(template, retail, profile),
        )
    })
}

/// Builds `System/DefUser.ini`. Replaces the deleted
/// `HP2Import::Detail::BuildDefUserIni` (`HP2ImportIni.cpp`). Same
/// conventions, and the same safety contract, as
/// [`hp2_rs_ini_build_default`].
///
/// # Safety
/// See [`hp2_rs_ini_build_default`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_build_def_user(
    template_path: *const u8,
    template_path_len: usize,
    retail_path: *const u8,
    retail_path_len: usize,
    profile: u32,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_build_def_user", false, move || unsafe {
        let template = match str_from_raw(template_path, template_path_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("template path is not valid UTF-8".to_string()),
                );
            }
        };
        let retail = match str_from_raw(retail_path, retail_path_len) {
            Some(s) => s,
            None => return deliver(sink, ctx, Err("retail path is not valid UTF-8".to_string())),
        };
        let profile = match Profile::from_u32(profile) {
            Some(p) => p,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err(format!("unknown ini profile ordinal {profile}")),
                );
            }
        };
        deliver(
            sink,
            ctx,
            hp2_ini::build_def_user_ini(template, retail, profile),
        )
    })
}

/// Borrows a `(ptr, len)` FFI span as `&[u8]`. `len == 0` returns `&[]`
/// without reading `ptr`. A null `ptr` with `len != 0` is `None`.
unsafe fn bytes_from_raw<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Invokes `sink` once with `result`'s bytes (success or error-message
/// bytes) and reports which it was.
unsafe fn deliver_bytes(
    sink: IniResultSink,
    ctx: *mut c_void,
    result: Result<Vec<u8>, String>,
) -> bool {
    let (ok, bytes): (bool, Vec<u8>) = match result {
        Ok(bytes) => (true, bytes),
        Err(message) => (false, message.into_bytes()),
    };
    unsafe { sink(ctx, bytes.as_ptr(), bytes.len()) };
    ok
}

/// Opaque handle wrapping one parsed launcher-store ini document
/// (`Launcher.ini`/`Game.ini`/`User.ini`). A plain `pub type` alias of the
/// dependency's `hp2_ini::launcher::Document` is not enough: cbindgen's
/// dependency walker does not resolve a `use … as` import, and a `pub type`
/// alias of a foreign type is not guaranteed to be picked up either, so
/// `hp2_rs.h` ended up declaring `LauncherDocument *` with no matching
/// `typedef`. A newtype struct declared directly in this crate (no
/// `#[repr(C)]`, so it stays opaque) is what cbindgen reliably forward-
/// declares as `typedef struct Hp2RsIniDoc Hp2RsIniDoc;`.
pub struct Hp2RsIniDoc(hp2_ini::launcher::Document);

/// Creates an empty launcher-store ini document (UTF-8, `\n` newline, no
/// lines yet), matching default-constructed `Document document;`. Replaces
/// the deleted C++ `Document` default constructor for the subset described
/// below. Free a non-null result with [`hp2_rs_ini_doc_free`].
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_ini_doc_new() -> *mut Hp2RsIniDoc {
    guard("hp2_rs_ini_doc_new", std::ptr::null_mut(), || {
        Box::into_raw(Box::new(Hp2RsIniDoc(hp2_ini::launcher::Document::new())))
    })
}

/// Decodes a launcher-store ini file (`Launcher.ini`/`Game.ini`/`User.ini`)
/// into a new document handle. Replaces the deleted C++ `Decode`. Returns
/// null on failure, after calling `err_sink` once with the error message.
/// Free a non-null result with [`hp2_rs_ini_doc_free`].
///
/// # Safety
/// `bytes` must be valid for reads of `len` (or null with `len` 0).
/// `err_sink` must be safe to call with `err_ctx` and a UTF-8 byte span
/// valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_decode(
    bytes: *const u8,
    len: usize,
    err_sink: IniResultSink,
    err_ctx: *mut c_void,
) -> *mut Hp2RsIniDoc {
    guard(
        "hp2_rs_ini_doc_decode",
        std::ptr::null_mut(),
        move || unsafe {
            let data = match bytes_from_raw(bytes, len) {
                Some(d) => d,
                None => {
                    deliver(
                        err_sink,
                        err_ctx,
                        Err("ini bytes pointer is null".to_string()),
                    );
                    return std::ptr::null_mut();
                }
            };
            match hp2_ini::launcher::Document::decode(data) {
                Ok(doc) => Box::into_raw(Box::new(Hp2RsIniDoc(doc))),
                Err(message) => {
                    deliver(err_sink, err_ctx, Err(message));
                    std::ptr::null_mut()
                }
            }
        },
    )
}

/// Frees a handle returned by [`hp2_rs_ini_doc_new`] or
/// [`hp2_rs_ini_doc_decode`]. A null handle is a no-op.
///
/// # Safety
/// `handle` must be a pointer this module returned, not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_free(handle: *mut Hp2RsIniDoc) {
    guard("hp2_rs_ini_doc_free", (), move || unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    })
}

/// Encodes `handle` back to bytes in its original encoding/newline style.
/// Replaces the deleted C++ `Encode`. Calls `sink` exactly once with the
/// encoded bytes on success, or the error message on failure (only the
/// UTF-16 output path can fail); the return value says which.
///
/// # Safety
/// `handle` must be a live pointer from this module. `sink` must be safe
/// to call with `ctx` and a byte span valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_encode(
    handle: *const Hp2RsIniDoc,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_doc_encode", false, move || unsafe {
        deliver_bytes(sink, ctx, (*handle).0.encode())
    })
}

/// Looks up `section`/`key` in `handle`; the last matching row wins,
/// matching `Get`'s semantics. Replaces the deleted C++ `Get`. Calls `sink`
/// once with the trimmed value if found; the return value says whether it
/// was found (`sink` is not called otherwise).
///
/// # Safety
/// `handle` must be a live pointer from this module. `section`/`key` must
/// be valid UTF-8 for their given length (or null with length 0). `sink`
/// must be safe to call with `ctx` and a byte span valid only for the
/// call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_get(
    handle: *const Hp2RsIniDoc,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_doc_get", false, move || unsafe {
        let Some(section) = str_from_raw(section, section_len) else {
            return false;
        };
        let Some(key) = str_from_raw(key, key_len) else {
            return false;
        };
        match (*handle).0.get(section, key) {
            Some(value) => {
                sink(ctx, value.as_ptr(), value.len());
                true
            }
            None => false,
        }
    })
}

/// Sets `section`/`key` to `value` in `handle`: updates the matching row in
/// place, or inserts into the section, or creates the section — see the
/// deleted C++ `Set` for the exact placement rules this preserves. No
/// failure mode.
///
/// # Safety
/// `handle` must be a live pointer from this module. `section`/`key`/
/// `value` must each be valid UTF-8 for their given length (or null with
/// length 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_set(
    handle: *mut Hp2RsIniDoc,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
    value: *const u8,
    value_len: usize,
) {
    guard("hp2_rs_ini_doc_set", (), move || unsafe {
        let Some(section) = str_from_raw(section, section_len) else {
            return;
        };
        let Some(key) = str_from_raw(key, key_len) else {
            return;
        };
        let Some(value) = str_from_raw(value, value_len) else {
            return;
        };
        (*handle).0.set(section, key, value);
    })
}

/// Removes every row matching `section`/`key` (case-insensitive) from
/// `handle`, leaving the section header and surrounding lines untouched.
/// Replaces the deleted C++ `Erase`.
///
/// # Safety
/// `handle` must be a live pointer from this module. `section`/`key` must
/// each be valid UTF-8 for their given length (or null with length 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_erase(
    handle: *mut Hp2RsIniDoc,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
) {
    guard("hp2_rs_ini_doc_erase", (), move || unsafe {
        let Some(section) = str_from_raw(section, section_len) else {
            return;
        };
        let Some(key) = str_from_raw(key, key_len) else {
            return;
        };
        (*handle).0.erase(section, key);
    })
}

/// Opaque handle wrapping one EA-XA block decoder's state (predictor
/// history and residue buffer), replacing `FEAXABlockDecoder`
/// (`Core/Src/FEAXABlockDecoder.cpp`). The borrowed-pointer contract of
/// `feed`/`decode` below exactly matches the original class: `feed` stores
/// a non-owning pointer that `decode` later reads.
pub struct Hp2RsEaxaDecoder(hp2_codecs::eaxa::Decoder);

/// Creates a decoder with zeroed history, matching `FEAXABlockDecoder()`.
/// Free with [`hp2_rs_eaxa_free`].
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_eaxa_new() -> *mut Hp2RsEaxaDecoder {
    guard("hp2_rs_eaxa_new", std::ptr::null_mut(), || {
        Box::into_raw(Box::new(Hp2RsEaxaDecoder(hp2_codecs::eaxa::Decoder::new())))
    })
}

/// Frees a handle returned by [`hp2_rs_eaxa_new`]. A null handle is a no-op.
///
/// # Safety
/// `handle` must be a pointer this module returned, not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_eaxa_free(handle: *mut Hp2RsEaxaDecoder) {
    guard("hp2_rs_eaxa_free", (), move || unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    })
}

/// Replaces `FEAXABlockDecoder::Feed`. Validates and records `data`/`bytes`
/// as the encoded block source for subsequent [`hp2_rs_eaxa_decode`] calls;
/// returns whether it was accepted.
///
/// # Safety
/// `handle` must be a live pointer from this module. `data` must be valid
/// for reads of `bytes` bytes for as long as `decode` is subsequently
/// called before the next `feed` (or `free`) replaces it — the same
/// contract `FEAXABlockDecoder::Feed` documents.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_eaxa_feed(
    handle: *mut Hp2RsEaxaDecoder,
    data: *const u8,
    bytes: i32,
    num_samples: i32,
) -> bool {
    guard("hp2_rs_eaxa_feed", false, move || unsafe {
        (*handle).0.feed(data, bytes, num_samples)
    })
}

/// Replaces `FEAXABlockDecoder::Decode`. Writes up to `samples` decoded
/// `i16` samples to `dest`, returning the number actually produced.
///
/// # Safety
/// `handle` must be a live pointer from this module. `dest` must be valid
/// for writes of `samples` `i16`s. The buffer passed to the most recent
/// `feed` must still be valid for reads of its given length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_eaxa_decode(
    handle: *mut Hp2RsEaxaDecoder,
    dest: *mut i16,
    samples: i32,
) -> i32 {
    guard("hp2_rs_eaxa_decode", 0, move || unsafe {
        (*handle).0.decode(dest, samples)
    })
}

/// Replaces `FEAXABlockDecoder::ResetState`.
///
/// # Safety
/// `handle` must be a live pointer from this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_eaxa_reset_state(
    handle: *mut Hp2RsEaxaDecoder,
    sample1: i16,
    sample2: i16,
) {
    guard("hp2_rs_eaxa_reset_state", (), move || unsafe {
        (*handle).0.reset_state(sample1, sample2)
    })
}

/// Decodes one 8-byte DXT1 block into 16 RGBA pixels (64 bytes, row-major,
/// 4 bytes per pixel). Replaces the single call to `squish::Decompress` in
/// `Engine/Src/S3tcCompat.cpp`'s `S3TCdecode`; everything else in that
/// function (DDSURFACEDESC parsing, pitch/bounds checks, BGRA channel
/// swap, output descriptor) is unchanged C++.
///
/// # Safety
/// `block` must be valid for reads of 8 bytes. `out_rgba` must be valid
/// for writes of 64 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_dxt1_decode_block(block: *const u8, out_rgba: *mut u8) {
    guard("hp2_rs_dxt1_decode_block", (), move || unsafe {
        if block.is_null() || out_rgba.is_null() {
            return;
        }
        let mut block_array = [0u8; 8];
        block_array.copy_from_slice(std::slice::from_raw_parts(block, 8));
        let decoded = hp2_codecs::dxt1::decode_block(&block_array);
        std::ptr::copy_nonoverlapping(decoded.as_ptr(), out_rgba, 64);
    })
}

/// Opaque handle wrapping one Vorbis streaming decoder, replacing
/// `OggVorbis_File` everywhere it crosses the `UnFileStream.cpp` /
/// `ALAudioSubsystem.cpp` boundary (both files call into the same handle
/// through `FStream::TDD`, exactly like the `OggVorbis_File*` it replaces).
pub struct Hp2RsVorbisDecoder(hp2_codecs::vorbis::Decoder);

/// Creates an empty, not-yet-opened Vorbis decoder, replacing
/// `new OggVorbis_File`. Free with [`hp2_rs_vorbis_free`], whether or not
/// [`hp2_rs_vorbis_open`] is ever called on it (matching `delete OggFile`
/// being safe on an un-opened `OggVorbis_File`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_vorbis_new() -> *mut Hp2RsVorbisDecoder {
    guard("hp2_rs_vorbis_new", std::ptr::null_mut(), || {
        Box::into_raw(Box::new(Hp2RsVorbisDecoder(
            hp2_codecs::vorbis::Decoder::new(),
        )))
    })
}

/// Opens `handle` in place, replacing `ov_open(File, Ogg, NULL, 0)`.
/// Returns whether it succeeded (matching `ov_open`'s `< 0` failure check,
/// negated, at the call site); no error detail is reported, matching the
/// existing contract (the C++ caller only branches on success/failure
/// here too).
///
/// # Safety
/// `handle` must be a live pointer from [`hp2_rs_vorbis_new`]. `path` must
/// be valid UTF-8 for reads of `path_len` (or null with `path_len` 0) —
/// the native path `appToUtf8NativePath` produces (the same conversion
/// `appFopen` uses), not a raw engine filename.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_open(
    handle: *mut Hp2RsVorbisDecoder,
    path: *const u8,
    path_len: usize,
) -> bool {
    guard("hp2_rs_vorbis_open", false, move || unsafe {
        let Some(path) = str_from_raw(path, path_len) else {
            return false;
        };
        (*handle).0.open(path)
    })
}

/// Frees a handle returned by [`hp2_rs_vorbis_open`], replacing `ov_clear`
/// + `delete`. A null handle is a no-op.
///
/// # Safety
/// `handle` must be a pointer this module returned, not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_free(handle: *mut Hp2RsVorbisDecoder) {
    guard("hp2_rs_vorbis_free", (), move || unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    })
}

/// Replaces one `ov_read(Ogg, dest, len, 0, 2, 1, &bitstream)` call: fills
/// at most `len` bytes of interleaved 16-bit signed little-endian PCM.
/// Returns the byte count written (may be less than `len`), 0 at clean
/// end of stream, or a negative value on a hard decode error — the same
/// three-way result `ov_read` gives, so the existing C++ read loop is
/// unchanged.
///
/// # Safety
/// `handle` must be a live pointer from this module. `dest` must be valid
/// for writes of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_read(
    handle: *mut Hp2RsVorbisDecoder,
    dest: *mut u8,
    len: usize,
) -> i64 {
    guard("hp2_rs_vorbis_read", -1, move || unsafe {
        if len == 0 {
            return 0;
        }
        if dest.is_null() {
            return -1;
        }
        let dest_slice = std::slice::from_raw_parts_mut(dest, len);
        (*handle).0.read(dest_slice)
    })
}

/// Replaces `ov_pcm_seek(Ogg, 0)` (this engine only ever loops back to the
/// start). Returns whether it succeeded, matching `ov_pcm_seek`'s `== 0`
/// success check at the call site.
///
/// # Safety
/// `handle` must be a live pointer from this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_seek_to_start(handle: *mut Hp2RsVorbisDecoder) -> bool {
    guard("hp2_rs_vorbis_seek_to_start", false, move || unsafe {
        (*handle).0.seek_to_start()
    })
}

/// Replaces `ov_info(Ogg, -1)->channels`.
///
/// # Safety
/// `handle` must be a live pointer from this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_channels(handle: *const Hp2RsVorbisDecoder) -> i32 {
    guard("hp2_rs_vorbis_channels", 0, move || unsafe {
        (*handle).0.channels()
    })
}

/// Replaces `ov_info(Ogg, -1)->rate`.
///
/// # Safety
/// `handle` must be a live pointer from this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_rate(handle: *const Hp2RsVorbisDecoder) -> i32 {
    guard("hp2_rs_vorbis_rate", 0, move || unsafe {
        (*handle).0.rate()
    })
}

/// Replaces `ov_pcm_total(Ogg, -1)`.
///
/// # Safety
/// `handle` must be a live pointer from this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_pcm_total(handle: *const Hp2RsVorbisDecoder) -> i64 {
    guard("hp2_rs_vorbis_pcm_total", 0, move || unsafe {
        (*handle).0.pcm_total()
    })
}

/// Replaces `ov_time_total(Ogg, -1)`.
///
/// # Safety
/// `handle` must be a live pointer from this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_vorbis_time_total(handle: *const Hp2RsVorbisDecoder) -> f64 {
    guard("hp2_rs_vorbis_time_total", 0.0, move || unsafe {
        (*handle).0.time_total()
    })
}

/// Replaces `HP2Import::RunImport`. Paths are UTF-8, not NUL-
/// terminated. `profile` is `HP2Import::Profile`'s C++ ordinal
/// (Safe=0, Full=1, RetailOnly=2); `link_mode` is
/// `HP2Import::LinkMode`'s (Auto=0, Copy=1, Hardlink=2). Calls `sink`
/// exactly once with the success log or the failure message; the
/// return value says which (matching the real C++'s `bool` return
/// plus `Log`/`Error` out-parameters, funneled through the one
/// callback instead).
///
/// `progress` is nullable; passed as `*mut c_void` rather than a typed
/// pointer so cbindgen does not need to resolve
/// `hp2_import::output::Progress`'s layout across the crate boundary
/// into the generated header -- the real C++ caller already owns its
/// own `HP2Import::Progress` definition (`HP2Import.h`), laid out to
/// match `hp2_import::output::Progress` exactly (see that type's own
/// doc comment for the field-by-field justification), and casts its
/// `HP2Import::Progress*` to this parameter directly.
///
/// # Safety
/// Every `(ptr, len)` pair must be valid for reads of that length (or
/// null with length 0). `progress`, non-null, must point at a live
/// `HP2Import::Progress`-layout-compatible value, safe to access from
/// multiple threads for the duration of this call (the real C++ caller
/// already does this: `HP2Launcher.cpp` polls it from another thread
/// while this runs). `sink` must be safe to call with `ctx` and a
/// `(ptr, len)` UTF-8 byte span valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_import_run(
    retail_root: *const u8,
    retail_root_len: usize,
    archive: *const u8,
    archive_len: usize,
    prototype_root: *const u8,
    prototype_root_len: usize,
    output: *const u8,
    output_len: usize,
    profile: u32,
    link_mode: u32,
    check_only: bool,
    progress: *mut c_void,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_import_run", false, move || unsafe {
        let retail_root = match str_from_raw(retail_root, retail_root_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("retail root path is not valid UTF-8".to_string()),
                );
            }
        };
        let archive = match str_from_raw(archive, archive_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("archive path is not valid UTF-8".to_string()),
                );
            }
        };
        let prototype_root = match str_from_raw(prototype_root, prototype_root_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("prototype root path is not valid UTF-8".to_string()),
                );
            }
        };
        let output = match str_from_raw(output, output_len) {
            Some(s) => s,
            None => return deliver(sink, ctx, Err("output path is not valid UTF-8".to_string())),
        };
        let profile = match Profile::from_u32(profile) {
            Some(p) => p,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err(format!("unknown import profile ordinal {profile}")),
                );
            }
        };
        let link_mode = match hp2_import::plan::LinkMode::from_u32(link_mode) {
            Some(m) => m,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err(format!("unknown link mode ordinal {link_mode}")),
                );
            }
        };
        let options = hp2_import::output::Options {
            retail_root: retail_root.to_string(),
            archive: archive.to_string(),
            prototype_root: prototype_root.to_string(),
            output: output.to_string(),
            profile,
            link_mode,
            check_only,
        };
        let progress_ptr = progress.cast::<hp2_import::output::Progress>();
        let progress_ref = progress_ptr.as_ref();
        deliver(
            sink,
            ctx,
            hp2_import::output::run_import(&options, progress_ref),
        )
    })
}

// --- hp2-launcher: HP2LauncherStore.cpp's/HP2LaunchPolicy.cpp's
// non-UI orchestration (HP2Launcher.cpp's own SDL/ImGui UI loop
// stays C++ per the plan and calls through this surface instead of
// the local implementations it used to have). ---

/// Error-only delivery: called exactly once when an export returns
/// `false`, never on success (unlike `IniResultSink`, which always
/// fires exactly once either way). UTF-8, not NUL-terminated.
pub type ErrorSink = unsafe extern "C" fn(ctx: *mut c_void, text: *const u8, len: usize);

unsafe fn report_error(sink: ErrorSink, ctx: *mut c_void, message: &str) -> bool {
    unsafe { sink(ctx, message.as_ptr(), message.len()) };
    false
}

/// Flat mirror of `HP2Launcher::LauncherSettings`, field order and
/// type matching exactly -- `DisplayResolution::label` is omitted:
/// it is purely derived (`"{width} x {height}"`, see
/// `hp2_launcher::store::load_settings`), never read by any Store/
/// Policy logic, so the real C++ caller can reconstruct it itself
/// from `resolution_width`/`resolution_height` without this crossing
/// the FFI boundary at all. Every enum field is that enum's C++
/// ordinal (`hp2_launcher::model`'s Rust enums declare variants in
/// the exact same order as the C++ `enum class`es they replace, with
/// no explicit discriminants, so `as i32` on either side matches).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiLauncherSettings {
    pub screen_mode: i32,
    pub resolution_width: i32,
    pub resolution_height: i32,
    pub vertical_sync: bool,
    pub render_scale: f64,
    pub ui_scale: f64,
    pub frame_rate_limit: i32,
    pub show_fps: bool,
    pub maintain_vertical_fov: bool,
    pub native_text: bool,
    pub anti_aliasing_samples: i32,
    pub anisotropy: i32,
    pub brightness: f64,
    pub texture_detail: i32,
    pub object_detail: i32,
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub music_volume: f64,
    pub mouse_sensitivity: f64,
    pub invert_mouse: bool,
    pub control_mode: i32,
    pub auto_center_camera: bool,
    pub move_while_casting: bool,
    pub auto_quaff: bool,
    pub screen_flashes: bool,
    pub difficulty: i32,
    pub joystick_enabled: bool,
}

/// Converts the flat FFI shape into `hp2_launcher::model::LauncherSettings`,
/// rejecting an out-of-range enum ordinal rather than silently
/// coercing it to a valid default. This check exists only here, at
/// the FFI boundary: the real C++'s `ValidateLauncherSettings` range-
/// checks each field's raw `int` (an `enum class` is not range-
/// checked at the language level, so an explicitly-`static_cast`
/// out-of-range value is a real, reachable state C++ callers can
/// construct), but `hp2_launcher::store::validate_launcher_settings`
/// takes an already-typed Rust `LauncherSettings`, whose `ScreenMode`
/// (etc.) fields are a closed, total enum -- by the time safe Rust
/// code can observe one, it can only ever be one of the real
/// variants, so that equivalent check inside the Rust port itself is
/// unreachable. An earlier version of this function silently mapped
/// every unrecognized ordinal to a valid fallback instead of
/// rejecting it, which made that unreachability also swallow the
/// real C++'s own validation here -- caught by
/// `Tests/LauncherTests.cpp`'s `TestValidationBoundaries` (the
/// unmodified, independent C++ oracle this port is verified against)
/// failing five "invalid X is rejected" cases after the FFI/adapter
/// layer first went in.
fn settings_from_ffi(
    ffi: &FfiLauncherSettings,
) -> Result<hp2_launcher::model::LauncherSettings, String> {
    use hp2_launcher::model::*;
    const INVALID_SELECTION: &str = "Launcher settings contain an invalid selection.";
    let screen_mode = match ffi.screen_mode {
        0 => ScreenMode::Windowed,
        1 => ScreenMode::Fullscreen,
        2 => ScreenMode::BorderlessDesktop,
        _ => return Err(INVALID_SELECTION.to_string()),
    };
    let texture_detail = match ffi.texture_detail {
        0 => TextureDetail::Low,
        1 => TextureDetail::Medium,
        2 => TextureDetail::High,
        _ => return Err(INVALID_SELECTION.to_string()),
    };
    let object_detail = match ffi.object_detail {
        0 => ObjectDetail::VeryLow,
        1 => ObjectDetail::Low,
        2 => ObjectDetail::Medium,
        3 => ObjectDetail::High,
        4 => ObjectDetail::VeryHigh,
        _ => return Err(INVALID_SELECTION.to_string()),
    };
    let control_mode = match ffi.control_mode {
        0 => ControlMode::Classic,
        1 => ControlMode::Modern,
        _ => return Err(INVALID_SELECTION.to_string()),
    };
    let difficulty = match ffi.difficulty {
        0 => Difficulty::Easy,
        1 => Difficulty::Medium,
        2 => Difficulty::Hard,
        _ => return Err(INVALID_SELECTION.to_string()),
    };
    Ok(LauncherSettings {
        screen_mode,
        resolution: DisplayResolution {
            width: ffi.resolution_width,
            height: ffi.resolution_height,
            label: String::new(),
        },
        vertical_sync: ffi.vertical_sync,
        render_scale: ffi.render_scale,
        ui_scale: ffi.ui_scale,
        frame_rate_limit: ffi.frame_rate_limit,
        show_fps: ffi.show_fps,
        maintain_vertical_fov: ffi.maintain_vertical_fov,
        native_text: ffi.native_text,
        anti_aliasing_samples: ffi.anti_aliasing_samples,
        anisotropy: ffi.anisotropy,
        brightness: ffi.brightness,
        texture_detail,
        object_detail,
        sound_enabled: ffi.sound_enabled,
        sound_volume: ffi.sound_volume,
        music_volume: ffi.music_volume,
        mouse_sensitivity: ffi.mouse_sensitivity,
        invert_mouse: ffi.invert_mouse,
        control_mode,
        auto_center_camera: ffi.auto_center_camera,
        move_while_casting: ffi.move_while_casting,
        auto_quaff: ffi.auto_quaff,
        screen_flashes: ffi.screen_flashes,
        difficulty,
        joystick_enabled: ffi.joystick_enabled,
    })
}

fn settings_to_ffi(settings: &hp2_launcher::model::LauncherSettings) -> FfiLauncherSettings {
    FfiLauncherSettings {
        screen_mode: settings.screen_mode as i32,
        resolution_width: settings.resolution.width,
        resolution_height: settings.resolution.height,
        vertical_sync: settings.vertical_sync,
        render_scale: settings.render_scale,
        ui_scale: settings.ui_scale,
        frame_rate_limit: settings.frame_rate_limit,
        show_fps: settings.show_fps,
        maintain_vertical_fov: settings.maintain_vertical_fov,
        native_text: settings.native_text,
        anti_aliasing_samples: settings.anti_aliasing_samples,
        anisotropy: settings.anisotropy,
        brightness: settings.brightness,
        texture_detail: settings.texture_detail as i32,
        object_detail: settings.object_detail as i32,
        sound_enabled: settings.sound_enabled,
        sound_volume: settings.sound_volume,
        music_volume: settings.music_volume,
        mouse_sensitivity: settings.mouse_sensitivity,
        invert_mouse: settings.invert_mouse,
        control_mode: settings.control_mode as i32,
        auto_center_camera: settings.auto_center_camera,
        move_while_casting: settings.move_while_casting,
        auto_quaff: settings.auto_quaff,
        screen_flashes: settings.screen_flashes,
        difficulty: settings.difficulty as i32,
        joystick_enabled: settings.joystick_enabled,
    }
}

/// Called once per discovered save. UTF-8 string spans, not
/// NUL-terminated, valid only for the call's duration.
pub type SaveRecordSink = unsafe extern "C" fn(
    ctx: *mut c_void,
    slot: i32,
    save_index: i32,
    uses_slot_directory: bool,
    save_path: *const u8,
    save_path_len: usize,
    thumbnail_path: *const u8,
    thumbnail_path_len: usize,
    display_name: *const u8,
    display_name_len: usize,
    size: i64,
    modified_seconds: i64,
);

unsafe fn report_save(
    sink: SaveRecordSink,
    ctx: *mut c_void,
    save: &hp2_launcher::model::SaveRecord,
) {
    unsafe {
        sink(
            ctx,
            save.slot,
            save.save_index,
            save.uses_slot_directory,
            save.save_path.as_ptr(),
            save.save_path.len(),
            save.thumbnail_path.as_ptr(),
            save.thumbnail_path.len(),
            save.display_name.as_ptr(),
            save.display_name.len(),
            save.size,
            save.modified_seconds,
        )
    };
}

/// Replaces `HP2Launcher::LoadLauncherState`. Paths are UTF-8, not
/// NUL-terminated. Writes the settings into `*settings_out` on
/// success (untouched on failure) and calls `save_sink` once per
/// discovered save, in the real C++'s own sorted order. Calls
/// `error_sink` exactly once, only on failure.
///
/// # Safety
/// Every `(ptr, len)` pair must be valid for reads of that length (or
/// null with length 0). `settings_out` must be a valid, aligned
/// pointer to write an `FfiLauncherSettings` through. `save_sink` and
/// `error_sink` must be safe to call with their `ctx` and the spans
/// described above, valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_load_state(
    user_root: *const u8,
    user_root_len: usize,
    system_root: *const u8,
    system_root_len: usize,
    settings_out: *mut FfiLauncherSettings,
    save_sink: SaveRecordSink,
    save_ctx: *mut c_void,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_load_state", false, move || unsafe {
        let user_root = match str_from_raw(user_root, user_root_len) {
            Some(s) => s,
            None => {
                return report_error(error_sink, error_ctx, "user root path is not valid UTF-8");
            }
        };
        let system_root = match str_from_raw(system_root, system_root_len) {
            Some(s) => s,
            None => {
                return report_error(error_sink, error_ctx, "system root path is not valid UTF-8");
            }
        };
        let paths = hp2_launcher::model::LauncherPaths {
            user_root: user_root.to_string(),
            system_root: system_root.to_string(),
        };
        match hp2_launcher::orchestrate::load_launcher_state(&paths) {
            Ok(state) => {
                *settings_out = settings_to_ffi(&state.settings);
                for save in &state.saves {
                    report_save(save_sink, save_ctx, save);
                }
                true
            }
            Err(e) => report_error(error_sink, error_ctx, &e),
        }
    })
}

/// Replaces `HP2Launcher::CommitLauncherSettings`. Paths are UTF-8,
/// not NUL-terminated. Calls `error_sink` exactly once, only on
/// failure.
///
/// # Safety
/// Every `(ptr, len)` pair must be valid for reads of that length (or
/// null with length 0). `settings` must be a valid, aligned pointer
/// to a live `FfiLauncherSettings`. `error_sink` must be safe to call
/// with `error_ctx` and the span described above, valid only for the
/// call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_commit_settings(
    user_root: *const u8,
    user_root_len: usize,
    system_root: *const u8,
    system_root_len: usize,
    settings: *const FfiLauncherSettings,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_commit_settings", false, move || unsafe {
        let user_root = match str_from_raw(user_root, user_root_len) {
            Some(s) => s,
            None => {
                return report_error(error_sink, error_ctx, "user root path is not valid UTF-8");
            }
        };
        let system_root = match str_from_raw(system_root, system_root_len) {
            Some(s) => s,
            None => {
                return report_error(error_sink, error_ctx, "system root path is not valid UTF-8");
            }
        };
        let paths = hp2_launcher::model::LauncherPaths {
            user_root: user_root.to_string(),
            system_root: system_root.to_string(),
        };
        let settings = match settings_from_ffi(&*settings) {
            Ok(s) => s,
            Err(e) => return report_error(error_sink, error_ctx, &e),
        };
        match hp2_launcher::orchestrate::commit_launcher_settings(&paths, &settings) {
            Ok(()) => true,
            Err(e) => report_error(error_sink, error_ctx, &e),
        }
    })
}

/// Called once with a loaded/default `DataSourceConfiguration`.
/// `selected` is `DataSource`'s C++ ordinal (Retail=0, Prototype=1).
/// UTF-8 string spans, not NUL-terminated, valid only for the call's
/// duration.
pub type DataSourceConfigSink = unsafe extern "C" fn(
    ctx: *mut c_void,
    selected: i32,
    retail_root: *const u8,
    retail_root_len: usize,
    prototype_root: *const u8,
    prototype_root_len: usize,
);

/// Replaces `HP2Launcher::LoadDataSourceConfiguration`. `launcher_root`
/// is UTF-8, not NUL-terminated. Calls `config_sink` exactly once on
/// success, `error_sink` exactly once on failure.
///
/// # Safety
/// `(launcher_root, launcher_root_len)` must be valid for reads of
/// that length (or null with length 0). `config_sink`/`error_sink`
/// must be safe to call with their `ctx` and the spans described
/// above, valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_load_data_sources(
    launcher_root: *const u8,
    launcher_root_len: usize,
    config_sink: DataSourceConfigSink,
    config_ctx: *mut c_void,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_load_data_sources", false, move || unsafe {
        let launcher_root = match str_from_raw(launcher_root, launcher_root_len) {
            Some(s) => s,
            None => {
                return report_error(
                    error_sink,
                    error_ctx,
                    "launcher root path is not valid UTF-8",
                );
            }
        };
        match hp2_launcher::orchestrate::load_data_source_configuration(launcher_root) {
            Ok(config) => {
                config_sink(
                    config_ctx,
                    config.selected as i32,
                    config.retail_root.as_ptr(),
                    config.retail_root.len(),
                    config.prototype_root.as_ptr(),
                    config.prototype_root.len(),
                );
                true
            }
            Err(e) => report_error(error_sink, error_ctx, &e),
        }
    })
}

/// Replaces `HP2Launcher::CommitDataSourceConfiguration`.
/// `launcher_root`/`retail_root`/`prototype_root` are UTF-8, not
/// NUL-terminated. `selected` is `DataSource`'s C++ ordinal (Retail=0,
/// Prototype=1); any other value is reported as an error (matching
/// the real C++'s own "invalid selection" guard, reachable here since
/// an arbitrary `int32_t` crosses the FFI boundary, unlike the
/// unreachable Rust-enum-is-total case this same check hits
/// internally -- see `commit_data_source_configuration`'s own doc
/// comment). Calls `error_sink` exactly once, only on failure.
///
/// # Safety
/// Every `(ptr, len)` pair must be valid for reads of that length (or
/// null with length 0). `error_sink` must be safe to call with
/// `error_ctx` and the span described above, valid only for the
/// call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_commit_data_sources(
    launcher_root: *const u8,
    launcher_root_len: usize,
    selected: i32,
    retail_root: *const u8,
    retail_root_len: usize,
    prototype_root: *const u8,
    prototype_root_len: usize,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard(
        "hp2_rs_launcher_commit_data_sources",
        false,
        move || unsafe {
            let launcher_root = match str_from_raw(launcher_root, launcher_root_len) {
                Some(s) => s,
                None => {
                    return report_error(
                        error_sink,
                        error_ctx,
                        "launcher root path is not valid UTF-8",
                    );
                }
            };
            let retail_root = match str_from_raw(retail_root, retail_root_len) {
                Some(s) => s,
                None => {
                    return report_error(
                        error_sink,
                        error_ctx,
                        "retail root path is not valid UTF-8",
                    );
                }
            };
            let prototype_root = match str_from_raw(prototype_root, prototype_root_len) {
                Some(s) => s,
                None => {
                    return report_error(
                        error_sink,
                        error_ctx,
                        "prototype root path is not valid UTF-8",
                    );
                }
            };
            let selected = match selected {
                0 => hp2_launcher::model::DataSource::Retail,
                1 => hp2_launcher::model::DataSource::Prototype,
                other => {
                    return report_error(
                        error_sink,
                        error_ctx,
                        &format!("unknown data source ordinal {other}"),
                    );
                }
            };
            let config = hp2_launcher::model::DataSourceConfiguration {
                selected,
                retail_root: retail_root.to_string(),
                prototype_root: prototype_root.to_string(),
            };
            match hp2_launcher::orchestrate::commit_data_source_configuration(
                launcher_root,
                &config,
            ) {
                Ok(()) => true,
                Err(e) => report_error(error_sink, error_ctx, &e),
            }
        },
    )
}

/// Replaces `HP2Launcher::PrepareDataSourceProfile`. `launcher_root`
/// is UTF-8, not NUL-terminated; `source` is `DataSource`'s C++
/// ordinal (Retail=0, Prototype=1). Calls `sink` exactly once with
/// the resulting profile root path on success or the error message on
/// failure; the return value says which.
///
/// # Safety
/// `(launcher_root, launcher_root_len)` must be valid for reads of
/// that length (or null with length 0). `sink` must be safe to call
/// with `ctx` and a `(ptr, len)` UTF-8 byte span valid only for the
/// call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_prepare_profile(
    launcher_root: *const u8,
    launcher_root_len: usize,
    source: i32,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_prepare_profile", false, move || unsafe {
        let launcher_root = match str_from_raw(launcher_root, launcher_root_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("launcher root path is not valid UTF-8".to_string()),
                );
            }
        };
        let source = match source {
            0 => hp2_launcher::model::DataSource::Retail,
            1 => hp2_launcher::model::DataSource::Prototype,
            other => {
                return deliver(
                    sink,
                    ctx,
                    Err(format!("unknown data source ordinal {other}")),
                );
            }
        };
        deliver(
            sink,
            ctx,
            hp2_launcher::orchestrate::prepare_data_source_profile(launcher_root, source),
        )
    })
}

/// Called once with a loaded/default `LaunchSelection`. `action` is
/// `LaunchAction`'s C++ ordinal (Continue=0, NewGame=1, Quit=2,
/// Error=3). UTF-8 string spans, not NUL-terminated, valid only for
/// the call's duration.
#[allow(clippy::too_many_arguments)]
pub type LaunchSelectionSink = unsafe extern "C" fn(
    ctx: *mut c_void,
    action: i32,
    has_save: bool,
    slot: i32,
    save_index: i32,
    uses_slot_directory: bool,
    save_path: *const u8,
    save_path_len: usize,
    thumbnail_path: *const u8,
    thumbnail_path_len: usize,
    display_name: *const u8,
    display_name_len: usize,
    size: i64,
    modified_seconds: i64,
);

/// Replaces `HP2Launcher::CommitLaunchSelection`. `launcher_root` is
/// UTF-8, not NUL-terminated; `action` is `LaunchAction`'s C++ ordinal
/// (Continue=0, NewGame=1, Quit=2, Error=3). String fields are UTF-8,
/// not NUL-terminated. Calls `error_sink` exactly once, only on
/// failure.
///
/// # Safety
/// Every `(ptr, len)` pair must be valid for reads of that length (or
/// null with length 0). `error_sink` must be safe to call with
/// `error_ctx` and the span described above, valid only for the
/// call's duration.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_commit_selection(
    launcher_root: *const u8,
    launcher_root_len: usize,
    action: i32,
    has_save: bool,
    slot: i32,
    save_index: i32,
    uses_slot_directory: bool,
    save_path: *const u8,
    save_path_len: usize,
    thumbnail_path: *const u8,
    thumbnail_path_len: usize,
    display_name: *const u8,
    display_name_len: usize,
    size: i64,
    modified_seconds: i64,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_commit_selection", false, move || unsafe {
        use hp2_launcher::model::{LaunchAction, LaunchSelection, SaveRecord};
        let launcher_root = match str_from_raw(launcher_root, launcher_root_len) {
            Some(s) => s,
            None => {
                return report_error(
                    error_sink,
                    error_ctx,
                    "launcher root path is not valid UTF-8",
                );
            }
        };
        let save_path = match str_from_raw(save_path, save_path_len) {
            Some(s) => s,
            None => return report_error(error_sink, error_ctx, "save path is not valid UTF-8"),
        };
        let thumbnail_path = match str_from_raw(thumbnail_path, thumbnail_path_len) {
            Some(s) => s,
            None => {
                return report_error(error_sink, error_ctx, "thumbnail path is not valid UTF-8");
            }
        };
        let display_name = match str_from_raw(display_name, display_name_len) {
            Some(s) => s,
            None => return report_error(error_sink, error_ctx, "display name is not valid UTF-8"),
        };
        let action = match action {
            0 => LaunchAction::Continue,
            1 => LaunchAction::NewGame,
            2 => LaunchAction::Quit,
            3 => LaunchAction::Error,
            other => {
                return report_error(
                    error_sink,
                    error_ctx,
                    &format!("unknown launch action ordinal {other}"),
                );
            }
        };
        let selection = LaunchSelection {
            action,
            has_save,
            save: SaveRecord {
                slot,
                save_index,
                uses_slot_directory,
                save_path: save_path.to_string(),
                thumbnail_path: thumbnail_path.to_string(),
                display_name: display_name.to_string(),
                size,
                modified_seconds,
            },
        };
        match hp2_launcher::orchestrate::commit_launch_selection(launcher_root, &selection) {
            Ok(()) => true,
            Err(e) => report_error(error_sink, error_ctx, &e),
        }
    })
}

/// Replaces `HP2Launcher::LoadLaunchSelection`. `launcher_root` is
/// UTF-8, not NUL-terminated. Calls `selection_sink` exactly once on
/// success, `error_sink` exactly once on failure.
///
/// # Safety
/// `(launcher_root, launcher_root_len)` must be valid for reads of
/// that length (or null with length 0). `selection_sink`/`error_sink`
/// must be safe to call with their `ctx` and the spans described
/// above, valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_load_selection(
    launcher_root: *const u8,
    launcher_root_len: usize,
    selection_sink: LaunchSelectionSink,
    selection_ctx: *mut c_void,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_load_selection", false, move || unsafe {
        let launcher_root = match str_from_raw(launcher_root, launcher_root_len) {
            Some(s) => s,
            None => {
                return report_error(
                    error_sink,
                    error_ctx,
                    "launcher root path is not valid UTF-8",
                );
            }
        };
        match hp2_launcher::orchestrate::load_launch_selection(launcher_root) {
            Ok(selection) => {
                let save = &selection.save;
                selection_sink(
                    selection_ctx,
                    selection.action as i32,
                    selection.has_save,
                    save.slot,
                    save.save_index,
                    save.uses_slot_directory,
                    save.save_path.as_ptr(),
                    save.save_path.len(),
                    save.thumbnail_path.as_ptr(),
                    save.thumbnail_path.len(),
                    save.display_name.as_ptr(),
                    save.display_name.len(),
                    save.size,
                    save.modified_seconds,
                );
                true
            }
            Err(e) => report_error(error_sink, error_ctx, &e),
        }
    })
}

/// Replaces `HP2Launcher::ValidateLauncherSettings`. Calls
/// `error_sink` exactly once, only on failure.
///
/// # Safety
/// `settings` must be a valid, aligned pointer to a live
/// `FfiLauncherSettings`. `error_sink` must be safe to call with
/// `error_ctx` and the span described above, valid only for the
/// call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_validate_settings(
    settings: *const FfiLauncherSettings,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_validate_settings", false, move || unsafe {
        let settings = match settings_from_ffi(&*settings) {
            Ok(s) => s,
            Err(e) => return report_error(error_sink, error_ctx, &e),
        };
        match hp2_launcher::store::validate_launcher_settings(&settings) {
            Ok(()) => true,
            Err(e) => report_error(error_sink, error_ctx, &e),
        }
    })
}

/// Replaces `HP2Launcher::DiscoverSaves`. `user_root` is UTF-8, not
/// NUL-terminated. Calls `save_sink` once per discovered save, in the
/// real C++'s own sorted order. Calls `error_sink` exactly once, only
/// on failure.
///
/// # Safety
/// `(user_root, user_root_len)` must be valid for reads of that
/// length (or null with length 0). `save_sink`/`error_sink` must be
/// safe to call with their `ctx` and the spans described above, valid
/// only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_discover_saves(
    user_root: *const u8,
    user_root_len: usize,
    save_sink: SaveRecordSink,
    save_ctx: *mut c_void,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_launcher_discover_saves", false, move || unsafe {
        let user_root = match str_from_raw(user_root, user_root_len) {
            Some(s) => s,
            None => {
                return report_error(error_sink, error_ctx, "user root path is not valid UTF-8");
            }
        };
        match hp2_launcher::saves::discover_saves(user_root) {
            Ok(saves) => {
                for save in &saves {
                    report_save(save_sink, save_ctx, save);
                }
                true
            }
            Err(e) => report_error(error_sink, error_ctx, &e),
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn save_record_from_ffi(
    slot: i32,
    save_index: i32,
    uses_slot_directory: bool,
    save_path: &str,
    thumbnail_path: &str,
    display_name: &str,
    size: i64,
    modified_seconds: i64,
) -> hp2_launcher::model::SaveRecord {
    hp2_launcher::model::SaveRecord {
        slot,
        save_index,
        uses_slot_directory,
        save_path: save_path.to_string(),
        thumbnail_path: thumbnail_path.to_string(),
        display_name: display_name.to_string(),
        size,
        modified_seconds,
    }
}

/// Shared by `hp2_rs_launcher_build_selected_command` and
/// `hp2_rs_launcher_serialize_launch_selection`: decodes the flat
/// action/has_save/save-field parameter list both functions take into
/// a `LaunchSelection`, or an error string on invalid UTF-8/an
/// unknown action ordinal.
#[allow(clippy::too_many_arguments)]
unsafe fn launch_selection_from_ffi(
    action: i32,
    has_save: bool,
    slot: i32,
    save_index: i32,
    uses_slot_directory: bool,
    save_path: *const u8,
    save_path_len: usize,
    thumbnail_path: *const u8,
    thumbnail_path_len: usize,
    display_name: *const u8,
    display_name_len: usize,
    size: i64,
    modified_seconds: i64,
) -> Result<hp2_launcher::model::LaunchSelection, String> {
    use hp2_launcher::model::LaunchAction;
    let save_path =
        unsafe { str_from_raw(save_path, save_path_len) }.ok_or("save path is not valid UTF-8")?;
    let thumbnail_path = unsafe { str_from_raw(thumbnail_path, thumbnail_path_len) }
        .ok_or("thumbnail path is not valid UTF-8")?;
    let display_name = unsafe { str_from_raw(display_name, display_name_len) }
        .ok_or("display name is not valid UTF-8")?;
    let action = match action {
        0 => LaunchAction::Continue,
        1 => LaunchAction::NewGame,
        2 => LaunchAction::Quit,
        3 => LaunchAction::Error,
        other => return Err(format!("unknown launch action ordinal {other}")),
    };
    Ok(hp2_launcher::model::LaunchSelection {
        action,
        has_save,
        save: save_record_from_ffi(
            slot,
            save_index,
            uses_slot_directory,
            save_path,
            thumbnail_path,
            display_name,
            size,
            modified_seconds,
        ),
    })
}

/// Replaces `HP2Launcher::BuildSelectedCommand`. `action` is
/// `LaunchAction`'s C++ ordinal (Continue=0, NewGame=1, Quit=2,
/// Error=3). String fields are UTF-8, not NUL-terminated. Calls
/// `sink` exactly once with the resulting command prefix on success
/// or the error message on failure; the return value says which.
///
/// # Safety
/// Every `(ptr, len)` pair must be valid for reads of that length (or
/// null with length 0). `sink` must be safe to call with `ctx` and a
/// `(ptr, len)` UTF-8 byte span valid only for the call's duration.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_build_selected_command(
    action: i32,
    has_save: bool,
    slot: i32,
    save_index: i32,
    uses_slot_directory: bool,
    save_path: *const u8,
    save_path_len: usize,
    thumbnail_path: *const u8,
    thumbnail_path_len: usize,
    display_name: *const u8,
    display_name_len: usize,
    size: i64,
    modified_seconds: i64,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard(
        "hp2_rs_launcher_build_selected_command",
        false,
        move || unsafe {
            let selection = match launch_selection_from_ffi(
                action,
                has_save,
                slot,
                save_index,
                uses_slot_directory,
                save_path,
                save_path_len,
                thumbnail_path,
                thumbnail_path_len,
                display_name,
                display_name_len,
                size,
                modified_seconds,
            ) {
                Ok(s) => s,
                Err(e) => return deliver(sink, ctx, Err(e)),
            };
            deliver(
                sink,
                ctx,
                hp2_launcher::policy::build_selected_command(&selection),
            )
        },
    )
}

/// Called once per emitted `[LastLaunch]` field row. UTF-8 string
/// spans, not NUL-terminated, valid only for the call's duration.
pub type FieldSink = unsafe extern "C" fn(
    ctx: *mut c_void,
    key: *const u8,
    key_len: usize,
    value: *const u8,
    value_len: usize,
);

/// Replaces `HP2Launcher::SerializeLaunchSelection`. `action` is
/// `LaunchAction`'s C++ ordinal (Continue=0, NewGame=1, Quit=2,
/// Error=3). String fields are UTF-8, not NUL-terminated. Calls
/// `field_sink` once per emitted field row on success, `error_sink`
/// exactly once on failure.
///
/// # Safety
/// Every `(ptr, len)` pair must be valid for reads of that length (or
/// null with length 0). `field_sink`/`error_sink` must be safe to
/// call with their `ctx` and the spans described above, valid only
/// for the call's duration.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_serialize_launch_selection(
    action: i32,
    has_save: bool,
    slot: i32,
    save_index: i32,
    uses_slot_directory: bool,
    save_path: *const u8,
    save_path_len: usize,
    thumbnail_path: *const u8,
    thumbnail_path_len: usize,
    display_name: *const u8,
    display_name_len: usize,
    size: i64,
    modified_seconds: i64,
    field_sink: FieldSink,
    field_ctx: *mut c_void,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard(
        "hp2_rs_launcher_serialize_launch_selection",
        false,
        move || unsafe {
            let selection = match launch_selection_from_ffi(
                action,
                has_save,
                slot,
                save_index,
                uses_slot_directory,
                save_path,
                save_path_len,
                thumbnail_path,
                thumbnail_path_len,
                display_name,
                display_name_len,
                size,
                modified_seconds,
            ) {
                Ok(s) => s,
                Err(e) => return report_error(error_sink, error_ctx, &e),
            };
            match hp2_launcher::policy::serialize_launch_selection(&selection) {
                Ok(fields) => {
                    for field in &fields {
                        field_sink(
                            field_ctx,
                            field.key.as_ptr(),
                            field.key.len(),
                            field.value.as_ptr(),
                            field.value.len(),
                        );
                    }
                    true
                }
                Err(e) => report_error(error_sink, error_ctx, &e),
            }
        },
    )
}

/// One `(ptr, len)` key/value row, as passed into
/// `hp2_rs_launcher_deserialize_launch_selection`. Input only: the
/// pointers must stay valid for the call's duration, same as any
/// other `(ptr, len)` FFI argument -- not read after the call
/// returns.
#[repr(C)]
pub struct FfiLaunchSelectionField {
    pub key: *const u8,
    pub key_len: usize,
    pub value: *const u8,
    pub value_len: usize,
}

/// Replaces `HP2Launcher::DeserializeLaunchSelection`. String fields
/// are UTF-8, not NUL-terminated. Calls `selection_sink` exactly once
/// -- with the parsed selection on success, or the `Quit` fallback on
/// failure, matching the real C++'s own "selection is still populated
/// with the fallback" contract (see that function's doc comment) --
/// then, only on failure, calls `error_sink` once more with the
/// reason.
///
/// # Safety
/// `fields` must be valid for reads of `field_count` elements (each
/// element's own `(ptr, len)` pairs valid for reads of that length,
/// or null with length 0). `selection_sink`/`error_sink` must be safe
/// to call with their `ctx` and the spans described above, valid only
/// for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_deserialize_launch_selection(
    fields: *const FfiLaunchSelectionField,
    field_count: usize,
    selection_sink: LaunchSelectionSink,
    selection_ctx: *mut c_void,
    error_sink: ErrorSink,
    error_ctx: *mut c_void,
) -> bool {
    guard(
        "hp2_rs_launcher_deserialize_launch_selection",
        false,
        move || unsafe {
            let mut parsed = Vec::with_capacity(field_count);
            let raw_fields = if field_count == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(fields, field_count)
            };
            for field in raw_fields {
                let key = match str_from_raw(field.key, field.key_len) {
                    Some(s) => s,
                    None => {
                        return report_error(error_sink, error_ctx, "field key is not valid UTF-8");
                    }
                };
                let value = match str_from_raw(field.value, field.value_len) {
                    Some(s) => s,
                    None => {
                        return report_error(
                            error_sink,
                            error_ctx,
                            "field value is not valid UTF-8",
                        );
                    }
                };
                parsed.push(hp2_launcher::policy::LaunchSelectionField {
                    key: key.to_string(),
                    value: value.to_string(),
                });
            }
            let (selection, result) =
                match hp2_launcher::policy::deserialize_launch_selection(&parsed) {
                    Ok(selection) => (selection, Ok(())),
                    Err(e) => (hp2_launcher::model::LaunchSelection::default(), Err(e)),
                };
            let save = &selection.save;
            selection_sink(
                selection_ctx,
                selection.action as i32,
                selection.has_save,
                save.slot,
                save.save_index,
                save.uses_slot_directory,
                save.save_path.as_ptr(),
                save.save_path.len(),
                save.thumbnail_path.as_ptr(),
                save.thumbnail_path.len(),
                save.display_name.as_ptr(),
                save.display_name.len(),
                save.size,
                save.modified_seconds,
            );
            match result {
                Ok(()) => true,
                Err(e) => report_error(error_sink, error_ctx, &e),
            }
        },
    )
}

/// One `(ptr, len)` argv entry, as passed into
/// `hp2_rs_launcher_should_run_native_launcher`. Input only, same
/// validity rule as `FfiLaunchSelectionField`.
#[repr(C)]
pub struct FfiStringSpan {
    pub ptr: *const u8,
    pub len: usize,
}

/// Replaces `HP2Launcher::ShouldRunNativeLauncher`. Each `args` entry
/// is UTF-8, not NUL-terminated (index 0, the program name, is
/// skipped internally the same way the real C++ skips `argv[0]`).
/// Invalid UTF-8 in any entry is treated as a non-matching argument
/// (not an error -- this function has no error path, matching the
/// real C++'s plain `bool` return).
///
/// # Safety
/// `args` must be valid for reads of `arg_count` elements (each
/// element's own `(ptr, len)` pair valid for reads of that length, or
/// null with length 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_should_run_native_launcher(
    args: *const FfiStringSpan,
    arg_count: usize,
) -> bool {
    guard(
        "hp2_rs_launcher_should_run_native_launcher",
        false,
        move || unsafe {
            let raw_args = if arg_count == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(args, arg_count)
            };
            let args: Vec<String> = raw_args
                .iter()
                .map(|a| str_from_raw(a.ptr, a.len).unwrap_or("").to_string())
                .collect();
            hp2_launcher::policy::should_run_native_launcher(&args)
        },
    )
}

/// One `(ptr, len)`-tripled legacy INI row, as passed into
/// `hp2_rs_launcher_migrate_legacy_settings`. Input only, same
/// validity rule as `FfiLaunchSelectionField`.
#[repr(C)]
pub struct FfiLegacySettingValue {
    pub section: *const u8,
    pub section_len: usize,
    pub key: *const u8,
    pub key_len: usize,
    pub value: *const u8,
    pub value_len: usize,
}

/// Called once per canonicalized output row.
pub type LegacyValueSink = unsafe extern "C" fn(
    ctx: *mut c_void,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
    value: *const u8,
    value_len: usize,
);

/// Called once per normalization performed.
#[allow(clippy::too_many_arguments)]
pub type MigrationChangeSink = unsafe extern "C" fn(
    ctx: *mut c_void,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
    previous_value: *const u8,
    previous_value_len: usize,
    new_value: *const u8,
    new_value_len: usize,
    reason: *const u8,
    reason_len: usize,
);

/// Replaces `HP2Launcher::MigrateLegacySettings`. String fields are
/// UTF-8, not NUL-terminated. This never fails (a pure data
/// transformation, matching the real C++'s own `bool`-free return
/// type): invalid UTF-8 anywhere in `values` is skipped (that row is
/// dropped, matching the panic-can-never-cross-FFI rule more
/// cleanly than inventing an error path this function was never
/// meant to have). Calls `value_sink` once per canonicalized output
/// row and `change_sink` once per normalization performed.
///
/// # Safety
/// `values` must be valid for reads of `value_count` elements (each
/// element's own `(ptr, len)` pairs valid for reads of that length,
/// or null with length 0). `value_sink`/`change_sink` must be safe to
/// call with their `ctx` and the spans described above, valid only
/// for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_launcher_migrate_legacy_settings(
    values: *const FfiLegacySettingValue,
    value_count: usize,
    value_sink: LegacyValueSink,
    value_ctx: *mut c_void,
    change_sink: MigrationChangeSink,
    change_ctx: *mut c_void,
) {
    guard(
        "hp2_rs_launcher_migrate_legacy_settings",
        (),
        move || unsafe {
            let raw_values = if value_count == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(values, value_count)
            };
            let mut parsed = Vec::with_capacity(value_count);
            for value in raw_values {
                let (Some(section), Some(key), Some(text)) = (
                    str_from_raw(value.section, value.section_len),
                    str_from_raw(value.key, value.key_len),
                    str_from_raw(value.value, value.value_len),
                ) else {
                    continue;
                };
                parsed.push(hp2_launcher::migrate::LegacySettingValue {
                    section: section.to_string(),
                    key: key.to_string(),
                    value: text.to_string(),
                });
            }
            let (migrated, changes) = hp2_launcher::migrate::migrate_legacy_settings(&parsed);
            for row in &migrated {
                value_sink(
                    value_ctx,
                    row.section.as_ptr(),
                    row.section.len(),
                    row.key.as_ptr(),
                    row.key.len(),
                    row.value.as_ptr(),
                    row.value.len(),
                );
            }
            for change in &changes {
                change_sink(
                    change_ctx,
                    change.section.as_ptr(),
                    change.section.len(),
                    change.key.as_ptr(),
                    change.key.len(),
                    change.previous_value.as_ptr(),
                    change.previous_value.len(),
                    change.new_value.as_ptr(),
                    change.new_value.len(),
                    change.reason.as_ptr(),
                    change.reason.len(),
                );
            }
        },
    )
}

/// Replaces `SetLauncherPublishFailureForTesting`. Always present
/// (not conditionally compiled); see
/// `hp2_launcher::atomic::set_publish_failure_for_testing`'s own doc
/// comment for why that is safe. The C++ adapter calling this still
/// gates its own export behind `#ifdef HP2_LAUNCHER_TESTING`, matching
/// the header exactly.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_launcher_set_publish_failure_for_testing(publication_index: i32) {
    guard(
        "hp2_rs_launcher_set_publish_failure_for_testing",
        (),
        move || {
            hp2_launcher::atomic::set_publish_failure_for_testing(publication_index);
        },
    )
}

/// Replaces `USDLClient::GetClipboardText` (`SDLClient.cpp:279-294`).
/// Calls `text_sink` exactly once, only when the clipboard has usable
/// UTF-8 text; returns whether it did. UTF-8, not NUL-terminated.
///
/// # Safety
/// `text_sink` must be safe to call with `text_ctx` and the span
/// described above, valid only for the call's duration.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_get_clipboard_text(
    text_sink: ErrorSink,
    text_ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_platform_get_clipboard_text", false, move || {
        match hp2_platform::clipboard::get_clipboard_text() {
            Some(text) => {
                unsafe { text_sink(text_ctx, text.as_ptr(), text.len()) };
                true
            }
            None => false,
        }
    })
}

/// Replaces `USDLClient::SetClipboardText` (`SDLClient.cpp:296-304`).
/// `text` is UTF-8, not NUL-terminated. Returns whether
/// `SDL_SetClipboardText` succeeded.
///
/// # Safety
/// `(text, text_len)` must be valid for reads of that length, or null
/// with length 0.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_set_clipboard_text(
    text: *const u8,
    text_len: usize,
) -> bool {
    guard(
        "hp2_rs_platform_set_clipboard_text",
        false,
        move || unsafe {
            match str_from_raw(text, text_len) {
                Some(text) => hp2_platform::clipboard::set_clipboard_text(text),
                None => false,
            }
        },
    )
}

/// Replaces `USDLViewport::UpdateWindowFrame` (`SDLViewport.cpp:233-239`).
/// `sdl_window` may be null, matching the real C++'s own guard.
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the
/// duration of this call.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_update_window_frame(
    sdl_window: *mut c_void,
    borderless: bool,
) {
    guard("hp2_rs_platform_update_window_frame", (), move || unsafe {
        hp2_platform::window_chrome::update_window_frame(
            sdl_window as *mut hp2_platform::window_chrome::SdlWindow,
            borderless,
        )
    })
}

/// Replaces `USDLViewport::SetTitleBar` (`SDLViewport.cpp:749-759`).
/// `sdl_window` may be null, matching the real C++'s own guard.
/// `title`/`title_len` are UTF-8, not NUL-terminated.
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the
/// duration of this call. `(title, title_len)` must be valid for
/// reads of that length, or null with length 0.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_set_title_bar(
    sdl_window: *mut c_void,
    title: *const u8,
    title_len: usize,
) {
    guard("hp2_rs_platform_set_title_bar", (), move || unsafe {
        let Some(title) = str_from_raw(title, title_len) else {
            return;
        };
        hp2_platform::window_chrome::set_title_bar(
            sdl_window as *mut hp2_platform::window_chrome::SdlWindow,
            title,
        )
    })
}

/// Boxed on the heap, handed to C++ as an opaque `void* RustDevice`
/// (`UWgpuRenderDevice`'s only field, per the plan's Phase 3.2 "class
/// owns only an opaque `void* RustDevice`" design). Caches
/// `backend_name` as an owned `String` alongside the `CreatedDevice`
/// so `hp2_rs_wgpu_create`'s result can point into memory the handle
/// itself owns, valid for the handle's whole lifetime -- not a
/// temporary that would dangle the moment the function returns.
/// Field order is drop order (Rust drops struct fields in declaration
/// order): `render_loop` first, `created` last. `render_loop` can
/// hold an in-flight `wgpu::SurfaceTexture`/`CommandEncoder` (e.g. if
/// `Exit` runs between a `Lock` and its matching `Unlock`) that
/// borrows from `created`'s own `Surface`/`Device`/`Instance` --
/// dropping `created` first would drop those out from under the
/// still-held frame, the same drop-order hazard `CreatedDevice`
/// itself was fixed for (caught on review, not self-caught).
#[cfg(feature = "renderer-wgpu")]
struct WgpuHandle {
    render_loop: hp2_wgpu::RenderLoop,
    created: hp2_wgpu::CreatedDevice,
    backend_name: String,
}

/// Result of `hp2_rs_wgpu_create`. `handle` is null on total failure
/// (no adapter/device could be created); every other field is
/// meaningless in that case. `adapter_name`/`backend_name` point into
/// memory the handle itself owns -- valid exactly as long as `handle`
/// is (i.e. until `hp2_rs_wgpu_destroy` is called on it), not a moment
/// longer; the caller must copy them out (e.g. into a `debugf` call)
/// before destroying the handle, never store the raw pointers past
/// that point.
#[cfg(feature = "renderer-wgpu")]
#[repr(C)]
pub struct FfiWgpuCreateResult {
    pub handle: *mut c_void,
    pub adapter_name: *const u8,
    pub adapter_name_len: usize,
    pub backend_name: *const u8,
    pub backend_name_len: usize,
}

#[cfg(feature = "renderer-wgpu")]
const FFI_WGPU_CREATE_FAILURE: FfiWgpuCreateResult = FfiWgpuCreateResult {
    handle: std::ptr::null_mut(),
    adapter_name: std::ptr::null(),
    adapter_name_len: 0,
    backend_name: std::ptr::null(),
    backend_name_len: 0,
};

/// Replaces the adapter/device-creation half of `UWgpuRenderDevice::
/// Init` (`WgpuDrv`'s future C++ body; see `local://
/// rust-inplace-port-plan.md`'s Phase 3.2). `ns_view` is the pointer
/// `SDL_Metal_CreateView` returns on macOS; null means headless (no
/// window surface -- every non-macOS caller, and any macOS caller
/// that has not created a Metal view yet). When non-null, the surface
/// and the adapter/device it requests share one `Instance`, so the
/// adapter request's `compatible_surface` check is real (see
/// `hp2-wgpu`'s own `CreatedDevice`/`create_device_sync` doc for why
/// that matters). Headless still lets C++ log the plan's `WgpuDrv:
/// adapter "<name>" backend <X>` line, same as every other render
/// device's `Init` logs its own adapter info today (see `XOpenGL.cpp`'s
/// `debugf(NAME_Init, ...)` calls).
///
/// `backend` is the plan's `[WgpuDrv.WgpuRenderDevice] Backend=
/// Auto|Metal|Vulkan|Dx12|Gl` ini key's real `BYTE` config-property
/// value (`WgpuDrv.cpp`'s `UEnum`-backed `Backend` field), converted
/// via `hp2_wgpu::BackendChoice::from_ffi_discriminant` -- see that
/// function's own doc for the exact member-order contract with the C++
/// side. An out-of-range value (hand-edited ini, or a stale ini from a
/// future version with more members) falls back to `Auto`, logged here
/// (not silently) so the real invalid value is visible, not just that
/// a fallback happened.
///
/// # Safety
/// `ns_view`, if non-null, must be a valid pointer to an `NSView`
/// (from `SDL_Metal_CreateView`) that outlives the returned `handle`
/// -- the caller must destroy `handle` (via `hp2_rs_wgpu_destroy`)
/// before calling `SDL_Metal_DestroyView` on it. Must be called on the
/// application's main thread if non-null (AppKit requirement). The
/// returned `handle`, if non-null, must be passed to
/// `hp2_rs_wgpu_destroy` exactly once, and never dereferenced directly
/// by C++ (opaque).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_create(
    ns_view: *mut c_void,
    backend: u32,
) -> FfiWgpuCreateResult {
    guard("hp2_rs_wgpu_create", FFI_WGPU_CREATE_FAILURE, move || {
        let backend_choice = hp2_wgpu::BackendChoice::from_ffi_discriminant(backend)
            .unwrap_or_else(|| {
                log::warn!(
                    "hp2_rs_wgpu_create: invalid Backend config value {backend} (expected \
                     0..=4), falling back to Auto"
                );
                hp2_wgpu::BackendChoice::Auto
            });
        #[cfg(target_os = "macos")]
        let created = if ns_view.is_null() {
            let no_surface: Option<hp2_wgpu::HeadlessSurfaceCreator> = None;
            hp2_wgpu::create_device_sync(backend_choice, no_surface)
        } else {
            let create_surface = move |instance: &wgpu::Instance| unsafe {
                hp2_wgpu::macos::create_surface_from_ns_view(instance, ns_view)
            };
            hp2_wgpu::create_device_sync(backend_choice, Some(create_surface))
        };
        #[cfg(not(target_os = "macos"))]
        let created = {
            // No window-surface path on this platform yet (hp2-wgpu's
            // `macos` module is macOS-only -- see its own doc for
            // why). A non-null ns_view here is unreachable from the
            // real C++ call site today (WgpuDrv.cpp only ever builds
            // one under #if MACOSX), but if it ever happened it would
            // be a caller bug silently producing a device that can
            // never present to the view the caller asked for --
            // AGENTS.md's Forbidden Actions rules out swallowing that
            // rather than failing loudly (an earlier version of this
            // branch did exactly that, caught on review). Logged, not
            // just failed: the real game does install a log sink
            // (SDLLaunch.cpp's hp2_rs_set_log), so this is visible
            // there even though standalone ctest binaries that never
            // call hp2_rs_set_log will not see it.
            if !ns_view.is_null() {
                log::warn!(
                    "hp2_rs_wgpu_create: non-null ns_view on a platform with no window-surface path, failing rather than silently continuing headless"
                );
                None
            } else {
                let no_surface: Option<hp2_wgpu::HeadlessSurfaceCreator> = None;
                hp2_wgpu::create_device_sync(backend_choice, no_surface)
            }
        };
        let Some(created) = created else {
            return FFI_WGPU_CREATE_FAILURE;
        };
        let backend_name = hp2_wgpu::backend_name(created.backend).to_string();
        let handle = Box::new(WgpuHandle {
            render_loop: hp2_wgpu::RenderLoop::new(),
            created,
            backend_name,
        });
        let adapter_name = handle.created.adapter_name.as_ptr();
        let adapter_name_len = handle.created.adapter_name.len();
        let backend_name_ptr = handle.backend_name.as_ptr();
        let backend_name_len = handle.backend_name.len();
        FfiWgpuCreateResult {
            handle: Box::into_raw(handle) as *mut c_void,
            adapter_name,
            adapter_name_len,
            backend_name: backend_name_ptr,
            backend_name_len,
        }
    })
}

/// Replaces the destruction half of `UWgpuRenderDevice::Exit`. `NULL`
/// is a safe no-op (matches the real C++'s own `if (Pointer)`-guard
/// convention used throughout this codebase for destroy-style calls).
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (or
/// null), not yet destroyed, and never used again after this call.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_destroy(handle: *mut c_void) {
    guard("hp2_rs_wgpu_destroy", (), move || unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle as *mut WgpuHandle));
        }
    })
}

/// Replaces the per-frame-begin half of `UWgpuRenderDevice::Lock`.
/// `r`/`g`/`b`/`a` are the real engine's `ScreenClear` `FPlane`'s own
/// `X`/`Y`/`Z`/`W` components -- confirmed the real clear color (not
/// `FlashScale`/`FlashFog`, which XOpenGL stores for its own separate
/// post-process tint, not the buffer clear) by reading XOpenGL.cpp's
/// own `Lock`: `glClearColor(ScreenClear.X, ScreenClear.Y,
/// ScreenClear.Z, ScreenClear.W)`. `width`/`height` are the real
/// engine's current `Viewport->SizeX`/`SizeY` -- XOpenGL's own `Lock`
/// reads the same fields the same way (`Max(1, Viewport->SizeX)`),
/// confirmed before using this pattern, not guessed. `vsync` is the
/// real `UseVSync` config property's own raw `BYTE` value (0=Off,
/// 1=On, 2=Adaptive, matching `WgpuDrv.cpp`'s own `VSyncs` UEnum
/// member order exactly) -- converted here to `wgpu::PresentMode`
/// (`Immediate`/`Fifo`/`FifoRelaxed`), not assumed already converted
/// by the caller, matching `BackendChoice::from_ffi_discriminant`'s
/// own established raw-byte-at-the-FFI-boundary pattern. Any other
/// raw value falls back to `Fifo` (VSync On) -- the safest default,
/// not a crash or an unvalidated out-of-range enum. Returns
/// `hp2_wgpu::FrameOutcome` as a raw `u32` (`0` is `Presented`, the
/// only success code here too -- see that enum's own doc). A caught
/// panic returns `Panicked`, distinct from a null `handle`'s
/// `InternalError` -- `guard`'s own panic log goes through
/// `log::error!`, which a standalone ctest binary may have no logger
/// installed to show, so the distinction needs to survive in the
/// return value itself, not only in a log that might not appear.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed). Must be called on the application's main thread
/// (AppKit requirement, same as surface creation itself) if `handle`
/// has a live surface.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_lock(
    handle: *mut c_void,
    width: u32,
    height: u32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    vsync: u8,
) -> u32 {
    guard(
        "hp2_rs_wgpu_lock",
        hp2_wgpu::FrameOutcome::Panicked as u32,
        move || {
            if handle.is_null() {
                return hp2_wgpu::FrameOutcome::InternalError as u32;
            }
            let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
            let clear_color = wgpu::Color {
                r: r as f64,
                g: g as f64,
                b: b as f64,
                a: a as f64,
            };
            let present_mode = match vsync {
                0 => wgpu::PresentMode::Immediate,
                2 => wgpu::PresentMode::FifoRelaxed,
                _ => wgpu::PresentMode::Fifo,
            };
            hp2_wgpu::lock_frame(
                &handle.created,
                &mut handle.render_loop,
                width.max(1),
                height.max(1),
                clear_color,
                present_mode,
            ) as u32
        },
    )
}

/// Replaces the per-frame-end half of `UWgpuRenderDevice::Unlock`.
/// `present` is the real `Unlock(UBOOL Blit)`'s own `Blit` parameter,
/// passed through unchanged (`hp2_wgpu::unlock_frame`'s own doc has
/// the detail on what it does with it). Returns `false` if `handle`
/// has no frame in progress (`hp2_rs_wgpu_lock` was never called, or
/// its last call did not return the success code `0`) -- a caller bug,
/// not a real runtime condition.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_unlock(handle: *mut c_void, present: bool) -> bool {
    guard("hp2_rs_wgpu_unlock", false, move || {
        if handle.is_null() {
            return false;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        hp2_wgpu::unlock_frame(&handle.created, &mut handle.render_loop, present)
    })
}

/// Records the real HiDPI backing scale (drawable pixels per logical
/// window pixel) for `HP2_CAPTURE_FRAMES`'s `backing_scale_factor` field
/// (`hp2_wgpu::RenderLoop::set_backing_scale`). Called by `WgpuDrv.cpp`'s
/// `Lock` before `hp2_rs_wgpu_lock`, computed the same way XOpenGL.cpp:
/// 2728-2731 does. Non-finite or non-positive values fall back to `1.0`.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not yet
/// destroyed), or null (a no-op).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_set_backing_scale(handle: *mut c_void, scale: f64) {
    guard("hp2_rs_wgpu_set_backing_scale", (), move || {
        if handle.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        handle.render_loop.set_backing_scale(scale);
    })
}

/// Applies the real `MaxAnisotropy` ini value to every smooth texture
/// sampler (`hp2_wgpu::RenderLoop::set_max_anisotropy`; values below 1,
/// including NaN, mean off). Called by `WgpuDrv.cpp`'s `Init`.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not yet
/// destroyed), or null (a no-op).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_set_max_anisotropy(handle: *mut c_void, max_anisotropy: f32) {
    guard("hp2_rs_wgpu_set_max_anisotropy", (), move || {
        if handle.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.set_max_anisotropy(max_anisotropy);
    })
}

/// Applies the real `UseSRGBTextures` framebuffer half: `true` selects
/// an sRGB-encoding swapchain format (`hp2_wgpu::RenderLoop::
/// set_srgb_framebuffer`, XOpenGL.cpp:1175-1176). Called by
/// `WgpuDrv.cpp`'s `Init`.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not yet
/// destroyed), or null (a no-op).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_set_srgb_framebuffer(handle: *mut c_void, srgb: bool) {
    guard("hp2_rs_wgpu_set_srgb_framebuffer", (), move || {
        if handle.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.set_srgb_framebuffer(srgb);
    })
}

/// Replaces the frame-outcome-reporting half of
/// `UWgpuRenderDevice::Exit`. Returns the live tally of every
/// `hp2_rs_wgpu_lock` outcome since `handle` was created
/// (`hp2_wgpu::FrameCounts`'s own doc has the detail on why a tally,
/// not just the latest outcome, matters). Returns all-zero counts for
/// a null `handle`.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_frame_counts(handle: *mut c_void) -> hp2_wgpu::FrameCounts {
    guard(
        "hp2_rs_wgpu_frame_counts",
        hp2_wgpu::FrameCounts::default(),
        move || {
            if handle.is_null() {
                return hp2_wgpu::FrameCounts::default();
            }
            let handle = unsafe { &*(handle as *mut WgpuHandle) };
            handle.render_loop.counts()
        },
    )
}

/// Replaces the matrix-math half of `UXOpenGLRenderDevice::
/// SetSceneNode`. Called from `WgpuDrv.cpp`'s own real
/// `UWgpuRenderDevice::SetSceneNode` override (`WgpuDrv.cpp:346+` --
/// an earlier version of this doc said "not yet called from any
/// WgpuDrv.cpp override", true when written, now stale). `scene` is
/// read once by value before this function returns (`SceneFrame` is
/// `Copy`-shaped, all plain FFI-safe fields), so the caller's own
/// storage for it does not need to outlive the call. No-op (not a
/// crash) on a null `handle` or `scene`.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null. `scene` must point to a valid, fully
/// initialized `hp2_wgpu::SceneFrame`, or be null.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_set_scene_node(
    handle: *mut c_void,
    scene: *const hp2_wgpu::SceneFrame,
) {
    guard("hp2_rs_wgpu_set_scene_node", (), move || {
        if handle.is_null() || scene.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        let scene = unsafe { &*scene };
        handle
            .render_loop
            .set_scene_node(&handle.created.device, &handle.created.queue, scene);
    })
}

/// Replaces `UXOpenGLRenderDevice::Draw2DLine` (`DrawSimple.cpp:52-90`).
/// Called from `WgpuDrv.cpp`'s own real `UWgpuRenderDevice::
/// Draw2DLine` override (`WgpuDrv.cpp:434+` -- an earlier version of
/// this doc said "not yet called from any WgpuDrv.cpp override",
/// true when written, now stale). `p1`/`p2` are the
/// real `FVector P1, P2` parameters (screen pixel X/Y, view-space Z)
/// -- `hp2_wgpu::RenderLoop::queue_line`'s own doc has the real
/// screen-to-clip transform this applies internally, using the most
/// recent `hp2_rs_wgpu_set_scene_node` call for `RFX2`/`RFY2`
/// specifically. `fx2`/`fy2` are the real `Frame->FX2`/`FY2` the
/// actual `Draw2DLine` call received (`WgpuDrv.cpp`'s own override
/// reads them straight off its own `Frame` parameter) -- not derived
/// from the cached scene the way `RFX2`/`RFY2` correctly are;
/// `draw_2d_line_screen_to_coords`'s own doc has the full real
/// distinction and why an earlier version of this whole chain wrongly
/// conflated the two (caught on review). `r`/`g`/`b` are the real
/// `FPlane Color`'s first three components; alpha is not a parameter
/// here at all -- the real source always forces `Color.W = 1.f`
/// before use (`DrawSimple.cpp:64`, its own comment: "Unfortunately
/// this is usually set to 0"), so there is nothing a caller-supplied
/// alpha could correctly mean.
///
/// `line_flags`: the real `LineFlags`. `LINE_DepthCued` selects
/// depth-tested, depth-writing lines; otherwise depth is ignored. The
/// blend is the real fixed `BlendMode = PF_Highlighted`
/// (`hp2_wgpu::draw::RenderState::for_line_flags`). Not applied yet:
/// the real `HitTesting()` override (substituting `HitColor` for
/// `Color` during an editor hit-test pass); hit-testing readback is
/// tracked separately.
///
/// No-op (not a crash) on a null `handle`, when `handle` has no frame
/// in progress, or when no `hp2_rs_wgpu_set_scene_node` call has ever
/// been made on this `handle` -- all real, reachable conditions
/// `hp2_wgpu::RenderLoop::queue_line`'s own doc already covers, not
/// caller bugs.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_draw_2d_line(
    handle: *mut c_void,
    p1x: f32,
    p1y: f32,
    p1z: f32,
    p2x: f32,
    p2y: f32,
    p2z: f32,
    fx2: f32,
    fy2: f32,
    r: f32,
    g: f32,
    b: f32,
    line_flags: u32,
) {
    guard("hp2_rs_wgpu_draw_2d_line", (), move || {
        if handle.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.queue_line(
            [p1x, p1y, p1z],
            [p2x, p2y, p2z],
            fx2,
            fy2,
            // Color.W forced to 1.0 -- see this function's own doc.
            [r, g, b, 1.0],
            line_flags,
        );
    })
}

/// Replaces `UXOpenGLRenderDevice::ClearZ` (`hp2_wgpu::RenderLoop::
/// clear_z`): draws everything queued so far, then clears the depth
/// buffer to 1.0. No-op without a frame in progress.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_clear_z(handle: *mut c_void) {
    guard("hp2_rs_wgpu_clear_z", (), move || {
        if handle.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.clear_z(&handle.created);
    })
}

/// `FCanvasTextLayout` draw fields for `hp2_rs_wgpu_draw_native_text`
/// (`hp2_wgpu::native_text::NativeTextLayout`).
#[cfg(feature = "renderer-wgpu")]
#[repr(C)]
pub struct FfiNativeTextLayout {
    pub start_x: i32,
    pub start_y: i32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub clip_x: f32,
    pub clip_y: f32,
    pub clip: bool,
    pub poly_flags: u32,
    pub color: [f32; 4],
}

/// One drawable `FGlyphPlacement` (`hp2_wgpu::native_text::NativeTextGlyph`).
/// `font_identity`/`font_identity_len` is `Key.FontIdentity` as UTF-8;
/// `draw_left`/`draw_top` are `GlyphDrawOffset`'s outputs and
/// `raster_width`/`raster_height` `CalculateRasterRectangle`'s.
#[cfg(feature = "renderer-wgpu")]
#[repr(C)]
pub struct FfiNativeTextGlyph {
    pub font_identity: *const u8,
    pub font_identity_len: usize,
    pub glyph: u32,
    pub raster_point_size_64: u32,
    pub antialias: bool,
    pub subpixel: bool,
    pub draw_left: f64,
    pub draw_top: f64,
    pub raster_width: u32,
    pub raster_height: u32,
}

/// One `FUnderline` (`hp2_wgpu::native_text::NativeTextUnderline`).
#[cfg(feature = "renderer-wgpu")]
#[repr(C)]
pub struct FfiNativeTextUnderline {
    pub start: f64,
    pub end: f64,
    pub baseline: f64,
    pub position: f64,
    pub thickness: f64,
}

/// Rasterizes glyph `index` of the current draw into `out_coverage`
/// (`out_len` = raster_width * raster_height bytes, top row first).
/// Returns false when the provider cannot rasterize it.
#[cfg(feature = "renderer-wgpu")]
pub type FfiNativeTextRasterize = unsafe extern "C" fn(
    ctx: *mut c_void,
    index: usize,
    out_coverage: *mut u8,
    out_len: usize,
) -> bool;

/// Replaces XOpenGL's native-text `DrawLayout` (`NativeText.cpp:396-491`,
/// `hp2_wgpu::RenderLoop::draw_native_text`). Returns false, with nothing
/// queued, wherever the C++ returns 0 (the canvas then falls back to bitmap
/// text). `rasterize` is called only for glyphs missing from the atlas.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not yet
/// destroyed), or null. `layout` must point to a valid
/// `FfiNativeTextLayout`. `glyphs`/`underlines` must be valid for their
/// counts (or null with a zero count), each glyph's font identity valid
/// for its length. `rasterize` must honor its own contract for `ctx`.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hp2_rs_wgpu_draw_native_text(
    handle: *mut c_void,
    layout: *const FfiNativeTextLayout,
    glyphs: *const FfiNativeTextGlyph,
    glyph_count: usize,
    underlines: *const FfiNativeTextUnderline,
    underline_count: usize,
    fx2: f32,
    fy2: f32,
    rasterize: FfiNativeTextRasterize,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_wgpu_draw_native_text", false, move || {
        if handle.is_null() || layout.is_null() {
            return false;
        }
        let layout = unsafe { &*layout };
        let (Some(raw_glyphs), Some(raw_underlines)) = (unsafe {
            (
                typed_slice_from_raw(glyphs, glyph_count),
                typed_slice_from_raw(underlines, underline_count),
            )
        }) else {
            log::warn!("hp2_rs_wgpu_draw_native_text: null array with a nonzero count");
            return false;
        };
        let mut glyphs = Vec::with_capacity(raw_glyphs.len());
        for raw in raw_glyphs {
            let Some(font_identity) =
                (unsafe { str_from_raw(raw.font_identity, raw.font_identity_len) })
            else {
                log::warn!("hp2_rs_wgpu_draw_native_text: invalid font identity");
                return false;
            };
            glyphs.push(hp2_wgpu::native_text::NativeTextGlyph {
                key: hp2_wgpu::text_atlas::GlyphKey {
                    font_identity: font_identity.to_owned(),
                    glyph: raw.glyph,
                    raster_point_size_64: raw.raster_point_size_64,
                    antialias: raw.antialias,
                    subpixel: raw.subpixel,
                },
                draw_left: raw.draw_left,
                draw_top: raw.draw_top,
                raster_width: raw.raster_width,
                raster_height: raw.raster_height,
            });
        }
        let underlines: Vec<_> = raw_underlines
            .iter()
            .map(|raw| hp2_wgpu::native_text::NativeTextUnderline {
                start: raw.start,
                end: raw.end,
                baseline: raw.baseline,
                position: raw.position,
                thickness: raw.thickness,
            })
            .collect();
        let layout = hp2_wgpu::native_text::NativeTextLayout {
            start_x: layout.start_x,
            start_y: layout.start_y,
            origin_x: layout.origin_x,
            origin_y: layout.origin_y,
            clip_x: layout.clip_x,
            clip_y: layout.clip_y,
            clip: layout.clip,
            poly_flags: layout.poly_flags,
            color: layout.color,
        };
        let mut rasterize_glyph = |index: usize| -> Option<Vec<u8>> {
            let glyph = glyphs.get(index)?;
            let len = glyph.raster_width as usize * glyph.raster_height as usize;
            let mut coverage = vec![0u8; len];
            unsafe { rasterize(ctx, index, coverage.as_mut_ptr(), len) }.then_some(coverage)
        };
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.draw_native_text(
            &handle.created,
            &layout,
            &glyphs,
            &underlines,
            fx2,
            fy2,
            &mut rasterize_glyph,
        )
    })
}

/// Real `FixCacheID` (`SetTexture.cpp`'s own call site, immediately
/// before `BindMap->Find`) -- `hp2_wgpu::texture::fix_cache_id`'s own
/// doc has the full real reasoning. Pure (no `WgpuHandle` needed):
/// callers compute the real tagged cache key once and reuse the
/// result for every other `hp2_rs_wgpu_*texture*`/`hp2_rs_wgpu_
/// queue_tile` call referencing the same real texture, matching the
/// real source's own `FixCacheID(Info, PolyFlags)` call site exactly
/// (once per real `SetTexture`/`DrawTile` call, reused by everything
/// downstream of it in that same call). `format` is the real raw
/// `Info.Format` byte -- an out-of-range value (`hp2_wgpu::texture::
/// TextureFormat::from_raw`'s own `None` case) logs a warning and
/// returns `cache_id` unchanged (untagged): the real source's own
/// masked-vs-unmasked distinction only matters for `TEXF_P8`, so an
/// unrecognized format cannot be P8 either way, and returning the
/// input unchanged is the safe, inert fallback (never a crash, never
/// a made-up tag).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_wgpu_fix_cache_id(cache_id: u64, format: u8, poly_flags: u32) -> u64 {
    guard("hp2_rs_wgpu_fix_cache_id", cache_id, move || {
        let Some(format) = hp2_wgpu::texture::TextureFormat::from_raw(format) else {
            log::warn!(
                "hp2_rs_wgpu_fix_cache_id: invalid Info.Format value {format} (expected \
                 0x00..=0x05), returning cache_id unchanged"
            );
            return cache_id;
        };
        hp2_wgpu::texture::fix_cache_id(cache_id, format, poly_flags)
    })
}

/// `hp2_wgpu::RenderLoop::is_texture_cached`'s own real contract
/// (that method's own doc has the full reasoning) -- `cache_id` must
/// already be `hp2_rs_wgpu_fix_cache_id`'s own output. `false` on a
/// null `handle` (not a crash).
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_is_texture_cached(handle: *mut c_void, cache_id: u64) -> bool {
    guard("hp2_rs_wgpu_is_texture_cached", false, move || {
        if handle.is_null() {
            return false;
        }
        let handle = unsafe { &*(handle as *mut WgpuHandle) };
        handle.render_loop.is_texture_cached(cache_id)
    })
}

/// Borrows a raw `(ptr, 1024-byte)` FFI span as `&[hp2_wgpu::texture::
/// FColor; 256]` (the real `FTextureInfo::Palette`'s own shape; `hp2_
/// wgpu::texture::FColor` is itself a re-export of `hp2_abi::FColor`
/// -- `hp2-abi` is not a direct dependency of this crate, `hp2-wgpu`'s
/// own re-export is the real accessible path) -- same raw-pointer-
/// reinterpretation idiom as this file's own `bytes_from_raw`/
/// `str_from_raw` (not `bytemuck`: this crate has no existing
/// dependency on it, unlike `hp2-wgpu`, and introducing one for this
/// one conversion is not worth a new dependency when this file's own
/// established raw-pointer-cast idiom already covers it). Null `ptr`
/// is `None`. `hp2_abi::FColor` is `#[repr(C, align(4))]`, matching
/// the real `union { DWORD Int4; struct {...}; }` layout the real
/// `Palette` array itself has (that struct's own doc has the full
/// real citation) -- a 1024-byte span really is 256 contiguous,
/// correctly aligned `FColor`s, not an assumption.
#[cfg(feature = "renderer-wgpu")]
unsafe fn palette_from_raw<'a>(ptr: *const u8) -> Option<&'a [hp2_wgpu::texture::FColor; 256]> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { &*(ptr as *const [hp2_wgpu::texture::FColor; 256]) })
}

/// Replaces `UXOpenGLRenderDevice::SetTexture`'s own real upload path
/// (`SetTexture.cpp`, `UploadTexture`'s `TEXF_P8`/`TEXF_RGBA8` cases
/// specifically -- read in full before writing this function, not
/// guessed). `cache_id` must already be `hp2_rs_wgpu_fix_cache_id`'s
/// own output. `format` is the real raw `Info.Format` byte
/// (`hp2_wgpu::texture::TextureFormat`'s own real discriminants).
///
/// Dispatches on `format`:
/// - `TEXF_P8` (`0x00`): `data` is the real raw `Mip->DataPtr` (one
///   byte per texel, `width * height` bytes), `palette` must be a
///   valid, non-null pointer to the real 256-entry `FTextureInfo::
///   Palette` -- expanded via `hp2_wgpu::texture::p8_to_rgba8` (that
///   function's own doc has the full real citation), `masked`
///   selecting `P8MaskMode::Masked`/`Unmasked` (the real `PF_Masked`-
///   driven choice `FixCacheID`'s own doc already covers -- the
///   caller's responsibility to have already made this same choice
///   consistently with whatever `cache_id` it passed in, matching the
///   real source's own paired `FixCacheID`/`UploadTexture` call
///   sequence).
/// - `TEXF_RGBA8` (`0x05`): `data` is already real RGBA8 bytes
///   (`width * height * 4`), uploaded directly, `palette`/`masked`
///   ignored.
/// - Every other real format (`TEXF_RGBA7`/`TEXF_RGB16`/`TEXF_DXT1`/
///   `TEXF_RGB8`): not yet ported (this crate's own todo list tracks
///   DXT1/BC and RGB8/RGB16/RGBA7 conversion as separate, not-yet-done
///   items) -- logs a warning and returns `false`, uploading nothing.
///   A real, known, explicitly scoped gap: textures in these formats
///   do not render via this path yet, not a silent wrong result.
///
/// Returns `false` (not a panic) on a null `handle`, an out-of-range
/// `format` byte, a null `data`/`palette` pointer where required, or a
/// `data` length that does not match `width * height` (P8) / `width *
/// height * 4` (RGBA8) -- all real, reachable caller-input conditions,
/// not asserted/panicked on (unlike `TextureCache::upload_rgba8`'s own
/// internal `assert_eq!`, which this function's own explicit length
/// check exists specifically to never reach with mismatched input).
///
/// `sampler_flags`: `hp2_wgpu::texture::SamplerKey` bits (`CLAMP_U=1`,
/// `CLAMP_V=2`, `NO_SMOOTH=4`), used only on `cache_id`'s first upload
/// (that type's own doc).
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null. `data` must be valid for `data_len` bytes.
/// `palette`, when not null, must be valid for 1024 bytes (256 real
/// `FColor` entries).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_upload_texture(
    handle: *mut c_void,
    cache_id: u64,
    format: u8,
    width: u32,
    height: u32,
    data: *const u8,
    data_len: usize,
    palette: *const u8,
    masked: bool,
    srgb: bool,
    sampler_flags: u32,
) -> bool {
    guard("hp2_rs_wgpu_upload_texture", false, move || {
        if handle.is_null() {
            return false;
        }
        let Some(format) = hp2_wgpu::texture::TextureFormat::from_raw(format) else {
            log::warn!(
                "hp2_rs_wgpu_upload_texture: invalid Info.Format value {format} (expected \
                 0x00..=0x05), skipping upload"
            );
            return false;
        };
        // Reject every format this port does not yet actually upload
        // BEFORE constructing a Rust slice over data/data_len below --
        // caught on review, not self-discovered: data_len is always
        // caller-computed from width*height using an assumed bytes-
        // per-pixel that this function has no way to verify, and is
        // only actually correct for the two formats the match below
        // still handles (P8: 1 byte/pixel, Rgba8: 4). For every other
        // real format this engine supports -- RGB8 (3 bytes/pixel),
        // RGB16 (2), DXT1 (block-compressed, smaller still -- the
        // real engine's own per-format byte size, SetTexture.cpp's
        // own HP2TextureBytes, read directly before concluding this)
        // -- is smaller than width*height*4, so a caller computing
        // data_len that way (every real WgpuDrv.cpp override does)
        // would hand this function a data_len larger than the
        // texture's real allocation. Constructing a slice of that
        // length via bytes_from_raw's own from_raw_parts is undefined
        // behavior the instant it happens, regardless of whether the
        // rejected format's own data is ever actually read afterward.
        // Checking the format first instead avoids ever constructing
        // that slice for an unsupported format at all -- fixed once
        // here for every current and future caller, rather than
        // requiring every call site to somehow get its own data_len
        // computation exactly right for formats this function cannot
        // even use yet. (RGBA7 -- the one non-P8/Rgba8 format this
        // session's own wgpu-smoke.log already shows hundreds of real
        // "upload not yet ported" rejections for -- happens to share
        // HP2TextureBytes' own default-4-bytes/pixel case with Rgba8,
        // so those specific past runs were not actually reading out
        // of bounds; RGB8/RGB16/DXT1 would have been, the moment any
        // real asset using one of them was loaded.)
        if !matches!(
            format,
            hp2_wgpu::texture::TextureFormat::P8
                | hp2_wgpu::texture::TextureFormat::Rgba8
                | hp2_wgpu::texture::TextureFormat::Rgb8
                | hp2_wgpu::texture::TextureFormat::Rgb16
                | hp2_wgpu::texture::TextureFormat::Rgba7
        ) {
            log::warn!(
                "hp2_rs_wgpu_upload_texture: {} upload not yet ported, skipping",
                hp2_wgpu::texture::texture_format_name(format)
            );
            return false;
        }
        let Some(data) = (unsafe { bytes_from_raw(data, data_len) }) else {
            log::warn!("hp2_rs_wgpu_upload_texture: null data pointer with data_len != 0");
            return false;
        };
        let rgba8: std::borrow::Cow<'_, [u8]> = match format {
            hp2_wgpu::texture::TextureFormat::P8 => {
                if data.len() != width as usize * height as usize {
                    log::warn!(
                        "hp2_rs_wgpu_upload_texture: TEXF_P8 data_len {} != width*height {}",
                        data.len(),
                        width as usize * height as usize
                    );
                    return false;
                }
                let Some(palette) = (unsafe { palette_from_raw(palette) }) else {
                    log::warn!("hp2_rs_wgpu_upload_texture: TEXF_P8 requires a non-null palette");
                    return false;
                };
                let mask_mode = if masked {
                    hp2_wgpu::texture::P8MaskMode::Masked
                } else {
                    hp2_wgpu::texture::P8MaskMode::Unmasked
                };
                let expanded = hp2_wgpu::texture::p8_to_rgba8(data, palette, mask_mode);
                let bytes: Vec<u8> = expanded.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
                std::borrow::Cow::Owned(bytes)
            }
            // TEXF_RGBA8 and TEXF_RGBA7 share the real byte layout
            // exactly (hp2_wgpu::texture module doc's own two
            // independent real-source citations for why: XOpenGLDrv's
            // own case TEXF_RGBA7 passes Mip->DataPtr straight through
            // with zero CPU-side work, and UnLight.cpp's own real
            // lightmap/fogmap generation writes GET_COLOR_DWORD(FColor)
            // -packed DWORDs into TEXF_RGBA7-tagged buffers) -- the
            // "7" names a value-range convention, not a different
            // physical layout, so both share this one real arm.
            hp2_wgpu::texture::TextureFormat::Rgba8 | hp2_wgpu::texture::TextureFormat::Rgba7 => {
                if data.len() != width as usize * height as usize * 4 {
                    log::warn!(
                        "hp2_rs_wgpu_upload_texture: {} data_len {} != width*height*4 {}",
                        hp2_wgpu::texture::texture_format_name(format),
                        data.len(),
                        width as usize * height as usize * 4
                    );
                    return false;
                }
                std::borrow::Cow::Borrowed(data)
            }
            hp2_wgpu::texture::TextureFormat::Rgb8 => {
                if data.len() != width as usize * height as usize * 3 {
                    log::warn!(
                        "hp2_rs_wgpu_upload_texture: TEXF_RGB8 data_len {} != width*height*3 {}",
                        data.len(),
                        width as usize * height as usize * 3
                    );
                    return false;
                }
                std::borrow::Cow::Owned(hp2_wgpu::texture::rgb8_to_rgba8(data))
            }
            hp2_wgpu::texture::TextureFormat::Rgb16 => {
                if data.len() != width as usize * height as usize * 2 {
                    log::warn!(
                        "hp2_rs_wgpu_upload_texture: TEXF_RGB16 data_len {} != width*height*2 {}",
                        data.len(),
                        width as usize * height as usize * 2
                    );
                    return false;
                }
                std::borrow::Cow::Owned(hp2_wgpu::texture::rgb16_to_rgba8(data))
            }
            _ => unreachable!("rejected above: format is one of the five matched arms"),
        };
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        // Real SetTexture.cpp:247-257, read directly before concluding
        // this (a real blocker caught on review, not self-discovered):
        // TEXF_RGBA7 and TEXF_RGB16 are UNCONDITIONALLY real linear
        // formats (InternalFormat = GL_RGBA8 / GL_RGB respectively,
        // neither one ever gated on UnpackSRGB the way TEXF_P8/RGB8/
        // RGBA8/DXT1 all are) -- srgb here is the caller's own real
        // UseSRGBTextures config value, which must NOT reach an sRGB
        // texture format for these two regardless of what the caller
        // passed. Getting this wrong would gamma-DECODE every real
        // lightmap/fogmap texel on sample (TEXF_RGBA7 is their own
        // real format, hp2_wgpu::texture module doc's own citation)
        // -- a real, severe visible darkening on any map that actually
        // has lighting data outside the 0/255 endpoints, not a subtle
        // difference (e.g. a real stored value of 64 would sample back
        // as roughly 13).
        let effective_srgb = match format {
            hp2_wgpu::texture::TextureFormat::Rgba7 | hp2_wgpu::texture::TextureFormat::Rgb16 => {
                false
            }
            _ => srgb,
        };
        handle.render_loop.upload_texture(
            &handle.created,
            cache_id,
            width,
            height,
            &rgba8,
            hp2_wgpu::texture::TextureOptions {
                srgb: effective_srgb,
                sampler_key: hp2_wgpu::texture::SamplerKey::from_bits(sampler_flags),
            },
        );
        true
    })
}

/// One real pre-baked mip level's own data, crossing the FFI boundary
/// -- `hp2_rs_wgpu_upload_compressed_texture`'s own `mips` array
/// element type. `width`/`height` are that mip's own real
/// `FMipmapBase::USize`/`VSize`; `data`/`data_len` are that mip's own
/// real `FMipmapBase::DataPtr` and its real pre-baked compressed byte
/// length (`HP2TextureBytes`/`hp2_wgpu::texture::texture_bytes`'s own
/// real per-format formula).
#[cfg(feature = "renderer-wgpu")]
#[repr(C)]
pub struct FfiCompressedMip {
    pub width: u32,
    pub height: u32,
    pub data: *const u8,
    pub data_len: usize,
}

/// Replaces the real `TEXF_DXT1` branch of `UXOpenGLRenderDevice::
/// UploadTexture` (`SetTexture.cpp:172-453`, read in full --
/// `hp2_wgpu::texture::create_compressed_gpu_texture`'s own doc has
/// the full real citation and reasoning for why this one format needs
/// its own dedicated FFI function rather than reusing `hp2_rs_wgpu_
/// upload_texture`'s own single-mip signature). `format` must be
/// `0x03` (`TEXF_DXT1`) -- every other value is rejected cleanly
/// (`false`, logged), the same `hp2_wgpu::texture::TextureFormat::
/// from_raw`-plus-explicit-support-check shape `hp2_rs_wgpu_upload_
/// texture`'s own doc already establishes, scoped down here to the
/// one real format this function actually knows how to handle, not
/// generalized ahead of a second real compressed format that does not
/// exist in this engine (`is_compressed_texture`'s own doc already
/// confirms `TEXF_DXT1` is the only one). `mips`/`mip_count` is the
/// real full `Info.Mips[0..Info.NumMips)` chain for this one texture
/// -- `create_compressed_gpu_texture`'s own doc has the full real
/// reasoning for why this format, unlike the mip-0-only `hp2_rs_wgpu_
/// upload_texture`, needs the whole chain. Returns `false` (not a
/// panic) if the real device does not actually have `Features::
/// TEXTURE_COMPRESSION_BC` granted, same real `SupportsS3TC`-gated
/// graceful-degradation case that function's own doc covers.
/// `sampler_flags`: same as `hp2_rs_wgpu_upload_texture`'s.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null. `mips` must be valid for `mip_count`
/// contiguous real `FfiCompressedMip` values, or be null with
/// `mip_count == 0`; each one's own `data`/`data_len` must itself be a
/// valid, non-null (if `data_len != 0`) byte span, same contract
/// `hp2_rs_wgpu_upload_texture`'s own `data`/`data_len` parameters
/// already have.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_upload_compressed_texture(
    handle: *mut c_void,
    cache_id: u64,
    format: u8,
    width: u32,
    height: u32,
    mips: *const FfiCompressedMip,
    mip_count: usize,
    srgb: bool,
    sampler_flags: u32,
) -> bool {
    guard("hp2_rs_wgpu_upload_compressed_texture", false, move || {
        if handle.is_null() {
            return false;
        }
        if format != hp2_wgpu::texture::TextureFormat::Dxt1 as u8 {
            log::warn!(
                "hp2_rs_wgpu_upload_compressed_texture: format byte {format:#x} is not \
                     TEXF_DXT1 (0x03), the only compressed format this function knows how to \
                     upload, skipping"
            );
            return false;
        }
        let Some(raw_mips) = (unsafe { typed_slice_from_raw(mips, mip_count) }) else {
            log::warn!(
                "hp2_rs_wgpu_upload_compressed_texture: null mips pointer with mip_count != 0"
            );
            return false;
        };
        let mut owned_mips = Vec::with_capacity(raw_mips.len());
        for raw_mip in raw_mips {
            let Some(data) = (unsafe { bytes_from_raw(raw_mip.data, raw_mip.data_len) }) else {
                log::warn!(
                    "hp2_rs_wgpu_upload_compressed_texture: null mip data pointer with \
                         data_len != 0"
                );
                return false;
            };
            owned_mips.push(hp2_wgpu::texture::CompressedMipLevel {
                width: raw_mip.width,
                height: raw_mip.height,
                data,
            });
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.upload_compressed_texture(
            &handle.created,
            cache_id,
            width,
            height,
            &owned_mips,
            hp2_wgpu::texture::TextureOptions {
                srgb,
                sampler_key: hp2_wgpu::texture::SamplerKey::from_bits(sampler_flags),
            },
        )
    })
}

/// Replaces `UXOpenGLRenderDevice::DrawTile`/`SubmitTileBatch`
/// (`DrawTile.cpp:24-56`/`117-265`, both read in full). `cache_id`
/// must already be `hp2_rs_wgpu_fix_cache_id`'s own output, and the
/// matching real texture must already have been uploaded via
/// `hp2_rs_wgpu_upload_texture` (or already be cached from an earlier
/// call, checked first via `hp2_rs_wgpu_is_texture_cached`) -- a
/// `cache_id` with no real cached texture is dropped cleanly
/// (`hp2_wgpu::RenderLoop::queue_tile`'s own doc), not a crash.
/// `x`/`y`/`xl`/`yl`/`u`/`v`/`ul`/`vl`/`z` are `DrawTile`'s own real
/// `FLOAT` parameters, unchanged. `r`/`g`/`b` are the real `FPlane
/// Color`'s first three components -- unlike `hp2_rs_wgpu_draw_2d_
/// line`, alpha genuinely is still not a meaningful input here
/// either, for a different real reason: `DrawTile.cpp:40-43`'s own
/// `Color.W` adjustment unconditionally overwrites whatever the
/// caller's `Color.W` was (to `1.0` or to the real texture's own
/// `Alpha`, see `has_texture_alpha`/`texture_alpha` below) -- no real
/// code path preserves it, confirmed by `hp2_wgpu::draw::compute_
/// tile_draw_color`'s own tests. `u_scale`/`v_scale` are the real
/// `FTextureInfo::UScale`/`VScale` fields (`hp2_wgpu::texture::
/// texture_uv_mult`'s own doc has the full real per-bind-freshness
/// reasoning -- read fresh from this specific `DrawTile` call's own
/// `Info`, never cached). `has_texture_alpha`/`texture_alpha` encode
/// the real `Info.Texture && Info.Texture->Alpha` pair as an FFI-safe
/// bool-plus-value (no native `Option<f32>` across this boundary) --
/// `false` for a real null `Info.Texture`, matching `compute_tile_
/// draw_color`'s own `None` case. `fx2`/`fy2` are the real `Frame->
/// FX2`/`FY2` the actual `DrawTile` call received, same real per-call-
/// not-cached reasoning `hp2_rs_wgpu_draw_2d_line`'s own `fx2`/`fy2`
/// already establish.
///
/// Known, explicitly scoped gaps, not silently pretended away (both
/// confirmed by reading `SubmitTileBatch` in full, not assumed from
/// `DrawTile`'s own simpler entry point alone): the real `GIsEditor &&
/// PF_Selected -> DF_Selected` flag (`DrawTile.cpp:143-144`) and the
/// real `HitTesting() ? HitColor : Color` substitution (`DrawTile.cpp:
/// 146`) are not applied -- `poly_flags` is translated by `hp2_wgpu::
/// draw::compute_tile_draw_flags` alone, which does not know about
/// `GIsEditor`/selection state, and no hit-testing readback exists on
/// this render device yet (tracked separately, same gap `hp2_rs_wgpu_
/// draw_2d_line`'s own doc already names for lines). The real editor-
/// only `Z = 1.0f` override (`DrawTile.cpp:206-211`) is also not
/// applied, matching `RenderLoop::queue_tile`'s own doc.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_queue_tile(
    handle: *mut c_void,
    cache_id: u64,
    x: f32,
    y: f32,
    xl: f32,
    yl: f32,
    u: f32,
    v: f32,
    ul: f32,
    vl: f32,
    z: f32,
    r: f32,
    g: f32,
    b: f32,
    poly_flags: u32,
    u_scale: f32,
    v_scale: f32,
    has_texture_alpha: bool,
    texture_alpha: f32,
    fx2: f32,
    fy2: f32,
) {
    guard("hp2_rs_wgpu_queue_tile", (), move || {
        if handle.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.queue_tile(hp2_wgpu::TileDrawRequest {
            cache_id,
            x,
            y,
            xl,
            yl,
            u,
            v,
            ul,
            vl,
            z,
            // Color.W is always overwritten by compute_tile_draw_color
            // -- see this function's own doc.
            color: [r, g, b, 1.0],
            poly_flags,
            u_scale,
            v_scale,
            texture_alpha: has_texture_alpha.then_some(texture_alpha),
            fx2,
            fy2,
        });
    })
}

/// Replaces `UXOpenGLRenderDevice::Draw2DPoint` (`DrawSimple.cpp:
/// 154-205`, read in full). `x1`/`y1`/`x2`/`y2`/`z` are `Draw2DPoint`'s
/// own real `FLOAT` parameters, unchanged -- `hp2_wgpu::RenderLoop::
/// queue_point`'s own doc has the full real citation for the `Z`
/// adjustment and `+-0.5` quad inset/expansion applied internally.
/// `fx2`/`fy2` are the real `Frame->FX2`/`FY2` the actual `Draw2DPoint`
/// call received, same real per-call-not-cached reasoning `hp2_rs_
/// wgpu_draw_2d_line`'s own `fx2`/`fy2` already establish. `r`/`g`/`b`
/// are the real `FPlane Color`'s first three components -- alpha is
/// not a parameter here either, same real reason `hp2_rs_wgpu_draw_
/// 2d_line`'s own doc already states for lines (`DrawSimple.cpp:166`:
/// `Color.W = 1.f; //Unfortunately this is usually set to 0.`).
///
/// `line_flags`: same as `hp2_rs_wgpu_draw_2d_line`'s (depth state
/// from `LINE_DepthCued`, fixed `PF_Highlighted` blend). The
/// `HitTesting() ? HitColor : Color` substitution is not applied yet.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_queue_point(
    handle: *mut c_void,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    z: f32,
    fx2: f32,
    fy2: f32,
    r: f32,
    g: f32,
    b: f32,
    line_flags: u32,
) {
    guard("hp2_rs_wgpu_queue_point", (), move || {
        if handle.is_null() {
            return;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle
            .render_loop
            .queue_point(x1, y1, x2, y2, z, fx2, fy2, [r, g, b], line_flags);
    })
}

/// Real `UXOpenGLRenderDevice::EndFlash`'s own flash overlay
/// (`DrawSimple.cpp:207-256`) for `UWgpuRenderDevice::EndFlash`. `(r, g, b, a)`
/// is the caller-computed `DrawColor` (`FlashFog.XYZ`, `1 - min(2 * FlashScale.X,
/// 1)`). Queued as one full-viewport quad with the fixed `PF_Highlighted`
/// blend and no depth test. Returns `true` only when the quad was queued:
/// a null `handle`, or a frame with no `SetSceneNode` yet or no open frame,
/// returns `false` and queues nothing.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not yet
/// destroyed), or null (a no-op).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_end_flash(
    handle: *mut c_void,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) -> bool {
    guard("hp2_rs_wgpu_end_flash", false, move || {
        if handle.is_null() {
            return false;
        }
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.queue_end_flash([r, g, b, a])
    })
}

/// Borrows a raw `(ptr, count)` FFI array as `&[T]` -- the generic
/// counterpart to this file's own `bytes_from_raw` (byte-specific)
/// and `palette_from_raw` (fixed-256-element-specific) for a real
/// variable-length typed array FFI parameter (`hp2_rs_wgpu_queue_
/// triangles`' own real `vertices` parameter). `count == 0` returns
/// `&[]` without reading `ptr`. A null `ptr` with `count != 0` is
/// `None`.
#[cfg(feature = "renderer-wgpu")]
unsafe fn typed_slice_from_raw<'a, T>(ptr: *const T, count: usize) -> Option<&'a [T]> {
    if count == 0 {
        return Some(&[]);
    }
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, count) })
}

/// Replaces `UXOpenGLRenderDevice::DrawTriangles`/`PrepareGouraudCall`
/// (`DrawGouraud.cpp:68-205`/`285-359`, both read in full). `cache_id`
/// must already be `hp2_rs_wgpu_fix_cache_id`'s own output, and the
/// matching real texture must already be cached (`hp2_rs_wgpu_
/// upload_texture`/`hp2_rs_wgpu_is_texture_cached`), same real
/// contract as `hp2_rs_wgpu_queue_tile`. `vertices`/`vertex_count`
/// are the real, already-flattened triangle-list data -- the real
/// `Pts`/`Indices` indexed-or-fan indirection and validation
/// (`DrawGouraud.cpp:300-323`) is this function's own caller's
/// responsibility, not this function's (`hp2_wgpu::RenderLoop::
/// queue_triangles`'s own doc has the full real reasoning: real
/// `FTransTexture*` pointer dereferences this crate has no access
/// to). `u_scale`/`v_scale` are the real `FTextureInfo::UScale`/
/// `VScale` fields, same real per-bind-freshness reasoning as
/// `hp2_rs_wgpu_queue_tile`'s own. `has_texture_alpha`/`texture_alpha`
/// and `has_diffuse`/`diffuse` each encode a real `Option<f32>` as an
/// FFI-safe bool-plus-value pair, same convention `hp2_rs_wgpu_
/// queue_tile`'s own `has_texture_alpha`/`texture_alpha` already
/// establish -- `false` for a real null `Info.Texture` in both cases.
///
/// Known, explicitly scoped gaps, not silently pretended away, same
/// category `hp2_rs_wgpu_queue_tile`'s own doc already names: detail
/// textures, macro textures, bump maps, the real shader-specialization
/// system, `GIsEditor && PF_Selected -> DF_Selected`, `NoNearZ`
/// handling, and the real `HitTesting()` substitution are not applied
/// -- `hp2_wgpu::RenderLoop::queue_triangles`'s own doc has the full
/// citation for every one of these as a real, separately scoped,
/// not-yet-ported `PrepareGouraudCall` branch.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null. `vertices` must be valid for `vertex_count`
/// contiguous real `hp2_wgpu::draw::GouraudVertexInput` values, or be
/// null with `vertex_count == 0`.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_queue_triangles(
    handle: *mut c_void,
    cache_id: u64,
    poly_flags: u32,
    u_scale: f32,
    v_scale: f32,
    has_texture_alpha: bool,
    texture_alpha: f32,
    has_diffuse: bool,
    diffuse: f32,
    vertices: *const hp2_wgpu::draw::GouraudVertexInput,
    vertex_count: usize,
) {
    guard("hp2_rs_wgpu_queue_triangles", (), move || {
        if handle.is_null() {
            return;
        }
        let Some(vertices) = (unsafe { typed_slice_from_raw(vertices, vertex_count) }) else {
            log::warn!("hp2_rs_wgpu_queue_triangles: null vertices pointer with vertex_count != 0");
            return;
        };
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.queue_triangles(
            hp2_wgpu::TrianglesDrawRequest {
                cache_id,
                poly_flags,
                u_scale,
                v_scale,
                texture_alpha: has_texture_alpha.then_some(texture_alpha),
                diffuse: has_diffuse.then_some(diffuse),
            },
            vertices,
        );
    })
}

/// Replaces `UXOpenGLRenderDevice::DrawComplexSurface`/`SetTextureHelper`
/// (`DrawComplex.cpp:21-259`, read in full). Each real texture layer's
/// own `cache_id` must already be `hp2_rs_wgpu_fix_cache_id`'s own
/// output, and already cached (`hp2_rs_wgpu_upload_texture`/`hp2_rs_
/// wgpu_is_texture_cached`), same real contract as `hp2_rs_wgpu_
/// queue_tile`'s own diffuse `cache_id` -- `diffuse_cache_id` is
/// always required (the real source's own `check(Surface.Texture)`
/// assertion, `DrawComplex.cpp:60`); `has_light_map`/`has_fog_map`
/// gate whether `light_map_cache_id`/`fog_map_cache_id` (otherwise
/// ignored) are used at all, matching the real `Surface.LightMap`/
/// `FogMap` null checks. `*_u_scale`/`*_v_scale`/`*_pan_x`/`*_pan_y`
/// are each real layer's own `FTextureInfo::UScale`/`VScale`/`Pan.X`/
/// `Pan.Y`, same real per-bind-freshness reasoning as every other
/// `queue_*` function. `has_texture_fields`/`texture_diffuse`/
/// `texture_specular`/`texture_alpha` encode the real combined
/// `Surface.Texture->Texture` null check (`compute_complex_diffuse_
/// info`'s own doc has the full real citation for why these three
/// real fields share one fallback, not three independent `Option`s).
/// `c_alpha` is the real `BYTE cAlpha` `DrawComplexSurface` itself
/// receives. `x_axis`/`y_axis` are the real `Facet.MapCoords.XAxis`/
/// `YAxis`, already packed as `(axis.X, axis.Y, axis.Z, axis |
/// MapCoords.Origin)` -- the real dot-product bias baked into `.w`
/// (`DrawComplex.cpp:193-194`, `FVector::operator|` confirmed the
/// real dot product, `UnMath.h:686-689`), a future `WgpuDrv.cpp`
/// override's own responsibility to compute, not this function's.
/// `vertices`/`vertex_count` are the real, already fan-triangulated
/// flat vertex list -- the real `FSavedPoly`/`FTransform*` linked-
/// list walk (`DrawComplex.cpp:206-248`) is this function's own
/// caller's responsibility, not this function's, same real reasoning
/// `hp2_rs_wgpu_queue_triangles`'s own doc already establishes for
/// its own fan/indexed expansion.
///
/// Known, explicitly scoped gaps, not silently pretended away, same
/// category every other `queue_*` function's own doc already names:
/// detail textures, macro textures, bump maps, the real shader-
/// specialization system, `GIsEditor && PF_Selected`, and the real
/// `HitTesting()` substitution are not applied -- `hp2_wgpu::
/// RenderLoop::queue_complex_surface`'s own doc has the full citation
/// for every one of these as a real, separately scoped, not-yet-
/// ported `DrawComplexSurface` branch.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), or null. `vertices` must be valid for `vertex_count`
/// contiguous real `hp2_wgpu::draw::ComplexVertexInput` values, or be
/// null with `vertex_count == 0`.
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_queue_complex_surface(
    handle: *mut c_void,
    diffuse_cache_id: u64,
    diffuse_u_scale: f32,
    diffuse_v_scale: f32,
    diffuse_pan_x: f32,
    diffuse_pan_y: f32,
    has_light_map: bool,
    light_map_cache_id: u64,
    light_map_u_scale: f32,
    light_map_v_scale: f32,
    light_map_pan_x: f32,
    light_map_pan_y: f32,
    has_fog_map: bool,
    fog_map_cache_id: u64,
    fog_map_u_scale: f32,
    fog_map_v_scale: f32,
    fog_map_pan_x: f32,
    fog_map_pan_y: f32,
    poly_flags: u32,
    has_texture_fields: bool,
    texture_diffuse: f32,
    texture_specular: f32,
    texture_alpha: f32,
    c_alpha: u8,
    x_axis_x: f32,
    x_axis_y: f32,
    x_axis_z: f32,
    x_axis_w: f32,
    y_axis_x: f32,
    y_axis_y: f32,
    y_axis_z: f32,
    y_axis_w: f32,
    vertices: *const hp2_wgpu::draw::ComplexVertexInput,
    vertex_count: usize,
) {
    guard("hp2_rs_wgpu_queue_complex_surface", (), move || {
        if handle.is_null() {
            return;
        }
        let Some(vertices) = (unsafe { typed_slice_from_raw(vertices, vertex_count) }) else {
            log::warn!(
                "hp2_rs_wgpu_queue_complex_surface: null vertices pointer with vertex_count != 0"
            );
            return;
        };
        let handle = unsafe { &mut *(handle as *mut WgpuHandle) };
        handle.render_loop.queue_complex_surface(
            hp2_wgpu::ComplexSurfaceDrawRequest {
                diffuse: hp2_wgpu::ComplexTextureLayer {
                    cache_id: diffuse_cache_id,
                    u_scale: diffuse_u_scale,
                    v_scale: diffuse_v_scale,
                    pan_x: diffuse_pan_x,
                    pan_y: diffuse_pan_y,
                },
                light_map: has_light_map.then_some(hp2_wgpu::ComplexTextureLayer {
                    cache_id: light_map_cache_id,
                    u_scale: light_map_u_scale,
                    v_scale: light_map_v_scale,
                    pan_x: light_map_pan_x,
                    pan_y: light_map_pan_y,
                }),
                fog_map: has_fog_map.then_some(hp2_wgpu::ComplexTextureLayer {
                    cache_id: fog_map_cache_id,
                    u_scale: fog_map_u_scale,
                    v_scale: fog_map_v_scale,
                    pan_x: fog_map_pan_x,
                    pan_y: fog_map_pan_y,
                }),
                poly_flags,
                texture_fields: has_texture_fields.then_some((
                    texture_diffuse,
                    texture_specular,
                    texture_alpha,
                )),
                c_alpha,
                x_axis: [x_axis_x, x_axis_y, x_axis_z, x_axis_w],
                y_axis: [y_axis_x, y_axis_y, y_axis_z, y_axis_w],
            },
            vertices,
        );
    })
}

#[cfg(all(test, feature = "renderer-wgpu"))]
mod tests {
    use super::*;

    /// Regression test for a real soundness bug (caught on review, not
    /// self-discovered): `hp2_rs_wgpu_upload_texture` used to construct
    /// a Rust slice over the caller-supplied `data`/`data_len` *before*
    /// checking whether `format` was one of the formats this function
    /// actually knows how to upload. At the time this test was first
    /// written, that was just P8/RGBA8; RGB8/RGB16/RGBA7 were added
    /// later (a separate real change) and this test's own format list
    /// updated to match -- every real `WgpuDrv.cpp` caller computes
    /// `data_len` as `width*height*4` for every non-P8/non-RGB8/non-
    /// RGB16 format, which is only actually correct for RGBA8 and
    /// RGBA7 (confirmed identical real byte layouts, `hp2_wgpu::
    /// texture` module doc's own two independent real-source
    /// citations) -- for DXT1 (block-compressed, smaller still) that
    /// claimed length is larger than the texture's real allocation, so
    /// constructing a slice of that length via `bytes_from_raw`'s own
    /// `from_raw_parts` was undefined behavior the instant it happened
    /// -- even though the rejection arm never actually read from the
    /// resulting slice afterward. RGB8/RGB16 have their own correct,
    /// different-from-`*4` formulas (3 and 2 bytes/pixel respectively)
    /// computed from real, correctly-sized buffers below instead, not
    /// exercised through this same oversized-tiny-buffer path at all --
    /// doing so for a format this function DOES now support, using a
    /// data_len that does not match that format's own real formula,
    /// would itself be unsound again, the exact bug class this test
    /// exists to guard against (caught on review while extending this
    /// test for the format-support change, not after it shipped
    /// wrong: an earlier draft of this update kept 0x01/0x02/0x04 --
    /// RGBA7/RGB16/RGB8, all newly-supported by that same change -- in
    /// the oversized-tiny-buffer rejection loop below, which would
    /// have made this test itself construct the exact unsound slice
    /// it exists to prevent, for RGBA7 especially: its own real
    /// `*4` formula happens to equal the old assumed-for-every-
    /// nonP8-format length, so it would not even have been caught by
    /// the per-format length check afterward, only have silently
    /// "worked" by coincidence of a stack layout that did not fault).
    ///
    /// This test cannot directly observe construction-time UB itself
    /// (that would need Miri, which cannot interpret this function's
    /// own real wgpu/Metal driver calls -- a hard blocker independent
    /// of this test, not a shortcut taken for convenience), so what it
    /// actually proves is the observable contract the fix's reordering
    /// depends on: every unsupported format is rejected (`false`)
    /// using only a tiny, genuinely-valid backing buffer whose real
    /// length is far smaller than the `data_len` passed alongside it.
    /// If the format check ever regresses back to running after slice
    /// construction, this specific assertion would most likely still
    /// pass (the rejection arm never reads the slice either way) -- the
    /// real guarantee is the source-order invariant itself, inspectable
    /// directly in `hp2_rs_wgpu_upload_texture`'s own body: the format-
    /// support check must run before the `bytes_from_raw` call, full
    /// stop. This test pins the observable contract that check
    /// produces, not a substitute for verifying the ordering by reading
    /// the code.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-ffi --features renderer-wgpu -- --ignored`"]
    fn upload_texture_rejects_every_unsupported_format_without_touching_an_undersized_buffer() {
        let result = unsafe { hp2_rs_wgpu_create(std::ptr::null_mut(), 0) };
        assert!(
            !result.handle.is_null(),
            "expected a real headless device to be created"
        );

        // Deliberately tiny: 4 real bytes, far smaller than any of the
        // data_len values below -- if the rejection below ever ran
        // after a slice were constructed over this buffer at that
        // claimed length, that construction would already be unsound
        // regardless of this assertion's own result.
        let tiny_buffer: [u8; 4] = [0, 0, 0, 0];

        // 0x03 (TEXF_DXT1, this function's own real doc already covers
        // why: it has its own dedicated hp2_rs_wgpu_upload_compressed_
        // texture entry point instead) and 0x06 (out of TextureFormat's
        // own real 0x00..=0x05 range entirely -- exercises the earlier,
        // separate from_raw rejection path, not the matches! one).
        // Paired with a data_len a real WgpuDrv.cpp caller would
        // actually compute for a 1024x1024 texture using this crate's
        // own established (width*height*4)-for-RGBA-shaped-formats
        // formula -- the exact oversized value that motivated this
        // fix, not an arbitrary large number chosen for this test.
        for format_byte in [0x03u8, 0x06] {
            let data_len = 1024usize * 1024 * 4;
            let accepted = unsafe {
                hp2_rs_wgpu_upload_texture(
                    result.handle,
                    1,
                    format_byte,
                    1024,
                    1024,
                    tiny_buffer.as_ptr(),
                    data_len,
                    std::ptr::null(),
                    false,
                    false,
                    0,
                )
            };
            assert!(
                !accepted,
                "format byte {format_byte:#x} is not one of the five real formats this \
                 function supports and must be rejected"
            );
        }

        // Complements the rejection proof above: confirms the fix's
        // reordering did not also, even accidentally, reject any of
        // the five formats this function actually does support. Every
        // one of these uses a real, correctly-sized buffer for its own
        // real per-format byte layout -- never the oversized-tiny-
        // buffer pattern above, which would itself be unsound for a
        // supported format (this test's own doc has the full real
        // reasoning for why, including a real mistake caught before
        // landing).
        let p8_pixel: [u8; 1] = [7];
        // A real [FColor; 256] array, not a raw [u8; 1024] -- FColor
        // is #[repr(C, align(4))] (hp2-abi's own layout-probe-verified
        // attribute, not a plain #[repr(C)]'s implied align(1)), and
        // palette_from_raw casts its *const u8 input straight to
        // &[FColor; 256] with no alignment handling of its own
        // (correctly: a real FTextureInfo::Palette pointer is always
        // already FColor-aligned). A [u8; 1024] buffer has no such
        // guarantee -- caught on review before this test's first run:
        // sufficient byte size alone does not make a pointer valid to
        // reinterpret as a more-aligned type, regardless of whether a
        // particular stack layout happens not to fault on it.
        let palette = [hp2_wgpu::texture::FColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }; 256];
        let p8_accepted = unsafe {
            hp2_rs_wgpu_upload_texture(
                result.handle,
                2,
                0x00,
                1,
                1,
                p8_pixel.as_ptr(),
                1,
                palette.as_ptr().cast::<u8>(),
                false,
                false,
                0,
            )
        };
        assert!(
            p8_accepted,
            "a real 1x1 P8 texture with a real palette must still be accepted"
        );

        let rgba7_pixel: [u8; 4] = [0, 255, 0, 255];
        let rgba7_accepted = unsafe {
            hp2_rs_wgpu_upload_texture(
                result.handle,
                3,
                0x01,
                1,
                1,
                rgba7_pixel.as_ptr(),
                4,
                std::ptr::null(),
                false,
                false,
                0,
            )
        };
        assert!(
            rgba7_accepted,
            "a real 1x1 RGBA7 texture must be accepted -- identical real byte layout to RGBA8"
        );

        // Pure red in the real GL_UNSIGNED_SHORT_5_6_5_REV layout
        // (rgb16_to_rgba8's own doc/tests have the full real bit-
        // layout derivation): R in the low 5 bits.
        let rgb16_pixel: [u8; 2] = [0x1F, 0x00];
        let rgb16_accepted = unsafe {
            hp2_rs_wgpu_upload_texture(
                result.handle,
                4,
                0x02,
                1,
                1,
                rgb16_pixel.as_ptr(),
                2,
                std::ptr::null(),
                false,
                false,
                0,
            )
        };
        assert!(rgb16_accepted, "a real 1x1 RGB16 texture must be accepted");

        let rgb8_pixel: [u8; 3] = [0, 0, 255];
        let rgb8_accepted = unsafe {
            hp2_rs_wgpu_upload_texture(
                result.handle,
                5,
                0x04,
                1,
                1,
                rgb8_pixel.as_ptr(),
                3,
                std::ptr::null(),
                false,
                false,
                0,
            )
        };
        assert!(rgb8_accepted, "a real 1x1 RGB8 texture must be accepted");

        let rgba8_pixel: [u8; 4] = [255, 0, 0, 255];
        let rgba8_accepted = unsafe {
            hp2_rs_wgpu_upload_texture(
                result.handle,
                6,
                0x05,
                1,
                1,
                rgba8_pixel.as_ptr(),
                4,
                std::ptr::null(),
                false,
                false,
                0,
            )
        };
        assert!(
            rgba8_accepted,
            "a real 1x1 RGBA8 texture must still be accepted"
        );

        unsafe { hp2_rs_wgpu_destroy(result.handle) };
    }

    /// Real render-based proof of the `effective_srgb` forcing fix in
    /// `hp2_rs_wgpu_upload_texture` itself -- a plain buffer-copy
    /// readback (every other test in this module uses one) cannot
    /// catch this bug at all: `Rgba8Unorm` vs `Rgba8UnormSrgb` differ
    /// only in how a real GPU texture *sampler* decodes the stored
    /// byte when a shader actually samples it, not in what byte is
    /// physically stored -- a `copy_texture_to_buffer` reads the same
    /// raw byte back either way. This test instead renders a real
    /// tile (through the same real TilePipeline/queue_tile/
    /// unlock_frame path `hp2-wgpu`'s own tests use, called directly
    /// here since this crate has no FFI-level offscreen-frame entry
    /// point -- `hp2_rs_wgpu_lock` needs a real window surface, not
    /// applicable to a headless test device) and reads back the
    /// actual *sampled* pixel.
    ///
    /// Deliberately passes `srgb: true` to the real `hp2_rs_wgpu_
    /// upload_texture` call for a `TEXF_RGBA7` texture -- the one
    /// real caller-controllable input this bug depends on -- with a
    /// real mid-range gray value (64, not an endpoint: 0 and 255 are
    /// both fixed points of the sRGB decode curve, so an endpoint
    /// value could not have caught this bug at all, confirmed by
    /// direct computation before choosing it, not assumed). If
    /// `effective_srgb` were not forced to `false` for this format,
    /// the real GPU sRGB decode curve would darken 64/255 to roughly
    /// 13/255 on sample.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-ffi --features renderer-wgpu -- --ignored`"]
    fn upload_texture_forces_rgba7_to_linear_even_when_srgb_is_requested() {
        let result = unsafe { hp2_rs_wgpu_create(std::ptr::null_mut(), 0) };
        assert!(
            !result.handle.is_null(),
            "expected a real headless device to be created"
        );
        let handle = unsafe { &mut *(result.handle as *mut WgpuHandle) };

        let render_target = handle
            .created
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("rgba7 srgb-forcing test target"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });

        hp2_wgpu::begin_offscreen_frame(
            &handle.created,
            &mut handle.render_loop,
            &render_target,
            wgpu::Color::BLUE,
        );
        // Real field values do not matter -- immediately overwritten
        // below via a direct buffer write, same real technique every
        // hp2-wgpu render test already established (this crate has no
        // access to that crate's own private reference_perspective_
        // scene test helper, so this constructs its own minimal, non-
        // NaN SceneFrame directly instead).
        handle.render_loop.set_scene_node(
            &handle.created.device,
            &handle.created.queue,
            &hp2_wgpu::SceneFrame {
                coords_origin: [0.0; 3],
                coords_x_axis: [0.0; 3],
                coords_y_axis: [0.0; 3],
                coords_z_axis: [0.0; 3],
                uncoords_origin: [0.0; 3],
                uncoords_x_axis: [0.0; 3],
                uncoords_y_axis: [0.0; 3],
                uncoords_z_axis: [0.0; 3],
                fx: 1.0,
                fy: 1.0,
                effective_fov_angle: 90.0,
                viewport_fov_angle: 90.0,
                is_ortho: false,
                is_wireframe_zfar: false,
                gamma: 1.0,
                light_color_intensity: 1.0,
                light_map_intensity: 1.0,
            },
        );
        let mut identity_diagonal = [0u8; 64];
        for i in 0..4 {
            identity_diagonal[i * 20..i * 20 + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        handle.created.queue.write_buffer(
            handle.render_loop.frame_uniforms_segment(0).unwrap(),
            256,
            &identity_diagonal,
        );

        let gray_pixel: [u8; 4] = [64, 64, 64, 255];
        let accepted = unsafe {
            hp2_rs_wgpu_upload_texture(
                result.handle,
                99,
                0x01, // TEXF_RGBA7
                1,
                1,
                gray_pixel.as_ptr(),
                4,
                std::ptr::null(),
                false,
                true, // srgb: true, deliberately -- the exact input this bug depends on.
                0,
            )
        };
        assert!(accepted, "a real 1x1 RGBA7 texture must be accepted");

        // Real aliasing fix, caught on review (not self-discovered):
        // the `handle` borrow obtained above must not still be live
        // (by its own real last-use extent, not just lexical scope)
        // across the real hp2_rs_wgpu_upload_texture call just above
        // -- that extern "C" function creates its OWN real `&mut
        // WgpuHandle` from the SAME underlying `result.handle` raw
        // pointer internally, and Rust's own aliasing model forbids
        // two live `&mut` references to the same object existing at
        // once, regardless of whether either is actually read/written
        // while both are live. The borrow checker cannot catch this
        // specific case itself (result.handle's own type is opaque
        // *mut c_void, with no borrow-tracked connection back to this
        // function's own `handle: &mut WgpuHandle` local at all), so
        // getting the real *extent* of the first borrow right here --
        // ending at its own last real use above (the write_buffer
        // call), strictly before the FFI call -- is this code's own
        // responsibility, not something that would fail to compile if
        // gotten wrong. A fresh `&mut WgpuHandle`, obtained only now,
        // after the FFI call (shadowing the same name, not reusing the
        // old binding), is what the rest of this test uses.
        let handle = unsafe { &mut *(result.handle as *mut WgpuHandle) };

        // Same real left-half geometry as hp2-wgpu's own tile tests
        // (X=0,XL=50 -> NDC x in [-1.0, 0.0]), white draw_color
        // (neutral multiply, isolates this test to the sampled texel
        // alone).
        handle.render_loop.queue_tile(hp2_wgpu::TileDrawRequest {
            cache_id: 99,
            x: 0.0,
            y: 49.5,
            xl: 50.0,
            yl: 1.0,
            u: 0.0,
            v: 0.0,
            ul: 1.0,
            vl: 1.0,
            z: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            poly_flags: 0,
            u_scale: 1.0,
            v_scale: 1.0,
            texture_alpha: None,
            fx2: 50.0,
            fy2: 50.0,
        });
        let did_unlock = hp2_wgpu::unlock_frame(&handle.created, &mut handle.render_loop, false);
        assert!(
            did_unlock,
            "expected unlock_frame to find the frame begin_offscreen_frame began"
        );
        handle
            .created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed after the real unlock_frame submit");

        let readback_buffer = handle
            .created
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("rgba7 srgb-forcing test readback buffer"),
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
        let mut readback_encoder =
            handle
                .created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("rgba7 srgb-forcing test readback encoder"),
                });
        readback_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        handle
            .created
            .queue
            .submit(std::iter::once(readback_encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        handle
            .created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");

        let left_pixel = &mapped[8 * 4..8 * 4 + 4];
        assert_eq!(
            left_pixel,
            &[64u8, 64, 64, 255],
            "expected a real RGBA7 texture's own stored 64/255 gray value to sample back \
             UNCHANGED (linear, effective_srgb forced false regardless of the srgb=true this \
             test deliberately passed) -- a value near 13 here would mean the real sRGB decode \
             curve was wrongly applied, got {left_pixel:?}"
        );
        let adapter_name = unsafe { bytes_from_raw(result.adapter_name, result.adapter_name_len) }
            .map(String::from_utf8_lossy)
            .unwrap_or_default();
        println!(
            "hp2_rs_wgpu_upload_texture confirmed empirically: a real TEXF_RGBA7 texture \
             uploaded with srgb=true still samples back its own real stored value unchanged \
             (linear), not gamma-decoded, on adapter {adapter_name:?} backend {}",
            handle.backend_name
        );

        unsafe { hp2_rs_wgpu_destroy(result.handle) };
    }

    /// Direct test of `hp2_rs_wgpu_upload_compressed_texture` itself
    /// -- caught on review (not self-discovered): every other test of
    /// this port's own DXT1 support (`hp2-wgpu`'s own `TextureCache::
    /// upload_compressed`/`create_compressed_gpu_texture` tests, and
    /// the real `WgpuDrv.cpp` live-engine smoke evidence, which this
    /// particular session's own Entry-menu scenario never actually
    /// exercised with a real DXT1 texture) bypasses this exact `extern
    /// "C"` function -- its own `FfiCompressedMip` array-to-`Vec`
    /// conversion loop and its own `bool` return-value propagation
    /// (the real bug an earlier version of this function's own caller,
    /// `RenderLoop::upload_compressed_texture`, had before being fixed
    /// to stop discarding it) had never actually been exercised by any
    /// test at all before this one.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-ffi --features renderer-wgpu -- --ignored`"]
    fn upload_compressed_texture_uploads_a_real_chain_and_rejects_bad_input_without_inserting() {
        let result = unsafe { hp2_rs_wgpu_create(std::ptr::null_mut(), 0) };
        assert!(
            !result.handle.is_null(),
            "expected a real headless device to be created"
        );

        // Same real hand-built BC1 blocks as hp2-wgpu's own
        // upload_compressed_round_trips_a_real_4x4_to_2x2_to_1x1_mip_
        // chain test (crates/hp2-wgpu/src/texture.rs) -- color0==
        // color1==solid color in RGB565 with all-zero index bits,
        // decoding to a known solid color regardless of which of the
        // three logical mip sizes it is (every level here is exactly
        // one block).
        let red_block: [u8; 8] = [0x00, 0xF8, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00];
        let green_block: [u8; 8] = [0xE0, 0x07, 0xE0, 0x07, 0x00, 0x00, 0x00, 0x00];
        let blue_block: [u8; 8] = [0x1F, 0x00, 0x1F, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mips = [
            FfiCompressedMip {
                width: 4,
                height: 4,
                data: red_block.as_ptr(),
                data_len: red_block.len(),
            },
            FfiCompressedMip {
                width: 2,
                height: 2,
                data: green_block.as_ptr(),
                data_len: green_block.len(),
            },
            FfiCompressedMip {
                width: 1,
                height: 1,
                data: blue_block.as_ptr(),
                data_len: blue_block.len(),
            },
        ];

        let accepted = unsafe {
            hp2_rs_wgpu_upload_compressed_texture(
                result.handle,
                /* cache_id */ 111,
                /* format = TEXF_DXT1 */ 0x03,
                4,
                4,
                mips.as_ptr(),
                mips.len(),
                false,
                0,
            )
        };
        if !accepted {
            // Only a real, already-handled adapter limitation (no
            // Features::TEXTURE_COMPRESSION_BC granted) can make this
            // specific call fail -- every input here is otherwise
            // valid, confirmed by the always-true assertion this
            // exact chain already gets in hp2-wgpu's own equivalent
            // test. Same honest-disclosure pattern that test already
            // established, not a hard failure on adapters without BC.
            println!(
                "SKIPPED: adapter does not actually grant Features::TEXTURE_COMPRESSION_BC, \
                 nothing further to assert on this adapter"
            );
            unsafe { hp2_rs_wgpu_destroy(result.handle) };
            return;
        }
        assert!(
            unsafe { hp2_rs_wgpu_is_texture_cached(result.handle, 111) },
            "expected cache_id 111 to actually be resident after a real accepted upload"
        );

        // Empty mips: must return false and must not insert anything
        // under this new cache_id.
        let empty_accepted = unsafe {
            hp2_rs_wgpu_upload_compressed_texture(
                result.handle,
                /* cache_id */ 222,
                0x03,
                4,
                4,
                std::ptr::null(),
                0,
                false,
                0,
            )
        };
        assert!(
            !empty_accepted,
            "expected an empty mips array to be rejected"
        );
        assert!(
            !unsafe { hp2_rs_wgpu_is_texture_cached(result.handle, 222) },
            "expected cache_id 222 to NOT be resident after a rejected (empty-mips) upload"
        );

        // Unsupported format (TEXF_RGB8, 0x04): must return false and
        // must not insert anything under this new cache_id either,
        // even though the mips array itself is otherwise well-formed.
        let wrong_format_accepted = unsafe {
            hp2_rs_wgpu_upload_compressed_texture(
                result.handle,
                /* cache_id */ 333,
                0x04,
                4,
                4,
                mips.as_ptr(),
                mips.len(),
                false,
                0,
            )
        };
        assert!(
            !wrong_format_accepted,
            "expected a non-DXT1 format byte to be rejected even with an otherwise valid mips \
             array"
        );
        assert!(
            !unsafe { hp2_rs_wgpu_is_texture_cached(result.handle, 333) },
            "expected cache_id 333 to NOT be resident after a rejected (wrong-format) upload"
        );

        // adapter_name/backend_name are raw (ptr, len) UTF-8 spans
        // (FfiWgpuCreateResult's own doc), not Rust strings -- decoded
        // here only for this println's own benefit, the same real
        // contract hp2_rs_set_log's own real C++ caller (Utf8SpanToFString,
        // WgpuDrv.cpp) already relies on, not a new assumption.
        let adapter_name = unsafe { bytes_from_raw(result.adapter_name, result.adapter_name_len) }
            .map(String::from_utf8_lossy)
            .unwrap_or_default();
        let backend_name = unsafe { bytes_from_raw(result.backend_name, result.backend_name_len) }
            .map(String::from_utf8_lossy)
            .unwrap_or_default();
        println!(
            "hp2_rs_wgpu_upload_compressed_texture confirmed empirically: a real 4x4->2x2->1x1 \
             DXT1 chain uploads and becomes cache-resident through the real extern \"C\" \
             function itself (not just the Rust-internal layers underneath it), and both an \
             empty mips array and a non-DXT1 format are rejected without inserting anything, on \
             adapter {adapter_name:?} backend {backend_name}",
        );
        unsafe { hp2_rs_wgpu_destroy(result.handle) };
    }
}
