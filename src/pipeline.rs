use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use bevy::{
    asset::{AssetId, Assets, Handle, RenderAssetUsages},
    light::{NotShadowCaster, OnlyShadowCaster},
    math::{Rect, Vec3},
    pbr::StandardMaterial,
    prelude::{
        AlphaMode, App, Color, Commands, Component, Entity, Image, InheritedVisibility,
        IntoScheduleConfigs, Mesh, Mesh3d, MeshMaterial3d, Plugin, PostUpdate, Query, Res, ResMut,
        Resource, Transform, Update, ViewVisibility, Visibility,
    },
};
use bevy_log::{debug, info, warn};
use bevy_mesh::{Indices, PrimitiveTopology, VertexAttributeValues};

use crate::{
    font::{Font, FontAtlasSets},
    pipeline_material::{ATTRIBUTE_POSITION, GlyphMaterial},
};

// The remainder of the file is the original 'pipeline.rs' content from open_space_mmo
// which defines the Text3d component, Text3dBuilder, mesh systems, and plugin.

// TODO: Add support for per-character animation, scale, rotation, color, shadow, extrusion depth, etc.
// This would likely involve increasing the entity count considerably.

/// Represents the quality level for glyph tessellation.
/// Lower quality values produce fewer triangles but lower visual fidelity.
/// Higher quality values produce more triangles but better visual fidelity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlyphTessellationQuality {
    /// Ultra high quality: 0.000001 tolerance (2444 vertices, 2442 triangles)
    UltraHigh,
    /// Very high quality: 0.00001 tolerance (771 vertices, 769 triangles)
    VeryHigh,
    /// High quality: 0.0001 tolerance (263 vertices, 261 triangles)
    High,
    /// Medium quality: 0.001 tolerance (94 vertices, 92 triangles)
    Medium,
    /// Low quality: 0.005 tolerance (53 vertices, 51 triangles)
    Low,
    /// Very low quality: 0.01 tolerance (35 vertices, 33 triangles)
    VeryLow,
    /// Minimal quality: 0.02 tolerance (30 vertices, 28 triangles)
    Minimal,
}

impl GlyphTessellationQuality {
    /// Returns the tolerance value for this quality level.
    /// Lower values produce higher quality meshes with more triangles.
    pub fn tolerance(&self) -> f32 {
        match self {
            GlyphTessellationQuality::UltraHigh => 0.000001,
            GlyphTessellationQuality::VeryHigh => 0.00001,
            GlyphTessellationQuality::High => 0.0001,
            GlyphTessellationQuality::Medium => 0.001,
            GlyphTessellationQuality::Low => 0.005,
            GlyphTessellationQuality::VeryLow => 0.01,
            GlyphTessellationQuality::Minimal => 0.02,
        }
    }
}

impl Default for GlyphTessellationQuality {
    fn default() -> Self {
        GlyphTessellationQuality::High
    }
}

/// A positioned glyph with UV coordinates and atlas index for rendering.
pub struct PositionedGlyph {
    pub position: Rect,
    pub uv: Rect,
    pub index: usize,
    pub color: [f32; 4],
}

/// A single glyph to be rendered, including its character, position, and color.
pub struct Glyph {
    pub position: Rect,
    pub character: char,
    pub color: [f32; 4],
}

impl Clone for Glyph {
    fn clone(&self) -> Self {
        Self {
            position: self.position,
            character: self.character,
            color: self.color,
        }
    }
}

impl Glyph {
    /// Construct a `Glyph` where `position` is the cursor origin (min).
    /// The Text3d system will apply the font's glyph offset and size when
    /// building final quads.
    ///
    /// Example:
    /// If you call:
    ///     Glyph::from_cursor(Rect::new(10.0, 20.0, 10.0, 20.0), 'A', [1.0,1.0,1.0,1.0]),
    /// you're saying "start drawing an 'A' at position (x = 10, y = 20), where position
    /// is the cursor origin (min), and let the font system
    /// figure out how tall and wide it should be and where exactly to place it."
    pub fn from_cursor(position: Rect, character: char, color: [f32; 4]) -> Self {
        Self {
            position,
            character,
            color,
        }
    }

    /// Construct a `Glyph` where `position` is already the final quad rect.
    /// Use this when you have precomputed the glyph quad (including offset).
    ///
    /// Example:
    /// If you call:
    ///     Glyph::from_rect(Rect::new(10.0, 20.0, 25.0, 35.0), 'A', [1.0,1.0,1.0,1.0]),
    /// you're saying "draw an 'A' exactly in this rectangle from (10,20) to (25,35),
    /// including all font offsets and sizing already calculated."
    pub fn from_rect(position: Rect, character: char, color: [f32; 4]) -> Self {
        Self {
            position,
            character,
            color,
        }
    }
}

/// Controls how glyph profile meshes are rendered for shadow casting and physics interactions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GlyphProfileRenderMode {
    /// No glyph profile mesh rendering - text will not cast shadows or participate in physics.
    None,
    /// Render glyph profile mesh with depth-only material for shadow casting only.
    DepthOnly,
    /// Render glyph profile mesh with visible material for debugging shadow casting.
    Visible,
}

impl Default for GlyphProfileRenderMode {
    fn default() -> Self {
        Self::DepthOnly
    }
}

/// A 3D text mesh component that manages glyph rendering through atlas-based meshes.
/// This component handles the creation and updating of text geometry, materials, and child entities.
#[derive(Component)]
pub struct Text3d {
    font: Handle<Font>,
    missing: Vec<char>,
    glyphs: Box<[Glyph]>,
    meshes: HashMap<usize, Handle<Mesh>>,
    child_entities: HashMap<usize, Entity>,
    glyph_profile_mesh: Option<Handle<Mesh>>,
    glyph_profile_child_entity: Option<Entity>,
    // Track last observed mesh attribute counts per-atlas to avoid
    // logging identical information every frame.
    last_mesh_counts: HashMap<usize, (usize, usize, usize, usize)>,
    // Remember which glyph characters we've already logged as missing an
    // atlas so we don't flood the logs repeatedly each frame.
    #[cfg(debug_assertions)]
    logged_missing_glyphs: HashSet<char>,
    // Cache hash of glyph data to avoid rebuilding meshes when unchanged
    glyphs_hash: Option<u64>,
    // Controls how glyph profile meshes are rendered for shadow casting
    glyph_profile_render_mode: GlyphProfileRenderMode,
}

// TODO: Our font atlas implementation vs. that of bevy's Text2d is justified due
// to the fundamentally different requirements.
// However we might want to consider:
// Extracting common atlas management utilities into shared functions
// Using Bevy's TextureAtlasLayout for consistency
// Potentially contributing SDF support back to Bevy's text system for future unification

impl Text3d {
    /// Creates a new empty Text3d component with the specified font.
    pub fn new(font: Handle<Font>) -> Self {
        Self {
            font,
            missing: Default::default(),
            glyphs: Default::default(),
            meshes: Default::default(),
            child_entities: Default::default(),
            glyph_profile_mesh: None,
            glyph_profile_child_entity: None,
            last_mesh_counts: Default::default(),
            #[cfg(debug_assertions)]
            logged_missing_glyphs: Default::default(),
            glyphs_hash: None,
            glyph_profile_render_mode: Default::default(),
        }
    }

    /// For testing purposes, clones the font handle, glyphs, and missing characters.
    /// Mesh and entity-related fields are left empty so they can be recreated by the engine.
    pub fn clone_for_spawn(&self) -> Self {
        Self {
            font: self.font.clone(),
            missing: self.missing.clone(),
            glyphs: self.glyphs.clone(),
            meshes: Default::default(),
            child_entities: Default::default(),
            glyph_profile_mesh: None,
            glyph_profile_child_entity: None,
            last_mesh_counts: Default::default(),
            #[cfg(debug_assertions)]
            logged_missing_glyphs: Default::default(),
            glyphs_hash: None,
            glyph_profile_render_mode: self.glyph_profile_render_mode,
        }
    }

    /// Returns the asset ID of the font used by this Text3d.
    pub fn font_id(&self) -> AssetId<Font> {
        self.font.id()
    }

    /// Returns the font name if available, for debugging purposes.
    /// This extracts the font family name from the TTF name table.
    pub fn font_name(&self, fonts: &Assets<Font>) -> Option<String> {
        fonts.get(&self.font).and_then(|font| font.name())
    }

    /// Adds code points to the list of missing glyphs that need atlas generation.
    pub fn add_missing(&mut self, missing: &[char]) {
        self.missing.extend_from_slice(missing);
    }

    /// Returns a slice of the current glyphs to be rendered.
    pub fn set_glyphs(&mut self, glyphs: Box<[Glyph]>) {
        self.glyphs = glyphs;
        self.glyphs_hash = None; // Invalidate cached hash
    }

    /// Return a clone of the glyph profile mesh handle if one has been created.
    pub fn glyph_profile_mesh_handle(&self) -> Option<Handle<Mesh>> {
        self.glyph_profile_mesh.clone()
    }

    /// Returns a slice of the current glyphs to be rendered.
    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    /// Sets the glyph profile render mode for this Text3d component.
    /// This controls how glyph profile meshes are rendered for shadow casting and physics.
    ///
    /// The default mode is `GlyphProfileRenderMode::DepthOnly`.
    pub fn with_glyph_profile_mode(mut self, mode: GlyphProfileRenderMode) -> Self {
        self.glyph_profile_render_mode = mode;
        self
    }

    /// Clears the glyph profile mesh and child entity, forcing recreation on the next frame.
    /// Used to change text glyph shadow caster tessellation quality settings.
    pub fn clear_glyph_profile(&mut self) {
        // TODO: Investigate need to implement a system to cleanup(or update?) existing abandoned child entities
        // TODO: Investigate doing this more gracefully without orphaning entities
        // TODO: Consider async glyph recreation - what if there are many text entities?
        debug!(
            "Clearing glyph profile for Text3d - mesh: {:?}, child: {:?}",
            self.glyph_profile_mesh, self.glyph_profile_child_entity
        );
        self.glyph_profile_mesh = None;
        self.glyph_profile_child_entity = None;
    }
}

/// System that processes missing code points for Text3d entities and adds them to font atlases.
/// This ensures that all required glyphs are available in texture atlases before mesh creation.
pub fn update_font_atlases_system(
    mut query: Query<&mut Text3d>,
    mut atlases: ResMut<FontAtlasSets>,
    mut textures: ResMut<Assets<Image>>,
    fonts: Res<Assets<Font>>,
) {
    for mut text_mesh in query.iter_mut() {
        if !text_mesh.missing.is_empty() {
            atlases.add_code_points(
                &text_mesh.missing,
                text_mesh.font_id(),
                &fonts,
                &mut textures,
            );
            text_mesh.missing.clear();
        }
    }
}

/// Create meshes for each text character in a `Text3d` that doesn't have
/// a mesh yet.
pub fn create_shadow_caster_meshes_system(
    mut query: Query<(Entity, &mut Text3d)>,
    mut commands: Commands,
    font_atlas: Res<FontAtlasSets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GlyphMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut depth_only_materials: ResMut<Assets<crate::pipeline_material::DepthOnlyMaterial>>,
    fonts: Res<Assets<Font>>,
    config: Res<TextMeshPluginConfig>,
) {
    for (entity, mut text_glyph_mesh) in query.iter_mut() {
        let has_atlas_meshes = !text_glyph_mesh.meshes.is_empty();
        let needs_glyph_profile = text_glyph_mesh.glyph_profile_mesh.is_none();

        // Only create atlas meshes if they don't exist yet
        if !has_atlas_meshes {
            // Ensure text glyph atlas exists for the chosen font.
            if let Some(data) = font_atlas.sets.get(&text_glyph_mesh.font.id()) {
                info!(
                    "Creating individual text character meshes for Text3d entity ({:?}) with text '{}'",
                    entity,
                    text_glyph_mesh
                        .glyphs()
                        .iter()
                        .map(|glyph| glyph.character)
                        .collect::<String>()
                );
                commands.entity(entity).insert((
                    Visibility::default(),
                    InheritedVisibility::default(),
                    ViewVisibility::default(),
                ));

                // Create meshes needed to cast shadows for this Text3d (a group of characters).
                let mut needed_atlases: HashSet<usize> = HashSet::new();
                let mut atlas_to_glyphs: HashMap<usize, Vec<char>> = HashMap::new();
                // Collect missing glyph characters we haven't warned about yet so
                // we can update `text_mesh` after finishing the iteration and
                // avoid mutable/immutable borrow conflicts.
                let mut newly_missing: Vec<char> = Vec::new();
                for glyph in text_glyph_mesh.glyphs.iter() {
                    if let Some(atlas_idx) = data.atlas(glyph.character) {
                        debug!(
                            "Text3d ({:?}) glyph={} needs atlas={}",
                            entity, glyph.character, atlas_idx
                        );
                        needed_atlases.insert(atlas_idx);
                        atlas_to_glyphs
                            .entry(atlas_idx)
                            .or_insert(Vec::new())
                            .push(glyph.character);
                    } else {
                        #[cfg(debug_assertions)]
                        if !text_glyph_mesh
                            .logged_missing_glyphs
                            .contains(&glyph.character)
                        {
                            newly_missing.push(glyph.character);
                        }
                        #[cfg(not(debug_assertions))]
                        {
                            newly_missing.push(glyph.character);
                        }
                    }
                }
                for code_point in newly_missing.into_iter() {
                    debug!(
                        "Text3d ({:?}) code point glyph={} needs to be added to atlas.",
                        entity, code_point
                    );
                    #[cfg(debug_assertions)]
                    text_glyph_mesh.logged_missing_glyphs.insert(code_point);
                }

                // Create meshes and child entities for each needed atlas.
                for &i in needed_atlases.iter() {
                    if text_glyph_mesh.meshes.contains_key(&i) {
                        continue;
                    }

                    let mesh = meshes.add(Mesh::new(
                        PrimitiveTopology::TriangleList,
                        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                    ));
                    // Insert handle into the Text3d so other systems can find it
                    let mesh_clone = mesh.clone();
                    text_glyph_mesh.meshes.insert(i, mesh_clone.clone());
                    // Instrument: check the atlas texture presence before adding
                    let atlas_texture = data.atlas_texture(i);
                    if atlas_texture.is_none() {
                        info!(
                            "Texture for glyphs {:?} in atlas {} (Text3d entity {:?}, font_id={:?}) - will be generated next frame",
                            atlas_to_glyphs.get(&i).unwrap_or(&vec![]),
                            i,
                            entity,
                            text_glyph_mesh.font_id()
                        );
                        continue;
                    }
                    let atlas_texture_handle = atlas_texture.unwrap();
                    let glyph_material_handle = materials.add(GlyphMaterial {
                        params: crate::pipeline_material::GlyphMaterialUniform::default(),
                        sdf_texture: atlas_texture_handle.clone(),
                    });

                    debug!(
                        "Creating material for atlas {}: material_handle={:?}, atlas_texture_handle={:?}",
                        i, glyph_material_handle, atlas_texture_handle
                    );

                    let child = commands
                        .spawn((
                            Mesh3d(mesh_clone),
                            bevy::pbr::MeshMaterial3d(glyph_material_handle.clone()),
                            bevy::prelude::Transform::IDENTITY,
                            bevy::prelude::Visibility::Inherited,
                            bevy::prelude::InheritedVisibility::default(),
                            ViewVisibility::default(),
                            NotShadowCaster,
                        ))
                        .id();

                    commands.entity(entity).add_child(child);
                    text_glyph_mesh.child_entities.insert(i, child);
                    info!(
                        "Created Mesh3d child entity={:?} for Text3d parent entity={:?}",
                        child, entity
                    );
                }
            } else {
                debug!(
                    "Font data not found for Text3d ({:?}) font_id={:?}. Will try again next frame.",
                    entity,
                    text_glyph_mesh.font_id()
                );
                continue;
            }
        }

        // Create glyph profile mesh for shadow casting if needed
        if needs_glyph_profile {
            if let Some(data) = font_atlas.sets.get(&text_glyph_mesh.font.id()) {
                info!(
                    "Creating glyph profile mesh for Text3d ({:?}) with quality {:?}",
                    entity, config.text_mesh_shadow_quality
                );
                let mut combined_mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                );
                let mut vertices = Vec::new();
                let mut indices = Vec::new();
                let mut vertex_offset = 0u32;

                for glyph in text_glyph_mesh.glyphs.iter() {
                    if let Some(info) = data.glyph_info(glyph.character) {
                        if let Some(glyph_profile_mesh) =
                            fonts.get(&text_glyph_mesh.font).and_then(|font_handle| {
                                font_handle.generate_glyph_profile_mesh_with_tolerance(
                                    info.id,
                                    glyph.character,
                                    config.text_mesh_shadow_quality.tolerance(),
                                )
                            })
                        {
                            // Transform vertices by glyph position
                            let offset = glyph.position.min + info.offset;
                            if let (
                                Some(VertexAttributeValues::Float32x3(positions)),
                                Some(Indices::U32(mesh_indices)),
                            ) = (
                                glyph_profile_mesh.attribute(Mesh::ATTRIBUTE_POSITION),
                                glyph_profile_mesh.indices(),
                            ) {
                                for pos in positions {
                                    vertices.push([
                                        pos[0] * config.font_scale.x + offset.x,
                                        pos[1] * config.font_scale.y + offset.y,
                                        pos[2],
                                    ]);
                                }
                                for idx in mesh_indices {
                                    indices.push(idx + vertex_offset);
                                }
                                vertex_offset += positions.len() as u32;
                            }
                        }
                    }
                }

                if !vertices.is_empty() {
                    // Insert positions and flat normals (Z+) so pipelines that expect normals
                    // (like StandardMaterial for shadow passes) have valid vertex inputs.
                    let vert_count = vertices.len();
                    combined_mesh.insert_attribute(
                        Mesh::ATTRIBUTE_POSITION,
                        VertexAttributeValues::Float32x3(vertices),
                    );
                    combined_mesh.insert_attribute(
                        Mesh::ATTRIBUTE_NORMAL,
                        VertexAttributeValues::Float32x3(vec![[0.0, 0.0, 1.0]; vert_count]),
                    );
                    combined_mesh.insert_indices(Indices::U32(indices));
                    let glyph_profile_mesh_handle = meshes.add(combined_mesh);
                    text_glyph_mesh.glyph_profile_mesh = Some(glyph_profile_mesh_handle.clone());
                    info!(
                        "Created shadow caster mesh with {} vertices for Text3d entity ({:?}) with text '{}'",
                        vert_count,
                        entity,
                        text_glyph_mesh
                            .glyphs()
                            .iter()
                            .map(|glyph| glyph.character)
                            .collect::<String>()
                    );

                    // Create child entity for glyph profile mesh based on the configured render mode
                    match text_glyph_mesh.glyph_profile_render_mode {
                        GlyphProfileRenderMode::None => {
                            // No glyph profile rendering
                        }
                        GlyphProfileRenderMode::DepthOnly => {
                            let depth_mat = depth_only_materials
                                .add(crate::pipeline_material::DepthOnlyMaterial {});
                            let glyph_profile_child = commands
                                .spawn((
                                    Mesh3d(glyph_profile_mesh_handle.clone()),
                                    MeshMaterial3d(depth_mat),
                                    Transform::IDENTITY,
                                    Visibility::Hidden,
                                    InheritedVisibility::default(),
                                    ViewVisibility::default(),
                                    OnlyShadowCaster,
                                ))
                                .id();
                            commands.entity(entity).add_child(glyph_profile_child);
                            text_glyph_mesh.glyph_profile_child_entity = Some(glyph_profile_child);
                            info!(
                                "Created depth-only glyph profile child entity={:?} for Text3d(entity={:?})",
                                glyph_profile_child, entity
                            );
                        }
                        GlyphProfileRenderMode::Visible => {
                            let debug_mat = standard_materials.add(StandardMaterial {
                                base_color: Color::BLACK,
                                alpha_mode: AlphaMode::Opaque,
                                ..Default::default()
                            });
                            let visible_glyph_profile = commands
                                .spawn((
                                    Mesh3d(glyph_profile_mesh_handle.clone()),
                                    MeshMaterial3d(debug_mat),
                                    Transform::IDENTITY,
                                    Visibility::Inherited,
                                    InheritedVisibility::default(),
                                    ViewVisibility::default(),
                                ))
                                .id();
                            commands.entity(entity).add_child(visible_glyph_profile);
                            text_glyph_mesh.glyph_profile_child_entity =
                                Some(visible_glyph_profile);
                            info!(
                                "Spawned visible debug glyph profile child={:?}",
                                visible_glyph_profile
                            );
                        }
                    }
                }
            }
        }
    }
}

/// System that updates atlas mesh geometry for Text3d entities.
/// Rebuilds mesh geometry when glyphs change, using change detection to avoid unnecessary work.
pub fn update_atlas_meshes_system(
    mut query: Query<(Entity, &mut Text3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    font_atlas: Res<FontAtlasSets>,
    config: Res<TextMeshPluginConfig>,
) {
    debug!("Running update_atlas_mesh system");
    for (entity, mut text_mesh) in query.iter_mut() {
        debug!("Processing Text3d entity: {:?}", entity);
        let Some(data) = font_atlas.sets.get(&text_mesh.font.id()) else {
            continue;
        };

        // Compute hash of current glyph data for change detection
        let mut hasher = DefaultHasher::new();
        for glyph in text_mesh.glyphs.iter() {
            glyph.character.hash(&mut hasher);
            glyph.position.min.x.to_bits().hash(&mut hasher);
            glyph.position.min.y.to_bits().hash(&mut hasher);
            glyph.position.max.x.to_bits().hash(&mut hasher);
            glyph.position.max.y.to_bits().hash(&mut hasher);
            glyph.color[0].to_bits().hash(&mut hasher);
            glyph.color[1].to_bits().hash(&mut hasher);
            glyph.color[2].to_bits().hash(&mut hasher);
            glyph.color[3].to_bits().hash(&mut hasher);
        }
        let current_hash = hasher.finish();

        // Skip mesh rebuild if glyphs haven't changed and meshes already exist
        // Unless the user has changed the tessellation quality, in which case
        // we need to rebuild the meshes.
        if text_mesh.glyphs_hash == Some(current_hash) && !text_mesh.meshes.is_empty() {
            debug!(
                "Text3d ({:?}) glyphs unchanged, skipping mesh rebuild",
                entity
            );
            continue;
        }

        // Update cached hash
        text_mesh.glyphs_hash = Some(current_hash);

        // Build positioned glyphs for those glyphs that have atlas UVs.
        let mut positioned: Vec<PositionedGlyph> = Vec::new();
        // Because we need to mutate `text_mesh.logged_missing_glyphs`, collect
        // newly-missing glyphs first to avoid mutable/immutable borrow conflicts.
        let mut newly_missing: Vec<char> = Vec::new();
        for glyph in text_mesh.glyphs.iter() {
            let info_opt = data.glyph_info(glyph.character);
            if info_opt.is_none() {
                info!(
                    "Text3d ({:?}) for ({}) not ready; will be available in a future frame once atlas generation completes",
                    entity, glyph.character
                );
                continue;
            }
            let info = info_opt.unwrap();

            match data.atlas(glyph.character) {
                Some(atlas_idx) => {
                    if let Some(uv_rect) = data.find_glyph_rect(info.id) {
                        let min = glyph.position.min + info.offset;
                        let size_scaled = info.size * config.font_scale.truncate();
                        let pos_rect = Rect::from_corners(min, min + size_scaled);
                        positioned.push(PositionedGlyph {
                            position: pos_rect,
                            uv: uv_rect,
                            index: atlas_idx,
                            color: glyph.color,
                        });
                    } else {
                        warn!(
                            "Text3d ({:?}) glyph={} has atlas entry but no uv rect; skipping quad",
                            entity, glyph.character
                        );
                    }
                }
                None => {
                    #[cfg(debug_assertions)]
                    if !text_mesh.logged_missing_glyphs.contains(&glyph.character) {
                        newly_missing.push(glyph.character);
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        newly_missing.push(glyph.character);
                    }
                }
            }
        }

        for c in newly_missing.into_iter() {
            warn!("Text3d ({:?}) glyph={} has no atlas!", entity, c);
            #[cfg(debug_assertions)]
            text_mesh.logged_missing_glyphs.insert(c);
        }

        // Iterate each atlas mesh and write geometry; only log counts when
        // they change to avoid repeating identical messages every frame.
        let atlas_pairs: Vec<(usize, Handle<Mesh>)> = text_mesh
            .meshes
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        for (index, mesh_handle) in atlas_pairs.into_iter() {
            if let Some(mesh) = meshes.get_mut(&mesh_handle) {
                let mut builder = Text3dBuilder::new(mesh);
                for pg in positioned.iter().filter(|pg| pg.index == index) {
                    builder.append_glyph(&pg.position, &pg.uv, &pg.color);
                }

                let pos_count = match mesh.attribute(ATTRIBUTE_POSITION) {
                    Some(VertexAttributeValues::Float32x2(v)) => v.len(),
                    _ => 0,
                };
                let uv_count = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
                    Some(VertexAttributeValues::Float32x2(v)) => v.len(),
                    _ => 0,
                };
                let color_count = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
                    Some(VertexAttributeValues::Float32x4(v)) => v.len(),
                    _ => 0,
                };
                let index_count = match mesh.indices() {
                    Some(Indices::U32(i)) => i.len(),
                    _ => 0,
                };

                let counts = (pos_count, uv_count, color_count, index_count);
                let prev_counts = text_mesh.last_mesh_counts.get(&index).cloned();
                let should_log = match prev_counts {
                    Some(prev) => prev != counts,
                    None => true,
                };

                // Use INFO so this is visible with the default RUST_LOG used by examples
                debug!(
                    "Text3d ({:?}) atlas={} -> positions={} uvs={} colors={} indices={}",
                    entity, index, pos_count, uv_count, color_count, index_count
                );
                text_mesh.last_mesh_counts.insert(index, counts);

                if index_count == 0 || pos_count == 0 {
                    // If there are no indices/positions then the mesh has no geometry
                    // and the glyphs won't render. Log an explicit warning to aid
                    // runtime debugging.
                    warn!(
                        "Text3d ({:?}) atlas={} has no geometry: positions={} indices={}; this will result in invisible glyphs",
                        entity, index, pos_count, index_count
                    );
                }

                let child_opt = text_mesh.child_entities.get(&index).cloned();
                if child_opt.is_none() {
                    warn!(
                        "Text3d ({:?}) atlas={} has no child entity yet",
                        entity, index
                    );
                } else if should_log && let Some(child) = child_opt {
                    debug!(
                        "Text3d ({:?}) atlas={} child_entity={:?}",
                        entity, index, child
                    );
                }
            }
        }
    }
}

/// Helper for building mesh geometry for text glyphs.
/// Clears existing mesh data and appends glyph quads with proper vertex attributes.
struct Text3dBuilder<'a> {
    index: u32,
    mesh: &'a mut Mesh,
}

// if we want to move text Z-direction relative to the other text, we may need f32x3 here..
// pub const ATTRIBUTE_TEXT_POSITION: MeshVertexAttribute =
//     MeshVertexAttribute::new("Text_Position", 988540917, VertexFormat::Float32x2);

impl<'a> Text3dBuilder<'a> {
    /// Creates a new Text3dBuilder, clearing all existing mesh attributes and indices.
    /// Ensures the mesh has the required vertex attribute arrays initialized.
    fn new(mesh: &'a mut Mesh) -> Self {
        if !mesh.contains_attribute(ATTRIBUTE_POSITION) {
            mesh.insert_attribute(ATTRIBUTE_POSITION, VertexAttributeValues::Float32x2(vec![]));
        }
        // Ensure the standard 3-component position attribute exists too so
        // that attaching Bevy's `StandardMaterial` (which expects a
        // Float32x3 `POSITION`) won't fail pipeline specialization.
        if !mesh.contains_attribute(Mesh::ATTRIBUTE_POSITION) {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                VertexAttributeValues::Float32x3(vec![]),
            );
        }

        if !mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0) {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_UV_0,
                VertexAttributeValues::Float32x2(vec![]),
            );
        }
        // FIXME: 4 vertices with f32x4 for color seems overkill for a single color glyph
        if !mesh.contains_attribute(Mesh::ATTRIBUTE_COLOR) {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                VertexAttributeValues::Float32x4(vec![]),
            );
        }
        if mesh.indices().is_none() {
            mesh.insert_indices(Indices::U32(vec![]));
        }

        if let Some(VertexAttributeValues::Float32x2(vertices)) =
            mesh.attribute_mut(ATTRIBUTE_POSITION)
        {
            vertices.clear();
        }
        if let Some(VertexAttributeValues::Float32x3(std_positions)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            std_positions.clear();
        }
        if let Some(VertexAttributeValues::Float32x2(uvs)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            uvs.clear();
        }
        if let Some(VertexAttributeValues::Float32x4(colors)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
        {
            colors.clear();
        }
        if let Some(Indices::U32(indices)) = mesh.indices_mut() {
            indices.clear();
        }

        Self { index: 0, mesh }
    }

    /// Appends a glyph quad to the mesh with the specified position, UV coordinates, and color.
    /// Creates 4 vertices and 6 indices (2 triangles) for the glyph quad.
    fn append_glyph(&mut self, position: &Rect, uv: &Rect, color: &[f32; 4]) {
        if let Some(VertexAttributeValues::Float32x2(vertices)) =
            self.mesh.attribute_mut(ATTRIBUTE_POSITION)
        {
            let rect = *position;
            vertices.push([rect.min.x, rect.min.y]);
            vertices.push([rect.max.x, rect.min.y]);
            vertices.push([rect.max.x, rect.max.y]);
            vertices.push([rect.min.x, rect.max.y]);
        }

        // Also write a 3-component POSITION with z=0 for compatibility with
        // standard Bevy materials / PBR pipelines.
        if let Some(VertexAttributeValues::Float32x3(std_positions)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            let rect = *position;
            std_positions.push([rect.min.x, rect.min.y, 0.0]);
            std_positions.push([rect.max.x, rect.min.y, 0.0]);
            std_positions.push([rect.max.x, rect.max.y, 0.0]);
            std_positions.push([rect.min.x, rect.max.y, 0.0]);
        }

        if let Some(VertexAttributeValues::Float32x2(uvs)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            let rect = *uv;
            uvs.push([rect.min.x, rect.min.y]);
            uvs.push([rect.max.x, rect.min.y]);
            uvs.push([rect.max.x, rect.max.y]);
            uvs.push([rect.min.x, rect.max.y]);
        }

        if let Some(VertexAttributeValues::Float32x4(colors)) =
            self.mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
        {
            colors.extend([*color; 4]); // FIXME: this wastes a ton of memory..
        }

        if let Some(Indices::U32(indices)) = self.mesh.indices_mut() {
            let base = self.index * 4;
            indices.extend([base, base + 1, base + 3, base + 1, base + 2, base + 3]);
        }

        self.index += 1;
    }
}

/// Configuration options for the TextMeshPlugin.
#[derive(Clone, Debug, Resource)]
pub struct TextMeshPluginConfig {
    /// The quality level used for tessellating glyph curves into triangles for shadow casting.
    /// Lower quality values produce fewer triangles but lower visual fidelity.
    /// Higher quality values produce more triangles but better visual fidelity.
    pub text_mesh_shadow_quality: GlyphTessellationQuality,
    /// Global scale applied to all text fonts.
    pub font_scale: Vec3,
}

impl Default for TextMeshPluginConfig {
    fn default() -> Self {
        Self {
            text_mesh_shadow_quality: GlyphTessellationQuality::High,
            font_scale: Vec3::ONE,
        }
    }
}

/// Plugin that adds the text mesh pipeline systems to the app.
/// Manages the lifecycle of 3D text rendering including atlas generation, mesh creation, and updates.
pub struct TextMeshPlugin {
    config: TextMeshPluginConfig,
}

impl TextMeshPlugin {
    /// Creates a new TextMeshPlugin with default configuration.
    pub fn new() -> Self {
        Self {
            config: Default::default(),
        }
    }

    /// Creates a new TextMeshPlugin with custom configuration.
    pub fn with_config(config: TextMeshPluginConfig) -> Self {
        Self { config }
    }
}

impl Default for TextMeshPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TextMeshPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .add_systems(Update, update_font_atlases_system)
            .add_systems(
                PostUpdate,
                create_shadow_caster_meshes_system.after(update_font_atlases_system),
            )
            .add_systems(
                PostUpdate,
                update_atlas_meshes_system.after(create_shadow_caster_meshes_system),
            );
    }
}

// Note: OnlyShadowCaster is provided by Bevy's light module (patched Bevy), and is used as
// a marker component to ensure hidden entities still contribute to shadow passes.
