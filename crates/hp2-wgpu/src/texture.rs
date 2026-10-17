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
/// `#[repr(u8)]` with explicit discriminants, not left implicit: the
/// real value arrives from C++ as `Info.Format`, a raw byte, so this
/// needs the same real-discriminant-plus-`from_raw` shape as
/// `BackendChoice::from_ffi_discriminant` already established for
/// the `Backend` ini key, not a Rust-side-only enum whose variant
/// order happens to coincide with the real one today.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    P8 = 0x00,
    Rgba7 = 0x01,
    Rgb16 = 0x02,
    Dxt1 = 0x03,
    Rgb8 = 0x04,
    Rgba8 = 0x05,
}

impl TextureFormat {
    /// Converts a raw `Info.Format` byte to the matching variant,
    /// `None` for anything outside `0x00..=0x05` (including the real
    /// `TEXF_MAX=0xff` sentinel itself, which is never a real
    /// per-texture format value -- see this enum's own doc).
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::P8),
            0x01 => Some(Self::Rgba7),
            0x02 => Some(Self::Rgb16),
            0x03 => Some(Self::Dxt1),
            0x04 => Some(Self::Rgb8),
            0x05 => Some(Self::Rgba8),
            _ => None,
        }
    }
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

/// One real uploaded GPU texture, keyed by the real `CacheID` in a
/// `TextureCache`. `width`/`height` are the real mip-0 `USize`/
/// `VSize` -- kept alongside the `wgpu::Texture` itself since wgpu
/// has no cheap "ask the texture its own size back" accessor as
/// convenient as just storing what was uploaded. `u_mult`/`v_mult`
/// are the real `FTexInfo::UMult`/`VMult` (`SetTexture.cpp:492-493`:
/// `Tex.UMult = 1.f / (Info.UScale * static_cast<FLOAT>(Info.USize))`,
/// confirmed at the real assignment site, not assumed from `1/USize`
/// alone -- an earlier version of this crate's own design omitted
/// the real `UScale`/`VScale` factor entirely, caught on review
/// before any real `DrawTile` UV math depended on it) -- computed
/// once at upload time, matching the real source's own one-time
/// `SetTexture` computation, not recomputed per draw.
pub struct CachedTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub u_mult: f32,
    pub v_mult: f32,
}

/// Real `BindMap`'s own role (`SetTexture.cpp`'s `TMap<QWORD,
/// FCachedTexture> *BindMap`) -- GPU texture objects keyed by the
/// real `CacheID` (`fix_cache_id`'s own output), looked up before
/// deciding whether a new upload is needed. Scope today, explicitly:
/// single mip level only (mip 0 -- the real `FTextureInfo::Mips[]`
/// chain and its own mipmap generation/upload are not ported yet);
/// every `upload_rgba8` call allocates a brand new `wgpu::Texture`
/// and replaces whatever was cached under that `cache_id`, rather
/// than reusing existing GPU storage in place the way the real
/// source's own `IsFirstUpload`/`PartialUpload` distinction does
/// (`UploadTexture`'s own real logic -- `glTexSubImage2D` for an
/// already-allocated texture, not a fresh `glTexImage2D` every time) --
/// correct, but reallocates GPU memory on every re-upload of an
/// already-cached, realtime-changing texture instead of streaming
/// into the existing allocation. A real, known, explicitly scoped
/// limitation, not silently pretended away -- not a correctness bug
/// for any texture that is only ever uploaded once, which is the
/// overwhelming majority of real textures (only realtime-changed ones,
/// e.g. dynamic light palettes or render-to-texture surfaces, re-
/// upload at all).
pub struct TextureCache {
    entries: std::collections::HashMap<u64, CachedTexture>,
}

/// Groups `TextureCache::upload_rgba8`'s own size/scale parameters --
/// real `FTextureInfo::USize`/`VSize`/`UScale`/`VScale` (`UnTex.h:
/// 388-391`), kept together both because they are only ever
/// meaningful as a group (there is no real sense in which a caller
/// would have `width`/`height` without also knowing `u_scale`/
/// `v_scale` from the same real `FTextureInfo`) and to keep
/// `upload_rgba8` itself under clippy's `too_many_arguments` limit
/// (these four fields as direct parameters would have pushed it to 9
/// parameters including `self`, well past the default 7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureDimensions {
    pub width: u32,
    pub height: u32,
    pub u_scale: f32,
    pub v_scale: f32,
}

impl Default for TextureCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// Real `BindMap->Find(Info.CacheID)` -- `cache_id` must already
    /// be the real `fix_cache_id`'s own output, not a raw, untagged
    /// `FTextureInfo::CacheID`, matching the real source's own
    /// `FixCacheID(Info, PolyFlags)` call immediately before its own
    /// `BindMap->Find` (`SetTexture.cpp:88-89`).
    pub fn get(&self, cache_id: u64) -> Option<&CachedTexture> {
        self.entries.get(&cache_id)
    }

    /// Real `UploadTexture`'s `TEXF_RGBA8`/expanded-`TEXF_P8` path,
    /// scoped to this struct's own doc (single mip, always a fresh
    /// allocation). `rgba8_bytes` must be exactly `width * height * 4`
    /// bytes, tightly packed (`FColor`'s own real `R,G,B,A` byte
    /// order, matching `Rgba8Unorm` directly -- `p8_to_rgba8`'s own
    /// doc already covers why no swizzle is needed for a P8-sourced
    /// buffer specifically). `srgb`: `UnpackSRGB`'s own real choice
    /// between `GL_RGBA8`/`GL_SRGB8_ALPHA8` -- `Rgba8Unorm` vs
    /// `Rgba8UnormSrgb` here, the direct wgpu equivalents. Inserts
    /// into (or replaces) this cache under `cache_id` and returns a
    /// reference to the new entry, with its real `u_mult`/`v_mult`
    /// computed from `dims.u_scale`/`v_scale` the same way real
    /// `SetTexture` does (`CachedTexture`'s own doc has the real
    /// citation).
    ///
    /// # Panics
    /// If `rgba8_bytes.len() != dims.width as usize * dims.height as
    /// usize * 4` -- a caller bug (a real `FTextureInfo`'s own
    /// `USize`/`VSize` and its mip-0 data length are never
    /// inconsistent with each other), not a runtime condition to
    /// handle gracefully. This function is not `extern "C"` itself
    /// (no FFI call site calls it directly yet -- see this module's
    /// own header), so a panic here does not yet risk crossing the
    /// FFI boundary; whatever future FFI wrapper calls this must
    /// validate its own inputs before calling in, the same discipline
    /// `hp2-ffi`'s own `guard` already establishes elsewhere in this
    /// port.
    pub fn upload_rgba8(
        &mut self,
        created: &crate::CreatedDevice,
        cache_id: u64,
        dims: TextureDimensions,
        rgba8_bytes: &[u8],
        srgb: bool,
    ) -> &CachedTexture {
        let TextureDimensions {
            width,
            height,
            u_scale,
            v_scale,
        } = dims;
        assert_eq!(
            rgba8_bytes.len(),
            width as usize * height as usize * 4,
            "rgba8_bytes length must be exactly width * height * 4 bytes"
        );
        let format = if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let texture = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hp2-wgpu cached texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            // COPY_SRC: lets this crate's own
            // upload_rgba8_round_trips_real_bytes_through_a_real_gpu_
            // texture test (and any future debug-readback tooling)
            // copy an uploaded texture's real contents out for
            // verification -- negligible cost, same reasoning as
            // RenderLoop::set_scene_node's own frame_uniforms_buffer
            // already established for the equivalent buffer case.
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        created.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba8_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Real UXOpenGLRenderDevice::SetTexture's own exact formula
        // (SetTexture.cpp:492-493) -- CachedTexture's own doc has the
        // full citation.
        let u_mult = 1.0 / (u_scale * width as f32);
        let v_mult = 1.0 / (v_scale * height as f32);
        self.entries.insert(
            cache_id,
            CachedTexture {
                texture,
                view,
                width,
                height,
                u_mult,
                v_mult,
            },
        );
        self.entries
            .get(&cache_id)
            .expect("just inserted this cache_id above")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_matches_the_real_un_tex_h_discriminants() {
        // Pins every real ETextureFormat value (UnTex.h:225-230):
        // TEXF_P8=0x00, TEXF_RGBA7=0x01, TEXF_RGB16=0x02,
        // TEXF_DXT1=0x03, TEXF_RGB8=0x04, TEXF_RGBA8=0x05.
        assert_eq!(TextureFormat::from_raw(0x00), Some(TextureFormat::P8));
        assert_eq!(TextureFormat::from_raw(0x01), Some(TextureFormat::Rgba7));
        assert_eq!(TextureFormat::from_raw(0x02), Some(TextureFormat::Rgb16));
        assert_eq!(TextureFormat::from_raw(0x03), Some(TextureFormat::Dxt1));
        assert_eq!(TextureFormat::from_raw(0x04), Some(TextureFormat::Rgb8));
        assert_eq!(TextureFormat::from_raw(0x05), Some(TextureFormat::Rgba8));
        // The real TEXF_MAX sentinel (0xff) is never a real
        // per-texture value -- must return None, not panic or wrap.
        assert_eq!(TextureFormat::from_raw(0xff), None);
        // Every other out-of-range byte must also return None, not
        // silently alias to a valid variant.
        assert_eq!(TextureFormat::from_raw(0x06), None);
        assert_eq!(TextureFormat::from_raw(0x80), None);
    }

    #[test]
    fn enum_discriminants_themselves_match_the_real_un_tex_h_values() {
        // The test above only checks from_raw's own match arms
        // return the right variant -- it would not catch the
        // `#[repr(u8)]` discriminants on TextureFormat itself
        // silently drifting from the real UnTex.h values while
        // from_raw's arms stayed internally self-consistent with the
        // wrong ones (caught on review: a real gap, not covered by
        // the test above). Checks `as u8` directly against the real
        // values, and that every discriminant round-trips through
        // from_raw back to the same variant.
        assert_eq!(TextureFormat::P8 as u8, 0x00);
        assert_eq!(TextureFormat::Rgba7 as u8, 0x01);
        assert_eq!(TextureFormat::Rgb16 as u8, 0x02);
        assert_eq!(TextureFormat::Dxt1 as u8, 0x03);
        assert_eq!(TextureFormat::Rgb8 as u8, 0x04);
        assert_eq!(TextureFormat::Rgba8 as u8, 0x05);
        for format in [
            TextureFormat::P8,
            TextureFormat::Rgba7,
            TextureFormat::Rgb16,
            TextureFormat::Dxt1,
            TextureFormat::Rgb8,
            TextureFormat::Rgba8,
        ] {
            assert_eq!(TextureFormat::from_raw(format as u8), Some(format));
        }
    }

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

    // Needs a real GPU adapter. CI runners (ci.yml) have none. Run with
    // `cargo test -p hp2-wgpu -- --ignored`. Don't make it pass when no
    // adapter is found: that would hide real failures on machines that
    // do have one.
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn upload_rgba8_round_trips_real_bytes_through_a_real_gpu_texture() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let mut cache = TextureCache::new();
        assert!(
            cache.get(42).is_none(),
            "no entry should exist before the first upload"
        );
        // 2x1 texture: left texel red, right texel green, both full
        // alpha -- matches FColor's own real R,G,B,A byte order
        // (fcolor_memory_layout_is_r_g_b_a_matching_rgba8unorm above),
        // directly as Rgba8Unorm bytes.
        let bytes: [u8; 8] = [255, 0, 0, 255, 0, 255, 0, 255];
        // u_scale=2.0, v_scale=0.5 (distinct, non-1.0 values) to
        // prove the real UMult/VMult formula is actually threaded
        // through, not just defaulted to 1/width.
        let cached = cache.upload_rgba8(
            &created,
            42,
            TextureDimensions {
                width: 2,
                height: 1,
                u_scale: 2.0,
                v_scale: 0.5,
            },
            &bytes,
            false,
        );
        assert_eq!(cached.width, 2);
        assert_eq!(cached.height, 1);
        // Real SetTexture.cpp:492-493 formula, hand-computed
        // independently: UMult = 1/(UScale*USize) = 1/(2.0*2) = 0.25,
        // VMult = 1/(VScale*VSize) = 1/(0.5*1) = 2.0.
        assert_eq!(cached.u_mult, 0.25);
        assert_eq!(cached.v_mult, 2.0);

        // Real proof the bytes actually reached the GPU, not just
        // that create_texture/write_texture didn't error: copy the
        // real uploaded texture back out and compare.
        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("texture cache readback buffer"),
            size: 256, // one row's min bytes_per_row alignment
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let cached_texture = cache
            .get(42)
            .expect("expected the entry just uploaded above to be found by get")
            .texture
            .clone();
        let mut encoder = created
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &cached_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        created.queue.submit(std::iter::once(encoder.finish()));
        let slice = readback_buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("readback buffer map_async should succeed on a working device");
        });
        created
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device poll should succeed on a working device");
        let mapped = slice
            .get_mapped_range()
            .expect("buffer should be mapped after a successful map_async + device.poll");
        assert_eq!(
            &mapped[0..4],
            &[255u8, 0, 0, 255],
            "expected the left texel to read back red"
        );
        assert_eq!(
            &mapped[4..8],
            &[0u8, 255, 0, 255],
            "expected the right texel to read back green"
        );
        println!(
            "TextureCache::upload_rgba8 confirmed empirically to round-trip real bytes through a \
             real GPU texture on adapter {:?} backend {}",
            created.adapter_name,
            crate::backend_name(created.backend)
        );
    }
}
