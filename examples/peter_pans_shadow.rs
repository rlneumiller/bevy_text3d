// file: examples/peter_pans_shadow.rs
use bevy::light::{DirectionalLightShadowMap, NotShadowCaster, OnlyShadowCaster};
use bevy::prelude::*;
use bevy_camera::visibility::RenderLayers;

// Layer indices used in examples to separate main camera layer (0) from shadow-only layer (1).
const DEFAULT_RENDER_LAYER: usize = 0;
const SHADOW_ONLY_LAYER: usize = 1;

// This example demonstrates "Peter Pan's Shadow" effect.
// It shows how to have a visible entity whose shadow is independent of the visible entity.
// We achieve this by having two entities:
// 1. A visible entity that does not cast shadows (NotShadowCaster).
// 2. An invisible entity that only casts shadows (OnlyShadowCaster).
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_systems(Startup, setup)
        .add_systems(Update, move_peter_pan)
        .run();
}

#[derive(Component)]
struct PeterPanBody;

#[derive(Component)]
struct PeterPanShadow;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 2.5, 6.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        ))
        .id();
    commands.entity(camera).insert(RenderLayers::from_layers(&[
        DEFAULT_RENDER_LAYER,
        SHADOW_ONLY_LAYER,
    ]));

    // Directional light with shadows
    let dir_light = commands
        .spawn((
            DirectionalLight {
                illuminance: 10_000.0,
                shadows_enabled: true,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, -0.5, 0.0)),
        ))
        .id();

    commands
        .entity(dir_light)
        .insert(RenderLayers::from_layers(&[
            DEFAULT_RENDER_LAYER,
            SHADOW_ONLY_LAYER,
        ]));

    // Floor plane to receive shadows
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(10.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            ..default()
        })),
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    ));

    // "Peter Pan"
    // We use a capsule to represent a humanoid shape
    let peter_pan_mesh = meshes.add(Capsule3d::new(0.3, 1.0));
    let peter_pan_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.8, 0.2), // Green for Peter Pan
        ..default()
    });

    // 1. The Visible Body (No Shadow)
    commands.spawn((
        Mesh3d(peter_pan_mesh.clone()),
        MeshMaterial3d(peter_pan_material.clone()),
        Transform::from_xyz(0.0, 1.0, 0.0),
        PeterPanBody,
        NotShadowCaster, // Don't cast a normal shadow
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    ));

    // 2. The Independent Shadow (Invisible, Shadow Only)
    commands.spawn((
        Mesh3d(peter_pan_mesh),
        MeshMaterial3d(peter_pan_material), // Material doesn't matter much for shadow caster, but keeping it same is fine
        Transform::from_xyz(0.0, 1.0, 0.0),
        PeterPanShadow,
        OnlyShadowCaster,
        Visibility::Hidden,
        RenderLayers::layer(SHADOW_ONLY_LAYER),
    ));

    // A visible reference object to show that normal objects work as expected
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.3),
            ..default()
        })),
        Transform::from_xyz(-2.0, 0.25, 0.0),
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    ));
}

// Animate Peter Pan and his shadow independently
fn move_peter_pan(
    time: Res<Time>,
    mut body_query: Query<&mut Transform, (With<PeterPanBody>, Without<PeterPanShadow>)>,
    mut shadow_query: Query<&mut Transform, (With<PeterPanShadow>, Without<PeterPanBody>)>,
) {
    let t = time.elapsed_secs();

    // Move Body: Bob up and down, move in a circle
    if let Some(mut body_transform) = body_query.iter_mut().next() {
        body_transform.translation.x = t.sin() * 2.0;
        body_transform.translation.z = t.cos() * 2.0;
        body_transform.translation.y = 1.0 + (t * 3.0).sin() * 0.5;
    }

    // Move Shadow: Follows x/z but stays on the ground (or lags behind)
    if let Some(mut shadow_transform) = shadow_query.iter_mut().next() {
        // Shadow tries to run away! (Offset phase)
        let shadow_t = t - 0.5;
        shadow_transform.translation.x = shadow_t.sin() * 2.0;
        shadow_transform.translation.z = shadow_t.cos() * 2.0;

        // Shadow stays grounded while body flies
        shadow_transform.translation.y = 0.3; // Slightly above floor to avoid z-fighting if it were visible, but it's invisible so it doesn't matter much, but good for shadow casting origin.

        // Rotate the shadow independently?
        shadow_transform.rotation = Quat::from_rotation_y(t * 2.0);
    }
}
