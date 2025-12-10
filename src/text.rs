use bevy::{app::Plugin, pbr::MaterialPlugin, prelude::*};

use crate::{
    font::FontPlugin,
    pipeline::{TextMeshPlugin, TextMeshPluginConfig},
    pipeline_material::{DepthOnlyMaterial, GlyphMaterial},
    shadow_casting::ShadowOnlyMaterialPlugin,
};

#[derive(Resource)]
pub struct Text3dConfig {
    pub text_mesh_config: TextMeshPluginConfig,
}

impl Default for Text3dConfig {
    fn default() -> Self {
        Self {
            text_mesh_config: Default::default(),
        }
    }
}

pub struct Text3dPlugin;

impl Text3dPlugin {
    /// Creates a new Text3dPlugin with custom configuration.
    /// Call this before adding the plugin to set the configuration.
    pub fn with_config(app: &mut App, config: TextMeshPluginConfig) -> &mut App {
        app.insert_resource(Text3dConfig {
            text_mesh_config: config,
        });
        app
    }
}

impl Plugin for Text3dPlugin {
    fn build(&self, app: &mut App) {
        let config = app
            .world()
            .get_resource::<Text3dConfig>()
            .map(|c| c.text_mesh_config.clone())
            .unwrap_or_default();
        app.add_plugins(FontPlugin)
            .add_plugins(TextMeshPlugin::with_config(config))
            .add_plugins(MaterialPlugin::<GlyphMaterial>::default())
            .add_plugins(MaterialPlugin::<DepthOnlyMaterial> {
                prepass_enabled: true,
                shadows_enabled: true,
                ..Default::default()
            })
            .add_plugins(ShadowOnlyMaterialPlugin);
    }
}
