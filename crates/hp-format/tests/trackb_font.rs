//! SmallFont fixture round-trip: the Rust reader must reproduce the glyph
//! table decoded from a real serialized UFont (Engine.u export #3402).

use hp_format::font::{GlyphAtlas, read_ufont_payload};

const PAYLOAD: &[u8] = include_bytes!("../../../Tests/Fixtures/font/smallfont_payload.bin");

#[test]
fn smallfont_fixture_parses_exactly() {
    let expect: serde_json::Value = serde_json::from_str(include_str!(
        "../../../Tests/Fixtures/font/smallfont_expect.json"
    ))
    .expect("fixture json");

    let font = read_ufont_payload(PAYLOAD).expect("payload parses");
    assert_eq!(font.pages.len(), expect["pages"].as_u64().unwrap() as usize);
    assert_eq!(
        font.pages[0].texture_ref,
        expect["texture_ref"].as_i64().unwrap() as i32
    );
    assert_eq!(
        font.characters_per_page,
        expect["characters_per_page"].as_i64().unwrap() as i32
    );
    assert!(font.char_remap.is_empty());
    assert!(!font.is_remapped);

    let rects = &font.pages[0].characters;
    assert_eq!(rects.len(), 256, "one rect per codepoint");
    let expected_rects = expect["character_rects"].as_array().unwrap();
    for (index, (got, want)) in rects.iter().zip(expected_rects.iter()).enumerate() {
        let want = [
            want[0].as_i64().unwrap() as i32,
            want[1].as_i64().unwrap() as i32,
            want[2].as_i64().unwrap() as i32,
            want[3].as_i64().unwrap() as i32,
        ];
        assert_eq!(
            [got.start_u, got.start_v, got.u_size, got.v_size],
            want,
            "glyph {index}"
        );
    }
}

#[test]
fn stock_glyphs_have_real_widths_and_page_slot_math_holds() {
    let font = read_ufont_payload(PAYLOAD).expect("parses");
    // 190 printable glyphs carry nonzero widths; controls are zero-size.
    let nonzero = font.pages[0]
        .characters
        .iter()
        .filter(|rect| rect.u_size > 0)
        .count();
    assert_eq!(nonzero, 190);

    // 'A' = 65 lives on page 0 slot 65 with characters_per_page=256.
    assert_eq!(font.page_and_slot(b'A'), Some((0, 65)));
    // All 256 rects exist on stock fonts; glyph 255 is real but zero-width.
    assert_eq!(font.page_and_slot(255), Some((0, 255)));

    // A font whose single page holds fewer rects rejects higher codes.
    let short_font = hp_format::font::UFontData {
        pages: vec![hp_format::font::FontPage {
            texture_ref: 1,
            characters: vec![hp_format::font::GlyphRect {
                start_u: 0,
                start_v: 0,
                u_size: 8,
                v_size: 8,
            }],
        }],
        characters_per_page: 256,
        char_remap: Vec::new(),
        is_remapped: false,
    };
    assert_eq!(short_font.page_and_slot(1), None);
}
#[test]
fn atlas_advances_mirror_u_size_column() {
    let font = read_ufont_payload(PAYLOAD).expect("parses");
    let page = &font.pages[0];
    let atlas = GlyphAtlas::from_page(vec![0u8; 4096], &page.characters);
    assert_eq!(atlas.pixels.len(), 4096);
    assert_eq!(atlas.advances.len(), page.characters.len());
    for (advance, rect) in atlas.advances.iter().zip(&page.characters) {
        assert_eq!(*advance, rect.u_size);
    }
}

#[test]
fn truncated_payload_is_loud_not_silent() {
    for cut in [0usize, 1, 5, 100] {
        let err = read_ufont_payload(&PAYLOAD[..cut]).expect_err("must reject");
        let message = err.to_string();
        assert!(
            message.starts_with("font."),
            "reason-coded prefix: {message}"
        );
    }
}
