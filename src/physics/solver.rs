//! Sequential impulse constraint solver.

use glam::Vec3;

use crate::ecs::components::physics::{RigidBody, RigidBodyType};
use crate::ecs::components::transform::GlobalTransform;

use super::contact::ContactManifold;

/// Baumgarte stabilization parameter.
const BAUMGARTE_BETA: f32 = 0.2;
/// Penetration slop (allowed penetration before position correction).
const PENETRATION_SLOP: f32 = 0.005;

/// Solve contact constraints using sequential impulse iteration.
pub fn solve_contacts(
    manifolds: &mut [ContactManifold],
    world: &mut hecs::World,
    solver_iterations: u32,
) {
    for _ in 0..solver_iterations {
        for manifold in manifolds.iter_mut() {
            solve_manifold(manifold, world);
        }
    }
}

fn solve_manifold(manifold: &mut ContactManifold, world: &hecs::World) {
    // Read rigid body data and positions for both entities
    let (rb_a_data, rb_b_data) = {
        let rb_a = world.get::<&RigidBody>(manifold.entity_a).ok();
        let rb_b = world.get::<&RigidBody>(manifold.entity_b).ok();
        let pos_a = world
            .get::<&GlobalTransform>(manifold.entity_a)
            .ok()
            .map_or(Vec3::ZERO, |t| t.0.transform_point3(Vec3::ZERO));
        let pos_b = world
            .get::<&GlobalTransform>(manifold.entity_b)
            .ok()
            .map_or(Vec3::ZERO, |t| t.0.transform_point3(Vec3::ZERO));

        let a = rb_a.map(|rb| RbData::from_rb(&rb, pos_a));
        let b = rb_b.map(|rb| RbData::from_rb(&rb, pos_b));
        (a, b)
    };

    let Some(rb_a_data) = rb_a_data else {
        return;
    };
    let Some(rb_b_data) = rb_b_data else {
        return;
    };

    // Skip if both are static/kinematic
    if rb_a_data.inv_mass == 0.0 && rb_b_data.inv_mass == 0.0 {
        return;
    }

    let normal = manifold.normal;
    let restitution = (rb_a_data.restitution + rb_b_data.restitution) * 0.5;
    let friction = (rb_a_data.friction + rb_b_data.friction) * 0.5;

    for contact in &mut manifold.contacts {
        // Compute relative velocity at contact point
        let r_a = contact.position - rb_a_data.position;
        let r_b = contact.position - rb_b_data.position;

        let vel_a = rb_a_data.linear_velocity + rb_a_data.angular_velocity.cross(r_a);
        let vel_b = rb_b_data.linear_velocity + rb_b_data.angular_velocity.cross(r_b);

        let relative_velocity = vel_b - vel_a;
        let contact_velocity = relative_velocity.dot(normal);

        // Normal impulse
        let r_a_cross_n = r_a.cross(normal);
        let r_b_cross_n = r_b.cross(normal);

        let inv_mass_sum = rb_a_data.inv_mass
            + rb_b_data.inv_mass
            + (rb_a_data.inv_inertia * r_a_cross_n).dot(r_a_cross_n)
            + (rb_b_data.inv_inertia * r_b_cross_n).dot(r_b_cross_n);

        if inv_mass_sum <= 0.0 {
            continue;
        }

        // Baumgarte position correction
        let bias =
            BAUMGARTE_BETA / (1.0 / 60.0) * (contact.penetration - PENETRATION_SLOP).max(0.0);

        let j_normal = (-(1.0 + restitution) * contact_velocity + bias) / inv_mass_sum;

        // Clamp accumulated normal impulse
        let old_impulse = contact.normal_impulse;
        contact.normal_impulse = (old_impulse + j_normal).max(0.0);
        let j_normal = contact.normal_impulse - old_impulse;

        let impulse = normal * j_normal;

        // Apply normal impulse (need to write back later)
        apply_impulse(
            world,
            manifold.entity_a,
            manifold.entity_b,
            impulse,
            r_a,
            r_b,
        );

        // Friction impulse
        // Re-read velocities after normal impulse
        let (rb_a_updated, rb_b_updated) = {
            let pos_a = world
                .get::<&GlobalTransform>(manifold.entity_a)
                .ok()
                .map_or(Vec3::ZERO, |t| t.0.transform_point3(Vec3::ZERO));
            let pos_b = world
                .get::<&GlobalTransform>(manifold.entity_b)
                .ok()
                .map_or(Vec3::ZERO, |t| t.0.transform_point3(Vec3::ZERO));
            let a = world
                .get::<&RigidBody>(manifold.entity_a)
                .ok()
                .map(|rb| RbData::from_rb(&rb, pos_a));
            let b = world
                .get::<&RigidBody>(manifold.entity_b)
                .ok()
                .map(|rb| RbData::from_rb(&rb, pos_b));
            (a, b)
        };

        if let (Some(a), Some(b)) = (rb_a_updated, rb_b_updated) {
            let vel_a2 = a.linear_velocity + a.angular_velocity.cross(r_a);
            let vel_b2 = b.linear_velocity + b.angular_velocity.cross(r_b);
            let rel_vel2 = vel_b2 - vel_a2;

            // Tangent velocity (remove normal component)
            let tangent_vel = rel_vel2 - normal * rel_vel2.dot(normal);
            let tangent_len = tangent_vel.length();

            if tangent_len > 1e-6 {
                let tangent = tangent_vel / tangent_len;

                let r_a_cross_t = r_a.cross(tangent);
                let r_b_cross_t = r_b.cross(tangent);

                let inv_mass_t = a.inv_mass
                    + b.inv_mass
                    + (a.inv_inertia * r_a_cross_t).dot(r_a_cross_t)
                    + (b.inv_inertia * r_b_cross_t).dot(r_b_cross_t);

                if inv_mass_t > 0.0 {
                    let j_tangent = -tangent_len / inv_mass_t;

                    // Coulomb friction: |Jt| <= mu * |Jn|
                    let max_friction = friction * contact.normal_impulse;
                    let j_tangent = j_tangent.clamp(-max_friction, max_friction);

                    let friction_impulse = tangent * j_tangent;
                    apply_impulse(
                        world,
                        manifold.entity_a,
                        manifold.entity_b,
                        friction_impulse,
                        r_a,
                        r_b,
                    );
                }
            }
        }
    }
}

/// Helper struct to cache rigid body data for solver calculations.
struct RbData {
    inv_mass: f32,
    inv_inertia: Vec3,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
    position: Vec3,
    restitution: f32,
    friction: f32,
}

impl RbData {
    fn from_rb(rb: &RigidBody, position: Vec3) -> Self {
        let inv_mass = if rb.body_type == RigidBodyType::Dynamic && rb.mass > 0.0 {
            1.0 / rb.mass
        } else {
            0.0
        };

        let inv_inertia = if rb.body_type == RigidBodyType::Dynamic {
            Vec3::new(
                if rb.inertia_tensor[0] > 0.0 {
                    1.0 / rb.inertia_tensor[0]
                } else {
                    0.0
                },
                if rb.inertia_tensor[4] > 0.0 {
                    1.0 / rb.inertia_tensor[4]
                } else {
                    0.0
                },
                if rb.inertia_tensor[8] > 0.0 {
                    1.0 / rb.inertia_tensor[8]
                } else {
                    0.0
                },
            )
        } else {
            Vec3::ZERO
        };

        Self {
            inv_mass,
            inv_inertia,
            linear_velocity: rb.linear_velocity,
            angular_velocity: rb.angular_velocity,
            position,
            restitution: rb.restitution,
            friction: rb.friction,
        }
    }
}

/// Apply an impulse to both bodies at the contact point.
fn apply_impulse(
    world: &hecs::World,
    entity_a: hecs::Entity,
    entity_b: hecs::Entity,
    impulse: Vec3,
    r_a: Vec3,
    r_b: Vec3,
) {
    // Apply to entity A (negative direction)
    if let Ok(mut rb) = world.get::<&mut RigidBody>(entity_a) {
        if rb.body_type == RigidBodyType::Dynamic && rb.mass > 0.0 {
            let inv_mass = 1.0 / rb.mass;
            rb.linear_velocity -= impulse * inv_mass;

            let inv_inertia = Vec3::new(
                if rb.inertia_tensor[0] > 0.0 {
                    1.0 / rb.inertia_tensor[0]
                } else {
                    0.0
                },
                if rb.inertia_tensor[4] > 0.0 {
                    1.0 / rb.inertia_tensor[4]
                } else {
                    0.0
                },
                if rb.inertia_tensor[8] > 0.0 {
                    1.0 / rb.inertia_tensor[8]
                } else {
                    0.0
                },
            );
            rb.angular_velocity -= inv_inertia * r_a.cross(impulse);
        }
    }

    // Apply to entity B (positive direction)
    if let Ok(mut rb) = world.get::<&mut RigidBody>(entity_b) {
        if rb.body_type == RigidBodyType::Dynamic && rb.mass > 0.0 {
            let inv_mass = 1.0 / rb.mass;
            rb.linear_velocity += impulse * inv_mass;

            let inv_inertia = Vec3::new(
                if rb.inertia_tensor[0] > 0.0 {
                    1.0 / rb.inertia_tensor[0]
                } else {
                    0.0
                },
                if rb.inertia_tensor[4] > 0.0 {
                    1.0 / rb.inertia_tensor[4]
                } else {
                    0.0
                },
                if rb.inertia_tensor[8] > 0.0 {
                    1.0 / rb.inertia_tensor[8]
                } else {
                    0.0
                },
            );
            rb.angular_velocity += inv_inertia * r_b.cross(impulse);
        }
    }
}
