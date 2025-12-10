use bevy::{
    asset::RenderAssetUsages,
    prelude::{Mesh, debug, error, info},
};
use bevy_mesh::{Indices, PrimitiveTopology};
use fdsm::{bezier::Order, transform::Transform};
use lyon::{
    math::point as lyon_point,
    path::Path,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
};
use nalgebra::{Affine2, Similarity2, Vector2};
use owned_ttf_parser::AsFaceRef;

use crate::font::{Font, GlyphId};

impl Font {
    /// Generate a Glyph Profile, which is the final 3D mesh output of the tessellation process
    pub fn generate_glyph_profile_mesh_with_tolerance(
        &self,
        glyph_id: GlyphId,
        code_point: char,
        tolerance: f32,
    ) -> Option<Mesh> {
        let face = self.face.clone();
        let face = face.as_ref().as_face_ref();

        debug!(
            "Generating glyph profile mesh for glyph {:?} {:?}",
            code_point, glyph_id
        );

        let units_per_em = face.units_per_em();
        if units_per_em == 0 {
            error!(
                "Font face has units_per_em == 0; cannot compute glyph profile mesh scale for the glyph {:?} {:?}",
                code_point, glyph_id
            );
            return None;
        }

        let scale = 1.0f64 / units_per_em as f64;
        let bbox = match face.glyph_bounding_box(glyph_id) {
            Some(bbox) => bbox,
            None => {
                debug!(
                    "Glyph {:?} {:?} has no bounding box; returning empty mesh",
                    code_point, glyph_id
                );
                return Some(Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                ));
            }
        };

        // Load the glyph shape
        let mut shape = Self::load_from_face(face, glyph_id, code_point);

        // Transform to normalized coordinates first
        let normalization_transform = nalgebra::convert::<_, Affine2<f64>>(Similarity2::new(
            Vector2::new(-bbox.x_min as f64 * scale, -bbox.y_min as f64 * scale),
            0.0,
            scale,
        ));
        shape.transform(&normalization_transform);

        // Use lyon to tessellate the glyph contours into filled triangles.
        // Build a lyon path (glyph_outline) with all contours (each contour becomes a sub-path)
        let mut glyph_outline_builder = Path::builder();
        for contour in &shape.contours {
            if contour.segments.is_empty() {
                continue;
            }

            // Start the sub-path at the start of the first segment
            let first_seg = &contour.segments[0];
            let start = first_seg.start();
            glyph_outline_builder.begin(lyon_point(start.x as f32, start.y as f32));

            for segment in &contour.segments {
                match segment.order() {
                    Order::Linear => {
                        let end = segment.end();
                        glyph_outline_builder.line_to(lyon_point(end.x as f32, end.y as f32));
                    }
                    Order::Quadratic => {
                        let ctrl = segment.control_point(1);
                        let end = segment.end();
                        glyph_outline_builder.quadratic_bezier_to(
                            lyon_point(ctrl.x as f32, ctrl.y as f32),
                            lyon_point(end.x as f32, end.y as f32),
                        );
                    }
                    Order::Cubic => {
                        let ctrl1 = segment.control_point(1);
                        let ctrl2 = segment.control_point(2);
                        let end = segment.end();
                        glyph_outline_builder.cubic_bezier_to(
                            lyon_point(ctrl1.x as f32, ctrl1.y as f32),
                            lyon_point(ctrl2.x as f32, ctrl2.y as f32),
                            lyon_point(end.x as f32, end.y as f32),
                        );
                    }
                }
            }

            glyph_outline_builder.close();
        }

        let glyph_outline: Path = glyph_outline_builder.build();

        // If the path is empty, return an empty mesh (no geometry)
        if glyph_outline.iter().next().is_none() {
            info!(
                "No geometry generated for glyph {:?} {:?}; returning empty mesh",
                code_point, glyph_id
            );
            return Some(Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            ));
        }

        // Tessellate the path (glyph_outline) into triangles
        let mut geometry: VertexBuffers<[f32; 3], u32> = VertexBuffers::new();
        let mut fill_tessellator = FillTessellator::new();
        let mut fill_options = FillOptions::default();
        fill_options.tolerance = tolerance;

        match fill_tessellator.tessellate_path(
            &glyph_outline,
            &fill_options,
            &mut BuffersBuilder::new(&mut geometry, |v: FillVertex| {
                let p = v.position();
                [p.x, p.y, 0.0]
            }),
        ) {
            Ok(()) => {
                // success
                // convert lyon geometry into vertices/indices for Bevy mesh below
            }
            Err(err) => {
                error!(
                    "Tessellation failed for glyph {:?} {:?}: {:?}",
                    code_point, glyph_id, err
                );
                return None;
            }
        }

        let vertices: Vec<[f32; 3]> = geometry
            .vertices
            .into_iter()
            .map(|[x, y, z]| [x, y, z])
            .collect();
        let indices: Vec<u32> = geometry.indices;

        if vertices.is_empty() || indices.is_empty() {
            info!(
                "No geometry generated for glyph {:?} {:?}; returning empty mesh",
                code_point, glyph_id
            );
            return Some(Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            ));
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.insert_indices(Indices::U32(indices));

        info!(
            "Generated profile mesh for glyph {:?} {:?} with {} vertices and {} triangles",
            code_point,
            glyph_id,
            mesh.count_vertices(),
            mesh.indices()
                .map(|indices| indices.len() / 3)
                .expect("Mesh should have indices after tessellation")
        );

        Some(mesh)
    }
}
