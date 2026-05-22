//! Line material for rendering lines and axes

use super::traits::ModelUniform;
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::PipelineBuilder;
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::core::vertex::VertexPC;
use crate::renderer::viewer::{CameraUniform, Viewer};
use glam::Mat4;

/// Material for rendering lines with per-vertex colors.
pub struct LineMaterial {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: RawUniformBuffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: RawUniformBuffer,
    model_bind_group: wgpu::BindGroup,
}

impl LineMaterial {
    /// Create a new line material.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Self::with_options(ctx, format, BlendState::Alpha, DepthState::read_write())
    }

    /// Create a new line material that ignores depth (always visible).
    pub fn no_depth(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Self::with_options(ctx, format, BlendState::Alpha, DepthState::none())
    }

    /// Create a new line material with custom options.
    pub fn with_options(
        ctx: &WgpuContext,
        format: wgpu::TextureFormat,
        blend: BlendState,
        depth: DepthState,
    ) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/line.wgsl");

        // Camera bind group layout (group 0)
        let camera_bind_group_layout =
            ctx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("line camera bind group layout"),
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
                    label: Some("line model bind group layout"),
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
            .label("line material pipeline")
            .shader(shader)
            .vertex_layout(VertexPC::layout())
            .bind_group_layout(&camera_bind_group_layout)
            .bind_group_layout(&model_bind_group_layout)
            .color_format(format)
            .depth(depth)
            .blend(blend)
            .cull(CullState::None)
            .topology(wgpu::PrimitiveTopology::LineList)
            .build()?;

        // Create camera uniform buffer
        let camera_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<CameraUniform>() as u64,
            Some("line camera uniform"),
        );

        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.buffer().as_entire_binding(),
            }],
        });

        // Create model uniform buffer
        let model_buffer = RawUniformBuffer::new(
            ctx,
            size_of::<ModelUniform>() as u64,
            Some("line model uniform"),
        );

        let model_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line model bind group"),
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

    /// Update uniforms before rendering.
    pub fn update_uniforms(&self, ctx: &WgpuContext, viewer: &dyn Viewer, model_matrix: Mat4) {
        let camera_uniform = CameraUniform::from_viewer(viewer);
        self.camera_buffer.write(ctx, &camera_uniform);

        let model_uniform = ModelUniform::from_matrix(model_matrix);
        self.model_buffer.write(ctx, &model_uniform);
    }

    /// Get the render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Get the camera bind group.
    pub fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    /// Get the model bind group.
    pub fn model_bind_group(&self) -> &wgpu::BindGroup {
        &self.model_bind_group
    }
}
