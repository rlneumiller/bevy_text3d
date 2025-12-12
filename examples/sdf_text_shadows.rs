use bevy::{
    light::{DirectionalLightShadowMap, OnlyShadowCaster},
    prelude::*,
};
use bevy_camera::visibility::RenderLayers;
use bevy_text3d::{
    Font, Glyph, GlyphProfileRenderMode, GlyphTessellationQuality, ShadowOnlyMaterial, Text3d,
    Text3dConfig, Text3dPlugin, TextMeshPluginConfig, create_shadow_only_material,
};

use bevy_light::light_consts::lux;

// Layer indices used in examples to separate main camera layer (0) from shadow-only layer (1).
#[allow(dead_code)]
const MAIN_CAMERA_LAYER: usize = 0;
const SHADOW_ONLY_LAYER: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShadowQuality {
    UltraHigh, // FiraCode-Bold.ttf 'S' 2444 vertices and 2442 triangles
    VeryHigh,  // FiraCode-Bold.ttf 'S' 771 vertices and 769 triangles
    High,      // FiraCode-Bold.ttf 'S' 263 vertices and 261 triangles
    Medium,    // FiraCode-Bold.ttf 'S' 94 vertices and 92 triangles
    Low,       // FiraCode-Bold.ttf 'S' 53 vertices and 51 triangles
    VeryLow,   // FiraCode-Bold.ttf 'S' 35 vertices and 33 triangles
    Minimal,   // FiraCode-Bold.ttf 'S' 17 vertices and 15 triangles
}

impl ShadowQuality {
    fn all_variants() -> &'static [Self] {
        &[
            Self::UltraHigh,
            Self::VeryHigh,
            Self::High,
            Self::Medium,
            Self::Low,
            Self::VeryLow,
            Self::Minimal,
        ]
    }

    fn next(&self) -> Self {
        let variants = Self::all_variants();
        let current_index = variants.iter().position(|&v| v == *self).unwrap_or(0);
        let next_index = (current_index + 1) % variants.len();
        variants[next_index]
    }

    fn prev(&self) -> Self {
        let variants = Self::all_variants();
        let current_index = variants.iter().position(|&v| v == *self).unwrap_or(0);
        let prev_index = if current_index == 0 {
            variants.len() - 1
        } else {
            current_index - 1
        };
        variants[prev_index]
    }

    fn to_glyph_quality(&self) -> GlyphTessellationQuality {
        match self {
            Self::UltraHigh => GlyphTessellationQuality::UltraHigh,
            Self::VeryHigh => GlyphTessellationQuality::VeryHigh,
            Self::High => GlyphTessellationQuality::High,
            Self::Medium => GlyphTessellationQuality::Medium,
            Self::Low => GlyphTessellationQuality::Low,
            Self::VeryLow => GlyphTessellationQuality::VeryLow,
            Self::Minimal => GlyphTessellationQuality::Minimal,
        }
    }
}

impl Default for ShadowQuality {
    fn default() -> Self {
        Self::Minimal
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum AppState {
    #[default]
    Loading,
    Ready,
}

#[derive(Resource)]
struct CurrentShadowQuality(ShadowQuality);

impl Default for CurrentShadowQuality {
    fn default() -> Self {
        Self(ShadowQuality::default())
    }
}

#[derive(Resource)]
struct FontHandle(Handle<Font>);

fn check_assets_loaded(
    asset_server: Res<AssetServer>,
    font_handle: Res<FontHandle>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let load_state = asset_server.get_load_state(font_handle.0.id());
    debug!("Font load state: {:?}", load_state);
    if asset_server.is_loaded_with_dependencies(font_handle.0.id()) {
        debug!("Font loaded, transitioning to Ready state");
        next_state.set(AppState::Ready);
    }
}

// TODO: Consider adding the option to precompute Signed Distance Fields (SDFs) and export them for static in-game text and load them as assets.

fn main() {
    let mut app = App::new();
    app.insert_resource(DirectionalLightShadowMap { size: 4096 });

    app.add_plugins(DefaultPlugins)
        .insert_resource(Text3dConfig {
            text_mesh_config: TextMeshPluginConfig {
                text_mesh_shadow_quality: GlyphTessellationQuality::High, // Reasonably smooth shadow outlines
                font_scale: Vec3::ONE,
            },
        })
        .insert_resource(CurrentShadowQuality(ShadowQuality::High))
        .add_plugins(Text3dPlugin)
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
        .add_systems(
            Update,
            debug_log_shadow_casters.run_if(in_state(AppState::Ready)),
        )
        .add_systems(
            Update,
            handle_quality_input.run_if(in_state(AppState::Ready)),
        )
        .add_systems(
            Update,
            update_shadow_quality.run_if(in_state(AppState::Ready)),
        )
        .run();
}

// If other cameras/lights are added later, render layers mitigate
// introducing unintended visibility.
// Shadow casters are now implemented using the `OnlyShadowCaster` component
// which allows meshes to be excluded from camera visibility while still
// participating in shadow map rendering.

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let font_handle = asset_server.load("fonts/FiraCode-Bold.ttf");

    // Store the font handle for the loading check system
    commands.insert_resource(FontHandle(font_handle.clone()));

    #[cfg(debug_assertions)]
    let load_state = asset_server.get_load_state(font_handle.id());
    #[cfg(debug_assertions)]
    debug!("Font load state: {:?}", load_state);

    let camera = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        ))
        .id();
    // For debugging, let the camera see both the default and the shadow-only layer
    commands.entity(camera).insert(RenderLayers::from_layers(&[
        DEFAULT_RENDER_LAYER,
        SHADOW_ONLY_LAYER,
    ]));

    const DEFAULT_RENDER_LAYER: usize = 0;
    const SHADOW_ONLY_LAYER: usize = 1;

    let dir_light_entity =
        commands
            .spawn((
                DirectionalLight {
                    illuminance: lux::FULL_DAYLIGHT,
                    shadows_enabled: true,
                    ..Default::default()
                },
                Transform::from_translation(Vec3::new(-0.5, 3.0, 0.0))
                    .with_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
            ))
            .id();
    // Allow this directional light to participate in layers 0 (default camera) and 1 (shadow-only)
    commands
        .entity(dir_light_entity)
        .insert(RenderLayers::from_layers(&[
            DEFAULT_RENDER_LAYER,
            SHADOW_ONLY_LAYER,
        ]));
    info!(
        "Directional light {:?} assigned layers {:?}",
        dir_light_entity,
        RenderLayers::from_layers(&[DEFAULT_RENDER_LAYER, SHADOW_ONLY_LAYER])
    );

    // Floor to receive shadows
    let floor_handle = meshes.add(Plane3d::default().mesh().size(20.0, 20.0));
    commands.spawn((
        Mesh3d(floor_handle),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Some cubes to cast and receive shadows
    let cube_handle = meshes.add(Cuboid::new(0.5, 0.5, 0.5));
    let cube_material = materials.add(Color::srgb(0.8, 0.2, 0.2));
    if std::env::var_os("BEVY_TEXT3D_DEBUG_DONT_SPAWN").is_none() {
        commands.spawn((
            Mesh3d(cube_handle.clone()),
            MeshMaterial3d(cube_material.clone()),
            Transform::from_xyz(-1.0, 0.25, 1.0),
        ));

        commands.spawn((
            Mesh3d(cube_handle),
            MeshMaterial3d(cube_material),
            Transform::from_xyz(1.0, 0.25, -1.0),
        ));
    }

    // Debug-only: spawn a small, visible cube on the shadow-only layer so we can
    // verify that shadow-only geometry on layer 1 participates in directional light shadow maps.
    // This cube is visible (not OnlyShadowCaster) and on layer 1, so it should be rendered by camera
    // only if RenderLayers were also assigned to the camera (which we don't do). However since
    // it's visible it helps localize layering issues. We also spawn a smaller OnlyShadowCaster cube on layer 1
    // which should cast shadows even though it's invisible to the camera.
    let debug_cube_handle = meshes.add(Cuboid::new(0.2, 0.2, 0.2));
    let debug_visible_material = materials.add(Color::srgb(0.2, 0.8, 0.2));
    // Visible cube in layer 1 (for visual debugging if camera layer includes 1)
    let visible_debug = commands
        .spawn((
            Mesh3d(debug_cube_handle.clone()),
            MeshMaterial3d(debug_visible_material.clone()),
            Transform::from_xyz(0.5, 0.25, 0.0),
        ))
        .id();
    commands
        .entity(visible_debug)
        .insert(RenderLayers::layer(SHADOW_ONLY_LAYER));

    // Invisible shadow-only cube that should cast shadows onto the floor (uses StandardMaterial + Visibility::Hidden)
    // We set `OnlyShadowCaster` so it is considered for shadow maps even though hidden from cameras.
    let debug_shadow_material = materials.add(StandardMaterial::default());
    let shadow_debug = commands
        .spawn((
            Mesh3d(debug_cube_handle.clone()),
            MeshMaterial3d(debug_shadow_material.clone()),
            Transform::from_xyz(0.0, 0.25, 0.0),
        ))
        .insert((OnlyShadowCaster, Visibility::Hidden))
        .id();
    commands
        .entity(shadow_debug)
        .insert(RenderLayers::layer(SHADOW_ONLY_LAYER));
}

// One entity handles the entire text
// The glyph_profile_mesh_handle() returns a single mesh for the combined outline of all glyphs
// The shadow casting uses this single outline mesh
fn spawn_text_when_loaded(
    mut commands: Commands,
    fonts: Res<Assets<Font>>,
    font_handle: Res<FontHandle>,
) {
    debug!("spawn_text_when_loaded called");
    if let Some(font) = fonts.get(font_handle.0.id()) {
        debug!("Font asset found, spawning text");
        let text = "SHADOWS".to_string();
        // Spawn immediately once the font asset is loaded; atlas/mesh/material
        // creation happens asynchronously in the Text3d plugin systems (handled by Text3dPlugin).

        // At this point atlases and textures are present for all codepoints.
        let mut text_cursor = Vec2::ZERO;
        let mut glyphs: Vec<Glyph> = Vec::new();
        for c in text.chars() {
            if let Some(info) = font.glyph(c) {
                // Use the glyph offset so the quad aligns with the glyph's bounding box
                let pos = bevy::math::Rect::from_corners(text_cursor, text_cursor + info.size);
                glyphs.push(Glyph {
                    position: pos,
                    character: c,
                    color: [1.0, 1.0, 1.0, 1.0],
                });
                // TODO: handle kerning properly
                text_cursor.x += info.advance.x + 0.02; // gap between characters
            }
        }

        // Spawn the Text3d with glyphs and request atlas generation for the used code points
        let mut text_mesh = Text3d::new(font_handle.0.clone());
        text_mesh.set_glyphs(glyphs.into_boxed_slice());
        // Atlases already generated above, so no need to request missing
        // codepoints here. But keep the call in case fonts change later.
        let codepoints: Vec<char> = text.chars().collect();
        text_mesh.add_missing(&codepoints);
        // Set glyph profile mode to None since we're handling shadow casting ourselves
        text_mesh = text_mesh.with_glyph_profile_mode(GlyphProfileRenderMode::None);

        commands.spawn((
            text_mesh,
            // Position the text to cast shadows
            Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(1.0)), // Makes text 1x larger,
        ));

        debug!("Spawned shadow casting text");
    }
}

fn sync_shadow_casters(
    mut commands: Commands,
    text_query: Query<(Entity, &Text3d, Option<&Children>)>,
    mut shadow_children: Query<&mut Mesh3d, With<MeshMaterial3d<ShadowOnlyMaterial>>>,
    mut shadow_materials: ResMut<Assets<ShadowOnlyMaterial>>,
) {
    for (entity, text3d, maybe_children) in text_query.iter() {
        // Get the glyph profile mesh (outline mesh for shadow casting)
        let Some(profile_mesh) = text3d.glyph_profile_mesh_handle() else {
            debug!(
                "sync_shadow_casters: No glyph profile mesh for text entity {:?}",
                entity
            );
            continue;
        };

        debug!(
            "sync_shadow_casters: Found glyph profile mesh {:?} for text entity {:?}",
            profile_mesh, entity
        );

        // Find existing shadow caster child (look for children that carry `OnlyShadowCaster`)
        let existing_shadow_child = maybe_children.and_then(|children| {
            children
                .iter()
                .find(|child| shadow_children.get(*child).is_ok())
        });

        if let Some(shadow_child_entity) = existing_shadow_child {
            // Update existing shadow caster's mesh if it changed
            if let Ok(mut mesh3d) = shadow_children.get_mut(shadow_child_entity) {
                if mesh3d.0 != profile_mesh {
                    debug!(
                        "Updating shadow mesh for entity ({:?}) with text '{}'",
                        entity,
                        text3d
                            .glyphs()
                            .iter()
                            .map(|g| g.character)
                            .collect::<String>()
                    );
                    mesh3d.0 = profile_mesh.clone();
                }
            }
        } else {
            info!(
                "sync_shadow_casters: Creating shadow-only child entity ({:?}) for text '{:?}'",
                entity,
                text3d
                    .glyphs()
                    .iter()
                    .map(|g| g.character)
                    .collect::<String>()
            );

            // Use a default StandardMaterial handle (material doesn't matter when the mesh is hidden
            // from camera views, but a material handle is required by the MeshMaterial3d wrapper).
            let material_handle =
                shadow_materials.add(create_shadow_only_material(StandardMaterial::default()));

            // Spawn invisible glyph mesh child that casts shadows using the shadow-only material.
            let child = commands
                .spawn((
                    Mesh3d(profile_mesh.clone()),
                    MeshMaterial3d(material_handle),
                    Transform::from_xyz(0.0, 0.0, -0.001),
                ))
                .insert((OnlyShadowCaster, Visibility::Hidden))
                .id();
            commands
                .entity(child)
                .insert(RenderLayers::layer(SHADOW_ONLY_LAYER));
            info!(
                "Created shadow-only child {:?} for Text3d entity {:?} on RenderLayer {}",
                child, entity, SHADOW_ONLY_LAYER
            );
            commands.entity(entity).add_child(child);

            info!(
                "sync_shadow_casters: Created shadow caster child entity ({:?}) of entity ({:?})",
                child, entity
            );
        }
    }
}

/// Debug system used to log positions and AABB information for shadow-only meshes and the floor.
fn debug_log_shadow_casters(
    query_shadow_casters: Query<(&GlobalTransform, &RenderLayers), With<OnlyShadowCaster>>,
    floor_query: Query<&GlobalTransform, Without<MeshMaterial3d<ShadowOnlyMaterial>>>,
) {
    for (transform, layers) in query_shadow_casters.iter() {
        debug!(
            "shadow-caster entity on layers {:?} world pos = {:?}",
            layers,
            transform.translation()
        );
    }
    for transform in floor_query.iter() {
        debug!("floor world pos = {:?}", transform.translation());
    }
}

fn handle_quality_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut current_quality: ResMut<CurrentShadowQuality>,
) {
    let old_quality = current_quality.0;
    if keys.just_pressed(KeyCode::ArrowUp) {
        current_quality.0 = current_quality.0.next();
        debug!(
            "Increased shadow quality from {:?} to {:?}",
            old_quality, current_quality.0
        );
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        current_quality.0 = current_quality.0.prev();
        debug!(
            "Decreased shadow quality from {:?} to {:?}",
            old_quality, current_quality.0
        );
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::ArrowDown) {
        debug!(
            "Key press detected - quality changed from {:?} to {:?}",
            old_quality, current_quality.0
        );
    }
}
fn update_shadow_quality(
    current_quality: Res<CurrentShadowQuality>,
    mut config: ResMut<TextMeshPluginConfig>,
    mut text_query: Query<&mut Text3d>,
    mut previous_quality: Local<ShadowQuality>,
) {
    debug!(
        "update_shadow_quality: current={:?}, previous={:?}",
        current_quality.0, *previous_quality
    );
    if *previous_quality != current_quality.0 {
        debug!(
            "Quality changed from {:?} to {:?}, updating config and clearing glyph profiles",
            *previous_quality, current_quality.0
        );
        *previous_quality = current_quality.0;
        config.text_mesh_shadow_quality = current_quality.0.to_glyph_quality();
        info!("Updated shadow quality to {:?}", current_quality.0);

        // Clear glyph profile meshes so they get recreated with new quality
        for mut text3d in text_query.iter_mut() {
            text3d.clear_glyph_profile();
        }
    }
}
