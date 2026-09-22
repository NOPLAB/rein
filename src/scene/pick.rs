//! Ray casting against scene objects (mouse picking).

use glam::{Mat4, Vec3};

use crate::renderer::geometry::Aabb;

use super::mesh::MeshData;

/// A half-line in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// Start point.
    pub origin: Vec3,
    /// Unit direction.
    pub direction: Vec3,
}

impl Ray {
    /// A ray from `origin` towards `direction` (normalized here).
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
        }
    }

    /// Point at parameter `t`.
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// The same ray expressed in the space `world_from_local` maps *from* (i.e. the ray
    /// transformed by the inverse). The direction is **not** re-normalized so that the
    /// parameter `t` stays comparable with the world ray.
    pub fn to_local(&self, world_from_local: Mat4) -> Self {
        let inv = world_from_local.inverse();
        Self {
            origin: inv.transform_point3(self.origin),
            direction: inv.transform_vector3(self.direction),
        }
    }

    /// Slab test: distance along the ray to the box entry point, or `None` on a miss.
    /// A ray starting inside returns `Some(0.0)`.
    pub fn hit_aabb(&self, aabb: &Aabb) -> Option<f32> {
        let mut t_min = 0.0_f32;
        let mut t_max = f32::INFINITY;
        for axis in 0..3 {
            let o = self.origin[axis];
            let d = self.direction[axis];
            let (lo, hi) = (aabb.min[axis], aabb.max[axis]);
            if d.abs() < 1e-12 {
                if o < lo || o > hi {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d;
            let (t0, t1) = ((lo - o) * inv, (hi - o) * inv);
            let (t0, t1) = if t0 <= t1 { (t0, t1) } else { (t1, t0) };
            t_min = t_min.max(t0);
            t_max = t_max.min(t1);
            if t_min > t_max {
                return None;
            }
        }
        Some(t_min)
    }

    /// Möller–Trumbore, both faces. Returns the ray parameter of the hit.
    pub fn hit_triangle(&self, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
        let e1 = b - a;
        let e2 = c - a;
        let p = self.direction.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        let s = self.origin - a;
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(e1);
        let v = self.direction.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let t = e2.dot(q) * inv;
        (t > 1e-6).then_some(t)
    }

    /// Nearest triangle hit in a mesh (local space). Returns the ray parameter.
    pub fn hit_mesh(&self, mesh: &MeshData) -> Option<f32> {
        let mut best: Option<f32> = None;
        for tri in mesh.indices.as_chunks::<3>().0 {
            let (a, b, c) = (
                mesh.positions[tri[0] as usize],
                mesh.positions[tri[1] as usize],
                mesh.positions[tri[2] as usize],
            );
            if let Some(t) = self.hit_triangle(a, b, c) {
                if best.is_none_or(|b| t < b) {
                    best = Some(t);
                }
            }
        }
        best
    }

    /// Intersection with the plane through `point` with normal `normal`, if in front.
    pub fn hit_plane(&self, point: Vec3, normal: Vec3) -> Option<f32> {
        let denom = normal.dot(self.direction);
        if denom.abs() < 1e-9 {
            return None;
        }
        let t = (point - self.origin).dot(normal) / denom;
        (t >= 0.0).then_some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_and_triangle_hits() {
        let r = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z);
        let b = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!((r.hit_aabb(&b).unwrap() - 4.0).abs() < 1e-5);
        let miss = Ray::new(Vec3::new(3.0, 0.0, 5.0), Vec3::NEG_Z);
        assert!(miss.hit_aabb(&b).is_none());
        let inside = Ray::new(Vec3::ZERO, Vec3::X);
        assert_eq!(inside.hit_aabb(&b), Some(0.0));

        let t = r
            .hit_triangle(
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            )
            .unwrap();
        assert!((t - 5.0).abs() < 1e-5);
        // Behind the origin is not a hit.
        let back = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::NEG_Z);
        assert!(back
            .hit_triangle(
                Vec3::new(-1.0, -1.0, 0.0),
                Vec3::new(1.0, -1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            )
            .is_none());
    }

    #[test]
    fn mesh_hit_through_transform() {
        let cube = MeshData::cuboid(Vec3::ONE);
        let world = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        let ray = Ray::new(Vec3::new(10.0, 0.0, 5.0), Vec3::NEG_Z);
        let t = ray.to_local(world).hit_mesh(&cube).unwrap();
        assert!((t - 4.5).abs() < 1e-5);
        assert!((ray.at(t) - Vec3::new(10.0, 0.0, 0.5)).length() < 1e-5);
    }

    #[test]
    fn plane_hit() {
        let r = Ray::new(Vec3::new(0.0, 0.0, 2.0), Vec3::NEG_Z);
        assert!((r.hit_plane(Vec3::ZERO, Vec3::Z).unwrap() - 2.0).abs() < 1e-6);
        let parallel = Ray::new(Vec3::new(0.0, 0.0, 2.0), Vec3::X);
        assert!(parallel.hit_plane(Vec3::ZERO, Vec3::Z).is_none());
    }
}
