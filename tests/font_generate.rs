use bevy_text3d::Font;
use owned_ttf_parser::OwnedFace;

// This is a headless test that runs without opening a window.
#[test]
fn font_generate_space_and_h() {
    // Read the font file from the workspace assets directory
    let font_path = std::path::Path::new("../../assets/fonts/FiraCode-Bold.ttf");
    let bytes = std::fs::read(font_path).expect("failed to read font file");

    // Construct an OwnedFace and then our Font
    let face = OwnedFace::from_vec(bytes, 0).expect("failed to parse font face");
    let font = Font::from(face);

    // Choose codepoints to test
    let cp_space = ' ';
    let cp_h = 'H';

    // Query glyph info (should be Some for both if glyph index exists)
    let glyph_space = font.glyph(cp_space).expect("glyph info for space missing");
    let glyph_h = font.glyph(cp_h).expect("glyph info for H missing");

    // Generate rasterized SDF images with a small padding range
    let img_space = font.generate(glyph_space.id, cp_space, 5.0);
    let img_h = font.generate(glyph_h.id, cp_h, 5.0);

    // Space has no bounding box -> generate returns Some with transparent placeholder
    assert!(
        img_space.is_some(),
        "Expected space glyph to generate Some (transparent placeholder), got None"
    );
    // H should generate Some(Image)
    assert!(img_h.is_some(), "Expected 'H' glyph to generate an image");
}
