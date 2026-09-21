//! Frustum culling for visibility determination
//!
//! Provides frustum extraction from view-projection matrices and
//! intersection tests with axis-aligned bounding boxes.

use super::geometry::Aabb;
use glam::{Mat4, Vec3, Vec4};

/// A plane in 3D space defined by the equation ax + by + cz + d = 0.
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    /// Normal vector (a, b, c) - not necessarily normalized.
    pub normal: Vec3,
    /// Distance from origin (d).
    pub distance: f32,
}

impl Plane {
    /// Create a new plane from normal and distance.
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    /// Create a plane from a Vec4 (xyz = normal, w = distance).
    pub fn from_vec4(v: Vec4) -> Self {
        Self {
            normal: Vec3::new(v.x, v.y, v.z),
            distance: v.w,
        }
    }

    /// Normalize the plane equation.
    pub fn normalize(&self) -> Self {
        let len = self.normal.length();
        if len > 0.0 {
            Self {
                normal: self.normal / len,
                distance: self.distance / len,
            }
        } else {
            *self
        }
    }

    /// Get the signed distance from a point to the plane.
    /// Positive = in front (same side as normal), Negative = behind.
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }
}

/// Result of a frustum intersection test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intersection {
    /// Completely outside the frustum.
    Outside,
    /// Completely inside the frustum.
    Inside,
    /// Partially inside (intersecting a plane).
    Intersecting,
}

/// View frustum defined by 6 planes.
///
/// The planes are oriented so that their normals point inward.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    /// Left, Right, Bottom, Top, Near, Far planes.
    pub planes: [Plane; 6],
}

impl Frustum {
    /// Extract frustum planes from a view-projection matrix.
    ///
    /// Uses the Gribb/Hartmann method for extracting planes from the
    /// combined view-projection matrix.
    pub fn from_view_projection(vp: Mat4) -> Self {
        // Row extraction for clip space to world space plane extraction
        // Each row of the transposed matrix gives us the plane coefficients
        let col0 = vp.col(0);
        let col1 = vp.col(1);
        let col2 = vp.col(2);
        let col3 = vp.col(3);

        // Gribb/Hartmann method - extract planes from matrix rows
        let planes = [
            // Left:   row3 + row0
            Plane::from_vec4(col3 + col0).normalize(),
            // Right:  row3 - row0
            Plane::from_vec4(col3 - col0).normalize(),
            // Bottom: row3 + row1
            Plane::from_vec4(col3 + col1).normalize(),
            // Top:    row3 - row1
            Plane::from_vec4(col3 - col1).normalize(),
            // Near:   row3 + row2 (for reverse-Z: row2)
            Plane::from_vec4(col3 + col2).normalize(),
            // Far:    row3 - row2 (for reverse-Z: row3 - row2)
            Plane::from_vec4(col3 - col2).normalize(),
        ];

        Self { planes }
    }

    /// Test if a point is inside the frustum.
    pub fn contains_point(&self, point: Vec3) -> bool {
        for plane in &self.planes {
            if plane.signed_distance(point) < 0.0 {
                return false;
            }
        }
        true
    }

    /// Test if an AABB intersects or is inside the frustum.
    ///
    /// Returns `Outside` if completely outside, `Inside` if completely inside,
    /// or `Intersecting` if partially inside.
    pub fn test_aabb(&self, aabb: &Aabb) -> Intersection {
        let mut result = Intersection::Inside;

        for plane in &self.planes {
            // Find the positive and negative vertices relative to the plane normal
            let p_vertex = Vec3::new(
                if plane.normal.x >= 0.0 {
                    aabb.max.x
                } else {
                    aabb.min.x
                },
                if plane.normal.y >= 0.0 {
                    aabb.max.y
                } else {
                    aabb.min.y
                },
                if plane.normal.z >= 0.0 {
                    aabb.max.z
                } else {
                    aabb.min.z
                },
            );

            let n_vertex = Vec3::new(
                if plane.normal.x >= 0.0 {
                    aabb.min.x
                } else {
                    aabb.max.x
                },
                if plane.normal.y >= 0.0 {
                    aabb.min.y
                } else {
                    aabb.max.y
                },
                if plane.normal.z >= 0.0 {
                    aabb.min.z
                } else {
                    aabb.max.z
                },
            );

            // If the positive vertex is outside, the entire AABB is outside
            if plane.signed_distance(p_vertex) < 0.0 {
                return Intersection::Outside;
            }

            // If the negative vertex is outside, we're intersecting
            if plane.signed_distance(n_vertex) < 0.0 {
                result = Intersection::Intersecting;
            }
        }

        result
    }

    /// Test if an AABB is at least partially inside the frustum.
    /// This is a faster test that only returns true/false.
    pub fn contains_aabb(&self, aabb: &Aabb) -> bool {
        self.test_aabb(aabb) != Intersection::Outside
    }

    /// Test if a sphere is at least partially inside the frustum.
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.signed_distance(center) < -radius {
                return false;
            }
        }
        true
    }
}

/// Helper struct for culling objects against a frustum.
pub struct FrustumCuller {
    frustum: Frustum,
}

impl FrustumCuller {
    /// Create a new frustum culler from a view-projection matrix.
    pub fn new(view_projection: Mat4) -> Self {
        Self {
            frustum: Frustum::from_view_projection(view_projection),
        }
    }

    /// Update the frustum with a new view-projection matrix.
    pub fn update(&mut self, view_projection: Mat4) {
        self.frustum = Frustum::from_view_projection(view_projection);
    }

    /// Get the underlying frustum.
    pub fn frustum(&self) -> &Frustum {
        &self.frustum
    }

    /// Test if an AABB should be culled (is outside the frustum).
    pub fn should_cull(&self, aabb: &Aabb) -> bool {
        !self.frustum.contains_aabb(aabb)
    }

    /// Test if a sphere should be culled.
    pub fn should_cull_sphere(&self, center: Vec3, radius: f32) -> bool {
        !self.frustum.contains_sphere(center, radius)
    }

    /// Filter a slice of items with AABBs, returning indices of visible items.
    pub fn filter_visible<T, F>(&self, items: &[T], get_aabb: F) -> Vec<usize>
    where
        F: Fn(&T) -> &Aabb,
    {
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.frustum.contains_aabb(get_aabb(item)))
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_plane_signed_distance() {
        // Plane at z=0, normal pointing in +Z direction
        let plane = Plane::new(Vec3::Z, 0.0);

        assert!(plane.signed_distance(Vec3::new(0.0, 0.0, 1.0)) > 0.0);
        assert!(plane.signed_distance(Vec3::new(0.0, 0.0, -1.0)) < 0.0);
        assert!((plane.signed_distance(Vec3::ZERO)).abs() < 0.0001);
    }

    #[test]
    fn test_frustum_contains_point() {
        // Simple orthographic-like frustum
        let vp =
            glam::camera::rh::proj::directx::orthographic(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0);
        let frustum = Frustum::from_view_projection(vp);

        // Point inside
        assert!(frustum.contains_point(Vec3::new(0.0, 0.0, -50.0)));

        // Point outside (beyond far plane)
        assert!(!frustum.contains_point(Vec3::new(0.0, 0.0, -150.0)));
    }

    #[test]
    fn test_aabb_inside_frustum() {
        let vp =
            glam::camera::rh::proj::directx::orthographic(-10.0, 10.0, -10.0, 10.0, 0.1, 100.0);
        let frustum = Frustum::from_view_projection(vp);

        // AABB completely inside
        let aabb = Aabb::new(Vec3::new(-5.0, -5.0, -50.0), Vec3::new(5.0, 5.0, -40.0));
        assert!(frustum.contains_aabb(&aabb));

        // AABB completely outside
        let aabb_outside = Aabb::new(Vec3::new(20.0, 20.0, -50.0), Vec3::new(30.0, 30.0, -40.0));
        assert!(!frustum.contains_aabb(&aabb_outside));
    }
}
