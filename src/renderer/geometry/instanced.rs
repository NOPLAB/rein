//! Instanced mesh rendering
//!
//! Provides efficient rendering of many instances of the same mesh.

use super::{Aabb, Geometry, Mesh};
use crate::context::WgpuContext;
use crate::core::buffer::{IndexBuffer, VertexBuffer};
use crate::core::instance::{InstanceBuffer, InstanceData};
use glam::{Mat4, Vec3};

/// A mesh rendered multiple times with different transforms and colors.
pub struct InstancedMesh {
    mesh: Mesh,
    instance_buffer: InstanceBuffer,
    aabb: Aabb,
}

impl InstancedMesh {
    /// Create an instanced mesh from a base mesh and instance data.
    pub fn new(ctx: &WgpuContext, mesh: Mesh, instances: &[InstanceData]) -> Self {
        let instance_buffer = InstanceBuffer::new(ctx, instances, Some("instance buffer"));

        // Calculate combined AABB from all instances
        let mesh_aabb = mesh.aabb();
        let aabb = Self::calculate_combined_aabb(&mesh_aabb, instances);

        Self {
            mesh,
            instance_buffer,
            aabb,
        }
    }

    /// Create an instanced mesh with a pre-allocated capacity.
    pub fn with_capacity(ctx: &WgpuContext, mesh: Mesh, capacity: u32) -> Self {
        let instance_buffer = InstanceBuffer::with_capacity(ctx, capacity, Some("instance buffer"));
        let aabb = mesh.aabb();

        Self {
            mesh,
            instance_buffer,
            aabb,
        }
    }

    /// Update the instance data.
    pub fn update_instances(
        &mut self,
        ctx: &WgpuContext,
        instances: &[InstanceData],
    ) -> Result<(), String> {
        self.instance_buffer.update(ctx, instances)?;
        self.aabb = Self::calculate_combined_aabb(&self.mesh.aabb(), instances);
        Ok(())
    }

    /// Get the number of instances.
    pub fn instance_count(&self) -> u32 {
        self.instance_buffer.count()
    }

    /// Get the instance buffer.
    pub fn instance_buffer(&self) -> &InstanceBuffer {
        &self.instance_buffer
    }

    /// Get the underlying mesh.
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    fn calculate_combined_aabb(mesh_aabb: &Aabb, instances: &[InstanceData]) -> Aabb {
        if instances.is_empty() {
            return *mesh_aabb;
        }

        let corners = mesh_aabb.corners();
        let mut combined_min = Vec3::splat(f32::MAX);
        let mut combined_max = Vec3::splat(f32::MIN);

        for instance in instances {
            let transform = Mat4::from_cols_array_2d(&instance.transform);
            for corner in &corners {
                let world_corner = transform.transform_point3(*corner);
                combined_min = combined_min.min(world_corner);
                combined_max = combined_max.max(world_corner);
            }
        }

        Aabb::new(combined_min, combined_max)
    }
}

impl Geometry for InstancedMesh {
    fn vertex_buffer(&self) -> &VertexBuffer {
        self.mesh.vertex_buffer()
    }

    fn index_buffer(&self) -> Option<&IndexBuffer> {
        self.mesh.index_buffer()
    }

    fn draw_count(&self) -> u32 {
        self.mesh.draw_count()
    }

    fn aabb(&self) -> Aabb {
        self.aabb
    }
}

/// Builder for creating instanced meshes from positions.
#[allow(dead_code, reason = "scaffolding for upcoming instancing API; kept compiled to prevent bitrot")]
pub(super) struct InstancedMeshBuilder {
    positions: Vec<Vec3>,
    colors: Vec<[f32; 4]>,
    transforms: Vec<Mat4>,
}

#[allow(dead_code, reason = "scaffolding for upcoming instancing API; kept compiled to prevent bitrot")]
impl InstancedMeshBuilder {
    /// Create a new builder.
    pub(super) fn new() -> Self {
        Self {
            positions: Vec::new(),
            colors: Vec::new(),
            transforms: Vec::new(),
        }
    }

    /// Add an instance at a position with white color.
    pub(super) fn add_position(mut self, position: Vec3) -> Self {
        self.positions.push(position);
        self.colors.push([1.0, 1.0, 1.0, 1.0]);
        self.transforms.push(Mat4::from_translation(position));
        self
    }

    /// Add an instance at a position with a specific color.
    pub(super) fn add_position_with_color(mut self, position: Vec3, color: [f32; 4]) -> Self {
        self.positions.push(position);
        self.colors.push(color);
        self.transforms.push(Mat4::from_translation(position));
        self
    }

    /// Add an instance with a full transform.
    pub(super) fn add_transform(mut self, transform: Mat4, color: [f32; 4]) -> Self {
        self.transforms.push(transform);
        self.colors.push(color);
        self.positions.push(transform.transform_point3(Vec3::ZERO));
        self
    }

    /// Build the instance data.
    pub(super) fn build(self) -> Vec<InstanceData> {
        self.transforms
            .into_iter()
            .zip(self.colors)
            .map(|(transform, color)| InstanceData::with_transform_and_color(transform, color))
            .collect()
    }
}

impl Default for InstancedMeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}
