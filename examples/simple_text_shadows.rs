use bevy::light::{DirectionalLightShadowMap, NotShadowCaster};
use bevy::prelude::*;
use bevy_text3d::{
    Font, Glyph, GlyphProfileRenderMode, ShadowOnlyMaterial, ShadowOnlyMeshBundle, Text3d,
    Text3dPlugin, create_shadow_only_material,
};

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum AppState {
    #[default]
    Loading,
    Ready,
}

#[derive(Resource)]
struct FontHandle(Handle<Font>);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(Text3dPlugin)
        .insert_resource(DirectionalLightShadowMap { size: 2048 })
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            check_font_loaded.run_if(in_state(AppState::Loading)),
        )
        .add_systems(OnEnter(AppState::Ready), spawn_text)
        .add_systems(
            Update,
            sync_shadow_casters.run_if(in_state(AppState::Ready)),
        );

    app.run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Store font handle for loading check
    let font_handle = asset_server.load("fonts/FiraCode-Bold.ttf");
    commands.insert_resource(FontHandle(font_handle));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.5, 6.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));

    // Directional light with shadows
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, -0.5, 0.0)),
    ));

    // Floor plane to receive shadows
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(10.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            ..default()
        })),
    ));

    // Reference cube to show shadow direction
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.3),
            ..default()
        })),
        Transform::from_xyz(-2.0, 0.25, 0.0),
    ));
}

fn check_font_loaded(
    asset_server: Res<AssetServer>,
    font_handle: Res<FontHandle>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if asset_server.is_loaded_with_dependencies(font_handle.0.id()) {
        next_state.set(AppState::Ready);
    }
}

fn spawn_text(mut commands: Commands, fonts: Res<Assets<Font>>, font_handle: Res<FontHandle>) {
    let Some(font) = fonts.get(font_handle.0.id()) else {
        error!("Font not loaded when entering Ready state");
        return;
    };

    // Create simple text
    let text = "SHADOWS";
    let mut text_cursor = Vec2::ZERO;
    let mut glyphs = Vec::new();

    for c in text.chars() {
        if let Some(info) = font.glyph(c) {
            let pos = bevy::math::Rect::from_corners(text_cursor, text_cursor + info.size);
            glyphs.push(Glyph {
                position: pos,
                character: c,
                color: [0.0, 0.0, 1.0, 1.0], // Blue text
            });
            text_cursor.x += info.advance.x + 0.02;
        }
    }

    // Spawn Text3d entity
    let mut text_mesh = Text3d::new(font_handle.0.clone());
    text_mesh.set_glyphs(glyphs.into_boxed_slice());
    let codepoints: Vec<char> = text.chars().collect();
    text_mesh.add_missing(&codepoints);

    // Set glyph profile mode to None since we're handling shadow casting ourselves
    text_mesh = text_mesh.with_glyph_profile_mode(GlyphProfileRenderMode::None);

    commands.spawn((text_mesh, Transform::from_xyz(-3.5, 1.0, 2.0)));

    info!("Text spawned - shadows should appear when glyph profile mesh is generated");
}

/// Synchronizes shadow-casting child entities for each Text3d entity.
///
/// This system spawns invisible child entities that use ShadowOnlyMaterial to cast
/// accurate shadow silhouettes matching the text character outlines.
/// Shadow casting can be disabled for individual Text3d entities by adding the NotShadowCaster component.
fn sync_shadow_casters(
    mut commands: Commands,
    text_query: Query<(Entity, &Text3d, Option<&Children>, Option<&NotShadowCaster>)>,
    shadow_children: Query<&Mesh3d, With<MeshMaterial3d<ShadowOnlyMaterial>>>,
    mut shadow_materials: ResMut<Assets<ShadowOnlyMaterial>>,
) {
    for (entity, text3d, maybe_children, not_shadow_caster) in text_query.iter() {
        // Get the glyph profile mesh (outline mesh for shadow casting)
        let Some(profile_mesh) = text3d.glyph_profile_mesh_handle() else {
            continue;
        };

        // Check if we already have a shadow child
        let has_shadow_child = maybe_children
            .map(|children| {
                children
                    .iter()
                    .any(|child| shadow_children.get(child).is_ok())
            })
            .unwrap_or(false);

        let should_cast_shadows = not_shadow_caster.is_none();

        if should_cast_shadows && !has_shadow_child {
            // Create shadow-only material
            let material =
                shadow_materials.add(create_shadow_only_material(StandardMaterial::default()));

            // Spawn invisible glyph_mesh child that casts shadows
            let child = commands
                .spawn(
                    ShadowOnlyMeshBundle::new(profile_mesh.clone(), material)
                        .with_transform(Transform::from_xyz(0.0, 0.0, -0.001)),
                )
                .id();

            commands.entity(entity).add_child(child);
            info!("Shadow caster child spawned for Text3d entity");
        } else if !should_cast_shadows && has_shadow_child {
            // Remove shadow children when NotShadowCaster is added
            if let Some(children) = maybe_children {
                for child in children.iter() {
                    if shadow_children.get(child).is_ok() {
                        commands.entity(child).despawn();
                    }
                }
            }
        }
    }
}
