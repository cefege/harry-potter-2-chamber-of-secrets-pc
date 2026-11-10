//! CPU mip-chain generation for uncompressed textures.
//!
//! The ported upload path sent only mip 0 for RGBA8 textures (DXT1 textures carry their own
//! chain), so distant surfaces were minified from full resolution and shimmered. The chain is a
//! 2x2 box filter: computed in linear light for sRGB textures, colour weighted by alpha so cut-out
//! edges do not darken, and for textures with transparency the alpha of every level is rescaled
//! so the fraction of texels that pass the 50% mask test matches mip 0 (masked foliage and fences
//! otherwise thin out and vanish with distance).

/// Number of levels in a full chain down to 1x1.
pub fn level_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

fn srgb_to_linear(v: u8) -> f32 {
    let c = f32::from(v) / 255.;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0., 1.);
    let v = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1. / 2.4) - 0.055 };
    (v * 255. + 0.5) as u8
}

/// Fraction of texels whose alpha would pass a 0.5 test after scaling by `scale`.
fn coverage(rgba: &[u8], scale: f32) -> f32 {
    let texels = rgba.len() / 4;
    if texels == 0 {
        return 0.;
    }
    let pass = rgba.chunks_exact(4).filter(|p| f32::from(p[3]) * scale >= 127.5).count();
    pass as f32 / texels as f32
}

/// Scales `rgba`'s alpha so its 0.5-test coverage is closest to `target`.
fn preserve_coverage(rgba: &mut [u8], target: f32) {
    let (mut lo, mut hi) = (0.0_f32, 8.0_f32);
    for _ in 0..16 {
        let mid = (lo + hi) / 2.;
        if coverage(rgba, mid) < target { lo = mid } else { hi = mid }
    }
    // Coverage moves in steps; take whichever bound lands closer to the target.
    let scale = if (coverage(rgba, lo) - target).abs() < (coverage(rgba, hi) - target).abs() { lo } else { hi };
    for p in rgba.chunks_exact_mut(4) {
        p[3] = (f32::from(p[3]) * scale).round().clamp(0., 255.) as u8;
    }
}

/// Levels 1.. of the chain for a `width` x `height` RGBA8 image (level 0 is the input).
pub fn generate(width: u32, height: u32, rgba: &[u8], srgb: bool) -> Vec<(u32, u32, Vec<u8>)> {
    assert_eq!(rgba.len(), width as usize * height as usize * 4);
    let translucent = rgba.chunks_exact(4).any(|p| p[3] < 255);
    let target = coverage(rgba, 1.);
    let decode = |v: u8| if srgb { srgb_to_linear(v) } else { f32::from(v) / 255. };
    let encode = |c: f32| if srgb { linear_to_srgb(c) } else { (c.clamp(0., 1.) * 255. + 0.5) as u8 };
    let mut levels = Vec::new();
    let (mut w, mut h) = (width.max(1), height.max(1));
    let mut source = rgba.to_vec();
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut out = vec![0_u8; nw as usize * nh as usize * 4];
        for y in 0..nh {
            for x in 0..nw {
                let mut color = [0_f32; 3];
                let (mut alpha, mut weight, mut plain) = (0_f32, 0_f32, [0_f32; 3]);
                let mut taps = 0_f32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(w - 1);
                    let sy = (y * 2 + dy).min(h - 1);
                    let i = (sy as usize * w as usize + sx as usize) * 4;
                    let a = f32::from(source[i + 3]) / 255.;
                    for c in 0..3 {
                        let v = decode(source[i + c]);
                        color[c] += v * a;
                        plain[c] += v;
                    }
                    alpha += a;
                    weight += a;
                    taps += 1.;
                }
                let o = (y as usize * nw as usize + x as usize) * 4;
                for c in 0..3 {
                    // Alpha-weighted where anything is visible; plain average for fully clear texels.
                    let v = if weight > 0. { color[c] / weight } else { plain[c] / taps };
                    out[o + c] = encode(v);
                }
                out[o + 3] = (alpha / taps * 255. + 0.5) as u8;
            }
        }
        if translucent && target > 0. && target < 1. {
            preserve_coverage(&mut out, target);
        }
        levels.push((nw, nh, out.clone()));
        source = out;
        w = nw;
        h = nh;
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_reaches_one_by_one() {
        assert_eq!(level_count(256, 64), 9);
        assert_eq!(level_count(1, 1), 1);
        let levels = generate(4, 2, &[255; 32], false);
        assert_eq!(levels.iter().map(|(w, h, _)| (*w, *h)).collect::<Vec<_>>(), vec![(2, 1), (1, 1)]);
    }

    #[test]
    fn opaque_averages_and_srgb_averages_in_linear_light() {
        let px = |v: u8| [v, v, v, 255];
        let img: Vec<u8> = [px(0), px(255), px(0), px(255)].concat();
        assert_eq!(generate(2, 2, &img, false)[0].2, vec![128, 128, 128, 255]);
        // Linear mid-grey is about 188 in sRGB, not 128.
        assert_eq!(generate(2, 2, &img, true)[0].2[0], 188);
    }

    #[test]
    fn clear_texels_do_not_darken_visible_colour() {
        let img: Vec<u8> = [[200, 100, 50, 255], [0, 0, 0, 0], [0, 0, 0, 0], [200, 100, 50, 255]].concat();
        let level = &generate(2, 2, &img, false)[0].2;
        assert_eq!(&level[..3], &[200, 100, 50]);
    }

    #[test]
    fn masked_coverage_is_preserved_down_the_chain() {
        // An opaque disc on a clear background (a leaf or a fence post): coverage must stay near
        // the original fraction instead of shrinking with every level.
        let mut img = Vec::new();
        for y in 0..32 {
            for x in 0..32 {
                let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
                img.extend_from_slice(&[0, 255, 0, if dx * dx + dy * dy < 81. { 255 } else { 0 }]);
            }
        }
        let target = coverage(&img, 1.);
        for (_, _, level) in generate(32, 32, &img, false).iter().take(3) {
            assert!((coverage(level, 1.) - target).abs() < 0.08, "coverage {} vs {target}", coverage(level, 1.));
        }
    }
}
