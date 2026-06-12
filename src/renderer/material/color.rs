//! Color material with Phong lighting

use super::traits::{Material, ModelUniform};
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::{PipelineBuilder, Vertex};
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::renderer::light::Light;
use crate::renderer::viewer::{CameraUniform, Viewer};
use glam::Mat4;

/// Simple color material with Phong lighting.
pub struct ColorMaterial {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: RawUniformBuffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: RawUniformBuffer,
    model_bind_group: wgpu::BindGroup,
}

impl ColorMaterial {
    /// Create a new color material.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/color.wgsl");

        // Camera bind group layout (group 0)
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("camera bind group layout"),
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

        // Model bind group layout (group 1)
        let model_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("model bind group layout"),
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
            .label("color material pipeline")
            .shader(shader)
            .vertex_layout(Vertex::layout())
            .bind_group_layout(&camera_bind_group_layout)
            .bind_group_layout(&model_bind_group_layout)
            .color_format(format)
            .depth(DepthState::read_write())
            .blend(BlendState::Opaque)
            .cull(CullState::Back)
            .build()?;

        // Create camera uniform buffer
        let camera_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<CameraUniform>() as u64,
            Some("camera uniform"),
        );

        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.buffer().as_entire_binding(),
            }],
        });

        // Create model uniform buffer
        let model_buffer =
            RawUniformBuffer::new(ctx, size_of::<ModelUniform>() as u64, Some("model uniform"));

        let model_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model bind group"),
            layout: &model_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_buffer.buffer().as_entire_binding(),
            }],
        });

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            model_buffer,
            model_bind_group,
        })
    }
}

impl Material for ColorMaterial {
    fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    fn model_bind_group(&self) -> &wgpu::BindGroup {
        &self.model_bind_group
    }

    fn update_uniforms(
        &self,
        ctx: &WgpuContext,
        viewer: &dyn Viewer,
        model_matrix: Mat4,
        _lights: &[&dyn Light],
    ) {
        let camera_uniform = CameraUniform::from_viewer(viewer);
        self.camera_buffer.write(ctx, &camera_uniform);

        let model_uniform = ModelUniform::from_matrix(model_matrix);
        self.model_buffer.write(ctx, &model_uniform);
    }
}
