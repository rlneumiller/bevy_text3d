use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

/// Compute the minimum world-space Y coordinate from an iterator of
/// (GlobalTransform, Aabb) tuples. Returns `None` if the iterator is empty.
pub fn min_world_y_from_pairs<'a, I>(pairs: I) -> Option<f32>
where
    I: IntoIterator<Item = (&'a GlobalTransform, &'a Aabb)>,
{
    let mut min_world_y: f32 = f32::INFINITY;
    for (global, aabb) in pairs {
        let center: Vec3 = Vec3::from(aabb.center);
        let half: Vec3 = Vec3::from(aabb.half_extents);
        for &sx in &[-1.0f32, 1.0f32] {
            for &sy in &[-1.0f32, 1.0f32] {
                for &sz in &[-1.0f32, 1.0f32] {
                    let local_corner = center + Vec3::new(sx * half.x, sy * half.y, sz * half.z);
                    let world_corner = global.transform_point(local_corner);
                    min_world_y = min_world_y.min(world_corner.y);
                }
            }
        }
    }

    if min_world_y < f32::INFINITY {
        Some(min_world_y)
    } else {
        None
    }
}

/// Compute the vertical offset needed to move the lowest descendant AABB corner
/// to `ground_y`. Returns `Some((min_y, offset))` or `None` if no AABBs are
/// present.
pub fn compute_ground_offset(
    root: Entity,
    children: &Query<&Children>,
    global_aabb_query: &Query<(&GlobalTransform, &Aabb)>,
    ground_y: f32,
) -> Option<(f32, f32)> {
    // Collect matching pairs from the query
    let mut pairs: Vec<(GlobalTransform, Aabb)> = Vec::new();
    for child in children.iter_descendants(root) {
        if let Ok((global, aabb)) = global_aabb_query.get(child) {
            pairs.push((global.clone(), aabb.clone()));
        }
    }

    if let Some(min_world_y) = min_world_y_from_pairs(pairs.iter().map(|(g, a)| (g, a))) {
        let offset = ground_y - min_world_y;
        Some((min_world_y, offset))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_world_y_from_pairs_single() {
        let global = GlobalTransform::from_translation(Vec3::new(0.0, 1.5, 0.0));
        let aabb = Aabb::from_min_max(Vec3::new(-0.1, -0.2, -0.1), Vec3::new(0.1, 0.2, 0.1));
        // min corner y should be 1.5 - 0.2 = 1.3
        let got = min_world_y_from_pairs([(&global, &aabb)].into_iter()).unwrap();
        assert!((got - 1.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_min_world_y_from_pairs_multiple() {
        let g1 = GlobalTransform::from_translation(Vec3::new(0.0, 0.5, 0.0));
        let a1 = Aabb::from_min_max(Vec3::new(-0.1, -0.1, -0.1), Vec3::new(0.1, 0.1, 0.1));
        // min world y for first: 0.5 - 0.1 = 0.4
        let g2 = GlobalTransform::from_translation(Vec3::new(0.0, -0.4, 0.0));
        let a2 = Aabb::from_min_max(Vec3::new(-0.2, -0.3, -0.2), Vec3::new(0.2, 0.3, 0.2));
        // min world y for second: -0.4 - 0.3 = -0.7
        let got = min_world_y_from_pairs([(&g1, &a1), (&g2, &a2)].into_iter()).unwrap();
        assert!((got + 0.7).abs() < 1e-6);
    }
}
