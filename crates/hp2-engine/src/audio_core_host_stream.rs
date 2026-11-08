use hp2_abi::engine::generated::USound;
use hp2_audio::host::{HostResult, VorbisInfo};
use hp2_core::{ansi::Tchar, core_access::{CoreAccess, RuntimeError}};
use std::{ffi::c_void, ptr};

use super::CoreHost;

struct SourceContext<'a> {
    access: CoreAccess<'a>,
    sound: *mut USound,
    xa: bool,
    path: Vec<Tchar>,
    error: Option<RuntimeError>,
}
/// Filename sources are native TCHARs, relative to System; file_stream converts to UTF-8.
unsafe fn rooted_ogg_path(
    data_root: &std::path::Path,
    input: *const Tchar,
) -> Result<Vec<Tchar>, RuntimeError> {
    let root = data_root.to_str()
        .ok_or_else(|| RuntimeError::new("Canonical Ogg data root is not UTF-8"))?;
    let units = unsafe { hp2_core::unix_text::tchar_length(input) };
    let capacity = units.checked_add(root.len()).and_then(|length| length.checked_add(9))
        .ok_or_else(|| RuntimeError::new("Canonical Ogg filename is too long"))?;
    let mut path = Vec::with_capacity(capacity);
    path.extend(root.chars().map(|ch| ch as Tchar));
    if !root.ends_with('/') { path.push(b'/' as Tchar); }
    path.extend(b"System/".iter().map(|&byte| byte as Tchar));
    path.extend_from_slice(unsafe { std::slice::from_raw_parts(input, units) });
    path.push(0);
    Ok(path)
}


unsafe extern "C" fn stream_source(
    opaque: *mut c_void,
    bytes: *mut *const u8,
    length: *mut usize,
    valid: *mut i32,
) -> i32 {
    if opaque.is_null() || bytes.is_null() || length.is_null() || valid.is_null() { return 1; }
    let source = unsafe { &mut *opaque.cast::<SourceContext<'_>>() };
    if source.sound.is_null() { source.error = Some(RuntimeError::new("Null canonical sound for audio stream")); return 1; }
    if source.xa {
        let data = unsafe { ptr::addr_of_mut!((*source.sound).data).cast::<crate::audio::SoundDataRaw>() };
        let slots = unsafe { super::sound::lazy_sound_slots(data) };
        if let Err(error) = unsafe { hp2_core::lazy_array::lazy_array_load(source.access, slots) } {
            source.error = Some(error);
            return 1;
        }
        let array = unsafe { ptr::addr_of!((*data).lazy.array).read_unaligned() };
        if array.array_num < 0 || (array.array_num > 0 && array.data.is_null()) {
            source.error = Some(RuntimeError::new("Invalid canonical XA sound data"));
            return 1;
        }
        unsafe {
            bytes.write(array.data.cast());
            length.write(array.array_num as usize);
            valid.write(1);
        }
        return 0;
    }
    let filename = unsafe { ptr::addr_of!((*source.sound).filename).read_unaligned() };
    if filename.num <= 0 || filename.data.is_null() {
        source.error = Some(RuntimeError::new("Invalid canonical Ogg filename"));
        return 1;
    }
    let input = filename.data.cast::<Tchar>() as *const Tchar;
    let first = unsafe { input.read() };
    let path = if first == b'/' as Tchar || first == b'\\' as Tchar {
        input.cast()
    } else {
        match unsafe { rooted_ogg_path(source.access.data_root(), input) } {
            Ok(path) => source.path = path,
            Err(error) => {
                source.error = Some(error);
                return 1;
            }
        }
        source.path.as_ptr().cast()
    };
    unsafe {
        bytes.write(path);
        length.write(0);
        valid.write(1);
    }
    0
}

fn stream_manager(host: &mut CoreHost<'_>) -> HostResult<*mut c_void> {
    let manager = hp2_audio::file_stream::hp2_rs_stream_manager();
    if manager.is_null() {
        Err(host.fail_with(RuntimeError::new("Canonical audio file-stream manager is not initialized")))
    } else {
        Ok(manager)
    }
}

fn sound(host: &mut CoreHost<'_>, pointer: usize) -> HostResult<*mut USound> {
    if pointer == 0 {
        Err(host.fail_with(RuntimeError::new("Null canonical USound for audio stream")))
    } else {
        Ok(pointer as *mut USound)
    }
}

fn callback_error(host: &mut CoreHost<'_>, source: &mut SourceContext<'_>, status: i32) -> HostResult<()> {
    if status == 0 { return Ok(()); }
    Err(host.fail_with(source.error.take().unwrap_or_else(|| RuntimeError::new("Canonical audio stream source failed"))))
}

fn create_stream(
    host: &mut CoreHost<'_>, raw: usize, xa: bool, samples: i32, chunk: i32, num: i32,
    prime: &mut [u8], looping: bool, decoder: *mut c_void,
) -> HostResult<i32> {
    let sound = sound(host, raw)?;
    let manager = stream_manager(host)?;
    let mut source = SourceContext { access: host.access, sound, xa, path: Vec::new(), error: None };
    let kind = match (xa, looping) {
        (false, false) => hp2_audio::file_stream::HP2_STREAM_OGG,
        (false, true) => hp2_audio::file_stream::HP2_STREAM_OGG_LOOPING,
        (true, false) => hp2_audio::file_stream::HP2_STREAM_XA,
        (true, true) => hp2_audio::file_stream::HP2_STREAM_XA_LOOPING,
    };
    let mut id = -1;
    let status = unsafe {
        hp2_audio::file_stream::hp2_rs_stream_create(
            manager, stream_source, (&mut source as *mut SourceContext).cast(),
            i32::from(xa), samples, chunk, num, prime.as_mut_ptr().cast(), kind,
            decoder, &mut id,
        )
    };
    callback_error(host, &mut source, status)?;
    Ok(id)
}

impl CoreHost<'_> {
    pub(super) fn host_vorbis_new(&mut self) -> HostResult<usize> {
        Ok(Box::into_raw(Box::new(hp2_codecs::vorbis::Decoder::new())) as usize)
    }

    pub(super) fn host_vorbis_free(&mut self, decoder: usize) -> HostResult<()> {
        if decoder != 0 {
            unsafe { drop(Box::from_raw(decoder as *mut hp2_codecs::vorbis::Decoder)); }
        }
        Ok(())
    }

    pub(super) fn host_vorbis_info(&mut self, decoder: usize) -> HostResult<VorbisInfo> {
        if decoder == 0 { return Err(self.fail_with(RuntimeError::new("Null Ogg decoder"))); }
        let decoder = unsafe { &*(decoder as *const hp2_codecs::vorbis::Decoder) };
        Ok(VorbisInfo { pcm_total: decoder.pcm_total(), rate: decoder.rate(), channels: decoder.channels(), time_total: decoder.time_total() })
    }

    pub(super) fn host_gfile_create_ogg_stream(
        &mut self, sound: usize, chunk: i32, num: i32, prime: &mut [u8], looping: bool, decoder: usize,
    ) -> HostResult<i32> {
        if decoder == 0 { return Err(self.fail_with(RuntimeError::new("Null Ogg decoder for audio stream"))); }
        create_stream(self, sound, false, -1, chunk, num, prime, looping, decoder as *mut c_void)
    }

    pub(super) fn host_gfile_create_xa_stream(
        &mut self, sound: usize, samples: i32, chunk: i32, num: i32, prime: &mut [u8], looping: bool,
    ) -> HostResult<i32> {
        create_stream(self, sound, true, samples, chunk, num, prime, looping, ptr::null_mut())
    }

    pub(super) fn host_gfile_destroy_stream(&mut self, id: i32) -> HostResult<()> {
        let manager = stream_manager(self)?;
        unsafe { hp2_audio::file_stream::hp2_rs_stream_destroy(manager, id, 0); }
        Ok(())
    }

    pub(super) fn host_gfile_request_chunk(&mut self, id: i32, destination: usize) -> HostResult<bool> {
        let manager = stream_manager(self)?;
        Ok(unsafe { hp2_audio::file_stream::hp2_rs_stream_request(manager, id, destination as *mut c_void) != 0 })
    }

    pub(super) fn host_gfile_pop_completed_chunk(&mut self, id: i32) -> HostResult<Option<(usize, bool)>> {
        let manager = stream_manager(self)?;
        let mut destination = ptr::null_mut();
        let mut terminal = 0;
        let has = unsafe { hp2_audio::file_stream::hp2_rs_stream_pop(manager, id, &mut destination, &mut terminal) };
        Ok((has != 0).then_some((destination as usize, terminal != 0)))
    }
}
#[cfg(test)]
mod sound_stream_owner_contracts {
    use super::*;
    use hp2_core::{core_access::CoreOptions, name_owner::FindName};
    use std::path::PathBuf;

    struct StreamManager;
    impl Drop for StreamManager {
        fn drop(&mut self) {
            unsafe { hp2_audio::file_stream::hp2_rs_stream_shutdown(); }
        }
    }
    #[test]
    fn stock_ogg_stream_decodes_native_relative_and_absolute_names() {
        let prototype = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prototype music"));
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("core-ogg-source-{}", std::process::id()));
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::create_dir_all(root.join("Music")).unwrap();
        std::fs::write(root.join("System/Default.ini"), "").unwrap();
        std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
        let filename = "sm_bur_PlayfulFail_01.ogg";
        let music = root.join("Music").join(filename);
        std::fs::copy(prototype.join("Music").join(filename), &music).unwrap();
        let owner = crate::EngineOwner::new(CoreOptions {
            data_root: root.clone(), system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(),
            output: |_, _, _| Ok(()), log_file: None,
        }).unwrap();
        let access = owner.access();
        let sound = unsafe {
            access.construct_object(
                access.native_class("Engine.Sound").unwrap(), access.transient_package(),
                access.intern_str("StreamFixture", FindName::Add).unwrap(), 0x4000, ptr::null_mut(),
            )
        }.unwrap().cast::<USound>();
        let mut manager = ptr::null_mut();
        assert_eq!(unsafe { hp2_audio::file_stream::hp2_rs_stream_init(2, &mut manager) }, 0);
        let _manager = StreamManager;
        let mut host = CoreHost::new(access);
        let relative = format!("..\\Music\\{filename}");
        for filename in [relative.as_str(), music.to_str().unwrap()] {
            let units: Vec<u32> = filename.chars().map(u32::from).chain([0]).collect();
            unsafe { access.assign_fstring(ptr::addr_of_mut!((*sound).filename), &units) }.unwrap();
            let decoder = host.host_vorbis_new().unwrap();
            let mut prime = [0u8; 512];
            let id = host.host_gfile_create_ogg_stream(
                sound as usize, 256, 2, &mut prime, true, decoder,
            ).unwrap();
            if id < 0 { host.host_vorbis_free(decoder).unwrap(); }
            assert!(id >= 0, "the actual file-stream consumer must decode {filename}");
            let info = host.host_vorbis_info(decoder).unwrap();
            assert_eq!(
                (info.pcm_total, info.rate, info.channels), (441344, 44100, 2),
                "the stock stream retains its actual sample count and stereo format",
            );
            host.host_gfile_destroy_stream(id).unwrap();
        }
        owner.shutdown().unwrap();
    }
}
