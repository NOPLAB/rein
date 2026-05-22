//! Terrain geometry with height-map based generation
//!
//! Provides terrain mesh generation from a height function with height-based coloring.

use super::{Aabb, Geometry};
use crate::context::WgpuContext;
use crate::core::buffer::{IndexBuffer, VertexBuffer};
use crate::core::pipeline::Vertex;
use glam::Vec3;

/// Level of detail for terrain generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainLod {
    /// High detail: full resolution.
    High,
    /// Medium detail: half resolution.
    Medium,
    /// Low detail: quarter resolution.
    Low,
}

impl TerrainLod {
    /// Get the step size multiplier for this LOD level.
    fn step_multiplier(self) -> u32 {
        match self {
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 4,
        }
    }
}

/// A terrain mesh generated from a height function.
///
/// Inspired by three-d's Terrain, this generates a height-map based terrain mesh
/// with height-based coloring and basic LOD support.
pub struct Terrain {
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,
    draw_count: u32,
    aabb: Aabb,
}

impl Terrain {
    /// Create a new terrain from a height function.
    ///
    /// - `width`: Total width of the terrain in world units.
    /// - `depth`: Total depth of the terrain in world units.
    /// - `resolution`: Number of vertices along each axis.
    /// - `height_fn`: Function that returns height given (x, z) world coordinates.
    /// - `lod`: Level of detail.
    pub fn new(
        ctx: &WgpuContext,
        width: f32,
        depth: f32,
        resolution: u32,
        height_fn: &dyn Fn(f32, f32) -> f32,
        lod: TerrainLod,
        color: [f32; 3],
    ) -> Self {
        let step = lod.step_multiplier();
        let effective_res = (resolution / step).max(2);

        let (vertices, indices, aabb) =
            Self::generate(width, depth, effective_res, height_fn, color);

        let vertex_buffer = VertexBuffer::new(ctx, &vertices, Some("terrain vertices"));
        let index_buffer = IndexBuffer::new_u32(ctx, &indices, Some("terrain indices"));
        let draw_count = indices.len() as u32;

        Self {
            vertex_buffer,
            index_buffer,
            draw_count,
            aabb,
        }
    }

    /// Create terrain from a heightmap array.
    ///
    /// - `width`, `depth`: World-space dimensions.
    /// - `heights`: Row-major heightmap of size `res_x * res_z`.
    /// - `res_x`, `res_z`: Resolution of the heightmap.
    pub fn from_heightmap(
        ctx: &WgpuContext,
        width: f32,
        depth: f32,
        heights: &[f32],
        res_x: u32,
        res_z: u32,
        color: [f32; 3],
    ) -> Self {
        let height_fn = |x: f32, z: f32| -> f32 {
            let u = (x / width + 0.5).clamp(0.0, 1.0);
            let v = (z / depth + 0.5).clamp(0.0, 1.0);
            let ix = ((u * (res_x - 1) as f32) as u32).min(res_x - 1);
            let iz = ((v * (res_z - 1) as f32) as u32).min(res_z - 1);
            heights[(iz * res_x + ix) as usize]
        };

        Self::new(
            ctx,
            width,
            depth,
            res_x.min(res_z),
            &height_fn,
            TerrainLod::High,
            color,
        )
    }

    fn generate(
        width: f32,
        depth: f32,
        resolution: u32,
        height_fn: &dyn Fn(f32, f32) -> f32,
        color: [f32; 3],
    ) -> (Vec<Vertex>, Vec<u32>, Aabb) {
        let mut vertices = Vec::with_capacity((resolution * resolution) as usize);
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        let hw = width / 2.0;
        let hd = depth / 2.0;

        // Generate vertex positions
        for iz in 0..resolution {
            for ix in 0..resolution {
                let u = ix as f32 / (resolution - 1) as f32;
                let v = iz as f32 / (resolution - 1) as f32;

                let x = -hw + u * width;
                let z = -hd + v * depth;
                let y = height_fn(x, z);

                min_y = min_y.min(y);
                max_y = max_y.max(y);

                // Normal will be calculated after all positions are known
                vertices.push(Vertex {
                    position: [x, y, z],
                    normal: [0.0, 1.0, 0.0],
                    color,
                });
            }
        }

        // Calculate normals using central differences
        for iz in 0..resolution {
            for ix in 0..resolution {
                let idx = (iz * resolution + ix) as usize;

                let left = if ix > 0 {
                    vertices[idx - 1].position[1]
                } else {
                    vertices[idx].position[1]
                };
                let right = if ix < resolution - 1 {
                    vertices[idx + 1].position[1]
                } else {
                    vertices[idx].position[1]
                };
                let down = if iz > 0 {
                    vertices[(idx as u32 - resolution) as usize].position[1]
                } else {
                    vertices[idx].position[1]
                };
                let up_val = if iz < resolution - 1 {
                    vertices[(idx as u32 + resolution) as usize].position[1]
                } else {
                    vertices[idx].position[1]
                };

                let dx = width / (resolution - 1) as f32;
                let dz = depth / (resolution - 1) as f32;

                let normal = Vec3::new(
                    (left - right) / (2.0 * dx),
                    1.0,
                    (down - up_val) / (2.0 * dz),
                )
                .normalize();

                vertices[idx].normal = [normal.x, normal.y, normal.z];
            }
        }

        // Generate indices
        let mut indices = Vec::with_capacity(((resolution - 1) * (resolution - 1) * 6) as usize);
        for iz in 0..resolution - 1 {
            for ix in 0..resolution - 1 {
                let top_left = iz * resolution + ix;
                let top_right = top_left + 1;
                let bottom_left = top_left + resolution;
                let bottom_right = bottom_left + 1;

                indices.push(top_left);
                indices.push(bottom_left);
                indices.push(top_right);

                indices.push(top_right);
                indices.push(bottom_left);
                indices.push(bottom_right);
            }
        }

        let aabb = Aabb::new(Vec3::new(-hw, min_y, -hd), Vec3::new(hw, max_y, hd));

        (vertices, indices, aabb)
    }
}

impl Geometry for Terrain {
    fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vertex_buffer
    }

    fn index_buffer(&self) -> Option<&IndexBuffer> {
        Some(&self.index_buffer)
    }

    fn draw_count(&self) -> u32 {
        self.draw_count
    }

    fn aabb(&self) -> Aabb {
        self.aabb
    }
}
