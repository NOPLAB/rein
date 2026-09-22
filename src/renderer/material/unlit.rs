//! Unlit material for UI elements and debug visualization

use super::common::{uniform_binding, uniform_layout};
use super::traits::{Material, ModelUniform};
use crate::context::WgpuContext;
use crate::core::buffer::RawUniformBuffer;
use crate::core::pipeline::{PipelineBuilder, Vertex};
use crate::core::render_states::{BlendState, CullState, DepthState};
use crate::renderer::light::Light;
use crate::renderer::viewer::{CameraUniform, Viewer};
use glam::Mat4;

/// Unlit material that renders vertex colors without lighting.
pub struct UnlitMaterial {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: RawUniformBuffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: RawUniformBuffer,
    model_bind_group: wgpu::BindGroup,
}

impl UnlitMaterial {
    /// Create a new unlit material.
    pub fn new(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Self::with_options(ctx, format, BlendState::Opaque, DepthState::read_write())
    }

    /// Create a new unlit material with alpha blending.
    pub fn with_alpha(ctx: &WgpuContext, format: wgpu::TextureFormat) -> anyhow::Result<Self> {
        Self::with_options(ctx, format, BlendState::Alpha, DepthState::read_only())
    }

    /// Create a new unlit material with custom blend and depth states.
    pub fn with_options(
        ctx: &WgpuContext,
        format: wgpu::TextureFormat,
        blend: BlendState,
        depth: DepthState,
    ) -> anyhow::Result<Self> {
        let shader = include_str!("../../shaders/unlit.wgsl");

        // Camera bind group layout (group 0)
        let camera_bind_group_layout = uniform_layout(
            ctx,
            "unlit camera bind group layout",
            wgpu::ShaderStages::VERTEX,
        );

        // Model bind group layout (group 1)
        let model_bind_group_layout = uniform_layout(
            ctx,
            "unlit model bind group layout",
            wgpu::ShaderStages::VERTEX,
        );

        let pipeline = PipelineBuilder::new(ctx)
            .label("unlit material pipeline")
            .shader(shader)
            .vertex_layout(Vertex::layout())
            .bind_group_layout(&camera_bind_group_layout)
            .bind_group_layout(&model_bind_group_layout)
            .color_format(format)
            .depth(depth)
            .blend(blend)
            .cull(CullState::Back)
            .build()?;

        // Create camera uniform buffer
        let (camera_buffer, camera_bind_group) = uniform_binding(
            ctx,
            &camera_bind_group_layout,
            size_of::<CameraUniform>() as u64,
            "unlit camera uniform",
        );

        // Create model uniform buffer
        let (model_buffer, model_bind_group) = uniform_binding(
            ctx,
            &model_bind_group_layout,
            size_of::<ModelUniform>() as u64,
            "unlit model uniform",
        );

        Ok(Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            model_buffer,
            model_bind_group,
        })
    }
}

impl Material for UnlitMaterial {
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
