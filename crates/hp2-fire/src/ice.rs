//! Port of the ice algorithm in `UnFractal.cpp`: `UIceTexture::MoveIcePosition`,
//! `BlitTexIce`, `BlitIceTex`, and the `PostLoad` local-source resample loop.
//! `UIceTexture::RenderIce` stays in C++ and calls the three entries in order.
//!
//! The UObject shells stay in C++. The C++ forwarders copy the panning members
//! into `FireIceState`, pass the pixel buffers as slices, and copy the mutable
//! fields back.

use crate::abi::FireTraceHooks;
use crate::globals;
use crate::tables::app_round;

/// `PanningType` values from `UnFractal.h`.
pub const SLIDE_LINEAR: u8 = 0;
pub const SLIDE_CIRCULAR: u8 = 1;
pub const SLIDE_GESTATION: u8 = 2;
pub const SLIDE_WAVY_X: u8 = 3;
pub const SLIDE_WAVY_Y: u8 = 4;

/// Mirrors the `UIceTexture` members the algorithm reads and writes. Field types
/// follow the C++ declarations: `BYTE` is `u8`, `INT` is `i32`, `FLOAT` is `f32`.
/// `move_ice` is the `MoveIce` bitfield, and `local_source_bitmap` is non-zero
/// when `LocalSourceBitmap` is set (the blits are then skipped).
/// `u_bits` is `UBits` (row stride is `1 << u_bits`). `u_size`/`v_size` are
/// `USize`/`VSize`, and `u_mask`/`v_mask` are `UMask`/`VMask`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FireIceState {
    pub panning_style: u8,
    pub horiz_pan_speed: u8,
    pub vert_pan_speed: u8,
    pub frequency: u8,
    pub amplitude: u8,
    pub move_ice: u8,
    pub local_source_bitmap: u8,
    pub u_bits: u8,
    pub force_refresh: i32,
    pub master_count: f32,
    pub u_displace: f32,
    pub v_displace: f32,
    pub u_position: f32,
    pub v_position: f32,
    pub old_u_disp: i32,
    pub old_v_disp: i32,
    pub u_size: i32,
    pub v_size: i32,
    pub u_mask: i32,
    pub v_mask: i32,
}

/// Parameters for the `PostLoad` local-source resample loop.
/// `u_scaler`/`v_scaler` are `UBits - SourceTexture->UBits` and the V
/// equivalent, and `src_u_bits` is `SourceTexture->UBits`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FireIceResample {
    pub u_size: i32,
    pub v_size: i32,
    pub u_scaler: i32,
    pub v_scaler: i32,
    pub src_u_bits: u8,
}

/// A read-only texel buffer for one blit input. `Dest` means the input is the
/// destination bitmap itself (`SourceTexture == this` or `GlassTexture == this`).
/// Reads then observe bytes already written by the same loop, as the C++
/// pointer aliasing did.
#[derive(Clone, Copy)]
pub enum IceRead<'a> {
    Separate(&'a [u8]),
    Dest,
}

#[inline(always)]
fn read(src: IceRead<'_>, dst: &[u8], index: usize) -> u8 {
    match src {
        IceRead::Separate(buf) => buf[index],
        IceRead::Dest => dst[index],
    }
}

/// `v << UBits`: byte offset of row `v` in a bitmap whose rows are `1 << u_bits` wide.
fn row_base(v: i32, u_bits: u8) -> usize {
    v.wrapping_shl(u_bits as u32) as usize
}

/// `appRound((FLOAT)scale * appSin(arg))`. `appSin` is `DOUBLE` in C, so the
/// product is formed in double and rounded to FLOAT when passed to `appRound`.
fn round_sin(scale: f32, arg: f32) -> i32 {
    app_round((scale as f64 * (arg as f64).sin()) as f32)
}

/// `appRound((FLOAT)scale * appCos(arg))`, same rounding as `round_sin`.
fn round_cos(scale: f32, arg: f32) -> i32 {
    app_round((scale as f64 * (arg as f64).cos()) as f32)
}

/// `UIceTexture::MoveIcePosition` algorithm.
fn move_ice_position_body(st: &mut FireIceState, delta_time: f32) {
    st.master_count += delta_time * 120.0;

    // `(SBYTE)(Speed - 128)`. The offset is in -128..=127, so the i8 cast is exact.
    let horiz = (st.horiz_pan_speed as i32 - 128) as i8 as i32;
    let vert = (st.vert_pan_speed as i32 - 128) as i8 as i32;
    st.u_displace += 2.0 * delta_time * -(horiz as f32);
    st.v_displace += 2.0 * delta_time * (vert as f32);

    let mc = st.master_count;
    // `(Frequency+1)` is an int that becomes FLOAT in the product.
    let freq = (st.frequency as i32 + 1) as f32;
    let amp = (st.amplitude as i32 + 1) as f32;
    // `MasterCount * (Frequency+1) * rate`, evaluated in FLOAT.
    let wave = |rate: f32| mc * freq * rate;

    match st.panning_style {
        SLIDE_LINEAR => {
            st.u_position = st.u_displace;
            st.v_position = st.v_displace;
        }
        SLIDE_CIRCULAR => {
            st.u_position = st.u_displace + round_sin(amp, wave(0.0012)) as f32;
            st.v_position = st.v_displace + round_cos(amp, wave(0.0012)) as f32;
        }
        SLIDE_GESTATION => {
            st.u_position = st.u_displace + round_sin(amp, wave(0.0012)) as f32;
            st.v_position = st.v_displace + round_cos(amp, wave(0.0011)) as f32;
        }
        SLIDE_WAVY_X => {
            // `0.5f*(FLOAT)(Amplitude+1)` is a FLOAT product.
            st.u_position = st.u_displace + round_sin(0.5 * amp, wave(0.0012)) as f32;
            st.v_position = st.v_displace;
        }
        SLIDE_WAVY_Y => {
            st.u_position = st.u_displace;
            st.v_position = st.v_displace + round_cos(0.5 * amp, wave(0.0012)) as f32;
        }
        _ => {}
    }
}

/// `UIceTexture::BlitTexIce` algorithm, C path.
fn blit_tex_ice_body(st: &FireIceState, dst: &mut [u8], tex: IceRead<'_>, glass: IceRead<'_>) {
    if st.local_source_bitmap != 0 {
        return;
    }
    let u_bits = st.u_bits;
    let u_mask = st.u_mask;
    let v_mask = st.v_mask;
    let u_disp = app_round(st.u_position) & u_mask;
    let v_disp = app_round(st.v_position) & v_mask;

    for v in 0..st.v_size {
        let line = row_base(v, u_bits);
        let glass_start = row_base((v + v_disp) & v_mask, u_bits);
        for u in (0..st.u_size).step_by(2) {
            let u2 = u + 1;

            let g = read(glass, dst, glass_start + ((u + u_disp) & u_mask) as usize);
            let t = read(tex, dst, line + ((u + g as i32) & u_mask) as usize);
            dst[line + u as usize] = t;

            let g2 = read(glass, dst, glass_start + ((u2 + u_disp) & u_mask) as usize);
            let t2 = read(tex, dst, line + ((u2 + g2 as i32) & u_mask) as usize);
            dst[line + u2 as usize] = t2;
        }
    }
}

/// `UIceTexture::BlitIceTex` algorithm, C path.
fn blit_ice_tex_body(st: &FireIceState, dst: &mut [u8], tex: IceRead<'_>, glass: IceRead<'_>) {
    if st.local_source_bitmap != 0 {
        return;
    }
    let u_bits = st.u_bits;
    let u_mask = st.u_mask;
    let v_mask = st.v_mask;
    let u_disp = app_round(st.u_position) & u_mask;
    let v_disp = app_round(st.v_position) & v_mask;

    for v in 0..st.v_size {
        let line = row_base(v, u_bits);
        let tex_start = row_base((v + v_disp) & v_mask, u_bits);
        let glass_start = line;
        for u in (0..st.u_size).step_by(2) {
            let u2 = u + 1;

            let g = read(glass, dst, glass_start + u as usize);
            let t = read(
                tex,
                dst,
                tex_start + ((u + u_disp + g as i32) & u_mask) as usize,
            );
            dst[line + u as usize] = t;

            let g2 = read(glass, dst, glass_start + u2 as usize);
            let t2 = read(
                tex,
                dst,
                tex_start + ((u2 + u_disp + g2 as i32) & u_mask) as usize,
            );
            dst[line + u2 as usize] = t2;
        }
    }
}

/// `UIceTexture::PostLoad` resample of `SourceTexture` into `LocalSourceBitmap`
/// (`UnFractal.cpp`, the `WrongSizedSource || WrongSizedGlass` branch). The
/// caller has already checked that both scalers are non-negative.
fn resample_source_body(p: &FireIceResample, dst: &mut [u8], src: IceRead<'_>) {
    for v in 0..p.v_size {
        for u in 0..p.u_size {
            let src_index = (u.wrapping_shr(p.u_scaler as u32)
                + v.wrapping_shr(p.v_scaler as u32)
                    .wrapping_shl(p.src_u_bits as u32)) as usize;
            let value = read(src, dst, src_index);
            dst[(u + v * p.u_size) as usize] = value;
        }
    }
}

/// Replaces `UIceTexture::MoveIcePosition`'s algorithm. Advances `MasterCount`,
/// the displacements, and the positions by `delta_time`.
pub fn move_ice_position(trace: FireTraceHooks, st: &mut FireIceState, delta_time: f32) {
    let mut g = globals::lock();
    g.trace = trace;
    move_ice_position_body(st, delta_time);
}

/// Replaces `UIceTexture::BlitTexIce`. Writes `bitmap` from `tex` and `glass`
/// with the glass displacing `tex`.
pub fn blit_tex_ice(
    trace: FireTraceHooks,
    st: &FireIceState,
    dst: &mut [u8],
    tex: IceRead<'_>,
    glass: IceRead<'_>,
) {
    let mut g = globals::lock();
    g.trace = trace;
    blit_tex_ice_body(st, dst, tex, glass);
}

/// Replaces `UIceTexture::BlitIceTex`. Writes `bitmap` from `tex` with the glass
/// displacing the sample position.
pub fn blit_ice_tex(
    trace: FireTraceHooks,
    st: &FireIceState,
    dst: &mut [u8],
    tex: IceRead<'_>,
    glass: IceRead<'_>,
) {
    let mut g = globals::lock();
    g.trace = trace;
    blit_ice_tex_body(st, dst, tex, glass);
}

/// Replaces the `PostLoad` resample loop that fills `LocalSourceBitmap`.
pub fn resample_source(
    trace: FireTraceHooks,
    p: &FireIceResample,
    dst: &mut [u8],
    src: IceRead<'_>,
) {
    let mut g = globals::lock();
    g.trace = trace;
    resample_source_body(p, dst, src);
}
