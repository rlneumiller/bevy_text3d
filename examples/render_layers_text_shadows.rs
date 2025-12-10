use bevy::light::{DirectionalLightShadowMap, OnlyShadowCaster};
use bevy::prelude::*;
use bevy_camera::visibility::RenderLayers;
use bevy_light::light_consts::lux;
use bevy_text3d::{
    Font, Glyph, GlyphProfileRenderMode, GlyphTessellationQuality, ShadowOnlyMaterial,
    ShadowOnlyMeshBundle, Text3d, Text3dConfig, Text3dPlugin, TextMeshPluginConfig,
    create_shadow_only_material,
};

// Layer indices used in examples to separate main camera layer (0) from shadow-only layer (1).
const DEFAULT_RENDER_LAYER: usize = 0;
const SHADOW_ONLY_LAYER: usize = 1;

// Example: Render shadows for 3D text where the shadow-caster mesh lives on a separate RenderLayer.
// Behavior:
// - Camera is on default layer (0) and sees the visible text and other objects.
// - Shadow-only child meshes (glyph profile meshes) are on RenderLayer 1 and hidden from camera.
// - Light affects both layers (0 and 1), so shadow-only geometry is included in shadow mapping.
// Implementation notes:
// - Use ShadowOnlyMeshBundle + OnlyShadowCaster for the shadow-caster child.
// - Insert `RenderLayers::from_layers(&[0, 1])` on the light, and `RenderLayers::layer(1)` on the shadow-only child.

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum AppState {
    #[default]
    Loading,
    Ready,
}

#[derive(Resource)]
struct FontHandle(Handle<Font>);

fn check_assets_loaded(
    asset_server: Res<AssetServer>,
    font_handle: Res<FontHandle>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if asset_server.is_loaded_with_dependencies(font_handle.0.id()) {
        next_state.set(AppState::Ready);
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(Text3dPlugin)
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .insert_resource(Text3dConfig {
            text_mesh_config: TextMeshPluginConfig {
                text_mesh_shadow_quality: GlyphTessellationQuality::High,
                font_scale: Vec3::ONE,
            },
        })
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            check_assets_loaded.run_if(in_state(AppState::Loading)),
        )
        .add_systems(OnEnter(AppState::Ready), spawn_text_when_loaded)
        .add_systems(
            Update,
            sync_shadow_casters.run_if(in_state(AppState::Ready)),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Load font and store handle for loading check
    let font_handle = asset_server.load("fonts/FiraCode-Bold.ttf");
    commands.insert_resource(FontHandle(font_handle.clone()));

    // Camera
    let camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 2.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .id();
    // For debugging, let the camera see both default and the shadow-only layer
    commands.entity(camera).insert(RenderLayers::from_layers(&[
        DEFAULT_RENDER_LAYER,
        SHADOW_ONLY_LAYER,
    ]));

    // Directional light with shadows enabled and affecting layers 0 and 1
    let light_entity = commands
        .spawn((
            DirectionalLight {
                illuminance: lux::FULL_DAYLIGHT,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.9, 0.0))
                .with_translation(Vec3::new(0.0, 5.0, 0.0)),
        ))
        .id();
    commands
        .entity(light_entity)
        .insert(RenderLayers::from_layers(&[
            DEFAULT_RENDER_LAYER,
            SHADOW_ONLY_LAYER,
        ]));

    // Simple floor to receive shadows
    let floor_handle = meshes.add(Plane3d::default().mesh().size(20.0, 20.0));
    commands.spawn((
        Mesh3d(floor_handle),
        MeshMaterial3d(materials.add(Color::srgb(0.2, 0.2, 0.2))),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    ));

    // A small cube on the main layer to help visualize the scene
    let cube_handle = meshes.add(Cuboid::new(0.5, 0.5, 0.5));
    commands.spawn((
        Mesh3d(cube_handle.clone()),
        MeshMaterial3d(materials.add(Color::srgb(0.6, 0.2, 0.2))),
        Transform::from_xyz(-1.5, 0.25, 0.0),
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    ));
}

fn spawn_text_when_loaded(
    mut commands: Commands,
    fonts: Res<Assets<Font>>,
    font_handle: Res<FontHandle>,
) {
    if let Some(font) = fonts.get(&font_handle.0) {
        // Build a short word
        let text = "SHADOWS".to_string();

        let mut text_cursor = Vec2::ZERO;
        let mut glyphs: Vec<Glyph> = Vec::new();
        for c in text.chars() {
            if let Some(info) = font.glyph(c) {
                let pos = bevy::math::Rect::from_corners(text_cursor, text_cursor + info.size);
                glyphs.push(Glyph {
                    position: pos,
                    character: c,
                    color: [1.0, 1.0, 1.0, 1.0],
                });
                text_cursor.x += info.advance.x + 0.02; // small gap
            }
        }

        let mut text3d = Text3d::new(font_handle.0.clone());
        text3d.set_glyphs(glyphs.into_boxed_slice());
        text3d.add_missing(&text.chars().collect::<Vec<_>>());
        // Disable glyph profile automatic handling - we will add our own shadow-only child
        text3d = text3d.with_glyph_profile_mode(GlyphProfileRenderMode::None);

        let entity = commands
            .spawn((
                text3d,
                Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(1.0)),
            ))
            .id();

        // info
        info!("Spawned text entity {:?}", entity);

        // The sync system will create the shadow-only child using the text's glyph_profile_mesh_handle
    }
}

fn sync_shadow_casters(
    mut commands: Commands,
    text_query: Query<(Entity, &Text3d, Option<&Children>)>,
    mut shadow_children: Query<&mut Mesh3d, With<MeshMaterial3d<ShadowOnlyMaterial>>>,
    mut shadow_materials: ResMut<Assets<ShadowOnlyMaterial>>,
) {
    for (entity, text3d, maybe_children) in text_query.iter() {
        let Some(profile_mesh) = text3d.glyph_profile_mesh_handle() else {
            continue;
        };

        // Find existing shadow caster child
        let existing_shadow_child = maybe_children.and_then(|children| {
            children
                .iter()
                .find(|child| shadow_children.get(*child).is_ok())
        });

        if let Some(shadow_child_entity) = existing_shadow_child {
            if let Ok(mut mesh3d) = shadow_children.get_mut(shadow_child_entity) {
                if mesh3d.0 != profile_mesh {
                    mesh3d.0 = profile_mesh.clone();
                }
            }
        } else {
            // Create a shadow-only material
            let material_handle =
                shadow_materials.add(create_shadow_only_material(StandardMaterial::default()));

            // Spawn shadow-only mesh child that casts shadows on layer 1, but is hidden from cameras
            let child = commands
                .spawn(
                    ShadowOnlyMeshBundle::new(profile_mesh.clone(), material_handle)
                        .with_transform(Transform::from_xyz(0.0, 0.0, -0.001)),
                )
                .insert((OnlyShadowCaster, Visibility::Hidden))
                .id();

            // Put the child on render layer 1 (shadow-only layer)
            commands.entity(child).insert(RenderLayers::layer(1));

            // Add as child
            commands.entity(entity).add_child(child);
        }
    }
}
