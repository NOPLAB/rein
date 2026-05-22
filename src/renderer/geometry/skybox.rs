//! Skybox geometry
//!
//! Provides a skybox rendered as an inverted cube with gradient sky colors.

use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::renderer::viewer::Viewer;
use glam::Mat4;
use wgpu::util::DeviceExt;

/// Uniform data for the skybox shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyboxUniform {
    view_proj_no_translation: [[f32; 4]; 4],
    color_top: [f32; 4],
    color_horizon: [f32; 4],
    color_bottom: [f32; 4],
}

/// Vertex type for skybox (position only).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyboxVertex {
    position: [f32; 3],
}

impl SkyboxVertex {
    const fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

/// A skybox that renders a gradient sky background.
///
/// Inspired by three-d's Skybox, but uses a procedural gradient instead of a cubemap texture.
/// The skybox is rendered as an inverted cube that always appears behind all other geometry.
pub struct Skybox {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform_buffer: RawUniformBuffer,
    bind_group: wgpu::BindGroup,

    /// Top sky color (zenith). RGBA.
    pub color_top: [f32; 4],
    /// Horizon sky color. RGBA.
    pub color_horizon: [f32; 4],
    /// Bottom sky color (nadir). RGBA.
    pub color_bottom: [f32; 4],
}

impl Skybox {
    /// Create a new skybox with default sky colors.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Self::with_colors(
            ctx,
            format,
            [0.1, 0.3, 0.8, 1.0],  // deep blue top
            [0.6, 0.7, 0.9, 1.0],  // light blue horizon
            [0.3, 0.25, 0.2, 1.0], // brownish bottom
        )
    }

    /// Create a new skybox with custom gradient colors.
    pub fn with_colors(
        ctx: &WgpuContext,
        format: wgpu::TextureFormat,
        color_top: [f32; 4],
        color_horizon: [f32; 4],
        color_bottom: [f32; 4],
    ) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/skybox.wgsl");

        // Create cube geometry
        let (vertices, indices) = Self::cube_geometry();

        let vertex_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("skybox vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("skybox index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let index_count = indices.len() as u32;

        // Uniform buffer
        let uniform_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<SkyboxUniform>() as u64,
            Some("skybox uniform"),
        );

        let bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("skybox bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
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
            label: Some("skybox bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.buffer().as_entire_binding(),
            }],
        });

        // Pipeline: no depth write, render inside the cube (front face culling)
        let pipeline = PipelineBuilder::new(ctx)
            .label("skybox pipeline")
            .shader(shader)
            .vertex_layout(SkyboxVertex::layout())
            .bind_group_layout(&bind_group_layout)
            .color_format(format)
            .depth(DepthState::read_only())
            .blend(BlendState::Opaque)
            .cull(CullState::Front)
            .build()?;

        Ok(Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            uniform_buffer,
            bind_group,
            color_top,
            color_horizon,
            color_bottom,
        })
    }

    /// Render the skybox.
    pub fn render<'a>(
        &'a self,
        ctx: &WgpuContext,
        viewer: &dyn Viewer,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        // Create view-projection matrix without translation
        let view = viewer.view_matrix();
        // Zero out translation (keep rotation only)
        let view_no_translate = Mat4::from_cols(
            view.col(0),
            view.col(1),
            view.col(2),
            glam::Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let vp = viewer.projection_matrix() * view_no_translate;

        let uniform = SkyboxUniform {
            view_proj_no_translation: vp.to_cols_array_2d(),
            color_top: self.color_top,
            color_horizon: self.color_horizon,
            color_bottom: self.color_bottom,
        };
        self.uniform_buffer.write(ctx, &uniform);

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    /// Generate a unit cube (positions only).
    fn cube_geometry() -> (Vec<SkyboxVertex>, Vec<u16>) {
        let vertices = vec![
            SkyboxVertex {
                position: [-1.0, -1.0, -1.0],
            },
            SkyboxVertex {
                position: [1.0, -1.0, -1.0],
            },
            SkyboxVertex {
                position: [1.0, 1.0, -1.0],
            },
            SkyboxVertex {
                position: [-1.0, 1.0, -1.0],
            },
            SkyboxVertex {
                position: [-1.0, -1.0, 1.0],
            },
            SkyboxVertex {
                position: [1.0, -1.0, 1.0],
            },
            SkyboxVertex {
                position: [1.0, 1.0, 1.0],
            },
            SkyboxVertex {
                position: [-1.0, 1.0, 1.0],
            },
        ];

        #[rustfmt::skip]
        let indices: Vec<u16> = vec![
            // Front
            0, 1, 2, 0, 2, 3,
            // Back
            5, 4, 7, 5, 7, 6,
            // Left
            4, 0, 3, 4, 3, 7,
            // Right
            1, 5, 6, 1, 6, 2,
            // Top
            3, 2, 6, 3, 6, 7,
            // Bottom
            4, 5, 1, 4, 1, 0,
        ];

        (vertices, indices)
    }
}
