//! Collider shape support functions for collision detection.

use glam::{Mat4, Vec3};

use crate::ecs::components::physics::ColliderShape;
use crate::ecs::components::transform::GlobalTransform;

/// Axis-aligned bounding box for broadphase collision detection.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsAabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl PhysicsAabb {
    /// Test whether two AABBs overlap.
    #[inline]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }
}

impl ColliderShape {
    /// GJK/EPA support function. Returns the farthest point in the given direction.
    #[inline]
    pub fn support(&self, direction: Vec3, transform: &GlobalTransform) -> Vec3 {
        let mat = transform.0;
        let inv = mat.inverse();
        // Transform direction to local space (rotation only, no translation)
        let local_dir = inv.transform_vector3(direction).normalize_or_zero();

        let local_point = match self {
            Self::Sphere { radius } => {
                if local_dir == Vec3::ZERO {
                    Vec3::ZERO
                } else {
                    local_dir * *radius
                }
            }
            Self::Box { half_extents } => Vec3::new(
                if local_dir.x >= 0.0 {
                    half_extents.x
                } else {
                    -half_extents.x
                },
                if local_dir.y >= 0.0 {
                    half_extents.y
                } else {
                    -half_extents.y
                },
                if local_dir.z >= 0.0 {
                    half_extents.z
                } else {
                    -half_extents.z
                },
            ),
            Self::Capsule {
                radius,
                half_height,
            } => {
                // Capsule along Y axis
                let base = if local_dir.y >= 0.0 {
                    Vec3::new(0.0, *half_height, 0.0)
                } else {
                    Vec3::new(0.0, -*half_height, 0.0)
                };
                if local_dir == Vec3::ZERO {
                    base
                } else {
                    base + local_dir * *radius
                }
            }
            Self::Cylinder {
                radius,
                half_height,
            } => {
                // Cylinder along Y axis
                let y = if local_dir.y >= 0.0 {
                    *half_height
                } else {
                    -*half_height
                };
                let xz = Vec3::new(local_dir.x, 0.0, local_dir.z);
                let xz_len = xz.length();
                let xz_point = if xz_len > 1e-6 {
                    xz * (*radius / xz_len)
                } else {
                    Vec3::ZERO
                };
                Vec3::new(xz_point.x, y, xz_point.z)
            }
            Self::ConvexHull { points } => {
                if points.is_empty() {
                    Vec3::ZERO
                } else {
                    let mut best = points[0];
                    let mut best_dot = best.dot(local_dir);
                    for p in &points[1..] {
                        let d = p.dot(local_dir);
                        if d > best_dot {
                            best_dot = d;
                            best = *p;
                        }
                    }
                    best
                }
            }
        };

        // Transform back to world space
        mat.transform_point3(local_point)
    }

    /// Compute the world-space AABB for this shape.
    #[inline]
    pub fn compute_aabb(&self, transform: &GlobalTransform) -> PhysicsAabb {
        let mat = transform.0;

        match self {
            Self::Sphere { radius } => {
                let center = mat.transform_point3(Vec3::ZERO);
                // Extract scale to account for non-uniform scaling
                let scale_x = mat.x_axis.truncate().length();
                let scale_y = mat.y_axis.truncate().length();
                let scale_z = mat.z_axis.truncate().length();
                let max_scale = scale_x.max(scale_y).max(scale_z);
                let world_radius = *radius * max_scale;
                PhysicsAabb {
                    min: center - Vec3::splat(world_radius),
                    max: center + Vec3::splat(world_radius),
                }
            }
            Self::Box { half_extents } => aabb_from_extents(*half_extents, mat),
            Self::Capsule {
                radius,
                half_height,
            } => {
                // Treat as bounding box of the capsule
                let extents = Vec3::new(*radius, *half_height + *radius, *radius);
                aabb_from_extents(extents, mat)
            }
            Self::Cylinder {
                radius,
                half_height,
            } => {
                let extents = Vec3::new(*radius, *half_height, *radius);
                aabb_from_extents(extents, mat)
            }
            Self::ConvexHull { points } => {
                if points.is_empty() {
                    let center = mat.transform_point3(Vec3::ZERO);
                    return PhysicsAabb {
                        min: center,
                        max: center,
                    };
                }
                let mut min = Vec3::splat(f32::MAX);
                let mut max = Vec3::splat(f32::MIN);
                for p in points {
                    let wp = mat.transform_point3(*p);
                    min = min.min(wp);
                    max = max.max(wp);
                }
                PhysicsAabb { min, max }
            }
        }
    }
}

/// Compute world-space AABB from local half-extents and a transform matrix.
#[inline]
fn aabb_from_extents(half_extents: Vec3, mat: Mat4) -> PhysicsAabb {
    let center = mat.transform_point3(Vec3::ZERO);

    // For each world axis, compute the extent by projecting the local box axes
    let abs_col0 = mat.x_axis.truncate().abs();
    let abs_col1 = mat.y_axis.truncate().abs();
    let abs_col2 = mat.z_axis.truncate().abs();

    let extent = abs_col0 * half_extents.x + abs_col1 * half_extents.y + abs_col2 * half_extents.z;

    PhysicsAabb {
        min: center - extent,
        max: center + extent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_aabb() {
        let shape = ColliderShape::Sphere { radius: 1.0 };
        let transform = GlobalTransform(Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0)));
        let aabb = shape.compute_aabb(&transform);

        let eps = 1e-5;
        assert!((aabb.min - Vec3::new(-1.0, 4.0, -1.0)).length() < eps);
        assert!((aabb.max - Vec3::new(1.0, 6.0, 1.0)).length() < eps);
    }

    #[test]
    fn test_box_aabb() {
        let shape = ColliderShape::Box {
            half_extents: Vec3::new(1.0, 2.0, 3.0),
        };
        let transform = GlobalTransform(Mat4::IDENTITY);
        let aabb = shape.compute_aabb(&transform);

        let eps = 1e-5;
        assert!((aabb.min - Vec3::new(-1.0, -2.0, -3.0)).length() < eps);
        assert!((aabb.max - Vec3::new(1.0, 2.0, 3.0)).length() < eps);
    }

    #[test]
    fn test_aabb_overlap() {
        let a = PhysicsAabb {
            min: Vec3::new(-1.0, -1.0, -1.0),
            max: Vec3::new(1.0, 1.0, 1.0),
        };
        let b = PhysicsAabb {
            min: Vec3::new(0.5, 0.5, 0.5),
            max: Vec3::new(2.0, 2.0, 2.0),
        };
        let c = PhysicsAabb {
            min: Vec3::new(2.0, 2.0, 2.0),
            max: Vec3::new(3.0, 3.0, 3.0),
        };
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn test_sphere_support() {
        let shape = ColliderShape::Sphere { radius: 2.0 };
        let transform = GlobalTransform(Mat4::from_translation(Vec3::new(0.0, 5.0, 0.0)));
        let support = shape.support(Vec3::Y, &transform);
        let eps = 1e-5;
        assert!((support - Vec3::new(0.0, 7.0, 0.0)).length() < eps);
    }
}
