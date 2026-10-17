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

fn settings_from_ffi(ffi: &FfiLauncherSettings) -> hp2_launcher::model::LauncherSettings {
    use hp2_launcher::model::*;
    LauncherSettings {
        screen_mode: match ffi.screen_mode {
            1 => ScreenMode::Fullscreen,
            2 => ScreenMode::BorderlessDesktop,
            _ => ScreenMode::Windowed,
        },
        resolution: DisplayResolution {
            width: ffi.resolution_width,
            height: ffi.resolution_height,
            label: String::new(),
        },
        render_backend: match ffi.render_backend {
            1 => RenderBackend::Vulkan,
            _ => RenderBackend::XOpenGL,
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
        texture_detail: match ffi.texture_detail {
            0 => TextureDetail::Low,
            1 => TextureDetail::Medium,
            _ => TextureDetail::High,
        },
        object_detail: match ffi.object_detail {
            0 => ObjectDetail::VeryLow,
            1 => ObjectDetail::Low,
            2 => ObjectDetail::Medium,
            3 => ObjectDetail::High,
            _ => ObjectDetail::VeryHigh,
        },
        sound_enabled: ffi.sound_enabled,
        sound_volume: ffi.sound_volume,
        music_volume: ffi.music_volume,
        mouse_sensitivity: ffi.mouse_sensitivity,
        invert_mouse: ffi.invert_mouse,
        control_mode: match ffi.control_mode {
            1 => ControlMode::Modern,
            _ => ControlMode::Classic,
        },
        auto_center_camera: ffi.auto_center_camera,
        move_while_casting: ffi.move_while_casting,
        auto_quaff: ffi.auto_quaff,
        screen_flashes: ffi.screen_flashes,
        difficulty: match ffi.difficulty {
            1 => Difficulty::Medium,
            2 => Difficulty::Hard,
            _ => Difficulty::Easy,
        },
        joystick_enabled: ffi.joystick_enabled,
    }
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
        let settings = settings_from_ffi(&*settings);
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
