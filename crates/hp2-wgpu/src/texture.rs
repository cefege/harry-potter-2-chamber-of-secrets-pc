//! Rust port of `ThirdParty/XOpenGLDrv/Src/SetTexture.cpp`'s texture
//! cache/upload (plan's "Texture upload/cache in Rust" Phase 3 item).
//! Scope today: the `TEXF_P8` (indexed-palette) -> RGBA8 expansion
//! (`SetTexture.cpp`'s own `case TEXF_P8:` conversion loop,
//! `ConvertP8_RGBA8888`, read in full), the real `ETextureFormat`
//! enum and its small pure helpers (`HP2TextureBytes`/
//! `HP2TextureFormatName`/`IsHP2CompressedTexture`), and `FixCacheID`'s
//! real masked-vs-unmasked bind-map-key tagging (the tagging logic
//! itself, not yet wired to any actual cache/`BindMap` structure --
//! no cache exists in this crate yet, see below). Not yet ported:
//! an actual texture cache structure (the real `BindMap`/
//! `FCachedTexture`, keyed by `CacheID`), `TEXF_DXT1`/BC upload and
//! the still-undecided mip strategy noted in this crate's own module
//! doc, actual GPU upload for any format, and the lightmap `GL_BGRA`
//! swizzle note (`DrawComplex_GLSL.cpp:439-444`'s `OPT_GLES .bgr`
//! branch) -- upload lightmaps as `Bgra8Unorm` (or CPU-swizzle before
//! upload) to keep `complex.wgsl`'s plain `.rgb` sampling correct
//! once lightmap upload is actually implemented; recorded here so the
//! decision is not lost before that lands.

/// Real `ETextureFormat` (`UnTex.h`): `TEXF_P8=0x00, TEXF_RGBA7=0x01,
/// TEXF_RGB16=0x02, TEXF_DXT1=0x03, TEXF_RGB8=0x04, TEXF_RGBA8=0x05`
/// (`TEXF_MAX=0xff` is a bound sentinel the real source never uses as
/// an actual per-texture format value, so it has no variant here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    P8,
    Rgba7,
    Rgb16,
    Dxt1,
    Rgb8,
    Rgba8,
}

/// `SetTexture.cpp`'s own `IsHP2CompressedTexture`: the only
/// compressed format in use is `TEXF_DXT1` -- confirmed by reading
/// every `case` this real source's own format-dispatch switches
/// handle, not assumed from the format list alone.
pub fn is_compressed_texture(format: TextureFormat) -> bool {
    format == TextureFormat::Dxt1
}

/// `SetTexture.cpp`'s own real `HP2TextureBytes`: BC1/DXT1's real
/// 4x4-block, 8-bytes-per-block size formula (`Max(1, (U+3)/4) *
/// Max(1, (V+3)/4) * 8`, correctly rounding a non-multiple-of-4
/// dimension up to a whole block, down to a 1x1-block floor for tiny
/// mips) for the one compressed format, plain `width * height *
/// bytes_per_pixel` for every uncompressed one.
pub fn texture_bytes(format: TextureFormat, u_size: i32, v_size: i32) -> i32 {
    if format == TextureFormat::Dxt1 {
        return 1.max((u_size + 3) / 4) * 1.max((v_size + 3) / 4) * 8;
    }
    let bytes_per_pixel = match format {
        TextureFormat::P8 => 1,
        TextureFormat::Rgb16 => 2,
        TextureFormat::Rgb8 => 3,
        // Rgba7/Rgba8/Dxt1(unreachable here) -- the real source's own
        // ternary chain falls through to 4 for everything not
        // explicitly named above, Dxt1 already returned earlier.
        _ => 4,
    };
    u_size * v_size * bytes_per_pixel
}

/// `SetTexture.cpp`'s own real `HP2TextureFormatName` -- byte-
/// identical strings, used in real log output this port must not
/// silently rename.
pub fn texture_format_name(format: TextureFormat) -> &'static str {
    match format {
        TextureFormat::P8 => "P8",
        TextureFormat::Rgba7 => "RGBA7",
        TextureFormat::Rgb16 => "RGB16",
        TextureFormat::Dxt1 => "DXT1",
        TextureFormat::Rgb8 => "RGB8",
        TextureFormat::Rgba8 => "RGBA8",
    }
}

/// Real `PF_Masked` (`UnObj.h`): `0x00000002`.
const PF_MASKED: u32 = 0x0000_0002;

/// Real `MASKED_TEXTURE_TAG` (`SetTexture.cpp`): bit 2 (`4`), reused
/// as a cache-key tag because "the least significant bits of a
/// CacheID are always zero" (the real source's own comment, not
/// re-derived here).
const MASKED_TEXTURE_TAG: u64 = 4;

/// Real `FixCacheID`: makes the masked and unmasked copies of a
/// `TEXF_P8` texture map to different real cache keys, since
/// `PF_Masked` drawing needs palette index 0 forced fully transparent
/// (`p8_to_rgba8`'s own `P8MaskMode::Masked`) while non-masked drawing
/// needs that same index's real, non-overridden color -- one texture
/// genuinely needs two different uploaded copies depending on how a
/// given draw call uses it, confirmed by reading the real source's
/// own comment on why, not just copying the bit-twiddling. Takes and
/// returns the real `CacheID` rather than mutating a passed-in
/// `FTextureInfo`-shaped struct in place: no such cache struct exists
/// in this crate yet (see this module's own header), so there is
/// nothing to mutate beyond the one `u64` value itself.
pub fn fix_cache_id(cache_id: u64, format: TextureFormat, poly_flags: u32) -> u64 {
    if (poly_flags & PF_MASKED) != 0 && format == TextureFormat::P8 {
        cache_id | MASKED_TEXTURE_TAG
    } else {
        cache_id & !MASKED_TEXTURE_TAG
    }
}

/// Real `FColor` (`UnTex.h`), re-used from `hp2-abi` rather than
/// redefined here (caught on review: an earlier version of this
/// module defined its own `#[repr(C)] struct FColor` with no
/// `align(4)`, which would have been a real layout bug if this value
/// ever became a direct view of raw `FTextureInfo::Palette` memory --
/// `hp2_abi::FColor` is already probe-verified against the real C++
/// layout, including that `align(4)` from the real `union { DWORD
/// Int4; struct {...}; }`, and its own doc already covers the real
/// R,G,B,A memory order this module's "no swizzle needed for
/// Rgba8Unorm" claim below depends on, not duplicated here). Rust-
/// internal only: never crosses the FFI boundary through cbindgen
/// (this module has no `extern "C"` functions at all yet), so this
/// does not risk the real redefinition collision a `SceneFrame`-level
/// change would have -- `hp2_rs.h` is `#include`d in many real C++
/// translation units that already define their own `class FColor`/
/// `FCoords`/`FVector` from the real UE1 headers (confirmed before
/// considering this: WgpuDrv.cpp, SDLClient.cpp, ALAudioSubsystem.cpp,
/// and others), so cbindgen must never be told to emit a conflicting
/// typedef for any of those three names -- `SceneFrame`'s own flat
/// `[f32; 3]` fields deliberately stay as they are for exactly this
/// reason, not changed to `hp2_abi::FCoords` despite the superficial
/// appeal of a direct struct-pointer pass-through.
pub use hp2_abi::FColor;

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

    #[test]
    fn texture_format_name_matches_the_real_source_strings_exactly() {
        assert_eq!(texture_format_name(TextureFormat::P8), "P8");
        assert_eq!(texture_format_name(TextureFormat::Rgba7), "RGBA7");
        assert_eq!(texture_format_name(TextureFormat::Rgb16), "RGB16");
        assert_eq!(texture_format_name(TextureFormat::Dxt1), "DXT1");
        assert_eq!(texture_format_name(TextureFormat::Rgb8), "RGB8");
        assert_eq!(texture_format_name(TextureFormat::Rgba8), "RGBA8");
    }

    #[test]
    fn is_compressed_texture_is_true_only_for_dxt1() {
        assert!(is_compressed_texture(TextureFormat::Dxt1));
        assert!(!is_compressed_texture(TextureFormat::P8));
        assert!(!is_compressed_texture(TextureFormat::Rgba7));
        assert!(!is_compressed_texture(TextureFormat::Rgb16));
        assert!(!is_compressed_texture(TextureFormat::Rgb8));
        assert!(!is_compressed_texture(TextureFormat::Rgba8));
    }

    #[test]
    fn texture_bytes_matches_the_real_formula_for_every_format() {
        // Uncompressed: plain width * height * bytes-per-pixel.
        assert_eq!(texture_bytes(TextureFormat::P8, 64, 64), 64 * 64);
        assert_eq!(texture_bytes(TextureFormat::Rgb16, 64, 64), 64 * 64 * 2);
        assert_eq!(texture_bytes(TextureFormat::Rgb8, 64, 64), 64 * 64 * 3);
        assert_eq!(texture_bytes(TextureFormat::Rgba7, 64, 64), 64 * 64 * 4);
        assert_eq!(texture_bytes(TextureFormat::Rgba8, 64, 64), 64 * 64 * 4);
        // DXT1: 4x4 blocks, 8 bytes/block, exact multiple of 4.
        assert_eq!(texture_bytes(TextureFormat::Dxt1, 64, 64), 16 * 16 * 8);
        // DXT1 non-multiple-of-4 dimensions round UP to a whole block
        // (the real source's own Max(1, (U+3)/4) -- not a floor
        // division, which would silently drop a partial block's real
        // storage requirement).
        assert_eq!(texture_bytes(TextureFormat::Dxt1, 5, 5), 2 * 2 * 8);
        // Tiny mips floor at one whole block, never zero blocks --
        // the real source's own Max(1, ...) on each axis.
        assert_eq!(texture_bytes(TextureFormat::Dxt1, 1, 1), 8);
    }

    #[test]
    fn fix_cache_id_tags_p8_masked_textures_only() {
        const PF_MASKED: u32 = 0x0000_0002;
        const PF_INVISIBLE: u32 = 0x0000_0001;
        // P8 + masked: real masked-copy tag bit set.
        assert_eq!(fix_cache_id(0x1000, TextureFormat::P8, PF_MASKED), 0x1004);
        // P8 + not masked: tag bit cleared (even if some other,
        // unrelated poly flag is set).
        assert_eq!(
            fix_cache_id(0x1004, TextureFormat::P8, PF_INVISIBLE),
            0x1000
        );
        // Non-P8 format + masked flag: the real source's own `&&
        // Info.Format == TEXF_P8` guard means masking never tags a
        // non-P8 format's cache key at all, regardless of PF_Masked.
        assert_eq!(
            fix_cache_id(0x1000, TextureFormat::Rgba8, PF_MASKED),
            0x1000
        );
        // Already-tagged non-P8 entry still gets the tag bit cleared
        // on a non-masked call -- the real source's own unconditional
        // `else` branch, not a no-op when the format isn't P8.
        assert_eq!(
            fix_cache_id(0x1004, TextureFormat::Rgba8, PF_INVISIBLE),
            0x1000
        );
    }

    #[test]
    fn fix_cache_id_only_touches_the_real_tag_bit() {
        // The real source only ORs/ANDs bit 2 (MASKED_TEXTURE_TAG=4)
        // -- every other bit of a real CacheID must survive
        // unchanged, masked or not.
        let cache_id = 0xDEAD_BEEF_0000_0010u64;
        let masked = fix_cache_id(cache_id, TextureFormat::P8, 0x0000_0002);
        assert_eq!(masked & !0b100, cache_id & !0b100);
        assert_eq!(masked & 0b100, 0b100);
        let unmasked = fix_cache_id(cache_id, TextureFormat::P8, 0);
        assert_eq!(unmasked, cache_id);
    }

    fn reference_palette() -> [FColor; 256] {
        let mut palette = [FColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }; 256];
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
