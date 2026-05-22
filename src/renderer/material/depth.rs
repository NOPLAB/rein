//! Depth material for shadow map generation

use super::traits::ModelUniform;
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{CullState, DepthState};
use crate::core::vertex::VertexP;
use glam::Mat4;

/// Light view-projection uniform for shadow mapping.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct LightMatrixUniform {
    pub light_view_proj: [[f32; 4]; 4],
}

/// Depth-only material for shadow map generation.
pub struct DepthMaterial {
    pipeline: wgpu::RenderPipeline,
    light_buffer: RawUniformBuffer,
    light_bind_group: wgpu::BindGroup,
    model_buffer: RawUniformBuffer,
    model_bind_group: wgpu::BindGroup,
}

impl DepthMaterial {
    /// Create a new depth material.
    pub fn new(ctx: &WgpuContext) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/depth.wgsl");

        // Light matrix bind group layout (group 0)
        let light_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("depth light bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        // Model bind group layout (group 1)
        let model_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("depth model bind group layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let pipeline = PipelineBuilder::new(ctx)
            .label("depth material pipeline")
            .shader(shader)
            .vertex_layout(VertexP::layout())
            .bind_group_layout(&light_bind_group_layout)
            .bind_group_layout(&model_bind_group_layout)
            .color_format(wgpu::TextureFormat::R8Unorm) // Dummy format, no color output
            .depth(DepthState::read_write())
            .cull(CullState::Back)
            .build()?;

        // Create light matrix buffer
        let light_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<LightMatrixUniform>() as u64,
            Some("depth light uniform"),
        );

        let light_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("depth light bind group"),
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.buffer().as_entire_binding(),
            }],
        });

        // Create model buffer
        let model_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<ModelUniform>() as u64,
            Some("depth model uniform"),
        );

        let model_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("depth model bind group"),
            layout: &model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.buffer().as_entire_binding(),
            }],
        });

        Ok(Self {
            pipeline,
            light_buffer,
            light_bind_group,
            model_buffer,
            model_bind_group,
        })
    }

    /// Update uniforms for shadow pass.
    pub fn update_uniforms(&self, ctx: &WgpuContext, light_view_proj: Mat4, model_matrix: Mat4) {
        let light_uniform = LightMatrixUniform {
            light_view_proj: light_view_proj.to_cols_array_2d(),
        };
        self.light_buffer.write(ctx, &light_uniform);

        let model_uniform = ModelUniform::from_matrix(model_matrix);
        self.model_buffer.write(ctx, &model_uniform);
    }

    /// Get the render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Get the light matrix bind group.
    pub fn light_bind_group(&self) -> &wgpu::BindGroup {
        &self.light_bind_group
    }

    /// Get the model bind group.
    pub fn model_bind_group(&self) -> &wgpu::BindGroup {
        &self.model_bind_group
    }
}
