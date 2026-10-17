//! Rust port of `ThirdParty/XOpenGLDrv/Src/SetTexture.cpp`'s texture
//! cache/upload (plan's "Texture upload/cache in Rust" Phase 3 item).
//! Scope today: the `TEXF_P8` (indexed-palette) -> RGBA8 expansion
//! only (`SetTexture.cpp`'s own `case TEXF_P8:` conversion loop,
//! `ConvertP8_RGBA8888`, read in full). Not yet ported: `CacheID`
//! keying/`FixCacheID`'s masked-vs-unmasked bind-map split (real
//! logic understood, not yet wired to any cache structure here),
//! `TEXF_DXT1`/BC upload and the still-undecided mip strategy noted
//! in this crate's own module doc, the `TEXF_RGBA7`/`RGB16`/`RGB8`
//! uncompressed-but-not-P8 formats, and the lightmap `GL_BGRA`
//! swizzle note (`DrawComplex_GLSL.cpp:439-444`'s `OPT_GLES .bgr`
//! branch) -- upload lightmaps as `Bgra8Unorm` (or CPU-swizzle before
//! upload) to keep `complex.wgsl`'s plain `.rgb` sampling correct
//! once lightmap upload is actually implemented; recorded here so the
//! decision is not lost before that lands.

/// Byte-for-byte mirror of the real `FColor` (`UnTex.h`): a union of
/// one `DWORD` (`Int4`) and four `BYTE`s in `R, G, B, A` memory order
/// on this project's own real build targets. `__INTEL_BYTE_ORDER__`
/// is unconditionally `1` for every platform this codebase actually
/// builds for (`UnGnuG.h`: macOS/Linux arm64 and x86_64, Emscripten --
/// each with its own `#error ... require little-endian byte order`
/// assertion right next to the `#define`), so the real in-memory
/// layout is always `R, G, B, A`, never the big-endian `A, B, G, R`
/// alternative in the same union -- confirmed by reading `UnTex.h`'s
/// own union definition and every platform header that sets the
/// macro, not assumed from the name alone.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Real source's `MASKED_TEXTURE_TAG`/`FixCacheID`-driven behavior
/// (`SetTexture.cpp`'s own comment: "we might render the textures
/// without PF_Masked later... we potentially need two copies of all
/// P8 textures"), expressed as the one real difference it makes to
/// this function rather than the cache-key logic around it (not
/// ported yet -- see this module's own header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P8MaskMode {
    /// `PolyFlags & PF_Masked` unset: palette index 0 keeps its real
    /// RGBA color (`Palette` used as-is).
    Unmasked,
    /// `PolyFlags & PF_Masked` set: a local copy of the palette has
    /// index 0 forced to `FColor(0, 0, 0, 0)` (`SetTexture.cpp`'s own
    /// `LocalPal[0] = FColor(0, 0, 0, 0);`) before expansion, so
    /// palette-0 texels become fully transparent instead of keeping
    /// whatever RGB the source palette actually stored there.
    Masked,
}

/// Replaces `SetTexture.cpp`'s real `case TEXF_P8:` expansion loop
/// (`ConvertP8_RGBA8888`, `*Ptr++ = GET_COLOR_DWORD(Palette[Mip->
/// DataPtr[i]]);`) plus the `PF_Masked` local-palette-copy branch
/// immediately above it in the same function. `indices` is one byte
/// per texel (the real `Mip->DataPtr`, a `TEXF_P8` mip's raw indexed
/// data); `palette` is the real 256-entry `FTextureInfo::Palette`.
/// Returns one `FColor` per input index, in the same order -- the
/// real source's own output shape before it gets reinterpreted as a
/// flat byte buffer for `glTexImage2D`'s `GL_RGBA`/`GL_UNSIGNED_BYTE`
/// upload; this crate's own future upload code reinterprets the
/// result the same way for `wgpu::TextureFormat::Rgba8Unorm` (or
/// `Rgba8UnormSrgb` under `UnpackSRGB` -- `SetTexture.cpp`'s own
/// `InternalFormat = UnpackSRGB ? GL_SRGB8_ALPHA8 : GL_RGBA8`), since
/// `FColor`'s own real `R,G,B,A` memory order already matches
/// `Rgba8Unorm`'s byte order exactly -- no swizzle needed for this
/// conversion specifically (unlike the lightmap `.bgr` case this
/// module's own header flags separately).
pub fn p8_to_rgba8(indices: &[u8], palette: &[FColor; 256], mask_mode: P8MaskMode) -> Vec<FColor> {
    let mut local_palette;
    let palette = match mask_mode {
        P8MaskMode::Unmasked => palette,
        P8MaskMode::Masked => {
            local_palette = *palette;
            local_palette[0] = FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
            &local_palette
        }
    };
    indices
        .iter()
        .map(|&index| palette[index as usize])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_palette() -> [FColor; 256] {
        let mut palette = [FColor::default(); 256];
        for (i, entry) in palette.iter_mut().enumerate() {
            // Distinct, recognizable per-index values, not all-zero:
            // a bug that only showed up for a non-zero index (e.g. an
            // off-by-one in the index->palette lookup) would pass a
            // single-entry or all-same-palette test by coincidence.
            *entry = FColor {
                r: i as u8,
                g: (i as u8).wrapping_mul(3),
                b: (i as u8).wrapping_mul(7),
                a: 255,
            };
        }
        palette
    }

    #[test]
    fn unmasked_expansion_uses_the_real_palette_entries_directly() {
        let palette = reference_palette();
        let indices = [0u8, 1, 5, 255, 5, 0];
        let out = p8_to_rgba8(&indices, &palette, P8MaskMode::Unmasked);
        let expected: Vec<FColor> = indices.iter().map(|&i| palette[i as usize]).collect();
        assert_eq!(out, expected);
        // Index 0 keeps its real palette color unmasked -- the real
        // source's own distinction this test exists to pin, checked
        // explicitly rather than only via the generic loop above.
        assert_eq!(out[0], palette[0]);
        assert_ne!(
            out[0],
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            },
            "unmasked index 0 must not be forced transparent"
        );
    }

    #[test]
    fn masked_expansion_forces_index_zero_to_fully_transparent() {
        let palette = reference_palette();
        // Confirm the reference palette's real index-0 entry is NOT
        // already (0,0,0,0) -- otherwise this test could pass even if
        // the masking logic were silently absent (a degenerate,
        // already-transparent input).
        assert_ne!(
            palette[0],
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            }
        );
        let indices = [0u8, 1, 0, 5];
        let out = p8_to_rgba8(&indices, &palette, P8MaskMode::Masked);
        assert_eq!(
            out[0],
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            }
        );
        assert_eq!(
            out[2],
            FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            }
        );
        // Every other index must be completely unaffected by masking
        // -- SetTexture.cpp's own LocalPal copy only overwrites index
        // 0, nothing else in the 256-entry table.
        assert_eq!(out[1], palette[1]);
        assert_eq!(out[3], palette[5]);
    }

    #[test]
    fn masked_and_unmasked_expansions_differ_only_at_index_zero() {
        let palette = reference_palette();
        let indices: Vec<u8> = (0u8..=255).collect();
        let unmasked = p8_to_rgba8(&indices, &palette, P8MaskMode::Unmasked);
        let masked = p8_to_rgba8(&indices, &palette, P8MaskMode::Masked);
        for i in 1..256usize {
            assert_eq!(
                unmasked[i], masked[i],
                "index {i} must be identical between masked and unmasked expansion"
            );
        }
        assert_ne!(unmasked[0], masked[0]);
    }

    #[test]
    fn output_length_and_order_match_the_input_indices_exactly() {
        let palette = reference_palette();
        let indices = [200u8, 3, 3, 3, 1];
        let out = p8_to_rgba8(&indices, &palette, P8MaskMode::Unmasked);
        assert_eq!(out.len(), indices.len());
        for (i, &index) in indices.iter().enumerate() {
            assert_eq!(out[i], palette[index as usize]);
        }
    }

    #[test]
    fn fcolor_memory_layout_is_r_g_b_a_matching_rgba8unorm() {
        // Confirms the #[repr(C)] field order really is R,G,B,A in
        // memory (not just in the Rust struct's own field list,
        // which repr(C) guarantees matches declaration order, but
        // worth pinning explicitly since this is the one fact the
        // whole module's "no swizzle needed for Rgba8Unorm" claim
        // depends on).
        let color = FColor {
            r: 0x11,
            g: 0x22,
            b: 0x33,
            a: 0x44,
        };
        let bytes: [u8; 4] = unsafe { std::mem::transmute::<FColor, [u8; 4]>(color) };
        assert_eq!(bytes, [0x11, 0x22, 0x33, 0x44]);
    }
}
