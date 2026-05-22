//! CPU-based physics engine with rigid body simulation and collision detection.
//!
//! # Architecture
//!
//! The physics pipeline runs in a fixed timestep loop:
//!
//! 1. Apply forces (gravity)
//! 2. Integrate velocities
//! 3. Broadphase collision detection (AABB overlap)
//! 4. Narrowphase collision detection (GJK/EPA, SAT, specialized tests)
//! 5. Solve contact constraints (sequential impulse)
//! 6. Integrate positions
//! 7. Synchronize transforms
//! 8. Clear force accumulators

pub mod broadphase;
pub mod collider;
pub mod contact;
#[cfg(feature = "gpu-physics")]
pub mod gpu;
pub mod narrowphase;
pub mod rigid_body;
pub mod solver;

use glam::Vec3;

use crate::ecs::components::physics::Collider;
use crate::ecs::components::transform::GlobalTransform;

use self::broadphase::SpatialHashGrid;
use self::contact::{ContactCache, ContactManifold, ContactPoint};
use self::narrowphase::detect_collision;

/// Configuration for the physics simulation.
#[derive(Debug, Clone)]
pub struct PhysicsConfig {
    /// Gravity vector. Default: (0, -9.81, 0).
    pub gravity: Vec3,
    /// Fixed timestep for physics updates in seconds. Default: 1/60.
    pub fixed_timestep: f64,
    /// Maximum number of sub-steps per frame. Default: 4.
    pub max_substeps: u32,
    /// Number of constraint solver iterations. Default: 8.
    pub solver_iterations: u32,
    /// Whether to use GPU acceleration when available. Default: false.
    /// Requires the `gpu-physics` feature.
    pub use_gpu: bool,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            fixed_timestep: 1.0 / 60.0,
            max_substeps: 4,
            solver_iterations: 8,
            use_gpu: false,
        }
    }
}

/// The main physics world managing simulation state.
pub struct PhysicsWorld {
    config: PhysicsConfig,
    accumulator: f64,
    broadphase: SpatialHashGrid,
    contacts: Vec<ContactManifold>,
    contact_cache: ContactCache,
    #[cfg(feature = "gpu-physics")]
    gpu_physics: Option<gpu::GpuPhysics>,
}

impl PhysicsWorld {
    /// Create a new physics world with the given configuration.
    pub fn new(config: PhysicsConfig) -> Self {
        Self {
            config,
            accumulator: 0.0,
            broadphase: SpatialHashGrid::new(),
            contacts: Vec::new(),
            contact_cache: ContactCache::new(),
            #[cfg(feature = "gpu-physics")]
            gpu_physics: None,
        }
    }

    /// Initialize GPU physics resources. Only available with the `gpu-physics` feature.
    ///
    /// Call this once after creating the physics world to enable GPU acceleration.
    #[cfg(feature = "gpu-physics")]
    pub fn init_gpu(
        &mut self,
        ctx: &crate::context::WgpuContext,
        initial_capacity: usize,
    ) -> anyhow::Result<()> {
        self.gpu_physics = Some(gpu::GpuPhysics::new(ctx, initial_capacity)?);
        Ok(())
    }

    /// Get a reference to the GPU physics instance, if initialized.
    #[cfg(feature = "gpu-physics")]
    pub fn gpu_physics_ref(&self) -> Option<&gpu::GpuPhysics> {
        self.gpu_physics.as_ref()
    }

    /// Step the physics simulation forward by `delta_time` seconds.
    ///
    /// Uses a fixed timestep accumulator to ensure deterministic simulation.
    pub fn step(&mut self, world: &mut hecs::World, delta_time: f64) {
        self.accumulator += delta_time;

        let mut substeps = 0_u32;
        while self.accumulator >= self.config.fixed_timestep && substeps < self.config.max_substeps
        {
            self.fixed_step(world, self.config.fixed_timestep as f32);
            self.accumulator -= self.config.fixed_timestep;
            substeps += 1;
        }

        // Clamp accumulator to avoid spiral of death
        if self.accumulator > self.config.fixed_timestep * f64::from(self.config.max_substeps) {
            self.accumulator = 0.0;
        }
    }

    /// Step the physics simulation with GPU-accelerated broadphase.
    ///
    /// Uses GPU compute for AABB broadphase when body count exceeds the threshold,
    /// falling back to CPU otherwise. Requires `gpu-physics` feature and prior
    /// call to [`init_gpu`].
    #[cfg(feature = "gpu-physics")]
    pub fn step_gpu(
        &mut self,
        world: &mut hecs::World,
        delta_time: f64,
        ctx: &crate::context::WgpuContext,
    ) {
        self.accumulator += delta_time;

        let mut substeps = 0_u32;
        while self.accumulator >= self.config.fixed_timestep && substeps < self.config.max_substeps
        {
            self.fixed_step_gpu(world, self.config.fixed_timestep as f32, ctx);
            self.accumulator -= self.config.fixed_timestep;
            substeps += 1;
        }

        if self.accumulator > self.config.fixed_timestep * f64::from(self.config.max_substeps) {
            self.accumulator = 0.0;
        }
    }

    #[cfg(feature = "gpu-physics")]
    fn fixed_step_gpu(
        &mut self,
        world: &mut hecs::World,
        dt: f32,
        ctx: &crate::context::WgpuContext,
    ) {
        rigid_body::apply_gravity(world, self.config.gravity);
        rigid_body::integrate_velocities(world, dt);

        // Sync transforms so GPU broadphase sees current positions
        rigid_body::sync_transforms(world);

        self.contacts.clear();

        if let Some(gpu) = &self.gpu_physics {
            let (body_count, entity_map, max_extent) = gpu.upload_aabbs(ctx, world);
            if gpu::GpuPhysics::should_use_gpu(body_count as usize) {
                // GPU broadphase
                let cell_size = (max_extent * 2.0).max(1.0);
                gpu.dispatch_broadphase_with_cell_size(ctx, body_count, cell_size);

                // Upload shape data for GPU narrowphase
                gpu.upload_shapes(ctx, world, &entity_map);

                // Check if all shapes are spheres (fast path: no box-box pairs possible).
                // GPU narrowphase handles sphere-sphere, sphere-box, box-sphere but NOT box-box.
                // Fast path avoids broadphase readback by using pair_buffer directly.
                let all_spheres = entity_map.iter().all(|e| {
                    world
                        .get::<&Collider>(*e)
                        .ok()
                        .is_some_and(|c| {
                            matches!(
                                c.shape,
                                crate::ecs::components::physics::ColliderShape::Sphere { .. }
                            )
                        })
                });

                if all_spheres {
                    // Fast path: all sphere-sphere, skip broadphase readback
                    let pair_count_data: Vec<u32> =
                        crate::compute::read_buffer_sync(ctx, gpu.pair_count_buffer(), 4);
                    let pair_count = pair_count_data
                        .first()
                        .copied()
                        .unwrap_or(0)
                        .min(gpu::MAX_PAIRS);

                    gpu.dispatch_narrowphase_direct(ctx, pair_count);
                    let gpu_results = gpu.readback_narrowphase(ctx, pair_count);
                    Self::collect_gpu_narrowphase_results(
                        world,
                        &gpu_results,
                        &entity_map,
                        &mut self.contacts,
                    );
                } else {
                    // Mixed path: readback pairs, classify per-pair, split GPU/CPU
                    let broadphase_pairs = gpu.readback_pairs(ctx);
                    let (gpu_np_count, cpu_pairs) =
                        gpu.dispatch_narrowphase(ctx, &broadphase_pairs, &entity_map, world);

                    if gpu_np_count > 0 {
                        let gpu_results = gpu.readback_narrowphase(ctx, gpu_np_count);
                        Self::collect_gpu_narrowphase_results(
                            world,
                            &gpu_results,
                            &entity_map,
                            &mut self.contacts,
                        );
                    }

                    Self::run_cpu_narrowphase(world, &cpu_pairs, &mut self.contacts);
                }
            } else {
                // Fallback to CPU
                let pairs = self.broadphase.find_pairs(world);
                Self::run_cpu_narrowphase(world, &pairs, &mut self.contacts);
            }
        } else {
            let pairs = self.broadphase.find_pairs(world);
            Self::run_cpu_narrowphase(world, &pairs, &mut self.contacts);
        }

        self.contact_cache.warm_start(&mut self.contacts);
        solver::solve_contacts(&mut self.contacts, world, self.config.solver_iterations);
        self.contact_cache.update(&self.contacts);
        rigid_body::integrate_positions(world, dt);
        rigid_body::sync_transforms(world);
        rigid_body::clear_forces(world);
        rigid_body::update_sleep_states(world, dt);
    }

    fn fixed_step(&mut self, world: &mut hecs::World, dt: f32) {
        // 1. Apply forces (gravity)
        rigid_body::apply_gravity(world, self.config.gravity);

        // 2. Integrate velocities
        rigid_body::integrate_velocities(world, dt);

        // 3. Broadphase collision detection
        let pairs = self.broadphase.find_pairs(world);

        // 4. Narrowphase collision detection
        self.contacts.clear();
        Self::run_cpu_narrowphase(world, &pairs, &mut self.contacts);

        // 5. Warm-start from cached impulses
        self.contact_cache.warm_start(&mut self.contacts);

        // 6. Solve contact constraints
        solver::solve_contacts(&mut self.contacts, world, self.config.solver_iterations);

        // 7. Update contact cache for next frame
        self.contact_cache.update(&self.contacts);

        // 8. Integrate positions
        rigid_body::integrate_positions(world, dt);

        // 9. Synchronize transforms
        rigid_body::sync_transforms(world);

        // 10. Clear force accumulators
        rigid_body::clear_forces(world);

        // 11. Update sleep states
        rigid_body::update_sleep_states(world, dt);
    }

    /// Collect GPU narrowphase results into contact manifolds.
    #[cfg(feature = "gpu-physics")]
    fn collect_gpu_narrowphase_results(
        world: &mut hecs::World,
        gpu_results: &[gpu::NarrowphaseResult],
        entity_map: &[hecs::Entity],
        contacts: &mut Vec<ContactManifold>,
    ) {
        for result in gpu_results {
            let entity_a = entity_map
                .get(result.entity_a as usize)
                .copied()
                .unwrap_or(hecs::Entity::DANGLING);
            let entity_b = entity_map
                .get(result.entity_b as usize)
                .copied()
                .unwrap_or(hecs::Entity::DANGLING);

            rigid_body::wake_body(world, entity_a);
            rigid_body::wake_body(world, entity_b);

            contacts.push(ContactManifold {
                entity_a,
                entity_b,
                normal: Vec3::from(result.normal),
                contacts: vec![ContactPoint {
                    position: Vec3::from(result.point),
                    penetration: result.penetration,
                    normal_impulse: 0.0,
                    tangent_impulse: [0.0; 2],
                }],
            });
        }
    }

    /// Run CPU narrowphase on a set of entity pairs, appending results to contacts.
    fn run_cpu_narrowphase(
        world: &mut hecs::World,
        pairs: &[(hecs::Entity, hecs::Entity)],
        contacts: &mut Vec<ContactManifold>,
    ) {
        for (entity_a, entity_b) in pairs {
            let contact = {
                let collider_a = world.get::<&Collider>(*entity_a);
                let collider_b = world.get::<&Collider>(*entity_b);
                let transform_a = world.get::<&GlobalTransform>(*entity_a);
                let transform_b = world.get::<&GlobalTransform>(*entity_b);

                if let (Ok(ca), Ok(cb), Ok(ta), Ok(tb)) =
                    (collider_a, collider_b, transform_a, transform_b)
                {
                    let adjusted_a = if ca.offset == Vec3::ZERO {
                        *ta
                    } else {
                        GlobalTransform(ta.0 * glam::Mat4::from_translation(ca.offset))
                    };
                    let adjusted_b = if cb.offset == Vec3::ZERO {
                        *tb
                    } else {
                        GlobalTransform(tb.0 * glam::Mat4::from_translation(cb.offset))
                    };

                    detect_collision(&ca.shape, &adjusted_a, &cb.shape, &adjusted_b)
                } else {
                    None
                }
            };

            if let Some(info) = contact {
                rigid_body::wake_body(world, *entity_a);
                rigid_body::wake_body(world, *entity_b);

                contacts.push(ContactManifold {
                    entity_a: *entity_a,
                    entity_b: *entity_b,
                    normal: info.normal,
                    contacts: vec![ContactPoint {
                        position: info.point,
                        penetration: info.penetration,
                        normal_impulse: 0.0,
                        tangent_impulse: [0.0; 2],
                    }],
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::physics::{Collider, ColliderShape, RigidBody};
    use crate::ecs::components::transform::{GlobalTransform, Transform};
    use glam::Mat4;

    #[test]
    fn test_physics_world_free_fall() {
        let mut world = hecs::World::new();
        let mut physics = PhysicsWorld::new(PhysicsConfig::default());

        let entity = world.spawn((
            Transform::from_position(Vec3::new(0.0, 10.0, 0.0)),
            GlobalTransform(Mat4::from_translation(Vec3::new(0.0, 10.0, 0.0))),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Sphere { radius: 0.5 },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        // Simulate ~1 second
        for _ in 0..60 {
            physics.step(&mut world, 1.0 / 60.0);
        }

        let transform = world.get::<&Transform>(entity).unwrap();
        assert!(
            transform.position.y < 10.0,
            "Body should have fallen: y = {}",
            transform.position.y
        );
    }

    #[test]
    fn test_physics_world_collision() {
        let mut world = hecs::World::new();
        let config = PhysicsConfig {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            fixed_timestep: 1.0 / 60.0,
            max_substeps: 4,
            solver_iterations: 8,
            use_gpu: false,
        };
        let mut physics = PhysicsWorld::new(config);

        // Dynamic box falling
        let dynamic_entity = world.spawn((
            Transform::from_position(Vec3::new(0.0, 2.0, 0.0)),
            GlobalTransform(Mat4::from_translation(Vec3::new(0.0, 2.0, 0.0))),
            RigidBody::new_dynamic(1.0),
            Collider {
                shape: ColliderShape::Box {
                    half_extents: Vec3::splat(0.5),
                },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        // Static ground plane (large box at y=0)
        world.spawn((
            Transform::from_position(Vec3::new(0.0, -0.5, 0.0)),
            GlobalTransform(Mat4::from_translation(Vec3::new(0.0, -0.5, 0.0))),
            RigidBody::new_static(),
            Collider {
                shape: ColliderShape::Box {
                    half_extents: Vec3::new(50.0, 0.5, 50.0),
                },
                offset: Vec3::ZERO,
                is_sensor: false,
            },
        ));

        // Simulate 3 seconds
        for _ in 0..180 {
            physics.step(&mut world, 1.0 / 60.0);
        }

        let transform = world.get::<&Transform>(dynamic_entity).unwrap();
        let rb = world.get::<&RigidBody>(dynamic_entity).unwrap();

        // The box should have fallen and been stopped by the ground
        // It should be near y=0.5 (half the box height above the ground surface)
        // Allow generous tolerance for solver precision
        assert!(
            transform.position.y > -2.0,
            "Box should not have fallen through the ground: y = {}",
            transform.position.y
        );
        assert!(
            transform.position.y < 2.0,
            "Box should have fallen from initial position: y = {}",
            transform.position.y
        );

        // Velocity should be near zero (settled)
        let speed = rb.linear_velocity.length();
        assert!(
            speed < 5.0,
            "Box should have mostly settled: speed = {speed}"
        );
    }

    #[test]
    fn test_physics_config_default() {
        let config = PhysicsConfig::default();
        assert_eq!(config.gravity, Vec3::new(0.0, -9.81, 0.0));
        assert!((config.fixed_timestep - 1.0 / 60.0).abs() < 1e-10);
        assert_eq!(config.max_substeps, 4);
        assert_eq!(config.solver_iterations, 8);
        assert!(!config.use_gpu);
    }
}
