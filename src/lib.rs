pub mod dump_glyph_profile;
pub mod font;
mod pipeline;
mod pipeline_material;
pub mod shadow_casting;
pub mod tessellation;
mod text;

pub use font::{Font, FontAtlasSet, FontAtlasSets};
// Re-export Bevy's `OnlyShadowCaster` so examples and other crates can import from `bevy_text3d`.
pub use bevy::light::OnlyShadowCaster;
pub use pipeline::{
    Glyph, GlyphProfileRenderMode, GlyphTessellationQuality, Text3d, TextMeshPluginConfig,
};
pub use pipeline_material::DepthOnlyMaterial;
pub use pipeline_material::GlyphMaterial;
pub use shadow_casting::{
    NoColorExt, ShadowOnlyMaterial, ShadowOnlyMaterialPlugin, ShadowOnlyMeshBundle,
    create_shadow_only_material,
};
pub use text::{Text3dConfig, Text3dPlugin};
