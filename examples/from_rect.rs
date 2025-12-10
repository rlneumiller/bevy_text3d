// This example demonstrates the power and flexibility of Glyph::from_rect()
// compared to Glyph::from_cursor().
//
// Key differences:
// - from_cursor(): Takes a cursor position and lets the font system calculate
//   final glyph positioning using font metrics (offset, size, advance).
//   Good for normal text layout.
//
// - from_rect(): Takes the exact final rectangle where the glyph should appear.
//   Gives complete control over positioning, scaling, and distortion.
//   Perfect for custom layouts, animations, and special effects.
//
// This example shows:
// 1. Side-by-side comparison of both methods with on-screen annotations
// 2. Creative layouts (waves, circles, spirals) only possible with from_rect
// 3. Individual character transforms (scaling, stretching) with centered control
// 4. Real-time animations using from_rect for dynamic positioning and motion

use std::collections::BTreeSet;

use bevy::{asset::AssetId, image::Image, prelude::*};
use bevy_log::info;
use bevy_text3d::{
    Font, FontAtlasSets, Glyph, Text3d, Text3dPlugin,
    dump_glyph_profile::dump_glyph_profile_obj_on_key,
};
// open_space_controller removed for standalone repository — use default Bevy camera instead

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
        .init_state::<AppState>()
        .init_state::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            check_assets_loaded.run_if(in_state(AppState::Loading)),
        )
        .add_systems(OnEnter(AppState::Ready), spawn_text_when_loaded)
        .add_systems(
            Update,
            animate_text_effects.run_if(in_state(AppState::Ready)),
        )
        // TODO: Move dump_glyph_profile_obj_on_key to OpenSpacePlugin and update UI there
        .add_systems(
            Update,
            dump_glyph_profile_obj_on_key.run_if(in_state(AppState::Ready)),
        )
        .run();
}

fn check_assets_loaded(
    asset_server: Res<AssetServer>,
    font_handle: Res<FontHandle>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if asset_server.is_loaded_with_dependencies(font_handle.0.id()) {
        next_state.set(AppState::Ready);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font_handle = asset_server.load("fonts/FiraCode-Bold.ttf");

    // Store the font handle for the loading check system
    commands.insert_resource(FontHandle(font_handle.clone()));

    #[cfg(debug_assertions)]
    let load_state = asset_server.get_load_state(font_handle.id());
    #[cfg(debug_assertions)]
    debug!("Font load state: {:?}", load_state);

    // Add a camera positioned to look at the origin
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 2.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Add a directional light so meshes are visible
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            ..Default::default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));
}

#[derive(Component)]
struct AnimatedText {
    text: String,
    font_handle: Handle<Font>,
    glyph_metrics: Vec<GlyphMetric>,
    animation_type: AnimationType,
}

const EXAMPLE_TEXTS: &[&str] = &[
    "Normal vs Manual",
    "from_cursor() (auto)",
    "from_rect() (manual)",
    "Creative layouts (from_rect)",
    "WAVE",
    "CIRCLE",
    "Per-glyph transforms (from_rect)",
    "SCALE",
    "STRETCH",
    "Animated effects (dynamic from_rect)",
    "PULSE",
    "SPIN",
];

#[derive(Clone)]
enum AnimationType {
    Pulsing,
    Rotating,
}

#[derive(Clone)]
struct GlyphMetric {
    advance: Vec2,
    offset: Vec2,
    size: Vec2,
}

fn precache_glyphs_for_texts(
    atlases: &mut FontAtlasSets,
    fonts: &Assets<Font>,
    images: &mut Assets<Image>,
    font_id: AssetId<Font>,
    texts: &[&str],
) {
    let mut unique = BTreeSet::new();
    for text in texts {
        unique.extend(text.chars());
    }

    if unique.is_empty() {
        return;
    }

    let chars: Vec<char> = unique.into_iter().collect();
    atlases.add_code_points(&chars, font_id, fonts, images);
}

fn glyph_metrics_for_text(font: &Font, text: &str) -> Vec<GlyphMetric> {
    text.chars()
        .map(|c| match font.glyph(c) {
            Some(info) => GlyphMetric {
                advance: info.advance,
                offset: info.offset,
                size: info.size,
            },
            None => {
                warn!("Missing glyph info for character '{}'", c);
                GlyphMetric {
                    advance: Vec2::new(0.6, 0.0),
                    offset: Vec2::ZERO,
                    size: Vec2::ZERO,
                }
            }
        })
        .collect()
}

fn spawn_cursor_text_label(
    commands: &mut Commands,
    font: &Font,
    font_handle: &Handle<Font>,
    text: &str,
    position: Vec3,
    scale: f32,
    color: [f32; 4],
) {
    let mut cursor = Vec2::ZERO;
    let mut glyphs = Vec::new();

    for c in text.chars() {
        if let Some(info) = font.glyph(c) {
            let rect = bevy::math::Rect::from_corners(cursor, cursor);
            glyphs.push(Glyph::from_cursor(rect, c, color));
            cursor.x += info.advance.x;
        }
    }

    let chars: Vec<char> = text.chars().collect();

    let mut mesh = Text3d::new(font_handle.clone());
    mesh.set_glyphs(glyphs.into_boxed_slice());
    mesh.add_missing(&chars);

    commands.spawn((
        mesh,
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
    ));
}

/// Demonstrates the difference between from_cursor and from_rect by showing
/// two text meshes side by side with different positioning approaches.
///
/// - Blue text (top): Uses from_cursor() - automatic font-based positioning
/// - Orange text (bottom): Uses from_rect() - manual positioning with exact control
///
/// Both should render identically, but from_rect gives you full control over
/// the final positioning, which enables custom layouts and effects.
fn spawn_comparison_example(commands: &mut Commands, font: &Font, font_handle: &Handle<Font>) {
    const LETTER_SPACING: f32 = 0.02;
    let text = "Normal vs Manual";
    let characters: Vec<char> = text.chars().collect();

    // Method 1: Using from_cursor (automatic font-based positioning)
    let mut cursor = Vec2::ZERO;
    let mut cursor_glyphs = Vec::with_capacity(characters.len());

    for &c in &characters {
        if let Some(info) = font.glyph(c) {
            let pos = bevy::math::Rect::from_corners(cursor, cursor);
            cursor_glyphs.push(Glyph::from_cursor(pos, c, [0.3, 0.8, 1.0, 1.0])); // Blue
            cursor.x += info.advance.x + LETTER_SPACING;
        }
    }

    let mut cursor_mesh = Text3d::new(font_handle.clone());
    cursor_mesh.set_glyphs(cursor_glyphs.into_boxed_slice());
    cursor_mesh.add_missing(&characters);

    commands.spawn((
        cursor_mesh,
        Transform::from_xyz(-2.0, 1.5, 0.0).with_scale(Vec3::splat(0.08)),
    ));

    // Method 2: Using from_rect (manual positioning with exact control)
    let mut rect_glyphs = Vec::with_capacity(characters.len());
    let mut x_pos = 0.0;

    for &c in &characters {
        if let Some(info) = font.glyph(c) {
            // Manually compute final rect with precise control
            let min = Vec2::new(x_pos + info.offset.x, info.offset.y);
            let pos = bevy::math::Rect::from_corners(min, min + info.size);
            rect_glyphs.push(Glyph::from_rect(pos, c, [1.0, 0.8, 0.3, 1.0])); // Orange
            x_pos += info.advance.x + LETTER_SPACING;
        }
    }

    let mut rect_mesh = Text3d::new(font_handle.clone());
    rect_mesh.set_glyphs(rect_glyphs.into_boxed_slice());
    rect_mesh.add_missing(&characters);

    commands.spawn((
        rect_mesh,
        Transform::from_xyz(-2.0, 1.2, 0.0).with_scale(Vec3::splat(0.08)),
    ));

    spawn_cursor_text_label(
        commands,
        font,
        font_handle,
        "from_cursor() (auto)",
        Vec3::new(-2.0, 1.75, 0.0),
        0.06,
        [0.55, 0.85, 1.0, 1.0],
    );

    spawn_cursor_text_label(
        commands,
        font,
        font_handle,
        "from_rect() (manual)",
        Vec3::new(-2.0, 1.05, 0.0),
        0.06,
        [1.0, 0.85, 0.45, 1.0],
    );
}

/// Demonstrates creative layouts that are only possible with from_rect's precise control.
/// These effects would be impossible or very difficult with from_cursor because
/// they require placing each character at arbitrary positions rather than
/// following normal text flow. Includes wave, circle, and spiral arrangements
/// with an on-screen label for context.
fn spawn_creative_layouts(commands: &mut Commands, font: &Font, font_handle: &Handle<Font>) {
    spawn_cursor_text_label(
        commands,
        font,
        font_handle,
        "Creative layouts (from_rect)",
        Vec3::new(-0.3, 1.0, 0.0),
        0.07,
        [0.9, 0.7, 1.0, 1.0],
    );

    let text = "WAVE";

    // Create a wave pattern
    let characters: Vec<char> = text.chars().collect();
    let mut wave_glyphs = Vec::with_capacity(characters.len());
    let mut x_pos = 0.0;

    for (i, c) in characters.iter().enumerate() {
        if let Some(info) = font.glyph(*c) {
            let wave_y = (i as f32 * 0.8).sin() * 0.3; // Sine wave
            let min = Vec2::new(x_pos + info.offset.x, wave_y + info.offset.y);
            let pos = bevy::math::Rect::from_corners(min, min + info.size);
            wave_glyphs.push(Glyph::from_rect(pos, *c, [1.0, 0.5, 0.8, 1.0])); // Pink
            x_pos += info.advance.x + 0.05;
        }
    }

    let mut wave_mesh = Text3d::new(font_handle.clone());
    wave_mesh.set_glyphs(wave_glyphs.into_boxed_slice());
    wave_mesh.add_missing(&characters);

    commands.spawn((
        wave_mesh,
        Transform::from_xyz(-1.5, 0.8, 0.0).with_scale(Vec3::splat(0.1)),
    ));

    // Create a circular pattern
    let circle_text = "CIRCLE";
    let circle_chars: Vec<char> = circle_text.chars().collect();
    let mut circle_glyphs = Vec::with_capacity(circle_chars.len());
    let radius = 0.8;

    for (i, c) in circle_chars.iter().enumerate() {
        if let Some(info) = font.glyph(*c) {
            let angle = (i as f32 / circle_text.len() as f32) * std::f32::consts::TAU;
            let pos_x = angle.cos() * radius;
            let pos_y = angle.sin() * radius;

            let min = Vec2::new(pos_x + info.offset.x, pos_y + info.offset.y);
            let pos = bevy::math::Rect::from_corners(min, min + info.size);
            circle_glyphs.push(Glyph::from_rect(pos, *c, [0.5, 1.0, 0.5, 1.0])); // Green
        }
    }

    let mut circle_mesh = Text3d::new(font_handle.clone());
    circle_mesh.set_glyphs(circle_glyphs.into_boxed_slice());
    circle_mesh.add_missing(&circle_chars);

    commands.spawn((
        circle_mesh,
        Transform::from_xyz(1.5, 0.5, 0.0).with_scale(Vec3::splat(0.08)),
    ));
}

/// Demonstrates individual character scaling and distortion effects.
/// from_rect allows you to modify the size of each glyph independently,
/// creating scaling effects, stretching, and other distortions that
/// preserve the character shape while changing its dimensions. The helper
/// label highlights that these transforms are computed manually per glyph.
fn spawn_scaled_characters(commands: &mut Commands, font: &Font, font_handle: &Handle<Font>) {
    spawn_cursor_text_label(
        commands,
        font,
        font_handle,
        "Per-glyph transforms (from_rect)",
        Vec3::new(-1.0, 0.35, 0.0),
        0.065,
        [1.0, 0.95, 0.6, 1.0],
    );

    let text = "SCALE";
    let characters: Vec<char> = text.chars().collect();
    let mut scaled_glyphs = Vec::with_capacity(characters.len());
    let mut x_pos = 0.0;

    for (i, c) in characters.iter().enumerate() {
        if let Some(info) = font.glyph(*c) {
            // Progressively scale each character
            let scale_factor = 0.8 + (i as f32 * 0.25);
            let scaled_size = info.size * scale_factor;

            let base_min = Vec2::new(x_pos + info.offset.x, info.offset.y);
            let scale_offset = (scaled_size - info.size) * 0.5;
            let min = base_min - scale_offset;
            let pos = bevy::math::Rect::from_corners(min, min + scaled_size);
            scaled_glyphs.push(Glyph::from_rect(pos, *c, [1.0, 1.0, 0.3, 1.0])); // Yellow

            x_pos += info.advance.x + 0.08;
        }
    }

    let mut scaled_mesh = Text3d::new(font_handle.clone());
    scaled_mesh.set_glyphs(scaled_glyphs.into_boxed_slice());
    scaled_mesh.add_missing(&characters);

    commands.spawn((
        scaled_mesh,
        Transform::from_xyz(-1.0, 0.0, 0.0).with_scale(Vec3::splat(0.1)),
    ));

    // Create squashed/stretched text
    let stretch_text = "STRETCH";
    let stretch_chars: Vec<char> = stretch_text.chars().collect();
    let mut stretch_glyphs = Vec::with_capacity(stretch_chars.len());
    x_pos = 0.0;

    for (i, c) in stretch_chars.iter().enumerate() {
        if let Some(info) = font.glyph(*c) {
            // Create a stretched feel by exaggerating bounding box aspect ratios and spacing
            let stretch_factor = match i % 4 {
                0 => Vec2::new(2.2, 0.55), // Very wide & low
                1 => Vec2::new(0.5, 2.4),  // Tall & narrow
                2 => Vec2::new(1.8, 0.7),  // Wide & somewhat short
                _ => Vec2::new(0.65, 2.1), // Tall & narrow
            };

            let size = Vec2::new(
                info.size.x * stretch_factor.x,
                info.size.y * stretch_factor.y,
            );

            let base_min = Vec2::new(x_pos + info.offset.x, info.offset.y);
            let base_max = base_min + info.size;

            let min_x = (base_min.x + info.size.x * 0.5) - size.x * 0.5;
            let min_y = base_max.y - size.y; // keep baseline consistent
            let min = Vec2::new(min_x, min_y);
            let pos = bevy::math::Rect::from_corners(min, min + size);
            stretch_glyphs.push(Glyph::from_rect(pos, *c, [0.8, 0.3, 1.0, 1.0])); // Purple

            // Space characters based on stretched width while keeping a minimum gap
            let extra_spacing = (size.x - info.size.x).max(0.0) * 0.4;
            x_pos += info.advance.x + extra_spacing + 0.12;
        }
    }

    let mut stretch_mesh = Text3d::new(font_handle.clone());
    stretch_mesh.set_glyphs(stretch_glyphs.into_boxed_slice());
    stretch_mesh.add_missing(&stretch_chars);

    commands.spawn((
        stretch_mesh,
        Transform::from_xyz(-1.5, -0.5, 0.0).with_scale(Vec3::splat(0.08)),
    ));
}

/// Creates annotated animated text examples that update in real-time.
/// This shows how from_rect enables dynamic effects by allowing
/// you to recalculate glyph positions and sizes every frame and keep the
/// text centered while it moves.
fn spawn_animated_examples(commands: &mut Commands, font: &Font, font_handle: &Handle<Font>) {
    spawn_cursor_text_label(
        commands,
        font,
        font_handle,
        "Animated effects (dynamic from_rect)",
        Vec3::new(0.25, -0.45, 0.0),
        0.06,
        [0.7, 0.9, 1.0, 1.0],
    );
    let pulse_text = "PULSE";
    let pulse_metrics = glyph_metrics_for_text(font, pulse_text);
    // Spawn pulsing text
    commands.spawn((
        Text3d::new(font_handle.clone()),
        Transform::from_xyz(-0.5, -0.95, 0.0).with_scale(Vec3::splat(0.25)),
        AnimatedText {
            text: pulse_text.to_string(),
            font_handle: font_handle.clone(),
            glyph_metrics: pulse_metrics,
            animation_type: AnimationType::Pulsing,
        },
    ));

    let spin_text = "SPIN";
    let spin_metrics = glyph_metrics_for_text(font, spin_text);
    // Spawn rotating text
    commands.spawn((
        Text3d::new(font_handle.clone()),
        Transform::from_xyz(1.1, -0.95, 0.0).with_scale(Vec3::splat(0.1)),
        AnimatedText {
            text: spin_text.to_string(),
            font_handle: font_handle.clone(),
            glyph_metrics: spin_metrics,
            animation_type: AnimationType::Rotating,
        },
    ));
}

/// System that animates text effects using from_rect
fn animate_text_effects(
    time: Res<Time>,
    fonts: Res<Assets<Font>>,
    mut query: Query<(&mut Text3d, &AnimatedText)>,
) {
    for (mut text_mesh, animated) in query.iter_mut() {
        if fonts.get(&animated.font_handle).is_none() {
            continue;
        }

        let elapsed = time.elapsed_secs();
        let mut glyphs = Vec::new();
        let characters: Vec<char> = animated.text.chars().collect();

        match animated.animation_type {
            AnimationType::Pulsing => {
                // Characters pulse in and out with controlled scaling
                let letter_spacing = 0.06;
                let mut total_width = 0.0;

                for (idx, metrics) in animated.glyph_metrics.iter().enumerate() {
                    total_width += metrics.advance.x;
                    if idx + 1 < animated.glyph_metrics.len() {
                        total_width += letter_spacing;
                    }
                }

                let mut x_pos = -total_width * 0.5;
                for (i, (c, metrics)) in characters
                    .iter()
                    .zip(animated.glyph_metrics.iter())
                    .enumerate()
                {
                    let pulse =
                        ((elapsed * 3.0 + i as f32 * 0.5).sin() * 0.3 + 1.0).clamp(0.7, 1.3);
                    let scaled_size = metrics.size * pulse;

                    let base_min = Vec2::new(x_pos + metrics.offset.x, metrics.offset.y);
                    let scale_offset = (scaled_size - metrics.size) * 0.5;
                    let min = base_min - scale_offset;
                    let pos = bevy::math::Rect::from_corners(min, min + scaled_size);
                    let color_shift = (pulse - 0.7) / 0.6; // normalized 0..1
                    glyphs.push(Glyph::from_rect(
                        pos,
                        *c,
                        [1.0, 0.35 + 0.4 * color_shift, 0.25, 1.0],
                    ));

                    x_pos += metrics.advance.x + letter_spacing;
                }
            }
            AnimationType::Rotating => {
                // Characters rotate around their center
                let center = Vec2::new(0.0, 0.0);
                let base_radius = 0.42;

                for (i, (c, metrics)) in characters
                    .iter()
                    .zip(animated.glyph_metrics.iter())
                    .enumerate()
                {
                    let angle_offset =
                        i as f32 * std::f32::consts::TAU / characters.len().max(1) as f32;
                    let angle = elapsed * 1.6 + angle_offset;
                    let radius = base_radius + (elapsed * 1.2 + i as f32).sin() * 0.06;
                    let pos_x = center.x + angle.cos() * radius;
                    let pos_y = center.y + angle.sin() * radius;

                    let min = Vec2::new(pos_x + metrics.offset.x, pos_y + metrics.offset.y);
                    let pos = bevy::math::Rect::from_corners(min, min + metrics.size);
                    let color_phase = ((elapsed * 0.8) + i as f32 * 0.5).sin() * 0.5 + 0.5;
                    glyphs.push(Glyph::from_rect(
                        pos,
                        *c,
                        [0.2 + 0.4 * color_phase, 0.7, 1.0, 1.0],
                    ));
                }
            }
        }

        text_mesh.set_glyphs(glyphs.into_boxed_slice());
        text_mesh.add_missing(&characters);
    }
}

fn spawn_text_when_loaded(
    mut commands: Commands,
    fonts: Res<Assets<Font>>,
    font_handle: Res<FontHandle>,
    mut atlases: ResMut<FontAtlasSets>,
    mut images: ResMut<Assets<Image>>,
) {
    if let Some(font) = fonts.get(font_handle.0.id()) {
        precache_glyphs_for_texts(
            &mut atlases,
            &fonts,
            &mut images,
            font_handle.0.id(),
            EXAMPLE_TEXTS,
        );
        // Demonstrate different from_rect capabilities
        spawn_comparison_example(&mut commands, font, &font_handle.0);
        spawn_creative_layouts(&mut commands, font, &font_handle.0);
        spawn_scaled_characters(&mut commands, font, &font_handle.0);
        spawn_animated_examples(&mut commands, font, &font_handle.0);

        info!("Spawned multiple TextMesh examples demonstrating from_rect capabilities");
    }
}
