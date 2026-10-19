//! Guarded C exports for `Engine/Src/UnAudio.cpp` (see `hp2_engine::audio`).
//! Each export that reaches the engine takes the `Hp2AudioServices` table the
//! C++ shell passes in. Object arguments are opaque engine pointers. Booleans
//! cross as `i32`, matching `UBOOL`.

use std::ffi::c_void;

use hp2_abi::engine::generated::{FWaveModInfo, USound};
use hp2_abi::render_scene::TArrayRaw;
use hp2_engine::audio::{self, Hp2AudioServices, LipSyncDataRaw, SoundDataRaw};
use hp2_ffi_support::guard;

/// # Safety
/// `svc` points to a live `Hp2AudioServices` for the duration of the call.
unsafe fn services<'a>(svc: *const Hp2AudioServices) -> &'a Hp2AudioServices {
    unsafe { &*svc }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_fsound_load(
    data: *mut c_void,
    svc: *const Hp2AudioServices,
) {
    guard("hp2_rs_engine_audio_fsound_load", (), move || unsafe {
        audio::sound_data_load(data as *mut SoundDataRaw, services(svc));
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_fsound_get_period(
    data: *mut c_void,
    svc: *const Hp2AudioServices,
) -> f32 {
    guard(
        "hp2_rs_engine_audio_fsound_get_period",
        0.0,
        move || unsafe { audio::sound_data_get_period(data as *mut SoundDataRaw, services(svc)) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_lip_get_mouth_pos(lip: *mut c_void, t: f32) -> f32 {
    guard(
        "hp2_rs_engine_audio_lip_get_mouth_pos",
        0.0,
        move || unsafe { audio::lip_mouth_pos(lip as *mut LipSyncDataRaw, t) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_wave_read_info(
    w: *mut c_void,
    arr: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_audio_wave_read_info", 0, move || unsafe {
        audio::read_wave_info(w as *mut FWaveModInfo, arr as *const TArrayRaw) as i32
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_serialize(
    sound: *mut c_void,
    ar: *mut c_void,
    svc: *const Hp2AudioServices,
) {
    guard("hp2_rs_engine_audio_usound_serialize", (), move || unsafe {
        audio::usound_serialize(sound as *mut USound, ar, services(svc));
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_import_post_process(
    sound: *mut c_void,
    filename: *const c_void,
    parms: *const c_void,
    svc: *const Hp2AudioServices,
) {
    guard(
        "hp2_rs_engine_audio_usound_import_post_process",
        (),
        move || unsafe {
            audio::usound_import_post_process(sound as *mut USound, filename, parms, services(svc));
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_calc_lip_sync(
    sound: *mut c_void,
    svc: *const Hp2AudioServices,
) -> i32 {
    guard(
        "hp2_rs_engine_audio_usound_calc_lip_sync",
        0,
        move || unsafe { audio::usound_calc_lip_sync(sound as *mut USound, services(svc)) as i32 },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_compress_xa(
    sound: *mut c_void,
    filename: *const c_void,
    svc: *const Hp2AudioServices,
) -> i32 {
    guard(
        "hp2_rs_engine_audio_usound_compress_xa",
        0,
        move || unsafe {
            audio::usound_compress_xa(sound as *mut USound, filename, services(svc)) as i32
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_bind_stream(
    sound: *mut c_void,
    id: i32,
    svc: *const Hp2AudioServices,
) {
    guard(
        "hp2_rs_engine_audio_usound_bind_stream",
        (),
        move || unsafe {
            audio::usound_bind_stream(sound as *mut USound, id, services(svc));
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_unbind_stream(
    sound: *mut c_void,
    id: i32,
    svc: *const Hp2AudioServices,
) {
    guard(
        "hp2_rs_engine_audio_usound_unbind_stream",
        (),
        move || unsafe {
            audio::usound_unbind_stream(sound as *mut USound, id, services(svc));
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_unbind_all_streams(
    sound: *mut c_void,
    svc: *const Hp2AudioServices,
) {
    guard(
        "hp2_rs_engine_audio_usound_unbind_all_streams",
        (),
        move || unsafe {
            audio::usound_unbind_all_streams(sound as *mut USound, services(svc));
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_is_registered(sound: *mut c_void) -> i32 {
    guard(
        "hp2_rs_engine_audio_usound_is_registered",
        0,
        move || unsafe { audio::usound_is_registered(sound as *mut USound) as i32 },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_set_handle(sound: *mut c_void, handle: i32) {
    guard(
        "hp2_rs_engine_audio_usound_set_handle",
        (),
        move || unsafe {
            audio::usound_set_handle(sound as *mut USound, handle);
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_add_stream_handle(
    sound: *mut c_void,
    id: i32,
    svc: *const Hp2AudioServices,
) {
    guard(
        "hp2_rs_engine_audio_usound_add_stream_handle",
        (),
        move || unsafe {
            audio::usound_add_stream_handle(sound as *mut USound, id, services(svc));
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_delete_stream_handle(
    sound: *mut c_void,
    id: i32,
    svc: *const Hp2AudioServices,
) {
    guard(
        "hp2_rs_engine_audio_usound_delete_stream_handle",
        (),
        move || unsafe {
            audio::usound_delete_stream_handle(sound as *mut USound, id, services(svc));
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_delete_all_stream_handles(
    sound: *mut c_void,
    svc: *const Hp2AudioServices,
) {
    guard(
        "hp2_rs_engine_audio_usound_delete_all_stream_handles",
        (),
        move || unsafe {
            audio::usound_delete_all_stream_handles(sound as *mut USound, services(svc));
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_audio_usound_get_handle(
    sound: *mut c_void,
    id: i32,
    svc: *const Hp2AudioServices,
) -> i32 {
    guard("hp2_rs_engine_audio_usound_get_handle", 0, move || unsafe {
        audio::usound_get_handle(sound as *mut USound, id, services(svc))
    })
}
