//! Bitmap glyph-quad canvas (Milestone M5).
//!
//! Menus and UWindow draw through this layer: text becomes camera-aligned
//! glyph quads over a `hp_format::font::GlyphAtlas` page texture, drawn by
//! the translucent surface pipeline. Metric math (advance accumulation,
//! clip wrapping) lives CPU-side so it is testable without a GPU.

use crate::mesh_gpu::{MeshVertex, billboard_quad};
use hp_format::font::{GlyphAtlas, UFontData};

/// Pen advance for one character: the glyph's `USize` (width) from its rect.
pub fn glyph_advance(atlas: &GlyphAtlas, font: &UFontData, code: u8) -> Option<i32> {
    let (_, slot) = font.page_and_slot(code)?;
    atlas.advances.get(slot).copied()
}

/// Total rendered width of `text` in pixels (sum of advances; unknown
/// characters contribute nothing — loud handling is upstream of metrics).
pub fn measure_text(atlas: &GlyphAtlas, font: &UFontData, text: &str) -> i32 {
    text.bytes()
        .filter_map(|code| glyph_advance(atlas, font, code))
        .sum()
}

/// Line-break offsets for `text` against `max_width`: greedy packing where
/// each break lands right after the last space that still fit. Spaces
/// consume their advance in UE1 canvas metrics; an empty result means the
/// whole text fits on one line.
pub fn wrap_points(atlas: &GlyphAtlas, font: &UFontData, text: &str, max_width: i32) -> Vec<usize> {
    let advances: Vec<(usize, u8, i32)> = text
        .bytes()
        .enumerate()
        .filter_map(|(index, code)| glyph_advance(atlas, font, code).map(|a| (index, code, a)))
        .collect();

    let mut breaks = Vec::new();
    // Invariants: line_start..cursor covers the current line; last_space is
    // the candidate break (text index after the space, width up to it).
    let mut cursor = 0usize;
    let mut line_width = 0i32;
    let mut last_space: Option<(usize, i32)> = None;
    while cursor < advances.len() {
        let (_, code, advance) = advances[cursor];
        if code == b' ' {
            last_space = Some((cursor + 1, line_width));
            line_width += advance;
            cursor += 1;
            continue;
        }
        if line_width + advance <= max_width || cursor == 0 {
            line_width += advance;
            cursor += 1;
            continue;
        }
        match last_space {
            Some((pos, _before)) => {
                breaks.push(pos);
                line_width = advance;
                last_space = None;
                cursor += 1;
            }
            None => {
                // Unbreakable run longer than the line: hard-break here.
                breaks.push(cursor + 1);
                line_width = 0;
                cursor += 1;
            }
        }
    }
    let _ = &advances;
    breaks.retain(|&pos| pos < text.len());
    breaks.sort_unstable();
    breaks.dedup();
    breaks
}
/// Glyph quads for one text run at `pen` origin (top-left), each vertex in
/// clip-ready world coordinates supplied via `to_world` closure over pixel
/// offsets. Returns quads in draw order with per-glyph UV rects.
pub struct GlyphQuad {
    pub vertices: [MeshVertex; 4],
    pub indices: [u16; 6],
}

/// Build blit quads for `text`. `glyph_uv` maps (glyph rect, page size) to
/// normalized UV corners `[bl, br, tr, tl]`; `place` converts pixel-space
/// (x, y, w, h) into world-space center/right/up.
pub fn text_quads(
    atlas: &GlyphAtlas,
    font: &UFontData,
    page_size: [f32; 2],
    text: &str,
    pen: [f32; 2],
    color: [f32; 4],
    place: impl Fn(f32, f32, f32, f32) -> ([f32; 3], [f32; 3], [f32; 3]),
) -> Vec<GlyphQuad> {
    let mut quads = Vec::new();
    let mut cursor_x = pen[0];
    for &byte in text.as_bytes() {
        let Some(advance) = glyph_advance(atlas, font, byte) else {
            continue;
        };
        if advance > 0
            && let Some((page, slot)) = font.page_and_slot(byte)
            && page == 0
        {
            let rect = &font.pages[0].characters[slot];
            let (w, h) = (rect.u_size as f32, rect.v_size as f32);
            let left = (rect.start_u as f32) / page_size[0];
            let right = ((rect.start_u + rect.u_size) as f32) / page_size[0];
            let top = (rect.start_v as f32) / page_size[1];
            let bottom = ((rect.start_v + rect.v_size) as f32) / page_size[1];
            let (center, right_ax, up_ax) = place(cursor_x + w * 0.5, pen[1] + h * 0.5, w, h);
            let (verts, idx) = billboard_quad(
                center,
                right_ax,
                up_ax,
                [w, h],
                [[left, bottom], [right, bottom], [right, top], [left, top]],
                color,
                1.0,
            );
            quads.push(GlyphQuad {
                vertices: verts,
                indices: idx,
            });
        }
        cursor_x += advance as f32;
    }
    quads
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp_format::font::{FontPage, GlyphRect};
    fn fixture() -> (GlyphAtlas, UFontData) {
        // 256 page-backed glyph slots like the C++ canvas contract fixture;
        // 'A'=9, 'B'=5, 'C'=7, everything else zero-width.
        let mut characters = vec![
            GlyphRect {
                start_u: 0,
                start_v: 0,
                u_size: 0,
                v_size: 0
            };
            256
        ];
        characters[b'A' as usize] = GlyphRect {
            start_u: 0,
            start_v: 0,
            u_size: 9,
            v_size: 18,
        };
        characters[b'B' as usize] = GlyphRect {
            start_u: 16,
            start_v: 0,
            u_size: 5,
            v_size: 18,
        };
        characters[b'C' as usize] = GlyphRect {
            start_u: 32,
            start_v: 0,
            u_size: 7,
            v_size: 18,
        };
        let font = UFontData {
            pages: vec![FontPage {
                texture_ref: 1,
                characters,
            }],
            characters_per_page: 256,
            char_remap: vec![],
            is_remapped: false,
        };
        let pixels = vec![255u8; 512 * 32];
        (
            GlyphAtlas::from_page(pixels, &font.pages[0].characters),
            font,
        )
    }

    #[test]
    fn advances_accumulate_like_the_canvas_contract() {
        let (atlas, font) = fixture();
        assert_eq!(glyph_advance(&atlas, &font, b'A'), Some(9));
        assert_eq!(measure_text(&atlas, &font, "AB"), 14);
        assert_eq!(measure_text(&atlas, &font, "ABC"), 21);
        assert_eq!(measure_text(&atlas, &font, ""), 0);
    }

    #[test]
    fn unknown_glyphs_contribute_nothing() {
        let (atlas, font) = fixture();
        // Zero-width page slots (like '1' in this fixture) advance nothing.
        assert_eq!(glyph_advance(&atlas, &font, b'1'), Some(0));
        assert_eq!(measure_text(&atlas, &font, "A!B"), 14);

        // A missing page table makes every lookup fail (callers decide the
        // loud fallback; metrics just report no contribution).
        let mut pageless = font.clone();
        pageless.pages.clear();
        assert_eq!(glyph_advance(&atlas, &pageless, b'A'), None);
        assert_eq!(measure_text(&atlas, &pageless, "AB"), 0);
    }

    #[test]
    fn wrap_points_break_at_last_space() {
        let (atlas, font) = fixture();
        // "AA BB" with A=9 B=5: widths AA=18, space=9, BB=10 → total 37.
        // Max 20 fits "AA" (18) but not "AA " (27): wrap after the space.
        assert_eq!(wrap_points(&atlas, &font, "AA BB", 20), vec![3]);
        assert!(wrap_points(&atlas, &font, "AA BB", 40).is_empty());
    }

    #[test]
    fn text_quads_place_glyphs_in_draw_order() {
        let (atlas, font) = fixture();
        let quads = text_quads(
            &atlas,
            &font,
            [512.0, 32.0],
            "AB",
            [0.0, 0.0],
            [1.0; 4],
            |x, y, _w, _h| ([x, y, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        );
        assert_eq!(quads.len(), 2);
        // Bottom-left of A sits at the pen origin...
        assert_eq!(quads[0].vertices[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(quads[0].vertices[2].position, [9.0, 18.0, 0.0]);
        // ...B starts after A's advance and spans its own width.
        assert_eq!(quads[1].vertices[0].position, [9.0, 0.0, 0.0]);
        assert_eq!(quads[1].vertices[2].position, [14.0, 18.0, 0.0]);
        // UVs sample B's glyph column (start_u=16 over a 512-wide page).
        let uv_bl = quads[1].vertices[0].uv0;
        assert!((uv_bl[0] - (16.0 / 512.0)).abs() < 1e-6);
    }
}
