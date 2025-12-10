use bevy::prelude::*;
use bevy_log::info;
use bevy_text3d::{Font, Glyph, Text3d, Text3dPlugin};

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum AppState {
    #[default]
    Loading,
    Ready,
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(Text3dPlugin)
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            check_font_loaded.run_if(in_state(AppState::Loading)),
        )
        .add_systems(OnEnter(AppState::Ready), spawn_text_when_loaded)
        //.add_systems(Update, _simulate_mouse_movement)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let font_handle = asset_server.load("fonts/FiraCode-Bold.ttf");

    // Store the font handle
    commands.insert_resource(FontHandle(font_handle.clone()));

    let load_state = asset_server.get_load_state(font_handle.id());
    info!("Font load state: {:?}", load_state);

    let camera_transform =
        Transform::from_xyz(0.0, 1.0, 10.0).looking_at(Vec3::new(0.0, 0.0, 1.0), Vec3::Y);

    commands.spawn((Camera3d::default(), camera_transform));

    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));

    let initial_size = Vec2::new(20.0, 20.0);
    let floor_handle = meshes.add(
        Plane3d::default()
            .mesh()
            .size(initial_size.x, initial_size.y),
    );
    commands.spawn((
        Mesh3d(floor_handle),
        MeshMaterial3d(materials.add(Color::srgb(0.1, 0.1, 0.2))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

fn check_font_loaded(
    fonts: Res<Assets<Font>>,
    font_handle: Res<FontHandle>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if fonts.get(&font_handle.0).is_some() {
        next_state.set(AppState::Ready);
    }
}

#[derive(Resource)]
struct FontHandle(Handle<Font>);

fn spawn_text_when_loaded(
    mut commands: Commands,
    fonts: Res<Assets<Font>>,
    font_handle: Res<FontHandle>,
) {
    if let Some(font) = fonts.get(&font_handle.0) {
        let text = "Hello, Bevy!".to_string();
        // Spawn immediately once the font asset is loaded; atlas/mesh/material
        // creation happens asynchronously in the TextMesh plugin systems.

        // At this point atlases and textures are present for all codepoints.
        let mut text_cursor = Vec2::ZERO;
        let mut glyphs: Vec<Glyph> = Vec::new();
        let mut min_corner = Vec2::splat(f32::INFINITY);
        let mut max_corner = Vec2::splat(f32::NEG_INFINITY);
        for c in text.chars() {
            if let Some(info) = font.glyph(c) {
                // Use the glyph offset so the quad aligns with the glyph's bounding box
                let pos = bevy::math::Rect::from_corners(text_cursor, text_cursor + info.size);
                min_corner = min_corner.min(pos.min);
                max_corner = max_corner.max(pos.max);
                glyphs.push(Glyph {
                    position: pos,
                    character: c,
                    color: [1.0, 1.0, 1.0, 1.0],
                });
                // TODO: handle kerning properly
                text_cursor.x += info.advance.x + 0.02; // gap between characters
            }
        }

        if !glyphs.is_empty() {
            let horizontal_center = (min_corner.x + max_corner.x) * 0.5;
            let baseline = min_corner.y;
            let offset = Vec2::new(horizontal_center, baseline);
            for glyph in glyphs.iter_mut() {
                let rect = glyph.position;
                glyph.position =
                    bevy::math::Rect::from_corners(rect.min - offset, rect.max - offset);
            }
        }

        // Spawn the Text3d mesh with glyphs and request atlas generation for the used code points
        let mut text_mesh = Text3d::new(font_handle.0.clone());
        text_mesh.set_glyphs(glyphs.into_boxed_slice());
        // Atlases already generated above, so no need to request missing
        // codepoints here. But keep the call in case fonts change later.
        let codepoints: Vec<char> = text.chars().collect();
        text_mesh.add_missing(&codepoints);

        let text_mesh_scale = 1.0;

        commands.spawn((
            text_mesh,
            // increase scale so glyphs are larger in world units
            Transform::from_xyz(0.0, 0.25, 0.0).with_scale(Vec3::splat(text_mesh_scale)),
        ));

        info!("Spawned TextMesh for '{}'", text);
    }
}
