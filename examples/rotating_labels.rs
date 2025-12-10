use bevy::prelude::*;
use bevy_log::info;
use bevy_text3d::{Font, Glyph, Text3d, Text3dPlugin};

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum AppState {
    #[default]
    Loading,
    Ready,
}

#[derive(Resource)]
struct FontHandle(Handle<Font>);

fn check_font_loaded(
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
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            check_font_loaded.run_if(in_state(AppState::Loading)),
        )
        .add_systems(OnEnter(AppState::Ready), spawn_text_when_loaded)
        .add_systems(Update, animate_rotations.run_if(in_state(AppState::Ready)))
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/FiraCode-Bold.ttf");
    commands.insert_resource(FontHandle(font.clone()));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            ..Default::default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));
}
fn spawn_text_when_loaded(
    mut commands: Commands,
    fonts: Res<Assets<Font>>,
    font_handle: Res<FontHandle>,
) {
    let mut glyphs = Vec::new();
    let text = "Look! I'm Rotating".to_string();
    let mut cursor = Vec2::ZERO;

    if let Some(font) = fonts.get(font_handle.0.id()) {
        for c in text.chars() {
            if let Some(info) = font.glyph(c) {
                // Pass the cursor origin; the TextMesh library will apply the
                // glyph offset and size when building the final quads.
                // Use from_rect instead of from_cursor if you want to specify the full rect and ignore the glyph offset
                let pos = bevy::math::Rect::from_corners(cursor, cursor + info.size);
                glyphs.push(Glyph::from_cursor(pos, c, [1.0, 1.0, 1.0, 1.0]));
                cursor.x += info.advance.x + 0.02;
            }
        }

        // Compute the bounding rect of the final glyph quads so we can
        // determine a pivot point to rotate around. We compute the final
        // glyph rects using the font glyph offset + size (the same logic
        // used by the TextMesh builder) so the pivot matches the rendered
        // geometry. Use the local `glyphs` Vec (cursor origins) and the
        // loaded font's glyph info to compute final quad bounds.
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);

        if let Some(font) = fonts.get(font_handle.0.id()) {
            for g in glyphs.iter() {
                if let Some(info) = font.glyph(g.character) {
                    let final_min = g.position.min + info.offset;
                    let final_max = final_min + info.size;
                    min = min.min(final_min);
                    max = max.max(final_max);
                }
            }
        }

        // Handle empty text case: if no glyphs were found, default pivot to 0
        let pivot = if min.x.is_finite() && max.x.is_finite() {
            Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, 0.0)
        } else {
            Vec3::ZERO
        };

        let mut base_mesh = Text3d::new(font_handle.0.clone());
        base_mesh.set_glyphs(glyphs.into_boxed_slice());
        base_mesh.add_missing(&text.chars().collect::<Vec<char>>());

        // spawn multiple labels with different rotations and scales
        let positions = [
            Vec3::new(-2.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
        ];
        let rotations = [
            Quat::from_rotation_z(0.0),
            Quat::from_rotation_z(0.5),
            Quat::from_rotation_z(1.0),
        ];
        let scales = [0.18, 0.12, 0.06];

        // simple deterministic LCG PRNG
        let mut seed = 0x12345678u32;
        fn lcg(seed: &mut u32) -> f32 {
            *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((*seed >> 8) as f32) / (u32::MAX as f32)
        }

        for i in 0..3 {
            let mesh = base_mesh.clone_for_spawn();
            // generate a pseudo-random axis and speed
            let rx = lcg(&mut seed) * 2.0 - 1.0;
            let ry = lcg(&mut seed) * 2.0 - 1.0;
            let rz = lcg(&mut seed) * 2.0 - 1.0;
            let mut axis = Vec3::new(rx, ry, rz);
            if axis.length_squared() <= 1e-6 {
                axis = Vec3::Y;
            } else {
                axis = axis.normalize();
            }
            // Increase base speed and slightly widen random range so labels rotate faster
            let speed = 0.6 + lcg(&mut seed) * 2.5; // radians/sec

            // Spawn a parent entity at the target world position which will
            // be the rotation pivot. The actual `TextMesh` is spawned as a
            // child with a translation of `-pivot` so rotating the parent
            // rotates the text around the computed pivot point.
            let scale_vec = Vec3::splat(scales[i]);
            commands
                .spawn((
                    // apply the initial rotation on the parent so animated
                    // rotation composes with the starting rotation around the
                    // pivot
                    Transform::from_translation(positions[i])
                        .with_rotation(rotations[i])
                        .with_scale(Vec3::ONE),
                    Visibility::Visible,
                    RotatingAxis { axis, speed },
                ))
                .with_children(|parent| {
                    parent.spawn((
                        mesh,
                        // offset the TextMesh so `pivot` is at the parent's origin (ChildOf relationship)
                        // and account for the child's scale so the pivot matches
                        // the scaled geometry center
                        Transform::from_translation(-pivot * scales[i]).with_scale(scale_vec),
                    ));
                });
        }

        info!("Spawned rotated labels");
    }
}

#[derive(Component)]
struct RotatingAxis {
    axis: Vec3,
    speed: f32,
}

fn animate_rotations(time: Res<Time>, mut query: Query<(&RotatingAxis, &mut Transform)>) {
    for (rot, mut transform) in query.iter_mut() {
        let angle = rot.speed * time.delta_secs();
        transform.rotate(Quat::from_axis_angle(rot.axis, angle));
    }
}
