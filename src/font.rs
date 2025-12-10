use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bevy::{
    asset::{AssetId, AssetLoader, Assets, Handle, LoadContext, RenderAssetUsages, io::Reader},
    math::{Rect, UVec2, Vec2},
    prelude::{
        App, Asset, AssetApp, DynamicTextureAtlasBuilder, Image, Plugin, Resource,
        TextureAtlasLayout,
    },
    reflect::TypePath,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
#[allow(unused_imports)]
use bevy_log::{debug, error, info, warn};
use fdsm::{
    bezier::{Point, Segment, scanline::FillRule},
    shape::{Contour, Shape},
    transform::Transform,
};
use image::{GrayImage, RgbaImage};
use nalgebra::{Affine2, Similarity2, Vector2};
pub use owned_ttf_parser::GlyphId;
use owned_ttf_parser::{AsFaceRef, OutlineBuilder, Rect as TtfRect};
use thiserror::Error;

/// The location of a glyph in an atlas,
/// and how it should be positioned when placed.
#[derive(Clone, Debug)]
pub struct GlyphAtlasLocation {
    pub glyph_index: usize,
}

// From font.rs
#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub id: GlyphId,
    pub advance: Vec2,
    pub offset: Vec2,
    pub size: Vec2,
}

#[derive(Asset, TypePath, Clone)]
pub struct Font {
    pub(crate) face: Arc<owned_ttf_parser::OwnedFace>,
}

impl Font {
    pub fn from(face: owned_ttf_parser::OwnedFace) -> Self {
        let font = Self {
            face: Arc::new(face),
        };
        // Try to log the font name for debugging
        if let Some(name) = font.name() {
            debug!("Font asset instantiated from parsed TTF face: {}", name);
        } else {
            warn!("Font asset instantiated from parsed TTF face with unknown name");
        }
        font
    }

    /// Returns the font name if available, for debugging purposes.
    /// Attempts to extract the font family name from the TTF name table.
    pub fn name(&self) -> Option<String> {
        let face = self.face.as_ref().as_face_ref();
        // Try to get the font family name (name ID 1) in English (platform 3, encoding 1)
        for name in face.names() {
            if name.name_id == 1
                && name.platform_id == owned_ttf_parser::PlatformId::Windows
                && name.encoding_id == 1
            {
                // Try UTF-8 first
                if let Ok(name_str) = String::from_utf8(name.name.to_vec()) {
                    return Some(name_str);
                }
                // Try UTF-16 if UTF-8 fails
                if name.name.len() % 2 == 0 {
                    let utf16: Vec<u16> = name
                        .name
                        .chunks(2)
                        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                        .collect();
                    if let Ok(name_str) = String::from_utf16(&utf16) {
                        return Some(name_str);
                    }
                }
            }
        }
        None
    }

    /// Get information about a glyph given its Unicode code point.
    pub fn glyph(&self, code_point: char) -> Option<GlyphInfo> {
        let face = self.face.clone();
        let face = face.as_ref().as_face_ref();
        let id = match face.glyph_index(code_point) {
            Some(id) => id,
            None => {
                error!("Glyph not found for code point: {}", code_point);
                return None;
            }
        };

        let bounds = match face.glyph_bounding_box(id) {
            Some(bbox) => bbox,
            None => {
                debug!(
                    "No bounding box for glyph: {:?}; using zero rect (expected for empty glyphs)",
                    id
                );
                TtfRect {
                    x_min: 0,
                    y_min: 0,
                    x_max: 0,
                    y_max: 0,
                }
            }
        };

        let units_per_em = face.units_per_em();
        if units_per_em == 0 {
            error!(
                "Font face has units_per_em == 0; cannot compute scale for glyph {:?}",
                id
            );
            let advance = Vec2::ZERO;
            let offset = Vec2::ZERO;
            let size = Vec2::ZERO;
            return Some(GlyphInfo {
                id,
                advance,
                offset,
                size,
            });
        }

        let scale = 1f32 / units_per_em as f32;

        let advance = Vec2::new(
            face.glyph_hor_advance(id).unwrap_or_default() as f32,
            face.glyph_ver_advance(id).unwrap_or_default() as f32,
        ) * scale;

        let offset = Vec2::new(bounds.x_min as f32, bounds.y_min as f32) * scale;

        let size = Vec2::new(
            (bounds.x_max - bounds.x_min) as f32,
            (bounds.y_max - bounds.y_min) as f32,
        ) * scale;

        Some(GlyphInfo {
            id,
            advance,
            offset,
            size,
        })
    }

    /// Load the shape of a glyph from the font face using its GlyphId.
    pub fn load_from_face(
        face: &owned_ttf_parser::Face,
        glyph_id: GlyphId,
        code_point: char,
    ) -> fdsm::shape::Shape<fdsm::shape::Contour> {
        let mut builder = ShapeBuilder {
            shape: Shape::default(),
            start_point: None,
            last_point: None,
        };
        face.outline_glyph(glyph_id, &mut builder);
        debug!(
            "Loaded shape from face for glyph {:?} ('{}') with {} contours",
            glyph_id,
            code_point,
            builder.shape.contours.len()
        );
        builder.shape
    }

    /// Generate a signed distance field (SDF) image for the given glyph.
    pub fn generate(&self, glyph_id: GlyphId, code_point: char, range: f64) -> Option<Image> {
        let face = self.face.clone();
        let face = face.as_ref().as_face_ref();

        debug!(
            "Generating SDF image for glyph {:?} ('{}', range={:?})",
            glyph_id, code_point, range
        );

        let units_per_em = face.units_per_em();

        if units_per_em == 0 {
            error!(
                "Font face has units_per_em == 0; cannot compute SDF generation scale for glyph {:?} ('{}')",
                glyph_id, code_point
            );
            return None;
        }

        // Normalize glyph coordinates so that the font's
        // em square is 100x100 units for SDF generation
        // Sweet spot for SDF generation scale is best between 0.01 and 0.2
        let scale = (1.0f64 / units_per_em as f64) * 100f64;
        if !(0.01..=0.2).contains(&scale) {
            warn!(
                "SDF generation scale ({}) is outside the optimal range (0.01-0.2). This may result in poor quality glyph rendering. Check the font's units_per_em value ({}).",
                scale, units_per_em
            );
        }

        debug!("SDF generation scale: {}", scale);

        let bbox = match face.glyph_bounding_box(glyph_id) {
            Some(bbox) => bbox,
            None => return Some(Self::transparent_placeholder_image(glyph_id, code_point)),
        };

        let transformation = nalgebra::convert::<_, Affine2<f64>>(Similarity2::new(
            Vector2::new(
                range - bbox.x_min as f64 * scale,
                range - bbox.y_min as f64 * scale,
            ),
            0.0,
            scale,
        ));

        let mut shape = Self::load_from_face(face, glyph_id, code_point);
        shape.transform(&transformation);

        let width = ((bbox.x_max as f64 - bbox.x_min as f64) * scale + range * 2f64).ceil() as u32;
        let height = ((bbox.y_max as f64 - bbox.y_min as f64) * scale + range * 2f64).ceil() as u32;

        if width == 0 || height == 0 {
            error!(
                "Computed zero dimensions for glyph texture {:?} ('{}'): {}x{}",
                glyph_id, code_point, width, height
            );
            return None;
        }

        let prepared_shape = shape.prepare();
        let mut sdf = GrayImage::new(width, height);
        fdsm::generate::generate_sdf(&prepared_shape, range, &mut sdf);
        fdsm::render::correct_sign_sdf(&mut sdf, &prepared_shape, FillRule::Nonzero);

        let mut msdf_rgba = RgbaImage::new(width, height);
        for (output, luma) in msdf_rgba.chunks_exact_mut(4).zip(sdf.iter()) {
            output.copy_from_slice(&[0, 0, 0, *luma]);
        }

        debug!(
            "Successfully generated glyph texture {:?} ('{}', width={}, height={})",
            glyph_id, code_point, width, height
        );
        Some(Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            msdf_rgba.into_raw(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        ))
    }

    fn transparent_placeholder_image(glyph_id: GlyphId, code_point: char) -> Image {
        debug!(
            "Glyph {:?} ('{}') has no bounding box; returning transparent 1x1 image. This may be expected if the font contains empty glyphs",
            glyph_id, code_point
        );
        let width = 1u32;
        let height = 1u32;
        let rgba = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));

        Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba.into_raw(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
    }

    pub fn line_gap(&self) -> f64 {
        let face = self.face.clone();
        let face = face.as_ref().as_face_ref();
        let units_per_em = face.units_per_em();
        if units_per_em == 0 {
            error!("Font face has units_per_em == 0; cannot compute line gap. Returning 0.");
            return 0.0;
        }
        let gap = face.height() as f64 / units_per_em as f64;
        debug!("Computed line gap from face metrics: {}", gap);
        gap
    }
}

// Borrowed from MIT Licensed https://gitlab.com/Kyarei/fdsm
#[derive(Debug)]
struct ShapeBuilder {
    shape: Shape<Contour>,
    start_point: Option<Point>,
    last_point: Option<Point>,
}

impl OutlineBuilder for ShapeBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        if let Some(contour) = self.shape.contours.last_mut()
            && self.start_point != self.last_point
        {
            contour.segments.push(Segment::line(
                self.last_point.unwrap(),
                self.start_point.unwrap(),
            ));
        }
        self.start_point = Some(Point::new(x.into(), y.into()));
        self.last_point = self.start_point;
        self.shape.contours.push(Contour::default());
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let next_point = Point::new(x.into(), y.into());
        self.shape
            .contours
            .last_mut()
            .unwrap()
            .segments
            .push(Segment::line(self.last_point.unwrap(), next_point));
        self.last_point = Some(next_point);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let next_point = Point::new(x.into(), y.into());
        self.shape
            .contours
            .last_mut()
            .unwrap()
            .segments
            .push(Segment::quad(
                self.last_point.unwrap(),
                Point::new(x1.into(), y1.into()),
                next_point,
            ));
        self.last_point = Some(next_point);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let next_point = Point::new(x.into(), y.into());
        self.shape
            .contours
            .last_mut()
            .unwrap()
            .segments
            .push(Segment::cubic(
                self.last_point.unwrap(),
                Point::new(x1.into(), y1.into()),
                Point::new(x2.into(), y2.into()),
                next_point,
            ));
        self.last_point = Some(next_point);
    }

    fn close(&mut self) {
        if let Some(contour) = self.shape.contours.last_mut()
            && self.start_point != self.last_point
        {
            contour.segments.push(Segment::line(
                self.last_point.take().unwrap(),
                self.start_point.take().unwrap(),
            ));
        }
    }
}

pub struct FontAtlas {
    pub dynamic_texture_atlas_builder: DynamicTextureAtlasBuilder,
    pub glyph_locations: HashMap<GlyphId, GlyphAtlasLocation>,
    pub atlas_layout: TextureAtlasLayout,
    pub texture: Handle<Image>,
}

impl FontAtlas {
    pub fn new(textures: &mut Assets<Image>, size: UVec2) -> FontAtlas {
        debug!("Creating FontAtlas with size: {:?}", size);
        let texture = textures.add(Image::new_fill(
            Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        ));
        Self {
            atlas_layout: TextureAtlasLayout::new_empty(size),
            glyph_locations: HashMap::default(),
            dynamic_texture_atlas_builder: DynamicTextureAtlasBuilder::new(size, 1),
            texture,
        }
    }

    pub fn insert_glyph(
        &mut self,
        textures: &mut Assets<Image>,
        glyph_id: GlyphId,
        texture: &Image,
    ) -> bool {
        debug!(
            "FontAtlas::insert_glyph called for glyph id: {:?}",
            glyph_id
        );
        if let Some(atlas_image) = textures.get_mut(&self.texture) {
            match self.dynamic_texture_atlas_builder.add_texture(
                &mut self.atlas_layout,
                texture,
                atlas_image,
            ) {
                Ok(index) => {
                    debug!("Added glyph id {:?} at atlas index {}", glyph_id, index);
                    self.glyph_locations
                        .insert(glyph_id, GlyphAtlasLocation { glyph_index: index });
                    true
                }
                Err(err) => {
                    error!(
                        "DynamicTextureAtlasBuilder failed to add glyph {:?}: {:?}",
                        glyph_id, err
                    );
                    false
                }
            }
        } else {
            error!(
                "Atlas image handle not found in Assets<Image> when adding glyph {:?}",
                glyph_id
            );
            false
        }
    }

    pub fn get_glyph_rect(&self, glyph_id: GlyphId, range: u8) -> Option<Rect> {
        debug!("Getting glyph rect for {:?} with range {}", glyph_id, range);
        self.glyph_locations
            .get(&glyph_id)
            .and_then(|location| {
                debug!(
                    "Found atlas location {:?} for glyph {:?}",
                    location, glyph_id
                );
                self.atlas_layout.textures.get(location.glyph_index)
            })
            .map(|rect| {
                let size_inv = 1f32 / self.atlas_layout.size.as_vec2();
                let rect = rect.inflate(-(range as i32));
                let result = Rect::from_corners(
                    (rect.min.as_vec2() * size_inv).into(),
                    (rect.max.as_vec2() * size_inv).into(),
                );
                debug!("Glyph {:?} rect (normalized): {:?}", glyph_id, result);
                result
            })
    }
}

impl std::fmt::Debug for FontAtlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontAtlas")
            .field("glyph_locations", &self.glyph_locations)
            .field("atlas_layout", &self.atlas_layout)
            .field("texture", &self.texture)
            .finish()
    }
}

// Borrowed from bevy's built-in bevy_text
/// Identifies a font configuration in a [`FontAtlasSet`].
///
/// For 3D text, we use a simple key since we don't have font size/smoothing variations.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct FontAtlasKey(pub u32);

/// A map of font configurations to their corresponding [`FontAtlas`]es, for a given font face.
///
/// Provides the interface for adding and retrieving SDF glyphs, and manages the [`FontAtlas`]es.
///
/// A `FontAtlasSet` is an [`Asset`].
///
/// There is one `FontAtlasSet` for each font face.
#[derive(Debug, TypePath, Asset)]
pub struct FontAtlasSet {
    font_atlases: HashMap<FontAtlasKey, Vec<FontAtlas>>,
    added: HashSet<char>,
    code_point_to_atlas: HashMap<char, usize>,
    code_point_to_glyph_info: HashMap<char, GlyphInfo>,
    range: u8,
    line_gap: f64,
}

impl Default for FontAtlasSet {
    fn default() -> Self {
        FontAtlasSet {
            font_atlases: HashMap::with_capacity_and_hasher(1, Default::default()),
            added: Default::default(),
            code_point_to_atlas: Default::default(),
            code_point_to_glyph_info: Default::default(),
            range: 6,
            line_gap: 0.0,
        }
    }
}

impl FontAtlasSet {
    pub fn from(face: &Font) -> Self {
        debug!(
            "Creating FontAtlasSet from face with line_gap {}",
            face.line_gap()
        );
        Self {
            font_atlases: HashMap::with_capacity_and_hasher(1, Default::default()),
            added: Default::default(),
            code_point_to_atlas: Default::default(),
            code_point_to_glyph_info: Default::default(),
            range: 6,
            line_gap: face.line_gap(),
        }
    }

    /// Check if a glyph is present in the atlas set.
    pub fn has_glyph(&self, code_point: char) -> bool {
        self.added.contains(&code_point)
    }

    pub fn add_glyph_to_atlas(
        &mut self,
        code_point: char,
        font: &Font,
        textures: &mut Assets<Image>,
    ) -> Option<usize> {
        debug!(
            "FontAtlasSet::add_glyph_to_atlas called for code point '{}'",
            code_point
        );
        self.added.insert(code_point);
        let Some(glyph_info) = font.glyph(code_point) else {
            warn!("No glyph generated for {code_point}. No glyph data available");
            return None;
        };
        debug!(
            "Got glyph info for {}: id={:?}, advance={:?}, offset={:?}, size={:?}",
            code_point, glyph_info.id, glyph_info.advance, glyph_info.offset, glyph_info.size
        );
        self.code_point_to_glyph_info
            .insert(code_point, glyph_info.clone());
        let glyph_texture = match font.generate(glyph_info.id, code_point, self.range as f64) {
            Some(tex) => tex,
            None => {
                warn!(
                    "Glyph for {code_point:?} produced no texture (likely empty glyph); skipping atlas insertion"
                );
                return None;
            }
        };
        debug!(
            "Generated texture for {} ({}x{})",
            code_point,
            glyph_texture.width(),
            glyph_texture.height()
        );

        // Use a single key for all 3D text atlases
        let atlas_key = FontAtlasKey(0);

        let font_atlases = self.font_atlases.entry(atlas_key).or_insert_with(|| vec![]);

        let atlas_index = font_atlases
            .iter_mut()
            .enumerate()
            .find_map(|(index, atlas)| {
                atlas
                    .insert_glyph(textures, glyph_info.id, &glyph_texture)
                    .then_some(index)
            })
            .unwrap_or_else(|| {
                let glyph_max_size: u32 = glyph_texture.width().max(glyph_texture.height());
                let containing = (1u32 << (32 - glyph_max_size.leading_zeros())).max(1024);
                debug!(
                    "No existing atlas could fit glyph {}, creating new atlas of size {}",
                    code_point, containing
                );
                let mut atlas = FontAtlas::new(textures, UVec2::new(containing, containing));
                if !atlas.insert_glyph(textures, glyph_info.id, &glyph_texture) {
                    error!("Failed adding glyph!");
                }
                let idx = font_atlases.len();
                font_atlases.push(atlas);
                idx
            });
        self.code_point_to_atlas.insert(code_point, atlas_index);
        debug!(
            "Inserted code point '{}' into atlas {}",
            code_point, atlas_index
        );
        Some(atlas_index)
    }

    /// Get information about a glyph given its Unicode code point.
    pub fn glyph_info(&self, code_point: char) -> Option<&GlyphInfo> {
        self.code_point_to_glyph_info.get(&code_point)
    }

    /// Get the total number of atlases in the set.
    pub fn atlas_count(&self) -> usize {
        self.font_atlases
            .values()
            .map(|atlases| atlases.len())
            .sum()
    }

    pub fn atlas(&self, code_point: char) -> Option<usize> {
        self.code_point_to_atlas.get(&code_point).copied()
    }

    /// Get the atlas index for a given code point.
    pub fn find_glyph_rect(&self, glyph_id: GlyphId) -> Option<Rect> {
        let atlas_key = FontAtlasKey(0);
        self.font_atlases.get(&atlas_key).and_then(|atlases| {
            atlases
                .iter()
                .find_map(|atlas| atlas.get_glyph_rect(glyph_id, self.range))
        })
    }

    /// Get the texture handle for a given atlas index.
    pub fn atlas_texture(&self, atlas: usize) -> Option<Handle<Image>> {
        let atlas_key = FontAtlasKey(0);
        self.font_atlases
            .get(&atlas_key)
            .and_then(|atlases| atlases.get(atlas))
            .map(|font_atlas| font_atlas.texture.clone())
    }

    /// Get the line gap for the font.
    pub fn line_gap(&self) -> f32 {
        self.line_gap as f32
    }
}

/// A map of font faces to their corresponding [`FontAtlasSet`]s.
#[derive(Debug, Default, Resource)]
pub struct FontAtlasSets {
    // PERF: in theory this could be optimized with Assets storage ... consider making some fast "simple" AssetMap
    pub(crate) sets: HashMap<AssetId<Font>, FontAtlasSet>,
}

impl FontAtlasSets {
    /// Get a reference to the [`FontAtlasSet`] with the given font asset id.
    pub fn get(&self, id: impl Into<AssetId<Font>>) -> Option<&FontAtlasSet> {
        let id: AssetId<Font> = id.into();
        self.sets.get(&id)
    }
    /// Get a mutable reference to the [`FontAtlasSet`] with the given font asset id.
    pub fn get_mut(&mut self, id: impl Into<AssetId<Font>>) -> Option<&mut FontAtlasSet> {
        let id: AssetId<Font> = id.into();
        self.sets.get_mut(&id)
    }
    /// Add the given code points to the font atlas set for the specified font asset id.
    /// If the font atlas set does not exist, it will be created.
    /// If a code point is already present, it will be skipped.
    pub fn add_code_points(
        &mut self,
        chars: &[char],
        font_id: AssetId<Font>,
        fonts: &Assets<Font>,
        textures: &mut Assets<Image>,
    ) {
        debug!(
            "FontAtlasSets::add_code_points: Received font_id: {:?}",
            font_id
        );
        let Some(font) = fonts.get(font_id) else {
            error!(
                "FontAtlasSets::add_code_points: Font {:?} not found in Assets<Font>!",
                font_id
            );
            return;
        };
        debug!(
            "Adding {} code points to font id {:?}",
            chars.len(),
            font_id
        );
        let font_atlas_set = self.sets.entry(font_id).or_insert_with(|| {
            debug!("Inserting new FontAtlasSet entry.");
            FontAtlasSet::from(font)
        });
        for code_point in chars {
            if !font_atlas_set.has_glyph(*code_point) {
                match font_atlas_set.add_glyph_to_atlas(*code_point, font, textures) {
                    Some(i) => {
                        debug!("Code point {code_point} added to glyph atlas {i}!");
                    }
                    None => {
                        warn!("Failed to generate or insert glyph for code point: {code_point:?}");
                    }
                }
            }
        }
    }
}

// From loader.rs
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum FontLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    FontInvalid(#[from] owned_ttf_parser::FaceParsingError),
}

#[derive(Default)]
pub struct FontLoader;

impl AssetLoader for FontLoader {
    type Asset = Font;
    type Settings = ();
    type Error = FontLoaderError;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let face = owned_ttf_parser::OwnedFace::from_vec(bytes, 0)?;
        Ok(Font::from(face))
    }

    fn extensions(&self) -> &[&str] {
        &["ttf", "otf"]
    }
}

pub struct FontPlugin;

impl Plugin for FontPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Font>()
            .init_asset::<FontAtlasSet>()
            .init_asset_loader::<FontLoader>()
            .init_resource::<FontAtlasSets>();
    }
}
