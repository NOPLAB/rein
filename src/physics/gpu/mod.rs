//! GPU-accelerated physics using compute shaders.
//!
//! Offloads broadphase collision detection and velocity/position integration
//! to the GPU for improved performance with large numbers of rigid bodies.
//!
//! # Strategy
//!
//! | Stage | CPU/GPU | Reason |
//! |-------|---------|--------|
//! | Velocity integration | GPU | Highly parallel (each body independent) |
//! | Position integration | GPU | Same |
//! | Broadphase AABB | GPU | Parallelizes O(n^2) pair testing |
//! | Narrowphase (GJK/EPA) | CPU | Branch-heavy, poor GPU fit |
//! | Contact solver | CPU | Sequential impulse is inherently serial |
//!
//! GPU offload is used when body count >= [`GPU_BODY_THRESHOLD`].

use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::compute::{compute_workgroup_count, read_buffer_sync, ComputeDispatcher};
use crate::context::WgpuContext;
use crate::core::{ComputePipelineBuilder, StorageBuffer};
use crate::ecs::components::physics::{Collider, ColliderShape, RigidBody, RigidBodyType};
use crate::ecs::components::transform::GlobalTransform;

/// Minimum number of bodies before GPU offload is used.
pub const GPU_BODY_THRESHOLD: usize = 256;

/// Maximum collision pairs the GPU broadphase can output.
pub const MAX_PAIRS: u32 = 65536;

/// Workgroup size matching the WGSL shaders.
const WORKGROUP_SIZE: u32 = 64;

/// GPU AABB data layout matching the broadphase shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuAabb {
    pub min: [f32; 3],
    pub entity_id: u32,
    pub max: [f32; 3],
    pub padding: u32,
}

/// GPU collision pair output.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CollisionPair {
    pub entity_a: u32,
    pub entity_b: u32,
}

/// GPU body data layout matching the integrate shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuBody {
    pub position: [f32; 3],
    pub body_type: u32,
    pub linear_velocity: [f32; 3],
    pub mass: f32,
    pub angular_velocity: [f32; 3],
    pub gravity_scale: f32,
    pub force_accumulator: [f32; 3],
    pub linear_damping: f32,
    pub torque_accumulator: [f32; 3],
    pub angular_damping: f32,
    pub inertia_diag: [f32; 3],
    pub padding: f32,
    pub rotation: [f32; 4],
}

/// GPU broadphase parameters.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BroadphaseParams {
    num_bodies: u32,
    max_pairs: u32,
    /// Cell size as f32 bits (bitcast<f32> in shader).
    cell_size_bits: u32,
    _pad0: u32,
}

/// GPU shape data for narrowphase.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuShapeData {
    pub position: [f32; 3],
    pub shape_type: u32, // 0=sphere, 1=box
    pub data: [f32; 4],  // sphere: [radius,0,0,0], box: [hx,hy,hz,0]
    pub axis_x: [f32; 3],
    pub scale_x: f32,
    pub axis_y: [f32; 3],
    pub scale_y: f32,
    pub axis_z: [f32; 3],
    pub scale_z: f32,
}

/// GPU narrowphase result.
///
/// Layout must match WGSL struct with vec3<f32> alignment (16 bytes).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NarrowphaseResult {
    pub entity_a: u32,
    pub entity_b: u32,
    pub pad0: u32,
    pub pad1: u32,
    pub normal: [f32; 3],
    pub penetration: f32,
    pub point: [f32; 3],
    pub has_contact: u32,
}

/// GPU integrate parameters.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct IntegrateParams {
    num_bodies: u32,
    dt: f32,
    gravity_x: f32,
    gravity_y: f32,
    gravity_z: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

/// GPU-accelerated physics engine.
///
/// Manages GPU buffers and compute pipelines for broadphase collision
/// detection and rigid body integration.
pub struct GpuPhysics {
    // Broadphase resources (legacy O(n^2) fallback, kept for potential direct use)
    _broadphase_pipeline: wgpu::ComputePipeline,
    // Spatial hash broadphase resources (2-pass)
    assign_cells_pipeline: wgpu::ComputePipeline,
    broadphase_spatial_pipeline: wgpu::ComputePipeline,
    aabb_buffer: StorageBuffer,
    pair_buffer: StorageBuffer,
    pair_count_buffer: StorageBuffer,
    cell_buffer: StorageBuffer,
    broadphase_data_layout: wgpu::BindGroupLayout,
    broadphase_params_layout: wgpu::BindGroupLayout,

    // Narrowphase resources
    narrowphase_pipeline: wgpu::ComputePipeline,
    narrowphase_pair_buffer: StorageBuffer,
    shape_buffer: StorageBuffer,
    narrowphase_result_buffer: StorageBuffer,
    narrowphase_data_layout: wgpu::BindGroupLayout,
    narrowphase_params_layout: wgpu::BindGroupLayout,

    // Integration resources
    integrate_vel_pipeline: wgpu::ComputePipeline,
    integrate_pos_pipeline: wgpu::ComputePipeline,
    body_buffer: StorageBuffer,
    integrate_data_layout: wgpu::BindGroupLayout,
    integrate_params_layout: wgpu::BindGroupLayout,

    // Capacity tracking
    max_bodies: usize,
    max_narrowphase_pairs: usize,
}

impl GpuPhysics {
    /// Create a new GPU physics instance.
    ///
    /// Allocates GPU buffers and compiles compute shaders.
    pub fn new(ctx: &WgpuContext, initial_capacity: usize) -> anyhow::Result<Self> {
        let max_bodies = initial_capacity.max(256);

        // --- Broadphase pipeline ---
        let broadphase_data_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("broadphase data layout"),
                    entries: &[
                        // AABBs (read)
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Pairs (read_write)
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Pair count (read_write)
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Cell assignments (read_write)
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let broadphase_params_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("broadphase params layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let broadphase_shader = include_str!("../../shaders/compute/broadphase.wgsl");
        let broadphase_pipeline = ComputePipelineBuilder::new(ctx)
            .label("broadphase compute (legacy)")
            .shader(broadphase_shader)
            .entry_point("cs_broadphase")
            .bind_group_layout(&broadphase_data_layout)
            .bind_group_layout(&broadphase_params_layout)
            .build()?;

        let assign_cells_pipeline = ComputePipelineBuilder::new(ctx)
            .label("assign cells compute")
            .shader(broadphase_shader)
            .entry_point("cs_assign_cells")
            .bind_group_layout(&broadphase_data_layout)
            .bind_group_layout(&broadphase_params_layout)
            .build()?;

        let broadphase_spatial_pipeline = ComputePipelineBuilder::new(ctx)
            .label("broadphase spatial compute")
            .shader(broadphase_shader)
            .entry_point("cs_broadphase_spatial")
            .bind_group_layout(&broadphase_data_layout)
            .bind_group_layout(&broadphase_params_layout)
            .build()?;

        // Broadphase buffers
        let aabb_size = (max_bodies * size_of::<GpuAabb>()) as u64;
        let aabb_buffer = StorageBuffer::new(ctx, aabb_size, Some("aabb buffer"));

        let pair_size = (MAX_PAIRS as usize * size_of::<CollisionPair>()) as u64;
        let pair_buffer = StorageBuffer::new(ctx, pair_size, Some("pair buffer"));

        // pair_count: single u32, atomic
        let pair_count_buffer = StorageBuffer::new(ctx, 4, Some("pair count buffer"));

        // CellAssignment: 8 * u32 (cell_hash) + u32 (num_cells) + 3 * u32 (padding) = 48 bytes
        let cell_size = (max_bodies * 48) as u64;
        let cell_buffer = StorageBuffer::new(ctx, cell_size, Some("cell assignment buffer"));

        // --- Integration pipelines ---
        let integrate_data_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("integrate data layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let integrate_params_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("integrate params layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let integrate_shader = include_str!("../../shaders/compute/integrate.wgsl");
        let integrate_vel_pipeline = ComputePipelineBuilder::new(ctx)
            .label("integrate velocities compute")
            .shader(integrate_shader)
            .entry_point("cs_integrate_velocities")
            .bind_group_layout(&integrate_data_layout)
            .bind_group_layout(&integrate_params_layout)
            .build()?;

        let integrate_pos_pipeline = ComputePipelineBuilder::new(ctx)
            .label("integrate positions compute")
            .shader(integrate_shader)
            .entry_point("cs_integrate_positions")
            .bind_group_layout(&integrate_data_layout)
            .bind_group_layout(&integrate_params_layout)
            .build()?;

        // Body buffer
        let body_size = (max_bodies * size_of::<GpuBody>()) as u64;
        let body_buffer = StorageBuffer::new(ctx, body_size, Some("body buffer"));

        // --- Narrowphase pipeline ---
        let narrowphase_data_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("narrowphase data layout"),
                    entries: &[
                        // Pairs (read)
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Shapes (read)
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Results (read_write)
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        let narrowphase_params_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("narrowphase params layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let narrowphase_shader = include_str!("../../shaders/compute/narrowphase.wgsl");
        let narrowphase_pipeline = ComputePipelineBuilder::new(ctx)
            .label("narrowphase compute")
            .shader(narrowphase_shader)
            .entry_point("cs_narrowphase")
            .bind_group_layout(&narrowphase_data_layout)
            .bind_group_layout(&narrowphase_params_layout)
            .build()?;

        let max_narrowphase_pairs = MAX_PAIRS as usize;
        let narrowphase_pair_size = (max_narrowphase_pairs * size_of::<CollisionPair>()) as u64;
        let narrowphase_pair_buffer =
            StorageBuffer::new(ctx, narrowphase_pair_size, Some("narrowphase pair buffer"));

        let shape_size = (max_bodies * size_of::<GpuShapeData>()) as u64;
        let shape_buffer = StorageBuffer::new(ctx, shape_size, Some("shape buffer"));

        let result_size = (max_narrowphase_pairs * size_of::<NarrowphaseResult>()) as u64;
        let narrowphase_result_buffer =
            StorageBuffer::new(ctx, result_size, Some("narrowphase result buffer"));

        Ok(Self {
            _broadphase_pipeline: broadphase_pipeline,
            assign_cells_pipeline,
            broadphase_spatial_pipeline,
            aabb_buffer,
            pair_buffer,
            pair_count_buffer,
            cell_buffer,
            broadphase_data_layout,
            broadphase_params_layout,
            narrowphase_pipeline,
            narrowphase_pair_buffer,
            shape_buffer,
            narrowphase_result_buffer,
            narrowphase_data_layout,
            narrowphase_params_layout,
            integrate_vel_pipeline,
            integrate_pos_pipeline,
            body_buffer,
            integrate_data_layout,
            integrate_params_layout,
            max_bodies,
            max_narrowphase_pairs,
        })
    }

    /// Upload AABB data from the ECS world to the GPU for broadphase.
    ///
    /// Returns (body_count, entity_map, max_aabb_extent).
    pub fn upload_aabbs(
        &self,
        ctx: &WgpuContext,
        world: &hecs::World,
    ) -> (u32, Vec<hecs::Entity>, f32) {
        let mut aabbs = Vec::new();
        let mut entity_map = Vec::new();
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

            let idx = aabbs.len() as u32;
            aabbs.push(GpuAabb {
                min: aabb.min.into(),
                entity_id: idx,
                max: aabb.max.into(),
                padding: rb.body_type as u32,
            });
            entity_map.push(entity);
        }

        if !aabbs.is_empty() && aabbs.len() <= self.max_bodies {
            self.aabb_buffer.write(ctx, &aabbs);
        }

        // Reset pair count to 0
        self.pair_count_buffer.write(ctx, &[0_u32]);

        (aabbs.len() as u32, entity_map, max_extent)
    }

    /// Dispatch the GPU broadphase compute shader using spatial hash grid.
    ///
    /// Uses a 2-pass algorithm:
    /// 1. Assign cells: compute cell IDs for each AABB
    /// 2. Spatial broadphase: test pairs within the same cell
    pub fn dispatch_broadphase(&self, ctx: &WgpuContext, body_count: u32) {
        self.dispatch_broadphase_with_cell_size(ctx, body_count, 4.0);
    }

    /// Dispatch broadphase with a specific cell size.
    pub fn dispatch_broadphase_with_cell_size(
        &self,
        ctx: &WgpuContext,
        body_count: u32,
        cell_size: f32,
    ) {
        if body_count == 0 {
            return;
        }

        let params = BroadphaseParams {
            num_bodies: body_count,
            max_pairs: MAX_PAIRS,
            cell_size_bits: cell_size.to_bits(),
            _pad0: 0,
        };

        let params_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("broadphase params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let data_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("broadphase data"),
            layout: &self.broadphase_data_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.aabb_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.pair_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.pair_count_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.cell_buffer.buffer().as_entire_binding(),
                },
            ],
        });

        let params_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("broadphase params"),
            layout: &self.broadphase_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        });

        let dispatcher = ComputeDispatcher::new(ctx);
        let workgroups = compute_workgroup_count(body_count, WORKGROUP_SIZE);

        // Pass 1: Assign cells
        dispatcher.dispatch(
            &self.assign_cells_pipeline,
            &[&data_bind_group, &params_bind_group],
            [workgroups, 1, 1],
            Some("assign cells"),
        );

        // Pass 2: Spatial broadphase
        dispatcher.dispatch(
            &self.broadphase_spatial_pipeline,
            &[&data_bind_group, &params_bind_group],
            [workgroups, 1, 1],
            Some("broadphase spatial"),
        );
    }

    /// Read back broadphase collision pairs from the GPU.
    ///
    /// Returns pairs as (entity_index_a, entity_index_b). Use the entity map
    /// from [`upload_aabbs`] to convert indices to `hecs::Entity`.
    pub fn readback_pairs(&self, ctx: &WgpuContext) -> Vec<CollisionPair> {
        // Read the pair count first
        let count_data: Vec<u32> = read_buffer_sync(ctx, self.pair_count_buffer.buffer(), 4);
        let pair_count = count_data.first().copied().unwrap_or(0).min(MAX_PAIRS);

        if pair_count == 0 {
            return Vec::new();
        }

        let read_size = (pair_count as usize * size_of::<CollisionPair>()) as u64;
        let mut pairs: Vec<CollisionPair> =
            read_buffer_sync(ctx, self.pair_buffer.buffer(), read_size);
        pairs.truncate(pair_count as usize);
        pairs
    }

    /// Upload shape data for GPU narrowphase.
    pub fn upload_shapes(
        &self,
        ctx: &WgpuContext,
        world: &hecs::World,
        entity_map: &[hecs::Entity],
    ) {
        let mut shapes = Vec::with_capacity(entity_map.len());

        for entity in entity_map {
            let collider = world.get::<&Collider>(*entity).ok();
            let transform = world.get::<&GlobalTransform>(*entity).ok();

            let shape_data = if let (Some(collider), Some(transform)) = (collider, transform) {
                let mut adjusted = *transform;
                if collider.offset != Vec3::ZERO {
                    adjusted.0 *= glam::Mat4::from_translation(collider.offset);
                }

                let position = adjusted.0.transform_point3(Vec3::ZERO);
                let axis_x = adjusted.0.x_axis.truncate().normalize_or_zero();
                let axis_y = adjusted.0.y_axis.truncate().normalize_or_zero();
                let axis_z = adjusted.0.z_axis.truncate().normalize_or_zero();
                let scale_x = adjusted.0.x_axis.truncate().length();
                let scale_y = adjusted.0.y_axis.truncate().length();
                let scale_z = adjusted.0.z_axis.truncate().length();

                match &collider.shape {
                    ColliderShape::Sphere { radius } => GpuShapeData {
                        position: position.into(),
                        shape_type: 0,
                        data: [*radius, 0.0, 0.0, 0.0],
                        axis_x: axis_x.into(),
                        scale_x,
                        axis_y: axis_y.into(),
                        scale_y,
                        axis_z: axis_z.into(),
                        scale_z,
                    },
                    ColliderShape::Box { half_extents } => GpuShapeData {
                        position: position.into(),
                        shape_type: 1,
                        data: [half_extents.x, half_extents.y, half_extents.z, 0.0],
                        axis_x: axis_x.into(),
                        scale_x,
                        axis_y: axis_y.into(),
                        scale_y,
                        axis_z: axis_z.into(),
                        scale_z,
                    },
                    _ => GpuShapeData {
                        position: position.into(),
                        shape_type: 255,
                        data: [0.0; 4],
                        axis_x: axis_x.into(),
                        scale_x,
                        axis_y: axis_y.into(),
                        scale_y,
                        axis_z: axis_z.into(),
                        scale_z,
                    },
                }
            } else {
                GpuShapeData {
                    position: [0.0; 3],
                    shape_type: 255,
                    data: [0.0; 4],
                    axis_x: [1.0, 0.0, 0.0],
                    scale_x: 1.0,
                    axis_y: [0.0, 1.0, 0.0],
                    scale_y: 1.0,
                    axis_z: [0.0, 0.0, 1.0],
                    scale_z: 1.0,
                }
            };
            shapes.push(shape_data);
        }

        if !shapes.is_empty() && shapes.len() <= self.max_bodies {
            self.shape_buffer.write(ctx, &shapes);
        }
    }

    /// Dispatch GPU narrowphase for GPU-compatible pairs.
    ///
    /// Returns the number of pairs dispatched. Pairs involving shapes that
    /// require GJK/EPA are returned in `cpu_pairs` for CPU processing.
    pub fn dispatch_narrowphase(
        &self,
        ctx: &WgpuContext,
        pairs: &[CollisionPair],
        entity_map: &[hecs::Entity],
        world: &hecs::World,
    ) -> (u32, Vec<(hecs::Entity, hecs::Entity)>) {
        let mut gpu_pairs = Vec::new();
        let mut cpu_pairs = Vec::new();

        for pair in pairs {
            let a_idx = pair.entity_a as usize;
            let b_idx = pair.entity_b as usize;

            let (Some(&entity_a), Some(&entity_b)) = (entity_map.get(a_idx), entity_map.get(b_idx))
            else {
                continue;
            };

            // Check if the pair is GPU-compatible.
            // GPU narrowphase handles: sphere-sphere, sphere-box, box-sphere.
            // Box-box (SAT) is NOT implemented on GPU and must go to CPU.
            let a_shape = world
                .get::<&Collider>(entity_a)
                .ok()
                .map(|c| match c.shape {
                    ColliderShape::Sphere { .. } => 0_u8,
                    ColliderShape::Box { .. } => 1_u8,
                    _ => 255_u8,
                });
            let b_shape = world
                .get::<&Collider>(entity_b)
                .ok()
                .map(|c| match c.shape {
                    ColliderShape::Sphere { .. } => 0_u8,
                    ColliderShape::Box { .. } => 1_u8,
                    _ => 255_u8,
                });

            // Supported on GPU: sphere-sphere, sphere-box, box-sphere.
            // box-box and other combos fall back to CPU narrowphase.
            let pair_gpu_compatible = matches!(
                (a_shape, b_shape),
                (Some(0), Some(0 | 1)) | (Some(1), Some(0))
            );

            if pair_gpu_compatible {
                gpu_pairs.push(CollisionPair {
                    entity_a: pair.entity_a,
                    entity_b: pair.entity_b,
                });
            } else {
                cpu_pairs.push((entity_a, entity_b));
            }
        }

        let gpu_pair_count = gpu_pairs.len().min(self.max_narrowphase_pairs) as u32;

        if gpu_pair_count > 0 {
            self.narrowphase_pair_buffer.write(ctx, &gpu_pairs);

            let params_data = [gpu_pair_count, 0_u32, 0_u32, 0_u32];
            let params_buffer = ctx
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("narrowphase params"),
                    contents: bytemuck::cast_slice(&params_data),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let data_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("narrowphase data"),
                layout: &self.narrowphase_data_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.narrowphase_pair_buffer.buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.shape_buffer.buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.narrowphase_result_buffer.buffer().as_entire_binding(),
                    },
                ],
            });

            let params_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("narrowphase params"),
                layout: &self.narrowphase_params_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                }],
            });

            let dispatcher = ComputeDispatcher::new(ctx);
            let workgroups = compute_workgroup_count(gpu_pair_count, WORKGROUP_SIZE);
            dispatcher.dispatch(
                &self.narrowphase_pipeline,
                &[&data_bind_group, &params_bind_group],
                [workgroups, 1, 1],
                Some("narrowphase"),
            );
        }

        (gpu_pair_count, cpu_pairs)
    }

    /// Dispatch GPU narrowphase using broadphase pair_buffer directly (no readback).
    ///
    /// This avoids the broadphase->CPU->narrowphase roundtrip when all shapes
    /// are GPU-compatible. Returns the pair count used.
    pub fn dispatch_narrowphase_direct(&self, ctx: &WgpuContext, pair_count: u32) {
        if pair_count == 0 {
            return;
        }

        let params_data = [pair_count, 0_u32, 0_u32, 0_u32];
        let params_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("narrowphase params direct"),
                contents: bytemuck::cast_slice(&params_data),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // Use the broadphase pair_buffer directly as narrowphase input
        let data_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("narrowphase data direct"),
            layout: &self.narrowphase_data_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.pair_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.shape_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.narrowphase_result_buffer.buffer().as_entire_binding(),
                },
            ],
        });

        let params_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("narrowphase params direct"),
            layout: &self.narrowphase_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        });

        let dispatcher = ComputeDispatcher::new(ctx);
        let workgroups = compute_workgroup_count(pair_count, WORKGROUP_SIZE);
        dispatcher.dispatch(
            &self.narrowphase_pipeline,
            &[&data_bind_group, &params_bind_group],
            [workgroups, 1, 1],
            Some("narrowphase direct"),
        );
    }

    /// Read back narrowphase results from the GPU.
    pub fn readback_narrowphase(
        &self,
        ctx: &WgpuContext,
        pair_count: u32,
    ) -> Vec<NarrowphaseResult> {
        if pair_count == 0 {
            return Vec::new();
        }

        let read_size = (pair_count as usize * size_of::<NarrowphaseResult>()) as u64;
        let results: Vec<NarrowphaseResult> =
            read_buffer_sync(ctx, self.narrowphase_result_buffer.buffer(), read_size);

        results
            .into_iter()
            .take(pair_count as usize)
            .filter(|r| r.has_contact != 0)
            .collect()
    }

    /// Upload rigid body data for GPU integration.
    ///
    /// Returns the body count and entity mapping.
    pub fn upload_bodies(
        &self,
        ctx: &WgpuContext,
        world: &hecs::World,
    ) -> (u32, Vec<hecs::Entity>) {
        let mut bodies = Vec::new();
        let mut entity_map = Vec::new();

        for (entity, (rb, transform)) in
            &mut world.query::<(&RigidBody, &crate::ecs::components::transform::Transform)>()
        {
            let body_type = match rb.body_type {
                RigidBodyType::Dynamic => 0_u32,
                RigidBodyType::Static => 1,
                RigidBodyType::Kinematic => 2,
            };

            bodies.push(GpuBody {
                position: transform.position.into(),
                body_type,
                linear_velocity: rb.linear_velocity.into(),
                mass: rb.mass,
                angular_velocity: rb.angular_velocity.into(),
                gravity_scale: rb.gravity_scale,
                force_accumulator: rb.force_accumulator.into(),
                linear_damping: rb.linear_damping,
                torque_accumulator: rb.torque_accumulator.into(),
                angular_damping: rb.angular_damping,
                inertia_diag: [
                    rb.inertia_tensor[0],
                    rb.inertia_tensor[4],
                    rb.inertia_tensor[8],
                ],
                padding: 0.0,
                rotation: [
                    transform.rotation.x,
                    transform.rotation.y,
                    transform.rotation.z,
                    transform.rotation.w,
                ],
            });
            entity_map.push(entity);
        }

        if !bodies.is_empty() && bodies.len() <= self.max_bodies {
            self.body_buffer.write(ctx, &bodies);
        }

        (bodies.len() as u32, entity_map)
    }

    /// Dispatch GPU velocity integration.
    pub fn dispatch_integrate_velocities(
        &self,
        ctx: &WgpuContext,
        body_count: u32,
        dt: f32,
        gravity: Vec3,
    ) {
        if body_count == 0 {
            return;
        }

        let params = IntegrateParams {
            num_bodies: body_count,
            dt,
            gravity_x: gravity.x,
            gravity_y: gravity.y,
            gravity_z: gravity.z,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };

        self.dispatch_integrate(ctx, &self.integrate_vel_pipeline, body_count, &params);
    }

    /// Dispatch GPU position integration.
    pub fn dispatch_integrate_positions(&self, ctx: &WgpuContext, body_count: u32, dt: f32) {
        if body_count == 0 {
            return;
        }

        let params = IntegrateParams {
            num_bodies: body_count,
            dt,
            gravity_x: 0.0,
            gravity_y: 0.0,
            gravity_z: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };

        self.dispatch_integrate(ctx, &self.integrate_pos_pipeline, body_count, &params);
    }

    fn dispatch_integrate(
        &self,
        ctx: &WgpuContext,
        pipeline: &wgpu::ComputePipeline,
        body_count: u32,
        params: &IntegrateParams,
    ) {
        let params_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("integrate params"),
                contents: bytemuck::bytes_of(params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let data_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("integrate data"),
            layout: &self.integrate_data_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.body_buffer.buffer().as_entire_binding(),
            }],
        });

        let params_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("integrate params"),
            layout: &self.integrate_params_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        });

        let dispatcher = ComputeDispatcher::new(ctx);
        let workgroups = compute_workgroup_count(body_count, WORKGROUP_SIZE);
        dispatcher.dispatch(
            pipeline,
            &[&data_bind_group, &params_bind_group],
            [workgroups, 1, 1],
            Some("integrate"),
        );
    }

    /// Read back integrated body data and sync to the ECS world.
    pub fn download_bodies(
        &self,
        ctx: &WgpuContext,
        world: &mut hecs::World,
        body_count: u32,
        entity_map: &[hecs::Entity],
    ) {
        if body_count == 0 {
            return;
        }

        let read_size = (body_count as usize * size_of::<GpuBody>()) as u64;
        let gpu_bodies: Vec<GpuBody> = read_buffer_sync(ctx, self.body_buffer.buffer(), read_size);

        for (gpu_body, entity) in gpu_bodies.iter().zip(entity_map.iter()) {
            if let Ok((rb, transform)) = world.query_one_mut::<(
                &mut RigidBody,
                &mut crate::ecs::components::transform::Transform,
            )>(*entity)
            {
                // Sync back velocities and forces
                rb.linear_velocity = Vec3::from(gpu_body.linear_velocity);
                rb.angular_velocity = Vec3::from(gpu_body.angular_velocity);
                rb.force_accumulator = Vec3::from(gpu_body.force_accumulator);
                rb.torque_accumulator = Vec3::from(gpu_body.torque_accumulator);

                // Sync back position and rotation
                transform.position = Vec3::from(gpu_body.position);
                transform.rotation = glam::Quat::from_xyzw(
                    gpu_body.rotation[0],
                    gpu_body.rotation[1],
                    gpu_body.rotation[2],
                    gpu_body.rotation[3],
                );
            }
        }
    }

    /// Get a reference to the pair count buffer (for direct narrowphase path).
    pub fn pair_count_buffer(&self) -> &wgpu::Buffer {
        self.pair_count_buffer.buffer()
    }

    /// Check if GPU offload should be used based on body count.
    pub fn should_use_gpu(body_count: usize) -> bool {
        body_count >= GPU_BODY_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_aabb_layout() {
        assert_eq!(size_of::<GpuAabb>(), 32);
    }

    #[test]
    fn test_collision_pair_layout() {
        assert_eq!(size_of::<CollisionPair>(), 8);
    }

    #[test]
    fn test_gpu_body_layout() {
        assert_eq!(size_of::<GpuBody>(), 112);
    }

    #[test]
    fn test_broadphase_params_layout() {
        assert_eq!(size_of::<BroadphaseParams>(), 16);
    }

    #[test]
    fn test_integrate_params_layout() {
        assert_eq!(size_of::<IntegrateParams>(), 32);
    }

    #[test]
    fn test_gpu_threshold() {
        assert!(!GpuPhysics::should_use_gpu(100));
        assert!(GpuPhysics::should_use_gpu(256));
        assert!(GpuPhysics::should_use_gpu(1000));
    }

    #[test]
    fn test_narrowphase_result_layout() {
        // Must match WGSL struct with vec3<f32> alignment (48 bytes, not 40)
        assert_eq!(size_of::<NarrowphaseResult>(), 48);
    }

    #[test]
    fn test_gpu_shape_data_layout() {
        // Must match WGSL ShapeData struct (80 bytes)
        assert_eq!(size_of::<GpuShapeData>(), 80);
    }
}
