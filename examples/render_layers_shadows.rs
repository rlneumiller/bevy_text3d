// file: examples/render_layers_shadows.rs
use bevy::light::DirectionalLightShadowMap;
use bevy::light::OnlyShadowCaster;
use bevy::prelude::*;
use bevy_camera::visibility::RenderLayers;

// Layer indices used in examples to separate main camera layer (0) from shadow-only layer (1).
const DEFAULT_RENDER_LAYER: usize = 0;
const SHADOW_ONLY_LAYER: usize = 1;

// This example demonstrates the use of RenderLayers with shadows.
// It showcases how to set up a camera and directional light with shadow casting.
// The entities are organized into different render layers for visibility control.
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_systems(Startup, setup)
        .run();
}

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

    // Directional light with shadows and configured to affect both layer 0 (camera) and layer 1 (shadow-only)
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
    // Allow this directional light to participate in layers 0 (default camera) and 1 (shadow-only)
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
        // Ensure floor is in the default render layer
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    ));

    // Reference cube to show shadow direction
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.3),
            ..default()
        })),
        Transform::from_xyz(-2.0, 0.25, 0.0),
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    ));

    // Debug: Spawn a visible cube and an invisible OnlyShadowCaster cube on the shadow-only layer
    let debug_cube_handle = meshes.add(Cuboid::new(0.2, 0.2, 0.2));
    let visible_debug_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.8, 0.2),
        ..default()
    });
    let visible_debug = commands
        .spawn((
            Mesh3d(debug_cube_handle.clone()),
            MeshMaterial3d(visible_debug_material.clone()),
            Transform::from_xyz(0.5, 0.25, 0.0),
        ))
        .id();
    commands
        .entity(visible_debug)
        .insert(RenderLayers::layer(SHADOW_ONLY_LAYER));

    // Invisible shadow-only cube
    let shadow_debug_material = materials.add(StandardMaterial::default());
    let shadow_debug = commands
        .spawn((
            Mesh3d(debug_cube_handle.clone()),
            MeshMaterial3d(shadow_debug_material.clone()),
            Transform::from_xyz(0.0, 0.25, 0.0),
        ))
        .insert((OnlyShadowCaster, Visibility::Hidden))
        .id();
    commands
        .entity(shadow_debug)
        .insert(RenderLayers::layer(SHADOW_ONLY_LAYER));
}
