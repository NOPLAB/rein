//! Frustum culling system for ECS entities.

use crate::ecs::components::rendering::{CameraComponent, FrustumCullable, MeshRenderer, Visible};
use crate::ecs::components::transform::GlobalTransform;
use crate::renderer::culling::Frustum;
use crate::renderer::geometry::Aabb;
use crate::renderer::viewer::Viewer;
use glam::Vec3;

/// Compute a world-space AABB from a local AABB and a world transform matrix.
fn compute_world_aabb(local_aabb: Aabb, world_matrix: glam::Mat4) -> Aabb {
    let corners = local_aabb.corners();
    let transformed: Vec<Vec3> = corners
        .iter()
        .map(|c| world_matrix.transform_point3(*c))
        .collect();
    Aabb::from_points(transformed)
}

/// Frustum culling system.
///
/// Finds the active camera, builds its view frustum, then tests each
/// `FrustumCullable` entity's world-space AABB. Entities that pass the test
/// receive the `Visible` marker; those that fail have it removed.
pub fn culling_system(world: &mut hecs::World) {
    // Find the active camera and build frustum.
    let frustum = {
        let mut found = None;
        for (cam, _global) in
            world.query_mut::<hecs::Without<(&CameraComponent, &GlobalTransform), &MeshRenderer>>()
        {
            if cam.active {
                let vp = cam.camera.view_projection_matrix();
                found = Some(Frustum::from_view_projection(vp));
                break;
            }
        }
        match found {
            Some(f) => f,
            None => return, // No active camera, skip culling.
        }
    };

    // Test each FrustumCullable entity and collect results.
    let mut to_add_visible: Vec<hecs::Entity> = Vec::new();
    let mut to_remove_visible: Vec<hecs::Entity> = Vec::new();

    for (entity, renderer, global) in world
        .query_mut::<hecs::With<(hecs::Entity, &MeshRenderer, &GlobalTransform), &FrustumCullable>>(
        )
    {
        let local_aabb = renderer.mesh.0.aabb();
        let world_aabb = compute_world_aabb(local_aabb, global.0);

        if frustum.contains_aabb(&world_aabb) {
            to_add_visible.push(entity);
        } else {
            to_remove_visible.push(entity);
        }
    }

    // Apply changes via CommandBuffer.
    let mut cmd = hecs::CommandBuffer::new();
    for entity in to_add_visible {
        if !world.satisfies::<&Visible>(entity) {
            cmd.insert_one(entity, Visible);
        }
    }
    for entity in to_remove_visible {
        if world.satisfies::<&Visible>(entity) {
            cmd.remove_one::<Visible>(entity);
        }
    }
    cmd.run_on(world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    #[test]
    fn test_compute_world_aabb() {
        let local = Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let translation = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
        let world = compute_world_aabb(local, translation);

        let eps = 1e-5;
        assert!((world.min.x - 4.0).abs() < eps);
        assert!((world.max.x - 6.0).abs() < eps);
    }
}
