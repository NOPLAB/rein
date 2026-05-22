//! GPU buffer abstractions
//!
//! Provides typed wrappers for vertex, index, and uniform buffers.

use crate::context::WgpuContext;
use bytemuck::{Pod, Zeroable};
use std::marker::PhantomData;

/// A GPU buffer containing vertex data.
pub struct VertexBuffer {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) count: u32,
    pub(crate) stride: u64,
}

impl VertexBuffer {
    /// Create a new vertex buffer from a slice of vertices.
    pub fn new<V: Pod + Zeroable>(ctx: &WgpuContext, vertices: &[V], label: Option<&str>) -> Self {
        use wgpu::util::DeviceExt;
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        Self {
            buffer,
            count: vertices.len() as u32,
            stride: size_of::<V>() as u64,
        }
    }

    /// Get the raw wgpu buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the number of vertices.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get the stride (size of one vertex in bytes).
    pub fn stride(&self) -> u64 {
        self.stride
    }

    /// Create a buffer slice for the entire buffer.
    pub fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }
}

/// A GPU buffer containing index data.
pub struct IndexBuffer {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) count: u32,
    pub(crate) format: wgpu::IndexFormat,
}

impl IndexBuffer {
    /// Create a new index buffer from u16 indices.
    pub fn new_u16(ctx: &WgpuContext, indices: &[u16], label: Option<&str>) -> Self {
        use wgpu::util::DeviceExt;
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        Self {
            buffer,
            count: indices.len() as u32,
            format: wgpu::IndexFormat::Uint16,
        }
    }

    /// Create a new index buffer from u32 indices.
    pub fn new_u32(ctx: &WgpuContext, indices: &[u32], label: Option<&str>) -> Self {
        use wgpu::util::DeviceExt;
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        Self {
            buffer,
            count: indices.len() as u32,
            format: wgpu::IndexFormat::Uint32,
        }
    }

    /// Get the raw wgpu buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the number of indices.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Get the index format (Uint16 or Uint32).
    pub fn format(&self) -> wgpu::IndexFormat {
        self.format
    }

    /// Create a buffer slice for the entire buffer.
    pub fn slice(&self) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..)
    }
}

/// A typed GPU uniform buffer.
pub struct UniformBuffer<T> {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
    _marker: PhantomData<T>,
}

impl<T: Pod + Zeroable> UniformBuffer<T> {
    /// Create a new uniform buffer with initial data.
    pub fn new(ctx: &WgpuContext, data: &T, binding: u32, label: Option<&str>) -> Self {
        use wgpu::util::DeviceExt;

        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::bytes_of(data),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: label.map(|l| format!("{l} layout")).as_deref(),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: label.map(|l| format!("{l} bind group")).as_deref(),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            bind_group_layout,
            bind_group,
            _marker: PhantomData,
        }
    }

    /// Update the buffer contents.
    pub fn update(&self, ctx: &WgpuContext, data: &T) {
        ctx.queue
            .write_buffer(&self.buffer, 0, bytemuck::bytes_of(data));
    }

    /// Get the raw wgpu buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the bind group layout.
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the bind group.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

/// Raw uniform buffer without type information (for dynamic usage).
pub struct RawUniformBuffer {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) size: u64,
}

impl RawUniformBuffer {
    /// Create a new raw uniform buffer with specified size.
    pub fn new(ctx: &WgpuContext, size: u64, label: Option<&str>) -> Self {
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self { buffer, size }
    }

    /// Write data to the buffer.
    pub fn write<T: Pod>(&self, ctx: &WgpuContext, data: &T) {
        ctx.queue
            .write_buffer(&self.buffer, 0, bytemuck::bytes_of(data));
    }

    /// Get the raw wgpu buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the buffer size.
    pub fn size(&self) -> u64 {
        self.size
    }
}

/// GPU storage buffer for compute shader read/write operations.
///
/// Used for compute shader read/write data. The buffer is created with
/// `STORAGE | COPY_DST | COPY_SRC` usage flags.
///
/// Unlike `UniformBuffer`, `StorageBuffer` does not manage its own bind group.
/// Bind group layout and bind group creation is left to the caller,
/// following the same pattern as `RawUniformBuffer`.
pub struct StorageBuffer {
    buffer: wgpu::Buffer,
    size: u64,
}

impl StorageBuffer {
    /// Create a new storage buffer with specified size.
    pub fn new(ctx: &WgpuContext, size: u64, label: Option<&str>) -> Self {
        let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Self { buffer, size }
    }

    /// Create a storage buffer initialized with typed data.
    pub fn from_data<T: Pod>(ctx: &WgpuContext, data: &[T], label: Option<&str>) -> Self {
        use wgpu::util::DeviceExt;
        let contents = bytemuck::cast_slice(data);
        let buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            });

        Self {
            size: contents.len() as u64,
            buffer,
        }
    }

    /// Write typed data to the buffer.
    pub fn write<T: Pod>(&self, ctx: &WgpuContext, data: &[T]) {
        ctx.queue
            .write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }

    /// Get the buffer size in bytes.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get the raw wgpu buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::WgpuContext;

    /// Try to create a GPU context for testing. Returns None if no GPU is
    /// available or if the `REIN_SKIP_GPU_TESTS` environment variable is set.
    fn try_create_ctx() -> Option<WgpuContext> {
        if std::env::var("REIN_SKIP_GPU_TESTS").is_ok() {
            return None;
        }
        WgpuContext::new_blocking(None).ok()
    }

    #[test]
    fn test_storage_buffer_new() {
        let Some(ctx) = try_create_ctx() else {
            eprintln!("Skipping: no GPU device available (set REIN_SKIP_GPU_TESTS to skip)");
            return;
        };
        let buf = StorageBuffer::new(&ctx, 256, Some("test"));
        assert_eq!(buf.size(), 256);
    }

    #[test]
    fn test_storage_buffer_from_data() {
        let Some(ctx) = try_create_ctx() else {
            eprintln!("Skipping: no GPU device available");
            return;
        };
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let buf = StorageBuffer::from_data(&ctx, &data, Some("test"));
        assert_eq!(buf.size(), (4 * size_of::<f32>()) as u64);
    }

    #[test]
    fn test_storage_buffer_write() {
        let Some(ctx) = try_create_ctx() else {
            eprintln!("Skipping: no GPU device available");
            return;
        };
        let buf = StorageBuffer::new(&ctx, 64, Some("test"));
        let data: Vec<u32> = vec![10, 20, 30, 40];
        buf.write(&ctx, &data);
        assert_eq!(buf.size(), 64);
    }
}
