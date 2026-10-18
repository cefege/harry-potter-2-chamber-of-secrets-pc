//! Rust port of `ThirdParty/XOpenGLDrv/Src/SetTexture.cpp`'s texture
//! cache/upload (plan's "Texture upload/cache in Rust" Phase 3 item).
//! Scope today: the `TEXF_P8` (indexed-palette) -> RGBA8 expansion
//! (`SetTexture.cpp`'s own `case TEXF_P8:` conversion loop,
//! `ConvertP8_RGBA8888`, read in full), the real `ETextureFormat`
//! enum and its small pure helpers (`HP2TextureBytes`/
//! `HP2TextureFormatName`/`IsHP2CompressedTexture`), `FixCacheID`'s
//! real masked-vs-unmasked bind-map-key tagging, a real `TextureCache`
//! (the real `BindMap`/`FCachedTexture`, keyed by `CacheID`) with
//! real GPU-backed `upload_rgba8`/`create_gpu_texture`, and real
//! `TEXF_P8`/`TEXF_RGBA8` GPU upload end to end (`hp2-ffi`'s own
//! `hp2_rs_wgpu_upload_texture`, wired from every real `WgpuDrv.cpp`
//! draw override that needs a texture -- diffuse and lightmap/fogmap
//! alike, `DrawComplexSurface`'s own override uploads all three
//! through this same path). Also ported: `TEXF_DXT1` -> `Bc1RgbaUnorm`
//! upload with the asset's own full mip chain (`create_compressed_gpu_
//! texture`/`write_bc1_levels`; wgpu cannot generate BC mips, so
//! uncompressed formats stay mip-0-only), `TEXF_RGB8`/`TEXF_RGB16`
//! conversion (`rgb8_to_rgba8`/`rgb16_to_rgba8`), and `TEXF_RGBA7`
//! (byte-identical to RGBA8; always linear, as is RGB16, matching
//! `SetTexture.cpp:247-257`). A re-upload whose width, height, format
//! and mip count all match the resident texture writes into it in
//! place instead of reallocating (`TextureCache::upload_rgba8`/
//! `upload_compressed`).
//!
//! Investigated and resolved, not left open: lightmap upload needed
//! no `GL_BGRA`/`.bgr` swizzle after all, unlike an earlier, more
//! cautious version of this paragraph assumed before actually reading
//! the real condition that gates it (`DrawComplex_GLSL.cpp:439-444`'s
//! `OPT_GLES` branch). That branch only ever compiles in when
//! `UXOpenGLRenderDevice::OpenGLVersion == GL_ES` (`ShaderProgram.cpp:
//! 1048-1051`), and this port's own two real target platforms never
//! produce that value: macOS hardcodes `OpenGLVersion = GL_Core`
//! unconditionally (`XOpenGL.cpp:391-392`, "HP2's macOS renderer is
//! deliberately limited to Apple's OpenGL 4.1 core surface"), and
//! Linux defaults to `GL_Core` too, only ever falling back to `GL_ES`
//! if the real GPU driver cannot provide OpenGL 3.3 core at all
//! (`XOpenGL.cpp:824-829`) -- not a configuration this project's own
//! supported Linux arm64 target normally hits. The real `#else`
//! branch (`OPT_GLCore`) is plain `LightColor.rgb`, no swizzle at all
//! -- exactly what `complex.wgsl`'s own already-shipped lightmap
//! sampling already does (`light_map_sample.rgb * (frame.light_map_
//! intensity * 255.0 / 127.0)`). No code change was needed.

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

/// Replaces `SetTexture.cpp`'s real `TEXF_RGB8` upload path
/// (`InternalFormat = UnpackSRGB ? GL_SRGB8 : GL_RGB8; SourceFormat =
/// GL_RGB;`, `ImgSrc = Mip->DataPtr` passed straight through --
/// confirmed by direct re-read that the real engine performs no CPU-
/// side conversion at all here, relying on OpenGL's own native
/// 3-byte-per-texel `GL_RGB`/`GL_RGB8` support). wgpu has no 3-byte-
/// per-texel texture format at all (no GPU API commonly exposes one,
/// for alignment reasons), so unlike the real source, this port must
/// expand to 4 bytes per texel before upload -- alpha forced to 255
/// (opaque), the real source's own implicit behavior too: `GL_RGB`
/// source data has no alpha channel for the driver to read in the
/// first place, and a real `GL_RGB8`-internal-format texture has no
/// alpha channel to sample either, so every real consumer of a
/// TEXF_RGB8 texture already only ever sees fully opaque texels.
/// `rgb_bytes` must be exactly `width * height * 3` bytes, tightly
/// packed (R,G,B each one byte, matching `HP2TextureBytes`'s own
/// real 3-bytes-per-pixel formula for this format).
///
/// # Panics
/// If `rgb_bytes.len()` is not a whole multiple of 3 -- a caller bug,
/// same `FTextureInfo`-fields-are-never-inconsistent reasoning
/// `TextureCache::upload_rgba8`'s own doc already establishes.
pub fn rgb8_to_rgba8(rgb_bytes: &[u8]) -> Vec<u8> {
    assert_eq!(
        rgb_bytes.len() % 3,
        0,
        "rgb_bytes length must be a whole multiple of 3"
    );
    rgb_bytes
        .chunks_exact(3)
        .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
        .collect()
}

/// Replaces `SetTexture.cpp`'s real `TEXF_RGB16` upload path
/// (`SourceType = GL_UNSIGNED_SHORT_5_6_5_REV`, `ImgSrc = Mip->
/// DataPtr` passed straight through to the driver, which performs the
/// packed-to-float unpacking in hardware). wgpu has no equivalent
/// packed-565 texture format, so unlike the real source this port
/// must unpack on the CPU before upload. Real bit layout confirmed
/// from the OpenGL specification's own packed-pixel-type table before
/// writing this, not assumed from the `_REV` suffix's name alone (an
/// easy direction to get backwards -- the non-REV `GL_UNSIGNED_SHORT_
/// 5_6_5` layout is the *opposite* of this one): for `GL_RGB` +
/// `_5_6_5_REV`, a little-endian 16-bit value holds B in bits 15..11
/// (5 bits), G in bits 10..5 (6 bits), R in bits 4..0 (5 bits) --
/// functionally identical to `GL_BGR` + plain `_5_6_5`, per the
/// OpenGL wiki's own Pixel Transfer page. Each N-bit channel is
/// expanded to a full byte via bit replication (`(v << (8-n)) | (v >>
/// (2n-8))`), the standard, widely-used technique that maps 0 to 0
/// and the real N-bit maximum to 255 exactly, not a plain linear
/// rescale (which also maps the endpoints correctly but distributes
/// the real OpenGL-driver-produced intermediate values differently --
/// bit replication is what real GPU texture samplers and drivers
/// generally use for this exact kind of fixed-point channel expansion).
/// Alpha is always 255 (opaque) -- `GL_RGB`, like `TEXF_RGB8`, has no
/// real alpha channel at all. `packed_bytes` must be exactly `width *
/// height * 2` bytes (one little-endian `u16` per texel, matching
/// `HP2TextureBytes`'s own real 2-bytes-per-pixel formula for this
/// format).
///
/// # Panics
/// If `packed_bytes.len()` is not a whole multiple of 2 -- a caller
/// bug, same reasoning as `rgb8_to_rgba8`'s own doc.
pub fn rgb16_to_rgba8(packed_bytes: &[u8]) -> Vec<u8> {
    assert_eq!(
        packed_bytes.len() % 2,
        0,
        "packed_bytes length must be a whole multiple of 2"
    );
    packed_bytes
        .chunks_exact(2)
        .flat_map(|pair| {
            let value = u16::from_le_bytes([pair[0], pair[1]]);
            let r5 = (value & 0x1F) as u8;
            let g6 = ((value >> 5) & 0x3F) as u8;
            let b5 = ((value >> 11) & 0x1F) as u8;
            let r8 = (r5 << 3) | (r5 >> 2);
            let g8 = (g6 << 2) | (g6 >> 4);
            let b8 = (b5 << 3) | (b5 >> 2);
            [r8, g8, b8, 255]
        })
        .collect()
}

/// One real uploaded GPU texture, keyed by the real `CacheID` in a
/// `TextureCache`. `width`/`height` are the real mip-0 `USize`/
/// `VSize` -- kept alongside the `wgpu::Texture` itself since wgpu
/// has no cheap "ask the texture its own size back" accessor as
/// convenient as just storing what was uploaded, and are the real
/// inputs `texture_uv_mult` needs. Does *not* store `UMult`/`VMult`
/// -- an earlier version of this struct did, computed once at upload
/// time (caught on review, reverted before any real `DrawTile`
/// wiring depended on it): the real source's own `SetTexture.cpp:
/// 492-493` computes `Tex.UMult`/`VMult` fresh on *every* real
/// `SetTexture` bind call, from `Info.UScale`/`VScale` -- confirmed
/// by direct re-read, the assignment sits at the very top of
/// `SetTexture`, before the cached-texture lookup and before the
/// already-bound-and-up-to-date fast-path bailout (`SetTexture.cpp:
/// 502`), not inside `UploadTexture`'s own once-per-upload logic.
/// Caching it here at upload time would go stale the moment the same
/// real `CacheID` is ever bound again with a different `UScale`/
/// `VScale` on its own `FTextureInfo` -- `texture_uv_mult`'s own doc
/// has the real standalone replacement.
pub struct CachedTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    /// Sampler state for this texture, fixed at its first upload
    /// (`SamplerKey`'s own doc).
    pub sampler_key: SamplerKey,
}

/// Real per-texture sampler state (`UXOpenGLRenderDevice::SetSampler`,
/// SetTexture.cpp:138-166). XOpenGL runs `SetSampler` only when a
/// `BindMap` entry is first created (`IsNewBind`, SetTexture.cpp:
/// 528-536), so a texture keeps its first upload's key for its whole
/// cache lifetime, including later re-uploads and reallocations.
/// Not ported: the `NoFiltering`/`LODBias` ini keys (WgpuDrv registers
/// neither) and `UseTrilinear=False` (XOpenGL's default is on, and any
/// `MaxAnisotropy > 0` forces it on anyway, CheckExtensions.cpp:317).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SamplerKey {
    /// `GL_CLAMP_TO_EDGE` on U instead of `GL_REPEAT`: the texture is
    /// a light/fog map (`Info.Texture == NULL`), or `0 < Info.UClamp <
    /// Info.USize`.
    pub clamp_u: bool,
    /// Same as `clamp_u`, for V (`Info.VClamp`/`Info.VSize`).
    pub clamp_v: bool,
    /// `PF_NoSmooth` on the diffuse slot: nearest filtering, no
    /// anisotropy.
    pub no_smooth: bool,
}

impl SamplerKey {
    pub const CLAMP_U: u32 = 1 << 0;
    pub const CLAMP_V: u32 = 1 << 1;
    pub const NO_SMOOTH: u32 = 1 << 2;

    /// Decodes the FFI bit set (`CLAMP_U | CLAMP_V | NO_SMOOTH`);
    /// unknown bits are ignored.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            clamp_u: bits & Self::CLAMP_U != 0,
            clamp_v: bits & Self::CLAMP_V != 0,
            no_smooth: bits & Self::NO_SMOOTH != 0,
        }
    }

    /// The wgpu equivalent of `SetSampler`'s GL calls. Smooth
    /// textures use linear mag/min/mip filtering (`GL_LINEAR` /
    /// `GL_LINEAR_MIPMAP_LINEAR`) plus `anisotropy_clamp`.
    /// `PF_NoSmooth` uses nearest throughout (`GL_NEAREST` /
    /// `GL_NEAREST_MIPMAP_NEAREST`) with no anisotropy, which wgpu
    /// requires for any non-linear filter.
    pub fn descriptor(self, anisotropy_clamp: u16) -> wgpu::SamplerDescriptor<'static> {
        let address_mode = |clamp: bool| {
            if clamp {
                wgpu::AddressMode::ClampToEdge
            } else {
                wgpu::AddressMode::Repeat
            }
        };
        let (filter, mipmap_filter, anisotropy_clamp) = if self.no_smooth {
            (
                wgpu::FilterMode::Nearest,
                wgpu::MipmapFilterMode::Nearest,
                1,
            )
        } else {
            (
                wgpu::FilterMode::Linear,
                wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp.max(1),
            )
        };
        wgpu::SamplerDescriptor {
            label: Some("hp2-wgpu texture sampler"),
            address_mode_u: address_mode(self.clamp_u),
            address_mode_v: address_mode(self.clamp_v),
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter,
            anisotropy_clamp,
            ..Default::default()
        }
    }
}

/// `MaxAnisotropy` ini value -> wgpu `anisotropy_clamp`. XOpenGL
/// applies it only when non-zero, capped at the driver maximum
/// (CheckExtensions.cpp:300-317, SetTexture.cpp:159-160). wgpu takes
/// 1..=16 (1 = off) and caps at 16 itself. Fractional values truncate.
pub fn anisotropy_clamp(max_anisotropy: f32) -> u16 {
    if max_anisotropy >= 1.0 {
        max_anisotropy.min(16.0) as u16
    } else {
        1
    }
}

/// Real `UXOpenGLRenderDevice::SetTexture`'s own `UMult`/`VMult`
/// computation (`SetTexture.cpp:492-493`: `Tex.UMult = 1.f /
/// (Info.UScale * static_cast<FLOAT>(Info.USize))`, `Tex.VMult`
/// symmetrically), run fresh on every real bind, not cached --
/// `CachedTexture`'s own doc has the full real citation and why. A
/// separate, pure, standalone function (not a `TextureCache`/
/// `CachedTexture` method) specifically so a future real `queue_tile`
/// call can compute this directly from the real `FTextureInfo`'s own
/// `UScale`/`VScale`/`USize`/`VSize` at each draw's own real call
/// time, matching the real source's own per-bind freshness exactly,
/// independent of whatever `width`/`height` a `CachedTexture` happens
/// to have been uploaded with (`u_size`/`v_size` here are the same
/// real values, just not assumed to always be read from a live
/// `CachedTexture` lookup -- the real `FTextureInfo` is the real
/// source of truth both places).
pub fn texture_uv_mult(u_scale: f32, v_scale: f32, u_size: u32, v_size: u32) -> (f32, f32) {
    (
        1.0 / (u_scale * u_size as f32),
        1.0 / (v_scale * v_size as f32),
    )
}

/// Real `UXOpenGLRenderDevice::SetTexture`'s own `UPan`/`VPan`
/// computation (`SetTexture.cpp:488-489`: `Tex.UPan = Info.Pan.X +
/// PanBias * Info.UScale`, `Tex.VPan` symmetrically from `Info.Pan.Y`/
/// `Info.VScale`) -- confirmed at the same real "Set panning" comment
/// immediately before the real `UMult`/`VMult` computation
/// `texture_uv_mult`'s own doc already has the full citation for, run
/// fresh on every real bind, not cached, same real reasoning. A
/// separate function, not folded into `texture_uv_mult`: real
/// `DrawComplexSurface` callers (`SetTextureHelper`, `DrawComplex.cpp:
/// 21-42`) need both `UMult`/`VMult` *and* `UPan`/`VPan` together per
/// real texture layer (`DrawCallParams->DiffuseUV`/`LightMapUV`/
/// `FogMapUV`, each a real `vec4(UMult, VMult, UPan, VPan)`), but
/// `DrawTile`'s own real callers only ever need `UMult`/`VMult` --
/// keeping them separate avoids a real caller that only needs one
/// pair having to compute (or fake) the other. `pan_bias` is the real
/// `PanBias` parameter `SetTexture`'s own real callers pass as a
/// caller-chosen constant, not a per-texture field -- `0.0` for
/// diffuse/detail/macro/bump-map layers, `-0.5` for lightmap/fogmap
/// layers (confirmed from `DrawComplex.cpp`'s own five real
/// `SetTextureHelper` call sites, `:168,175,178,181,184`), a real
/// per-*layer*, not per-texture, constant this function's own callers
/// must supply, not derive.
pub fn texture_uv_pan(
    pan_x: f32,
    pan_y: f32,
    pan_bias: f32,
    u_scale: f32,
    v_scale: f32,
) -> (f32, f32) {
    (pan_x + pan_bias * u_scale, pan_y + pan_bias * v_scale)
}

/// Real `BindMap`'s own role (`SetTexture.cpp`'s `TMap<QWORD,
/// FCachedTexture> *BindMap`) -- GPU texture objects keyed by the
/// real `CacheID` (`fix_cache_id`'s own output), looked up before
/// deciding whether a new upload is needed. Scope today, explicitly:
/// single mip level only (mip 0 -- the real `FTextureInfo::Mips[]`
/// chain and its own mipmap generation/upload are not ported yet);
/// `upload_rgba8`/`upload_compressed` write into an existing entry's GPU
/// texture (`write_texture`) when width, height, format and mip count
/// all match, mirroring the real source's `PartialUpload` path
/// (`glTexSubImage2D` on an already-allocated texture). Any other
/// change allocates a new `wgpu::Texture`, since wgpu cannot resize or
/// re-format an existing texture in place.
pub struct TextureCache {
    entries: std::collections::HashMap<u64, CachedTexture>,
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
    /// scoped to this struct's own doc (single mip). `rgba8_bytes` must
    /// be exactly `width * height * 4`
    /// bytes, tightly packed (`FColor`'s own real `R,G,B,A` byte
    /// order, matching `Rgba8Unorm` directly -- `p8_to_rgba8`'s own
    /// doc already covers why no swizzle is needed for a P8-sourced
    /// buffer specifically). `srgb`: `UnpackSRGB`'s own real choice
    /// between `GL_RGBA8`/`GL_SRGB8_ALPHA8` -- `Rgba8Unorm` vs
    /// `Rgba8UnormSrgb` here, the direct wgpu equivalents. Inserts
    /// into this cache under `cache_id` and returns a reference to that
    /// entry. Reuses the existing GPU texture via `write_texture` when
    /// width, height, format and mip count all match, and reallocates
    /// otherwise. Does not take `UScale`/`VScale` --
    /// an earlier version did, and cached the resulting `UMult`/
    /// `VMult` on `CachedTexture` (caught on review, reverted):
    /// `CachedTexture`'s own doc has the real reason this is wrong
    /// (`SetTexture.cpp:492-493` recomputes them fresh on every real
    /// bind, not once at upload) -- `texture_uv_mult` is the real
    /// per-bind replacement, called separately by whatever future
    /// `queue_tile` wiring needs it, from the real `FTextureInfo`'s
    /// own fields at each draw's own time.
    ///
    /// # Panics
    /// If `rgba8_bytes.len() != width as usize * height as usize * 4`
    /// -- a caller bug (a real `FTextureInfo`'s own `USize`/`VSize`
    /// and its mip-0 data length are never inconsistent with each
    /// other), not a runtime condition to handle gracefully. This
    /// function is not `extern "C"` itself (no FFI call site calls it
    /// directly yet -- see this module's own header), so a panic here
    /// does not yet risk crossing the FFI boundary; whatever future
    /// FFI wrapper calls this must validate its own inputs before
    /// calling in, the same discipline `hp2-ffi`'s own `guard` already
    /// establishes elsewhere in this port.
    pub fn upload_rgba8(
        &mut self,
        created: &crate::CreatedDevice,
        cache_id: u64,
        width: u32,
        height: u32,
        rgba8_bytes: &[u8],
        options: TextureOptions,
    ) -> &CachedTexture {
        let TextureOptions { srgb, sampler_key } = options;
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
        let reuse = self.entries.get(&cache_id).is_some_and(|existing| {
            existing.texture.width() == width
                && existing.texture.height() == height
                && existing.texture.format() == format
                && existing.texture.mip_level_count() == 1
        });
        if reuse {
            let existing = self
                .entries
                .get(&cache_id)
                .expect("reuse was decided from this cache_id's entry above");
            write_rgba8_level(created, &existing.texture, width, height, rgba8_bytes);
            return existing;
        }
        let sampler_key = self.first_sampler_key(cache_id, sampler_key);
        let mut texture = create_gpu_texture(created, width, height, rgba8_bytes, srgb);
        texture.sampler_key = sampler_key;
        self.entries.insert(cache_id, texture);
        self.entries
            .get(&cache_id)
            .expect("just inserted this cache_id above")
    }

    /// Inserts a zero-initialised RGBA8 entry under `cache_id` with no
    /// initial write, replacing any existing texture and keeping its
    /// sampler key (`first_sampler_key`). Used for native-text atlas
    /// pages, which are then filled region by region with
    /// `write_rgba8_region`.
    pub fn insert_blank_rgba8(
        &mut self,
        created: &crate::CreatedDevice,
        cache_id: u64,
        width: u32,
        height: u32,
        sampler_key: SamplerKey,
    ) {
        let mut texture = create_blank_gpu_texture(created, width, height);
        texture.sampler_key = self.first_sampler_key(cache_id, sampler_key);
        self.entries.insert(cache_id, texture);
    }

    /// Writes `rgba8_bytes` (tightly packed, `width * height * 4` bytes)
    /// into the rectangle at `(x, y)` of size `(width, height)` in mip 0
    /// of the resident entry `cache_id`. Returns false when `cache_id` is
    /// not resident. The caller flushes queued draws first when they may
    /// sample the region.
    pub fn write_rgba8_region(
        &self,
        created: &crate::CreatedDevice,
        cache_id: u64,
        (x, y): (u32, u32),
        (width, height): (u32, u32),
        rgba8_bytes: &[u8],
    ) -> bool {
        assert_eq!(
            rgba8_bytes.len(),
            width as usize * height as usize * 4,
            "rgba8_bytes length must be exactly width * height * 4 bytes"
        );
        let Some(entry) = self.entries.get(&cache_id) else {
            return false;
        };
        assert!(
            x + width <= entry.width && y + height <= entry.height,
            "write_rgba8_region rectangle must lie inside the texture"
        );
        created.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &entry.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
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
        true
    }

    /// The key a (re)allocated entry keeps: the existing entry's, if
    /// any (`SamplerKey`'s own doc), else the caller's.
    fn first_sampler_key(&self, cache_id: u64, requested: SamplerKey) -> SamplerKey {
        self.entries
            .get(&cache_id)
            .map_or(requested, |existing| existing.sampler_key)
    }

    /// Real `TEXF_DXT1` upload, scoped to this struct's own doc the
    /// same way `upload_rgba8` is (full real mip chain here, not
    /// single-mip -- `create_compressed_gpu_texture`'s own doc has
    /// the full real reasoning for why this one format needs that).
    /// Returns `None` (inserting nothing) exactly when
    /// `create_compressed_gpu_texture` would -- `mips` empty, or the
    /// real device does not actually have `Features::
    /// TEXTURE_COMPRESSION_BC` granted -- that function's own doc has
    /// the full real `SupportsS3TC`-parity reasoning. Replaces any
    /// existing entry under `cache_id` only on success; an existing
    /// entry is left untouched on a `None` return, matching the real
    /// source's own `UnsupportedTexture` early-outs never touching
    /// `BindMap` either.
    /// Reuses the existing GPU texture via `write_texture` when width,
    /// height, format and mip count all match, and reallocates otherwise.
    pub fn upload_compressed(
        &mut self,
        created: &crate::CreatedDevice,
        cache_id: u64,
        width: u32,
        height: u32,
        mips: &[CompressedMipLevel<'_>],
        options: TextureOptions,
    ) -> Option<&CachedTexture> {
        let TextureOptions { srgb, sampler_key } = options;
        if mips.is_empty() {
            return None;
        }
        if !created
            .device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        {
            return None;
        }
        let format = if srgb {
            wgpu::TextureFormat::Bc1RgbaUnormSrgb
        } else {
            wgpu::TextureFormat::Bc1RgbaUnorm
        };
        let reuse = self.entries.get(&cache_id).is_some_and(|existing| {
            existing.texture.width() == width
                && existing.texture.height() == height
                && existing.texture.format() == format
                && existing.texture.mip_level_count() == mips.len() as u32
        });
        if reuse {
            let existing = self
                .entries
                .get(&cache_id)
                .expect("reuse was decided from this cache_id's entry above");
            write_bc1_levels(created, &existing.texture, mips);
            return Some(existing);
        }
        let sampler_key = self.first_sampler_key(cache_id, sampler_key);
        let mut texture = create_compressed_gpu_texture(created, width, height, mips, srgb)?;
        texture.sampler_key = sampler_key;
        self.entries.insert(cache_id, texture);
        Some(
            self.entries
                .get(&cache_id)
                .expect("just inserted this cache_id above"),
        )
    }
}

/// Per-upload options for `TextureCache::upload_rgba8`/
/// `upload_compressed`. `srgb`: `UnpackSRGB`'s `GL_RGBA8` vs
/// `GL_SRGB8_ALPHA8` choice. `sampler_key`: used only when `cache_id`
/// has no entry yet (`SamplerKey`'s own doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextureOptions {
    pub srgb: bool,
    pub sampler_key: SamplerKey,
}

/// The real GPU-texture-object-creation core of `TextureCache::
/// upload_rgba8` (that method's own doc has the full real contract
/// and panics), factored out as a standalone function so a caller
/// that needs a real, valid, but *not real-engine-sourced* GPU
/// texture -- `RenderLoop::dummy_texture`'s own doc has the full real
/// reasoning for why one is needed -- can build one without going
/// through `TextureCache`'s own `CacheID`-keyed `HashMap` at all (an
/// earlier version of that call site used a throwaway `cache_id: 0`
/// insert-then-remove through `TextureCache::upload_rgba8` itself
/// instead -- caught on review before landing: a real, if unlikely,
/// collision risk against a genuine real `CacheID` of exactly `0`,
/// and pointless indirection through a `HashMap` entry that would
/// have been removed again immediately anyway).
pub fn create_gpu_texture(
    created: &crate::CreatedDevice,
    width: u32,
    height: u32,
    rgba8_bytes: &[u8],
    srgb: bool,
) -> CachedTexture {
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
    write_rgba8_level(created, &texture, width, height, rgba8_bytes);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    CachedTexture {
        texture,
        view,
        width,
        height,
        sampler_key: SamplerKey::default(),
    }
}

/// Writes `rgba8_bytes` into mip 0 of an existing RGBA8 texture:
/// `create_gpu_texture`'s own write, factored out so
/// `TextureCache::upload_rgba8` can reuse it when width, height, format
/// and mip count match.
fn write_rgba8_level(
    created: &crate::CreatedDevice,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    rgba8_bytes: &[u8],
) {
    created.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
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
}

/// Zero-initialised RGBA8 texture with no initial write: the descriptor
/// `create_gpu_texture` uses, minus its `write_rgba8_level` call. wgpu
/// zero-initialises texture contents on creation.
fn create_blank_gpu_texture(
    created: &crate::CreatedDevice,
    width: u32,
    height: u32,
) -> CachedTexture {
    let texture = created.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hp2-wgpu blank texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    CachedTexture {
        texture,
        view,
        width,
        height,
        sampler_key: SamplerKey::default(),
    }
}

/// One real pre-baked mip level's own data for a compressed-format
/// upload. `width`/`height` are that mip's own real `FMipmapBase::
/// USize`/`VSize` -- not derived by halving the base level's own
/// dimensions: UE1's own mip chain already carries these explicit
/// per-level (and DXT1's own 4x4-block rounding does not always halve
/// exactly), so trusting the real explicit value matches this crate's
/// own established "never guess, use the real explicit value"
/// discipline. `data` is that mip's own real pre-baked block-
/// compressed bytes (`FMipmapBase::DataPtr`, already exactly
/// `texture_bytes(Dxt1, width, height)` bytes long -- the real engine
/// performs no runtime re-encoding for `TEXF_DXT1`, it just re-
/// uploads each mip's own asset-baked bytes directly, confirmed by
/// reading `SetTexture.cpp`'s own `case TEXF_DXT1: CompImageSize =
/// HP2TextureBytes(...); ImgSrc = Mip->DataPtr;`, not assumed from
/// the general compressed-format shape alone).
pub struct CompressedMipLevel<'a> {
    pub width: u32,
    pub height: u32,
    pub data: &'a [u8],
}

/// Real `TEXF_DXT1` upload (`SetTexture.cpp`'s own `case TEXF_DXT1`
/// branch -- `InternalFormat = GL_COMPRESSED_(SRGB_ALPHA)?_S3TC_DXT1_
/// EXT`, read directly). `mips` is the real engine's own full
/// `Info.Mips[0..Info.NumMips)` chain for this one texture, not just
/// the base level -- unlike this module's own `create_gpu_texture`/
/// `TextureCache::upload_rgba8` (mip-0-only, a real, separately-
/// scoped limitation for the uncompressed formats those cover), a
/// compressed BC1 texture has no wgpu-side mip-generation option at
/// all: `glGenerateMipmap` has no wgpu equivalent to begin with (this
/// module's own header doc already covers this), and BC-compressed
/// textures additionally cannot be render or storage targets in wgpu,
/// so even a custom downsample-blit pass is not an option the way it
/// would be for an uncompressed format. The asset's own pre-baked
/// chain is the only real source of additional mip levels for this
/// format, so uploading it in full here is not an enhancement, it is
/// the only way this format can have more than one mip level at all.
///
/// Returns `None` (not a panic, and not a texture) if `mips` is empty,
/// or if `created.device.features()` does not actually include
/// `Features::TEXTURE_COMPRESSION_BC` -- the real `SupportsS3TC`-
/// gated graceful-degradation case (`SetTexture.cpp:233: if
/// (!SupportsS3TC && IsHP2CompressedTexture(Info.Format))
/// UnsupportedTexture = true;`, not a hard failure in the real source
/// either), checked here again at upload time against what the
/// device actually negotiated, not just what `try_create_device_once`
/// requested (an adapter can still report the feature as unsupported
/// even after a best-effort request; its own doc has the full real
/// reasoning for requesting rather than requiring it).
///
/// # Panics
/// If any one `mips[i].data.len()` is not exactly `texture_bytes
/// (TextureFormat::Dxt1, mips[i].width, mips[i].height)` bytes -- a
/// caller bug (a real `FMipmapBase`'s own `USize`/`VSize` and its
/// pre-baked compressed data length are never inconsistent with each
/// other), matching `TextureCache::upload_rgba8`'s own established
/// panic-is-a-caller-bug contract for the same reason.
pub fn create_compressed_gpu_texture(
    created: &crate::CreatedDevice,
    width: u32,
    height: u32,
    mips: &[CompressedMipLevel<'_>],
    srgb: bool,
) -> Option<CachedTexture> {
    if mips.is_empty() {
        return None;
    }
    if !created
        .device
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    {
        return None;
    }
    let format = if srgb {
        wgpu::TextureFormat::Bc1RgbaUnormSrgb
    } else {
        wgpu::TextureFormat::Bc1RgbaUnorm
    };
    let texture = created.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hp2-wgpu cached compressed texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // COPY_SRC: same real debug-readback-capability reasoning as
        // create_gpu_texture's own doc -- BC-compressed textures can
        // still be a real copy *source* in wgpu, the restriction that
        // actually applies to this format is RENDER_ATTACHMENT/
        // STORAGE_BINDING, neither requested here.
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    write_bc1_levels(created, &texture, mips);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Some(CachedTexture {
        texture,
        view,
        width,
        height,
        sampler_key: SamplerKey::default(),
    })
}

/// Writes each pre-baked BC1 mip level into the matching `mip_level` of
/// an existing texture: `create_compressed_gpu_texture`'s own per-level
/// upload, factored out so `TextureCache::upload_compressed` can reuse
/// it when width, height, format and mip count match. Panics as
/// documented on `create_compressed_gpu_texture`.
fn write_bc1_levels(
    created: &crate::CreatedDevice,
    texture: &wgpu::Texture,
    mips: &[CompressedMipLevel<'_>],
) {
    for (level, mip) in mips.iter().enumerate() {
        let expected_len = texture_bytes(TextureFormat::Dxt1, mip.width as i32, mip.height as i32);
        assert_eq!(
            mip.data.len(),
            expected_len as usize,
            "mip level {level} (width={}, height={}) data length must be exactly the real BC1 \
             block size",
            mip.width,
            mip.height,
        );
        // Real wgpu block-row semantics (wgpu-types' own TexelCopy
        // BufferLayout doc table, confirmed directly before writing
        // this, not assumed from the pixel-format case): bytes_per_row
        // and rows_per_image are both expressed in *blocks*, not
        // pixels, for a compressed format -- the same
        // Max(1,(size+3)/4) rounding texture_bytes already applies to
        // the total size above, decomposed here into its own per-row
        // and per-column halves.
        let blocks_wide = 1.max((mip.width as i32 + 3) / 4);
        let blocks_high = 1.max((mip.height as i32 + 3) / 4);
        created.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((blocks_wide * 8) as u32),
                rows_per_image: Some(blocks_high as u32),
            },
            // Real wgpu-core validation (transfer.rs:579-589, read
            // directly before trusting this, not assumed): copy_size
            // for a compressed format must itself be a whole multiple
            // of the block size, checked against the mip level's own
            // *physical* (block-rounded) size, not its logical one --
            // a bare 2x2 or 1x1 tail mip's own logical size is
            // neither, so passing mip.width/mip.height directly here
            // would fail validation outright for any non-block-
            // aligned mip (caught on review, confirmed by actually
            // reproducing the real wgpu validation error before
            // fixing, not just reasoning about it -- see this
            // function's own new multi-mip test). The texture's own
            // logical size (mip.width/mip.height) is still what every
            // other real caller of this crate (TextureCache::get's
            // own consumers, sampling UV math, etc.) continues to see
            // and reason about -- only the raw copy-extent argument to
            // write_texture itself needs the physical, block-rounded
            // value.
            wgpu::Extent3d {
                width: (blocks_wide * 4) as u32,
                height: (blocks_high * 4) as u32,
                depth_or_array_layers: 1,
            },
        );
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
    fn rgb8_to_rgba8_expands_each_texel_and_forces_opaque_alpha() {
        // Red, then green, tightly packed 3 bytes each -- no padding
        // between texels (HP2TextureBytes' own real width*height*3
        // formula, no slack to leave room for).
        let rgb = [255u8, 0, 0, 0, 255, 0];
        let rgba = rgb8_to_rgba8(&rgb);
        assert_eq!(rgba, vec![255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn rgb8_to_rgba8_dropping_the_alpha_byte_is_caught() {
        // Negative control: an earlier, wrong version of this
        // function might plausibly have forgotten to append the
        // alpha byte at all (just passed rgb_bytes through
        // unchanged) -- confirms the real test above actually
        // distinguishes "4 bytes with alpha=255 appended" from "3
        // bytes, no alpha" rather than passing either way.
        let rgb = [255u8, 0, 0];
        let wrong = rgb.to_vec();
        let right = rgb8_to_rgba8(&rgb);
        assert_ne!(
            wrong, right,
            "a 3-byte passthrough must not equal the real 4-byte expansion"
        );
        assert_eq!(right, vec![255, 0, 0, 255]);
    }

    #[test]
    fn rgb16_to_rgba8_decodes_the_real_gl_unsigned_short_5_6_5_rev_layout() {
        // Pure red: R=0b11111 (5 bits, max) in bits 4..0, G=0, B=0.
        // Little-endian u16 0x001F -> bytes [0x1F, 0x00].
        let red = rgb16_to_rgba8(&[0x1F, 0x00]);
        assert_eq!(red, vec![255, 0, 0, 255]);
        // Pure green: G=0b111111 (6 bits, max) in bits 10..5 ->
        // 0b0000_0111_1110_0000 = 0x07E0, little-endian [0xE0, 0x07].
        let green = rgb16_to_rgba8(&[0xE0, 0x07]);
        assert_eq!(green, vec![0, 255, 0, 255]);
        // Pure blue: B=0b11111 (5 bits, max) in bits 15..11 ->
        // 0b1111_1000_0000_0000 = 0xF800, little-endian [0x00, 0xF8].
        let blue = rgb16_to_rgba8(&[0x00, 0xF8]);
        assert_eq!(blue, vec![0, 0, 255, 255]);
        // Black and white: all-zero and all-one bits respectively,
        // same regardless of channel order -- a weaker check on its
        // own (passes even with a channel-order bug), kept alongside
        // the three single-channel cases above rather than instead of
        // them.
        assert_eq!(rgb16_to_rgba8(&[0x00, 0x00]), vec![0, 0, 0, 255]);
        assert_eq!(rgb16_to_rgba8(&[0xFF, 0xFF]), vec![255, 255, 255, 255]);
    }

    #[test]
    fn rgb16_to_rgba8_using_the_non_rev_layout_is_caught() {
        // Negative control: the non-REV GL_UNSIGNED_SHORT_5_6_5 bit
        // layout (R in bits 15..11, G in 10..5, B in 4..0) is the
        // *opposite* of the real _REV one this function implements --
        // an easy direction to get backwards from the suffix's name
        // alone (this function's own doc already covers why). Confirms
        // a pure-red _REV-encoded pixel, decoded under the wrong
        // (non-REV) layout, would produce the exact wrong color this
        // function must NOT produce: pure blue, not red.
        let red_rev_encoded = [0x1Fu8, 0x00];
        let value = u16::from_le_bytes(red_rev_encoded);
        let wrong_b5 = (value & 0x1F) as u8; // non-REV: B is the LOW 5 bits
        let wrong_g6 = ((value >> 5) & 0x3F) as u8;
        let wrong_r5 = ((value >> 11) & 0x1F) as u8; // non-REV: R is the HIGH 5 bits
        let wrong_r8 = (wrong_r5 << 3) | (wrong_r5 >> 2);
        let wrong_g8 = (wrong_g6 << 2) | (wrong_g6 >> 4);
        let wrong_b8 = (wrong_b5 << 3) | (wrong_b5 >> 2);
        let wrong_decode = [wrong_r8, wrong_g8, wrong_b8, 255];
        let real_decode = rgb16_to_rgba8(&red_rev_encoded);
        assert_ne!(
            wrong_decode.to_vec(),
            real_decode,
            "the real _REV decode must not match the non-REV (opposite-order) decode"
        );
        assert_eq!(real_decode, vec![255, 0, 0, 255]);
        assert_eq!(wrong_decode, [0, 0, 255, 255]);
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
    fn texture_uv_mult_matches_the_real_set_texture_formula() {
        // Real SetTexture.cpp:492-493 formula, hand-computed
        // independently: UMult = 1/(UScale*USize) = 1/(2.0*2) = 0.25,
        // VMult = 1/(VScale*VSize) = 1/(0.5*1) = 2.0. Distinct,
        // non-1.0 u_scale/v_scale values so this cannot pass by
        // accident if the scale factor were ever dropped and the
        // formula silently fell back to plain 1/size.
        let (u_mult, v_mult) = texture_uv_mult(2.0, 0.5, 2, 1);
        assert_eq!(u_mult, 0.25);
        assert_eq!(v_mult, 2.0);
    }

    #[test]
    fn texture_uv_mult_with_unit_scale_is_plain_reciprocal_size() {
        let (u_mult, v_mult) = texture_uv_mult(1.0, 1.0, 4, 8);
        assert_eq!(u_mult, 0.25);
        assert_eq!(v_mult, 0.125);
    }

    #[test]
    fn texture_uv_pan_matches_the_real_set_texture_formula() {
        // pan_x=10.0, pan_y=20.0, pan_bias=-0.5 (the real lightmap/
        // fogmap layer constant), u_scale=2.0, v_scale=4.0. Hand-
        // verified: u_pan = 10.0 + (-0.5)*2.0 = 9.0, v_pan = 20.0 +
        // (-0.5)*4.0 = 18.0.
        let (u_pan, v_pan) = texture_uv_pan(10.0, 20.0, -0.5, 2.0, 4.0);
        assert_eq!(u_pan, 9.0);
        assert_eq!(v_pan, 18.0);
    }

    #[test]
    fn texture_uv_pan_with_zero_bias_is_plain_pan() {
        // pan_bias=0.0 (the real diffuse/detail/macro/bump-map layer
        // constant) -- the bias term vanishes regardless of scale,
        // leaving the raw pan values unchanged.
        let (u_pan, v_pan) = texture_uv_pan(10.0, 20.0, 0.0, 2.0, 4.0);
        assert_eq!(u_pan, 10.0);
        assert_eq!(v_pan, 20.0);
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
        let cached = cache.upload_rgba8(
            &created,
            42,
            2,
            1,
            &bytes,
            crate::texture::TextureOptions::default(),
        );
        assert_eq!(cached.width, 2);
        assert_eq!(cached.height, 1);

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

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn upload_compressed_with_empty_mips_returns_none_and_leaves_an_existing_entry_untouched() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let mut cache = TextureCache::new();

        // Seed cache_id 7 with a real (uncompressed) entry first, so
        // this test can tell "upload_compressed did nothing" apart
        // from "the cache happened to be empty anyway".
        cache.upload_rgba8(
            &created,
            7,
            1,
            1,
            &[1, 2, 3, 4],
            crate::texture::TextureOptions::default(),
        );

        let none_result = cache.upload_compressed(
            &created,
            7,
            4,
            4,
            &[],
            crate::texture::TextureOptions::default(),
        );
        assert!(
            none_result.is_none(),
            "expected upload_compressed to return None for an empty mips slice"
        );
        let still_there = cache
            .get(7)
            .expect("expected cache_id 7's existing entry to survive a None-returning call");
        assert_eq!(
            still_there.width, 1,
            "expected the existing entry's own real fields to be completely untouched, not \
             partially overwritten"
        );
        println!(
            "TextureCache::upload_compressed confirmed empirically: an empty mips slice returns \
             None and leaves an existing cache_id entry completely untouched, on adapter {:?} \
             backend {}",
            created.adapter_name,
            crate::backend_name(created.backend)
        );
    }

    // The real deterministic failure a single-mip test cannot reach:
    // wgpu-core's own copy-extent validation (transfer.rs:579-589,
    // read directly before writing this test, not assumed) rejects
    // any compressed-format copy_size whose width/height is not
    // itself a whole multiple of the block size (4 for BC1) -- a bare
    // 2x2 or 1x1 tail mip's own *logical* size is neither. Caught on
    // review before this ever shipped uncaught: an earlier version of
    // create_compressed_gpu_texture passed each mip's own logical
    // width/height directly as the write_texture copy extent, which
    // this exact 4x4->2x2->1x1 chain would have failed validation on
    // (reproduced empirically before fixing, not just reasoned about
    // -- see this test's own real three-level chain below, the
    // smallest real case that actually has a non-block-aligned tail).
    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn upload_compressed_round_trips_a_real_4x4_to_2x2_to_1x1_mip_chain() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let mut cache = TextureCache::new();

        // Three real pre-baked mip levels, one real BC1 block each
        // (every level here is <=4x4, so every level is exactly one
        // block, same real Max(1,(size+3)/4) rounding texture_bytes
        // already covers) -- a different solid color per level so a
        // level reading back some *other* level's color (a real,
        // plausible bug shape for an off-by-one mip_level index, not
        // just a hypothetical) is distinguishable from the correct
        // result, not just "did not crash".
        let red_block: [u8; 8] = [0x00, 0xF8, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00];
        let green_block: [u8; 8] = [0xE0, 0x07, 0xE0, 0x07, 0x00, 0x00, 0x00, 0x00];
        let blue_block: [u8; 8] = [0x1F, 0x00, 0x1F, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mips = [
            CompressedMipLevel {
                width: 4,
                height: 4,
                data: &red_block,
            },
            CompressedMipLevel {
                width: 2,
                height: 2,
                data: &green_block,
            },
            CompressedMipLevel {
                width: 1,
                height: 1,
                data: &blue_block,
            },
        ];
        let Some(cached) = cache.upload_compressed(
            &created,
            99,
            4,
            4,
            &mips,
            crate::texture::TextureOptions::default(),
        ) else {
            println!(
                "SKIPPED: adapter {:?} backend {} does not actually grant \
                 Features::TEXTURE_COMPRESSION_BC, nothing to assert here on this adapter",
                created.adapter_name,
                crate::backend_name(created.backend)
            );
            return;
        };
        let texture = cached.texture.clone();

        // Real proof each real mip level landed at its own real
        // mip_level index with its own real bytes, not just that
        // three write_texture calls returned without erroring: read
        // each level back individually (one real BC1 block == 8 bytes
        // regardless of which of the three logical sizes it is, since
        // every level here rounds to exactly one block).
        for (level, expected_block) in [
            (0u32, [0xFFu8, 0, 0, 255]),
            (1u32, [0, 255, 0, 255]),
            (2u32, [0, 0, 255, 255]),
        ] {
            let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("compressed mip chain readback buffer"),
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = created
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            // Real wgpu-core rule again (transfer.rs:579-589): the
            // *readback* copy extent needs the same block-rounded
            // physical size the fixed upload path now uses, not each
            // level's own raw logical width/height -- every level
            // here is a single block, so that physical size is always
            // (4, 4) regardless of which of the three logical sizes
            // is being read.
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback_buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        // Real wgpu rule for copy_texture_to_buffer
                        // specifically (unlike write_texture, which
                        // this struct's own create_compressed_gpu_
                        // texture uses and has no such requirement):
                        // bytes_per_row must itself be a multiple of
                        // COPY_BYTES_PER_ROW_ALIGNMENT (256) -- caught
                        // empirically (a real wgpu validation panic on
                        // this test's own first run, "Bytes per row
                        // does not respect COPY_BYTES_PER_ROW_
                        // ALIGNMENT"), not assumed from the upload
                        // side's own different rule.
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 4,
                    height: 4,
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
            let readback_block: [u8; 8] = mapped[0..8].try_into().unwrap();
            let expected_block_bytes: [u8; 8] = match level {
                0 => red_block,
                1 => green_block,
                _ => blue_block,
            };
            assert_eq!(
                readback_block, expected_block_bytes,
                "expected mip level {level}'s own real raw BC1 block bytes to round-trip \
                 unchanged, decoded color would be {expected_block:?}"
            );
        }
        println!(
            "TextureCache::upload_compressed confirmed empirically to round-trip a real \
             4x4->2x2->1x1 BC1 mip chain (the real non-block-aligned tail-mip case) through a \
             real GPU texture on adapter {:?} backend {}",
            created.adapter_name,
            crate::backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn upload_rgba8_reuses_the_existing_texture_when_shape_matches() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let mut cache = TextureCache::new();
        let red: [u8; 8] = [255, 0, 0, 255, 255, 0, 0, 255];
        cache.upload_rgba8(
            &created,
            5,
            2,
            1,
            &red,
            crate::texture::TextureOptions::default(),
        );
        let before = cache
            .get(5)
            .expect("expected the first upload to be cached under cache_id 5")
            .texture
            .clone();

        let blue: [u8; 8] = [0, 0, 255, 255, 0, 0, 255, 255];
        cache.upload_rgba8(
            &created,
            5,
            2,
            1,
            &blue,
            crate::texture::TextureOptions::default(),
        );
        assert_eq!(
            before,
            cache
                .get(5)
                .expect("expected cache_id 5 to still be cached after re-upload")
                .texture,
            "expected a matching re-upload to write into the existing texture, not reallocate"
        );

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("texture cache reuse readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let cached_texture = cache
            .get(5)
            .expect("expected cache_id 5 to still be cached after re-upload")
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
            &mapped[0..8],
            &[0u8, 0, 255, 255, 0, 0, 255, 255],
            "expected both texels to read back blue after the reuse upload"
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn upload_rgba8_reallocates_when_size_changes() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let mut cache = TextureCache::new();
        let two_by_one: [u8; 8] = [255, 0, 0, 255, 0, 255, 0, 255];
        cache.upload_rgba8(
            &created,
            6,
            2,
            1,
            &two_by_one,
            crate::texture::TextureOptions::default(),
        );
        let first = cache
            .get(6)
            .expect("expected the first upload to be cached under cache_id 6")
            .texture
            .clone();

        let four_by_one: [u8; 16] = [
            0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
        ];
        cache.upload_rgba8(
            &created,
            6,
            4,
            1,
            &four_by_one,
            crate::texture::TextureOptions::default(),
        );
        let cached = cache
            .get(6)
            .expect("expected cache_id 6 to still be cached after re-upload");
        assert_eq!(
            cached.width, 4,
            "expected the cache entry to report the new width"
        );
        assert_eq!(
            cached.texture.width(),
            4,
            "expected the GPU texture itself to be the new width"
        );
        assert_ne!(
            first, cached.texture,
            "expected a size change to allocate a new texture, not reuse the old one"
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn upload_compressed_reuses_the_existing_texture_when_shape_matches() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let mut cache = TextureCache::new();
        let red_block: [u8; 8] = [0x00, 0xF8, 0x00, 0xF8, 0x00, 0x00, 0x00, 0x00];
        let green_block: [u8; 8] = [0xE0, 0x07, 0xE0, 0x07, 0x00, 0x00, 0x00, 0x00];

        let red_mip = [CompressedMipLevel {
            width: 4,
            height: 4,
            data: &red_block,
        }];
        if cache
            .upload_compressed(
                &created,
                7,
                4,
                4,
                &red_mip,
                crate::texture::TextureOptions::default(),
            )
            .is_none()
        {
            println!(
                "SKIPPED: adapter {:?} backend {} does not actually grant \
                 Features::TEXTURE_COMPRESSION_BC, nothing to assert here on this adapter",
                created.adapter_name,
                crate::backend_name(created.backend)
            );
            return;
        }
        let before = cache
            .get(7)
            .expect("expected the first upload to be cached under cache_id 7")
            .texture
            .clone();

        let green_mip = [CompressedMipLevel {
            width: 4,
            height: 4,
            data: &green_block,
        }];
        cache
            .upload_compressed(
                &created,
                7,
                4,
                4,
                &green_mip,
                crate::texture::TextureOptions::default(),
            )
            .expect("expected the second upload to succeed, since the first one did");
        assert_eq!(
            before,
            cache
                .get(7)
                .expect("expected cache_id 7 to still be cached after re-upload")
                .texture,
            "expected a matching re-upload to write into the existing texture, not reallocate"
        );

        let readback_buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compressed reuse readback buffer"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let cached_texture = cache
            .get(7)
            .expect("expected cache_id 7 to still be cached after re-upload")
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
                width: 4,
                height: 4,
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
        let readback_block: [u8; 8] = mapped[0..8].try_into().unwrap();
        assert_eq!(
            readback_block, green_block,
            "expected mip 0 to read back the green block after the reuse upload"
        );
    }

    #[test]
    fn anisotropy_clamp_maps_ini_values_to_wgpu_range() {
        // Values below 1 mean off, as XOpenGL applies MaxAnisotropy only when non-zero.
        assert_eq!(anisotropy_clamp(0.0), 1);
        assert_eq!(anisotropy_clamp(0.5), 1);
        assert_eq!(anisotropy_clamp(1.0), 1);
        // Fractional values truncate.
        assert_eq!(anisotropy_clamp(4.7), 4);
        assert_eq!(anisotropy_clamp(16.0), 16);
        // wgpu caps at 16 itself.
        assert_eq!(anisotropy_clamp(64.0), 16);
        // NaN and negatives are off.
        assert_eq!(anisotropy_clamp(f32::NAN), 1);
        assert_eq!(anisotropy_clamp(-3.0), 1);
    }

    #[test]
    fn sampler_key_descriptor_matches_set_sampler() {
        let default = SamplerKey::default().descriptor(8);
        assert_eq!(default.address_mode_u, wgpu::AddressMode::Repeat);
        assert_eq!(default.address_mode_v, wgpu::AddressMode::Repeat);
        assert_eq!(default.mag_filter, wgpu::FilterMode::Linear);
        assert_eq!(default.min_filter, wgpu::FilterMode::Linear);
        assert_eq!(default.mipmap_filter, wgpu::MipmapFilterMode::Linear);
        assert_eq!(default.anisotropy_clamp, 8);

        let clamp_u = SamplerKey::from_bits(SamplerKey::CLAMP_U).descriptor(1);
        assert_eq!(clamp_u.address_mode_u, wgpu::AddressMode::ClampToEdge);
        assert_eq!(clamp_u.address_mode_v, wgpu::AddressMode::Repeat);

        let clamp_v = SamplerKey::from_bits(SamplerKey::CLAMP_V).descriptor(1);
        assert_eq!(clamp_v.address_mode_u, wgpu::AddressMode::Repeat);
        assert_eq!(clamp_v.address_mode_v, wgpu::AddressMode::ClampToEdge);

        // NO_SMOOTH forces nearest filtering and no anisotropy, whatever the ini asked for.
        let no_smooth = SamplerKey::from_bits(SamplerKey::NO_SMOOTH).descriptor(16);
        assert_eq!(no_smooth.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(no_smooth.min_filter, wgpu::FilterMode::Nearest);
        assert_eq!(no_smooth.mipmap_filter, wgpu::MipmapFilterMode::Nearest);
        assert_eq!(no_smooth.anisotropy_clamp, 1);

        // Unknown bits are ignored.
        assert_eq!(SamplerKey::from_bits(0xFFFF_FFF8), SamplerKey::default());
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn sampler_key_is_fixed_by_the_first_upload() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let mut cache = TextureCache::new();
        let first_key = SamplerKey {
            clamp_u: true,
            clamp_v: false,
            no_smooth: true,
        };
        cache.upload_rgba8(
            &created,
            3,
            1,
            1,
            &[1, 2, 3, 4],
            TextureOptions {
                srgb: false,
                sampler_key: first_key,
            },
        );
        // Reallocating re-upload under the same cache_id, with a different key requested:
        // the texture keeps the first upload's key.
        let reallocated = cache.upload_rgba8(&created, 3, 2, 1, &[0; 8], TextureOptions::default());
        assert_eq!(reallocated.sampler_key, first_key);
        assert_eq!(reallocated.width, 2);
    }
}
