// file: examples/peter_pans_shadow.rs
use avian3d::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::gltf::GltfAssetLabel;
use bevy::light::{DirectionalLightShadowMap, NotShadowCaster, OnlyShadowCaster};
use bevy::prelude::*;
use bevy::scene::{SceneInstanceReady, SceneRoot};
use bevy_camera::visibility::RenderLayers;
// The `Gltf` asset type is re-exported by the engine prelude when the `gltf` feature is enabled
use bevy::animation::AnimationClip;
use bevy::gltf::Gltf;
use bevy_text3d::grounding::compute_ground_offset;

// Layer indices used in examples to separate main camera layer (0) from shadow-only layer (1).
const MAIN_CAMERA_LAYER: usize = 0;
const SHADOW_ONLY_LAYER: usize = 1;

// This example demonstrates "Peter Pan's Shadow" effect.
// It shows how to have a visible entity whose shadow is independent of the visible entity.
// We achieve this by having two entities:
// 1. A visible entity that does not cast shadows (NotShadowCaster).
// 2. An invisible entity that only casts shadows (OnlyShadowCaster).
fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, PhysicsPlugins::default()))
        .insert_resource(PeterPanGrounding { offset_y: None })
        .init_resource::<PeterPanEntities>()
        .init_resource::<AnimationOverrideState>()
        .insert_resource(DirectionalLightShadowMap { size: 4096 })
        .add_systems(Startup, setup)
        .add_systems(Update, move_peter_pan)
        .add_systems(Update, try_apply_named_animations)
        .add_systems(Update, toggle_shadows_on_key_s)
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
        MAIN_CAMERA_LAYER,
        SHADOW_ONLY_LAYER,
    ]));

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
            MAIN_CAMERA_LAYER,
            SHADOW_ONLY_LAYER,
        ]));

    // Floor plane to receive shadows
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(10.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            ..default()
        })),
        RenderLayers::layer(MAIN_CAMERA_LAYER),
    ));

    // "Peter Pan"
    // Load the animated GLB scene (scene 0) and the first animation (if present)
    // NOTE: asset paths are relative to the `assets/` directory
    // Use GltfAssetLabel helpers to target sub-assets (Scene 0 and Animation 0)
    // Use the Fox model included in bevy_0.17 known-good example assets
    let gltf_path: &str = "models/animated/Fox.glb";
    let glb_scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(gltf_path));
    // Also load the main Gltf to discover named animations if present
    let gltf_asset = asset_server.load(gltf_path);
    // We'll load both the survey and run animations by index. If your model
    // provides named animations, replace these indices with the correct ones
    // or use the `Gltf` asset and discover names at runtime.
    let glb_survey_anim = asset_server.load(GltfAssetLabel::Animation(0).from_asset(gltf_path));
    let glb_run_anim = asset_server.load(GltfAssetLabel::Animation(2).from_asset(gltf_path));
    let glb_scene_scale = 0.025;

    // 1. The Visible Body (No Shadow)
    // Create AnimationGraph from the animation clip and store the graph handle
    let (survey_graph, survey_index): (AnimationGraph, AnimationNodeIndex) =
        AnimationGraph::from_clip(glb_survey_anim.clone());
    let survey_graph_handle = graphs.add(survey_graph);
    let survey_animation = AnimationToPlay {
        graph_handle: survey_graph_handle.clone(),
        index: survey_index,
    };

    let body_entity = commands
        .spawn((
            SceneRoot(glb_scene.clone()),
            Transform::from_xyz(0.0, 0.1217422, 0.0).with_scale(Vec3::splat(glb_scene_scale)),
            PeterPanBody,
            survey_animation,
            // NotShadowCaster will be applied to descendant mesh entities in a SceneInstanceReady handler
            RenderLayers::layer(MAIN_CAMERA_LAYER),
        ))
        .observe(play_peter_pan_when_ready)
        .id();

    // 2. The Independent Shadow (Invisible, Shadow Only)
    // Create AnimationGraph for 'run' and attach to the shadow entity
    let (run_graph, run_index): (AnimationGraph, AnimationNodeIndex) =
        AnimationGraph::from_clip(glb_run_anim.clone());
    let run_graph_handle = graphs.add(run_graph);
    let run_animation = AnimationToPlay {
        graph_handle: run_graph_handle.clone(),
        index: run_index,
    };

    let shadow_entity = commands
        .spawn((
            SceneRoot(glb_scene.clone()),
            Transform::from_xyz(0.0, 0.1217422, 0.0).with_scale(Vec3::splat(glb_scene_scale)),
            PeterPanShadow,
            // We'll apply OnlyShadowCaster + Hidden to descendants when the scene is ready
            RenderLayers::layer(SHADOW_ONLY_LAYER),
            run_animation,
        ))
        .observe(play_peter_pan_when_ready)
        .id();

    // Store the entity ids so that we can patch them later if we discover named
    // animations from the loaded `Gltf` asset.
    commands.insert_resource(PeterPanEntities {
        body: Some(body_entity),
        shadow: Some(shadow_entity),
        gltf_handle: gltf_asset,
    });

    // The AnimationToPlay component will be used when the scene instance is ready
    // A visible reference object to show that normal objects work as expected
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.3),
            ..default()
        })),
        Transform::from_xyz(-2.0, 0.25, 0.0),
        RenderLayers::layer(MAIN_CAMERA_LAYER),
    ));
}

/// Triggered when a scene instance is spawned; this will find AnimationPlayer
/// components in the scene and start the requested animation, and also apply
/// shadow-related components (NotShadowCaster / OnlyShadowCaster) to mesh
/// children depending on whether the root is a `PeterPanBody` or `PeterPanShadow`.
///
fn play_peter_pan_when_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    animation_query: Query<&AnimationToPlay>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    body_query: Query<&PeterPanBody>,
    shadow_query: Query<&PeterPanShadow>,
    mut transforms: Query<&mut Transform>,
    global_aabb_query: Query<(&GlobalTransform, &Aabb)>,
    mut grounding: ResMut<PeterPanGrounding>,
) {
    // Optionally get the AnimationToPlay; it's not required to apply the shadow components
    let anim = animation_query.get(scene_ready.entity).ok();
    // If this is the visible body root, compute the minimum Y of all descendant AABBs
    if body_query.get(scene_ready.entity).is_ok() {
        if let Some((min_world_y, offset)) =
            compute_ground_offset(scene_ready.entity, &children, &global_aabb_query, 0.0)
        {
            // Ground plane is at y = 0.0. We'll lower the body so the minimum point sits on the ground
            if let Ok(mut t) = transforms.get_mut(scene_ready.entity) {
                t.translation.y += offset;
                info!(
                    "Peter Pan body root translation after offset: {}",
                    t.translation.y
                );
            } else {
                // Fallback in case the transform was not present for some reason
                commands
                    .entity(scene_ready.entity)
                    .insert(Transform::from_translation(Vec3::new(0.0, offset, 0.0)));
            }
            info!("Peter Pan body min_y: {} offset: {}", min_world_y, offset);
            grounding.offset_y = Some(offset);
            let expected_min_after = min_world_y + offset;
            info!(
                "Peter Pan body new min_y (expected after applying offset): {}",
                expected_min_after
            );
        }
    }

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
            commands.entity(child).insert((
                OnlyShadowCaster,
                Visibility::Hidden,
                RenderLayers::layer(SHADOW_ONLY_LAYER),
            ));
        }
    }

    // If this is the shadow root and we already computed a body offset, apply the same offset
    if shadow_query.get(scene_ready.entity).is_ok() {
        if let Some(offset) = grounding.offset_y {
            // Apply the same offset so the visible body and the shadow instance sit at the same baseline.
            // Add a tiny additional offset for the shadow caster so its skin doesn't Z-fight with the plane.
            let shadow_lift = 0.05;
            if let Ok(mut t) = transforms.get_mut(scene_ready.entity) {
                t.translation.y += offset + shadow_lift;
                info!(
                    "Peter Pan shadow root translation after offset+lift: {}",
                    t.translation.y
                );
            } else {
                commands
                    .entity(scene_ready.entity)
                    .insert(Transform::from_translation(Vec3::new(
                        0.0,
                        offset + shadow_lift,
                        0.0,
                    )));
            }
            info!(
                "Peter Pan shadow: applied offset {} + lift {}",
                offset, shadow_lift
            );
        }
    }
}

// Animate Peter Pan and his shadow independently
fn move_peter_pan(
    time: Res<Time>,
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

        // Shadow stays grounded while body flies: do not override the initial Y value

        // Rotate the shadow independently?
        shadow_transform.rotation = Quat::from_rotation_y(t * 2.0);
    }
}

// Toggle shadows between visible fox and invisible fox on KeyS press
fn toggle_shadows_on_key_s(
    mut keyboard_input: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    body_entities: Query<Entity, With<PeterPanBody>>,
    shadow_entities: Query<Entity, With<PeterPanShadow>>,
    children: Query<&Children>,
    mut state: Local<bool>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyS) {
        keyboard_input.clear_just_pressed(KeyCode::KeyS);
        *state = !*state;

        if *state {
            // Swap: Visible fox shadow disabled, invisible fox shadow enabled
            // Add OnlyShadowCaster to body's descendants, remove NotShadowCaster
            for body_entity in &body_entities {
                for child_entity in children.iter_descendants(body_entity) {
                    commands.entity(child_entity).remove::<NotShadowCaster>();
                    commands.entity(child_entity).insert(OnlyShadowCaster);
                }
            }

            // Add NotShadowCaster to shadow's descendants, remove OnlyShadowCaster
            for shadow_entity in &shadow_entities {
                for child_entity in children.iter_descendants(shadow_entity) {
                    commands.entity(child_entity).insert(NotShadowCaster);
                    commands.entity(child_entity).remove::<OnlyShadowCaster>();
                }
            }
        } else {
            // Restore: Visible fox shadow enabled, invisible fox shadow disabled
            // Add NotShadowCaster to body's descendants, remove OnlyShadowCaster
            for body_entity in &body_entities {
                for child_entity in children.iter_descendants(body_entity) {
                    commands.entity(child_entity).insert(NotShadowCaster);
                    commands.entity(child_entity).remove::<OnlyShadowCaster>();
                }
            }

            // Add OnlyShadowCaster to shadow's descendants, remove NotShadowCaster
            for shadow_entity in &shadow_entities {
                for child_entity in children.iter_descendants(shadow_entity) {
                    commands.entity(child_entity).remove::<NotShadowCaster>();
                    commands.entity(child_entity).insert(OnlyShadowCaster);
                }
            }
        }
    }
}

// Resource used to store the computed Y offset when the visible body is grounded
#[derive(Resource)]
struct PeterPanGrounding {
    offset_y: Option<f32>,
}

#[derive(Resource, Default)]
struct PeterPanEntities {
    body: Option<Entity>,
    shadow: Option<Entity>,
    gltf_handle: Handle<Gltf>,
}

#[derive(Resource, Default)]
struct AnimationOverrideState {
    applied: bool,
}

fn try_apply_named_animations(
    mut commands: Commands,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    entities: Res<PeterPanEntities>,
    mut override_state: ResMut<AnimationOverrideState>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    if override_state.applied {
        return;
    }
    let gltf_handle = &entities.gltf_handle;
    if let Some(gltf) = gltfs.get(gltf_handle) {
        // Try to find named animations "survey" / "Survey"
        if let Some(body_entity) = entities.body {
            if let Some(anim_handle) = gltf
                .named_animations
                .get("Survey")
                .or_else(|| gltf.named_animations.get("survey"))
            {
                let anim_clip: Handle<AnimationClip> = anim_handle.clone();
                let (graph, idx) = AnimationGraph::from_clip(anim_clip);
                let graph_handle = graphs.add(graph);
                commands.entity(body_entity).insert(AnimationToPlay {
                    graph_handle: graph_handle.clone(),
                    index: idx,
                });
                // Start the anim on any existing players in case the scene already spawned
                for child in children.iter_descendants(body_entity) {
                    if let Ok(mut player) = players.get_mut(child) {
                        player.play(idx).repeat();
                        commands
                            .entity(child)
                            .insert(AnimationGraphHandle(graph_handle.clone()));
                    }
                }
                info!("Applied named 'Survey' animation to body. index: {:?}", idx);
            }
        }
        // Run for shadow
        if let Some(shadow_entity) = entities.shadow {
            if let Some(anim_handle) = gltf
                .named_animations
                .get("Run")
                .or_else(|| gltf.named_animations.get("run"))
            {
                let anim_clip: Handle<AnimationClip> = anim_handle.clone();
                let (graph, idx) = AnimationGraph::from_clip(anim_clip);
                let graph_handle = graphs.add(graph);
                commands.entity(shadow_entity).insert(AnimationToPlay {
                    graph_handle: graph_handle.clone(),
                    index: idx,
                });
                for child in children.iter_descendants(shadow_entity) {
                    if let Ok(mut player) = players.get_mut(child) {
                        player.play(idx).repeat();
                        commands
                            .entity(child)
                            .insert(AnimationGraphHandle(graph_handle.clone()));
                    }
                }
                info!("Applied named 'Run' animation to shadow. index: {:?}", idx);
            }
        }
        override_state.applied = true;
    }
}
