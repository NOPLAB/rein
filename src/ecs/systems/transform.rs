//! Transform hierarchy propagation system.

use crate::ecs::components::transform::{Children, GlobalTransform, Parent, Transform};

/// Propagate transforms through the Parent/Children hierarchy.
///
/// Phase 1: Update root entities (no Parent) - GlobalTransform = Transform.to_matrix()
/// Phase 2: Recursively propagate through Children hierarchy.
pub fn transform_system(world: &mut hecs::World) {
    // Phase 1: Root entities (entities with Transform + GlobalTransform but no Parent).
    // Collect root entities and their matrices first to avoid borrow conflicts.
    let roots: Vec<(hecs::Entity, glam::Mat4)> = world
        .query_mut::<hecs::Without<(&Transform, &GlobalTransform), &Parent>>()
        .into_iter()
        .map(|(entity, (transform, _))| (entity, transform.to_matrix()))
        .collect();

    for (entity, matrix) in &roots {
        if let Ok(mut global) = world.get::<&mut GlobalTransform>(*entity) {
            global.0 = *matrix;
        }
    }

    // Phase 2: Propagate through hierarchy.
    // Collect root entities that have children to start traversal.
    let root_with_children: Vec<(hecs::Entity, glam::Mat4)> = roots
        .iter()
        .filter(|(entity, _)| world.satisfies::<&Children>(*entity).unwrap_or(false))
        .copied()
        .collect();

    for (entity, parent_matrix) in root_with_children {
        propagate_children(world, entity, parent_matrix);
    }
}

/// Recursively propagate GlobalTransform to children.
fn propagate_children(world: &mut hecs::World, parent: hecs::Entity, parent_global: glam::Mat4) {
    // Get the list of children (clone to release borrow).
    let children = match world.get::<&Children>(parent) {
        Ok(c) => c.0.clone(),
        Err(_) => return,
    };

    for child in children {
        // Compute child's global transform.
        let child_global = match world.get::<&Transform>(child) {
            Ok(transform) => parent_global * transform.to_matrix(),
            Err(_) => parent_global,
        };

        // Update the child's GlobalTransform.
        if let Ok(mut global) = world.get::<&mut GlobalTransform>(child) {
            global.0 = child_global;
        }

        // Recurse into grandchildren.
        if world.satisfies::<&Children>(child).unwrap_or(false) {
            propagate_children(world, child, child_global);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::transform::{Children, GlobalTransform, Parent, Transform};
    use glam::{Mat4, Vec3};

    #[test]
    fn test_root_entity_propagation() {
        let mut world = hecs::World::new();

        let pos = Vec3::new(1.0, 2.0, 3.0);
        let entity = world.spawn((Transform::from_position(pos), GlobalTransform::default()));

        transform_system(&mut world);

        let global = world.get::<&GlobalTransform>(entity).unwrap();
        let expected = Mat4::from_translation(pos);
        assert_eq!(global.0, expected);
    }

    #[test]
    fn test_parent_child_propagation() {
        let mut world = hecs::World::new();

        // Create parent at position (1, 0, 0)
        let parent_pos = Vec3::new(1.0, 0.0, 0.0);
        let parent = world.spawn((
            Transform::from_position(parent_pos),
            GlobalTransform::default(),
        ));

        // Create child at local position (0, 2, 0)
        let child_pos = Vec3::new(0.0, 2.0, 0.0);
        let child = world.spawn((
            Transform::from_position(child_pos),
            GlobalTransform::default(),
            Parent(parent),
        ));

        // Set up Children on parent
        world.insert_one(parent, Children(vec![child])).unwrap();

        transform_system(&mut world);

        // Child's global position should be (1, 2, 0)
        let child_global = world.get::<&GlobalTransform>(child).unwrap();
        let expected_pos = parent_pos + child_pos;
        let actual_pos = child_global.0.transform_point3(Vec3::ZERO);
        let eps = 1e-5;
        assert!((actual_pos - expected_pos).length() < eps);
    }

    #[test]
    fn test_three_level_hierarchy() {
        let mut world = hecs::World::new();

        // Grandparent at (1, 0, 0)
        let grandparent = world.spawn((
            Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
            GlobalTransform::default(),
        ));

        // Parent at local (0, 1, 0)
        let parent = world.spawn((
            Transform::from_position(Vec3::new(0.0, 1.0, 0.0)),
            GlobalTransform::default(),
            Parent(grandparent),
        ));

        // Child at local (0, 0, 1)
        let child = world.spawn((
            Transform::from_position(Vec3::new(0.0, 0.0, 1.0)),
            GlobalTransform::default(),
            Parent(parent),
        ));

        // Set up hierarchy
        world
            .insert_one(grandparent, Children(vec![parent]))
            .unwrap();
        world.insert_one(parent, Children(vec![child])).unwrap();

        transform_system(&mut world);

        // Child's global position should be (1, 1, 1)
        let child_global = world.get::<&GlobalTransform>(child).unwrap();
        let actual_pos = child_global.0.transform_point3(Vec3::ZERO);
        let expected = Vec3::new(1.0, 1.0, 1.0);
        let eps = 1e-5;
        assert!(
            (actual_pos - expected).length() < eps,
            "Expected {expected:?}, got {actual_pos:?}"
        );
    }
}
