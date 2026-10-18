//! Native text on the wgpu device: the `DrawLayout` and `DrawAtlasRect`
//! port of `ThirdParty/XOpenGLDrv/Src/NativeText.cpp` (`DrawLayout`
//! L396-491, `DrawAtlasRect` L700-756), over the renderer-neutral atlas
//! in `text_atlas.rs`.
//!
//! Atlas page `p` lives in the texture cache under
//! `NATIVE_TEXT_PAGE_CACHE_ID_BASE | p`. Every atlas rectangle is queued
//! as one tile through `RenderLoop::queue_tile`, which is this crate's
//! `DrawNativeTextTile` (`ThirdParty/XOpenGLDrv/Src/DrawTile.cpp` L58-109).
//!
//! Provider contract: the caller passes only drawable placements
//! (`bDrawable` in C++). `draw_left`/`draw_top` are the outputs of
//! `GlyphDrawOffset` (`NativeTextShared.cpp` L968-971), and
//! `raster_width`/`raster_height` are the `Width`/`Height` of
//! `CalculateRasterRectangle` (`NativeTextShared.cpp` L897-925).

use crate::text_atlas::{self, AtlasSlot, GlyphKey, PreparedSlot, RasterizedGlyph};
use crate::texture::SamplerKey;
use crate::{CreatedDevice, RenderLoop, TileDrawRequest};

/// Texture-cache id base for atlas pages: page `p` is `BASE | p`.
pub const NATIVE_TEXT_PAGE_CACHE_ID_BASE: u64 = 0xFFFF_0000_0000_0000;

/// Texture-cache id of atlas page `page`.
pub fn native_text_page_cache_id(page: u32) -> u64 {
    NATIVE_TEXT_PAGE_CACHE_ID_BASE | u64::from(page)
}

/// `UnObj.h` poly flags used by `DrawAtlasRect` (L741).
const PF_MASKED: u32 = 0x0000_0002;
const PF_TRANSLUCENT: u32 = 0x0000_0004;
const PF_TWO_SIDED: u32 = 0x0000_0100;
const PF_HIGHLIGHTED: u32 = 0x1000_0000;

/// One drawable glyph placement (`FGlyphPlacement` with `bDrawable`).
pub struct NativeTextGlyph {
    /// Glyph-cache key (`FGlyphPlacement::Key`).
    pub key: GlyphKey,
    /// `GlyphDrawOffset` left (`NativeTextShared.cpp` L968-971).
    pub draw_left: f64,
    /// `GlyphDrawOffset` top (`NativeTextShared.cpp` L968-971).
    pub draw_top: f64,
    /// `CalculateRasterRectangle` width (`NativeTextShared.cpp` L897-925).
    pub raster_width: u32,
    /// `CalculateRasterRectangle` height (`NativeTextShared.cpp` L897-925).
    pub raster_height: u32,
}

/// One underline (`FUnderline`), in layout space.
pub struct NativeTextUnderline {
    pub start: f64,
    pub end: f64,
    pub baseline: f64,
    pub position: f64,
    pub thickness: f64,
}

/// The `FCanvasTextLayout` fields `DrawAtlasRect` reads (`NativeText.cpp`
/// L700-756). `color` is `FPlane` RGBA; `W` is forced to 1 at draw time
/// (L742-743), so the caller's alpha is ignored.
pub struct NativeTextLayout {
    /// `Layout.StartX` (`FCanvasTextLayout`, integer).
    pub start_x: i32,
    /// `Layout.StartY` (`FCanvasTextLayout`, integer).
    pub start_y: i32,
    /// `Layout.OriginX`.
    pub origin_x: f32,
    /// `Layout.OriginY`.
    pub origin_y: f32,
    /// `Layout.ClipX`, used only when `clip` is set.
    pub clip_x: f32,
    /// `Layout.ClipY`, used only when `clip` is set.
    pub clip_y: f32,
    /// `Layout.bClip`.
    pub clip: bool,
    /// `Layout.PolyFlags` (`PF_Masked` is cleared at L741).
    pub poly_flags: u32,
    /// `Layout.Color` RGBA.
    pub color: [f32; 4],
}

/// `DrawAtlasRect`'s rectangle inputs: the offsets from the layout's start
/// point, plus the requested size (`NativeText.cpp` L700).
#[derive(Clone, Copy)]
struct AtlasRect {
    offset_x: f32,
    offset_y: f32,
    width: f32,
    height: f32,
}

/// A prepared glyph with its draw offset already applied (`GlyphDrawOffset`
/// results, cast to `FLOAT` as at `NativeText.cpp` L446-449).
#[derive(Clone, Copy)]
struct PlacedGlyph {
    left: f32,
    top: f32,
    slot: AtlasSlot,
}

/// `Hp2NativeText::SafeFloat` (`NativeTextShared.cpp` L727-733): finite and
/// inside the `INT` range.
fn safe_float(value: f32) -> bool {
    let value = f64::from(value);
    value.is_finite() && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX)
}

/// `CalculateRasterRectangle`'s size rule (`NativeTextShared.cpp` L897-925):
/// the page must be larger than `2 * Padding`, and the raster rectangle
/// must be non-empty and at most `PageSize - 2 * Padding` on each axis.
fn native_text_raster_rect_fits(width: u32, height: u32, page_size: u32) -> bool {
    if page_size <= 2 * text_atlas::PADDING {
        return false;
    }
    let max = page_size - 2 * text_atlas::PADDING;
    (1..=max).contains(&width) && (1..=max).contains(&height)
}

impl RenderLoop {
    /// `DrawLayout` (`NativeText.cpp` L396-491). `rasterize(index)` supplies
    /// the coverage bytes (`width * height`) of `glyphs[index]` when the atlas
    /// misses; `None` fails the draw. `fx2`/`fy2` are the frame's `FX2`/`FY2`
    /// (`DrawTile.cpp`).
    ///
    /// Returns false with nothing queued wherever the C++ returns 0. Every
    /// glyph and underline is preflighted before the first tile is queued.
    /// Atlas pages touched by a failed draw may still have been written
    /// (the C++ does the same).
    // C++ `DrawLayout` takes the same inputs; the provider contract above is
    // the reason the signature keeps them separate.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_native_text(
        &mut self,
        created: &CreatedDevice,
        layout: &NativeTextLayout,
        glyphs: &[NativeTextGlyph],
        underlines: &[NativeTextUnderline],
        fx2: f32,
        fy2: f32,
        rasterize: &mut dyn FnMut(usize) -> Option<Vec<u8>>,
    ) -> bool {
        // `DrawLayout` L398: no frame, no draw.
        if self.last_scene.is_none() {
            return false;
        }
        // Pre-check (L401-411): reject every pathological glyph before any
        // atlas lookup or allocation. Bounds finiteness and positivity are
        // the caller's `CalculateRasterRectangle` inputs (`NativeTextGlyph`);
        // the size rule is checked here.
        let page_size = self.text_atlas.page_size();
        for glyph in glyphs {
            if !native_text_raster_rect_fits(glyph.raster_width, glyph.raster_height, page_size) {
                return false;
            }
        }
        // `PinnedPages` (L413) lives for this one pass.
        self.text_atlas.begin_draw();
        let drawn = self.draw_native_text_passes(
            created,
            layout,
            glyphs,
            underlines,
            (fx2, fy2),
            rasterize,
        );
        self.text_atlas.end_draw();
        drawn
    }

    /// `DrawLayout` L413-489 after the pre-check: pin, prepare, preflight,
    /// submit. Pins are dropped by the caller's `end_draw`.
    fn draw_native_text_passes(
        &mut self,
        created: &CreatedDevice,
        layout: &NativeTextLayout,
        glyphs: &[NativeTextGlyph],
        underlines: &[NativeTextUnderline],
        frame: (f32, f32),
        rasterize: &mut dyn FnMut(usize) -> Option<Vec<u8>>,
    ) -> bool {
        // Pin pass (L413-418): every drawable glyph already in the atlas
        // keeps its page from eviction during this draw.
        for glyph in glyphs {
            self.text_atlas.pin_cached_glyph(&glyph.key);
        }

        // Prepare pass (L419-426). A cache hit is touched, which is the
        // `PrepareGlyph` hit path (L108-115); a miss rasterizes and packs it.
        let mut placed = Vec::with_capacity(glyphs.len());
        for (index, glyph) in glyphs.iter().enumerate() {
            let slot = match self.text_atlas.touch_cached_glyph(&glyph.key) {
                Some(slot) => slot,
                None => {
                    let Some(coverage) = rasterize(index) else {
                        return false;
                    };
                    let raster = RasterizedGlyph {
                        width: glyph.raster_width,
                        height: glyph.raster_height,
                        coverage: &coverage,
                    };
                    let Ok(prepared) = self.text_atlas.prepare_glyph(glyph.key.clone(), &raster)
                    else {
                        return false;
                    };
                    self.apply_native_text_upload(created, &prepared);
                    prepared.slot
                }
            };
            placed.push(PlacedGlyph {
                left: glyph.draw_left as f32,
                top: glyph.draw_top as f32,
                slot,
            });
        }

        // Solid pixel for underlines (L428-430), only when underlines exist.
        let solid = if underlines.is_empty() {
            None
        } else {
            let Ok(prepared) = self.text_atlas.prepare_solid_pixel() else {
                return false;
            };
            self.apply_native_text_upload(created, &prepared);
            Some(prepared.slot)
        };

        // Preflight (L439-462), then submit (L465-489). Nothing is queued
        // unless every item passes the preflight.
        if !self.draw_native_text_batch(layout, &placed, underlines, solid, frame, false) {
            return false;
        }
        self.draw_native_text_batch(layout, &placed, underlines, solid, frame, true)
    }

    /// One pass over glyphs then underlines, in the C++ order (L439-462 and
    /// L465-489). `submit == false` is the preflight.
    fn draw_native_text_batch(
        &mut self,
        layout: &NativeTextLayout,
        placed: &[PlacedGlyph],
        underlines: &[NativeTextUnderline],
        solid: Option<AtlasSlot>,
        frame: (f32, f32),
        submit: bool,
    ) -> bool {
        for glyph in placed {
            let rect = AtlasRect {
                offset_x: glyph.left,
                offset_y: glyph.top,
                width: glyph.slot.width as f32,
                height: glyph.slot.height as f32,
            };
            if !self.draw_atlas_rect(layout, &glyph.slot, rect, frame, submit) {
                return false;
            }
        }
        for underline in underlines {
            let width = (underline.end - underline.start) as f32;
            if width <= 0.0 {
                continue;
            }
            let Some(solid) = solid else {
                return false;
            };
            let rect = AtlasRect {
                offset_x: underline.start as f32,
                offset_y: (underline.baseline - underline.position - underline.thickness * 0.5)
                    as f32,
                width,
                height: underline.thickness as f32,
            };
            if !self.draw_atlas_rect(layout, &solid, rect, frame, submit) {
                return false;
            }
        }
        true
    }

    /// `DrawAtlasRect` (`NativeText.cpp` L700-756). `submit == false` is the
    /// preflight: the same clipping and validity checks run, nothing is
    /// queued, and the result is whether a submit would be accepted now
    /// (C++ `!Renderer.NoDrawTile`, L751-752).
    fn draw_atlas_rect(
        &mut self,
        layout: &NativeTextLayout,
        slot: &AtlasSlot,
        rect: AtlasRect,
        frame: (f32, f32),
        submit: bool,
    ) -> bool {
        // Page lookup (L702-705). Pages not in the texture cache are missing.
        let cache_id = native_text_page_cache_id(slot.page);
        let page_ready = self
            .texture_cache
            .get(cache_id)
            .is_some_and(|page| page.width > 0);
        if !page_ready || rect.width <= 0.0 || rect.height <= 0.0 {
            return false;
        }

        // Source rectangle and origin (L707-713).
        let mut width = rect.width;
        let mut height = rect.height;
        let mut source_width = slot.width as f32;
        let mut source_height = slot.height as f32;
        let mut x = layout.start_x as f32 + rect.offset_x;
        let mut y = layout.start_y as f32 + rect.offset_y;
        let mut u = slot.x as f32;
        let mut v = slot.y as f32;

        // Clipping (L714-738). Left and top clip shift the source; right and
        // bottom clip shrink it. The division by `width`/`height` follows the
        // C++ exactly, including the zero-width case that the empty check
        // below catches.
        if layout.clip {
            if x < 0.0 {
                let delta = (-x).min(width);
                let source_delta = delta * source_width / width;
                x += delta;
                width -= delta;
                u += source_delta;
                source_width -= source_delta;
            }
            if y < 0.0 {
                let delta = (-y).min(height);
                let source_delta = delta * source_height / height;
                y += delta;
                height -= delta;
                v += source_delta;
                source_height -= source_delta;
            }
            if x + width > layout.clip_x {
                let keep = (layout.clip_x - x).max(0.0);
                source_width *= keep / width;
                width = keep;
            }
            if y + height > layout.clip_y {
                let keep = (layout.clip_y - y).max(0.0);
                source_height *= keep / height;
                height = keep;
            }
        }
        // Fully clipped away: skipped, and still reported as handled (L739-740).
        if width <= 0.0 || height <= 0.0 || source_width <= 0.0 || source_height <= 0.0 {
            return true;
        }

        // Text poly flags (L741): masked off, highlighted, translucent, two-sided.
        let text_poly_flags =
            (layout.poly_flags & !PF_MASKED) | PF_HIGHLIGHTED | PF_TRANSLUCENT | PF_TWO_SIDED;
        let draw_x = layout.origin_x + x;
        let draw_y = layout.origin_y + y;
        if !safe_float(draw_x)
            || !safe_float(draw_y)
            || !safe_float(width)
            || !safe_float(height)
            || !safe_float(u)
            || !safe_float(v)
            || !safe_float(source_width)
            || !safe_float(source_height)
        {
            return false;
        }

        // Queue state (L751-752 and `DrawNativeTextTile`'s own guards). The
        // preflight returns whether the device could accept a tile now.
        if !self.native_text_can_queue(cache_id) {
            return false;
        }
        if !submit {
            return true;
        }
        self.queue_tile(TileDrawRequest {
            cache_id,
            x: draw_x,
            y: draw_y,
            xl: width,
            yl: height,
            u,
            v,
            ul: source_width,
            vl: source_height,
            // `DrawAtlasRect` passes Z = 1 (L755).
            z: 1.0,
            color: [layout.color[0], layout.color[1], layout.color[2], 1.0],
            poly_flags: text_poly_flags,
            // `DrawNativeTextTile` sets UMult = 1/TextureWidth and
            // VMult = 1/TextureHeight (`DrawTile.cpp` L58-109). A scale of
            // 1 gives `texture_uv_mult` exactly that.
            u_scale: 1.0,
            v_scale: 1.0,
            texture_alpha: None,
            fx2: frame.0,
            fy2: frame.1,
        });
        true
    }

    /// Mirrors `RenderLoop::queue_tile`'s early returns (`lib.rs` L1770-1781):
    /// no open frame, no scene node, no uniform segment, or no texture-cache
    /// entry means the tile would be silently dropped. C++ `DrawNativeTextTile`
    /// returns 0 in the same cases (`DrawTile.cpp` L78), so the preflight must
    /// refuse them here instead of reporting success.
    fn native_text_can_queue(&self, cache_id: u64) -> bool {
        self.in_progress.is_some()
            && self.last_scene.is_some()
            && !self.frame_uniforms_segments.is_empty()
            && self.texture_cache.get(cache_id).is_some()
    }

    /// Uploads one prepared slot's pixels into the texture cache. Queued
    /// draws are flushed first: they may sample a region the upload is about
    /// to overwrite, or a page the atlas just evicted and reused.
    fn apply_native_text_upload(&mut self, created: &CreatedDevice, prepared: &PreparedSlot) {
        if prepared.uploads.is_empty() && prepared.evicted_page.is_none() {
            return;
        }
        self.flush_pending_batches(created);
        let page_size = self.text_atlas.page_size();
        for upload in &prepared.uploads {
            let cache_id = native_text_page_cache_id(upload.page);
            if self.texture_cache.get(cache_id).is_none() {
                self.texture_cache.insert_blank_rgba8(
                    created,
                    cache_id,
                    page_size,
                    page_size,
                    SamplerKey {
                        clamp_u: true,
                        clamp_v: true,
                        no_smooth: false,
                    },
                );
            }
            let written = self.texture_cache.write_rgba8_region(
                created,
                cache_id,
                (upload.x, upload.y),
                (upload.width, upload.height),
                &upload.rgba,
            );
            assert!(written, "native text page must be resident after insert");
        }
    }
}
