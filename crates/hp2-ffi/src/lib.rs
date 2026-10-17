//! The single staticlib linked into hp2_game, hp2_ucc, hp2_import and the
//! test executables. Port crates' `extern "C"` APIs are re-exported here so
//! the archive and the generated `hp2_rs.h` cover all of them.

use std::ffi::c_void;

use hp2_ffi_support::{guard, install_log_sink};
use hp2_ini::Profile;

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
    pub render_backend: i32,
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
    let render_backend = match ffi.render_backend {
        0 => RenderBackend::XOpenGL,
        1 => RenderBackend::Vulkan,
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
        render_backend,
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
        render_backend: settings.render_backend as i32,
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

/// Mirrors `hp2_platform::input_event::HP2InputEvent`, which itself
/// mirrors `FHP2InputEvent` (`HP2InputEvents.h:37-43`, size/offset
/// asserted there and in `input_event.rs`). Declared separately here,
/// not re-exported, so cbindgen (which only parses this crate plus
/// `hp2-ffi-support`, per `cbindgen.toml`) emits it into `hp2_rs.h`,
/// matching every other FFI struct in this file.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiInputEvent {
    pub key: i32,
    pub action: i32,
    pub delta: f32,
    pub text: [u8; 8],
}

const _: () = assert!(std::mem::size_of::<FfiInputEvent>() == 20);
const _: () = assert!(std::mem::offset_of!(FfiInputEvent, key) == 0);
const _: () = assert!(std::mem::offset_of!(FfiInputEvent, action) == 4);
const _: () = assert!(std::mem::offset_of!(FfiInputEvent, delta) == 8);
const _: () = assert!(std::mem::offset_of!(FfiInputEvent, text) == 12);

impl From<hp2_platform::input_event::HP2InputEvent> for FfiInputEvent {
    fn from(e: hp2_platform::input_event::HP2InputEvent) -> Self {
        FfiInputEvent {
            key: e.key,
            action: e.action,
            delta: e.delta,
            text: e.text,
        }
    }
}

/// Replaces `HP2MapKeysym` (`HP2InputEvents.h:53-185`). Used only by
/// `USDLViewport`'s constructor to fill its own `KeysymMap[512]` --
/// a field that is itself never read anywhere in this codebase, kept
/// here for line-by-line fill-behavior parity with the pre-cutover
/// C++, not because anything consults the array.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_input_map_keysym(sym: i32) -> i32 {
    guard(
        "hp2_rs_input_map_keysym",
        hp2_platform::input_event::ik::NONE,
        || hp2_platform::input_event::map_keysym(sym),
    )
}

/// Replaces `HP2MakeKeyEvent` (`HP2InputEvents.h:189-195`), called at
/// `SDLViewport.cpp:345` for `SDL_KEYDOWN`/`SDL_KEYUP`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_input_make_key_event(sym: i32, pressed: bool) -> FfiInputEvent {
    guard(
        "hp2_rs_input_make_key_event",
        hp2_platform::input_event::make_none_event().into(),
        || hp2_platform::input_event::make_key_event(sym, pressed).into(),
    )
}

/// Replaces `HP2MakeMouseButtonEvent` (`HP2InputEvents.h:200-212`),
/// called at `SDLViewport.cpp:370` for `SDL_MOUSEBUTTONDOWN`/`UP`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_input_make_mouse_button_event(button: u8, pressed: bool) -> FfiInputEvent {
    guard(
        "hp2_rs_input_make_mouse_button_event",
        hp2_platform::input_event::make_none_event().into(),
        || hp2_platform::input_event::make_mouse_button_event(button, pressed).into(),
    )
}

/// Replaces `HP2MakeWheelEvents` (`HP2InputEvents.h:219-239`), called
/// at `SDLViewport.cpp:378` for `SDL_MOUSEWHEEL`. `out` must point to
/// space for at least 3 `FfiInputEvent`s. Returns the count written
/// (0..=3), matching the real C++'s `INT` return.
///
/// # Safety
/// `out` must be a valid, aligned pointer to at least 3
/// `FfiInputEvent` slots, writable for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_make_wheel_events(
    wheel_y: i32,
    out: *mut FfiInputEvent,
) -> i32 {
    guard("hp2_rs_input_make_wheel_events", 0, move || unsafe {
        let mut events = [hp2_platform::input_event::make_none_event(); 3];
        let count = hp2_platform::input_event::make_wheel_events(wheel_y, &mut events);
        for (i, event) in events.iter().take(count as usize).enumerate() {
            *out.add(i) = (*event).into();
        }
        count
    })
}

/// Replaces `HP2MakeTextEvent` (`HP2InputEvents.h:276-289`), called at
/// `SDLViewport.cpp:352` for `SDL_TEXTINPUT`. `text` is UTF-8, not
/// NUL-terminated; `text == NULL` mirrors the real C++'s
/// `Utf8Text == NULL` case, matching the real call site's own
/// `Event.text.text` (always a valid, non-null pointer into the SDL
/// event struct's embedded buffer, even for an empty string).
///
/// # Safety
/// `(text, text_len)` must be valid for reads of that length, or null
/// with any length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_make_text_event(
    text: *const u8,
    text_len: usize,
) -> FfiInputEvent {
    guard(
        "hp2_rs_input_make_text_event",
        hp2_platform::input_event::make_none_event().into(),
        move || unsafe {
            let bytes = if text.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(text, text_len))
            };
            hp2_platform::input_event::make_text_event(bytes).into()
        },
    )
}

/// Mirrors `hp2_platform::mouse_capture::Decision`
/// (`action`: `Action::None/Release/Restore` as `0/1/2`; `cursor`:
/// `Cursor::Hide/Leave/Show` as `-1/0/1`, both already `#[repr(i32)]`
/// with explicit discriminants matching `HP2Capture::EAction`/
/// `ECursor` exactly). Declared directly in this crate, same reason
/// as `FfiInputEvent` above (cbindgen only parses this crate plus
/// `hp2-ffi-support`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiCaptureDecision {
    pub action: i32,
    pub cursor: i32,
}

const _: () = assert!(std::mem::size_of::<FfiCaptureDecision>() == 8);
const _: () = assert!(std::mem::offset_of!(FfiCaptureDecision, action) == 0);
const _: () = assert!(std::mem::offset_of!(FfiCaptureDecision, cursor) == 4);

impl From<hp2_platform::mouse_capture::Decision> for FfiCaptureDecision {
    fn from(d: hp2_platform::mouse_capture::Decision) -> Self {
        FfiCaptureDecision {
            action: d.action as i32,
            cursor: d.cursor as i32,
        }
    }
}

/// Replaces `HP2Capture::OnLoss` (`HP2MouseCapturePolicy.h:81-90`),
/// called at `SDLViewport.cpp:434` for a loss-kind `SDL_WINDOWEVENT`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_mouse_capture_on_loss(
    currently_grabbed: bool,
    loss_pending: bool,
    capture_mouse_config: bool,
) -> FfiCaptureDecision {
    guard(
        "hp2_rs_mouse_capture_on_loss",
        FfiCaptureDecision {
            action: 0,
            cursor: 0,
        },
        || {
            let state = hp2_platform::mouse_capture::State {
                currently_grabbed,
                loss_pending,
                capture_mouse_config,
            };
            hp2_platform::mouse_capture::on_loss(&state).into()
        },
    )
}

/// Replaces `HP2Capture::OnRestore` (`HP2MouseCapturePolicy.h:93-102`),
/// called at `SDLViewport.cpp:451` for a restore-kind
/// `SDL_WINDOWEVENT`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_mouse_capture_on_restore(
    currently_grabbed: bool,
    loss_pending: bool,
    capture_mouse_config: bool,
    has_input_focus: bool,
    minimized_or_hidden: bool,
) -> FfiCaptureDecision {
    guard(
        "hp2_rs_mouse_capture_on_restore",
        FfiCaptureDecision {
            action: 0,
            cursor: 0,
        },
        || {
            let state = hp2_platform::mouse_capture::State {
                currently_grabbed,
                loss_pending,
                capture_mouse_config,
            };
            let window = hp2_platform::mouse_capture::WindowFlags {
                has_input_focus,
                minimized_or_hidden,
            };
            hp2_platform::mouse_capture::on_restore(&state, &window).into()
        },
    )
}

/// Replaces `USDLViewport::GetViewportButtonFlags` (`SDLViewport.cpp:544-548`).
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_platform_get_viewport_button_flags() -> u32 {
    guard("hp2_rs_platform_get_viewport_button_flags", 0, || {
        hp2_platform::mouse_state::get_viewport_button_flags()
    })
}

/// Replaces `USDLViewport::GetMouseState` (`SDLViewport.cpp:550-556`).
/// Writes the scaled `X`/`Y` into `out_x`/`out_y`, returns the button
/// flags (matching the real `DWORD` return of
/// `GetViewportButtonFlags`, reused here since `GetMouseState`'s own
/// `Buttons` out-param carries the identical value).
///
/// # Safety
/// `out_x`/`out_y` must be valid, aligned, writable pointers for the
/// duration of this call.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_get_mouse_state(
    mouse_scale_x: f32,
    mouse_scale_y: f32,
    out_x: *mut f32,
    out_y: *mut f32,
) -> u32 {
    guard("hp2_rs_platform_get_mouse_state", 0, move || unsafe {
        let (x, y, buttons) =
            hp2_platform::mouse_state::get_mouse_state(mouse_scale_x, mouse_scale_y);
        *out_x = x;
        *out_y = y;
        buttons
    })
}

/// Replaces `USDLViewport::UpdateMouseGrabState` (`SDLViewport.cpp:558-579`).
/// `sdl_window` may be null (matches the real C++'s `if (SdlWindow)`
/// guard). Writes the new `bWindowsMouseAvailable` into
/// `out_windows_mouse_available`; returns the new `MouseIsGrabbed`.
/// Caller stores both back into its own `USDLViewport` fields.
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the
/// duration of this call. `out_windows_mouse_available` must be a
/// valid, aligned, writable pointer.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_update_mouse_grab_state(
    sdl_window: *mut c_void,
    capture: bool,
    mouse_is_grabbed: bool,
    out_windows_mouse_available: *mut bool,
) -> bool {
    guard(
        "hp2_rs_platform_update_mouse_grab_state",
        mouse_is_grabbed,
        move || unsafe {
            let (new_grabbed, new_windows_mouse_available) =
                hp2_platform::mouse_state::update_mouse_grab_state(
                    sdl_window as *mut hp2_platform::mouse_state::SdlWindow,
                    capture,
                    mouse_is_grabbed,
                );
            *out_windows_mouse_available = new_windows_mouse_available;
            new_grabbed
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

/// Replaces `ButtonsFromSdlState` (`SDLViewport.cpp:46-53`), the
/// local static helper the ungrabbed `SDL_MOUSEMOTION` handler
/// (`:361`) uses. Pure bit translation, no native dependency -- not
/// gated behind `platform-sdl` (unlike the rest of
/// `hp2_rs_platform_*`), matching `hp2_platform::mouse_state::
/// buttons_from_sdl_state`'s own unconditional availability.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_platform_buttons_from_sdl_state(state: u32) -> u32 {
    guard("hp2_rs_platform_buttons_from_sdl_state", 0, || {
        hp2_platform::mouse_state::buttons_from_sdl_state(state)
    })
}

/// Replaces `USDLClient::OpenJoystick` (`SDLClient.cpp:234-258`),
/// minus its own `CloseJoystick()` call at the start -- the real C++
/// caller still makes that call itself (now also FFI-forwarded, see
/// `hp2_rs_platform_close_joystick` below) before calling this.
/// Returns the opaque `SDL_Joystick*` handle on success (the caller
/// stores it in its own `Joystick` field, to hand back to
/// `hp2_rs_platform_close_joystick` later), or null on any failure --
/// `out_buttons`/`out_hats`/`out_axes`/`out_hat_folds_to_buttons` are
/// left unwritten in that case, matching the real C++'s own `return 0`
/// leaving `JoyButtons` etc. untouched.
///
/// # Safety
/// `out_buttons`/`out_hats`/`out_axes`/`out_hat_folds_to_buttons` must
/// be valid, aligned, writable pointers for the duration of this call.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_open_joystick(
    joystick_number: i32,
    ignore_hat: bool,
    out_buttons: *mut i32,
    out_hats: *mut i32,
    out_axes: *mut i32,
    out_hat_folds_to_buttons: *mut bool,
) -> *mut c_void {
    guard(
        "hp2_rs_platform_open_joystick",
        std::ptr::null_mut(),
        move || unsafe {
            let Some((handle, buttons, hats, axes, hat_folds_to_buttons)) =
                hp2_platform::joystick::open_joystick(joystick_number, ignore_hat)
            else {
                return std::ptr::null_mut();
            };
            *out_buttons = buttons;
            *out_hats = hats;
            *out_axes = axes;
            *out_hat_folds_to_buttons = hat_folds_to_buttons;
            handle as *mut c_void
        },
    )
}

/// Replaces the SDL half of `USDLClient::CloseJoystick`
/// (`SDLClient.cpp:261-274`); the caller zeroes its own
/// `JoyButtons`/`JoyHats`/`JoyAxes`/`JoyHatFoldsToButtons`/`Joystick`
/// fields, which Rust does not own. `joystick` may be null, matching
/// the real C++'s own `if (Joystick)` guard.
///
/// # Safety
/// `joystick`, if non-null, must be a valid, still-open
/// `SDL_Joystick*` obtained from `hp2_rs_platform_open_joystick` and
/// not already closed.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_close_joystick(joystick: *mut c_void) {
    guard("hp2_rs_platform_close_joystick", (), move || unsafe {
        hp2_platform::joystick::close_joystick(joystick as *mut hp2_platform::joystick::SdlJoystick)
    })
}

/// Mirrors `(key, action)` pairs `hp2_rs_input_fold_joystick_hat`
/// returns. Declared directly in this crate, same reason as the
/// other FFI mirror structs above.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiKeyAction {
    pub key: i32,
    pub action: i32,
}

/// Replaces the `SDL_JOYAXISMOTION` case
/// (`SDLViewport.cpp:373-386`). Writes the mapped key/scaled value
/// into `out_key`/`out_value` and returns `true` on success; returns
/// `false` (out-params untouched) when `axis` is out of range,
/// matching the real C++'s own `if (Event.jaxis.axis >=
/// MaxJoystickAxes) break;` guard.
///
/// # Safety
/// `out_key`/`out_value` must be valid, aligned, writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_map_joystick_axis(
    axis: i32,
    raw_value: i32,
    dead_zone: f32,
    scale_x: f32,
    scale_y: f32,
    out_key: *mut i32,
    out_value: *mut f32,
) -> bool {
    guard("hp2_rs_input_map_joystick_axis", false, move || unsafe {
        let Some((key, value)) = hp2_platform::joystick_input::map_joystick_axis(
            axis, raw_value, dead_zone, scale_x, scale_y,
        ) else {
            return false;
        };
        *out_key = key;
        *out_value = value;
        true
    })
}

/// Replaces the `SDL_JOYBUTTONDOWN`/`SDL_JOYBUTTONUP` case
/// (`SDLViewport.cpp:388-395`). Writes the mapped key/action into
/// `out_key`/`out_action` and returns `true` on success; returns
/// `false` (out-params untouched) when `button >= 16`, matching the
/// real C++'s own guard.
///
/// # Safety
/// `out_key`/`out_action` must be valid, aligned, writable pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_map_joystick_button(
    button: i32,
    pressed: bool,
    out_key: *mut i32,
    out_action: *mut i32,
) -> bool {
    guard("hp2_rs_input_map_joystick_button", false, move || unsafe {
        let Some((key, action)) =
            hp2_platform::joystick_input::map_joystick_button(button, pressed)
        else {
            return false;
        };
        *out_key = key;
        *out_action = action;
        true
    })
}

/// Replaces the `SDL_JOYHATMOTION` case (`SDLViewport.cpp:396-415`).
/// `last_folded` is the real C++'s own `LastJoyHat` field, read
/// before the call. `out_events` must point to space for at least 2
/// `FfiKeyAction`s. Returns the number of events written (0, 1, or
/// 2); `out_new_last_folded` always receives the value to store back
/// into `LastJoyHat`, regardless of the event count. The real C++'s
/// own `if (!JoyHatFoldsToButtons) break;` guard happens at the call
/// site, not here.
///
/// # Safety
/// `out_events` must be valid, aligned, writable for at least 2
/// `FfiKeyAction` slots. `out_new_last_folded` must be a valid,
/// aligned, writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_fold_joystick_hat(
    hat_bits: u8,
    last_folded: i32,
    out_events: *mut FfiKeyAction,
    out_new_last_folded: *mut i32,
) -> i32 {
    guard("hp2_rs_input_fold_joystick_hat", 0, move || unsafe {
        let (new_last_folded, events, count) =
            hp2_platform::joystick_input::fold_joystick_hat(hat_bits, last_folded);
        *out_new_last_folded = new_last_folded;
        for (i, &(key, action)) in events.iter().take(count).enumerate() {
            *out_events.add(i) = FfiKeyAction { key, action };
        }
        count as i32
    })
}

/// Mirrors the three `ALSource` fields `hp2_audio::priority::
/// SourceSlot` needs (`crates/hp2-audio/src/priority.rs`). Declared
/// directly in this crate for the same cbindgen-parsing reason as
/// the other FFI mirror structs above.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiAudioSourceSlot {
    pub has_sound: bool,
    pub id: i32,
    pub priority: f32,
}

/// Replaces `UALAudioSubsystem::FindLeastPriorityNonTalkSource`'s
/// body (`ALAudioSubsystem.cpp:2085-2101`). `sources` must point to
/// `count` contiguous `FfiAudioSourceSlot`s; the caller builds this
/// array from its own `TArray<ALSource>` immediately before the call
/// (no `ALSource` field Rust does not need crosses the boundary).
///
/// # Safety
/// `sources` must be a valid, aligned pointer to at least `count`
/// readable `FfiAudioSourceSlot`s (or `count == 0`, in which case
/// `sources` may be dangling).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_find_least_priority_non_talk_source(
    sources: *const FfiAudioSourceSlot,
    count: usize,
) -> i32 {
    guard(
        "hp2_rs_audio_find_least_priority_non_talk_source",
        -1,
        move || unsafe {
            let slots: Vec<hp2_audio::priority::SourceSlot> = if count == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(sources, count)
                    .iter()
                    .map(|s| hp2_audio::priority::SourceSlot {
                        has_sound: s.has_sound,
                        id: s.id,
                        priority: s.priority,
                    })
                    .collect()
            };
            hp2_audio::priority::find_least_priority_non_talk_source(&slots)
        },
    )
}

/// Replaces `UALAudioSubsystem::SoundPriority`'s body
/// (`ALAudioSubsystem.cpp:1946-1961`). `dist_squared` is the caller's
/// own `FDistSquared(Location, ViewTarget->Location)`; `flags` is the
/// sound's `WorkingFlags`/`Flags` bits as-is.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_audio_sound_priority(
    dist_squared: f32,
    radius: f32,
    audio_max_radius_multiplier: f32,
    volume: f32,
    flags: i32,
) -> f32 {
    guard("hp2_rs_audio_sound_priority", 0.0, move || {
        hp2_audio::priority::sound_priority(
            dist_squared,
            radius,
            audio_max_radius_multiplier,
            volume,
            flags,
        )
    })
}

/// Mirrors `hp2_audio::priority::LeastImportantSlot`
/// (`crates/hp2-audio/src/priority.rs`). `sound` is a `USound*`'s raw
/// identity bits (`(uintptr_t)Source.Sound` in C++); this crate never
/// dereferences it, only compares it for equality.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiLeastImportantSlot {
    pub sound: usize,
    pub id: i32,
    pub dist_squared: f32,
    pub volume: f32,
    pub radius: f32,
    pub flags: i32,
}

/// Replaces `UALAudioSubsystem::FindLeastImportantSound`'s body
/// (`ALAudioSubsystem.cpp:2105-2156`). `slots` must point to `count`
/// contiguous `FfiLeastImportantSlot`s in the same order as the real
/// C++'s `Sources` array (the "most frequent sound" tie-break depends
/// on that order -- see `find_least_important_sound`'s own doc).
/// `incoming_sound` is the raw identity bits of the `USound*` about
/// to play.
///
/// # Safety
/// `slots` must be a valid, aligned pointer to at least `count`
/// readable `FfiLeastImportantSlot`s (or `count == 0`, in which case
/// `slots` may be dangling).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_find_least_important_sound(
    slots: *const FfiLeastImportantSlot,
    count: usize,
    incoming_sound: usize,
    audio_max_radius_multiplier: f32,
) -> i32 {
    guard(
        "hp2_rs_audio_find_least_important_sound",
        -1,
        move || unsafe {
            let owned: Vec<hp2_audio::priority::LeastImportantSlot> = if count == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(slots, count)
                    .iter()
                    .map(|s| hp2_audio::priority::LeastImportantSlot {
                        sound: s.sound,
                        id: s.id,
                        dist_squared: s.dist_squared,
                        volume: s.volume,
                        radius: s.radius,
                        flags: s.flags,
                    })
                    .collect()
            };
            hp2_audio::priority::find_least_important_sound(
                &owned,
                incoming_sound,
                audio_max_radius_multiplier,
            )
        },
    )
}

/// Boxed on the heap, handed to C++ as an opaque `void* RustDevice`
/// (`UWgpuRenderDevice`'s only field, per the plan's Phase 3.2 "class
/// owns only an opaque `void* RustDevice`" design). Caches
/// `backend_name` as an owned `String` alongside the `CreatedDevice`
/// so `hp2_rs_wgpu_create`'s result can point into memory the handle
/// itself owns, valid for the handle's whole lifetime -- not a
/// temporary that would dangle the moment the function returns.
#[cfg(feature = "renderer-wgpu")]
struct WgpuHandle {
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
pub unsafe extern "C" fn hp2_rs_wgpu_create(ns_view: *mut c_void) -> FfiWgpuCreateResult {
    guard("hp2_rs_wgpu_create", FFI_WGPU_CREATE_FAILURE, move || {
        #[cfg(target_os = "macos")]
        let created = if ns_view.is_null() {
            let no_surface: Option<hp2_wgpu::HeadlessSurfaceCreator> = None;
            hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, no_surface)
        } else {
            let create_surface = move |instance: &wgpu::Instance| unsafe {
                hp2_wgpu::macos::create_surface_from_ns_view(instance, ns_view)
            };
            hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, Some(create_surface))
        };
        #[cfg(not(target_os = "macos"))]
        let created = {
            // No window-surface path on this platform yet (hp2-wgpu's
            // `macos` module is macOS-only -- see its own doc for
            // why); a non-null ns_view here would be a caller bug, not
            // something this function can act on, so it is silently
            // ignored rather than causing total failure.
            let _ = ns_view;
            let no_surface: Option<hp2_wgpu::HeadlessSurfaceCreator> = None;
            hp2_wgpu::create_device_sync(hp2_wgpu::BackendChoice::Auto, no_surface)
        };
        let Some(created) = created else {
            return FFI_WGPU_CREATE_FAILURE;
        };
        let backend_name = hp2_wgpu::backend_name(created.backend).to_string();
        let handle = Box::new(WgpuHandle {
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

/// Testing-only: configures `handle`'s surface, acquires a frame, and
/// presents it. Returns `hp2_wgpu::PresentOnceOutcome` as a raw `u32`
/// (`0` is `Presented`, the only success code -- see that enum's own
/// doc for every other code's meaning) rather than a bare bool: a
/// bare bool hid the real cause of this function's own first failing
/// run (caught on review before a comment guessing at the cause, and
/// wrongly claiming it was "confirmed empirically", ever landed). No
/// production C++ caller yet -- there is no draw-call forwarding for
/// `URenderDevice`'s pure virtuals to drive this from yet
/// (`hp2-wgpu`'s own module doc); this exists for the plan's
/// "stronger surface-working proof" item, called directly from a C++
/// ctest against a real SDL window and a `handle` that was itself
/// created with a non-null `ns_view`.
///
/// # Safety
/// `handle` must be exactly what `hp2_rs_wgpu_create` returned (not
/// yet destroyed), created with a non-null `ns_view` (a `handle`
/// created headless has no surface and this always returns
/// `NoSurface` for it, but is not itself unsound to call). Must be
/// called on the application's main thread (AppKit requirement, same
/// as surface creation itself).
#[cfg(feature = "renderer-wgpu")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_wgpu_present_once_for_testing(
    handle: *mut c_void,
    width: u32,
    height: u32,
) -> u32 {
    guard(
        "hp2_rs_wgpu_present_once_for_testing",
        hp2_wgpu::PresentOnceOutcome::InternalError as u32,
        move || {
            if handle.is_null() {
                return hp2_wgpu::PresentOnceOutcome::InternalError as u32;
            }
            let handle = unsafe { &*(handle as *mut WgpuHandle) };
            hp2_wgpu::configure_and_present_once(&handle.created, width, height) as u32
        },
    )
}
