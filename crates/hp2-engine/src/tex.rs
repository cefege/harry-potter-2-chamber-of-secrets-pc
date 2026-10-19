//! Port of the non-serialization bodies of `Engine/Src/UnTex.cpp`: palette
//! and colour helpers (`FGetHSV`, `UPalette::BestMatch`, `FixPalette`),
//! mip-zero and colour-range reduction, the 8-bit palettized mip downsample
//! (`UTexture::CreateMips`), the animation timing (`Tick`, `Prime`) and the
//! RGBA7 max-colour cache (`FTextureInfo::CacheMaxColor`).
//!
//! C++ keeps the UObject plumbing: `Lock`/`Unlock`/`Update`/`DefaultLOD`
//! (render-device and client wiring), `ConstantTimeTick` (virtual pointer
//! walk over `AnimNext`), `ReplaceWithExisting` (`TObjectIterator`), the
//! `Compress`/`Decompress` stubs, the mip allocation loop, serialization,
//! and the BMP/snapshot loaders.
//!
//! Arithmetic mirrors the C++ exactly:
//! - `FVector`/`FPlane` `operator/` multiplies by `1.f/Scale` (not a true
//!   division), and `FPlane::operator/` scales `W` by `Scale`, not `1/Scale`.
//! - `FColor(const FPlane&)` clamps `floor(v*256)` to `0..=255`.
//! - `appSqrt` is `DOUBLE`, so `FGetHSV`'s brightness step runs in `f64`.
//! - `FColor` memory order is `R,G,B,A` (`__INTEL_BYTE_ORDER__` is set on
//!   every supported target; see `Core/Inc/UnGnuG.h`).

use hp2_abi::FColor;

/// `TEXF_P8`.
pub const TEXF_P8: u8 = 0x00;
/// `TEXF_RGBA7`.
pub const TEXF_RGBA7: u8 = 0x01;
/// `NUM_PAL_COLORS`.
pub const NUM_PAL_COLORS: usize = 256;

/// One mip level as the C++ shell sees it: `DataArray` plus its dimensions.
pub struct MipLevel<'a> {
    pub data: &'a mut [u8],
    pub size_u: i32,
    pub size_v: i32,
}

/// `FColor` from its packed `Int4` value (`R` in the low byte).
pub fn color_from_int4(v: u32) -> FColor {
    FColor {
        r: v as u8,
        g: (v >> 8) as u8,
        b: (v >> 16) as u8,
        a: (v >> 24) as u8,
    }
}

/// `FColor::Int4` packing (`R` in the low byte).
pub fn color_to_int4(c: FColor) -> u32 {
    u32::from(c.r) | (u32::from(c.g) << 8) | (u32::from(c.b) << 16) | (u32::from(c.a) << 24)
}

type Plane = [f32; 4];

/// `FColor::Plane()` / `FPlane(R/255.f, G/255.f, B/255.f, A/255.f)`.
fn plane_of(c: FColor) -> Plane {
    [
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
        f32::from(c.a) / 255.0,
    ]
}

/// `FPlane::operator/(FLOAT)`: `X,Y,Z` times `1/Scale`, `W` times `Scale`.
fn plane_div(p: Plane, scale: f32) -> Plane {
    let r = 1.0f32 / scale;
    [p[0] * r, p[1] * r, p[2] * r, p[3] * scale]
}

fn plane_add(a: Plane, b: Plane) -> Plane {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

/// `FColor(const FPlane&)`: `Clamp(appFloor(v*256), 0, 255)` per channel.
fn color_of_plane(p: Plane) -> FColor {
    let clamp8 = |v: f32| ((v * 256.0).floor() as i32).clamp(0, 255) as u8;
    FColor {
        r: clamp8(p[0]),
        g: clamp8(p[1]),
        b: clamp8(p[2]),
        a: clamp8(p[3]),
    }
}

/// `FGetHSV(H, S, V)`: byte hue/saturation/brightness to an `FPlane` (W = 0).
pub fn fget_hsv(h: u8, s: u8, v: u8) -> Plane {
    let mut brightness: f32 = f32::from(v) * 1.4f32 / 255.0f32;
    // `appSqrt` returns DOUBLE: the whole step is evaluated in f64.
    let scale = 0.7f32 as f64 / (0.01f32 as f64 + (brightness as f64).sqrt());
    brightness = ((brightness as f64) * scale) as f32;
    // NaN and -0.0 pass through unchanged, as in the original comparisons.
    brightness = brightness.clamp(0.0, 1.0);

    let hf = i32::from(h);
    let (hx, hy, hz): (f32, f32, f32) = if hf < 86 {
        ((85 - hf) as f32 / 85.0, hf as f32 / 85.0, 0.0)
    } else if hf < 171 {
        (0.0, (170 - hf) as f32 / 85.0, (hf - 85) as f32 / 85.0)
    } else {
        ((hf - 170) as f32 / 85.0, 0.0, (255 - hf) as f32 / 84.0)
    };
    let sf = f32::from(s) / 255.0;
    [
        (hx + sf * (1.0 - hx)) * brightness,
        (hy + sf * (1.0 - hy)) * brightness,
        (hz + sf * (1.0 - hz)) * brightness,
        0.0,
    ]
}

/// `UPalette::BestMatch(Color, First)`: closest palette entry in
/// `First..256`, weighted green 8, red 4, blue 1.
pub fn best_match(palette: &[FColor], color: FColor, first: usize) -> u8 {
    let mut best_delta = i32::MAX;
    let mut best_unscaled = i32::MAX;
    let mut best_color = first;
    let tex_r = i32::from(color.r);
    let tex_g = i32::from(color.g);
    let tex_b = i32::from(color.b);
    for (test, c) in palette.iter().enumerate().take(NUM_PAL_COLORS).skip(first) {
        let dg = i32::from(c.g) - tex_g;
        let green_delta = dg * dg;
        // Same as comparing 8*GreenDelta with BestDelta.
        if green_delta < best_unscaled {
            let dr = i32::from(c.r) - tex_r;
            let db = i32::from(c.b) - tex_b;
            let delta = 8 * green_delta + 4 * dr * dr + db * db;
            if delta < best_delta {
                best_color = test;
                best_delta = delta;
                best_unscaled = (delta + 7) >> 3;
            }
        }
    }
    best_color as u8
}

/// `UPalette::FixPalette()`: reorders the imported palette into the shaded
/// layout and sets each alpha to `i + 0x10` (byte-truncated); entry 0 is
/// alpha 0. `colors` must hold `NUM_PAL_COLORS` entries.
pub fn fix_palette(colors: &mut [FColor]) {
    let mut temp = [colors[0]; NUM_PAL_COLORS];
    for i_color in 0..8usize {
        let i_start = if i_color == 0 { 1 } else { 32 * i_color };
        for i_shade in 0..28usize {
            temp[16 + i_color + (i_shade << 3)] = colors[i_start + i_shade];
        }
    }
    for (i, slot) in colors.iter_mut().enumerate().take(NUM_PAL_COLORS) {
        *slot = temp[i];
        slot.a = (i as u32 + 0x10) as u8;
    }
    colors[0].a = 0;
}

/// `UTexture::CreateMips` mip-zero: average of the 8-bit colours of mip 0
/// (`data`, palette `palette`). Non-palettized textures and empty mips give
/// the zero colour, which is what the C++ arithmetic yields for them.
pub fn mip_zero(palette: &[FColor], data: &[u8], p8: bool) -> FColor {
    if !p8 || data.is_empty() {
        return FColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
    }
    // Original quirk: FPlane's += and / come from FVector and touch only X/Y/Z,
    // so the W (alpha) channel of the average is always 0.
    let mut c: Plane = [0.0; 4];
    for &i in data {
        let p = plane_of(palette[usize::from(i)]);
        c = plane_add(c, [p[0], p[1], p[2], 0.0]);
    }
    let avg = plane_div(c, data.len() as f32);
    color_of_plane([avg[0], avg[1], avg[2], 0.0])
}

/// `UTexture::CreateColorRange`: per-channel maximum over every mip texel's
/// palette colour. With no palette the range is opaque white.
pub fn color_range(palette: Option<&[FColor]>, mips: &[&[u8]]) -> FColor {
    let Some(palette) = palette else {
        return FColor {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
    };
    let mut max = FColor {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    for mip in mips {
        for &i in mip.iter() {
            let c = palette[usize::from(i)];
            max.r = max.r.max(c.r);
            max.g = max.g.max(c.g);
            max.b = max.b.max(c.b);
            max.a = max.a.max(c.a);
        }
    }
    max
}

/// `UTexture::CreateMips` downsample for 8-bit palettized textures. `mips[0]`
/// is the populated base level; each following level is built from the next
/// larger one with a 2x2 box filter. Unmasked levels average four texels
/// (truecolor), masked levels average the non-zero texels and keep a texel
/// only when at least two are set. Levels beyond level 1 use the previous
/// level's truecolor output as their source, exactly as the C++ does.
pub fn downsample_p8(palette: &[FColor], masked: bool, mips: &mut [MipLevel<'_>]) {
    let mut true_dest: Option<Vec<FColor>> = None;
    for lvl in 1..mips.len() {
        let (head, tail) = mips.split_at_mut(lvl);
        let src = &head[lvl - 1];
        let dst = &mut tail[0];
        let true_source = true_dest.take();
        let tu = src.size_u;
        let tv = src.size_v;
        let mut td = vec![
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            };
            (src.size_u * src.size_v) as usize
        ];
        let mask_u = (tu - 1) as u32;
        let mask_v = (tv - 1) as u32;
        let fetch = |idx: usize| -> Plane {
            match &true_source {
                Some(ts) => plane_of(ts[idx]),
                None => plane_of(palette[usize::from(src.data[idx])]),
            }
        };

        if !masked {
            let ud = (1 & mask_u) as i32;
            let vd = (1 & mask_v) as i32 * tu;
            for v in 0..dst.size_v {
                for u in 0..dst.size_u {
                    let tc = u * 2 + v * 2 * tu;
                    let mut c = [0.0f32; 3];
                    for off in [0, ud, vd, ud + vd] {
                        let p = fetch((tc + off) as usize);
                        // `FVector += FPlane`: W is not accumulated.
                        c[0] += p[0];
                        c[1] += p[1];
                        c[2] += p[2];
                    }
                    // `FColor(C/4.f)`: FVector operator/ then FPlane(FVector), W = 0.
                    let mip = color_of_plane([c[0] * 0.25, c[1] * 0.25, c[2] * 0.25, 0.0]);
                    let di = (v * dst.size_u + u) as usize;
                    td[di] = mip;
                    dst.data[di] = best_match(palette, mip, 0);
                }
            }
        } else {
            for v in 0..dst.size_v {
                for u in 0..dst.size_u {
                    let mut f = 0i32;
                    let mut c: Plane = [0.0; 4];
                    let tc = v * 2 * tu + u * 2;
                    for i in 0..2u32 {
                        for j in 0..2u32 {
                            let idx = tc + (i & mask_u) as i32 + (j & mask_v) as i32 * tu;
                            let b = src.data[idx as usize];
                            if b != 0 {
                                f += 1;
                                c = plane_add(c, fetch(idx as usize));
                            }
                        }
                    }
                    let di = (v * dst.size_u + u) as usize;
                    if f >= 2 {
                        let mip = color_of_plane(plane_div(c, f as f32));
                        td[di] = mip;
                        dst.data[di] = best_match(palette, mip, 1);
                    } else {
                        td[di] = FColor {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        };
                        dst.data[di] = 0;
                    }
                }
            }
        }
        true_dest = Some(td);
    }
}

/// `UTexture::Tick` timing: returns how many `ConstantTimeTick` calls the
/// frame makes and updates `accumulator`. A zero `max_frame_rate` ticks once
/// per call and leaves the accumulator alone.
///
/// Divergence: the C++ runs its tick before subtracting `MaxTime` from the
/// accumulator; the port updates the accumulator first. Only an override of
/// `ConstantTimeTick` that reads `Accumulator` can observe the difference.
pub fn tick_plan(
    max_frame_rate: f32,
    min_frame_rate: f32,
    accumulator: &mut f32,
    delta: f32,
) -> u32 {
    if max_frame_rate == 0.0 {
        return 1;
    }
    let clamp = |x: f32, lo: f32, hi: f32| {
        if x < lo {
            lo
        } else if x > hi {
            hi
        } else {
            x
        }
    };
    let min_time = 1.0f32 / clamp(max_frame_rate, 0.1, 100.0);
    let max_time = 1.0f32 / clamp(min_frame_rate, 0.1, 100.0);
    *accumulator += delta;
    if *accumulator < min_time {
        0
    } else if *accumulator < max_time {
        *accumulator = 0.0;
        1
    } else {
        *accumulator -= max_time;
        if *accumulator > max_time {
            *accumulator = max_time;
        }
        1
    }
}

/// `UTexture::Prime` loop count: the number of `ConstantTimeTick` calls
/// needed to bring `prime_current` up to `prime_count`.
pub fn prime_steps(prime_current: u8, prime_count: u8) -> u32 {
    u32::from(prime_count.saturating_sub(prime_current))
}

/// `FTextureInfo::CacheMaxColor`: for RGBA7 textures whose cached max colour
/// is still the `0xffffffff` sentinel, scans the clamped `uclamp x vclamp`
/// region (rows `usize` texels apart) for the per-byte maximum via the SWAR
/// trick, then stores `FColor(B*2, G*2, R*2, 255)`. Other inputs return
/// `max` unchanged.
pub fn cache_max_color(
    format: u8,
    max: FColor,
    data: &[u8],
    usize_: i32,
    uclamp: i32,
    vclamp: i32,
) -> FColor {
    if format != TEXF_RGBA7 || color_to_int4(max) != 0xffff_ffff {
        return max;
    }
    let word = |i: usize| {
        u32::from_le_bytes([
            data[4 * i],
            data[4 * i + 1],
            data[4 * i + 2],
            data[4 * i + 3],
        ])
    };
    let mut m: u32 = 0x0101_0101;
    let mut idx = 0usize;
    for _ in 0..vclamp {
        for _ in 0..uclamp {
            let t = word(idx);
            let flow = m.wrapping_sub(t) & 0x8080_8080;
            if flow != 0 {
                let mask = flow.wrapping_sub(flow >> 7);
                m = (t & mask) | (m & 0x7f7f_7f7fu32.wrapping_sub(mask));
            }
            idx += 1;
        }
        idx += (usize_ - uclamp) as usize;
    }
    let c = color_from_int4(m);
    FColor {
        r: c.b.wrapping_mul(2),
        g: c.g.wrapping_mul(2),
        b: c.r.wrapping_mul(2),
        a: 255,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_int4_roundtrip() {
        let c = FColor {
            r: 1,
            g: 2,
            b: 3,
            a: 4,
        };
        assert_eq!(color_to_int4(c), 0x0403_0201);
        assert_eq!(color_from_int4(0x0403_0201), c);
    }

    #[test]
    fn hsv_zero_brightness_is_black() {
        assert_eq!(fget_hsv(0, 0, 0), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn tick_plan_zero_rate_ticks_each_call() {
        let mut acc = 3.0;
        assert_eq!(tick_plan(0.0, 0.0, &mut acc, 0.5), 1);
        assert_eq!(acc, 3.0);
    }

    #[test]
    fn prime_steps_counts_remaining() {
        assert_eq!(prime_steps(2, 5), 3);
        assert_eq!(prime_steps(5, 2), 0);
    }

    #[test]
    fn tick_plan_branches() {
        // max 10 -> min_time 0.1; min 2 -> max_time 0.5.
        let mut acc = 0.0f32;
        assert_eq!(tick_plan(10.0, 2.0, &mut acc, 0.05), 0); // skip, acc 0.05
        assert_eq!(tick_plan(10.0, 2.0, &mut acc, 0.1), 1); // normal, acc reset
        assert_eq!(acc, 0.0);
        let mut acc = 0.0f32;
        assert_eq!(tick_plan(10.0, 2.0, &mut acc, 0.7), 1); // catch-up, acc 0.2 -> clamp 0.2
        assert!((acc - 0.2).abs() < 1e-6);
    }

    #[test]
    fn best_match_tie_prefers_first_seen() {
        let mut pal = [FColor {
            r: 9,
            g: 9,
            b: 9,
            a: 0,
        }; NUM_PAL_COLORS];
        pal[1] = FColor {
            r: 5,
            g: 5,
            b: 5,
            a: 0,
        };
        pal[2] = FColor {
            r: 5,
            g: 5,
            b: 5,
            a: 0,
        };
        let c = FColor {
            r: 5,
            g: 5,
            b: 5,
            a: 0,
        };
        assert_eq!(best_match(&pal, c, 1), 1);
    }

    #[test]
    fn fix_palette_sets_alpha_ramp() {
        let mut pal = [FColor {
            r: 1,
            g: 2,
            b: 3,
            a: 0,
        }; NUM_PAL_COLORS];
        fix_palette(&mut pal);
        assert_eq!(pal[0].a, 0);
        assert_eq!(pal[1].a, 0x11);
        assert_eq!(pal[255].a, (255u32 + 0x10) as u8);
    }

    #[test]
    fn mip_zero_empty_or_non_p8_is_zero() {
        let pal = [FColor {
            r: 7,
            g: 7,
            b: 7,
            a: 7,
        }; NUM_PAL_COLORS];
        assert_eq!(color_to_int4(mip_zero(&pal, &[], true)), 0);
        assert_eq!(color_to_int4(mip_zero(&pal, &[1, 2], false)), 0);
    }

    #[test]
    fn cache_max_color_passthrough() {
        let m = FColor {
            r: 1,
            g: 2,
            b: 3,
            a: 4,
        };
        // Non-RGBA7 and non-sentinel max return the input and never read data.
        assert_eq!(
            color_to_int4(cache_max_color(TEXF_P8, m, &[], 4, 1, 1)),
            color_to_int4(m)
        );
        assert_eq!(
            color_to_int4(cache_max_color(TEXF_RGBA7, m, &[], 4, 1, 1)),
            color_to_int4(m)
        );
    }
}
