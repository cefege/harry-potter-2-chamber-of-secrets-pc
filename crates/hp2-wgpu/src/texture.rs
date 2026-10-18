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
//! through this same path). Not yet ported: `TEXF_DXT1`/BC upload and
//! the still-undecided mip strategy (both this crate's own open todo
//! item; `texture_bytes` already has the real per-format byte-size
//! formula ready for whichever strategy lands), and `TEXF_RGB8`/
//! `TEXF_RGB16`/`TEXF_RGBA7` conversion (currently rejected cleanly by
//! `hp2_rs_wgpu_upload_texture`, not silently mis-uploaded -- see that
//! function's own doc, including the real soundness bug an earlier
//! version of its rejection path had and the fix that closed it).
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
    /// reference to the new entry. Does not take `UScale`/`VScale` --
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
        srgb: bool,
    ) -> &CachedTexture {
        self.entries.insert(
            cache_id,
            create_gpu_texture(created, width, height, rgba8_bytes, srgb),
        );
        self.entries
            .get(&cache_id)
            .expect("just inserted this cache_id above")
    }
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
    CachedTexture {
        texture,
        view,
        width,
        height,
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
        let cached = cache.upload_rgba8(&created, 42, 2, 1, &bytes, false);
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
}
