//! Port of `Engine/Src/UnDownload.cpp` (`UDownload`, `UChannelDownload`).
//!
//! Engine calls go through `Hp2NetDemoServices` (`net_demo.rs`). Each thunk returns an
//! `int` status; a thrown status becomes `Err(Thrown)` and propagates with `?` to the FFI
//! export, which returns status 1. The C++ shell then rethrows via `Hp2RethrowPending`.
//!
//! Divergences: `DownloadError` copies with an unbounded `appStrcpy`, as the original does.
//! Oversized messages overflow `Error[256]`; this is preserved, not clamped.

use std::ffi::c_void;
use std::ptr::null_mut;

use hp2_abi::engine::generated::{FArchive, UChannelDownload, UDownload};

use crate::net_demo::{Hp2NetDemoServices, R, Thrown, out, st, text, wide};

/// `PKG_ClientOptional` (UnObjBas.h:38).
pub const PKG_CLIENT_OPTIONAL: u32 = 0x0002;

#[inline]
fn dl_of(this: *mut c_void) -> *mut UDownload {
    this.cast()
}

#[inline]
fn chdl_of(this: *mut c_void) -> *mut UChannelDownload {
    this.cast()
}

pub(crate) enum FormatArg<'a> {
    Text(&'a str),
    Int(i32),
    Float(f64),
}

/// Applies the shared native printf subset used for download localization.
pub(crate) fn localized_format(format: &str, args: &[FormatArg<'_>]) -> Vec<u32> {
    let mut result = String::new();
    let mut chars = format.chars().peekable();
    let mut args = args.iter();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            result.push('%');
            continue;
        }
        let (mut left, mut plus, mut space, mut zero, mut alternate) =
            (false, false, false, false, false);
        loop {
            match chars.peek() {
                Some('-') => left = true,
                Some('+') => plus = true,
                Some(' ') => space = true,
                Some('0') => zero = true,
                Some('#') => alternate = true,
                _ => break,
            }
            chars.next();
        }
        let mut width = 0usize;
        while let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
            width = width * 10 + digit as usize;
            chars.next();
        }
        let precision = if chars.peek() == Some(&'.') {
            chars.next();
            let mut value = 0usize;
            while let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
                value = value * 10 + digit as usize;
                chars.next();
            }
            Some(value)
        } else {
            None
        };
        let conversion = chars.next();
        let arg = args.next().expect("download localization argument");
        let mut value = match (conversion, arg) {
            (Some('s'), FormatArg::Text(value)) => value
                .chars()
                .take(precision.unwrap_or(usize::MAX))
                .collect(),
            (Some('d' | 'i'), FormatArg::Int(value)) => {
                let digits = value.unsigned_abs().to_string();
                let mut result = String::new();
                if *value < 0 {
                    result.push('-');
                } else if plus {
                    result.push('+');
                } else if space {
                    result.push(' ');
                }
                result.extend(std::iter::repeat_n(
                    '0',
                    precision.unwrap_or(1).saturating_sub(digits.len()),
                ));
                if *value != 0 || precision != Some(0) {
                    result.push_str(&digits);
                }
                result
            }
            (Some('f' | 'F'), FormatArg::Float(value)) => {
                let mut result = if value.is_nan() {
                    if value.is_sign_negative() {
                        "-nan".into()
                    } else {
                        "nan".into()
                    }
                } else {
                    format!("{value:.precision$}", precision = precision.unwrap_or(6))
                };
                if alternate && precision == Some(0) && value.is_finite() {
                    result.push('.');
                }
                if !value.is_sign_negative() {
                    if plus {
                        result.insert(0, '+');
                    } else if space {
                        result.insert(0, ' ');
                    }
                }
                if conversion == Some('F') {
                    result.make_ascii_uppercase();
                }
                result
            }
            _ => panic!("invalid download localization conversion"),
        };
        let padding = width.saturating_sub(value.chars().count());
        if left {
            result.push_str(&value);
            result.extend(std::iter::repeat_n(' ', padding));
        } else if zero
            && !matches!(arg, FormatArg::Text(_))
            && !(matches!(arg, FormatArg::Int(_)) && precision.is_some())
            && !matches!(arg, FormatArg::Float(value) if !value.is_finite())
        {
            if value.starts_with(['-', '+', ' ']) {
                result.push(value.remove(0));
            }
            result.extend(std::iter::repeat_n('0', padding));
            result.push_str(&value);
        } else {
            result.extend(std::iter::repeat_n(' ', padding));
            result.push_str(&value);
        }
    }
    wide(&result)
}

unsafe fn parent_name(svc: &Hp2NetDemoServices, info: *mut c_void) -> R<String> {
    unsafe { Ok(text(out(|p| (svc.demo_info_parent_name)(info, p))?)) }
}

unsafe fn guid_string(svc: &Hp2NetDemoServices, info: *mut c_void) -> R<String> {
    unsafe {
        let string = out(|p| (svc.demo_fstring_new)(p))?;
        st((svc.demo_info_guid_string)(info, string))?;
        let result = text(out(|p| (svc.demo_fstring_chars)(string, p))?);
        st((svc.demo_fstring_delete)(string))?;
        Ok(result)
    }
}

unsafe fn localized_error(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    key: &std::ffi::CStr,
    args: &[FormatArg<'_>],
) -> R<()> {
    unsafe {
        let format = out(|p| (svc.demo_localize_error_ansi)(key.as_ptr(), p))?;
        if args.is_empty() {
            return st((svc.demo_dl_error_virtual)(this, format));
        }
        let message = localized_format(&text(format), args);
        st((svc.demo_dl_error_virtual)(this, message.as_ptr().cast()))
    }
}

unsafe fn open_error(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    unsafe {
        let key = wide("NetOpen");
        let message = out(|p| (svc.demo_localize_error_wide)(key.as_ptr().cast(), p))?;
        st((svc.demo_dl_error_virtual)(this, message))
    }
}

unsafe fn decompress(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    unsafe {
        let d = dl_of(this);
        let mut compressed_name = [0u32; 256];
        st((svc.demo_app_strcpy)(
            compressed_name.as_mut_ptr().cast(),
            (*d).temp_filename.as_ptr().cast(),
        ))?;
        let cache = out(|p| (svc.demo_sys_cache_path)(p))?;
        st((svc.demo_app_create_temp_filename)(
            cache,
            (*d).temp_filename.as_mut_ptr().cast(),
        ))?;
        let reader = out::<*mut c_void>(|p| {
            (svc.demo_fm_create_reader)(compressed_name.as_ptr().cast(), p)
        })?;
        let writer = out::<*mut c_void>(|p| {
            (svc.demo_fm_create_writer)((*d).temp_filename.as_ptr().cast(), p)
        })?;
        if reader.is_null() || writer.is_null() {
            open_error(svc, this)?;
        } else {
            let signature = out(|p| (svc.demo_ar_read_i32)(reader, p))?;
            if signature != 1234 && signature != 5678 {
                localized_error(svc, this, c"NetSize", &[])?;
            } else {
                st((svc.demo_ar_read_fstring_discard)(reader))?;
                let codec = out(|p| (svc.demo_codec_new)(p))?;
                st((svc.demo_codec_add_rle)(codec))?;
                st((svc.demo_codec_add_bwt)(codec))?;
                st((svc.demo_codec_add_mtf)(codec))?;
                if signature == 5678 {
                    st((svc.demo_codec_add_rle)(codec))?;
                }
                st((svc.demo_codec_add_huffman)(codec))?;
                st((svc.demo_codec_decode)(codec, reader, writer))?;
                st((svc.demo_codec_delete)(codec))?;
            }
        }
        if !reader.is_null() {
            st((svc.demo_fm_delete)(compressed_name.as_ptr().cast()))?;
            st((svc.demo_ar_delete)(reader))?;
        }
        if !writer.is_null() {
            st((svc.demo_ar_delete)(writer))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// UDownload
// ---------------------------------------------------------------------------

/// `UDownload::ReceiveFile`; preserves the virtual method's Params and InCompression arguments.
///
/// # Safety
/// `this` is a live `UDownload`; `conn` is live and `params` is NUL-terminated TCHAR.
pub unsafe fn download_receive_file(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    conn: *mut c_void,
    package_index: i32,
    params: *const u32,
    compression: i32,
) -> R<()> {
    let mut ops = NativeDownloadOps { services: svc, cache_ini: null_mut() };
    unsafe {
        crate::download_owner::receive_file_with(
            &mut ops, dl_of(this), conn.cast(), package_index, params, compression != 0,
        )
    }
}

// Source `UDownload::ReceiveFile` receives Params/InCompression but its native
// body only resolves and stores the package metadata; neither argument affects it.
impl crate::download_owner::ReceiveFileOps for NativeDownloadOps<'_> {
    type Error = Thrown;
    unsafe fn resolve_info(
        &mut self,
        connection: *mut hp2_abi::engine::generated::UNetConnection,
        package_index: i32,
        _params: *const u32,
        _compression: bool,
    ) -> R<*mut hp2_abi::engine::generated::FPackageInfo> {
        out::<*mut c_void>(|p| unsafe {
            (self.services.demo_dl_resolve_info)(connection.cast(), package_index, p)
        }).map(|info| info.cast())
    }
    fn invalid_request(&mut self) -> Thrown { Thrown }
}

struct NativeDownloadOps<'a> {
    services: &'a Hp2NetDemoServices,
    cache_ini: *mut c_void,
}

impl crate::download_owner::DownloadOps for NativeDownloadOps<'_> {
    type Error = Thrown;

    unsafe fn file_is_open(&mut self, download: *mut UDownload) -> R<bool> {
        Ok(unsafe { !(*download).recv_file_ar.is_null() })
    }
    unsafe fn transferred(&mut self, download: *mut UDownload) -> R<i32> {
        Ok(unsafe { (*download).transfered })
    }
    unsafe fn prepare_receive(&mut self, download: *mut UDownload) -> R<()> {
        let name = unsafe { parent_name(self.services, (*download).info)? };
        let message = wide(&format!("Receiving package '{name}'"));
        st(unsafe { (self.services.demo_debugf_devnet_text)(message.as_ptr().cast()) })?;
        let cache = out(|p| unsafe { (self.services.demo_sys_cache_path)(p) })?;
        st(unsafe { (self.services.demo_fm_make_directory)(cache) })?;
        st(unsafe {
            (self.services.demo_app_create_temp_filename)(
                cache,
                (*download).temp_filename.as_mut_ptr().cast(),
            )
        })?;
        (*download).recv_file_ar = out(|p| unsafe {
            (self.services.demo_fm_create_writer)((*download).temp_filename.as_ptr().cast(), p)
        })?;
        Ok(())
    }
    unsafe fn write_receive_data(
        &mut self,
        download: *mut UDownload,
        data: *const u8,
        count: i32,
    ) -> R<bool> {
        if count > 0 {
            st(unsafe { (self.services.demo_ar_serialize)((*download).recv_file_ar, data.cast_mut().cast(), count) })?;
        }
        Ok(unsafe { (*(*download).recv_file_ar.cast::<FArchive>()).ar_is_error == 0 })
    }
    unsafe fn add_transferred(&mut self, download: *mut UDownload, count: i32) -> R<()> {
        unsafe { (*download).transfered = (*download).transfered.wrapping_add(count); }
        Ok(())
    }
    unsafe fn open_error(&mut self, download: *mut UDownload) -> R<()> {
        unsafe { open_error(self.services, download.cast()) }
    }
    unsafe fn write_error(&mut self, download: *mut UDownload) -> R<()> {
        let filename = text(unsafe { (*download).temp_filename.as_ptr().cast() });
        unsafe { localized_error(self.services, download.cast(), c"NetWrite", &[FormatArg::Text(&filename)]) }
    }
    unsafe fn report_progress(&mut self, download: *mut UDownload) -> R<()> {
        let flags = out(|p| unsafe { (self.services.demo_info_package_flags)((*download).info, p) })?;
        let key = if flags & PKG_CLIENT_OPTIONAL != 0 { c"ReceiveOptionalFile" } else { c"ReceiveFile" };
        let receive_format = text(out(|p| unsafe {
            (self.services.demo_localize_progress_ansi)(key.as_ptr(), p)
        })?);
        let name = unsafe { parent_name(self.services, (*download).info)? };
        let size_format = text(out(|p| unsafe {
            (self.services.demo_localize_progress_ansi)(c"ReceiveSize".as_ptr(), p)
        })?);
        let size = out(|p| unsafe { (self.services.demo_info_file_size)((*download).info, p) })?;
        let (message, progress) = crate::download_owner::receive_progress(
            &name, unsafe { (*download).transfered }, size, &receive_format, &size_format,
        );
        st(unsafe {
            (self.services.demo_conn_notify_progress)(
                (*download).connection,
                message.as_ptr().cast(),
                progress.as_ptr().cast(),
                4.0,
            )
        })
    }
    fn invalid_data(&mut self) -> Thrown { Thrown }
}
impl crate::download_owner::DownloadCompletionOps for NativeDownloadOps<'_> {
    type Error = Thrown;

    unsafe fn close_receive_file(&mut self, download: *mut UDownload) -> R<()> {
        if unsafe { !(*download).recv_file_ar.is_null() } {
            st(unsafe { (self.services.demo_ar_delete)((*download).recv_file_ar) })?;
            unsafe { (*download).recv_file_ar = null_mut(); }
        }
        Ok(())
    }
    unsafe fn skipped(&mut self, download: *mut UDownload) -> R<bool> {
        Ok(unsafe { (*download).skipped_file != 0 })
    }
    unsafe fn package_name(&mut self, download: *mut UDownload) -> R<String> {
        unsafe { parent_name(self.services, (*download).info) }
    }
    unsafe fn log_skipped(&mut self, _download: *mut UDownload, name: &str) -> R<()> {
        let message = wide(&format!("Skipped download of '{name}'"));
        st(unsafe { (self.services.demo_debugf_text)(message.as_ptr().cast()) })
    }
    unsafe fn remove_temp(&mut self, download: *mut UDownload) -> R<()> {
        st(unsafe { (self.services.demo_fm_delete)((*download).temp_filename.as_ptr().cast()) })
    }
    unsafe fn progress(&mut self, download: *mut UDownload, message: &str) -> R<()> {
        let caption = wide("Success");
        let message = wide(message);
        st(unsafe {
            (self.services.demo_conn_notify_progress)(
                (*download).connection, caption.as_ptr().cast(), message.as_ptr().cast(), 4.0,
            )
        })
    }
    unsafe fn notify_received(&mut self, download: *mut UDownload, skipped: bool) -> R<()> {
        let empty = [0u32];
        st(unsafe {
            (self.services.demo_conn_notify_received_file)(
                (*download).connection,
                (*download).package_index,
                if skipped { empty.as_ptr().cast() } else { (*download).error.as_ptr().cast() },
                i32::from(skipped),
            )
        })
    }
    unsafe fn transferred(&mut self, download: *mut UDownload) -> R<i32> {
        Ok(unsafe { (*download).transfered })
    }
    unsafe fn has_error(&mut self, download: *mut UDownload) -> R<bool> {
        Ok(unsafe { (*download).error[0] != 0 })
    }
    unsafe fn set_error(
        &mut self,
        download: *mut UDownload,
        key: &'static str,
        name: Option<&str>,
    ) -> R<()> {
        let key = match key {
            "NetRefused" => c"NetRefused",
            "NetSize" => c"NetSize",
            "NetMove" => c"NetMove",
            _ => return Err(Thrown),
        };
        let args = name.map_or_else(Vec::new, |name| vec![FormatArg::Text(name)]);
        unsafe { localized_error(self.services, download.cast(), key, &args) }
    }
    unsafe fn is_compressed(&mut self, download: *mut UDownload) -> R<bool> {
        Ok(unsafe { (*download).is_compressed != 0 })
    }
    unsafe fn decompress(&mut self, download: *mut UDownload) -> R<bool> {
        unsafe { decompress(self.services, download.cast())?; }
        Ok(unsafe { (*download).error[0] == 0 })
    }
    unsafe fn size_matches(&mut self, download: *mut UDownload) -> R<bool> {
        let size = out(|p| unsafe {
            (self.services.demo_fm_file_size)((*download).temp_filename.as_ptr().cast(), p)
        })?;
        let expected = out(|p| unsafe { (self.services.demo_info_file_size)((*download).info, p) })?;
        Ok(size == expected)
    }
    unsafe fn move_to_cache(&mut self, download: *mut UDownload) -> R<bool> {
        let cache = text(out(|p| unsafe { (self.services.demo_sys_cache_path)(p) })?);
        let guid = unsafe { guid_string(self.services, (*download).info)? };
        let destination = wide(&format!("{cache}/{guid}.uxx"));
        let mut moved = 0;
        st(unsafe {
            (self.services.demo_fm_move)(
                destination.as_ptr().cast(),
                (*download).temp_filename.as_ptr().cast(),
                &mut moved,
            )
        })?;
        Ok(moved != 0)
    }
    unsafe fn update_cache(&mut self, download: *mut UDownload) -> R<()> {
        self.cache_ini = out(|p| unsafe { (self.services.demo_cache_ini_new)(p) })?;
        let cache = text(out(|p| unsafe { (self.services.demo_sys_cache_path)(p) })?);
        let ini_name = wide(&format!("{cache}/cache.ini"));
        let url = out::<*mut c_void>(|p| unsafe {
            (self.services.demo_info_url_chars)((*download).info, p)
        })?;
        let value = if unsafe { *url.cast::<u32>() != 0 } {
            url
        } else {
            out(|p| unsafe { (self.services.demo_info_parent_name)((*download).info, p) })?
        };
        let guid = wide(&unsafe { guid_string(self.services, (*download).info)? });
        let section = wide("Cache");
        st(unsafe {
            (self.services.demo_cache_ini_set_string)(
                self.cache_ini, section.as_ptr().cast(), guid.as_ptr().cast(), value, ini_name.as_ptr().cast(),
            )
        })
    }
    unsafe fn flush_cache(&mut self, _download: *mut UDownload) -> R<()> {
        if self.cache_ini.is_null() { return Ok(()); }
        let cache_ini = std::mem::replace(&mut self.cache_ini, null_mut());
        st(unsafe { (self.services.demo_cache_ini_delete)(cache_ini) })
    }
}
impl crate::download_owner::DownloadDestroyOps for NativeDownloadOps<'_> {
    type Error = Thrown;

    unsafe fn close_and_remove_partial(&mut self, download: *mut UDownload) -> R<()> {
        if unsafe { !(*download).recv_file_ar.is_null() } {
            st(unsafe { (self.services.demo_ar_delete)((*download).recv_file_ar) })?;
            unsafe { (*download).recv_file_ar = null_mut(); }
            st(unsafe { (self.services.demo_fm_delete)((*download).temp_filename.as_ptr().cast()) })?;
        }
        Ok(())
    }
    unsafe fn clear_connection_download(&mut self, download: *mut UDownload) -> R<()> {
        let connection = unsafe { (*download).connection };
        if !connection.is_null()
            && out::<*mut c_void>(|p| unsafe { (self.services.demo_conn_download)(connection, p) })?
                == download.cast()
        {
            st(unsafe { (self.services.demo_conn_set_download)(connection, null_mut()) })?;
        }
        unsafe { (*download).connection = null_mut(); }
        Ok(())
    }
}

impl Drop for NativeDownloadOps<'_> {
    fn drop(&mut self) {
        if !self.cache_ini.is_null() {
            let cache_ini = std::mem::replace(&mut self.cache_ini, null_mut());
            // SAFETY: the cache transaction was created by this adapter and remains live.
            let _ = unsafe { (self.services.demo_cache_ini_delete)(cache_ini) };
        }
    }
}

/// `UDownload::ReceiveData`.
///
/// # Safety
/// `this` is a live `UDownload`; `data` points to at least `count` readable bytes when
/// `count > 0`.
pub unsafe fn download_receive_data(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    data: *mut c_void,
    count: i32,
) -> R<()> {
    let mut ops = NativeDownloadOps { services: svc, cache_ini: null_mut() };
    unsafe {
        crate::download_owner::receive_data_with(
            &mut ops,
            dl_of(this),
            data.cast::<u8>(),
            count,
        )
    }
}

/// `UDownload::DownloadError` body (`appStrcpy(Error, InError)`).
///
/// # Safety
/// `this` is a live `UDownload`; `msg` is a NUL-terminated TCHAR string.
pub unsafe fn download_error(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    msg: *const c_void,
) -> R<()> {
    let _ = svc;
    unsafe {
        crate::download_owner::copy_error(dl_of(this), msg.cast()).map_err(|_| Thrown)
    }
}

/// `UDownload::DownloadDone`.
///
/// # Safety
/// `this` is a live `UDownload`.
pub unsafe fn download_done(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    let mut ops = NativeDownloadOps { services: svc, cache_ini: null_mut() };
    unsafe { crate::download_owner::download_done_with(&mut ops, dl_of(this)) }
}

/// `UDownload::TrySkipFile`. Returns `UBOOL` as `i32`.
///
/// # Safety
/// `this` is a live `UDownload`.
pub unsafe fn download_try_skip_file(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<i32> {
    unsafe {
        let d = dl_of(this);
        let file_open = !(*d).recv_file_ar.is_null();
        if !file_open {
            return Ok(0);
        }
        let flags = out::<u32>(|p| (svc.demo_info_package_flags)((*d).info, p))?;
        if crate::download_owner::skip_eligible(file_open, flags) {
            (*d).skipped_file = 1;
            return Ok(1);
        }
        Ok(0)
    }
}

/// `UDownload::Destroy`.
///
/// # Safety
/// `this` is a live `UDownload` being destroyed.
pub unsafe fn download_destroy(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    let mut ops = NativeDownloadOps { services: svc, cache_ini: null_mut() };
    unsafe { crate::download_owner::destroy_with(&mut ops, dl_of(this))?; }
    st(unsafe { (svc.demo_obj_super_destroy)(this) })
}

// ---------------------------------------------------------------------------
// UChannelDownload
// ---------------------------------------------------------------------------

/// `UChannelDownload::ReceiveFile`.
///
/// # Safety
/// `this` is a live `UChannelDownload`; `conn` is a live `UNetConnection`.
struct NativeChannelReceiveOps<'a> {
    services: &'a Hp2NetDemoServices,
}

impl crate::download_owner::ChannelReceiveOps for NativeChannelReceiveOps<'_> {
    type Error = Thrown;

    unsafe fn receive_base(
        &mut self,
        download: *mut UChannelDownload,
        connection: *mut hp2_abi::engine::generated::UNetConnection,
        package_index: i32,
        params: *const u32,
        compression: bool,
    ) -> R<()> {
        unsafe {
            download_receive_file(
                self.services, download.cast(), connection.cast(), package_index, params,
                i32::from(compression),
            )
        }
    }
    unsafe fn create_file_channel(
        &mut self,
        connection: *mut hp2_abi::engine::generated::UNetConnection,
    ) -> R<*mut hp2_abi::engine::generated::UChannel> {
        out::<*mut c_void>(|p| unsafe {
            (self.services.demo_conn_create_file_channel)(connection.cast(), p)
        }).map(|channel| channel.cast())
    }
    unsafe fn attach(
        &mut self,
        channel: *mut hp2_abi::engine::generated::UChannel,
        download: *mut UChannelDownload,
        package_index: i32,
    ) -> R<()> {
        unsafe {
            (*download).ch = channel.cast();
            st((self.services.demo_chdl_set_ch_download)(channel.cast(), download.cast()))?;
            st((self.services.demo_chdl_set_package_index)(channel.cast(), package_index))
        }
    }
    unsafe fn allocation_failed(&mut self, download: *mut UChannelDownload) -> R<()> {
        unsafe {
            localized_error(self.services, download.cast(), c"ChAllocate", &[])?;
            st((self.services.demo_dl_done_virtual)(download.cast()))
        }
    }
    unsafe fn send_guid(
        &mut self,
        channel: *mut hp2_abi::engine::generated::UChannel,
        download: *mut UChannelDownload,
    ) -> R<()> {
        unsafe {
            crate::net_bunch::with_out(&self.services.bunch, channel.cast(), 0, |bunch| {
                let bunch = bunch.cast::<c_void>();
                let result: R<()> = (|| {
                    st((self.services.demo_bunch_write_guid)(bunch, (*dl_of(download.cast())).info))?;
                    st((self.services.demo_bunch_set_reliable)(bunch))?;
                    if out(|p| (self.services.demo_bunch_is_error)(bunch, p))? != 0 {
                        st((self.services.demo_app_fail_assert)(
                            c"!Bunch.IsError()".as_ptr(),
                            c"UnDownload.cpp".as_ptr(),
                            259,
                        ))?;
                    }
                    st((self.services.demo_bunch_send)(channel.cast(), bunch))
                })();
                result.map_err(|_| crate::net_chan::Thrown)
            }).map_err(|_| crate::net_demo::Thrown)
        }
    }
}

pub unsafe fn chdl_receive_file(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    conn: *mut c_void,
    package_index: i32,
    params: *const u32,
    compression: i32,
) -> R<()> {
    let mut ops = NativeChannelReceiveOps { services: svc };
    unsafe {
        crate::download_owner::channel_receive_file_with(
            &mut ops, chdl_of(this), conn.cast(), package_index, params, compression != 0,
        )
    }
}
struct NativeChannelSkipOps<'a> {
    services: &'a Hp2NetDemoServices,
}

impl crate::download_owner::ChannelSkipOps for NativeChannelSkipOps<'_> {
    type Error = Thrown;
    unsafe fn channel(&mut self, download: *mut UChannelDownload) -> R<*mut hp2_abi::engine::generated::UChannel> {
        if download.is_null() { return Err(Thrown); }
        Ok(unsafe { (*download).ch.cast() })
    }
    unsafe fn try_skip(&mut self, download: *mut UChannelDownload) -> R<i32> {
        unsafe { download_try_skip_file(self.services, download.cast()) }
    }
    unsafe fn send_skip(&mut self, channel: *mut hp2_abi::engine::generated::UChannel) -> R<()> {
        unsafe {
            crate::net_bunch::with_out(&self.services.bunch, channel.cast(), 1, |bunch| {
                let bunch = bunch.cast::<c_void>();
                let result: R<()> = (|| {
                    let command = wide("SKIP");
                    st((self.services.demo_bunch_write_text)(bunch, command.as_ptr().cast()))?;
                    st((self.services.demo_bunch_set_reliable)(bunch))?;
                    st((self.services.demo_bunch_send)(channel.cast(), bunch))
                })();
                result.map_err(|_| crate::net_chan::Thrown)
            }).map_err(|_| crate::net_demo::Thrown)
        }
    }
}

/// `UChannelDownload::TrySkipFile`. Returns `UBOOL` as `i32`.
///
/// # Safety
/// `this` is a live `UChannelDownload`.
pub unsafe fn chdl_try_skip_file(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<i32> {
    let mut ops = NativeChannelSkipOps { services: svc };
    unsafe { crate::download_owner::channel_try_skip_with(&mut ops, chdl_of(this)) }
}

/// `UChannelDownload::Destroy`.
///
/// # Safety
/// `this` is a live `UChannelDownload` being destroyed.
pub unsafe fn chdl_destroy(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    unsafe {
        let c = chdl_of(this);
        if !(*c).ch.is_null()
            && out::<*mut c_void>(|p| (svc.demo_chdl_ch_download)((*c).ch, p))? == this
        {
            st((svc.demo_chdl_set_ch_download)((*c).ch, null_mut()))?;
        }
        (*c).ch = null_mut();
        download_destroy(svc, this)
    }
}

/// `UDownload` and `UChannelDownload` class-default policy.
///
/// The C++ shell keeps the typed `FString` assignment because its packed
/// engine storage is intentionally opaque to Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticDefaults {
    Download,
    ChannelDownload,
}

/// `DownloadParams` source: empty for `UDownload`, `"Enabled"` for
/// `UChannelDownload`.
#[inline]
pub fn static_params(defaults: StaticDefaults) -> i32 {
    match defaults {
        StaticDefaults::Download => 0,
        StaticDefaults::ChannelDownload => 1,
    }
}

/// `UDownload::UseCompression` default. The channel-derived constructor leaves
/// the base-class value intact.
#[inline]
pub fn static_use_compression(defaults: StaticDefaults) -> Option<i32> {
    match defaults {
        StaticDefaults::Download => Some(0),
        StaticDefaults::ChannelDownload => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FormatArg, StaticDefaults, localized_format, static_params, static_use_compression,
    };

    fn assert_format(format: &str, args: &[FormatArg<'_>], expected: &str) {
        let actual = localized_format(format, args);
        assert_eq!(actual.last(), Some(&0));
        let expected: Vec<u32> = expected.chars().map(u32::from).chain([0]).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn class_defaults_preserve_base_and_channel_values() {
        assert_eq!(static_params(StaticDefaults::Download), 0);
        assert_eq!(static_use_compression(StaticDefaults::Download), Some(0));
        assert_eq!(static_params(StaticDefaults::ChannelDownload), 1);
        assert_eq!(
            static_use_compression(StaticDefaults::ChannelDownload),
            None
        );
    }

    #[test]
    fn localized_download_messages_use_the_supplied_format() {
        assert_format(
            "Réception de '%s' (F10 pour annuler)",
            &[FormatArg::Text("Château")],
            "Réception de 'Château' (F10 pour annuler)",
        );
        assert_format(
            "Taille %iK, Total %3.1f%%",
            &[FormatArg::Int(2048), FormatArg::Float(12.5)],
            "Taille 2048K, Total 12.5%",
        );
        assert_format(
            "%dK: %.0f%%",
            &[FormatArg::Int(-1), FormatArg::Float(87.5)],
            "-1K: 88%",
        );
        assert_format("100%%", &[], "100%");
    }

    #[test]
    fn string_width_and_precision_count_tchar_characters() {
        assert_format(
            "[%6.3s][%-5.2s]",
            &[FormatArg::Text("école"), FormatArg::Text("雪山林")],
            "[   éco][雪山   ]",
        );
    }

    #[test]
    fn integer_width_precision_and_sign_match_printf() {
        assert_format(
            "[%+06d][% 5i][%-5d][%05.3i]",
            &[
                FormatArg::Int(42),
                FormatArg::Int(7),
                FormatArg::Int(-9),
                FormatArg::Int(12),
            ],
            "[+00042][    7][-9   ][  012]",
        );
        assert_format(
            "[%.0d][%+.0d][%d]",
            &[
                FormatArg::Int(0),
                FormatArg::Int(0),
                FormatArg::Int(i32::MIN),
            ],
            "[][+][-2147483648]",
        );
    }

    #[test]
    fn floating_width_precision_sign_and_alternate_form_match_printf() {
        assert_format(
            "[%+08.2f][%-7.1f][% .0f][%#.0f]",
            &[
                FormatArg::Float(12.5),
                FormatArg::Float(-2.25),
                FormatArg::Float(2.5),
                FormatArg::Float(3.0),
            ],
            "[+0012.50][-2.2   ][ 2][3.]",
        );
        assert_format(
            "[%f][%+.1f][%07.1f]",
            &[
                FormatArg::Float(1.25),
                FormatArg::Float(0.0),
                FormatArg::Float(-0.0),
            ],
            "[1.250000][+0.0][-0000.0]",
        );
    }

    #[test]
    fn nonfinite_values_preserve_sign_case_and_space_padding() {
        assert_format(
            "[%08.1f][%+08F][%08f][%f][%F]",
            &[
                FormatArg::Float(f64::INFINITY),
                FormatArg::Float(f64::INFINITY),
                FormatArg::Float(f64::NEG_INFINITY),
                FormatArg::Float(f64::NAN.copysign(1.0)),
                FormatArg::Float(f64::NAN.copysign(-1.0)),
            ],
            "[     inf][    +INF][    -inf][nan][-NAN]",
        );
    }

    #[test]
    fn receive_percentage_rounds_in_f32_before_printf_promotion() {
        // 3/2000 is 0.15 percent. Its f32 value lies above the decimal
        // half-way point, while a direct f64 calculation lies below it.
        let transferred = 3i32;
        let file_size = 2000i32;
        let percent = 100.0f32 * transferred as f32 / file_size as f32;
        assert_format("%.1f%%", &[FormatArg::Float(percent as f64)], "0.2%");
        let double_percent = 100.0f64 * transferred as f64 / file_size as f64;
        assert_format("%.1f%%", &[FormatArg::Float(double_percent)], "0.1%");
    }
}
