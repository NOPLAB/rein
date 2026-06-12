//! Broadphase collision detection using spatial hash grid.

use std::collections::{HashMap, HashSet};

use glam::Vec3;

use crate::ecs::components::physics::{Collider, RigidBody, RigidBodyType};
use crate::ecs::components::transform::GlobalTransform;

use super::collider::PhysicsAabb;

type CellKey = (i32, i32, i32);
type CellEntry = (hecs::Entity, PhysicsAabb, RigidBodyType);

/// Spatial hash grid broadphase for O(n) average-case pair detection.
pub struct SpatialHashGrid {
    cell_size: f32,
    cells: HashMap<CellKey, Vec<CellEntry>>,
}

impl Default for SpatialHashGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl SpatialHashGrid {
    pub fn new() -> Self {
        Self {
            cell_size: 2.0,
            cells: HashMap::new(),
        }
    }

    /// Compute cell coordinates for a point.
    #[inline]
    fn cell_coords(&self, point: Vec3) -> (i32, i32, i32) {
        let inv = 1.0 / self.cell_size;
        (
            (point.x * inv).floor() as i32,
            (point.y * inv).floor() as i32,
            (point.z * inv).floor() as i32,
        )
    }

    /// Find all pairs of entities whose AABBs overlap.
    ///
    /// Only returns pairs where at least one entity is dynamic.
    pub fn find_pairs(&mut self, world: &hecs::World) -> Vec<(hecs::Entity, hecs::Entity)> {
        self.cells.clear();

        // Collect all entries and determine max AABB size for cell sizing
        let mut entries: Vec<(hecs::Entity, PhysicsAabb, RigidBodyType)> = Vec::new();
        let mut max_extent: f32 = 0.0;

        for (entity, (collider, transform, rb)) in
            &mut world.query::<(&Collider, &GlobalTransform, &RigidBody)>()
        {
            if collider.is_sensor {
                continue;
            }
            let mut adjusted_transform = *transform;
            if collider.offset != Vec3::ZERO {
                adjusted_transform.0 *= glam::Mat4::from_translation(collider.offset);
            }
            let aabb = collider.shape.compute_aabb(&adjusted_transform);

            let extent = (aabb.max - aabb.min).max_element();
            if extent > max_extent {
                max_extent = extent;
            }

            entries.push((entity, aabb, rb.body_type));
        }

        // Set cell size to 2x the max AABB extent (minimum 1.0)
        self.cell_size = (max_extent * 2.0).max(1.0);

        // Insert entries into cells
        for &(entity, ref aabb, body_type) in &entries {
            let min_cell = self.cell_coords(aabb.min);
            let max_cell = self.cell_coords(aabb.max);

            for cx in min_cell.0..=max_cell.0 {
                for cy in min_cell.1..=max_cell.1 {
                    for cz in min_cell.2..=max_cell.2 {
                        self.cells
                            .entry((cx, cy, cz))
                            .or_default()
                            .push((entity, *aabb, body_type));
                    }
                }
            }
        }

        // Find pairs within each cell
        let mut pairs = Vec::with_capacity(entries.len() * 4);
        let mut seen = HashSet::new();

        for cell in self.cells.values() {
            for i in 0..cell.len() {
                for j in (i + 1)..cell.len() {
                    let (entity_a, ref aabb_a, type_a) = cell[i];
                    let (entity_b, ref aabb_b, type_b) = cell[j];

                    // Skip static-static pairs
                    if type_a == RigidBodyType::Static && type_b == RigidBodyType::Static {
                        continue;
                    }

                    // Canonical ordering to avoid duplicates
                    let pair = if entity_a < entity_b {
                        (entity_a, entity_b)
                    } else {
                        (entity_b, entity_a)
                    };

                    if seen.contains(&pair) {
                        continue;
                    }

                    if aabb_a.overlaps(aabb_b) {
                        seen.insert(pair);
                        pairs.push(pair);
                    }
                }
            }
        }

        pairs
    }
}

/// Legacy alias for backward compatibility.
pub type SweepAndPrune = SpatialHashGrid;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::physics::{ColliderShape, RigidBody};
    use crate::ecs::components::transform::{GlobalTransform, Transform};
    use glam::Mat4;

    #[test]
    fn test_broadphase_overlapping() {
        let mut world = hecs::World::new();

        // Two overlapping spheres
        let _a = world.spawn((
            Transform::from_position(Vec3::new(0.0, 0.0, 0.0)),
            GlobalTransform(Mat4::IDENTITY),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Sphere { radius: 1.0 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        let _b = world.spawn((
            Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
            GlobalTransform(Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Sphere { radius: 1.0 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        let mut broadphase = SpatialHashGrid::new();
        let pairs = broadphase.find_pairs(&world);
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn test_broadphase_no_overlap() {
        let mut world = hecs::World::new();

        // Two far apart spheres
        let _a = world.spawn((
            Transform::from_position(Vec3::ZERO),
            GlobalTransform(Mat4::IDENTITY),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Sphere { radius: 0.5 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        let _b = world.spawn((
            Transform::from_position(Vec3::new(10.0, 0.0, 0.0)),
            GlobalTransform(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0))),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Sphere { radius: 0.5 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        let mut broadphase = SpatialHashGrid::new();
        let pairs = broadphase.find_pairs(&world);
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_broadphase_static_static_skipped() {
        let mut world = hecs::World::new();

        // Two overlapping static bodies - should NOT be returned
        world.spawn((
            Transform::identity(),
            GlobalTransform(Mat4::IDENTITY),
            RigidBody::new_static(),
            Collider {
                shape: ColliderShape::Sphere { radius: 1.0 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        world.spawn((
            Transform::identity(),
            GlobalTransform(Mat4::IDENTITY),
            RigidBody::new_static(),
            Collider {
                shape: ColliderShape::Sphere { radius: 1.0 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        let mut broadphase = SpatialHashGrid::new();
        let pairs = broadphase.find_pairs(&world);
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_broadphase_many_bodies() {
        let mut world = hecs::World::new();

        // Create a grid of overlapping spheres
        for i in 0..10 {
            for j in 0..10 {
                let pos = Vec3::new(i as f32 * 1.5, 0.0, j as f32 * 1.5);
                world.spawn((
                    Transform::from_position(pos),
                    GlobalTransform(Mat4::from_translation(pos)),
                    RigidBody::new_dynamic(1.0),
                    Collider {
                        shape: ColliderShape::Sphere { radius: 1.0 },
                        offset: Vec3::ZERO,
                        is_sensor: false,
                    },
                ));
            }
        }

        let mut broadphase = SpatialHashGrid::new();
        let pairs = broadphase.find_pairs(&world);
        // Should find some pairs (adjacent spheres overlap)
        assert!(!pairs.is_empty());
    }
}
