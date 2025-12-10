use std::{
    fs::File,
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;
use bevy_log::{info, warn};
use bevy_mesh::{Indices, VertexAttributeValues};

use crate::Text3d;

pub fn dump_glyph_profile_obj_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    query: Query<(Entity, &Text3d)>,
    meshes: Res<Assets<Mesh>>,
) {
    if keys.just_pressed(KeyCode::KeyO) {
        for (entity, text_mesh) in query.iter() {
            if let Some(outline_handle) = text_mesh.glyph_profile_mesh_handle() {
                if let Some(mesh) = meshes.get(&outline_handle) {
                    // gather positions and indices
                    let mut positions: Vec<[f32; 3]> = Vec::new();
                    let mut tris: Vec<[u32; 3]> = Vec::new();
                    if let Some(VertexAttributeValues::Float32x3(pos_attr)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    {
                        positions.extend_from_slice(pos_attr);
                    }
                    if let Some(Indices::U32(idx_attr)) = mesh.indices() {
                        for chunk in idx_attr.chunks(3) {
                            if chunk.len() == 3 {
                                tris.push([chunk[0], chunk[1], chunk[2]]);
                            }
                        }
                    }

                    if !positions.is_empty() && !tris.is_empty() {
                        let ts = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let text_preview: String = text_mesh
                            .glyphs()
                            .iter()
                            .take(10)
                            .map(|g| g.character)
                            .collect();
                        let safe_preview =
                            text_preview.replace(|c: char| !c.is_alphanumeric() && c != '_', "_");
                        let filename =
                            format!("debug_{}_{}_{}.obj", safe_preview, entity.index(), ts);
                        if let Ok(mut f) = File::create(&filename) {
                            for v in &positions {
                                let _ = writeln!(f, "v {} {} {}", v[0], v[1], v[2]);
                            }
                            for t in &tris {
                                let _ = writeln!(f, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1);
                            }
                            info!(
                                "Wrote debug glyph profile OBJ: {} (verts={}, tris={})",
                                filename,
                                positions.len(),
                                tris.len()
                            );
                        } else {
                            warn!("Failed to create debug glyph profile OBJ file");
                        }
                    } else {
                        info!(
                            "Glyph profile mesh had no positions/triangles to dump (verts={}, tris={})",
                            positions.len(),
                            tris.len()
                        );
                    }
                }
            }
        }
    }
}
