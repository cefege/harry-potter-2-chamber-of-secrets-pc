//! Port of `Engine/Src/UnAudio.cpp`: `USound`, `FSoundData`, `FLipSyncData`,
//! and `FWaveModInfo`.
//!
//! The C++ shell keeps the UObject plumbing: `IMPLEMENT_CLASS`,
//! `USound::Destroy`, `USound::PostLoad`, and the `FLipSyncData` constructor.
//! Every other method forwards here. Engine internals that this port does not
//! model (TLazyArray loading, TArray and TMap allocation, FName, FArchive
//! operators, `Parse`, and log calls) go through [`Hp2AudioServices`].
//!
//! Divergences from the C++ (all on paths C leaves undefined or crashing):
//! - Float-to-int casts saturate in Rust; C is undefined for out-of-range values.
//! - `ReadWaveInfo` returns false when the array holds fewer than 12 bytes.
//!   C reads past the array.
//! - Chunk ID and length reads past the array end yield 0, so they never match
//!   and the chunk walk stops. C reads the bytes beyond the array.
//! - A truncated `smpl` chunk reads `cSampleLoops` as 0 when the field is past
//!   the array end. C reads it unchecked.
//! - `CalcLipSyncData` returns false when `ReadWaveInfo` fails. C ignores the
//!   result and dereferences unset wave pointers.
//! - `CompressXA` returns false when `ReadWaveInfo` fails. C dereferences
//!   unset wave pointers.
//! - Division by zero (a zero channel count or a zero `new_data_size`) panics
//!   in Rust where C faults.
//! - Reads that C performs unchecked stay unchecked: the lip-sync index reads
//!   in `GetInterpAmplitude`, the PS2 `GetPeriod` read, and the wave-info
//!   pointer dereferences after a successful `ReadWaveInfo`.
//! - The `_WINDOWS` branch of `CompressXA` and the PS2 `ReadWaveInfo` branch
//!   are removed. On non-Windows builds `CompressXA` always returned false.
//!   The `HAVE_VAG` body of `PS2Convert` is removed because `HAVE_VAG` is 0.
//! - The RMS and AVERAGE sample-window modes (`#if 0`) are removed.
//! - Sample windows are NOT clamped: as in C, a window overhanging
//!   `SampleDataEnd` reads the bytes after the data chunk (fixtured).

use std::ffi::c_void;
use std::mem::{offset_of, size_of, zeroed};
use std::ptr::{addr_of_mut, copy};

use hp2_abi::engine::generated::{FWaveModInfo, USound};
use hp2_abi::render_scene::TArrayRaw;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `SF_Streaming` in `UnAudio.h`.
pub const SF_STREAMING: i32 = 4;
/// `SF_HasLipSync` in `UnAudio.h`.
pub const SF_HAS_LIP_SYNC: i32 = 128;
/// `SF_Compressed` in `UnAudio.h`.
pub const SF_COMPRESSED: i32 = 256;

/// `FLipSyncData::fAmpSampleInterval`.
const LIP_AMP_SAMPLE_INTERVAL: f32 = 0.020;
/// `FLipSyncData::fAmpSampleWindowSize`.
const LIP_AMP_SAMPLE_WINDOW_SIZE: f32 = 0.005;
/// Mouth-position scale: `kfAnimEndRange * kfMuteFactor`.
const MOUTH_ANIM_END_RANGE: f32 = 0.9;
const MOUTH_MUTE_FACTOR: f32 = 0.7;

/// Smallest file that is compressed: `32 * 1024 + 32 / 8`.
const SMALLEST_COMPRESSABLE_BYTES: u32 = 32 * 1024 + 32 / 8;

/// Compile-time switches from the original `if(0)` branch of
/// `FSoundData::Load`. Both are false, so that branch never runs.
const FORCE_8BIT: bool = false;
const FORCE_HALVE: bool = false;

const FOURCC_WAVE: u32 = u32::from_le_bytes(*b"WAVE");
const FOURCC_FMT: u32 = u32::from_le_bytes(*b"fmt ");
const FOURCC_DATA: u32 = u32::from_le_bytes(*b"data");
const FOURCC_SMPL: u32 = u32::from_le_bytes(*b"smpl");

/// Parse keys checked by `ImportPostProcess`.
pub const PARSE_COMPRESS: u32 = 0;
pub const PARSE_LIPSYNC: u32 = 1;

/// Sample-window alignment for `GetSampleWindowAmplitude`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SampleWindow {
    Trailing,
    Leading,
    Centered,
}

// ---------------------------------------------------------------------------
// Services
// ---------------------------------------------------------------------------

/// Engine services implemented in C++ (`Hp2AudioServices.cpp`). Field order
/// matches the `const` table there.
///
/// Array arguments are the address of the `TArray` subobject. Lazy arguments
/// are the address of the owning `FSoundData` or `FLipSyncData`. Map arguments
/// are the owning `USound`. Archive arguments are the `FArchive*`.
#[repr(C)]
pub struct Hp2AudioServices {
    /// `TLazyArray<BYTE>::Load()`.
    pub lazy_load: unsafe extern "C" fn(lazy: *mut c_void),
    /// `TLazyArray<BYTE>::Unload()`.
    pub lazy_unload: unsafe extern "C" fn(lazy: *mut c_void),
    /// `TLazyArray<BYTE>::SavedPos`.
    pub lazy_saved_pos: unsafe extern "C" fn(lazy: *const c_void) -> i32,
    /// `TArray<BYTE>::Set(count)`.
    pub array_u8_set: unsafe extern "C" fn(arr: *mut c_void, count: i32),
    /// `TArray<BYTE>::Remove(index, count)`.
    pub array_u8_remove: unsafe extern "C" fn(arr: *mut c_void, index: i32, count: i32),
    /// `TArray<INT>::Add(1)`.
    pub array_i32_add: unsafe extern "C" fn(arr: *mut c_void),
    /// `TArray<INT>::Remove(index, 1)`.
    pub array_i32_remove: unsafe extern "C" fn(arr: *mut c_void, index: i32),
    /// `TArray<INT>::Empty()`.
    pub array_i32_empty: unsafe extern "C" fn(arr: *mut c_void),
    /// `USound::PlayingStreams.Set(key, value)`.
    pub map_set: unsafe extern "C" fn(sound: *mut c_void, key: i32, value: i32),
    /// `USound::PlayingStreams.FindRef(key)`.
    pub map_find_ref: unsafe extern "C" fn(sound: *mut c_void, key: i32) -> i32,
    /// `USound::PlayingStreams.Remove(key)`.
    pub map_remove: unsafe extern "C" fn(sound: *mut c_void, key: i32),
    /// `USound::PlayingStreams.Empty()`.
    pub map_empty: unsafe extern "C" fn(sound: *mut c_void),
    /// `USound::FileType == FName("PS2")`.
    pub file_type_is_ps2: unsafe extern "C" fn(sound: *const c_void) -> i32,
    /// `Parse(parms, "Compress=" | "Lipsync=")` compared to its expected value.
    /// `which` is `PARSE_COMPRESS` or `PARSE_LIPSYNC`.
    pub parse_is: unsafe extern "C" fn(parms: *const c_void, which: u32) -> i32,
    /// `GWarn->Logf("Sample rate not supported: ...")`.
    pub log_sample_rate_unsupported: unsafe extern "C" fn(filename: *const c_void, rate: i32),
    /// `debugf(NAME_DevSound, "Deleteing a bound stream!")`.
    pub log_deleting_bound_stream: unsafe extern "C" fn(),
    /// `Ar.IsLoading()`.
    pub archive_is_loading: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar.IsSaving()`.
    pub archive_is_saving: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar.Ver()`.
    pub archive_ver: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `Ar << *(INT*)value`.
    pub archive_serialize_i32: unsafe extern "C" fn(ar: *mut c_void, value: *mut c_void),
    /// `Ar << *(FLOAT*)value`.
    pub archive_serialize_f32: unsafe extern "C" fn(ar: *mut c_void, value: *mut c_void),
    /// `Ar << *(FName*)value`.
    pub archive_serialize_name: unsafe extern "C" fn(ar: *mut c_void, value: *mut c_void),
    /// `Ar << *(FSoundData*)value`.
    pub archive_serialize_sound_data: unsafe extern "C" fn(ar: *mut c_void, value: *mut c_void),
    /// `Ar << *(FLipSyncData*)value`.
    pub archive_serialize_lip_sync: unsafe extern "C" fn(ar: *mut c_void, value: *mut c_void),
    /// `Ar.CountBytes(num, max)`.
    pub archive_count_bytes: unsafe extern "C" fn(ar: *mut c_void, num: i32, max: i32),
    /// `Super::Serialize(Ar)` for `USound`.
    pub sound_super_serialize: unsafe extern "C" fn(sound: *mut c_void, ar: *mut c_void),
}

// ---------------------------------------------------------------------------
// Layout mirrors
// ---------------------------------------------------------------------------

/// Head of `TLazyArray<BYTE>`: the `FLazyLoader` base (vptr, SavedAr,
/// SavedPos) followed by the `TArray<BYTE>` at offset 24.
#[repr(C)]
pub struct LazyBytes {
    _lazy: [u8; 24],
    pub array: TArrayRaw,
}

/// `FSoundData`: `TLazyArray<BYTE>` plus `Owner` at offset 40.
#[repr(C)]
pub struct SoundDataRaw {
    pub lazy: LazyBytes,
    pub owner: *mut USound,
}

/// `FLipSyncData`: `TLazyArray<BYTE>`, `Owner` at offset 40, and
/// `m_iMaxAmplitude` at offset 48.
#[repr(C)]
pub struct LipSyncDataRaw {
    pub lazy: LazyBytes,
    pub owner: *mut USound,
    pub max_amplitude: i32,
}

const _: () = {
    assert!(size_of::<LazyBytes>() == 40);
    assert!(offset_of!(LazyBytes, array) == 24);
    assert!(size_of::<SoundDataRaw>() == 48);
    assert!(offset_of!(SoundDataRaw, owner) == 40);
    assert!(size_of::<LipSyncDataRaw>() == 56);
    assert!(offset_of!(LipSyncDataRaw, max_amplitude) == 48);
    assert!(offset_of!(USound, data) == 72);
    assert!(offset_of!(USound, lip_sync_data) == 120);
    assert!(offset_of!(USound, original_size) == 176);
    assert!(offset_of!(USound, duration) == 180);
    assert!(offset_of!(USound, open_streams) == 200);
    assert!(offset_of!(USound, handle) == 248);
    assert!(offset_of!(USound, file_type) == 256);
    assert!(offset_of!(USound, core_flags) == 280);
};

// ---------------------------------------------------------------------------
// Raw memory helpers
// ---------------------------------------------------------------------------

/// Unaligned read at address `a`.
///
/// # Safety
/// `a` must be readable for `size_of::<T>()` bytes.
#[inline]
unsafe fn ld<T: Copy>(a: usize) -> T {
    unsafe { (a as *const T).read_unaligned() }
}

/// Unaligned write at address `a`.
///
/// # Safety
/// `a` must be writable for `size_of::<T>()` bytes.
#[inline]
unsafe fn st<T: Copy>(a: usize, v: T) {
    unsafe { (a as *mut T).write_unaligned(v) }
}

/// Reads a `u32` at `a` only when all four bytes are inside `[a, end)`.
///
/// # Safety
/// `end` must be the end of a readable region that contains the valid bytes.
unsafe fn checked_u32(a: usize, end: usize) -> u32 {
    if a.checked_add(4).is_some_and(|e| e <= end) {
        unsafe { ld::<u32>(a) }
    } else {
        0
    }
}

/// `Pad16Bit(x)`: rounds `x` up to an even number.
#[inline]
fn pad16(x: u32) -> u32 {
    x.wrapping_add(1) & !1
}

/// Address of the `TArray<BYTE>` inside a `FSoundData` or `FLipSyncData`.
///
/// # Safety
/// `lazy` must point to a live `LazyBytes`-headed object.
#[inline]
unsafe fn lazy_array(lazy: *mut LazyBytes) -> *mut TArrayRaw {
    unsafe { addr_of_mut!((*lazy).array) }
}

/// Base pointer of a `TArrayRaw` read as an address.
///
/// # Safety
/// `arr` must point to a live `TArrayRaw`.
#[inline]
unsafe fn arr_base(arr: *const TArrayRaw) -> usize {
    unsafe { (*arr).data as usize }
}

/// `FSoundData` embedded at `USound::Data`.
#[inline]
fn sound_data_of(sound: *mut USound) -> *mut SoundDataRaw {
    (sound as usize + offset_of!(USound, data)) as *mut SoundDataRaw
}

/// `FLipSyncData` embedded at `USound::LipSyncData`.
#[inline]
fn lip_data_of(sound: *mut USound) -> *mut LipSyncDataRaw {
    (sound as usize + offset_of!(USound, lip_sync_data)) as *mut LipSyncDataRaw
}

/// `USound::OpenStreams` as a raw array.
#[inline]
fn open_streams_of(sound: *mut USound) -> *mut TArrayRaw {
    (sound as usize + offset_of!(USound, open_streams)) as *mut TArrayRaw
}

// ---------------------------------------------------------------------------
// FWaveModInfo
// ---------------------------------------------------------------------------

/// Builds a zeroed `FWaveModInfo`. The C++ constructor sets `SampleLoopsNum`
/// and `NoiseGate` to 0 and leaves the rest unset, so zero matches its
/// initial state.
pub fn wave_mod_info_new() -> FWaveModInfo {
    // SAFETY: FWaveModInfo is plain data (raw pointers and integers), and
    // all-zero is a valid value.
    unsafe { zeroed() }
}

/// Channel count (`*pChannels`).
///
/// # Safety
/// `w` must hold valid pointers from a successful `read_wave_info`.
#[inline]
unsafe fn channels(w: &FWaveModInfo) -> u32 {
    unsafe { ld::<u16>(w.p_channels as usize) as u32 }
}

/// Bits per sample (`*pBitsPerSample`).
///
/// # Safety
/// As for [`channels`].
#[inline]
unsafe fn bits(w: &FWaveModInfo) -> u32 {
    unsafe { ld::<u16>(w.p_bits_per_sample as usize) as u32 }
}

/// Samples per second (`*pSamplesPerSec`).
///
/// # Safety
/// As for [`channels`].
#[inline]
unsafe fn samples_per_sec(w: &FWaveModInfo) -> u32 {
    unsafe { ld::<u32>(w.p_samples_per_sec as usize) }
}

/// `FWaveModInfo::SampleSize`: `channels * bytes per sample`.
///
/// # Safety
/// As for [`channels`].
unsafe fn sample_size(w: &FWaveModInfo) -> i32 {
    unsafe {
        let bytes = (bits(w) as i32 + 7) / 8;
        channels(w) as i32 * bytes
    }
}

/// First chunk at or after `cur` whose ID is `id`. Stops at `end`.
///
/// # Safety
/// `[cur, end)` must be readable.
unsafe fn find_chunk(mut cur: usize, end: usize, id: u32) -> usize {
    unsafe {
        while cur.wrapping_add(8) < end && checked_u32(cur, end) != id {
            let len = checked_u32(cur.wrapping_add(4), end);
            cur = cur.wrapping_add(pad16(len).wrapping_add(8) as usize);
        }
        cur
    }
}

/// `FWaveModInfo::ReadWaveInfo`, non-PS2 branch.
///
/// # Safety
/// `w` must be writable. `arr` must point to a live `TArrayRaw` whose data is
/// readable for `array_num` bytes.
pub unsafe fn read_wave_info(w: *mut FWaveModInfo, arr: *const TArrayRaw) -> bool {
    unsafe {
        let num = (*arr).array_num;
        if num < 12 {
            return false;
        }
        let base = arr_base(arr);
        let end = base + num as usize;
        (*w).wave_data_end = end as *mut c_void;

        // RIFF header: rID@0, ChunkLen@4, wID@8.
        (*w).p_master_size = (base + 4) as *mut c_void;
        if checked_u32(base + 8, end) != FOURCC_WAVE {
            return false;
        }

        // 'fmt ' chunk: wFormatTag@0, nChannels@2, nSamplesPerSec@4,
        // nAvgBytesPerSec@8, nBlockAlign@12, wBitsPerSample@14, cbSize@16.
        let fmt_chunk = find_chunk(base + 12, end, FOURCC_FMT);
        if checked_u32(fmt_chunk, end) != FOURCC_FMT {
            return false;
        }
        let fmt = fmt_chunk + 8;
        (*w).p_channels = (fmt + 2) as *mut c_void;
        (*w).p_samples_per_sec = (fmt + 4) as *mut c_void;
        (*w).p_avg_bytes_per_sec = (fmt + 8) as *mut c_void;
        (*w).p_block_align = (fmt + 12) as *mut c_void;
        (*w).p_bits_per_sample = (fmt + 14) as *mut c_void;

        // 'data' chunk.
        let data_chunk = find_chunk(base + 12, end, FOURCC_DATA);
        if checked_u32(data_chunk, end) != FOURCC_DATA {
            return false;
        }
        let data_size = checked_u32(data_chunk.wrapping_add(4), end);
        (*w).sample_data_start = (data_chunk + 8) as *mut c_void;
        (*w).p_wave_data_size = (data_chunk + 4) as *mut c_void;
        (*w).sample_data_size = data_size;
        (*w).old_bits_per_sample = bits(&*w);
        (*w).sample_data_end = (data_chunk + 8).wrapping_add(data_size as usize) as *mut c_void;
        (*w).new_data_size = data_size;

        // Optional 'smpl' chunk. cSampleLoops is the eighth DWORD, at +28.
        let smpl_chunk = find_chunk(base + 12, end, FOURCC_SMPL);
        if smpl_chunk + 4 < end && checked_u32(smpl_chunk, end) == FOURCC_SMPL {
            (*w).sample_loops_num = checked_u32(smpl_chunk + 8 + 28, end) as i32;
            (*w).p_sample_loop = (smpl_chunk + 8 + 36) as *mut c_void;
        }
        true
    }
}

/// `FWaveModInfo::OffsetFromTime`: unclamped, as in C.
///
/// # Safety
/// As for [`channels`].
unsafe fn offset_from_time(w: &FWaveModInfo, t: f32) -> usize {
    unsafe {
        let bytes_per_sample = (bits(w) as i32 + 7) / 8;
        let per_second = samples_per_sec(w)
            .wrapping_mul(channels(w))
            .wrapping_mul(bytes_per_sample as u32);
        let offset = ((per_second as f32) * t) as i32;
        let start = w.sample_data_start as usize;
        let mut p = start.wrapping_add_signed(offset as isize);
        if bytes_per_sample > 1 {
            p &= !1;
        }
        p
    }
}

/// `FWaveModInfo::SampleAmplitude`: one sample folded to a byte of amplitude.
///
/// # Safety
/// `p` must address a full sample inside the wave data.
unsafe fn sample_amplitude(w: &FWaveModInfo, p: usize) -> u8 {
    unsafe {
        if bits(w) <= 8 {
            let mut amp: u8 = if channels(w) == 1 {
                ld::<u8>(p)
            } else {
                ((ld::<u8>(p) as u32 + ld::<u8>(p + 1) as u32) / 2) as u8
            };
            // Silence in an 8-bit sample is 128.
            if amp < 128 {
                amp = 128 - amp;
            } else {
                amp -= 128;
            }
            amp
        } else if channels(w) == 1 {
            ((ld::<i16>(p) as i32).abs() / 128) as u8
        } else {
            let a = (ld::<i16>(p) as i32).abs();
            let b = (ld::<i16>(p + 2) as i32).abs();
            ((a + b) / 256) as u8
        }
    }
}

/// `FWaveModInfo::GetSampleWindowAmplitude`: peak amplitude over a window.
///
/// # Safety
/// As for [`sample_amplitude`]. Like the C loop, a window that overhangs the sample data reads
/// the bytes after it (no clamping); see the module-level divergence notes.
pub unsafe fn sample_window_amplitude(
    w: &FWaveModInfo,
    f_time: f32,
    f_window: f32,
    window: SampleWindow,
) -> u32 {
    unsafe {
        let (start, end) = match window {
            SampleWindow::Trailing => (f_time, f_time + f_window),
            SampleWindow::Leading => (f_time - f_window, f_time),
            SampleWindow::Centered => (f_time - f_window / 2.0, f_time + f_window / 2.0),
        };
        let mut p = offset_from_time(w, start);
        let p_end = offset_from_time(w, end);
        let stride = sample_size(w) as usize;
        let mut max = 0u32;
        while p < p_end {
            let amp = sample_amplitude(w, p) as u32;
            if amp > max {
                max = amp;
            }
            p = p.wrapping_add(stride);
        }
        max
    }
}

/// `FWaveModInfo::UpdateWaveData`: applies a shrunk `new_data_size`.
///
/// # Safety
/// `w` must hold valid pointers from a successful `read_wave_info`. `arr` must
/// be the array those pointers were read from.
pub unsafe fn update_wave_data(
    w: *mut FWaveModInfo,
    arr: *mut TArrayRaw,
    svc: &Hp2AudioServices,
) -> bool {
    unsafe {
        let sds = (*w).sample_data_size;
        let nds = (*w).new_data_size;
        if nds >= sds {
            return true;
        }
        let shrink = pad16(sds).wrapping_sub(pad16(nds)) as i32;

        st::<u32>((*w).p_wave_data_size as usize, nds);
        let master = ld::<u32>((*w).p_master_size as usize);
        st::<u32>(
            (*w).p_master_size as usize,
            master.wrapping_sub(shrink as u32),
        );

        let block = (channels(&*w) * (bits(&*w) >> 3)) as u16;
        st::<u16>((*w).p_block_align as usize, block);
        let avg = (block as u32).wrapping_mul(samples_per_sec(&*w));
        st::<u32>((*w).p_avg_bytes_per_sec as usize, avg);

        if (*w).sample_loops_num != 0 {
            let divisor = (sds.wrapping_mul(bits(&*w)) / nds) as i32 as u32;
            let old_bits = (*w).old_bits_per_sample;
            let loops = (*w).p_sample_loop as usize;
            for i in 0..(*w).sample_loops_num.max(0) as usize {
                let loop_addr = loops + i * 24;
                let start = ld::<u32>(loop_addr + 8);
                st::<u32>(loop_addr + 8, start.wrapping_mul(old_bits) / divisor);
                let end = ld::<u32>(loop_addr + 12);
                st::<u32>(loop_addr + 12, end.wrapping_mul(old_bits) / divisor);
            }
        }

        // Shift the trailing bytes after the sample data down by `shrink`.
        let sde = (*w).sample_data_end as usize;
        let wde = (*w).wave_data_end as usize;
        let count = wde.wrapping_sub(sde) as isize;
        if count > 0 {
            copy(
                sde as *const u8,
                (sde.wrapping_sub(shrink as usize)) as *mut u8,
                count as usize,
            );
        }

        (svc.array_u8_remove)(arr.cast(), (*arr).array_num - shrink, shrink);
        true
    }
}

/// `FWaveModInfo::HalveReduce16to8`: halves the sample rate and reduces
/// 16-bit samples to 8-bit with error diffusion.
///
/// # Safety
/// `w` must hold valid pointers from a successful `read_wave_info`.
pub unsafe fn halve_reduce_16_to_8(w: *mut FWaveModInfo) {
    unsafe {
        let sample_bytes = ((*w).sample_data_size >> 1) >> 1;
        let sd = (*w).sample_data_start as usize;
        let mut err: i32 = 0;
        let mut next0 = ld::<i16>(sd) as i32;
        for t in 0..sample_bytes as usize {
            let next1 = ld::<i16>(sd + t * 4) as i32;
            let next2 = ld::<i16>(sd + t * 4 + 2) as i32;
            let filtered = 32768 * 4 + next0 + next1 + next1 + next2;
            next0 = next2;
            let orig = err.wrapping_add(filtered);
            let mut new_value = orig.wrapping_add(512) & (0xfffffc00u32 as i32);
            if new_value > 0x3fc00 {
                new_value = 0x3fc00;
            }
            st::<u8>(sd + t, (new_value >> 10) as u8);
            err = orig - new_value;
        }
        (*w).new_data_size = sample_bytes;
        st::<u16>((*w).p_bits_per_sample as usize, 8);
        let sps = samples_per_sec(&*w);
        st::<u32>((*w).p_samples_per_sec as usize, sps >> 1);
        (*w).noise_gate = 1;
    }
}

/// `FWaveModInfo::Reduce16to8`: 16-bit to 8-bit with error diffusion.
///
/// # Safety
/// As for [`halve_reduce_16_to_8`].
pub unsafe fn reduce_16_to_8(w: *mut FWaveModInfo) {
    unsafe {
        let sample_bytes = (*w).sample_data_size >> 1;
        let sd = (*w).sample_data_start as usize;
        let mut err: i32 = 0;
        for t in 0..sample_bytes as usize {
            let orig = err
                .wrapping_add(32768)
                .wrapping_add(ld::<i16>(sd + t * 2) as i32);
            let mut new_value = orig.wrapping_add(127) & (0xffffff00u32 as i32);
            if new_value > 0xff00 {
                new_value = 0xff00;
            }
            st::<u8>(sd + t, (new_value >> 8) as u8);
            err = orig - new_value;
        }
        (*w).new_data_size = sample_bytes;
        st::<u16>((*w).p_bits_per_sample as usize, 8);
        (*w).noise_gate = 1;
    }
}

/// `FWaveModInfo::HalveData`: halves the sample rate with error diffusion.
/// 16-bit and 8-bit samples are handled. Other depths are left unchanged.
///
/// # Safety
/// As for [`halve_reduce_16_to_8`].
pub unsafe fn halve_data(w: *mut FWaveModInfo) {
    unsafe {
        let sd = (*w).sample_data_start as usize;
        let sps = samples_per_sec(&*w);
        match bits(&*w) {
            16 => {
                let scaled = ((*w).sample_data_size >> 1) >> 1;
                let mut err: i32 = 0;
                let mut next0 = ld::<i16>(sd) as i32;
                for t in 0..scaled as usize {
                    let next1 = ld::<i16>(sd + t * 4) as i32;
                    let next2 = ld::<i16>(sd + t * 4 + 2) as i32;
                    let filtered = 32768 * 4 + next0 + next1 + next1 + next2;
                    next0 = next2;
                    let orig = err.wrapping_add(filtered);
                    let mut new_value = orig.wrapping_add(2) & 0x3fffc;
                    if new_value > 0x3fffc {
                        new_value = 0x3fffc;
                    }
                    st::<i16>(sd + t * 2, ((new_value >> 2) - 32768) as i16);
                    err = orig - new_value;
                }
                (*w).new_data_size = scaled * 2;
                st::<u32>((*w).p_samples_per_sec as usize, sps >> 1);
            }
            8 => {
                let count = (*w).sample_data_size >> 1;
                let mut err: i32 = 0;
                let mut next0 = ld::<u8>(sd) as i32;
                for t in 0..count as usize {
                    let next1 = ld::<u8>(sd + t * 2) as i32;
                    let next2 = ld::<u8>(sd + t * 2 + 1) as i32;
                    let filtered = next0 + next1 + next1 + next2;
                    next0 = next2;
                    let orig = err.wrapping_add(filtered);
                    let mut new_value = orig.wrapping_add(2) & 0x3fc;
                    if new_value > 0x3fc {
                        new_value = 0x3fc;
                    }
                    st::<u8>(sd + t, (new_value >> 2) as u8);
                    err = orig - new_value;
                }
                (*w).new_data_size = count;
                st::<u32>((*w).p_samples_per_sec as usize, sps >> 1);
            }
            _ => {}
        }
    }
}

/// `FWaveModInfo::NoiseGateFilter`: silences low-amplitude runs of at least
/// `MinBlankSize` bytes. Its only caller in the original is commented out, so
/// the port keeps it as a public function without a caller.
///
/// # Safety
/// `w` must hold valid pointers from a successful `read_wave_info`.
pub unsafe fn noise_gate_filter(w: *mut FWaveModInfo) {
    unsafe {
        let sps = samples_per_sec(&*w);
        let sample_data = (*w).sample_data_start as usize;
        let sample_bytes = ld::<u32>((*w).p_wave_data_size as usize) as i32;
        let blocks = (sps / 11025) as i32;
        let min_blank = 860 * blocks;
        let amplitude = 18i32;
        let glitch = 32 * blocks;
        let mut start_silence = 0i32;
        let mut end_silence = 0i32;
        for t in 0..sample_bytes {
            let sample = ld::<u8>(sample_data + t as usize) as i32;
            let loud = (sample - 128).abs() >= amplitude
                && !(start_silence > 0 && t - start_silence < glitch);
            if start_silence == 0 {
                if !loud {
                    start_silence = t;
                }
            } else if (end_silence == 0 && loud) || t == sample_bytes - 1 {
                end_silence = t;
                if end_silence - start_silence >= min_blank {
                    for er in start_silence..end_silence {
                        st::<u8>(sample_data + er as usize, 128);
                    }
                }
                start_silence = 0;
                end_silence = 0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FSoundData
// ---------------------------------------------------------------------------

/// `FSoundData::Load`.
///
/// # Safety
/// `data` must point to a live `FSoundData` whose owner is a live `USound`.
pub unsafe fn sound_data_load(data: *mut SoundDataRaw, svc: &Hp2AudioServices) {
    unsafe {
        let loaded = (svc.lazy_saved_pos)(data.cast()) > 0;
        (svc.lazy_load)(data.cast());
        if !loaded {
            return;
        }
        let owner = (*data).owner;
        if (svc.file_type_is_ps2)(owner.cast()) == 0 {
            force_wave_conversion(data, svc);
        }
        (*owner).original_size = (*lazy_array(addr_of_mut!((*data).lazy))).array_num;
    }
}

/// The disabled body of `FSoundData::Load`. Runs only when `FORCE_8BIT` or
/// `FORCE_HALVE` is set, and both are false.
///
/// # Safety
/// As for [`sound_data_load`].
unsafe fn force_wave_conversion(data: *mut SoundDataRaw, svc: &Hp2AudioServices) {
    unsafe {
        const FREQ_THRESHOLD: u32 = 22050;
        if !(FORCE_8BIT || FORCE_HALVE) {
            return;
        }
        let arr = lazy_array(addr_of_mut!((*data).lazy));
        let mut w = wave_mod_info_new();
        if !read_wave_info(&mut w, arr) || w.sample_data_size <= 4 {
            return;
        }
        let reduce_bits = FORCE_8BIT && bits(&w) == 16;
        let reduce_freq = FORCE_HALVE && samples_per_sec(&w) >= FREQ_THRESHOLD;
        if reduce_bits && reduce_freq {
            halve_reduce_16_to_8(&mut w);
        } else if reduce_bits {
            reduce_16_to_8(&mut w);
        } else if reduce_freq {
            halve_data(&mut w);
        }
        update_wave_data(&mut w, arr, svc);
    }
}

/// `FSoundData::GetPeriod`.
///
/// # Safety
/// As for [`sound_data_load`].
pub unsafe fn sound_data_get_period(data: *mut SoundDataRaw, svc: &Hp2AudioServices) -> f32 {
    unsafe {
        let owner = (*data).owner;
        (svc.lazy_load)(data.cast());
        let arr = lazy_array(addr_of_mut!((*data).lazy));
        if (svc.file_type_is_ps2)(owner.cast()) != 0 {
            return ld::<f32>(arr_base(arr) + 4);
        }
        let mut w = wave_mod_info_new();
        let mut period = 0.0f32;
        if read_wave_info(&mut w, arr) {
            let dur_div = channels(&w)
                .wrapping_mul(bits(&w))
                .wrapping_mul(samples_per_sec(&w)) as i32;
            if dur_div != 0 {
                period = ld::<u32>(w.p_wave_data_size as usize) as f32 * 8.0 / (dur_div as f32);
            }
        }
        period
    }
}

// ---------------------------------------------------------------------------
// FLipSyncData
// ---------------------------------------------------------------------------

/// `FLipSyncData::GetMaxAmplitude`: the cached maximum, computed on first use.
///
/// # Safety
/// `lip` must point to a live `FLipSyncData`.
unsafe fn lip_max_amplitude(lip: *mut LipSyncDataRaw) -> u8 {
    unsafe {
        if (*lip).max_amplitude < 0 {
            let arr = lazy_array(addr_of_mut!((*lip).lazy));
            let base = arr_base(arr);
            let mut max = 0i32;
            for i in (0..(*arr).array_num).rev() {
                let v = ld::<u8>(base + i as usize) as i32;
                if v > max {
                    max = v;
                }
            }
            (*lip).max_amplitude = max;
        }
        (*lip).max_amplitude as u8
    }
}

/// `FLipSyncData::GetInterpAmplitude`: amplitude at time `t`, interpolated
/// between the two nearest samples.
///
/// # Safety
/// `lip` must point to a live `FLipSyncData` with a live owner.
unsafe fn lip_interp_amplitude(lip: *mut LipSyncDataRaw, t: f32) -> u8 {
    unsafe {
        let duration = (*(*lip).owner).duration;
        let t = if t < 0.0 {
            0.0
        } else if t > duration {
            duration
        } else {
            t
        };
        let arr = lazy_array(addr_of_mut!((*lip).lazy));
        let base = arr_base(arr);
        let num = (*arr).array_num;

        if t + LIP_AMP_SAMPLE_INTERVAL > duration {
            let idx = ((t / LIP_AMP_SAMPLE_INTERVAL).floor() as i32)
                .max(0)
                .min(num - 1);
            ld::<u8>(base.wrapping_add_signed(idx as isize))
        } else {
            let i_start = (t / LIP_AMP_SAMPLE_INTERVAL).floor() as i32;
            let i_end = ((t + LIP_AMP_SAMPLE_INTERVAL) / LIP_AMP_SAMPLE_INTERVAL).floor() as i32;
            let w = t % LIP_AMP_SAMPLE_INTERVAL;
            let a = ld::<u8>(base.wrapping_add_signed(i_start as isize)) as f32;
            let b = ld::<u8>(base.wrapping_add_signed(i_end as isize)) as f32;
            // Oracle fuses `Data[start]*(I-w) + Data[end]*w` (UnAudio.cpp:1446-1447; fmadd in
            // GetMouthPos/GetInterpAmplitude); `b*w` is computed first, as in the object.
            let v = a.mul_add(LIP_AMP_SAMPLE_INTERVAL - w, b * w) / LIP_AMP_SAMPLE_INTERVAL;
            v as u8
        }
    }
}

/// `FLipSyncData::GetMouthPos`: normalized mouth opening at time `t`.
///
/// # Safety
/// As for [`lip_interp_amplitude`].
pub unsafe fn lip_mouth_pos(lip: *mut LipSyncDataRaw, t: f32) -> f32 {
    unsafe {
        let amp = lip_interp_amplitude(lip, t);
        let max = lip_max_amplitude(lip);
        (amp as f32 / max as f32) * MOUTH_ANIM_END_RANGE * MOUTH_MUTE_FACTOR
    }
}

// ---------------------------------------------------------------------------
// USound
// ---------------------------------------------------------------------------

/// `USound::Serialize`.
///
/// # Safety
/// `sound` must be a live `USound`. `ar` must be a live `FArchive`.
pub unsafe fn usound_serialize(sound: *mut USound, ar: *mut c_void, svc: &Hp2AudioServices) {
    unsafe {
        (svc.sound_super_serialize)(sound.cast(), ar);
        (svc.archive_serialize_name)(ar, addr_of_mut!((*sound).file_type).cast());
        (svc.archive_serialize_i32)(ar, addr_of_mut!((*sound).core_flags).cast());
        (svc.archive_serialize_f32)(ar, addr_of_mut!((*sound).duration).cast());

        let ver = (svc.archive_ver)(ar);
        if ver > 76 {
            (svc.archive_serialize_i32)(ar, addr_of_mut!((*sound).raw_num_samples).cast());
        }
        if ver > 77 {
            (svc.archive_serialize_i32)(ar, addr_of_mut!((*sound).raw_bits_per_sample).cast());
            (svc.archive_serialize_i32)(ar, addr_of_mut!((*sound).raw_num_channels).cast());
        }
        if ver > 78 {
            (svc.archive_serialize_i32)(ar, addr_of_mut!((*sound).raw_sample_rate).cast());
        }

        if (svc.archive_is_loading)(ar) != 0 || (svc.archive_is_saving)(ar) != 0 {
            (svc.archive_serialize_sound_data)(ar, sound_data_of(sound).cast());
            if (*sound).core_flags & SF_HAS_LIP_SYNC != 0 {
                (svc.archive_serialize_lip_sync)(ar, lip_data_of(sound).cast());
            }
        } else {
            (svc.archive_count_bytes)(ar, (*sound).original_size, (*sound).original_size);
        }
    }
}

/// `USound::ImportPostProcess`.
///
/// # Safety
/// `sound` must be a live `USound`. `filename` and `parms` must be the
/// `FString` data pointers the services expect.
pub unsafe fn usound_import_post_process(
    sound: *mut USound,
    filename: *const c_void,
    parms: *const c_void,
    svc: &Hp2AudioServices,
) {
    unsafe {
        (*sound).duration = sound_data_get_period(sound_data_of(sound), svc);
        let compress = (svc.parse_is)(parms, PARSE_COMPRESS) != 0;
        let lipsync = (svc.parse_is)(parms, PARSE_LIPSYNC) != 0;
        if lipsync {
            usound_calc_lip_sync(sound, svc);
        }
        if compress {
            usound_compress_xa(sound, filename, svc);
        }
    }
}

/// `USound::CalcLipSyncData`.
///
/// # Safety
/// `sound` must be a live `USound` with a loaded `Data`.
pub unsafe fn usound_calc_lip_sync(sound: *mut USound, svc: &Hp2AudioServices) -> bool {
    unsafe {
        let data = sound_data_of(sound);
        sound_data_load(data, svc);
        let arr = lazy_array(addr_of_mut!((*data).lazy));
        let mut w = wave_mod_info_new();
        if !read_wave_info(&mut w, arr) {
            return false;
        }

        let duration = (*sound).duration;
        let n = (duration / LIP_AMP_SAMPLE_INTERVAL) as i32;
        let lip = lip_data_of(sound);
        let lip_arr = lazy_array(addr_of_mut!((*lip).lazy));
        (svc.array_u8_set)(lip_arr.cast(), n + 1);

        let mut f_time = 0.0f32;
        for i in 0..n {
            let amp = sample_window_amplitude(
                &w,
                f_time,
                LIP_AMP_SAMPLE_WINDOW_SIZE,
                SampleWindow::Trailing,
            );
            st::<u8>(arr_base(lip_arr) + i as usize, amp as u8);
            f_time += LIP_AMP_SAMPLE_INTERVAL;
        }
        let last = sample_window_amplitude(
            &w,
            duration,
            LIP_AMP_SAMPLE_WINDOW_SIZE,
            SampleWindow::Leading,
        );
        st::<u8>(arr_base(lip_arr) + n.max(0) as usize, last as u8);

        (*sound).core_flags |= SF_HAS_LIP_SYNC;
        true
    }
}

/// `USound::CompressXA`. The `_WINDOWS` encoder path is removed, so this
/// always returns false. It logs unsupported sample rates.
///
/// # Safety
/// `sound` must be a live `USound`. `source_filename` must be the `FString`
/// data pointer the log service expects.
pub unsafe fn usound_compress_xa(
    sound: *mut USound,
    source_filename: *const c_void,
    svc: &Hp2AudioServices,
) -> bool {
    unsafe {
        let data = sound_data_of(sound);
        sound_data_load(data, svc);
        let arr = lazy_array(addr_of_mut!((*data).lazy));
        let mut w = wave_mod_info_new();
        let ok = read_wave_info(&mut w, arr);

        let too_small = ((*arr).array_num as u32) < SMALLEST_COMPRESSABLE_BYTES;
        if too_small || !ok {
            (svc.lazy_unload)(data.cast());
            return false;
        }

        let bytes = sample_size(&w);
        (*sound).raw_num_samples = (w.sample_data_size / bytes as u32) as i32;
        (*sound).raw_bits_per_sample = bits(&w) as i32;
        (*sound).raw_num_channels = channels(&w) as i32;
        let rate = samples_per_sec(&w) as i32;
        (*sound).raw_sample_rate = rate;
        (svc.lazy_unload)(data.cast());

        if !matches!(rate, 16000 | 22050 | 32000 | 44100) {
            (svc.log_sample_rate_unsupported)(source_filename, rate);
        }
        false
    }
}

/// `USound::BindStream`.
///
/// # Safety
/// `sound` must be a live `USound`.
pub unsafe fn usound_bind_stream(sound: *mut USound, id: i32, svc: &Hp2AudioServices) {
    unsafe {
        if (*sound).core_flags & SF_STREAMING == 0 {
            return;
        }
        let arr = open_streams_of(sound);
        let base = arr_base(arr);
        for i in 0..(*arr).array_num as usize {
            let slot = base + i * 4;
            let v = ld::<i32>(slot);
            if v & 1 == 0 {
                (svc.map_set)(sound.cast(), id, v);
                st::<i32>(slot, v | 1);
                return;
            }
        }
    }
}

/// `USound::UnbindStream`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_unbind_stream(sound: *mut USound, id: i32, svc: &Hp2AudioServices) {
    unsafe {
        let open = (svc.map_find_ref)(sound.cast(), id);
        let arr = open_streams_of(sound);
        let base = arr_base(arr);
        for i in 0..(*arr).array_num as usize {
            let slot = base + i * 4;
            let v = ld::<i32>(slot);
            if (v & !1) == open {
                st::<i32>(slot, v & !1);
                break;
            }
        }
        (svc.map_remove)(sound.cast(), id);
    }
}

/// `USound::UnbindAllStreams`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_unbind_all_streams(sound: *mut USound, svc: &Hp2AudioServices) {
    unsafe { (svc.map_empty)(sound.cast()) }
}

/// `USound::IsRegistered`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_is_registered(sound: *mut USound) -> bool {
    unsafe {
        if (*sound).core_flags & SF_STREAMING != 0 {
            let arr = open_streams_of(sound);
            let base = arr_base(arr);
            (0..(*arr).array_num as usize).any(|i| ld::<i32>(base + i * 4) & 1 == 0)
        } else {
            (*sound).handle != 0
        }
    }
}

/// `USound::SetHandle`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_set_handle(sound: *mut USound, handle: i32) {
    unsafe {
        if (*sound).core_flags & SF_STREAMING == 0 {
            (*sound).handle = handle;
        }
    }
}

/// `USound::AddStreamHandle`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_add_stream_handle(sound: *mut USound, id: i32, svc: &Hp2AudioServices) {
    unsafe {
        let arr = open_streams_of(sound);
        (svc.array_i32_add)(arr.cast());
        let last = (*arr).array_num as usize - 1;
        st::<i32>(arr_base(arr) + last * 4, id.wrapping_mul(2));
    }
}

/// `USound::DeleteStreamHandle`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_delete_stream_handle(sound: *mut USound, id: i32, svc: &Hp2AudioServices) {
    unsafe {
        let comp = id.wrapping_mul(2);
        let arr = open_streams_of(sound);
        let mut i = 0i32;
        while i < (*arr).array_num {
            let v = ld::<i32>(arr_base(arr) + i as usize * 4);
            if comp == (v & !1) {
                if comp != v {
                    (svc.log_deleting_bound_stream)();
                }
                (svc.array_i32_remove)(arr.cast(), i);
                return;
            }
            i += 1;
        }
    }
}

/// `USound::DeleteAllStreamHandles`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_delete_all_stream_handles(sound: *mut USound, svc: &Hp2AudioServices) {
    unsafe { (svc.array_i32_empty)(open_streams_of(sound).cast()) }
}

/// `USound::GetHandle`.
///
/// # Safety
/// As for [`usound_bind_stream`].
pub unsafe fn usound_get_handle(sound: *mut USound, id: i32, svc: &Hp2AudioServices) -> i32 {
    unsafe {
        if (*sound).core_flags & SF_STREAMING != 0 {
            (svc.map_find_ref)(sound.cast(), id) / 2
        } else {
            (*sound).handle
        }
    }
}
