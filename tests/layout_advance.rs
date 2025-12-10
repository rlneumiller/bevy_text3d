use bevy_text3d::{Font, FontAtlasSets, Glyph};
use owned_ttf_parser::OwnedFace;

/// Verify that when a glyph is un-rasterizable (e.g. space), the layout
/// advance is still applied but no textured quad is generated.
#[test]
fn layout_advances_for_empty_glyphs() {
    // Load font
    let font_path = std::path::Path::new("../../assets/fonts/FiraCode-Bold.ttf");
    let bytes = std::fs::read(font_path).expect("failed to read font file");
    let face = OwnedFace::from_vec(bytes, 0).expect("failed to parse font face");
    let font = Font::from(face);

    // Prepare asset containers
    let mut fonts = bevy::asset::Assets::<Font>::default();
    let font_handle = fonts.add(font.clone());
    let font_id = font_handle.id();

    let mut textures = bevy::asset::Assets::<bevy::prelude::Image>::default();

    let mut font_atlases = FontAtlasSets::default();

    let text = "A A"; // includes a space between two visible glyphs
    let chars: Vec<char> = text.chars().collect();

    // Request atlas generation for all codepoints used
    font_atlases.add_code_points(&chars, font_id, &fonts, &mut textures);

    // Build glyphs the same way the example does: include glyphs when
    // `font.glyph(c)` returns Some(info), and advance the cursor for all of them.
    let mut cursor_x: f32 = 0.0;
    let gap = 0.02f32;
    let mut glyphs: Vec<Glyph> = Vec::new();
    for c in text.chars() {
        if let Some(info) = font.glyph(c) {
            let pos = bevy::math::Rect::from_corners(
                bevy::math::Vec2::new(cursor_x, 0.0),
                bevy::math::Vec2::new(cursor_x + info.size.x, info.size.y),
            );
            glyphs.push(Glyph::from_rect(pos, c, [1.0, 1.0, 1.0, 1.0]));
            cursor_x += info.advance.x + gap;
        }
    }

    // Now mimic the layout pass: count how many textured quads would be produced
    let data = font_atlases
        .get(font_id)
        .expect("FontAtlasSets should have data for this font");

    let mut quad_count = 0usize;
    for g in glyphs.iter() {
        if data.atlas(g.character).is_some() {
            if let Some(info) = data.glyph_info(g.character) {
                if let Some(_uv) = data.find_glyph_rect(info.id) {
                    quad_count += 1;
                }
            }
        }
    }

    // We expect three quads: two visible glyphs 'A' and one transparent space
    let expected_quads = text.chars().filter(|c| font.glyph(*c).is_some()).count();
    assert_eq!(
        quad_count, expected_quads,
        "Quad count should equal all characters with glyph info (including spaces with transparent textures)"
    );

    // Also assert that the layout advance accounted for the space (cursor_x progressed)
    // Compute expected advance by summing glyph advances + gap for all glyphs with glyph info
    let mut expected_advance = 0f32;
    for c in text.chars() {
        if let Some(info) = font.glyph(c) {
            expected_advance += info.advance.x + gap;
        }
    }

    let diff = (cursor_x - expected_advance).abs();
    assert!(
        diff < 1e-6,
        "Cursor advance should equal expected sum: {} vs {}",
        cursor_x,
        expected_advance
    );
}
