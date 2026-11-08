//! Port of the wet texture algorithms in `UnFractal.cpp`:
//! `UWetTexture::ApplyWetTexture`, the source resample loop inside
//! `UWetTexture::PostLoad`, and `UWetTexture::SetRefractionTable`.
//! C++ keeps the UObject shells, source-texture locking, the host-owned
//! `LocalSourceBitmap` allocation (`Find/Allocate/FreeWetLocalSourceBitmap`),
//! and all branch decisions that touch member state.
//!
//! Buffer lengths come from the C++ side (`DataArray.Num()`). Each entry checks
//! the lengths it needs up front and returns without writing if a buffer is too
//! short. The C loops would read or write past the end in that case.

use std::ffi::c_void;

use crate::abi::{FireArena, FireTraceHooks};
use hp2_abi::engine::generated::{UTexture, UWaterTexture, UWetTexture};

/// Dimensions of the wet texture's own bitmap: `USize`, `VSize`, `UBits`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireWetLayout {
    pub u_size: i32,
    pub v_size: i32,
    pub u_bits: i32,
}

/// Reads `USize`/`VSize`/`UBits` from a live `UWetTexture`'s own `UTexture` base.
/// Replaces the C++ `UWetTexture::ApplyWetTexture` field reads.
///
/// # Safety
/// `texture` must be non-null and point to a live `UWetTexture`.
pub unsafe fn wet_layout_from_texture(texture: *const c_void) -> FireWetLayout {
    unsafe {
        let t = texture as *const UTexture;
        FireWetLayout {
            u_size: (*t).base.u_size,
            v_size: (*t).base.v_size,
            u_bits: (*t).base.u_bits as i32,
        }
    }
}

/// Reads `WaveAmp` from a live `UWetTexture` (inherited from `UWaterTexture`).
/// Replaces the C++ `UWetTexture::SetRefractionTable` field read.
///
/// # Safety
/// `texture` must be non-null and point to a live `UWetTexture`.
pub unsafe fn wave_amp_from_texture(texture: *const c_void) -> u8 {
    unsafe { (*(texture as *const UWaterTexture)).wave_amp }
}

/// Inputs to the PostLoad resample of a source texture into `LocalSourceBitmap`.
/// `u_scaler`/`v_scaler` are `UBits - SourceUBits` and `VBits - SourceVBits`
/// (both non-negative); `source_u_bits` is the source texture's `UBits`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireWetResample {
    pub u_size: i32,
    pub v_size: i32,
    pub u_scaler: i32,
    pub v_scaler: i32,
    pub source_u_bits: i32,
}

/// Builds the resample params from a live `UWetTexture`'s own dimensions and
/// `SourceTexture->UBits`. `u_scaler`/`v_scaler` come from the caller (not
/// texture fields). Replaces the C++ `FractalLeaf_WetResample` field reads.
///
/// # Safety
/// `texture` must be non-null and point to a live `UWetTexture` whose
/// `SourceTexture` is non-null and points to a live `UTexture`.
pub unsafe fn wet_resample_params_from_texture(
    texture: *const c_void,
    u_scaler: i32,
    v_scaler: i32,
) -> FireWetResample {
    unsafe {
        let t = texture as *const UTexture;
        let w = texture as *const UWetTexture;
        let src = (*w).source_texture as *const UTexture;
        FireWetResample {
            u_size: (*t).base.u_size,
            v_size: (*t).base.v_size,
            u_scaler,
            v_scaler,
            source_u_bits: (*src).base.u_bits as i32,
        }
    }
}

/// Displacement pass of `UWetTexture::ApplyWetTexture`, the C path
/// (`WETASM` branch deleted).
///
/// Applies wet displacement directly to native storage. Source and destination
/// may overlap; every read observes preceding writes in native loop order.
///
/// # Safety
/// Arena pointers must remain valid for their declared lengths.
pub unsafe fn apply_wet_texture(
    trace: FireTraceHooks,
    bitmap: FireArena<u8>,
    source: FireArena<u8>,
    layout: &FireWetLayout,
) {
    let _ = trace;

    if layout.v_size <= 0 || layout.u_size <= 0 {
        return;
    }
    // Largest index touched: last row base plus the last column (u_size - 1).
    let need = (((layout.v_size - 1) as usize) << (layout.u_bits as u32)) + layout.u_size as usize;
    assert!(
        bitmap.len >= need,
        "apply_wet_texture: bitmap shorter than {need}"
    );
    assert!(
        source.len >= need,
        "apply_wet_texture: source shorter than {need}"
    );

    let mask = layout.u_size - 1; // UMask = USize - 1
    for v in 0..layout.v_size {
        // C: BYTE* LineStart = BitMapAddr + (v << UBits); SourceStart likewise.
        let row = (v.wrapping_shl(layout.u_bits as u32)) as usize;
        let mut u = 0i32;
        while u < layout.u_size {
            // C: LineStart[u] = SourceStart[(u + LineStart[u]) & UMask];
            let i0 = row + u as usize;
            assert!(i0 < bitmap.len);
            let s0 = row + ((u + unsafe { bitmap.data.add(i0).read() } as i32) & mask) as usize;
            assert!(s0 < source.len);
            unsafe {
                bitmap.data.add(i0).write(source.data.add(s0).read());
            }

            // C: LineStart[u2] = SourceStart[(u2 + LineStart[u2]) & UMask]; u2 = u+1.
            // Read after the store above, as in C.
            let u2 = u + 1;
            let i1 = row + u2 as usize;
            assert!(i1 < bitmap.len);
            let s1 = row + ((u2 + unsafe { bitmap.data.add(i1).read() } as i32) & mask) as usize;
            assert!(s1 < source.len);
            unsafe {
                bitmap.data.add(i1).write(source.data.add(s1).read());
            }

            u += 2;
        }
    }
}

/// PostLoad resample loop: copies `src` into `dst` (`USize*VSize` bytes) with
/// `LocalBitmap[U + V*USize] = SourceMapAddr[(U >> UScaler) + ((V >> VScaler) << SourceUBits)]`.
/// The size checks, allocation and the give-up branch stay in C++.
///
/// # Safety
/// Arenas must remain valid. They may overlap; reads and writes are sequential.
pub unsafe fn resample_wet_source(
    trace: FireTraceHooks,
    dst: FireArena<u8>,
    src: FireArena<u8>,
    p: &FireWetResample,
) {
    let _ = trace;

    if p.u_size <= 0 || p.v_size <= 0 {
        return;
    }
    let dst_need = (p.u_size as usize) * (p.v_size as usize);
    // Largest source index: last shifted row plus last shifted column.
    let last_row = ((p.v_size - 1).wrapping_shr(p.v_scaler as u32)) as usize;
    let last_col = ((p.u_size - 1).wrapping_shr(p.u_scaler as u32)) as usize;
    let src_need = (last_row << (p.source_u_bits as u32)) + last_col + 1;
    assert!(
        dst.len >= dst_need,
        "resample_wet_source: dst shorter than {dst_need}"
    );
    assert!(
        src.len >= src_need,
        "resample_wet_source: src shorter than {src_need}"
    );

    for v in 0..p.v_size {
        for u in 0..p.u_size {
            let s = u.wrapping_shr(p.u_scaler as u32)
                + (v.wrapping_shr(p.v_scaler as u32)).wrapping_shl(p.source_u_bits as u32);
            unsafe {
                dst.data
                    .add((u + v * p.u_size) as usize)
                    .write(src.data.add(s as usize).read());
            }
        }
    }
}

/// `UWetTexture::SetRefractionTable`: linear refraction table, `RenderTable[0..1024]`.
/// `render_table` must hold at least 1024 entries (`RenderTable` is 1028).
#[allow(clippy::needless_range_loop)] // index loop mirrors the C `for (i...)`
pub fn set_refraction_table(trace: FireTraceHooks, wave_amp: u8, render_table: &mut [u8]) {
    let _ = trace;
    assert!(
        render_table.len() >= 1024,
        "set_refraction_table: need 1024 entries"
    );

    // 'zero' is 512
    let scale: f32 = (wave_amp as f32) / 512.0f32;
    for i in 0..1024usize {
        // C: (INT) ((+i-511) * ((FLOAT)WaveAmp/512.0F))
        let temp_light = (((i as i32) - 511) as f32 * scale) as i32;
        // C: Clamp(TempLight, -128, 127) stored into BYTE (modular).
        render_table[i] = temp_light.clamp(-128, 127) as u8;
    }
}
