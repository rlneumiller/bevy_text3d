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
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Replace this path with your actual font file
    let font_handle = asset_server.load("fonts/FiraCode-Bold.ttf");

    // Store the font handle
    commands.insert_resource(FontHandle(font_handle.clone()));

    // Log font load state
    let load_state = asset_server.get_load_state(font_handle.id());
    info!("Font load state: {:?}", load_state);

    // Check if we're in screenshot mode (no interactive controls needed)
    let take_screenshot = std::env::args().any(|arg| arg == "--take-screenshot");

    // Add a camera positioned to look at the origin
    let mut camera = commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 2.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Only add interactive camera controls when not in screenshot mode
    if !take_screenshot {
        // No interactive camera plugin included in the standalone repo; keep camera static
    }

    // Add a directional light so meshes are visible
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            ..Default::default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
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
    // Font is loaded; retrieve it and build glyphs for the text with custom colors
    if let Some(font) = fonts.get(&font_handle.0) {
        let text = "Hello, Bevy!";
        let mut cursor = Vec2::ZERO;
        let mut glyphs: Vec<Glyph> = Vec::new();

        // Define custom colors for each character
        let colors = [
            [1.0, 0.0, 0.0, 1.0], // Red for 'H'
            [0.0, 1.0, 0.0, 1.0], // Green for 'e'
            [0.0, 0.0, 1.0, 1.0], // Blue for 'l'
            [1.0, 1.0, 0.0, 1.0], // Yellow for 'l'
            [1.0, 0.0, 1.0, 1.0], // Magenta for 'o'
            [0.0, 1.0, 1.0, 1.0], // Cyan for ','
            [1.0, 0.5, 0.0, 1.0], // Orange for ' '
            [0.5, 0.5, 0.5, 1.0], // Gray for 'B'
            [1.0, 0.0, 0.5, 1.0], // Pink for 'e'
            [0.5, 0.0, 1.0, 1.0], // Purple for 'v'
            [0.0, 0.5, 1.0, 1.0], // Light blue for 'y'
            [1.0, 1.0, 1.0, 1.0], // White for '!'
        ];

        for (i, c) in text.chars().enumerate() {
            if let Some(info) = font.glyph(c) {
                // Pass the cursor origin; the TextMesh library will apply the
                // glyph offset and size when building the final quads.
                let pos = bevy::math::Rect::from_corners(cursor, cursor + info.size);
                let color = colors.get(i).copied().unwrap_or([1.0, 1.0, 1.0, 1.0]);
                glyphs.push(Glyph::from_cursor(pos, c, color));
                cursor.x += info.advance.x + 0.02; // small gap
            }
        }

        // Spawn the TextMesh with glyphs and request atlas generation for the used code points
        let mut text_mesh = Text3d::new(font_handle.0.clone());
        text_mesh.set_glyphs(glyphs.into_boxed_slice());
        let codepoints: Vec<char> = text.chars().collect();
        text_mesh.add_missing(&codepoints);

        commands.spawn((
            text_mesh,
            // increase scale so glyphs are larger in world units
            Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(0.12)),
        ));

        // Remove the resource so we only spawn once
        info!("Spawned TextMesh for '{}'", text);
    }
}
