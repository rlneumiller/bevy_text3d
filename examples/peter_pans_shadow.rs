// file: examples/peter_pans_shadow.rs
use bevy::light::{DirectionalLightShadowMap, NotShadowCaster, OnlyShadowCaster};
use bevy::prelude::*;
use bevy::gltf::GltfAssetLabel;
use bevy::scene::{SceneRoot, SceneInstanceReady};
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

#[derive(Component, Clone)]
struct AnimationToPlay {
    graph_handle: Handle<AnimationGraph>,
    index: AnimationNodeIndex,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
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
    // Load the animated GLB scene (scene 0) and the first animation (if present)
    // NOTE: asset paths are relative to the `assets/` directory
    // Use GltfAssetLabel helpers to target sub-assets (Scene 0 and Animation 0)
    // Use the Fox model included in bevy_0.17 known-good example assets
    let gltf_path: &str = "models/animated/Fox.glb";
    let glb_scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(gltf_path));
    let glb_animation = asset_server.load(GltfAssetLabel::Animation(0).from_asset(gltf_path));
    let glb_scene_scale = 0.025;

    // 1. The Visible Body (No Shadow)
    // Create AnimationGraph from the animation clip and store the graph handle
    let (graph, index): (AnimationGraph, AnimationNodeIndex) =
        AnimationGraph::from_clip(glb_animation.clone());
    let graph_handle = graphs.add(graph);

    let animation_component = AnimationToPlay { graph_handle: graph_handle.clone(), index };

    commands.spawn((
        SceneRoot(glb_scene.clone()),
        Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(glb_scene_scale)),
        PeterPanBody,
        // NotShadowCaster will be applied to descendant mesh entities in a SceneInstanceReady handler
        RenderLayers::layer(DEFAULT_RENDER_LAYER),
    )).observe(play_peter_pan_when_ready);

    // 2. The Independent Shadow (Invisible, Shadow Only)
    commands.spawn((
        SceneRoot(glb_scene.clone()),
        Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(glb_scene_scale)),
        PeterPanShadow,
        // We'll apply OnlyShadowCaster + Hidden to descendants when the scene is ready
        RenderLayers::layer(SHADOW_ONLY_LAYER),
        animation_component,
    )).observe(play_peter_pan_when_ready);

    // Nothing else needed here; the AnimationToPlay component will be used when the scene instance is ready

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

/// Triggered when a scene instance is spawned; this will find AnimationPlayer
/// components in the scene and start the requested animation, and also apply
/// shadow-related components (NotShadowCaster / OnlyShadowCaster) to mesh
/// children depending on whether the root is a `PeterPanBody` or `PeterPanShadow`.
fn play_peter_pan_when_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    animation_query: Query<&AnimationToPlay>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    body_query: Query<&PeterPanBody>,
    shadow_query: Query<&PeterPanShadow>,
    ) {
    // Optionally get the AnimationToPlay; it's not required for apply the shadow components
    let anim = animation_query.get(scene_ready.entity).ok();
    for child in children.iter_descendants(scene_ready.entity) {
        // Start animations on any AnimationPlayer only if AnimationToPlay exists
        if let Some(animation_to_play) = anim {
            if let Ok(mut player) = players.get_mut(child) {
                player.play(animation_to_play.index).repeat();
                // Connect the animation graph to the player
                commands
                    .entity(child)
                    .insert(AnimationGraphHandle(animation_to_play.graph_handle.clone()));
            }
        }

        // Apply shadow components to mesh nodes by checking descendant entities independent of animation
        if body_query.get(scene_ready.entity).is_ok() {
            // Apply NotShadowCaster to all descendants so the visible mesh doesn't cast a shadow.
            // We can insert the component on every descendant; it only affects renderables.
            commands.entity(child).insert(NotShadowCaster);
        }
        if shadow_query.get(scene_ready.entity).is_ok() {
            // For shadow root, ensure children are only shadow-casters and hidden from camera
            commands.entity(child).insert((OnlyShadowCaster, Visibility::Hidden, RenderLayers::layer(SHADOW_ONLY_LAYER)));
        }
    }
}

// Animate Peter Pan and his shadow independently
fn move_peter_pan(
    time: Res<Time>,
    mut body_query: Query<&mut Transform, (With<PeterPanBody>, Without<PeterPanShadow>)>,
    mut shadow_query: Query<&mut Transform, (With<PeterPanShadow>, Without<PeterPanBody>)>,
) {
    let t = time.elapsed_secs();

    // Body stays stationary -- we intentionally do not move the visible Peter Pan body

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
