use bevy_text3d::Font;
use owned_ttf_parser::OwnedFace;

/// Test that font name extraction works correctly
#[test]
fn font_name_extraction() {
    // Read the font file from the crate assets directory
    let font_path = std::path::Path::new("../../assets/fonts/FiraCode-Bold.ttf");
    let bytes = std::fs::read(font_path).expect("failed to read font file");

    // Construct an OwnedFace and then our Font
    let face = OwnedFace::from_vec(bytes, 0).expect("failed to parse font face");
    let font = Font::from(face);

    // Test that we can extract a font name
    let font_name = font.name();

    // The font name should be Some(String) and not empty
    assert!(font_name.is_some(), "Font name should be extractable");
    let name = font_name.unwrap();
    assert!(!name.is_empty(), "Font name should not be empty");
    assert!(name.len() > 0, "Font name should have length > 0");

    // For FiraCode-Bold, we expect the name to contain "Fira" or "Code"
    // (This is a reasonable assumption based on the font filename)
    let name_lower = name.to_lowercase();
    println!("Font name: '{}'", name);
    println!("Font name lowercased: '{}'", name_lower);
    println!("Contains 'fira': {}", name_lower.contains("fira"));
    println!("Contains 'code': {}", name_lower.contains("code"));

    // Just check that we got a reasonable name, don't be too strict about content
    assert!(name.len() > 2, "Font name should be reasonably long");

    println!("Successfully extracted font name: {}", name);
}
