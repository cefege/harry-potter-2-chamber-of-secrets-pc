//! Serialized `UFont` glyph records → blittable glyph data.
//!
//! Wire format (pinned against a real dump of Engine.u export #3402,
//! SmallFont; see Tests/Fixtures/font/): the export payload carries no
//! tagged properties for stock fonts — it opens with the empty property
//! terminator (compact index 0), then
//!   TArray<FFontPage>          compact count
//!     per page: texture ref    compact object reference (export+1)
//!               TArray<FFontCharacter> compact count + 4×i32 rects
//!                                 (StartU, StartV, USize, VSize)
//!   CharactersPerPage          i32
//!   [package version ≥ 69]     TMap<TCHAR,TCHAR> compact count + pairs,
//!                              then IsRemapped u32
//!
//! Errors are loud with `font.*` reason-code prefixes.

use crate::package79::{ByteCursor, PackageError, read_compact_index};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    /// The payload began with something other than an empty property list.
    UnexpectedProperties { name_index: i32 },
    /// Fewer bytes than the declared structure requires.
    Truncated { need: usize, got: usize },
    /// Any package-level decode rejection while walking the payload.
    Malformed { detail: String },
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::UnexpectedProperties { name_index } => write!(
                f,
                "font.unexpected_property: name index {name_index} opens a UFont payload"
            ),
            FontError::Truncated { need, got } => {
                write!(f, "font.truncated: need {need} bytes, got {got}")
            }
            FontError::Malformed { detail } => write!(f, "font.malformed: {detail}"),
        }
    }
}

impl std::error::Error for FontError {}

impl From<PackageError> for FontError {
    fn from(error: PackageError) -> Self {
        match error {
            PackageError::Truncated { need, got } => FontError::Truncated { need, got },
            other => FontError::Malformed {
                detail: other.to_string(),
            },
        }
    }
}

/// One character's rect inside its page texture (`FFontCharacter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphRect {
    pub start_u: i32,
    pub start_v: i32,
    pub u_size: i32,
    pub v_size: i32,
}

impl GlyphRect {
    fn read(cur: &mut ByteCursor<'_>) -> Result<GlyphRect, FontError> {
        Ok(GlyphRect {
            start_u: cur.i32()?,
            start_v: cur.i32()?,
            u_size: cur.i32()?,
            v_size: cur.i32()?,
        })
    }
}

/// One texture page plus its character rects (`FFontPage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontPage {
    /// Compact object reference to the page texture (export index + 1).
    pub texture_ref: i32,
    pub characters: Vec<GlyphRect>,
}

/// Parsed `UFont` native payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UFontData {
    pub pages: Vec<FontPage>,
    pub characters_per_page: i32,
    /// Case remap pairs (empty on all stock fonts).
    pub char_remap: Vec<(i32, i32)>,
    pub is_remapped: bool,
}

impl UFontData {
    /// Canvas lookup math: character code → (page, slot within page).
    pub fn page_and_slot(&self, code: u8) -> Option<(usize, usize)> {
        let per_page = usize::try_from(self.characters_per_page).ok()?;
        if per_page == 0 || self.pages.is_empty() {
            return None;
        }
        let page = code as usize / per_page;
        if page >= self.pages.len() {
            return None;
        }
        let slot = code as usize % per_page;
        if slot >= self.pages[page].characters.len() {
            return None;
        }
        Some((page, slot))
    }
}

/// Parse one serialized `UFont` payload.
pub fn read_ufont_payload(bytes: &[u8]) -> Result<UFontData, FontError> {
    let mut cur = ByteCursor::new(bytes);
    let terminator = read_compact_index(&mut cur)?;
    if terminator != 0 {
        return Err(FontError::UnexpectedProperties {
            name_index: terminator,
        });
    }

    let page_count = read_compact_index(&mut cur)?;
    let mut pages = Vec::with_capacity(page_count.max(0) as usize);
    for _ in 0..page_count {
        let texture_ref = read_compact_index(&mut cur)?;
        let char_count = read_compact_index(&mut cur)?;
        let mut characters = Vec::with_capacity(char_count.max(0) as usize);
        for _ in 0..char_count {
            characters.push(GlyphRect::read(&mut cur)?);
        }
        pages.push(FontPage {
            texture_ref,
            characters,
        });
    }

    let characters_per_page = cur.i32()?;
    let remap_entries = read_compact_index(&mut cur)?;
    let mut char_remap = Vec::with_capacity(remap_entries.max(0) as usize);
    for _ in 0..remap_entries {
        let from = cur.i32()?;
        let to = cur.i32()?;
        char_remap.push((from, to));
    }
    let is_remapped = cur.u32()? != 0;

    Ok(UFontData {
        pages,
        characters_per_page,
        char_remap,
        is_remapped,
    })
}

/// Blit-ready glyph set for one decoded page bitmap: the bitmap itself plus
/// per-character advance widths (`USize`), matching what the canvas draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphAtlas {
    pub pixels: Vec<u8>,
    pub advances: Vec<i32>,
}

impl GlyphAtlas {
    /// `pixels` is the decoded page bitmap in row-major form (P8 indices or
    /// expanded channels — opaque to the atlas); advances come from the
    /// glyph rects.
    pub fn from_page(pixels: Vec<u8>, characters: &[GlyphRect]) -> GlyphAtlas {
        GlyphAtlas {
            pixels,
            advances: characters.iter().map(|rect| rect.u_size).collect(),
        }
    }
}
