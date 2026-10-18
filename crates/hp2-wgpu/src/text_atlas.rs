//! Renderer-neutral port of the native-text glyph atlas (`FNativeTextAtlas`).
//!
//! Line-by-line port of `ThirdParty/XOpenGLDrv/Src/NativeText.cpp`:
//! - L27 `NativeTextAtlasPageLimit` -> [`PAGE_LIMIT`]
//! - L34-53 page / glyph records -> [`Page`], [`AtlasSlot`]
//! - L55-373 `FNativeTextAtlas` -> [`TextAtlas`]
//! - L413-418 (`DrawLayout` pin pass) -> [`TextAtlas::begin_draw`],
//!   [`TextAtlas::pin_cached_glyph`], [`TextAtlas::end_draw`]
//!
//! No wgpu types live here. Every write to a page is returned as a
//! [`PageUpload`] for the GPU side to apply later. Eviction is reported via
//! [`PreparedSlot::evicted_page`] so the GPU side can flush pending draws
//! before applying the uploads (C++ flushes unconditionally at L340).

use std::collections::HashMap;

/// Atlas page edge length in texels. `NativeTextShared.h` L65 (Apple) / L208 (FreeType).
pub const PAGE_SIZE: u32 = 1024;

/// Transparent border texels around every glyph. `NativeTextShared.h` L66 (Apple) / L209 (FreeType).
pub const PADDING: u32 = 1;

/// Maximum number of atlas pages. `NativeText.cpp` L27 (`NativeTextAtlasPageLimit`).
pub const PAGE_LIMIT: u32 = 4;

/// Identity of a rasterized glyph. Mirrors `FGlyphKey` (`NativeTextShared.h` L68-84 Apple,
/// L216-232 FreeType); `FontIdentity` is a `std::string` in both variants.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_identity: String,
    pub glyph: u32,
    pub raster_point_size_64: u32,
    pub antialias: bool,
    pub subpixel: bool,
}

/// Borrowed 8-bit coverage bitmap. Mirrors `FRasterizedGlyph`
/// (`NativeTextShared.h` L126-131 Apple, L274-279 FreeType); `Alpha` is `width * height` bytes.
#[derive(Clone, Copy, Debug)]
pub struct RasterizedGlyph<'a> {
    pub width: u32,
    pub height: u32,
    pub coverage: &'a [u8],
}

/// Location of a glyph's pixels inside an atlas page, excluding padding.
/// Mirrors `FAtlasSlot` (`NativeTextShared.h` L133-139) / `FNativeTextAtlasGlyph`
/// (`NativeText.cpp` L46-53). UVs are taken from `x`/`y` (`DrawAtlasRect`, L700-713).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasSlot {
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// One rectangle of RGBA8 texels the GPU side must write into a page.
/// `rgba` is `width * height * 4` bytes, row-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageUpload {
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Result of preparing a glyph or the solid pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedSlot {
    pub slot: AtlasSlot,
    /// Texel writes to apply, in order. Empty on a cache hit.
    pub uploads: Vec<PageUpload>,
    /// Page whose contents were discarded to make room. The GPU side must flush
    /// pending draws before applying `uploads`.
    pub evicted_page: Option<u32>,
}

/// Failure of [`TextAtlas::prepare_glyph`] / [`TextAtlas::prepare_solid_pixel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtlasError {
    /// Padded glyph cannot be packed on an empty page. The C++ equivalent is the
    /// `FPageAllocate`/`Allocate` failure on a fresh page (`NativeText.cpp` L284-302),
    /// which the C++ caller never reaches because `CalculateRasterRectangle` already
    /// rejects such glyphs (`NativeTextShared.cpp` L899-918).
    GlyphTooLarge,
    /// `coverage.len() != width * height`. C++ indexes `Raster.Alpha` unchecked (L153).
    InvalidCoverage,
    /// No page has room, no page can be created (limit reached or page too small),
    /// and every existing page is pinned (`NativeText.cpp` L224-231, L337-338).
    NoPageAvailable,
}

/// One atlas page. Mirrors `FNativeTextAtlasPage` (`NativeText.cpp` L34-44).
/// Shelf cursor: `next_x`/`next_y` is the next free origin on the current row,
/// `row_height` the tallest outer rectangle on that row.
#[derive(Clone, Copy, Debug)]
struct Page {
    size: u32,
    next_x: u32,
    next_y: u32,
    row_height: u32,
    /// Monotonic use stamp, compared for LRU eviction.
    last_use: u64,
}

impl Page {
    /// Empty page. Cursor starts at `(1, 0)`: texel `(0, 0)` is reserved for the
    /// solid white texel (`NativeText.cpp` L233-237, L260-261).
    fn fresh(size: u32) -> Self {
        Self {
            size,
            next_x: 1,
            next_y: 0,
            row_height: 1,
            last_use: 0,
        }
    }

    /// Pure shelf placement. Mirrors the wrap rule in `FindPageForRectangle`
    /// (`NativeText.cpp` L304-318): when the rectangle does not fit at the end of the
    /// current row, wrap to column 0 on the row below the current one.
    /// Returns the origin, or `None` if it does not fit.
    fn place(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        let (mut x, mut y) = (self.next_x, self.next_y);
        if x.saturating_add(width) > self.size {
            x = 0;
            y = y.saturating_add(self.row_height);
        }
        if width > self.size
            || x.saturating_add(width) > self.size
            || y.saturating_add(height) > self.size
        {
            return None;
        }
        Some((x, y))
    }

    /// Mirrors `FNativeTextAtlas::Allocate` (`NativeText.cpp` L284-302). Commits the
    /// cursor only on success, so a failed allocation leaves the page unchanged.
    fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        let wrapped = self.next_x.saturating_add(width) > self.size;
        let (x, y) = self.place(width, height)?;
        self.next_x = x + width;
        self.next_y = y;
        self.row_height = if wrapped {
            height
        } else {
            self.row_height.max(height)
        };
        Some((x, y))
    }
}

/// Where a rectangle was placed by [`TextAtlas::find_page_for_rectangle`].
#[derive(Clone, Copy, Debug)]
struct Placed {
    page: u32,
    x: u32,
    y: u32,
    evicted_page: Option<u32>,
}

/// Glyph atlas. Mirrors `FNativeTextAtlas` (`NativeText.cpp` L55-373).
#[derive(Debug)]
pub struct TextAtlas {
    page_size: u32,
    pages: Vec<Page>,
    glyphs: HashMap<GlyphKey, AtlasSlot>,
    /// Pages referenced by the current draw. Cleared by [`begin_draw`]/[`end_draw`].
    pinned: Vec<u32>,
    /// Starts at 1 (`NativeText.cpp` L58-73 `Reset` increments it).
    generation: u64,
    /// Starts at 0; pre-incremented on every touch.
    use_serial: u64,
}

impl Default for TextAtlas {
    fn default() -> Self {
        Self::new()
    }
}

impl TextAtlas {
    /// Atlas with the default [`PAGE_SIZE`].
    pub fn new() -> Self {
        Self::with_page_size(PAGE_SIZE)
    }

    /// Atlas with a caller-chosen page edge. The C++ constructor clamps
    /// `PageSize` to `MaxTextureSize` (`NativeText.cpp` L29-32); that clamp stays with the caller.
    pub fn with_page_size(page_size: u32) -> Self {
        Self {
            page_size,
            pages: Vec::new(),
            glyphs: HashMap::new(),
            pinned: Vec::new(),
            generation: 1,
            use_serial: 0,
        }
    }

    /// Clears every page and glyph, bumps the generation, and drops pins.
    /// Mirrors `FNativeTextAtlas::Reset` (`NativeText.cpp` L58-73). The caller flushes
    /// its pending batch; `use_serial` is left alone, as in C++.
    pub fn reset(&mut self) {
        self.pages.clear();
        self.glyphs.clear();
        self.pinned.clear();
        self.generation += 1;
    }

    /// Mirrors `GetGeneration` (`NativeText.cpp` L75-78).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Mirrors `PageCount` (`NativeText.cpp` L80-83).
    pub fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }

    /// Page edge length used for every page of this atlas.
    pub fn page_size(&self) -> u32 {
        self.page_size
    }

    /// Start of a `DrawLayout` pass: drops the previous pass's pins
    /// (`NativeText.cpp` L413, local `PinnedPages`).
    pub fn begin_draw(&mut self) {
        self.pinned.clear();
    }

    /// End of a `DrawLayout` pass: drops this pass's pins.
    pub fn end_draw(&mut self) {
        self.pinned.clear();
    }

    /// Pins the page holding `key` without touching LRU (`NativeText.cpp` L96-101).
    /// Used by the pin pass in `DrawLayout` (L413-418).
    pub fn pin_cached_glyph(&mut self, key: &GlyphKey) {
        if let Some(slot) = self.glyphs.get(key).copied() {
            self.pin(slot.page);
        }
    }

    /// Cache-hit branch of `PrepareGlyph` (`NativeText.cpp` L108-115): if `key` is
    /// cached, touches and pins its page and returns the slot.
    pub fn touch_cached_glyph(&mut self, key: &GlyphKey) -> Option<AtlasSlot> {
        let slot = self.glyphs.get(key).copied()?;
        self.touch_page(slot.page);
        self.pin(slot.page);
        Some(slot)
    }

    /// Read-only lookup. Mirrors `FindGlyph` (`NativeText.cpp` L194-201), which is
    /// `const` and does not touch LRU.
    pub fn lookup(&self, key: &GlyphKey) -> Option<AtlasSlot> {
        self.glyphs.get(key).copied()
    }

    /// Packs `glyph` into the atlas. Mirrors `PrepareGlyph` (`NativeText.cpp` L103-178).
    ///
    /// Cache hit (L108-115): touches and pins, returns no uploads.
    /// Miss: rejects oversize or mis-sized input, finds room (existing page, then a new
    /// page, then LRU eviction of an unpinned page), packs the padded outer rectangle
    /// (L148-160), and records the glyph. The page is touched and pinned (L161-177).
    pub fn prepare_glyph(
        &mut self,
        key: GlyphKey,
        glyph: &RasterizedGlyph<'_>,
    ) -> Result<PreparedSlot, AtlasError> {
        if let Some(slot) = self.touch_cached_glyph(&key) {
            return Ok(PreparedSlot {
                slot,
                uploads: Vec::new(),
                evicted_page: None,
            });
        }

        let outer_width = glyph
            .width
            .checked_add(PADDING * 2)
            .ok_or(AtlasError::GlyphTooLarge)?;
        let outer_height = glyph
            .height
            .checked_add(PADDING * 2)
            .ok_or(AtlasError::GlyphTooLarge)?;

        // Preflight: the padded rectangle must fit on an empty page, or no eviction
        // can ever make room. Checked before any page is created or evicted.
        if Page::fresh(self.page_size)
            .place(outer_width, outer_height)
            .is_none()
        {
            return Err(AtlasError::GlyphTooLarge);
        }

        if (glyph.width as usize).checked_mul(glyph.height as usize) != Some(glyph.coverage.len()) {
            return Err(AtlasError::InvalidCoverage);
        }

        let mut uploads = Vec::new();
        let placed = self
            .find_page_for_rectangle(outer_width, outer_height, &mut uploads)
            .ok_or(AtlasError::NoPageAvailable)?;

        uploads.push(PageUpload {
            page: placed.page,
            x: placed.x,
            y: placed.y,
            width: outer_width,
            height: outer_height,
            rgba: pack_outer_rectangle(glyph, outer_width, outer_height),
        });

        let slot = AtlasSlot {
            page: placed.page,
            x: placed.x + PADDING,
            y: placed.y + PADDING,
            width: glyph.width,
            height: glyph.height,
        };
        self.glyphs.insert(key, slot);
        self.touch_page(placed.page);
        self.pin(placed.page);

        Ok(PreparedSlot {
            slot,
            uploads,
            evicted_page: placed.evicted_page,
        })
    }

    /// Slot of the solid white texel at `(0, 0)` of page 0. Mirrors
    /// `PrepareSolidPixel` (`NativeText.cpp` L180-192). Creates page 0 if the atlas is
    /// empty. Texel `(0, 0)` is never overwritten by glyph packing, so it stays white.
    pub fn prepare_solid_pixel(&mut self) -> Result<PreparedSlot, AtlasError> {
        let mut uploads = Vec::new();
        if self.pages.is_empty() && self.create_page(&mut uploads).is_none() {
            return Err(AtlasError::NoPageAvailable);
        }
        self.touch_page(0);
        self.pin(0);
        Ok(PreparedSlot {
            slot: AtlasSlot {
                page: 0,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            uploads,
            evicted_page: None,
        })
    }

    /// Mirrors `FindPageForRectangle` (`NativeText.cpp` L304-355): first existing page
    /// with room, else a new page, else evict the least-recently-used unpinned page.
    fn find_page_for_rectangle(
        &mut self,
        width: u32,
        height: u32,
        uploads: &mut Vec<PageUpload>,
    ) -> Option<Placed> {
        for (index, page) in self.pages.iter_mut().enumerate() {
            if let Some((x, y)) = page.allocate(width, height) {
                return Some(Placed {
                    page: index as u32,
                    x,
                    y,
                    evicted_page: None,
                });
            }
        }

        if let Some(index) = self.create_page(uploads) {
            let (x, y) = self.pages[index as usize].allocate(width, height)?;
            return Some(Placed {
                page: index,
                x,
                y,
                evicted_page: None,
            });
        }

        // Strict `<` over ascending page index: ties go to the lowest index.
        let mut oldest = u64::MAX;
        let mut victim: Option<usize> = None;
        for (index, page) in self.pages.iter().enumerate() {
            if self.pinned.contains(&(index as u32)) {
                continue;
            }
            if page.last_use < oldest {
                oldest = page.last_use;
                victim = Some(index);
            }
        }
        let victim = victim?;
        let victim_page = victim as u32;

        self.pages[victim] = Page::fresh(self.page_size);
        self.glyphs.retain(|_, slot| slot.page != victim_page);
        self.generation += 1;
        let (x, y) = self.pages[victim].allocate(width, height)?;
        Some(Placed {
            page: victim_page,
            x,
            y,
            evicted_page: Some(victim_page),
        })
    }

    /// Mirrors `CreatePage` (`NativeText.cpp` L224-282). Enforces the page limit and the
    /// minimum size, then appends a fresh page with its solid white texel upload.
    fn create_page(&mut self, uploads: &mut Vec<PageUpload>) -> Option<u32> {
        if self.pages.len() >= PAGE_LIMIT as usize || self.page_size <= PADDING * 2 + 1 {
            return None;
        }
        let index = self.pages.len() as u32;
        self.pages.push(Page::fresh(self.page_size));
        uploads.push(PageUpload {
            page: index,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, 255],
        });
        Some(index)
    }

    /// Mirrors `TouchPage` (`NativeText.cpp` L357-361).
    fn touch_page(&mut self, page: u32) {
        self.use_serial += 1;
        if let Some(entry) = self.pages.get_mut(page as usize) {
            entry.last_use = self.use_serial;
        }
    }

    /// Mirrors `Pin` (`NativeText.cpp` L363-367).
    fn pin(&mut self, page: u32) {
        if !self.pinned.contains(&page) {
            self.pinned.push(page);
        }
    }
}

/// Expands 8-bit coverage into the padded RGBA8 outer rectangle (`NativeText.cpp` L148-160):
/// zeroed outer rectangle, then each glyph texel at `(row + PADDING, col + PADDING)` as
/// `(255, 255, 255, coverage)`.
fn pack_outer_rectangle(
    glyph: &RasterizedGlyph<'_>,
    outer_width: u32,
    outer_height: u32,
) -> Vec<u8> {
    let outer_w = outer_width as usize;
    let width = glyph.width as usize;
    let height = glyph.height as usize;
    let pad = PADDING as usize;
    let mut rgba = vec![0u8; outer_w * outer_height as usize * 4];
    for row in 0..height {
        for col in 0..width {
            let dst = ((row + pad) * outer_w + col + pad) * 4;
            rgba[dst..dst + 4].copy_from_slice(&[255, 255, 255, glyph.coverage[row * width + col]]);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(glyph: u32) -> GlyphKey {
        GlyphKey {
            font_identity: String::from("Test"),
            glyph,
            raster_point_size_64: 640,
            antialias: true,
            subpixel: false,
        }
    }

    fn white(alpha: u8) -> [u8; 4] {
        [255, 255, 255, alpha]
    }

    #[test]
    fn first_glyph_lands_after_solid_texel() {
        let mut atlas = TextAtlas::new();
        let cov = [1u8, 2, 3, 4];
        let glyph = RasterizedGlyph {
            width: 2,
            height: 2,
            coverage: &cov,
        };
        atlas.begin_draw();
        let prepared = atlas.prepare_glyph(key(1), &glyph).unwrap();
        atlas.end_draw();

        assert_eq!(
            prepared.slot,
            AtlasSlot {
                page: 0,
                x: 2,
                y: 1,
                width: 2,
                height: 2
            }
        );
        assert_eq!(prepared.uploads.len(), 2);
        assert_eq!(
            prepared.uploads[0],
            PageUpload {
                page: 0,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            }
        );
        assert_eq!(
            (
                prepared.uploads[1].page,
                prepared.uploads[1].x,
                prepared.uploads[1].y
            ),
            (0, 1, 0)
        );
        assert_eq!(prepared.uploads[1].width, 4);
        assert_eq!(prepared.uploads[1].height, 4);
        assert_eq!(prepared.evicted_page, None);
    }

    #[test]
    fn second_glyph_packs_on_same_shelf() {
        let mut atlas = TextAtlas::new();
        let cov_a = [1u8; 4];
        let cov_b = [1u8; 6];
        atlas.begin_draw();
        atlas
            .prepare_glyph(
                key(1),
                &RasterizedGlyph {
                    width: 2,
                    height: 2,
                    coverage: &cov_a,
                },
            )
            .unwrap();
        let second = atlas
            .prepare_glyph(
                key(2),
                &RasterizedGlyph {
                    width: 3,
                    height: 2,
                    coverage: &cov_b,
                },
            )
            .unwrap();
        atlas.end_draw();

        assert_eq!(second.slot.page, 0);
        assert_eq!((second.slot.x, second.slot.y), (6, 1));
        assert_eq!(second.uploads.len(), 1);
        assert_eq!((second.uploads[0].x, second.uploads[0].y), (5, 0));
    }

    #[test]
    fn shelf_wraps_to_next_row_then_new_page() {
        let mut atlas = TextAtlas::with_page_size(10);
        let cov = [1u8; 9];
        let glyph = RasterizedGlyph {
            width: 3,
            height: 3,
            coverage: &cov,
        };
        atlas.begin_draw();
        let a = atlas.prepare_glyph(key(1), &glyph).unwrap();
        let b = atlas.prepare_glyph(key(2), &glyph).unwrap();
        let c = atlas.prepare_glyph(key(3), &glyph).unwrap();
        let d = atlas.prepare_glyph(key(4), &glyph).unwrap();
        atlas.end_draw();

        assert_eq!(
            a.slot,
            AtlasSlot {
                page: 0,
                x: 2,
                y: 1,
                width: 3,
                height: 3
            }
        );
        assert_eq!(
            b.slot,
            AtlasSlot {
                page: 0,
                x: 1,
                y: 6,
                width: 3,
                height: 3
            }
        );
        assert_eq!(
            c.slot,
            AtlasSlot {
                page: 0,
                x: 6,
                y: 6,
                width: 3,
                height: 3
            }
        );
        assert_eq!(
            d.slot,
            AtlasSlot {
                page: 1,
                x: 2,
                y: 1,
                width: 3,
                height: 3
            }
        );
        assert_eq!(d.uploads.len(), 2);
        assert_eq!(d.uploads[0].page, 1);
        assert_eq!(d.uploads[0].rgba, vec![255; 4]);
        assert_eq!(
            (d.uploads[1].page, d.uploads[1].x, d.uploads[1].y),
            (1, 1, 0)
        );
        assert_eq!(atlas.page_count(), 2);
    }

    #[test]
    fn existing_key_returns_same_slot_without_upload() {
        let mut atlas = TextAtlas::new();
        let cov = [9u8; 4];
        let glyph = RasterizedGlyph {
            width: 2,
            height: 2,
            coverage: &cov,
        };
        atlas.begin_draw();
        let first = atlas.prepare_glyph(key(1), &glyph).unwrap();
        let second = atlas.prepare_glyph(key(1), &glyph).unwrap();
        atlas.end_draw();

        assert_eq!(first.slot, second.slot);
        assert!(second.uploads.is_empty());
        assert_eq!(second.evicted_page, None);
    }

    #[test]
    fn rgba_layout_for_2x2_glyph() {
        let mut atlas = TextAtlas::new();
        let cov = [10u8, 20, 30, 40];
        let glyph = RasterizedGlyph {
            width: 2,
            height: 2,
            coverage: &cov,
        };
        atlas.begin_draw();
        let prepared = atlas.prepare_glyph(key(1), &glyph).unwrap();
        atlas.end_draw();

        let clear = [0u8; 4];
        let rows: [[u8; 4]; 16] = [
            clear,
            clear,
            clear,
            clear,
            clear,
            white(10),
            white(20),
            clear,
            clear,
            white(30),
            white(40),
            clear,
            clear,
            clear,
            clear,
            clear,
        ];
        assert_eq!(prepared.uploads[1].rgba, rows.as_flattened().to_vec());
    }

    #[test]
    fn lru_evicts_least_recently_used_unpinned_page() {
        let mut atlas = TextAtlas::with_page_size(10);
        let cov = [7u8; 49];
        let glyph = RasterizedGlyph {
            width: 7,
            height: 7,
            coverage: &cov,
        };
        for n in 1..=4 {
            atlas.begin_draw();
            let prepared = atlas.prepare_glyph(key(n), &glyph).unwrap();
            atlas.end_draw();
            assert_eq!(prepared.slot.page, n - 1);
        }

        atlas.begin_draw();
        assert!(atlas.touch_cached_glyph(&key(1)).is_some());
        atlas.end_draw();

        atlas.begin_draw();
        let prepared = atlas.prepare_glyph(key(5), &glyph).unwrap();
        atlas.end_draw();

        assert_eq!(prepared.evicted_page, Some(1));
        assert_eq!(prepared.slot.page, 1);
        assert_eq!(prepared.uploads.len(), 1);
        assert_eq!(atlas.generation(), 2);
        assert_eq!(atlas.lookup(&key(2)), None);
        assert!(atlas.lookup(&key(1)).is_some());
        assert!(atlas.lookup(&key(3)).is_some());
        assert!(atlas.lookup(&key(4)).is_some());
        assert!(atlas.lookup(&key(5)).is_some());
    }

    #[test]
    fn pinned_page_is_skipped_by_eviction() {
        let mut atlas = TextAtlas::with_page_size(10);
        let cov = [7u8; 49];
        let glyph = RasterizedGlyph {
            width: 7,
            height: 7,
            coverage: &cov,
        };
        for n in 1..=4 {
            atlas.begin_draw();
            atlas.prepare_glyph(key(n), &glyph).unwrap();
            atlas.end_draw();
        }

        atlas.begin_draw();
        atlas.pin_cached_glyph(&key(2));
        let prepared = atlas.prepare_glyph(key(5), &glyph).unwrap();
        atlas.end_draw();

        assert_eq!(prepared.evicted_page, Some(0));
        assert_eq!(prepared.slot.page, 0);
        assert!(atlas.lookup(&key(2)).is_some());
        assert_eq!(atlas.lookup(&key(1)), None);
    }

    #[test]
    fn all_pages_pinned_returns_no_page_available() {
        let mut atlas = TextAtlas::with_page_size(10);
        let cov = [7u8; 49];
        let glyph = RasterizedGlyph {
            width: 7,
            height: 7,
            coverage: &cov,
        };
        for n in 1..=4 {
            atlas.begin_draw();
            atlas.prepare_glyph(key(n), &glyph).unwrap();
            atlas.end_draw();
        }

        atlas.begin_draw();
        for n in 1..=4 {
            atlas.pin_cached_glyph(&key(n));
        }
        let result = atlas.prepare_glyph(key(5), &glyph);
        atlas.end_draw();

        assert_eq!(result, Err(AtlasError::NoPageAvailable));
        assert_eq!(atlas.page_count(), 4);
        assert_eq!(atlas.generation(), 1);
        assert_eq!(atlas.lookup(&key(5)), None);
    }

    #[test]
    fn reset_clears_everything() {
        let mut atlas = TextAtlas::new();
        let cov = [0u8; 4];
        let glyph = RasterizedGlyph {
            width: 2,
            height: 2,
            coverage: &cov,
        };
        atlas.begin_draw();
        atlas.prepare_glyph(key(1), &glyph).unwrap();
        atlas.end_draw();

        atlas.reset();
        assert_eq!(atlas.page_count(), 0);
        assert_eq!(atlas.lookup(&key(1)), None);
        assert_eq!(atlas.generation(), 2);

        atlas.begin_draw();
        let prepared = atlas.prepare_glyph(key(1), &glyph).unwrap();
        atlas.end_draw();
        assert_eq!(prepared.slot.page, 0);
        assert_eq!(prepared.uploads.len(), 2);
        assert_eq!(prepared.uploads[0].rgba, vec![255; 4]);
    }

    #[test]
    fn glyph_larger_than_page_is_too_large() {
        let mut atlas = TextAtlas::with_page_size(10);
        let cov8 = [0u8; 64];
        let cov9 = [0u8; 81];
        assert_eq!(
            atlas.prepare_glyph(
                key(1),
                &RasterizedGlyph {
                    width: 8,
                    height: 8,
                    coverage: &cov8,
                },
            ),
            Err(AtlasError::GlyphTooLarge)
        );
        assert_eq!(
            atlas.prepare_glyph(
                key(2),
                &RasterizedGlyph {
                    width: 9,
                    height: 9,
                    coverage: &cov9,
                },
            ),
            Err(AtlasError::GlyphTooLarge)
        );
        assert_eq!(atlas.page_count(), 0);
    }

    #[test]
    fn wide_glyph_wraps_onto_second_row_and_fits() {
        let mut atlas = TextAtlas::with_page_size(10);
        let cov = [5u8; 8];
        let glyph = RasterizedGlyph {
            width: 8,
            height: 1,
            coverage: &cov,
        };
        atlas.begin_draw();
        let prepared = atlas.prepare_glyph(key(1), &glyph).unwrap();
        atlas.end_draw();
        assert_eq!(
            prepared.slot,
            AtlasSlot {
                page: 0,
                x: 1,
                y: 2,
                width: 8,
                height: 1
            }
        );
        assert_eq!((prepared.uploads[1].x, prepared.uploads[1].y), (0, 1));
    }

    #[test]
    fn invalid_coverage_is_rejected() {
        let mut atlas = TextAtlas::new();
        let cov = [1u8; 3];
        assert_eq!(
            atlas.prepare_glyph(
                key(1),
                &RasterizedGlyph {
                    width: 2,
                    height: 2,
                    coverage: &cov,
                },
            ),
            Err(AtlasError::InvalidCoverage)
        );
        assert_eq!(atlas.page_count(), 0);
    }

    #[test]
    fn solid_pixel_creates_page_zero() {
        let mut atlas = TextAtlas::new();
        atlas.begin_draw();
        let prepared = atlas.prepare_solid_pixel().unwrap();
        atlas.end_draw();
        assert_eq!(
            prepared.slot,
            AtlasSlot {
                page: 0,
                x: 0,
                y: 0,
                width: 1,
                height: 1
            }
        );
        assert_eq!(prepared.uploads.len(), 1);
        assert_eq!(prepared.uploads[0].rgba, vec![255; 4]);
        assert_eq!(atlas.page_count(), 1);
    }

    #[test]
    fn page_size_too_small_for_padding() {
        let mut atlas = TextAtlas::with_page_size(PADDING * 2 + 1);
        let cov: [u8; 0] = [];
        assert_eq!(
            atlas.prepare_glyph(
                key(1),
                &RasterizedGlyph {
                    width: 0,
                    height: 0,
                    coverage: &cov,
                },
            ),
            Err(AtlasError::NoPageAvailable)
        );
        assert_eq!(
            atlas.prepare_solid_pixel(),
            Err(AtlasError::NoPageAvailable)
        );
        assert_eq!(atlas.page_count(), 0);
    }
}
