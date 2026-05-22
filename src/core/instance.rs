//! Instance buffer for instanced rendering
//!
//! Provides GPU buffer management for instance data.

use crate::context::WgpuContext;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Per-instance data for instanced rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    /// Model transform matrix (column-major).
    pub transform: [[f32; 4]; 4],
    /// Instance color (RGBA).
    pub color: [f32; 4],
}

impl InstanceData {
    /// Create a new instance with identity transform and white color.
    pub fn new() -> Self {
        Self {
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Create a new instance with the given transform and color.
    pub fn with_transform_and_color(transform: glam::Mat4, color: [f32; 4]) -> Self {
        Self {
            transform: transform.to_cols_array_2d(),
            color,
        }
    }

    /// Create a new instance with the given position and color.
    pub fn with_position_and_color(position: glam::Vec3, color: [f32; 4]) -> Self {
        Self {
            transform: glam::Mat4::from_translation(position).to_cols_array_2d(),
            color,
        }
    }

    /// Get the vertex buffer layout for instance data.
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // transform column 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // transform column 1
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // transform column 2
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // transform column 3
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

impl Default for InstanceData {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU buffer for storing instance data.
pub struct InstanceBuffer {
    buffer: wgpu::Buffer,
    count: u32,
    capacity: u32,
}

impl InstanceBuffer {
    /// Create a new instance buffer from a slice of instance data.
    pub fn new(ctx: &WgpuContext, instances: &[InstanceData], label: Option<&str>) -> Self {
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        Self {
            buffer,
            count: instances.len() as u32,
            capacity: instances.len() as u32,
        }
    }

    /// Create an empty instance buffer with the given capacity.
    pub fn with_capacity(ctx: &WgpuContext, capacity: u32, label: Option<&str>) -> Self {
        let size = (capacity as usize * size_of::<InstanceData>()) as u64;
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            count: 0,
            capacity,
        }
    }

    /// Update the instance buffer with new data.
    /// If the new data is larger than capacity, returns an error.
    pub fn update(&mut self, ctx: &WgpuContext, instances: &[InstanceData]) -> Result<(), String> {
        if instances.len() as u32 > self.capacity {
            return Err(format!(
                "Instance count {} exceeds buffer capacity {}",
                instances.len(),
                self.capacity
            ));
        }

        ctx.queue
            .write_buffer(&self.buffer, 0, bytemuck::cast_slice(instances));
        self.count = instances.len() as u32;
        Ok(())
    }

    /// Get the raw wgpu buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the number of instances.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get the buffer capacity.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Create a buffer slice for the entire buffer.
    pub fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_data_size() {
        // 16 floats for transform + 4 floats for color = 20 floats * 4 bytes = 80 bytes
        assert_eq!(size_of::<InstanceData>(), 80);
    }

    #[test]
    fn test_instance_data_default() {
        let instance = InstanceData::new();
        // Check identity matrix
        assert_eq!(instance.transform[0][0], 1.0);
        assert_eq!(instance.transform[1][1], 1.0);
        assert_eq!(instance.transform[2][2], 1.0);
        assert_eq!(instance.transform[3][3], 1.0);
        // Check white color
        assert_eq!(instance.color, [1.0, 1.0, 1.0, 1.0]);
    }
}
