//! Extended vertex types
//!
//! Provides various vertex types for different rendering needs.

use bytemuck::{Pod, Zeroable};

/// Vertex with position, normal, UV, and color.
/// Used for full-featured meshes with texturing support.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexPNUC {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl VertexPNUC {
    pub const fn new(position: [f32; 3], normal: [f32; 3], uv: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            position,
            normal,
            uv,
            color,
        }
    }

    /// Get the vertex buffer layout for this vertex type.
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Vertex with position and normal only.
/// Used for meshes that don't need per-vertex color or UVs.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexPN {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl VertexPN {
    pub const fn new(position: [f32; 3], normal: [f32; 3]) -> Self {
        Self { position, normal }
    }

    /// Get the vertex buffer layout for this vertex type.
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Vertex with position and color only.
/// Used for line rendering and simple colored geometry.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexPC {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl VertexPC {
    pub const fn new(position: [f32; 3], color: [f32; 4]) -> Self {
        Self { position, color }
    }

    /// Create a vertex with RGB color (alpha = 1.0).
    pub const fn from_rgb(position: [f32; 3], color: [f32; 3]) -> Self {
        Self {
            position,
            color: [color[0], color[1], color[2], 1.0],
        }
    }

    /// Get the vertex buffer layout for this vertex type.
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // color
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Vertex with position only.
/// Used for shadow map rendering and depth-only passes.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexP {
    pub position: [f32; 3],
}

impl VertexP {
    pub const fn new(position: [f32; 3]) -> Self {
        Self { position }
    }

    /// Get the vertex buffer layout for this vertex type.
    pub const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_sizes() {
        assert_eq!(size_of::<VertexPNUC>(), 48); // 3+3+2+4 floats = 12 floats * 4 bytes
        assert_eq!(size_of::<VertexPN>(), 24); // 3+3 floats = 6 floats * 4 bytes
        assert_eq!(size_of::<VertexPC>(), 28); // 3+4 floats = 7 floats * 4 bytes
        assert_eq!(size_of::<VertexP>(), 12); // 3 floats = 3 floats * 4 bytes
    }

    #[test]
    fn test_vertex_pc_from_rgb() {
        let v = VertexPC::from_rgb([1.0, 2.0, 3.0], [0.5, 0.6, 0.7]);
        assert_eq!(v.color[3], 1.0);
    }
}
